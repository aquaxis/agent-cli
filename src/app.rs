use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::{mpsc, oneshot, watch, RwLock};

use crate::agent::{Agent, AgentEvent, AgentInput, ApprovalRequest};
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
pub(crate) struct ReplState {
    registry_dir: PathBuf,
    agents_dir: PathBuf,
    persona_file_setting: String,
    cli_persona_path: Option<PathBuf>,
    name: Option<String>,
    persona: RwLock<Persona>,
    tool_names: Vec<String>,
    history_path: PathBuf,
    history: RwLock<Vec<String>>,
    /// `/auto on|off|status` で切替するため `Arc<AtomicBool>` を共有（FR-04-2／設計書 4.3A）。
    auto_approve: Arc<AtomicBool>,
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

    let mut ipc_server = IpcServer::bind(socket_path.clone()).await?;
    let mut ipc_rx = ipc_server
        .take_rx()
        .expect("IpcServer rx should be available immediately after bind");

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
    let auto_approve = Arc::new(AtomicBool::new(
        config.runtime.auto_approve_tools || args.auto_approve_tools,
    ));

    // 承認チャネル（FR-04-1／設計書 4.3A）。agent タスクが入力ループへ y/N を求める経路。
    let (approval_tx, approval_rx) = mpsc::channel::<ApprovalRequest>(8);

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
        auto_approve: auto_approve.clone(),
        approval_tx: Some(approval_tx),
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
        auto_approve: auto_approve.clone(),
    });

    let (input_tx, input_rx) = mpsc::channel::<AgentInput>(32);
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(64);

    // shutdown 連携チャネル（FR-13／設計書 4.9）。`/quit`／EOF／SIGINT／SIGTERM の
    // いずれを契機としても、`shutdown_tx.send(true)` が全タスクに伝播する。
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // AI 応答完了通知チャネル（FR-03-2／設計書 4.2A）。display_task が `Done` を
    // 観測した際に発火し、入力ループの `Pending` 状態を解除する。
    let (agent_idle_tx, agent_idle_rx) = mpsc::channel::<()>(8);

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

    // SIGINT / SIGTERM ハンドラ
    let signal_task = {
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            wait_for_termination_signal().await;
            tracing::debug!("termination signal received, broadcasting shutdown");
            let _ = shutdown_tx.send(true);
        })
    };

    // IPC 受信を AgentInput に流す（shutdown 監視つき）
    let input_tx_for_ipc = input_tx.clone();
    let ipc_task = {
        let mut shutdown_rx = shutdown_rx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    res = ipc_rx.recv() => {
                        match res {
                            Some(IpcMessage::Prompt { from, from_name, text }) => {
                                if input_tx_for_ipc
                                    .send(AgentInput::PeerPrompt { from, from_name, text })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Some(_) => {}
                            None => break,
                        }
                    }
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        })
    };

    // 標準入力の読み取り（refactor 済み：run_input_loop で testable）
    let input_tx_for_stdin = input_tx.clone();
    let state_for_stdin = state.clone();
    let stdin_task = {
        let shutdown_tx = shutdown_tx.clone();
        let shutdown_rx = shutdown_rx.clone();
        tokio::spawn(async move {
            run_input_loop(
                tokio::io::stdin(),
                input_tx_for_stdin,
                state_for_stdin,
                shutdown_tx,
                shutdown_rx,
                agent_idle_rx,
                approval_rx,
                true,
            )
            .await;
        })
    };

    // イベント表示。`Done` または `Error` を観測したら入力ループへ idle 通知（FR-03-2）。
    // `Error` も idle として扱うのは、Provider 構築直後の失敗で `Done` が来ないケースに
    // 入力ループが永久に Pending で固まるのを防ぐ防衛策。
    let display_task = tokio::spawn(async move {
        while let Some(ev) = event_rx.recv().await {
            let is_idle = matches!(ev, AgentEvent::Done | AgentEvent::Error { .. });
            display_event(ev);
            if is_idle {
                let _ = agent_idle_tx.send(()).await;
            }
        }
    });

    // shutdown 通知を受けるまでブロック（複数経路の合流点）
    {
        let mut shutdown_rx = shutdown_rx.clone();
        loop {
            if *shutdown_rx.borrow() {
                break;
            }
            if shutdown_rx.changed().await.is_err() {
                break;
            }
        }
    }
    // 入力経路を停止
    stdin_task.abort();
    let _ = stdin_task.await;
    ipc_task.abort();
    let _ = ipc_task.await;
    signal_task.abort();
    let _ = signal_task.await;

    // 残った input senders を解放 → input_rx が None を返し、agent ループが終了する
    drop(input_tx);

    // agent タスクの終了を待つ。in-flight の Provider ストリームがある場合に備えて
    // 短いタイムアウトを設け、超過時は abort する（FR-13: 1 秒以内目標）。
    let agent_abort = agent_handle.abort_handle();
    let abort_timer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        agent_abort.abort();
    });
    let _ = agent_handle.await;
    abort_timer.abort();
    let _ = abort_timer.await;
    // event_tx は agent のクロージャ終了で drop され、display_task が抜ける
    let _ = display_task.await;

    // IPC サーバーと registry handle は Drop で自動クリーンアップされるが、
    // 二重実行は無害なので明示的にも実行しておく。
    drop(ipc_server);
    registry_handle.cleanup();
    IpcServer::cleanup(&socket_path);
    Ok(())
}

/// 入力ループの状態（FR-03-2／FR-04-1／設計書 4.2A／4.3A）。
///
/// - `Ready`：プロンプト描画と stdin 読取を行う。
/// - `Pending`：直前のユーザー入力に対する AI 応答を待っており、通常入力の stdin 読取は抑止。
/// - `AwaitingApproval(resp_tx)`：agent からツール実行承認を求められている状態。
///   次の stdin 行を y/N と解釈し、`resp_tx` で agent へ返却する。
enum PromptState {
    Ready,
    Pending,
    AwaitingApproval(oneshot::Sender<bool>),
}

impl PromptState {
    fn is_ready(&self) -> bool {
        matches!(self, PromptState::Ready)
    }
    fn is_pending(&self) -> bool {
        matches!(self, PromptState::Pending)
    }
    fn is_awaiting_approval(&self) -> bool {
        matches!(self, PromptState::AwaitingApproval(_))
    }
}

/// 標準入力（または任意の `AsyncRead`）からの行入力を `AgentInput` に変換するメインループ。
///
/// 役割：
/// - FR-13「アプリ終了」の入力側エンドポイント：
///   - `/quit` を受領した場合は `shutdown_tx` に `true` を送信して終了。
///   - `lines.next_line()` が `Ok(None)` を返した（EOF=`Ctrl+D`）場合も同様。
///   - `shutdown_rx` の通知を受けた場合（SIGINT などの外部経路）も即座に終了。
/// - FR-03-2「REPL入出力サイクル」のプロンプト同期：
///   - ユーザー入力送信後は `Pending` 状態となり、`agent_idle_rx` から AI 応答完了通知
///     （`Done` イベント検出）を受けるまで stdin を読まない。これによりストリーミング
///     出力と入力エコーの混在を防ぐ。
///
/// `interactive` が `true` のときのみプロンプト（`> `）を描画する。
/// 単体テストでは `interactive = false` で標準出力を汚さない。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_input_loop<R>(
    reader: R,
    input_tx: mpsc::Sender<AgentInput>,
    state: Arc<ReplState>,
    shutdown_tx: watch::Sender<bool>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut agent_idle_rx: mpsc::Receiver<()>,
    approval_rx: mpsc::Receiver<ApprovalRequest>,
    interactive: bool,
) where
    R: AsyncRead + Unpin,
{
    let buffered = BufReader::new(reader);
    let mut lines = buffered.lines();
    let mut prompt_state = PromptState::Ready;
    // 承認チャネルが閉じたら `None` にして以後 `pending()` 待ちに切替（busy loop 回避）
    let mut approval_rx: Option<mpsc::Receiver<ApprovalRequest>> = Some(approval_rx);
    loop {
        if interactive && prompt_state.is_ready() {
            print_prompt();
        }
        tokio::select! {
            biased;
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            // Pending 状態のときだけ AI 応答完了通知を待つ（stdin は休止）
            res = agent_idle_rx.recv(), if prompt_state.is_pending() => {
                match res {
                    Some(()) => {
                        prompt_state = PromptState::Ready;
                        // 次のループ先頭でプロンプトが再描画される
                    }
                    None => break, // display_task 終了 → これ以上待っても来ない
                }
            }
            // 承認リクエスト到着（AwaitingApproval 中ではないとき限定）。FR-04-1
            req = async {
                match approval_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending::<Option<ApprovalRequest>>().await,
                }
            }, if !prompt_state.is_awaiting_approval() && approval_rx.is_some() => {
                match req {
                    Some(req) => {
                        if interactive {
                            println!();
                            println!("[tool approval] {} {}", req.tool_name, req.args);
                            print!("approve? [y/N]: ");
                            let _ = std::io::stdout().flush();
                        }
                        prompt_state = PromptState::AwaitingApproval(req.response);
                    }
                    None => {
                        // 承認チャネルが閉じた → 以後待たない
                        approval_rx = None;
                    }
                }
            }
            // Ready または AwaitingApproval のときに stdin から読み込む
            next = lines.next_line(), if prompt_state.is_ready() || prompt_state.is_awaiting_approval() => {
                match next {
                    Ok(Some(line)) => {
                        let trimmed = line.trim_end_matches('\r').trim().to_string();

                        // AwaitingApproval なら入力を y/N として解釈し agent へ返答
                        if prompt_state.is_awaiting_approval() {
                            let approved = matches!(
                                trimmed.to_ascii_lowercase().as_str(),
                                "y" | "yes"
                            );
                            // 状態を Pending に遷移させながら oneshot を取り出して送信
                            let prev = std::mem::replace(&mut prompt_state, PromptState::Pending);
                            if let PromptState::AwaitingApproval(resp_tx) = prev {
                                let _ = resp_tx.send(approved);
                            }
                            // tool 実行→続報→Done を待つ。古い idle 通知が残っていれば drain
                            while agent_idle_rx.try_recv().is_ok() {}
                            continue;
                        }

                        if let Some(rest) = trimmed.strip_prefix('/') {
                            if !handle_repl_command(rest, &input_tx, &state).await {
                                break;
                            }
                            continue;
                        }
                        if trimmed.is_empty() {
                            continue;
                        }
                        // 履歴へ保存（永続＋メモリ）
                        append_history(&state.history_path, &trimmed);
                        {
                            let mut h = state.history.write().await;
                            h.push(trimmed.clone());
                            let len = h.len();
                            if len > HISTORY_LIMIT {
                                h.drain(..len - HISTORY_LIMIT);
                            }
                        }
                        if input_tx.send(AgentInput::UserPrompt(trimmed)).await.is_err() {
                            break;
                        }
                        // 過去の peer prompt 等で残った idle 通知を捨ててから Pending へ
                        while agent_idle_rx.try_recv().is_ok() {}
                        prompt_state = PromptState::Pending;
                    }
                    Ok(None) => break, // EOF (Ctrl+D)
                    Err(_) => break,
                }
            }
        }
    }
    // shutdown 経路で AwaitingApproval を抜ける場合、agent の oneshot::Receiver を
    // ぶら下げないよう false（拒否）を送って解放する（設計書 4.3A）。
    if let PromptState::AwaitingApproval(resp_tx) =
        std::mem::replace(&mut prompt_state, PromptState::Ready)
    {
        let _ = resp_tx.send(false);
    }
    let _ = shutdown_tx.send(true);
}

/// SIGINT（`Ctrl+C`）または SIGTERM を待つ。Linux 以外の環境では `ctrl_c` のみ。
async fn wait_for_termination_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!(error = %e, "failed to install SIGTERM handler");
                None
            }
        };
        let ctrl_c = tokio::signal::ctrl_c();
        tokio::select! {
            res = ctrl_c => {
                if let Err(e) = res {
                    tracing::warn!(error = %e, "ctrl_c handler error");
                }
            }
            _ = async {
                if let Some(s) = term.as_mut() {
                    s.recv().await;
                } else {
                    futures::future::pending::<()>().await;
                }
            } => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
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
    println!("type /help for commands. /quit, /exit, or ^D to terminate.");
}

/// `/auto [on|off|status]` ハンドラ（FR-04-2／設計書 4.3A）。
fn handle_auto_command(arg: &str, state: &Arc<ReplState>) {
    let arg = arg.trim().to_ascii_lowercase();
    match arg.as_str() {
        "on" | "true" | "1" => {
            state.auto_approve.store(true, Ordering::SeqCst);
            println!("[auto] tool approval: on (skipping y/N prompts)");
        }
        "off" | "false" | "0" => {
            state.auto_approve.store(false, Ordering::SeqCst);
            println!("[auto] tool approval: off (will ask y/N for each tool call)");
        }
        "" | "status" => {
            let cur = if state.auto_approve.load(Ordering::SeqCst) {
                "on"
            } else {
                "off"
            };
            println!("[auto] tool approval: {cur}");
        }
        other => {
            eprintln!("usage: /auto [on|off|status]  (got: {other})");
        }
    }
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
            println!("Commands:");
            println!("  /list                       List currently running peers (id / name / provider / model / role).");
            println!(
                "  /send <peer> <text>         Send a one-shot prompt to a peer (id or name)."
            );
            println!("  /tools                      List tools enabled for this agent.");
            println!("  /persona                    Show this agent's persona (role / skills / source path).");
            println!("  /reload-persona             Re-resolve and reload the persona; system prompt is replaced, history kept.");
            println!("  /peer <id_or_name>          Show a peer's persona summary.");
            println!("  /history [n]                Show last n (default 20) user inputs from this session.");
            println!("  /clear, /reset              Clear conversation history (persona / system prompt are kept).");
            println!("  /cancel                     Request cancel of the in-flight AI response or tool call.");
            println!("  /auto [on|off|status]       Toggle tool-approval skip. No arg / 'status' shows current value.");
            println!("  /help                       Show this help.");
            println!("  /quit, /exit                Terminate (full aliases). Ctrl+D, Ctrl+C, SIGTERM also exit cleanly.");
            println!();
            println!("Tool approval can be skipped via:");
            println!("  - REPL command  : /auto on  (toggleable at runtime)");
            println!("  - CLI flag      : agent-cli run --auto-approve-tools");
            println!("  - Config file   : [runtime] auto_approve_tools = true");
        }
        "auto" => handle_auto_command(arg, state),
        "clear" | "reset" => {
            // 会話履歴を初期化（システムプロンプトのみ残す）。
            if input_tx.send(AgentInput::ClearHistory).await.is_err() {
                eprintln!("[error] failed to send clear request");
            }
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

#[cfg(test)]
mod tests {
    //! FR-13「アプリ終了」の入力ループ部分（`/quit` と Ctrl+D=EOF）を回帰テストする。
    use super::*;
    use crate::persona::Persona;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    fn build_state(dir: &Path) -> Arc<ReplState> {
        Arc::new(ReplState {
            registry_dir: dir.to_path_buf(),
            agents_dir: dir.to_path_buf(),
            persona_file_setting: String::new(),
            cli_persona_path: None,
            name: Some("test".into()),
            persona: RwLock::new(Persona::builtin_default()),
            tool_names: Vec::new(),
            history_path: dir.join("history.txt"),
            history: RwLock::new(Vec::new()),
            auto_approve: Arc::new(AtomicBool::new(false)),
        })
    }

    /// テスト用ヘルパー：未使用の approval チャネルを返す。
    fn dummy_approval_rx() -> mpsc::Receiver<ApprovalRequest> {
        let (_tx, rx) = mpsc::channel::<ApprovalRequest>(4);
        rx
    }

    #[tokio::test]
    async fn input_loop_terminates_on_eof() {
        // 空 stdin（即 EOF）→ run_input_loop は break して shutdown_tx.send(true) を発行する。
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_idle_tx, idle_rx) = mpsc::channel::<()>(8);
        let shutdown_observer = shutdown_rx.clone();

        // EOF を即起こす reader
        let reader = tokio::io::empty();

        let handle = tokio::spawn(async move {
            run_input_loop(
                reader,
                input_tx,
                state,
                shutdown_tx,
                shutdown_rx,
                idle_rx,
                dummy_approval_rx(),
                false,
            )
            .await;
        });

        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "input loop should terminate on EOF");
        // ループ終了後、shutdown_tx.send(true) が発行されている
        assert!(
            *shutdown_observer.borrow(),
            "EOF should propagate as shutdown=true"
        );
    }

    #[tokio::test]
    async fn input_loop_terminates_on_quit_command() {
        // /quit を投入 → break して shutdown_tx.send(true) を発行する。
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_idle_tx, idle_rx) = mpsc::channel::<()>(8);
        let shutdown_observer = shutdown_rx.clone();

        let (mut writer, reader) = tokio::io::duplex(64);
        writer.write_all(b"/quit\n").await.unwrap();
        // writer は drop しない（テストでは /quit 経由の終了を確認する）

        let handle = tokio::spawn(async move {
            run_input_loop(
                reader,
                input_tx,
                state,
                shutdown_tx,
                shutdown_rx,
                idle_rx,
                dummy_approval_rx(),
                false,
            )
            .await;
        });

        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "input loop should terminate on /quit");
        assert!(
            *shutdown_observer.borrow(),
            "/quit should propagate as shutdown=true"
        );
    }

    #[tokio::test]
    async fn input_loop_responds_to_external_shutdown() {
        // 外部（SIGINT 想定）から shutdown を受領 → ループは break する。
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_idle_tx, idle_rx) = mpsc::channel::<()>(8);

        // EOF を起こさず、入力もない reader（duplex の writer 側を保持）
        let (_writer, reader) = tokio::io::duplex(64);

        let shutdown_tx_clone = shutdown_tx.clone();
        let handle = tokio::spawn(async move {
            run_input_loop(
                reader,
                input_tx,
                state,
                shutdown_tx,
                shutdown_rx,
                idle_rx,
                dummy_approval_rx(),
                false,
            )
            .await;
        });

        // 100ms 後に外部から shutdown 通知
        tokio::time::sleep(Duration::from_millis(100)).await;
        shutdown_tx_clone.send(true).unwrap();

        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(
            result.is_ok(),
            "input loop should terminate on external shutdown signal"
        );
    }

    #[tokio::test]
    async fn input_loop_waits_for_agent_idle_between_user_prompts() {
        // FR-03-2 / 設計書 4.2A：
        // ユーザー入力 1 件目を送信 → Pending → idle 受領まで 2 件目は agent へ届かない。
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (idle_tx, idle_rx) = mpsc::channel::<()>(8);

        let (mut writer, reader) = tokio::io::duplex(1024);
        // 2 件まとめて投入
        writer.write_all(b"first\nsecond\n").await.unwrap();

        let handle = tokio::spawn(async move {
            run_input_loop(
                reader,
                input_tx,
                state,
                shutdown_tx,
                shutdown_rx,
                idle_rx,
                dummy_approval_rx(),
                false,
            )
            .await;
        });

        // 1 件目は速やかに到達する
        let msg1 = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("first prompt timeout")
            .expect("input_rx closed");
        match msg1 {
            AgentInput::UserPrompt(s) => assert_eq!(s, "first"),
            other => panic!("expected UserPrompt(\"first\"), got {:?}", other),
        }

        // 2 件目は idle 通知が来るまで届かない（Pending 状態が stdin を抑止）
        let blocked = tokio::time::timeout(Duration::from_millis(300), input_rx.recv()).await;
        assert!(
            blocked.is_err(),
            "second prompt should not arrive while input loop is Pending"
        );

        // idle を発行 → 入力ループは Ready に復帰し、2 件目を読み込む
        idle_tx.send(()).await.unwrap();

        let msg2 = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("second prompt timeout")
            .expect("input_rx closed");
        match msg2 {
            AgentInput::UserPrompt(s) => assert_eq!(s, "second"),
            other => panic!("expected UserPrompt(\"second\"), got {:?}", other),
        }

        // 後始末
        drop(writer);
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    #[tokio::test]
    async fn stale_idle_signal_is_drained_before_pending() {
        // peer prompt 等で agent が独立して Done を出した結果として idle が溜まっていても、
        // 次のユーザー入力直後にそれを誤って消費して Pending を即解除しないこと。
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (idle_tx, idle_rx) = mpsc::channel::<()>(8);

        // 古い idle 通知を仕込んでおく
        idle_tx.send(()).await.unwrap();
        idle_tx.send(()).await.unwrap();

        let (mut writer, reader) = tokio::io::duplex(1024);
        writer.write_all(b"only\n").await.unwrap();

        let handle = tokio::spawn(async move {
            run_input_loop(
                reader,
                input_tx,
                state,
                shutdown_tx,
                shutdown_rx,
                idle_rx,
                dummy_approval_rx(),
                false,
            )
            .await;
        });

        // 入力 1 件は届く
        let msg = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("prompt timeout")
            .expect("input_rx closed");
        match msg {
            AgentInput::UserPrompt(s) => assert_eq!(s, "only"),
            other => panic!("unexpected: {:?}", other),
        }

        // 古い idle はドレインされたので、これ以上は来ない（Pending 中、stdin は未読）
        writer.write_all(b"should-not-pass\n").await.unwrap();
        let blocked = tokio::time::timeout(Duration::from_millis(300), input_rx.recv()).await;
        assert!(
            blocked.is_err(),
            "stale idle signals should have been drained, leaving the loop Pending"
        );

        // 新しく idle を出せば 2 件目が解放される
        idle_tx.send(()).await.unwrap();
        let msg2 = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("second prompt timeout")
            .expect("input_rx closed");
        match msg2 {
            AgentInput::UserPrompt(s) => assert_eq!(s, "should-not-pass"),
            other => panic!("unexpected: {:?}", other),
        }

        drop(writer);
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    /// FR-13／T-507：`/exit` で `/quit` 同等の終了が起きる。
    #[tokio::test]
    async fn input_loop_terminates_on_exit_command() {
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_idle_tx, idle_rx) = mpsc::channel::<()>(8);
        let shutdown_observer = shutdown_rx.clone();

        let (mut writer, reader) = tokio::io::duplex(64);
        writer.write_all(b"/exit\n").await.unwrap();

        let handle = tokio::spawn(async move {
            run_input_loop(
                reader,
                input_tx,
                state,
                shutdown_tx,
                shutdown_rx,
                idle_rx,
                dummy_approval_rx(),
                false,
            )
            .await;
        });

        let result = tokio::time::timeout(Duration::from_secs(2), handle).await;
        assert!(result.is_ok(), "input loop should terminate on /exit");
        assert!(
            *shutdown_observer.borrow(),
            "/exit should propagate as shutdown=true"
        );
    }

    /// FR-04-1／T-506：承認リクエスト到着 → "y" 入力 → oneshot に true が届き Pending へ遷移、
    /// その間ユーザー入力は agent へ流出しない。
    #[tokio::test]
    async fn approval_y_resolves_true_and_blocks_user_prompt() {
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (idle_tx, idle_rx) = mpsc::channel::<()>(8);
        let (approval_tx, approval_rx) = mpsc::channel::<ApprovalRequest>(4);

        let (mut writer, reader) = tokio::io::duplex(1024);
        // 後から y を投入する

        let handle = tokio::spawn(async move {
            run_input_loop(
                reader,
                input_tx,
                state,
                shutdown_tx,
                shutdown_rx,
                idle_rx,
                approval_rx,
                false,
            )
            .await;
        });

        // 承認リクエスト送信
        let (resp_tx, resp_rx) = oneshot::channel::<bool>();
        approval_tx
            .send(ApprovalRequest {
                tool_name: "shell".into(),
                args: serde_json::json!({"cmd": "echo hi"}),
                response: resp_tx,
            })
            .await
            .unwrap();

        // 状態が AwaitingApproval に遷移するまで少し待つ
        tokio::time::sleep(Duration::from_millis(50)).await;

        // この時点でユーザーが「これは普通の入力だ」と思って打ったテキストは
        // y/N 解釈になるべきで、UserPrompt として agent に届いてはならない。
        writer.write_all(b"some text\n").await.unwrap();
        let leaked = tokio::time::timeout(Duration::from_millis(200), input_rx.recv()).await;
        assert!(
            leaked.is_err(),
            "approval-mode input must not reach agent as UserPrompt"
        );
        // 上で投入した "some text" は y/yes に該当しないので oneshot は false を受領済み
        let approved = tokio::time::timeout(Duration::from_secs(2), resp_rx)
            .await
            .expect("oneshot timeout")
            .expect("oneshot dropped");
        assert!(!approved, "non-y input should resolve to false");

        // 続けて承認リクエスト → 今度は y を投入 → true で解決
        let (resp_tx2, resp_rx2) = oneshot::channel::<bool>();
        approval_tx
            .send(ApprovalRequest {
                tool_name: "shell".into(),
                args: serde_json::json!({"cmd": "echo hi"}),
                response: resp_tx2,
            })
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        writer.write_all(b"y\n").await.unwrap();
        let approved2 = tokio::time::timeout(Duration::from_secs(2), resp_rx2)
            .await
            .expect("oneshot2 timeout")
            .expect("oneshot2 dropped");
        assert!(approved2, "'y' should resolve to true");

        // 承認後、Pending → idle で Ready に戻り通常入力を受け付ける
        idle_tx.send(()).await.unwrap();
        writer.write_all(b"after\n").await.unwrap();
        let msg = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("post-approval prompt timeout")
            .expect("input_rx closed");
        match msg {
            AgentInput::UserPrompt(s) => assert_eq!(s, "after"),
            other => panic!("unexpected: {:?}", other),
        }

        drop(writer);
        drop(approval_tx);
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    /// FR-04-1：shutdown 経路で AwaitingApproval を抜ける際、oneshot に false（拒否）が届く。
    #[tokio::test]
    async fn shutdown_during_awaiting_approval_replies_false() {
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_idle_tx, idle_rx) = mpsc::channel::<()>(8);
        let (approval_tx, approval_rx) = mpsc::channel::<ApprovalRequest>(4);

        let (_writer, reader) = tokio::io::duplex(64);

        let shutdown_clone = shutdown_tx.clone();
        let handle = tokio::spawn(async move {
            run_input_loop(
                reader,
                input_tx,
                state,
                shutdown_tx,
                shutdown_rx,
                idle_rx,
                approval_rx,
                false,
            )
            .await;
        });

        // 承認リクエストを送って AwaitingApproval に遷移させる
        let (resp_tx, resp_rx) = oneshot::channel::<bool>();
        approval_tx
            .send(ApprovalRequest {
                tool_name: "shell".into(),
                args: serde_json::json!({}),
                response: resp_tx,
            })
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // shutdown を発火
        shutdown_clone.send(true).unwrap();

        // oneshot が false で解決される（agent 側がぶら下がらない）
        let resp = tokio::time::timeout(Duration::from_secs(2), resp_rx)
            .await
            .expect("oneshot timeout");
        match resp {
            Ok(b) => assert!(!b, "shutdown should deny pending approval"),
            Err(_) => panic!("oneshot was dropped without sending; agent would hang"),
        }

        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }
}
