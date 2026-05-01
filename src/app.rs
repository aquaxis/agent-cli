use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, RwLock};

use crate::agent::{Agent, AgentEvent, AgentInput};
use crate::ai;
use crate::cli::RunArgs;
use crate::config::Config;
use crate::error::Result;
use crate::id::AgentId;
use crate::ipc::registry::{RegistryEntry, RegistryHandle};
use crate::ipc::server::IpcServer;
use crate::ipc::IpcMessage;
use crate::log::ConversationLog;
use crate::persona::{self, Persona, PersonaResolution};
use crate::tools::ToolRegistry;

/// REPL コマンドハンドラから参照する共有状態。
struct ReplState {
    registry_dir: PathBuf,
    agents_dir: PathBuf,
    persona_file_setting: String,
    cli_persona_path: Option<PathBuf>,
    name: Option<String>,
    persona: RwLock<Persona>,
    tool_names: Vec<String>,
    history_path: PathBuf,
    history: RwLock<Vec<String>>,
}

const HISTORY_LIMIT: usize = 200;

fn load_history(path: &Path) -> Vec<String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mut lines: Vec<String> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect();
    let len = lines.len();
    if len > HISTORY_LIMIT {
        lines.drain(..len - HISTORY_LIMIT);
    }
    lines
}

fn append_history(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

pub async fn run(mut config: Config, args: RunArgs) -> Result<()> {
    config.apply_overrides(args.provider.as_deref(), args.model.as_deref());

    let id = AgentId::new();
    let name = args.name.clone();
    let agents_dir = config.agents_dir()?;
    let resolution: PersonaResolution = persona::resolve(
        args.persona.as_deref(),
        &config.runtime.persona_file,
        &agents_dir,
        name.as_deref(),
    )?;

    if resolution.builtin_used {
        tracing::info!("using builtin default persona");
    }

    // ペルソナ由来の model / temperature を Provider 設定へ反映
    config.apply_persona_overrides(
        resolution.persona.frontmatter.model.as_deref(),
        resolution.persona.frontmatter.temperature,
    );

    // Provider 構築（接続前検証）
    let provider = ai::build(&config)?;
    let caps = provider.capabilities();

    let registry_dir = config.registry_dir()?;
    let socket_path = registry_dir.join(format!("{}.sock", id.as_str()));

    let ipc_server = IpcServer::bind(socket_path.clone()).await?;

    let entry = RegistryEntry {
        id: id.clone(),
        name: name.clone(),
        pid: std::process::id(),
        started_at: Utc::now(),
        provider: config.provider.kind.clone(),
        model: provider.model().to_string(),
        socket: socket_path.clone(),
        persona: Some(resolution.persona.summary()),
    };
    let registry_handle = RegistryHandle::register(&registry_dir, &entry).await?;
    let registry_handle = Arc::new(registry_handle);

    // ログ
    let log = ConversationLog::open(&config.log_dir()?, &id).await?;

    // Tools
    let allowed = resolution.persona.frontmatter.allowed_tools.clone();
    let denied = resolution.persona.frontmatter.denied_tools.clone();
    let tools = ToolRegistry::build(&config, allowed.as_deref(), denied.as_deref());
    let tool_names = tools.names();

    let history = Agent::build_initial_history(&resolution.persona);
    let auto_approve = config.runtime.auto_approve_tools || args.auto_approve_tools;

    let initial_persona = resolution.persona.clone();
    let agent = Agent {
        id: id.clone(),
        name: name.clone(),
        persona: resolution.persona,
        provider,
        tools,
        config: config.clone(),
        registry_dir: registry_dir.clone(),
        log: Some(log),
        auto_approve,
        history,
    };

    let history_path = config.log_dir()?.join("history.txt");
    let initial_history = load_history(&history_path);
    let state = Arc::new(ReplState {
        registry_dir: registry_dir.clone(),
        agents_dir: agents_dir.clone(),
        persona_file_setting: config.runtime.persona_file.clone(),
        cli_persona_path: args.persona.clone(),
        name: name.clone(),
        persona: RwLock::new(initial_persona),
        tool_names,
        history_path,
        history: RwLock::new(initial_history),
    });

    let (input_tx, input_rx) = mpsc::channel::<AgentInput>(32);
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(64);

    print_header(
        &id,
        name.as_deref(),
        &config.provider.kind,
        agent.provider.model(),
        &agent.persona,
        caps,
    );

    // Agent タスク
    let agent_handle = tokio::spawn(async move { agent.run(input_rx, event_tx).await });

    // IPC 受信を AgentInput に流す
    let input_tx_for_ipc = input_tx.clone();
    let mut ipc_rx = ipc_server.rx;
    tokio::spawn(async move {
        while let Some(msg) = ipc_rx.recv().await {
            if let IpcMessage::Prompt {
                from,
                from_name,
                text,
            } = msg
            {
                let _ = input_tx_for_ipc
                    .send(AgentInput::PeerPrompt {
                        from,
                        from_name,
                        text,
                    })
                    .await;
            }
        }
    });

    // 標準入力の読み取り
    let input_tx_for_stdin = input_tx.clone();
    let state_for_stdin = state.clone();
    let stdin_task = tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();
        loop {
            print_prompt();
            let next = lines.next_line().await;
            match next {
                Ok(Some(line)) => {
                    let trimmed = line.trim_end_matches('\r').trim().to_string();
                    if let Some(rest) = trimmed.strip_prefix('/') {
                        if !handle_repl_command(rest, &input_tx_for_stdin, &state_for_stdin).await {
                            break;
                        }
                        continue;
                    }
                    if trimmed.is_empty() {
                        continue;
                    }
                    // 履歴へ保存（永続＋メモリ）
                    append_history(&state_for_stdin.history_path, &trimmed);
                    {
                        let mut h = state_for_stdin.history.write().await;
                        h.push(trimmed.clone());
                        let len = h.len();
                        if len > HISTORY_LIMIT {
                            h.drain(..len - HISTORY_LIMIT);
                        }
                    }
                    if input_tx_for_stdin
                        .send(AgentInput::UserPrompt(trimmed))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
    });

    // イベント表示
    let display_task = tokio::spawn(async move {
        while let Some(ev) = event_rx.recv().await {
            display_event(ev);
        }
    });

    let _ = stdin_task.await;
    drop(input_tx);
    let _ = agent_handle.await;
    let _ = display_task.await;

    registry_handle.cleanup();
    IpcServer::cleanup(&socket_path);
    Ok(())
}

fn print_header(
    id: &AgentId,
    name: Option<&str>,
    provider: &str,
    model: &str,
    persona: &crate::persona::Persona,
    caps: crate::ai::Capabilities,
) {
    let display_name = name.unwrap_or("(unnamed)");
    println!("agent-cli ready");
    println!("  id        : {id}");
    println!("  name      : {display_name}");
    println!("  provider  : {provider} ({model})");
    println!(
        "  features  : streaming={} tool_use={} thinking={}",
        caps.streaming, caps.tool_use, caps.thinking
    );
    println!("  role      : {}", persona.frontmatter.role);
    if !persona.frontmatter.skills.is_empty() {
        println!("  skills    : {}", persona.frontmatter.skills.join(", "));
    }
    println!("type /help for commands. ^D to exit.");
}

fn print_prompt() {
    print!("> ");
    let _ = std::io::stdout().flush();
}

fn display_event(ev: AgentEvent) {
    match ev {
        AgentEvent::Text { delta } => {
            print!("{delta}");
            let _ = std::io::stdout().flush();
        }
        AgentEvent::Thinking { text } => {
            eprintln!("\n[thinking] {text}");
        }
        AgentEvent::ToolCall { name, args } => {
            eprintln!("\n[tool-call] {name} {args}");
        }
        AgentEvent::ToolResult { name, ok, output } => {
            let mark = if ok { "ok" } else { "ERR" };
            eprintln!("[tool-result {mark}] {name}: {output}");
        }
        AgentEvent::Done => {
            println!();
        }
        AgentEvent::Error { message } => {
            eprintln!("\n[error] {message}");
        }
        AgentEvent::Info { message } => {
            eprintln!("[info] {message}");
        }
    }
}

async fn handle_repl_command(
    rest: &str,
    input_tx: &mpsc::Sender<AgentInput>,
    state: &Arc<ReplState>,
) -> bool {
    let mut parts = rest.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("").trim();
    let arg = parts.next().unwrap_or("").trim();
    match cmd {
        "quit" | "exit" => return false,
        "help" => {
            println!("/list / /send <peer> <text> / /tools / /persona / /reload-persona / /peer <id> / /history [n] / /cancel / /quit");
        }
        "history" => {
            let n: usize = arg.parse().unwrap_or(20);
            let h = state.persona.read().await;
            let _ = h; // 無関係
            let hist = state.history.read().await;
            let total = hist.len();
            let start = total.saturating_sub(n);
            for (i, line) in hist.iter().enumerate().skip(start) {
                println!("{:>4}  {}", i + 1, line);
            }
            if total == 0 {
                println!("(empty)");
            }
        }
        "list" => list_peers(&state.registry_dir),
        "send" => send_to_peer(arg, &state.registry_dir).await,
        "tools" => {
            if state.tool_names.is_empty() {
                println!("(no tools enabled)");
            } else {
                println!("tools: {}", state.tool_names.join(", "));
            }
        }
        "persona" => {
            let p = state.persona.read().await;
            print_persona(&p);
        }
        "reload-persona" => reload_persona(state, input_tx).await,
        "peer" => peer_summary(arg, &state.registry_dir),
        "cancel" => {
            let _ = input_tx.send(AgentInput::Cancel).await;
        }
        _ => {
            eprintln!("unknown command: {cmd}");
        }
    }
    true
}

fn list_peers(registry_dir: &Path) {
    match crate::ipc::registry::list_entries(registry_dir) {
        Ok(entries) => {
            if entries.is_empty() {
                println!("(no agents running)");
                return;
            }
            for e in entries {
                let role = e
                    .persona
                    .as_ref()
                    .map(|p| p.role.clone())
                    .unwrap_or_default();
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    e.id,
                    e.name.clone().unwrap_or_else(|| "-".into()),
                    e.provider,
                    e.model,
                    role
                );
            }
        }
        Err(e) => eprintln!("[error] {e}"),
    }
}

async fn send_to_peer(arg: &str, registry_dir: &Path) {
    let mut p = arg.splitn(2, ' ');
    let peer = p.next().unwrap_or("").trim();
    let text = p.next().unwrap_or("").trim();
    if peer.is_empty() || text.is_empty() {
        eprintln!("usage: /send <peer> <text>");
        return;
    }
    match crate::ipc::registry::resolve_peer(registry_dir, peer) {
        Ok(p) => {
            let msg = crate::ipc::IpcMessage::Prompt {
                from: p.id.clone(),
                from_name: None,
                text: text.to_string(),
            };
            if let Err(e) = crate::ipc::client::send(&p.socket, &msg).await {
                eprintln!("[error] {e}");
            } else {
                println!("delivered to {}", p.id);
            }
        }
        Err(e) => eprintln!("[error] {e}"),
    }
}

fn print_persona(persona: &Persona) {
    println!(
        "name        : {}",
        persona.frontmatter.name.as_deref().unwrap_or("-")
    );
    println!("role        : {}", persona.frontmatter.role);
    if !persona.frontmatter.skills.is_empty() {
        println!("skills      : {}", persona.frontmatter.skills.join(", "));
    }
    if let Some(d) = &persona.frontmatter.description {
        println!("description : {d}");
    }
    if let Some(t) = persona.frontmatter.temperature {
        println!("temperature : {t}");
    }
    if let Some(allow) = &persona.frontmatter.allowed_tools {
        println!("allowed     : {}", allow.join(", "));
    }
    if let Some(deny) = &persona.frontmatter.denied_tools {
        println!("denied      : {}", deny.join(", "));
    }
    if let Some(p) = &persona.source_path {
        println!("source      : {}", p.display());
    } else {
        println!("source      : (builtin default)");
    }
}

async fn reload_persona(state: &Arc<ReplState>, input_tx: &mpsc::Sender<AgentInput>) {
    let resolution = match persona::resolve(
        state.cli_persona_path.as_deref(),
        &state.persona_file_setting,
        &state.agents_dir,
        state.name.as_deref(),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[error] {e}");
            return;
        }
    };
    let prompt = resolution.persona.to_system_prompt();
    {
        let mut guard = state.persona.write().await;
        *guard = resolution.persona;
    }
    if let Err(e) = input_tx.send(AgentInput::SetSystemPrompt(prompt)).await {
        eprintln!("[error] {e}");
        return;
    }
    let p = state.persona.read().await;
    println!(
        "persona reloaded from {}",
        match &p.source_path {
            Some(path) => path.display().to_string(),
            None => "(builtin default)".to_string(),
        }
    );
}

fn peer_summary(arg: &str, registry_dir: &Path) {
    let key = arg.trim();
    if key.is_empty() {
        eprintln!("usage: /peer <id_or_name>");
        return;
    }
    match crate::ipc::registry::resolve_peer(registry_dir, key) {
        Ok(e) => {
            println!(
                "[{}] name={} provider={} model={}",
                e.id,
                e.name.unwrap_or_else(|| "-".into()),
                e.provider,
                e.model
            );
            if let Some(p) = &e.persona {
                println!("role        : {}", p.role);
                if !p.skills.is_empty() {
                    println!("skills      : {}", p.skills.join(", "));
                }
                if let Some(d) = &p.description {
                    println!("description : {d}");
                }
            } else {
                println!("(no persona summary)");
            }
        }
        Err(e) => eprintln!("[error] {e}"),
    }
}
