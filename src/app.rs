use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use crossterm::event::{self, Event as CtEvent, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, ClearType};
use crossterm::ExecutableCommand;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::{mpsc, oneshot, watch, RwLock};

use crate::agent::{Agent, AgentEvent, AgentInput, ApprovalRequest, CancelToken};
use crate::ai;
use crate::cli::RunArgs;
use crate::config::{Config, ConfigSource, ShowThinkingMode};
use crate::custom_commands::{self, CustomCommand};
use crate::editor::InputState;
use crate::error::Result;
use crate::id::AgentId;
use crate::ipc::registry::{RegistryEntry, RegistryHandle};
use crate::ipc::server::IpcServer;
use crate::ipc::IpcMessage;
use crate::log::ConversationLog;
use crate::persona::{self, Persona, PersonaResolution};
use crate::theme::{Role, Theme};
use crate::tools::ToolRegistry;
/// In raw mode, the terminal does not convert LF to CR+LF. These helpers ensure
/// line endings include CR so the cursor returns to column 0 after each newline.

/// Print a string to stdout, replacing LF with CR+LF, then append CR+LF.
fn raw_println(raw: bool, msg: &str) {
    if raw {
        let out = msg.replace('\n', "\r\n");
        let _ = std::io::stdout().write_all(out.as_bytes());
        let _ = std::io::stdout().write_all(b"\r\n");
        let _ = std::io::stdout().flush();
    } else {
        println!("{}", msg);
    }
}

/// Print a string to stdout, replacing LF with CR+LF (no trailing newline added).
fn raw_print_str(raw: bool, msg: &str) {
    if raw {
        let out = msg.replace('\n', "\r\n");
        let _ = std::io::stdout().write_all(out.as_bytes());
        let _ = std::io::stdout().flush();
    } else {
        print!("{}", msg);
        let _ = std::io::stdout().flush();
    }
}

/// Print a string to stderr, replacing LF with CR+LF, then append CR+LF.
fn raw_eprintln(raw: bool, msg: &str) {
    if raw {
        let out = msg.replace('\n', "\r\n");
        let _ = std::io::stderr().write_all(out.as_bytes());
        let _ = std::io::stderr().write_all(b"\r\n");
        let _ = std::io::stderr().flush();
    } else {
        eprintln!("{}", msg);
    }
}

/// Print a string to stderr, replacing LF with CR+LF (no trailing newline added).
fn raw_eprint(raw: bool, msg: &str) {
    if raw {
        let out = msg.replace('\n', "\r\n");
        let _ = std::io::stderr().write_all(out.as_bytes());
        let _ = std::io::stderr().flush();
    } else {
        eprint!("{}", msg);
    }
}

/// Shared state referenced by REPL command handlers.
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
    /// Shared via `Arc<AtomicBool>` for `/auto on|off|status` runtime toggle (FR-04-2 / design doc 4.3A).
    auto_approve: Arc<AtomicBool>,
    /// Raised by `Esc` / `Ctrl-C` during a turn and by `/cancel`; the agent task
    /// observes it at every await point and unwinds the turn.
    cancel: Arc<CancelToken>,
    /// Number of cancellations whose turn has not reached its terminal event
    /// yet. The display task drops the events (and the idle notification) of
    /// those turns so the cancelled tail cannot print over the fresh prompt.
    suppress: Arc<AtomicUsize>,
    /// Progress indicator for the running turn, shared with the display task.
    /// The input loop suspends it before writing at the bottom of the screen,
    /// so the two writers never interleave escape sequences.
    indicator: Arc<std::sync::Mutex<StatusIndicator>>,
    /// Resolved directory for custom slash commands (`*.md` files).
    commands_dir: PathBuf,
    /// Discovered custom commands, refreshable via `/reload-commands`.
    commands: RwLock<Vec<CustomCommand>>,
    /// This session's config source path, used by `/spawn` to launch a detached
    /// peer that shares the same config file (hence the same registry_dir).
    config_source: ConfigSource,
    /// This session's effective group; `/spawn` passes it to a detached child so
    /// the child inherits the launcher's group.
    group: Option<crate::id::GroupId>,
    /// Colour scheme, resolved once at startup and shared by every writer.
    theme: Theme,
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

fn append_history(path: &Path, line: &str, last_line: Option<&str>) {
    // Skip consecutive duplicate (bash HISTCONTROL=ignoredups behaviour)
    if let Some(last) = last_line {
        if last == line {
            return;
        }
    }
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

/// Push a line into in-memory history (with dedup and limit) and persist it.
async fn push_history(state: &Arc<ReplState>, line: &str) {
    let last_line = state.history.read().await.last().cloned();
    append_history(&state.history_path, line, last_line.as_deref());
    let mut h = state.history.write().await;
    if h.last().map(|l| l.as_str()) != Some(line) {
        h.push(line.to_string());
        let len = h.len();
        if len > HISTORY_LIMIT {
            h.drain(..len - HISTORY_LIMIT);
        }
    }
}

/// Remove consecutive duplicate entries from a history slice, preserving order.
/// Non-consecutive duplicates are kept (e.g., ["a", "b", "a"] stays as-is).
fn dedup_consecutive(history: &[String]) -> Vec<String> {
    let mut result = Vec::with_capacity(history.len());
    for entry in history {
        if result.last().map(|l: &String| l.as_str()) != Some(entry.as_str()) {
            result.push(entry.clone());
        }
    }
    result
}

pub async fn run(mut config: Config, source: ConfigSource, args: RunArgs) -> Result<()> {
    config.apply_overrides(args.provider.as_deref(), args.model.as_deref());

    let id = AgentId::new();
    let name = args.name.clone();
    let group = config.resolve_group(args.group.as_deref());
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

    // Reflect persona-derived model / temperature into Provider settings
    config.apply_persona_overrides(
        resolution.persona.frontmatter.model.as_deref(),
        resolution.persona.frontmatter.temperature,
    );

    // Build provider (pre-connection validation)
    let provider = ai::build(&mut config, &source)?;
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
        group: group.clone(),
        pid: std::process::id(),
        started_at: Utc::now(),
        provider: config.provider.kind.clone(),
        model: provider.model().to_string(),
        socket: socket_path.clone(),
        persona: Some(resolution.persona.summary()),
    };
    let registry_handle = RegistryHandle::register(&registry_dir, &entry).await?;
    let registry_handle = Arc::new(registry_handle);

    // Logging
    let log = ConversationLog::open(&config.log_dir()?, &id).await?;

    // Tools
    let allowed = resolution.persona.frontmatter.allowed_tools.clone();
    let denied = resolution.persona.frontmatter.denied_tools.clone();
    let mut tools = ToolRegistry::build(&config, allowed.as_deref(), denied.as_deref());
    // Connect declared MCP servers and register their tools (fail-soft: a bad
    // server is logged and skipped, never aborting startup).
    let mcp_tools = crate::mcp::connect_all(&config.mcp).await;
    tools.attach(mcp_tools, allowed.as_deref(), denied.as_deref());
    let tool_names = tools.names();

    let history = Agent::build_initial_history(&resolution.persona);
    let auto_approve = Arc::new(AtomicBool::new(
        config.runtime.auto_approve_tools || args.auto_approve_tools,
    ));
    // Cancellation plumbing: the token stops the agent, the counter tells the
    // display task how many cancelled turns are still draining.
    let cancel = Arc::new(CancelToken::default());
    let suppress = Arc::new(AtomicUsize::new(0));
    // Progress indicator for the running turn. Shared with the input loop so it
    // can clear the block before writing at the bottom of the screen itself.
    // Only ever drawn on an interactive terminal.
    let progress_enabled = {
        use std::io::IsTerminal;
        stdin_is_tty() && std::io::stderr().is_terminal() && config.ui.show_progress
    };
    // Colour scheme. Resolved once, separately per stream, so a redirected
    // answer stays plain even while the status display keeps its colour.
    let theme = Theme::from_env(config.ui.color_mode());
    let indicator = Arc::new(std::sync::Mutex::new(if progress_enabled {
        let mut ind = StatusIndicator::new(Box::new(std::io::stderr()), true, stdin_is_tty());
        ind.thinking_view = config.ui.show_thinking_mode() != ShowThinkingMode::Hidden;
        ind.theme = theme;
        ind
    } else {
        StatusIndicator::disabled()
    }));

    // Approval channel (FR-04-1 / design doc 4.3A). Route for agent task to request y/N from input loop.
    let (approval_tx, approval_rx) = mpsc::channel::<ApprovalRequest>(8);

    let initial_persona = resolution.persona.clone();
    let agent = Agent {
        id: id.clone(),
        name: name.clone(),
        group: group.clone(),
        persona: resolution.persona,
        provider,
        tools,
        config: config.clone(),
        config_source: source.clone(),
        registry_dir: registry_dir.clone(),
        log: Some(log),
        auto_approve: auto_approve.clone(),
        cancel: cancel.clone(),
        approval_tx: Some(approval_tx),
        history,
    };

    let history_path = config.log_dir()?.join("history.txt");
    let initial_history = load_history(&history_path);
    let commands_dir = custom_commands::resolve_dir(&config.runtime.commands_dir);
    let initial_commands = custom_commands::discover(&commands_dir);
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
        cancel: cancel.clone(),
        suppress: suppress.clone(),
        indicator: indicator.clone(),
        commands_dir,
        commands: RwLock::new(initial_commands),
        config_source: source.clone(),
        group: group.clone(),
        theme,
    });

    let (input_tx, input_rx) = mpsc::channel::<AgentInput>(32);
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(64);

    // Shutdown coordination channel (FR-13 / design doc 4.9). Regardless of whether
    // triggered by `/quit`, EOF, SIGINT, or SIGTERM, `shutdown_tx.send(true)` propagates to all tasks.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // AI response completion notification channel (FR-03-2 / design doc 4.2A). Fired when
    // display_task observes `Done`, releasing the input loop from `Pending` state.
    let (agent_idle_tx, agent_idle_rx) = mpsc::channel::<()>(8);

    print_header(
        &id,
        name.as_deref(),
        &config.provider.kind,
        agent.provider.model(),
        &agent.persona,
        caps,
        &theme,
    );

    // Agent task
    let agent_handle = tokio::spawn(async move { agent.run(input_rx, event_tx).await });

    // SIGINT / SIGTERM handler
    let signal_task = {
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            wait_for_termination_signal().await;
            tracing::debug!("termination signal received, broadcasting shutdown");
            let _ = shutdown_tx.send(true);
        })
    };

    // Forward IPC messages to AgentInput (with shutdown monitoring)
    let input_tx_for_ipc = input_tx.clone();
    let ipc_task = {
        let mut shutdown_rx = shutdown_rx.clone();
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    res = ipc_rx.recv() => {
                        match res {
                            Some(IpcMessage::Prompt { from, from_name, text, reply_to }) => {
                                if input_tx_for_ipc
                                    .send(AgentInput::PeerPrompt { from, from_name, text, reply_to })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            // A peer requested we shut down (e.g. `agent-cli stop` / `/stop`).
                            Some(IpcMessage::Shutdown) => {
                                let _ = shutdown_tx.send(true);
                                break;
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

    // Read from stdin (refactored: testable via run_input_loop)
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

    // Display events. On `Done` or `Error`, notify the input loop of idle state (FR-03-2).
    // Treating `Error` as idle is a defensive measure to prevent the input loop from
    // getting stuck in Pending forever if Provider construction fails without a `Done`.
    let show_thinking = config.ui.show_thinking_mode();
    let display_suppress = suppress.clone();
    let display_indicator = indicator.clone();
    let display_task = tokio::spawn(async move {
        let mut display_state = DisplayState::new(stdin_is_tty());
        // With the indicator drawing right beneath it, a tool call is described
        // on a single row instead of dumping its raw arguments.
        display_state.compact_output = progress_enabled;
        // With the indicator on, reasoning is shown live in its block rather
        // than streamed into the scrollback.
        display_state.capture_thinking =
            progress_enabled && show_thinking != ShowThinkingMode::Hidden;
        display_state.theme = theme;
        // Where the cursor stands after the last displayed event; the indicator
        // may only paint from column 0 of a fresh row.
        let mut at_line_start = true;
        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        // A tick missed while the task was busy must not produce a burst of
        // catch-up repaints.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            let ev = tokio::select! {
                ev = event_rx.recv() => match ev {
                    Some(ev) => ev,
                    None => break,
                },
                _ = ticker.tick() => {
                    lock_indicator(&display_indicator).tick();
                    continue;
                }
            };
            let is_idle = matches!(ev, AgentEvent::Done | AgentEvent::Error { .. });
            // While a cancelled turn is still draining, its remaining events are
            // dropped and its terminal event yields no idle notification (it
            // must not release a *later* turn from Pending).
            let (drop_event, notify_idle, outstanding) =
                suppression_decision(is_idle, display_suppress.load(Ordering::SeqCst));
            display_suppress.store(outstanding, Ordering::SeqCst);
            {
                // The guard covers only synchronous terminal writes; it is
                // dropped before the idle notification is awaited.
                let mut ind = lock_indicator(&display_indicator);
                // Nothing may be printed on top of the indicator's rows.
                ind.clear();
                if !drop_event {
                    if matches!(ev, AgentEvent::TurnStart) {
                        ind.start();
                    }
                    if display_state.capture_thinking {
                        if let AgentEvent::Thinking { text } = &ev {
                            ind.push_thinking(text);
                        }
                    }
                    let mark = match &ev {
                        AgentEvent::Error { .. } => Mark::Err,
                        _ => Mark::Ok,
                    };
                    at_line_start = event_ends_at_line_start(
                        &ev,
                        show_thinking,
                        display_state.capture_thinking,
                        at_line_start,
                    );
                    display_event(ev, show_thinking, &mut display_state);
                    if is_idle {
                        ind.finish(mark);
                    } else {
                        ind.paint(at_line_start);
                    }
                } else {
                    // The cancelled turn's output is discarded, so it gets no
                    // completion line either.
                    ind.abandon_run();
                    if is_idle {
                        // The cancelled turn ended: start the next one from a clean slate.
                        display_state.reset();
                        at_line_start = true;
                    }
                }
            }
            if notify_idle {
                let _ = agent_idle_tx.send(()).await;
            }
        }
    });

    // Block until shutdown notification (convergence point for multiple routes)
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
    // Stop input routes
    stdin_task.abort();
    let _ = stdin_task.await;
    ipc_task.abort();
    let _ = ipc_task.await;
    signal_task.abort();
    let _ = signal_task.await;

    // Release remaining input senders -> input_rx returns None, agent loop exits
    drop(input_tx);

    // Wait for the agent task to finish. In case of an in-flight Provider stream,
    // set a short timeout and abort if exceeded (FR-13: target under 1 second).
    let agent_abort = agent_handle.abort_handle();
    let abort_timer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        agent_abort.abort();
    });
    let _ = agent_handle.await;
    abort_timer.abort();
    let _ = abort_timer.await;
    // event_tx is dropped when the agent closure ends, causing display_task to exit
    let _ = display_task.await;

    // IPC server and registry handle are auto-cleaned on Drop, but explicit cleanup
    // is harmless and ensures thoroughness.
    drop(ipc_server);
    registry_handle.cleanup();
    IpcServer::cleanup(&socket_path);
    Ok(())
}

/// Headless run: register and serve peer prompts over IPC with no interactive
/// REPL. This is the target a detached `spawn` launches (`Command::Serve`); it
/// may also be run directly for a foreground headless agent.
///
/// It builds the same provider / IPC server / registry / agent / display stack
/// as [`run`], but omits the stdin input loop entirely — a detached process has
/// `stdin` pointed at `/dev/null`, and [`run`] treats stdin EOF as shutdown,
/// which would kill the agent immediately. With no console to answer tool
/// approval, serve mode forces `auto_approve = true` and builds the agent with
/// `approval_tx: None`, so it never blocks on a y/N prompt. Shutdown converges
/// on `SIGINT` / `SIGTERM` (via the signal task) or an `IpcMessage::Shutdown`.
pub async fn run_headless(mut config: Config, source: ConfigSource, args: RunArgs) -> Result<()> {
    config.apply_overrides(args.provider.as_deref(), args.model.as_deref());

    let id = AgentId::new();
    let name = args.name.clone();
    let group = config.resolve_group(args.group.as_deref());
    let agents_dir = config.agents_dir()?;
    let resolution: PersonaResolution = persona::resolve(
        args.persona.as_deref(),
        &config.runtime.persona_file,
        &agents_dir,
        name.as_deref(),
    )?;

    config.apply_persona_overrides(
        resolution.persona.frontmatter.model.as_deref(),
        resolution.persona.frontmatter.temperature,
    );

    let provider = ai::build(&mut config, &source)?;

    let registry_dir = config.registry_dir()?;
    let socket_path = registry_dir.join(format!("{}.sock", id.as_str()));

    let mut ipc_server = IpcServer::bind(socket_path.clone()).await?;
    let mut ipc_rx = ipc_server
        .take_rx()
        .expect("IpcServer rx should be available immediately after bind");

    let entry = RegistryEntry {
        id: id.clone(),
        name: name.clone(),
        group: group.clone(),
        pid: std::process::id(),
        started_at: Utc::now(),
        provider: config.provider.kind.clone(),
        model: provider.model().to_string(),
        socket: socket_path.clone(),
        persona: Some(resolution.persona.summary()),
    };
    let registry_handle = RegistryHandle::register(&registry_dir, &entry).await?;
    let registry_handle = Arc::new(registry_handle);

    let log = ConversationLog::open(&config.log_dir()?, &id).await?;

    let allowed = resolution.persona.frontmatter.allowed_tools.clone();
    let denied = resolution.persona.frontmatter.denied_tools.clone();
    let mut tools = ToolRegistry::build(&config, allowed.as_deref(), denied.as_deref());
    let mcp_tools = crate::mcp::connect_all(&config.mcp).await;
    tools.attach(mcp_tools, allowed.as_deref(), denied.as_deref());

    let history = Agent::build_initial_history(&resolution.persona);
    // A headless agent has no console to answer tool-approval prompts, so it
    // must auto-approve and never request confirmation (approval_tx: None).
    let auto_approve = Arc::new(AtomicBool::new(true));

    let agent = Agent {
        id: id.clone(),
        name: name.clone(),
        group: group.clone(),
        persona: resolution.persona,
        provider,
        tools,
        config: config.clone(),
        config_source: source.clone(),
        registry_dir: registry_dir.clone(),
        log: Some(log),
        auto_approve,
        // Headless: no console, so nothing ever raises this token.
        cancel: Arc::new(CancelToken::default()),
        approval_tx: None,
        history,
    };

    let (input_tx, input_rx) = mpsc::channel::<AgentInput>(32);
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(64);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Serve mode never colours: `Theme::plain()` makes this the same bytes as
    // before, and keeps every headless consumer's parsing intact.
    let theme = Theme::plain();
    println!(
        "{}",
        theme.out(
            Role::Banner,
            &format!(
                "agent-cli serving headless: id={} name={} provider={} model={}",
                id.as_str(),
                name.as_deref().unwrap_or("-"),
                config.provider.kind,
                agent.provider.model(),
            )
        )
    );
    let _ = std::io::stdout().flush();

    let agent_handle = tokio::spawn(async move { agent.run(input_rx, event_tx).await });

    // SIGINT / SIGTERM handler
    let signal_task = {
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            wait_for_termination_signal().await;
            let _ = shutdown_tx.send(true);
        })
    };

    // Forward IPC prompts to the agent; an IpcMessage::Shutdown triggers shutdown.
    let input_tx_for_ipc = input_tx.clone();
    let ipc_task = {
        let mut shutdown_rx = shutdown_rx.clone();
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    res = ipc_rx.recv() => {
                        match res {
                            Some(IpcMessage::Prompt { from, from_name, text, reply_to }) => {
                                if input_tx_for_ipc
                                    .send(AgentInput::PeerPrompt { from, from_name, text, reply_to })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Some(IpcMessage::Shutdown) => {
                                let _ = shutdown_tx.send(true);
                                break;
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

    let show_thinking = config.ui.show_thinking_mode();
    let display_task = tokio::spawn(async move {
        let mut display_state = DisplayState::new(stdin_is_tty());
        while let Some(ev) = event_rx.recv().await {
            display_event(ev, show_thinking, &mut display_state);
        }
    });

    // Block until a shutdown notification arrives (signal or IPC Shutdown).
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

    ipc_task.abort();
    let _ = ipc_task.await;
    signal_task.abort();
    let _ = signal_task.await;

    drop(input_tx);

    let agent_abort = agent_handle.abort_handle();
    let abort_timer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        agent_abort.abort();
    });
    let _ = agent_handle.await;
    abort_timer.abort();
    let _ = abort_timer.await;
    let _ = display_task.await;

    drop(ipc_server);
    registry_handle.cleanup();
    IpcServer::cleanup(&socket_path);
    Ok(())
}

/// Input loop state (FR-03-2 / FR-04-1 / design doc 4.2A / 4.3A).
///
/// - `Ready`: Render prompt and read from stdin.
/// - `Pending`: Waiting for AI response to the previous user input; normal stdin reads are suppressed.
/// - `AwaitingApproval(resp_tx)`: Agent is requesting tool execution approval.
///   The next stdin line is interpreted as y/N and returned to the agent via `resp_tx`.
enum PromptState {
    Ready,
    Pending,
    AwaitingApproval(oneshot::Sender<bool>),
}

/// Outcome of `handle_repl_command` (custom-command support, FR-08/FR-11).
///
/// - `Continue`: command completed (or unknown); redraw the prompt (existing behavior).
/// - `SubmittedPrompt`: a custom command sent a `UserPrompt` to the agent; the
///   caller must drain stale idle notifications and enter `PromptState::Pending`
///   (same as a normal user prompt).
/// - `Quit`: terminate the REPL (`/quit`, `/exit`).
#[derive(Debug)]
enum CommandResult {
    Continue,
    SubmittedPrompt,
    Quit,
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

/// RAII guard that enables crossterm raw mode on creation and disables it on drop.
/// Ensures the terminal is restored even on panic or early return (NFR-04).
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> std::io::Result<Self> {
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        // The progress indicator turns mouse reporting on for the duration of a
        // turn; make sure it can never outlive the raw-mode session.
        let _ = std::io::stderr().execute(crossterm::event::DisableMouseCapture);
        let _ = terminal::disable_raw_mode();
    }
}

/// Compute the rendered layout of a prompt + edit buffer for a terminal of the
/// given width. Returns `(end_row, cursor_row, cursor_col)`, all measured in
/// terminal cells relative to the top-left of the rendered block:
///
/// - `end_row`: the physical row the cursor sits on after the whole content is
///   printed (accounting for the "phantom" last-column wrap, where a line that
///   exactly fills the width leaves the cursor on the same row rather than the
///   next one).
/// - `cursor_row` / `cursor_col`: where the logical cursor should be placed.
///
/// `prompt_cols` is the display width of the prompt prefix, `cursor_cols` the
/// display width of the text before the cursor, and `total_cols` the display
/// width of the prompt + entire line. `width` is the terminal width in columns
/// (clamped to at least 1 by the caller).
fn layout_cursor(
    prompt_cols: usize,
    cursor_cols: usize,
    total_cols: usize,
    width: usize,
) -> (usize, usize, usize) {
    let w = width.max(1);
    let cursor_abs = prompt_cols + cursor_cols;
    // Physical row of the cursor after printing all content. When the content
    // fills the final row exactly, the terminal keeps the cursor on that row
    // (pending-wrap) instead of advancing, so subtract one in that case.
    let end_row = if total_cols == 0 {
        0
    } else if total_cols.is_multiple_of(w) {
        total_cols / w - 1
    } else {
        total_cols / w
    };
    let cursor_row = cursor_abs / w;
    let cursor_col = cursor_abs % w;
    (end_row, cursor_row, cursor_col)
}

/// Stateful, wrap-aware renderer for the raw-mode prompt line.
///
/// In raw mode the terminal performs no cursor or wrap bookkeeping for us, so the
/// renderer tracks how many physical rows down from the top of the rendered block
/// the cursor currently sits (`cursor_row`). On each render it moves back up to the
/// top of the previous block, clears it (including any wrapped rows), reprints the
/// prompt + line, and positions the cursor at the correct (row, column) computed
/// from display widths — fixing both multibyte cursor drift and line wrapping.
struct PromptRenderer {
    /// Physical row offset of the cursor from the top of the last rendered block.
    cursor_row: u16,
}

impl PromptRenderer {
    fn new() -> Self {
        Self { cursor_row: 0 }
    }

    /// Mark the cursor as being on a fresh, empty line (no previous block above).
    /// Call this after emitting a newline outside the renderer (e.g. after submit
    /// or printing an out-of-band message).
    fn reset(&mut self) {
        self.cursor_row = 0;
    }

    fn terminal_width() -> usize {
        terminal::size().map(|(w, _)| w as usize).unwrap_or(80).max(1)
    }

    /// Move to the top-left of the previously rendered block and clear it
    /// (including any wrapped rows below). Leaves the cursor at column 0 and
    /// resets the tracked row to 0.
    fn clear_block(&mut self, stdout: &mut std::io::Stdout) {
        use crossterm::cursor::{MoveToColumn, MoveUp};
        if self.cursor_row > 0 {
            let _ = stdout.execute(MoveUp(self.cursor_row));
        }
        let _ = stdout.execute(MoveToColumn(0));
        let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
        self.cursor_row = 0;
        let _ = stdout.flush();
    }

    /// Finalize a submitted line: clear the in-progress (cursor-positioned)
    /// render, then re-emit `prompt` + the submitted `line` as a static, fully
    /// visible line followed by a newline — like a normal shell echoing input on
    /// Enter. Leaves the cursor at column 0 of a fresh line.
    fn finish_line(
        &mut self,
        stdout: &mut std::io::Stdout,
        theme: &Theme,
        prompt_role: Role,
        prompt: &str,
        line: &str,
    ) {
        use crossterm::style::Print;
        // Remove the interactive render (cursor may be mid-line) before echoing.
        self.clear_block(stdout);
        let _ = stdout.execute(Print(theme.out(prompt_role, prompt)));
        let _ = stdout.execute(Print(theme.out(Role::InputText, line)));
        // Advance to a fresh line; CR+LF is required in raw mode.
        let _ = stdout.execute(Print("\r\n"));
        self.cursor_row = 0;
        let _ = stdout.flush();
    }

    /// Render `prompt` + the editor buffer, wrapping correctly and positioning the
    /// cursor at the logical insertion point. When `suggestion` is `Some`, a
    /// one-line candidate hint is printed **above** the prompt line (used for
    /// live slash-command completion), so the line being typed always stays at
    /// the bottom of the block where the eye already is. The hint occupies one
    /// extra row at the top of the block, which `clear_block` removes on the
    /// next render (it moves to the block top and clears downward).
    fn render(
        &mut self,
        stdout: &mut std::io::Stdout,
        theme: &Theme,
        prompt_role: Role,
        prompt: &str,
        state: &InputState,
        suggestion: Option<&str>,
    ) {
        use crossterm::cursor::{MoveDown, MoveToColumn, MoveUp};
        use crossterm::style::Print;

        let width = Self::terminal_width();

        // 1. Clear the previous block (handles multi-row renders + suggestion).
        self.clear_block(stdout);

        // 2. Print the candidate hint first, on its own row above the prompt.
        //    Skipped when the terminal is too narrow to be useful (e.g. a pty
        //    with no reported size), so we never render a lone ellipsis.
        let sug_rows = suggestion_rows(suggestion, width);
        if sug_rows > 0 {
            let sug = truncate_suggestion(suggestion.unwrap_or(""), width - 1);
            let _ = stdout.execute(Print(theme.out(Role::Hint, &sug)));
            // CR+LF is required in raw mode; this lands at column 0 of the
            // prompt row, one row below the hint.
            let _ = stdout.execute(Print("\r\n"));
        }

        // 3. Print prompt + line; the terminal auto-wraps long content.
        let _ = stdout.execute(Print(theme.out(prompt_role, prompt)));
        let _ = stdout.execute(Print(theme.out(Role::InputText, &state.line)));

        // 4. Compute the target cursor position within the prompt rows. The
        //    measurements are taken from the *plain* text: a style adds no
        //    printable column, but its escape sequence would be counted.
        let prompt_cols = crate::editor::str_display_width(prompt);
        let total_cols = prompt_cols + state.display_width();
        let cursor_cols = state.display_cursor();
        let (end_row, cursor_row, cursor_col) =
            layout_cursor(prompt_cols, cursor_cols, total_cols, width);

        // 5. Move from the post-print position (end_row) to the cursor row.
        //    Both are relative to the prompt row, so the hint above does not
        //    change this step — only the block-relative bookkeeping in 6.
        if end_row > cursor_row {
            let _ = stdout.execute(MoveUp((end_row - cursor_row) as u16));
        } else if cursor_row > end_row {
            // Cursor belongs on a fresh wrapped row past the printed content
            // (cursor at the end of a line that exactly fills the width).
            let _ = stdout.execute(MoveDown((cursor_row - end_row) as u16));
        }
        let _ = stdout.execute(MoveToColumn(cursor_col as u16));

        // 6. Remember where the cursor ended up for the next render, counting
        //    the hint row so `clear_block` moves all the way back to the top.
        self.cursor_row = sug_rows + cursor_row as u16;
        let _ = stdout.flush();
    }
}

/// Number of rows the candidate hint occupies above the prompt: 1 when there is
/// something to show and the terminal is wide enough, 0 otherwise. Pure so the
/// renderer's row bookkeeping can be unit-tested without a TTY.
fn suggestion_rows(suggestion: Option<&str>, width: usize) -> u16 {
    match suggestion {
        Some(s) if !s.is_empty() && width >= 8 => 1,
        _ => 0,
    }
}

/// Whether a click at absolute row `row` lands on the progress indicator's
/// block. While a turn runs the indicator is the last writer, so the cursor
/// stands on the block's final row and the block covers `painted_rows` rows
/// ending there. Pure so it can be unit-tested without a terminal.
fn click_hits_indicator(painted_rows: u16, cursor_row: u16, row: u16) -> bool {
    if painted_rows == 0 {
        return false;
    }
    let top = cursor_row.saturating_sub(painted_rows - 1);
    row >= top && row <= cursor_row
}

/// Classify whether a key event should trigger a cancel during the Pending state
/// (LLM executing / tool running). Returns `true` for ESC and Ctrl-C, `false` for
/// all other keys. This is a pure function so it can be unit-tested without a TTY.
fn is_cancel_key(key_event: &KeyEvent) -> bool {
    matches!(key_event.code, KeyCode::Esc)
        || (key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c')))
}

/// Display-side decision for one agent event while `outstanding` cancellations
/// are still draining. Returns `(drop_event, notify_idle, new_outstanding)`.
///
/// With nothing outstanding this is the historical behaviour: show the event,
/// and notify idle on a terminal one. With a cancellation outstanding, every
/// event is dropped up to and including that turn's terminal event, which
/// consumes exactly one outstanding cancellation and sends no idle. Since the
/// event channel is FIFO and every turn emits exactly one terminal event, this
/// suppresses precisely the cancelled turn's remainder — even when a new prompt
/// was already submitted.
fn suppression_decision(is_terminal: bool, outstanding: usize) -> (bool, bool, usize) {
    if outstanding == 0 {
        (false, is_terminal, 0)
    } else if is_terminal {
        (true, false, outstanding - 1)
    } else {
        (true, false, outstanding)
    }
}

/// Book-keeping for a forced return to the prompt: raise the cancellation and
/// record that one more turn is now draining. Returns the new outstanding count.
fn begin_cancel(cancel: &CancelToken, suppress: &AtomicUsize) -> usize {
    cancel.cancel();
    suppress.fetch_add(1, Ordering::SeqCst) + 1
}

/// Handle a single key event in interactive (raw-mode) editing.
/// Returns `Some(action)` indicating what to do next.
enum KeyAction {
    /// Continue editing (prompt was redrawn).
    Continue,
    /// Submit the current line content.
    Submit(String),
    /// Clear the current line (Ctrl+C or Escape).
    ClearLine,
    /// EOF / quit signal.
    Eof,
}

/// Navigate history up and return the history entries for `InputState`.
/// This function needs the history slice, which we read from `ReplState`.
fn handle_key(key_event: KeyEvent, input: &mut InputState, history: &[String]) -> Option<KeyAction> {
    // Ignore key release events (Windows sends both press and release)
    if key_event.kind == crossterm::event::KeyEventKind::Release {
        return None;
    }

    let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);

    match key_event.code {
        KeyCode::Enter => {
            let line = input.submit();
            Some(KeyAction::Submit(line))
        }
        KeyCode::Char(c) if ctrl => match c {
            'a' => {
                input.move_home();
                Some(KeyAction::Continue)
            }
            'e' => {
                input.move_end();
                Some(KeyAction::Continue)
            }
            'c' => {
                // Ctrl+C: raw mode suppresses the terminal's SIGINT, so handle it
                // explicitly. With pending input, first clear the line; on an empty
                // line, exit cleanly (matches the documented "Ctrl+C ... exit" and
                // the previous cooked-mode SIGINT behavior).
                if input.line.is_empty() {
                    Some(KeyAction::Eof)
                } else {
                    input.clear_line();
                    Some(KeyAction::ClearLine)
                }
            }
            'd' => {
                // Ctrl+D: EOF if line is empty, otherwise no-op
                if input.line.is_empty() {
                    Some(KeyAction::Eof)
                } else {
                    Some(KeyAction::Continue)
                }
            }
            _ => None,
        },
        KeyCode::Char(c) => {
            input.insert_char(c);
            Some(KeyAction::Continue)
        }
        KeyCode::Backspace => {
            input.backspace();
            Some(KeyAction::Continue)
        }
        KeyCode::Delete => {
            input.delete();
            Some(KeyAction::Continue)
        }
        KeyCode::Left => {
            input.move_left();
            Some(KeyAction::Continue)
        }
        KeyCode::Right => {
            input.move_right();
            Some(KeyAction::Continue)
        }
        KeyCode::Home => {
            input.move_home();
            Some(KeyAction::Continue)
        }
        KeyCode::End => {
            input.move_end();
            Some(KeyAction::Continue)
        }
        KeyCode::Up => {
            // History navigation: move to older entry (FR-03)
            input.navigate_up(history);
            Some(KeyAction::Continue)
        }
        KeyCode::Down => {
            // History navigation: move to newer entry (FR-03)
            input.navigate_down(history);
            Some(KeyAction::Continue)
        }
        KeyCode::Esc => {
            // Escape: exit history browse if browsing, otherwise clear line
            if input.history_index.is_some() {
                input.exit_history();
                Some(KeyAction::Continue)
            } else {
                input.clear_line();
                Some(KeyAction::ClearLine)
            }
        }
        KeyCode::Tab => {
            // Completion is handled by the raw-mode loop before this point (it
            // needs the custom-command list); reaching here means there was
            // nothing to complete, so just redraw.
            Some(KeyAction::Continue)
        }
        _ => None,
    }
}

/// Check if stdin is a TTY (for deciding whether to enable raw mode).
fn stdin_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// Main loop that converts line input from stdin (or any `AsyncRead`) into `AgentInput`.
///
/// When `interactive` is `true` and stdin is a TTY, uses crossterm raw-mode input
/// with history navigation (up/down arrows) and line editing (FR-01/FR-02/FR-03).
/// When `interactive` is `false` (tests, piped input), falls back to line-oriented
/// `BufReader::lines()` reading (original behaviour).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_input_loop<R>(
    reader: R,
    input_tx: mpsc::Sender<AgentInput>,
    state: Arc<ReplState>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    agent_idle_rx: mpsc::Receiver<()>,
    approval_rx: mpsc::Receiver<ApprovalRequest>,
    interactive: bool,
) where
    R: AsyncRead + Unpin,
{
    // Determine whether to use crossterm raw mode
    let use_raw_mode = interactive && stdin_is_tty();

    if use_raw_mode {
        run_input_loop_raw(
            input_tx,
            state,
            shutdown_tx,
            shutdown_rx,
            agent_idle_rx,
            approval_rx,
        )
        .await;
    } else {
        run_input_loop_line(
            reader,
            input_tx,
            state,
            shutdown_tx,
            shutdown_rx,
            agent_idle_rx,
            approval_rx,
            interactive,
        )
        .await;
    }
}

/// Raw-mode input loop with crossterm (FR-01/FR-02/FR-03).
/// Uses `crossterm::event::poll` + `read()` for key events inside `tokio::select!`.
#[allow(clippy::too_many_arguments)]
async fn run_input_loop_raw(
    input_tx: mpsc::Sender<AgentInput>,
    state: Arc<ReplState>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    agent_idle_rx: mpsc::Receiver<()>,
    approval_rx: mpsc::Receiver<ApprovalRequest>,
) {
    // Enable raw mode; guard restores on drop (NFR-04)
    let _raw_guard = match RawModeGuard::enable() {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("failed to enable raw mode: {e}");
            // Fall back to non-raw mode — but we can't do history navigation without raw mode.
            // Just proceed with line-oriented input using stdin.
            let stdin = tokio::io::stdin();
            run_input_loop_line(
                stdin,
                input_tx,
                state,
                shutdown_tx,
                shutdown_rx,
                agent_idle_rx,
                approval_rx,
                true,
            )
            .await;
            return;
        }
    };

    let mut input_state = InputState::new();
    let mut prompt_state = PromptState::Ready;
    let mut approval_rx: Option<mpsc::Receiver<ApprovalRequest>> = Some(approval_rx);
    let mut stdout = std::io::stdout();
    let mut shutdown_rx = shutdown_rx;
    let mut agent_idle_rx = agent_idle_rx;
    let mut renderer = PromptRenderer::new();
    const PROMPT: &str = "> ";
    const APPROVAL_PROMPT: &str = "approve? [y/N]: ";

    // Draw initial prompt
    render_prompt(&mut renderer, &mut stdout, PROMPT, &state, &input_state).await;

    loop {
        // Read current history for navigation (deduplicated for smooth up/down browsing)
        let history_snapshot: Vec<String> = {
            let h = state.history.read().await;
            dedup_consecutive(&h)
        };

        // Key events are polled in all states (Ready, Pending, AwaitingApproval)
        // so the user can interrupt LLM execution with ESC or Ctrl-C.

        tokio::select! {
            biased;
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
            // Only wait for AI response completion when in Pending state
            res = agent_idle_rx.recv(), if prompt_state.is_pending() => {
                match res {
                    Some(()) => {
                        prompt_state = PromptState::Ready;
                        // Redraw prompt after AI response on a fresh line
                        renderer.reset();
                        render_prompt(&mut renderer, &mut stdout, PROMPT, &state, &input_state).await;
                    }
                    None => break,
                }
            }
            // Approval request arrived (only when not AwaitingApproval). FR-04-1
            req = async {
                match approval_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending::<Option<ApprovalRequest>>().await,
                }
            }, if !prompt_state.is_awaiting_approval() && approval_rx.is_some() => {
                match req {
                    Some(req) => {
                        // Clear the current (possibly wrapped) prompt block, then print
                        // the approval request on a fresh line. The indicator is
                        // suspended for as long as the approval prompt owns the
                        // bottom of the screen.
                        lock_indicator(&state.indicator).suspend();
                        renderer.clear_block(&mut stdout);
                        raw_println(
                            true,
                            &state.theme.out(
                                Role::Confirm,
                                &format!("[tool approval] {} {}", req.tool_name, req.args),
                            ),
                        );
                        renderer.reset();
                        // Reset input state for the approval prompt
                        input_state = InputState::new();
                        prompt_state = PromptState::AwaitingApproval(req.response);
                        renderer.render(&mut stdout, &state.theme, Role::Confirm, APPROVAL_PROMPT, &input_state, None);
                    }
                    None => {
                        approval_rx = None;
                    }
                }
            }
            // Poll for crossterm key events in all states (Ready, Pending, AwaitingApproval).
            // During Pending, ESC and Ctrl-C send AgentInput::Cancel; other keys are ignored.
            _ = tokio::task::spawn_blocking(move || {
                // This blocks the calling thread until a key event or timeout.
                // We use a short timeout so tokio::select! can check other branches frequently.
                let _ = event::poll(Duration::from_millis(50));
            }) => {
                // Drain all pending key events. `exit_loop` lets an inner break
                // (quit/exit command, EOF, closed channel) propagate out of the
                // event-drain `while` to terminate the outer input loop — a plain
                // `break` here would only stop draining events, not exit the REPL.
                let mut exit_loop = false;
                while event::poll(Duration::from_millis(0)).unwrap_or(false) {
                    let ct_event = event::read();
                    // Mouse reporting is only on while a turn is running, so a
                    // click there is aimed at the progress indicator: clicking
                    // its block switches the live `thinking` view between the
                    // last few rows and everything the terminal can hold.
                    if let Ok(CtEvent::Mouse(mouse_event)) = &ct_event {
                        if prompt_state.is_pending()
                            && matches!(
                                mouse_event.kind,
                                crossterm::event::MouseEventKind::Down(
                                    crossterm::event::MouseButton::Left
                                )
                            )
                        {
                            let painted = lock_indicator(&state.indicator).painted_rows();
                            // Without a cursor report the row cannot be placed;
                            // a click during a turn is then taken at face value.
                            let hit = match crossterm::cursor::position() {
                                Ok((_, cursor_row)) => {
                                    click_hits_indicator(painted, cursor_row, mouse_event.row)
                                }
                                Err(_) => painted > 0,
                            };
                            if hit {
                                lock_indicator(&state.indicator).toggle_expanded();
                            }
                        }
                        continue;
                    }
                    if let Ok(CtEvent::Key(key_event)) = ct_event {
                        // During Pending, only ESC and Ctrl-C are honoured (cancel).
                        // Other key events are ignored to avoid corrupting the edit
                        // buffer while the agent is processing.
                        if prompt_state.is_pending() {
                            if is_cancel_key(&key_event) {
                                // Forced return: raise the cancellation and take the
                                // prompt back immediately, without waiting for the
                                // agent to unwind. The token is the whole signal —
                                // queueing an `AgentInput::Cancel` here would only be
                                // read once the turn had already ended, and would then
                                // print over the freshly drawn prompt.
                                begin_cancel(&state.cancel, &state.suppress);
                                // Take the bottom rows back from the indicator
                                // before printing over them.
                                lock_indicator(&state.indicator).suspend();
                                raw_eprintln(true, &state.theme.err(Role::Cancelled, "[cancelled]"));
                                prompt_state = PromptState::Ready;
                                renderer.reset();
                                render_prompt(&mut renderer, &mut stdout, PROMPT, &state, &input_state).await;
                            }
                            // All other keys during Pending are ignored.
                            continue;
                        }
                        let is_awaiting_approval = prompt_state.is_awaiting_approval();

                        // Esc at the approval prompt denies the pending tool and
                        // cancels the turn (Ctrl-C keeps its exit meaning there).
                        if is_awaiting_approval
                            && key_event.code == KeyCode::Esc
                            && key_event.kind != crossterm::event::KeyEventKind::Release
                        {
                            let prev = std::mem::replace(&mut prompt_state, PromptState::Ready);
                            if let PromptState::AwaitingApproval(resp_tx) = prev {
                                let _ = resp_tx.send(false);
                            }
                            begin_cancel(&state.cancel, &state.suppress);
                            lock_indicator(&state.indicator).suspend();
                            renderer.clear_block(&mut stdout);
                            raw_eprintln(true, &state.theme.err(Role::Cancelled, "[cancelled]"));
                            input_state = InputState::new();
                            renderer.reset();
                            render_prompt(&mut renderer, &mut stdout, PROMPT, &state, &input_state).await;
                            continue;
                        }

                        // Tab completes the slash command being typed. Handled
                        // here rather than in `handle_key` because it needs the
                        // custom-command list, which lives behind an async lock.
                        if key_event.code == KeyCode::Tab
                            && key_event.kind != crossterm::event::KeyEventKind::Release
                        {
                            let customs: Vec<String> = state
                                .commands
                                .read()
                                .await
                                .iter()
                                .map(|c| c.name.clone())
                                .collect();
                            if let Some(completed) =
                                command_completion(&input_state.line, &customs)
                            {
                                input_state.line = completed;
                                input_state.move_end();
                            }
                            render_prompt(&mut renderer, &mut stdout, PROMPT, &state, &input_state).await;
                            continue;
                        }

                        if let Some(action) = handle_key(key_event, &mut input_state, &history_snapshot) {
                            match action {
                                KeyAction::Submit(line) => {
                                    let trimmed = line.trim().to_string();

                                    if is_awaiting_approval {
                                        // Approval response: y/N
                                        let approved = matches!(
                                            trimmed.to_ascii_lowercase().as_str(),
                                            "y" | "yes"
                                        );
                                        let prev = std::mem::replace(&mut prompt_state, PromptState::Pending);
                                        if let PromptState::AwaitingApproval(resp_tx) = prev {
                                            let _ = resp_tx.send(approved);
                                        }
                                        // Drain stale idle notifications
                                        while agent_idle_rx.try_recv().is_ok() {}
                                        // Echo the approval response, then move to next line
                                        renderer.finish_line(&mut stdout, &state.theme, Role::Confirm, APPROVAL_PROMPT, &line);
                                        // The turn continues: hand the bottom
                                        // rows back to the indicator.
                                        lock_indicator(&state.indicator).resume();
                                        continue;
                                    }

                                    // Echo the submitted line, then move to next line
                                    renderer.finish_line(&mut stdout, &state.theme, Role::PromptSymbol, PROMPT, &line);

                                    if let Some(rest) = trimmed.strip_prefix('/') {
                                        match handle_repl_command(rest, &input_tx, &state, true).await {
                                            CommandResult::Quit => {
                                                exit_loop = true;
                                                break;
                                            }
                                            CommandResult::SubmittedPrompt => {
                                                while agent_idle_rx.try_recv().is_ok() {}
                                                prompt_state = PromptState::Pending;
                                            }
                                            CommandResult::Continue => {
                                                // Redraw prompt after command
                                                render_prompt(&mut renderer, &mut stdout, PROMPT, &state, &input_state).await;
                                            }
                                        }
                                        continue;
                                    }
                                    if trimmed.is_empty() {
                                        // Blank line: just redraw prompt
                                        render_prompt(&mut renderer, &mut stdout, PROMPT, &state, &input_state).await;
                                        continue;
                                    }

                                    // Save to history (persistent + in-memory, skip consecutive duplicates)
                                    {
                                        let last_line = state.history.read().await.last().cloned();
                                        append_history(&state.history_path, &trimmed, last_line.as_deref());
                                    }
                                    {
                                        let mut h = state.history.write().await;
                                        if h.last().map(|last| last == &trimmed).unwrap_or(false) {
                                            // Skip consecutive duplicate
                                        } else {
                                            h.push(trimmed.clone());
                                            let len = h.len();
                                            if len > HISTORY_LIMIT {
                                                h.drain(..len - HISTORY_LIMIT);
                                            }
                                        }
                                    }

                                    if input_tx.send(AgentInput::UserPrompt(trimmed)).await.is_err() {
                                        exit_loop = true;
                                        break;
                                    }
                                    // Discard stale idle notifications from past peer prompts, then enter Pending
                                    while agent_idle_rx.try_recv().is_ok() {}
                                    prompt_state = PromptState::Pending;
                                }
                                KeyAction::ClearLine => {
                                    // Redraw prompt with cleared state
                                    if prompt_state.is_awaiting_approval() {
                                        renderer.render(&mut stdout, &state.theme, Role::Confirm, APPROVAL_PROMPT, &input_state, None);
                                    } else {
                                        render_prompt(&mut renderer, &mut stdout, PROMPT, &state, &input_state).await;
                                    }
                                }
                                KeyAction::Eof => {
                                    exit_loop = true;
                                    break;
                                }
                                KeyAction::Continue => {
                                    if prompt_state.is_awaiting_approval() {
                                        renderer.render(&mut stdout, &state.theme, Role::Confirm, APPROVAL_PROMPT, &input_state, None);
                                    } else {
                                        render_prompt(&mut renderer, &mut stdout, PROMPT, &state, &input_state).await;
                                    }
                                }
                            }
                        }
                    }
                    // Ignore resize and other events in this inner loop
                }
                if exit_loop {
                    break;
                }
            }
        }
    }

    // When exiting AwaitingApproval via the shutdown route, send false (deny) to the
    // agent's oneshot::Receiver to avoid leaving it dangling (design doc 4.3A).
    if let PromptState::AwaitingApproval(resp_tx) =
        std::mem::replace(&mut prompt_state, PromptState::Ready)
    {
        let _ = resp_tx.send(false);
    }
    // Ensure the prompt block (possibly wrapped) is cleaned up before exit
    lock_indicator(&state.indicator).suspend();
    renderer.clear_block(&mut stdout);
    let _ = shutdown_tx.send(true);
    // _raw_guard dropped here: terminal mode restored
}

/// Line-oriented input loop (original behaviour, used for non-interactive / test mode).
#[allow(clippy::too_many_arguments)]
async fn run_input_loop_line<R>(
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
    // Once the approval channel closes, set to `None` and switch to `pending()` wait (avoid busy loop)
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
            // Only wait for AI response completion when in Pending state (stdin is paused)
            res = agent_idle_rx.recv(), if prompt_state.is_pending() => {
                match res {
                    Some(()) => {
                        prompt_state = PromptState::Ready;
                        // Prompt will be redrawn at the top of the next loop iteration
                    }
                    None => break, // display_task terminated -> no more events coming
                }
            }
            // Approval request arrived (only when not AwaitingApproval). FR-04-1
            req = async {
                match approval_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending::<Option<ApprovalRequest>>().await,
                }
            }, if !prompt_state.is_awaiting_approval() && approval_rx.is_some() => {
                match req {
                    Some(req) => {
                        if interactive {
                            raw_println(false, "");
                            println!(
                                "{}",
                                state.theme.out(
                                    Role::Confirm,
                                    &format!("[tool approval] {} {}", req.tool_name, req.args),
                                )
                            );
                            print!("{}", state.theme.out(Role::Confirm, "approve? [y/N]: "));
                            let _ = std::io::stdout().flush();
                        }
                        prompt_state = PromptState::AwaitingApproval(req.response);
                    }
                    None => {
                        // Approval channel closed -> don't wait anymore
                        approval_rx = None;
                    }
                }
            }
            // Read from stdin when Ready or AwaitingApproval
            next = lines.next_line(), if prompt_state.is_ready() || prompt_state.is_awaiting_approval() => {
                match next {
                    Ok(Some(line)) => {
                        let trimmed = line.trim_end_matches('\r').trim().to_string();

                        // If AwaitingApproval, interpret input as y/N and send response to agent
                        if prompt_state.is_awaiting_approval() {
                            let approved = matches!(
                                trimmed.to_ascii_lowercase().as_str(),
                                "y" | "yes"
                            );
                            // Transition to Pending while extracting and sending the oneshot
                            let prev = std::mem::replace(&mut prompt_state, PromptState::Pending);
                            if let PromptState::AwaitingApproval(resp_tx) = prev {
                                let _ = resp_tx.send(approved);
                            }
                            // Wait for tool execution -> follow-up -> Done. Drain stale idle notifications.
                            while agent_idle_rx.try_recv().is_ok() {}
                            continue;
                        }

                        if let Some(rest) = trimmed.strip_prefix('/') {
                            match handle_repl_command(rest, &input_tx, &state, false).await {
                                CommandResult::Quit => break,
                                CommandResult::SubmittedPrompt => {
                                    while agent_idle_rx.try_recv().is_ok() {}
                                    prompt_state = PromptState::Pending;
                                }
                                CommandResult::Continue => {}
                            }
                            continue;
                        }
                        if trimmed.is_empty() {
                            continue;
                        }
                        // Save to history (persistent + in-memory, skip consecutive duplicates)
                        {
                            let last_line = state.history.read().await.last().cloned();
                            append_history(&state.history_path, &trimmed, last_line.as_deref());
                        }
                        {
                            let mut h = state.history.write().await;
                            if h.last().map(|last| last == &trimmed).unwrap_or(false) {
                                // Skip consecutive duplicate
                            } else {
                                h.push(trimmed.clone());
                                let len = h.len();
                                if len > HISTORY_LIMIT {
                                    h.drain(..len - HISTORY_LIMIT);
                                }
                            }
                        }
                        if input_tx.send(AgentInput::UserPrompt(trimmed)).await.is_err() {
                            break;
                        }
                        // Discard stale idle notifications from past peer prompts, then enter Pending
                        while agent_idle_rx.try_recv().is_ok() {}
                        prompt_state = PromptState::Pending;
                    }
                    Ok(None) => break, // EOF (Ctrl+D)
                    Err(_) => break,
                }
            }
        }
    }
    // When exiting AwaitingApproval via the shutdown route, send false (deny) to the
    // agent's oneshot::Receiver to avoid leaving it dangling (design doc 4.3A).
    if let PromptState::AwaitingApproval(resp_tx) =
        std::mem::replace(&mut prompt_state, PromptState::Ready)
    {
        let _ = resp_tx.send(false);
    }
    let _ = shutdown_tx.send(true);
}

/// Wait for SIGINT (Ctrl+C) or SIGTERM. On non-Linux, only `ctrl_c` is available.
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
    theme: &Theme,
) {
    let display_name = name.unwrap_or("(unnamed)");
    // The banner carries the brand colour; its detail rows are dimmed, so the
    // eye lands on the name and moves on.
    let detail = |line: String| println!("{}", theme.out(Role::BannerDetail, &line));
    println!("{}", theme.out(Role::Banner, "agent-cli ready"));
    detail(format!("  id        : {id}"));
    detail(format!("  name      : {display_name}"));
    detail(format!("  provider  : {provider} ({model})"));
    detail(format!(
        "  features  : streaming={} tool_use={} thinking={}",
        caps.streaming, caps.tool_use, caps.thinking
    ));
    detail(format!("  role      : {}", persona.frontmatter.role));
    if !persona.frontmatter.skills.is_empty() {
        detail(format!(
            "  skills    : {}",
            persona.frontmatter.skills.join(", ")
        ));
    }
    detail("type /help for commands. /quit, /exit, or ^D to terminate.".to_string());
}

/// `/auto [on|off|status]` handler (FR-04-2 / design doc 4.3A).
fn handle_auto_command(arg: &str, state: &Arc<ReplState>, raw_mode: bool) {
    let arg = arg.trim().to_ascii_lowercase();
    match arg.as_str() {
        "on" | "true" | "1" => {
            state.auto_approve.store(true, Ordering::SeqCst);
            let msg = "[auto] tool approval: on (skipping y/N prompts)";
            raw_println(raw_mode, &state.theme.out(Role::Info, msg));
        }
        "off" | "false" | "0" => {
            state.auto_approve.store(false, Ordering::SeqCst);
            let msg = "[auto] tool approval: off (will ask y/N for each tool call)";
            raw_println(raw_mode, &state.theme.out(Role::Info, msg));
        }
        "" | "status" => {
            let cur = if state.auto_approve.load(Ordering::SeqCst) {
                "on"
            } else {
                "off"
            };
            let msg = format!("[auto] tool approval: {cur}");
            raw_println(raw_mode, &state.theme.out(Role::Info, &msg));
        }
        other => {
            let msg = format!("usage: /auto [on|off|status]  (got: {other})");
            raw_eprintln(raw_mode, &state.theme.err(Role::Info, &msg));
        }
    }
}

fn print_prompt() {
    print!("> ");
    let _ = std::io::stdout().flush();
}

struct DisplayState {
    thinking_printed: bool,
    answer_printed: bool,
    /// Whether any section header (`[answer]`/`[thinking]`/`[tool-call]`) has been
    /// printed in the current turn. The first header of a turn is emitted without a
    /// leading newline so it sits directly under the echoed input line, avoiding an
    /// extra blank line (FR-15); subsequent headers keep the leading newline to
    /// separate from preceding streamed content.
    section_printed: bool,
    /// When true, output is in crossterm raw mode and newlines must use CR+LF.
    raw_mode: bool,
    /// When true the progress indicator is drawing beneath the output, so the
    /// lines around it are kept short: a tool call is cut to a single terminal
    /// row and a tool result to [`TOOL_RESULT_ROWS`] rows, since the raw dumps
    /// would otherwise push the indicator many rows down.
    compact_output: bool,
    /// When true, `thinking` text is not printed inline: the progress
    /// indicator shows it live under the spinner instead.
    capture_thinking: bool,
    /// Colour scheme. Styling is applied at the print sites below, always
    /// *after* the line has been measured and cut, so the column maths in
    /// `crate::editor` never sees an escape sequence.
    theme: Theme,
}

impl DisplayState {
    fn new(raw_mode: bool) -> Self {
        Self {
            thinking_printed: false,
            answer_printed: false,
            section_printed: false,
            raw_mode,
            compact_output: false,
            capture_thinking: false,
            theme: Theme::plain(),
        }
    }

    fn reset(&mut self) {
        self.thinking_printed = false;
        self.answer_printed = false;
        self.section_printed = false;
    }

    /// Print a section header (e.g. `[answer]`), prefixing a newline only if a
    /// previous section already printed this turn (FR-15), styled with `role`.
    fn section_header(&mut self, label: &str, role: Role) {
        let styled = self.theme.err(role, label).into_owned();
        self.section_header_styled(&styled);
    }

    /// Same, for a header the caller has already styled — the `[tool-call]`
    /// line, whose name and arguments carry different roles.
    ///
    /// The separating newline is added around the styled text, never inside it,
    /// so a blank row never carries a colour into a region the display erases.
    fn section_header_styled(&mut self, styled: &str) {
        raw_eprintln(
            self.raw_mode,
            &section_header_text(styled, self.section_printed),
        );
        self.section_printed = true;
    }
}

/// Build a section header line: prefix a newline only when a previous section has
/// already printed this turn, so the first header sits directly under the echoed
/// input with no extra blank line (FR-15 / Defect #7).
fn section_header_text(label: &str, section_printed: bool) -> String {
    if section_printed {
        format!("\n{label}")
    } else {
        label.to_string()
    }
}

/// Spinner frames, advanced one per tick while a turn runs.
const SPINNER_FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Repaint interval of the progress indicator.
const TICK_INTERVAL: Duration = Duration::from_millis(100);

/// Rows of live `thinking` text shown under the spinner before the view is
/// elided. Clicking the block shows as much as the terminal can hold instead.
const THINKING_ROWS: usize = 10;

/// Rows a tool result is cut to while the progress indicator is drawing, so a
/// large output cannot push the indicator off the screen. The full text is
/// always in the conversation log.
const TOOL_RESULT_ROWS: usize = 5;

/// Spinner frame `n`, cycling through [`SPINNER_FRAMES`].
fn spinner_frame(n: usize) -> char {
    SPINNER_FRAMES[n % SPINNER_FRAMES.len()]
}

/// Elapsed time of a turn: one decimal below a minute (`0.4s`, `12.4s`),
/// minutes and whole seconds above it (`1m00s`, `2m03s`). Truncated rather
/// than rounded, so the display never briefly shows `60.0s`.
fn format_elapsed(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        let tenths = d.as_millis() / 100;
        format!("{}.{}s", tenths / 10, tenths % 10)
    } else {
        format!("{}m{:02}s", secs / 60, secs % 60)
    }
}

/// One-line rendering of a tool call for the indicator's activity row. Mirrors
/// the wording of the permanent `[tool-call]` line so the two read the same.
fn one_line_tool_call(name: &str, args: &serde_json::Value) -> String {
    crate::editor::flatten_one_line(&format!("[tool-call] {name} {args}"))
}

/// A tool result cut to `max_rows` terminal rows, with a marker counting what
/// was left out — the counterpart of the one-line tool call, so a `bash` dump
/// cannot push the progress indicator far down the screen. The text is wrapped
/// (not cut) at `cols`, so the visible part is complete as far as it goes.
fn compact_tool_result(
    mark: &str,
    name: &str,
    output: &str,
    cols: usize,
    max_rows: usize,
) -> String {
    let full = format!("[tool-result {mark}] {name}: {output}");
    let rows = crate::editor::wrap_display(&full, cols);
    if rows.len() <= max_rows {
        return full;
    }
    let hidden = rows.len() - max_rows;
    let mut out = rows[..max_rows].join("\n");
    out.push_str(&format!("\n… +{hidden} more lines"));
    out
}

/// Style a finished `[tool-call] <name> <args>` line: the marker and the tool
/// name carry the activity colour, the arguments are dimmed to grey.
///
/// The split is on the second space, which is exact because the line is built
/// as `"[tool-call] {name} {args}"` and a tool name never contains a space. A
/// line already truncated to the terminal width may end before that point, in
/// which case the whole of it is the name.
fn style_tool_call_line(theme: &Theme, line: &str) -> String {
    match line
        .match_indices(' ')
        .nth(1)
        .map(|(i, _)| line.split_at(i + 1))
    {
        // The space between the two parts stays outside both sequences.
        Some((head, args)) => format!(
            "{} {}",
            theme.err(Role::ToolName, head.trim_end()),
            theme.err(Role::ToolArgs, args),
        ),
        None => theme.err(Role::ToolName, line).into_owned(),
    }
}

/// Style a possibly multi-row message row by row, so every row opens and closes
/// its own sequence. A colour must never span a line break: the row below may be
/// erased and redrawn (the progress indicator), or scrolled away.
fn style_rows(theme: &Theme, role: Role, text: &str) -> String {
    if !theme.stderr {
        return text.to_string();
    }
    text.split('\n')
        .map(|row| theme.err(role, row).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether the cursor sits at column 0 of a fresh row once `ev` has been
/// displayed. `Text` leaves the cursor mid-row unless its delta ends with a
/// newline, and `Thinking` always does (except when hidden, which prints
/// nothing at all); every other event is printed as whole lines. `prev` is the
/// state before the event, returned unchanged when the event prints nothing.
fn event_ends_at_line_start(
    ev: &AgentEvent,
    mode: ShowThinkingMode,
    capture_thinking: bool,
    prev: bool,
) -> bool {
    match ev {
        // Prints nothing: the indicator alone reacts to it.
        AgentEvent::TurnStart => prev,
        // Captured by the indicator, so nothing reaches the cursor.
        AgentEvent::Thinking { .. } if capture_thinking => prev,
        AgentEvent::Text { delta } => {
            if delta.is_empty() {
                prev
            } else {
                delta.ends_with('\n')
            }
        }
        AgentEvent::Thinking { .. } => match mode {
            ShowThinkingMode::Hidden => prev,
            _ => false,
        },
        AgentEvent::ToolCall { .. }
        | AgentEvent::ToolResult { .. }
        | AgentEvent::Info { .. }
        | AgentEvent::Done
        | AgentEvent::Error { .. } => true,
    }
}

/// Terminal mark closing a finished turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Ok,
    Err,
}

impl Mark {
    fn glyph(self) -> char {
        match self {
            Mark::Ok => '✔',
            Mark::Err => '✗',
        }
    }
}

/// The turn currently being tracked by the indicator.
struct Run {
    started: std::time::Instant,
    frame: usize,
}

/// Live progress indicator for the running turn.
///
/// While a turn runs it owns the bottom row of the output, showing
/// `<spinner> <elapsed>` directly beneath the line describing what is being
/// executed — the echoed question at first, then each `[tool-call]` line, which
/// the display shortens to a single row while the indicator is on. The row is
/// cut to one terminal row so it can never wrap, which keeps the erase
/// (`MoveToColumn(0)` + `Clear(FromCursorDown)`) exact.
///
/// The block is painted only from column 0 of a fresh row: after streamed text
/// has left the cursor mid-row there is no way to erase the block without
/// destroying that row, so the indicator simply stays unpainted until a whole
/// line has been emitted (the flowing text is itself the sign of activity).
///
/// Writes go through an injectable sink so the rendering can be asserted in
/// tests without a TTY. A disabled indicator (not a terminal, `[ui]
/// show_progress = false`, serve mode) never writes a single byte.
struct StatusIndicator {
    out: Box<dyn std::io::Write + Send>,
    enabled: bool,
    /// Raw mode is on, so line breaks must be CR+LF.
    raw_mode: bool,
    /// Fixed terminal width, used by tests instead of querying the terminal.
    width_override: Option<usize>,
    run: Option<Run>,
    /// The input loop owns the bottom of the screen (cancel notice, approval
    /// prompt, shutdown); the indicator must not paint until it is resumed.
    suspended: bool,
    /// The block is currently on screen.
    painted: bool,
    /// Number of terminal rows the painted block occupies (1 for the spinner
    /// row, plus the live `thinking` rows below it). Needed to move back to the
    /// top of the block when erasing it.
    painted_rows: u16,
    /// Reasoning text streamed so far this turn, shown live under the spinner.
    thinking: String,
    /// Show the whole `thinking` text (as much as the terminal can hold)
    /// instead of the last [`THINKING_ROWS`] rows. Toggled by clicking the
    /// block; kept across turns, since it is a view preference.
    expanded: bool,
    /// Fixed terminal height, used by tests instead of querying the terminal.
    height_override: Option<usize>,
    /// Whether reasoning is shown at all (`[ui] show_thinking` is not
    /// `hidden`). With no `thinking` view there is nothing to click, so mouse
    /// reporting is left alone and text selection keeps working during a turn.
    thinking_view: bool,
    /// Colour scheme. Each row is styled after it has been cut to the terminal
    /// width, and separately from its neighbours, so the row bookkeeping and
    /// the erase are unaffected.
    theme: Theme,
}

impl StatusIndicator {
    fn new(out: Box<dyn std::io::Write + Send>, enabled: bool, raw_mode: bool) -> Self {
        Self {
            out,
            enabled,
            raw_mode,
            width_override: None,
            run: None,
            suspended: false,
            painted: false,
            painted_rows: 0,
            thinking: String::new(),
            expanded: false,
            height_override: None,
            thinking_view: true,
            theme: Theme::plain(),
        }
    }

    /// An indicator that never draws anything: used where there is no
    /// interactive terminal (serve mode, piped stdin, `show_progress = false`).
    fn disabled() -> Self {
        Self::new(Box::new(std::io::sink()), false, false)
    }

    fn newline(&self) -> &'static str {
        if self.raw_mode {
            "\r\n"
        } else {
            "\n"
        }
    }

    fn width(&self) -> usize {
        self.width_override
            .unwrap_or_else(PromptRenderer::terminal_width)
    }

    fn height(&self) -> usize {
        self.height_override.unwrap_or_else(|| {
            terminal::size()
                .map(|(_, h)| h as usize)
                .unwrap_or(24)
                .max(1)
        })
    }

    /// Begin tracking a turn. Any suspension from a previous turn is lifted;
    /// the block itself is drawn by the following [`StatusIndicator::paint`].
    fn start(&mut self) {
        if !self.enabled {
            return;
        }
        self.suspended = false;
        self.thinking.clear();
        self.run = Some(Run {
            started: std::time::Instant::now(),
            frame: 0,
        });
        // Clicking the block toggles the thinking view, so the terminal has to
        // report mouse events — but only while a turn is running, so ordinary
        // text selection keeps working at the prompt.
        self.set_mouse_capture(true);
    }

    /// Append streamed reasoning text to the live `thinking` view.
    fn push_thinking(&mut self, text: &str) {
        if !self.enabled || !self.thinking_view || self.run.is_none() {
            return;
        }
        self.thinking.push_str(text);
    }

    /// Switch the `thinking` view between the last [`THINKING_ROWS`] rows and
    /// as much of the text as the terminal can hold, redrawing the block in
    /// place. Called from the input loop when the block is clicked.
    fn toggle_expanded(&mut self) {
        if !self.enabled || self.run.is_none() {
            return;
        }
        self.expanded = !self.expanded;
        if self.painted && !self.suspended {
            self.repaint();
        }
    }

    /// Number of rows the block occupies on screen, `0` when it is not drawn.
    fn painted_rows(&self) -> u16 {
        if self.painted {
            self.painted_rows
        } else {
            0
        }
    }

    fn set_mouse_capture(&mut self, on: bool) {
        if !self.enabled || !self.raw_mode || !self.thinking_view {
            return;
        }
        let _ = if on {
            crossterm::queue!(self.out, crossterm::event::EnableMouseCapture)
        } else {
            crossterm::queue!(self.out, crossterm::event::DisableMouseCapture)
        };
        let _ = self.out.flush();
    }

    /// The rows drawn below the spinner: the tail of the reasoning text, or as
    /// much of it as fits when expanded, with a marker row counting what was
    /// left out.
    fn thinking_rows(&self) -> Vec<String> {
        if self.thinking.trim().is_empty() {
            return Vec::new();
        }
        let cols = self.width().saturating_sub(1);
        // Indented by two columns so the block reads as a detail of the
        // spinner row above it.
        let rows = crate::editor::wrap_display(self.thinking.trim_end(), cols.saturating_sub(2));
        // Never take more than the screen can hold: the activity line, the
        // spinner row, the marker row and two rows of context stay visible, so
        // expanding cannot push the whole conversation off the screen.
        let budget = self.height().saturating_sub(5).max(1);
        let visible = if self.expanded {
            budget
        } else {
            THINKING_ROWS.min(budget)
        };
        let hidden = rows.len().saturating_sub(visible);
        let mut out: Vec<String> = Vec::with_capacity(visible + 1);
        if hidden > 0 || self.expanded {
            let hint = if self.expanded {
                "click to collapse".to_string()
            } else {
                "click to expand".to_string()
            };
            let marker = if hidden > 0 {
                format!("… +{hidden} more ({hint})")
            } else {
                format!("… ({hint})")
            };
            out.push(crate::editor::truncate_display(&marker, cols));
        }
        out.extend(
            rows[rows.len() - visible.min(rows.len())..]
                .iter()
                .map(|r| crate::editor::truncate_display(&format!("  {r}"), cols)),
        );
        out
    }

    /// Advance the spinner and refresh the elapsed time, but only where the
    /// row is already on screen — a tick never *introduces* a row, so it
    /// cannot land in the middle of streamed output.
    fn tick(&mut self) {
        if self.run.is_none() || !self.enabled || self.suspended || !self.painted {
            return;
        }
        if let Some(run) = self.run.as_mut() {
            run.frame = run.frame.wrapping_add(1);
        }
        self.repaint();
    }

    /// Draw the row when allowed to. `at_line_start` is the caller's knowledge
    /// of where the cursor is.
    fn paint(&mut self, at_line_start: bool) {
        if !self.enabled || self.suspended || self.run.is_none() || !at_line_start {
            return;
        }
        self.repaint();
    }

    fn repaint(&mut self) {
        self.clear();
        let Some(run) = self.run.as_ref() else {
            return;
        };
        let status = crate::editor::truncate_display(
            &format!(
                "{} {}",
                spinner_frame(run.frame),
                format_elapsed(run.started.elapsed())
            ),
            self.width().saturating_sub(1),
        );
        let mut rows = Vec::with_capacity(1 + THINKING_ROWS);
        rows.push(status);
        rows.extend(self.thinking_rows());
        let painted_rows = rows.len() as u16;
        // Style last, once every row has been cut to the terminal width, and
        // row by row so no sequence spans a line break: the block is erased and
        // redrawn ten times a second, and a colour left open would survive it.
        // The reasoning rows are the indented ones; the status row and the
        // elision marker above them belong to the progress colour.
        let styled: Vec<String> = rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let role = if i > 0 && row.starts_with("  ") {
                    Role::Thinking
                } else {
                    Role::Progress
                };
                self.theme.err(role, row).into_owned()
            })
            .collect();
        let nl = self.newline();
        let _ = write!(self.out, "{}", styled.join(nl));
        let _ = self.out.flush();
        self.painted = true;
        self.painted_rows = painted_rows;
    }

    /// Erase the block, leaving the cursor exactly at its origin (column 0 of
    /// the row where painting began), so the output above is untouched and the
    /// next write continues where it would have without the indicator.
    fn clear(&mut self) {
        if !self.painted {
            return;
        }
        use crossterm::cursor::{MoveToColumn, MoveUp};
        // The cursor sits on the last row of the block; go back to its first.
        if self.painted_rows > 1 {
            let _ = crossterm::queue!(self.out, MoveUp(self.painted_rows - 1));
        }
        let _ = crossterm::queue!(
            self.out,
            MoveToColumn(0),
            terminal::Clear(ClearType::FromCursorDown)
        );
        let _ = self.out.flush();
        self.painted = false;
        self.painted_rows = 0;
    }

    /// Close the turn: erase the row and leave one permanent line marking the
    /// outcome and the total elapsed time.
    fn finish(&mut self, mark: Mark) {
        self.clear();
        let Some(run) = self.run.take() else {
            return;
        };
        self.thinking.clear();
        self.set_mouse_capture(false);
        if !self.enabled || self.suspended {
            return;
        }
        let cols = self.width().saturating_sub(1);
        let line = crate::editor::truncate_display(
            &format!("{} {}", mark.glyph(), format_elapsed(run.started.elapsed())),
            cols,
        );
        let role = match mark {
            Mark::Ok => Role::Success,
            Mark::Err => Role::Failure,
        };
        let line = self.theme.err(role, &line);
        let nl = self.newline();
        let _ = write!(self.out, "{line}{nl}");
        let _ = self.out.flush();
    }

    /// Drop the turn without marking it (its output was discarded, e.g. a
    /// cancelled turn draining behind a fresh prompt). Also used when the turn
    /// ends with nothing to mark.
    fn abandon_run(&mut self) {
        self.clear();
        if self.run.take().is_some() {
            self.thinking.clear();
            self.set_mouse_capture(false);
        }
    }

    /// The input loop is about to write at the bottom of the screen.
    fn suspend(&mut self) {
        self.clear();
        self.suspended = true;
    }

    fn resume(&mut self) {
        self.suspended = false;
    }
}

/// Lock the shared indicator, recovering from a poisoned mutex: a panic in one
/// writer must not take the REPL's display down with it.
fn lock_indicator(
    indicator: &std::sync::Mutex<StatusIndicator>,
) -> std::sync::MutexGuard<'_, StatusIndicator> {
    indicator.lock().unwrap_or_else(|e| e.into_inner())
}

fn display_event(ev: AgentEvent, show_thinking: ShowThinkingMode, state: &mut DisplayState) {
    let rm = state.raw_mode;
    match ev {
        // The turn boundary drives the progress indicator only; it prints
        // nothing itself, so every non-interactive path stays byte-identical.
        AgentEvent::TurnStart => {}
        AgentEvent::Text { delta } => {
            if !state.answer_printed {
                state.section_header("[answer]", Role::AnswerMarker);
                state.answer_printed = true;
            }
            // The answer body is the longest thing on screen and is left
            // uncoloured on purpose.
            raw_print_str(rm, &delta);
        }
        // Captured by the progress indicator, which shows it live under the
        // spinner; printing it here too would duplicate it into the scrollback.
        AgentEvent::Thinking { .. } if state.capture_thinking => {}
        AgentEvent::Thinking { text } => match show_thinking {
            ShowThinkingMode::Hidden => {}
            ShowThinkingMode::Collapsed => {
                if !state.thinking_printed {
                    state.section_header("[thinking]", Role::Thinking);
                    state.thinking_printed = true;
                } else {
                    raw_eprint(rm, " ");
                }
                let collapsed = collapse_thinking_text(&text);
                raw_eprint(rm, &state.theme.err(Role::Thinking, &collapsed));
            }
            ShowThinkingMode::Expanded => {
                if !state.thinking_printed {
                    state.section_header("[thinking]", Role::Thinking);
                    state.thinking_printed = true;
                }
                raw_eprint(rm, &state.theme.err(Role::Thinking, &text));
            }
        },
        AgentEvent::ToolCall { name, args } => {
            let line = one_line_tool_call(&name, &args);
            // Cut first, style second: the truncation counts columns, so it
            // must never see an escape sequence.
            let line = if state.compact_output {
                crate::editor::truncate_display(
                    &line,
                    PromptRenderer::terminal_width().saturating_sub(1),
                )
            } else {
                format!("[tool-call] {name} {args}")
            };
            let styled = style_tool_call_line(&state.theme, &line);
            state.section_header_styled(&styled);
        }
        AgentEvent::ToolResult { name, ok, output } => {
            let mark = if ok { "ok" } else { "ERR" };
            let line = if state.compact_output {
                compact_tool_result(
                    mark,
                    &name,
                    &output,
                    PromptRenderer::terminal_width(),
                    TOOL_RESULT_ROWS,
                )
            } else {
                format!("[tool-result {mark}] {name}: {output}")
            };
            raw_eprintln(rm, &style_rows(&state.theme, Role::ToolOutput, &line));
        }
        AgentEvent::Done => {
            state.reset();
            raw_println(rm, "");
        }
        AgentEvent::Error { message } => {
            state.reset();
            let line = format!("[error] {message}");
            // The separating newline stays outside the sequence.
            raw_eprintln(rm, &format!("\n{}", style_rows(&state.theme, Role::Failure, &line)));
        }
        AgentEvent::Info { message } => {
            let line = format!("[info] {message}");
            raw_eprintln(rm, &style_rows(&state.theme, Role::Info, &line));
        }
    }
}

/// Truncate thinking delta to 1 line / 80 characters when `[ui] show_thinking = "collapsed"`
/// (FR-03-1-2 / design doc 4.3C). Prevents models that return long reasoning
/// (e.g. `glm-5.1:cloud`) from flooding the REPL output in a single turn.
fn collapse_thinking_text(text: &str) -> String {
    const MAX: usize = 80;
    let first_line = text.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return String::from("...");
    }
    let truncated_at_chars: String = first_line.chars().take(MAX).collect();
    let truncated = truncated_at_chars.chars().count() < first_line.chars().count();
    let multiline = text.lines().count() > 1 || text.ends_with('\n');
    if truncated || multiline {
        format!("{truncated_at_chars}...")
    } else {
        truncated_at_chars
    }
}

/// Built-in REPL slash command names (the `match cmd` arms in
/// `handle_repl_command`). Used for live candidate suggestions.
const BUILTIN_COMMANDS: &[&str] = &[
    "quit",
    "exit",
    "help",
    "auto",
    "clear",
    "reset",
    "history",
    "list",
    "send",
    "spawn",
    "stop",
    "tools",
    "persona",
    "reload-persona",
    "peer",
    "cancel",
    "commands",
    "reload-commands",
];

/// Return command names (without `/`) that start with `prefix`, built-in names
/// first (in declaration order) then custom names (sorted), deduplicated. A
/// custom command that collides with a built-in is dropped (built-ins take
/// precedence, FR-07). An empty `prefix` matches all.
fn slash_candidates(prefix: &str, custom_names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in BUILTIN_COMMANDS {
        if b.starts_with(prefix) {
            out.push((*b).to_string());
        }
    }
    let builtin_set: std::collections::HashSet<&str> = BUILTIN_COMMANDS.iter().copied().collect();
    let mut customs: Vec<String> = custom_names
        .iter()
        .filter(|c| c.starts_with(prefix) && !builtin_set.contains(c.as_str()))
        .cloned()
        .collect();
    customs.sort();
    out.extend(customs);
    out
}

/// Build the single-line suggestion string shown below the prompt when the
/// user is typing a `/<name>` command (no space yet). Returns `None` when
/// there is nothing useful to suggest (no match, or the typed text is already
/// the sole exact match).
fn command_suggestion(prefix: &str, custom_names: &[String]) -> Option<String> {
    let cands = slash_candidates(prefix, custom_names);
    if cands.is_empty() {
        return None;
    }
    if cands.len() == 1 && cands[0] == prefix {
        return None;
    }
    let display = cands
        .iter()
        .map(|c| format!("/{}", c))
        .collect::<Vec<_>>()
        .join("  ");
    Some(display)
}

/// Compute the live command suggestion for the current input line. Returns
/// `None` unless the line starts with `/` and has no space (i.e. the user is
/// still typing the command name).
async fn current_suggestion(state: &Arc<ReplState>, line: &str) -> Option<String> {
    let after = line.strip_prefix('/')?;
    if after.contains(' ') {
        return None;
    }
    let customs: Vec<String> = state
        .commands
        .read()
        .await
        .iter()
        .map(|c| c.name.clone())
        .collect();
    command_suggestion(after, &customs)
}

/// Longest prefix shared by every candidate. Empty when the list is empty or
/// the candidates diverge at the first character.
fn longest_common_prefix(cands: &[String]) -> String {
    let Some(first) = cands.first() else {
        return String::new();
    };
    let mut end = first.len();
    for c in &cands[1..] {
        let mut common = 0;
        for ((i, a), (_, b)) in first.char_indices().zip(c.char_indices()) {
            if a != b {
                break;
            }
            common = i + a.len_utf8();
        }
        end = end.min(common);
    }
    first[..end].to_string()
}

/// Tab completion for the slash-command being typed. Returns the replacement
/// line, or `None` when there is nothing to complete.
///
/// - Exactly one candidate: complete it and append a space, so an argument can
///   be typed straight away (`/sen` → `/send `).
/// - Several candidates: extend as far as they agree (`/re` →
///   `/reload-` for `reload-persona` / `reload-commands`). The candidate list is
///   already on screen above the prompt, so nothing else is printed.
/// - Nothing to add, no match, not a slash command, or an argument already
///   started: `None`, and the keypress is a no-op.
fn command_completion(line: &str, custom_names: &[String]) -> Option<String> {
    let prefix = line.strip_prefix('/')?;
    if prefix.contains(' ') {
        return None;
    }
    let cands = slash_candidates(prefix, custom_names);
    match cands.len() {
        0 => None,
        1 => Some(format!("/{} ", cands[0])),
        _ => {
            let lcp = longest_common_prefix(&cands);
            (lcp.len() > prefix.len()).then(|| format!("/{lcp}"))
        }
    }
}

/// Truncate `s` to fit within `max_chars` display columns (command names are
/// ASCII, so char-based truncation is accurate), appending `…` when truncated.
fn truncate_suggestion(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    t.push('…');
    t
}

/// Render the prompt with a live command suggestion (when the line starts
/// with `/` and has no space). Used by the raw-mode input loop.
async fn render_prompt(
    renderer: &mut PromptRenderer,
    stdout: &mut std::io::Stdout,
    prompt: &str,
    state: &Arc<ReplState>,
    input_state: &InputState,
) {
    let sug = current_suggestion(state, &input_state.line).await;
    renderer.render(
        stdout,
        &state.theme,
        Role::PromptSymbol,
        prompt,
        input_state,
        sug.as_deref(),
    );
}

async fn handle_repl_command(
    rest: &str,
    input_tx: &mpsc::Sender<AgentInput>,
    state: &Arc<ReplState>,
    raw_mode: bool,
) -> CommandResult {
    let mut parts = rest.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("").trim();
    let arg = parts.next().unwrap_or("").trim();
    match cmd {
        "quit" | "exit" => return CommandResult::Quit,
        "help" => {
            raw_println(raw_mode, "Commands:");
            raw_println(raw_mode, "  /list                       List currently running peers (id / name / provider / model / role).");
            println!(
                "  /send <peer> <text>         Send a one-shot prompt to a peer (id or name)."
            );
            raw_println(raw_mode, "  /tools                      List tools enabled for this agent.");
            raw_println(raw_mode, "  /persona                    Show this agent's persona (role / skills / source path).");
            raw_println(raw_mode, "  /reload-persona             Re-resolve and reload the persona; system prompt is replaced, history kept.");
            raw_println(raw_mode, "  /spawn [name] [provider]    Create a detached agent-cli peer that outlives this session.");
            raw_println(raw_mode, "  /stop <peer>                Stop a running peer (id or name); detached agents too.");
            raw_println(raw_mode, "  /peer <id_or_name>          Show a peer's persona summary.");
            raw_println(raw_mode, "  /history [n]                Show last n (default 20) user inputs from this session.");
            raw_println(raw_mode, "  /clear, /reset              Clear conversation history (persona / system prompt are kept).");
            raw_println(raw_mode, "  /cancel                     Request cancel of the in-flight AI response or tool call.");
            raw_println(raw_mode, "  /auto [on|off|status]       Toggle tool-approval skip. No arg / 'status' shows current value.");
            raw_println(raw_mode, "  /commands                   List custom slash commands from .agent-cli/commands.");
            raw_println(raw_mode, "  /reload-commands            Re-scan the custom commands directory.");
            raw_println(raw_mode, "  /help                       Show this help.");
            raw_println(raw_mode, "  /quit, /exit                Terminate (full aliases). Ctrl+D, Ctrl+C, SIGTERM also exit cleanly.");
            raw_println(raw_mode, "");
            raw_println(raw_mode, "Tool approval can be skipped via:");
            raw_println(raw_mode, "  - REPL command  : /auto on  (toggleable at runtime)");
            raw_println(raw_mode, "  - CLI flag      : agent-cli run --auto-approve-tools");
            raw_println(raw_mode, "  - Config file   : [runtime] auto_approve_tools = true");
            // Custom commands section (FR-12).
            let cmds = state.commands.read().await;
            if !cmds.is_empty() {
                raw_println(raw_mode, "");
                raw_println(raw_mode, "Custom commands (.agent-cli/commands):");
                for cc in cmds.iter() {
                    let first_line = cc.content.lines().next().unwrap_or("");
                    raw_println(raw_mode, &format!("  /{}  {}", cc.name, first_line));
                }
            }
        }
        "auto" => handle_auto_command(arg, state, raw_mode),
        "clear" | "reset" => {
            // Clear conversation history (keep only system prompt).
            if input_tx.send(AgentInput::ClearHistory).await.is_err() {
                raw_eprintln(raw_mode, "[error] failed to send clear request");
            }
        }
        "history" => {
            let n: usize = arg.parse().unwrap_or(20);
            let h = state.persona.read().await;
            let _ = h; // unused
            let hist = state.history.read().await;
            let total = hist.len();
            let start = total.saturating_sub(n);
            for (i, line) in hist.iter().enumerate().skip(start) {
                // Past input is replayed dimmed, so it stays distinct from the
                // line the user is writing now.
                let entry = format!("{:>4}  {}", i + 1, line);
                raw_println(raw_mode, &state.theme.out(Role::HistoryEntry, &entry));
            }
            if total == 0 {
                raw_println(raw_mode, "(empty)");
            }
        }
        "list" => list_peers(&state.registry_dir, raw_mode),
        "send" => send_to_peer(arg, &state.registry_dir, raw_mode).await,
        "spawn" => spawn_peer(arg, state, raw_mode).await,
        "stop" => stop_peer_cmd(arg, &state.registry_dir, raw_mode).await,
        "tools" => {
            if state.tool_names.is_empty() {
                raw_println(raw_mode, "(no tools enabled)");
            } else {
                raw_println(raw_mode, &format!("tools: {}", state.tool_names.join(", ")));
            }
        }
        "persona" => {
            let p = state.persona.read().await;
            print_persona(&p, raw_mode);
        }
        "reload-persona" => reload_persona(state, input_tx, raw_mode).await,
        "peer" => peer_summary(arg, &state.registry_dir, raw_mode),
        "cancel" => {
            // Same signal as `Esc` during a turn: stops an in-flight turn (e.g.
            // one started by a peer prompt) rather than merely asking for it.
            state.cancel.cancel();
            let _ = input_tx.send(AgentInput::Cancel).await;
        }
        "commands" => {
            // FR-13: list custom commands with file path and first line.
            let cmds = state.commands.read().await;
            if cmds.is_empty() {
                raw_println(raw_mode, "(no custom commands)");
            } else {
                for cc in cmds.iter() {
                    let first_line = cc.content.lines().next().unwrap_or("");
                    raw_println(
                        raw_mode,
                        &format!("  /{}  {}  [{}]", cc.name, first_line, cc.path.display()),
                    );
                }
            }
        }
        "reload-commands" => {
            // FR-16: re-scan the commands directory without restarting.
            let dir = state.commands_dir.clone();
            let new_cmds = custom_commands::discover(&dir);
            let count = new_cmds.len();
            let mut cmds = state.commands.write().await;
            *cmds = new_cmds;
            raw_println(raw_mode, &format!("[reload-commands] {} command(s) loaded", count));
        }
        _ => {
            // FR-05/FR-06: built-in commands take precedence; otherwise look up
            // a custom command file and, if found, send its expanded content as
            // a user prompt (FR-08/FR-11).
            let cc = {
                let cmds = state.commands.read().await;
                cmds.iter().find(|c| c.name == cmd).cloned()
            };
            if let Some(cc) = cc {
                let prompt = custom_commands::expand_template(&cc.content, arg);
                if input_tx.send(AgentInput::UserPrompt(prompt)).await.is_err() {
                    raw_eprintln(raw_mode, "[error] failed to send custom command prompt");
                    return CommandResult::Continue;
                }
                // Record in history (FR-17).
                let full_cmd = if arg.is_empty() {
                    format!("/{}", cmd)
                } else {
                    format!("/{} {}", cmd, arg)
                };
                push_history(state, &full_cmd).await;
                return CommandResult::SubmittedPrompt;
            }
            // FR-18: prefix match on custom commands. If exactly one custom command
            // starts with `cmd`, auto-execute it. If multiple, list candidates.
            let prefix_matches: Vec<CustomCommand> = {
                let cmds = state.commands.read().await;
                cmds.iter()
                    .filter(|c| c.name.starts_with(cmd))
                    .cloned()
                    .collect()
            };
            match prefix_matches.len() {
                0 => {
                    raw_eprintln(raw_mode, &format!("unknown command: {cmd}"));
                }
                1 => {
                    let cc = &prefix_matches[0];
                    let resolved_name = cc.name.clone();
                    let prompt = custom_commands::expand_template(&cc.content, arg);
                    raw_println(
                        raw_mode,
                        &format!("[auto] /{} → /{}", cmd, resolved_name),
                    );
                    if input_tx.send(AgentInput::UserPrompt(prompt)).await.is_err() {
                        raw_eprintln(raw_mode, "[error] failed to send custom command prompt");
                        return CommandResult::Continue;
                    }
                    let full_cmd = if arg.is_empty() {
                        format!("/{}", resolved_name)
                    } else {
                        format!("/{} {}", resolved_name, arg)
                    };
                    push_history(state, &full_cmd).await;
                    return CommandResult::SubmittedPrompt;
                }
                _ => {
                    raw_println(
                        raw_mode,
                        &format!("ambiguous command: /{cmd} matches:"),
                    );
                    for cc in &prefix_matches {
                        let first_line = cc.content.lines().next().unwrap_or("");
                        raw_println(
                            raw_mode,
                            &format!("  /{}  {}", cc.name, first_line),
                        );
                    }
                }
            }
        }
    }
    // Record built-in commands in history (FR-17), except quit/exit.
    let full_cmd = if arg.is_empty() {
        format!("/{}", cmd)
    } else {
        format!("/{} {}", cmd, arg)
    };
    push_history(state, &full_cmd).await;
    CommandResult::Continue
}

/// `/spawn [name] [provider]` — launch a detached agent-cli peer sharing this
/// session's config (and therefore its `registry_dir`). The child outlives this
/// process (design §5, §6).
async fn spawn_peer(arg: &str, state: &Arc<ReplState>, raw_mode: bool) {
    let mut it = arg.split_whitespace();
    let name = it.next().map(|s| s.to_string());
    let provider = it.next().map(|s| s.to_string());
    let run_args = RunArgs {
        name,
        // Inherit the launcher's group so the spawned child joins the same cohort.
        group: state.group.as_ref().map(|g| g.to_string()),
        provider,
        model: None,
        persona: None,
        auto_approve_tools: false,
    };
    match crate::commands::spawn_detached(&state.config_source.path, &state.registry_dir, &run_args).await {
        Ok(entry) => raw_println(
            raw_mode,
            &format!(
                "[spawn] detached agent id={} name={} pid={}",
                entry.id,
                entry.name.as_deref().unwrap_or("-"),
                entry.pid
            ),
        ),
        Err(e) => raw_eprintln(raw_mode, &format!("[spawn] failed: {e}")),
    }
}

/// `/stop <peer>` — request a peer (id or name) to shut down (design §7).
async fn stop_peer_cmd(arg: &str, registry_dir: &Path, raw_mode: bool) {
    let peer = arg.trim();
    if peer.is_empty() {
        raw_eprintln(raw_mode, "usage: /stop <peer>");
        return;
    }
    if let Err(e) = crate::commands::stop_peer(registry_dir, peer).await {
        raw_eprintln(raw_mode, &format!("[stop] {e}"));
    }
}

fn list_peers(registry_dir: &Path, raw_mode: bool) {
    match crate::ipc::registry::list_entries(registry_dir) {
        Ok(entries) => {
            if entries.is_empty() {
                raw_println(raw_mode, "(no agents running)");
                return;
            }
            for e in entries {
                let role = e
                    .persona
                    .as_ref()
                    .map(|p| p.role.clone())
                    .unwrap_or_default();
                raw_println(raw_mode, &format!(
                    "{}\t{}\t{}\t{}\t{}",
                    e.id,
                    e.name.clone().unwrap_or_else(|| "-".into()),
                    e.provider,
                    e.model,
                    role
                ));
            }
        }
        Err(e) => raw_eprintln(raw_mode, &format!("[error] {e}")),
    }
}

async fn send_to_peer(arg: &str, registry_dir: &Path, raw_mode: bool) {
    let mut p = arg.splitn(2, ' ');
    let peer = p.next().unwrap_or("").trim();
    let text = p.next().unwrap_or("").trim();
    if peer.is_empty() || text.is_empty() {
        raw_eprintln(raw_mode, "usage: /send <peer> <text>");
        return;
    }
    match crate::ipc::registry::resolve_peer(registry_dir, peer) {
        Ok(p) => {
            let msg = crate::ipc::IpcMessage::Prompt {
                from: p.id.clone(),
                from_name: None,
                text: text.to_string(),
                reply_to: None,
            };
            if let Err(e) = crate::ipc::client::send(&p.socket, &msg).await {
                raw_eprintln(raw_mode, &format!("[error] {e}"));
            } else {
                raw_println(raw_mode, &format!("delivered to {}", p.id));
            }
        }
        Err(e) => raw_eprintln(raw_mode, &format!("[error] {e}")),
    }
}

fn print_persona(persona: &Persona, raw_mode: bool) {
    raw_println(raw_mode, &format!(
        "name        : {}",
        persona.frontmatter.name.as_deref().unwrap_or("-")
    ));
    raw_println(raw_mode, &format!("role        : {}", persona.frontmatter.role));
    if !persona.frontmatter.skills.is_empty() {
        raw_println(raw_mode, &format!("skills      : {}", persona.frontmatter.skills.join(", ")));
    }
    if let Some(d) = &persona.frontmatter.description {
        raw_println(raw_mode, &format!("description : {d}"));
    }
    if let Some(t) = &persona.frontmatter.temperature {
        raw_println(raw_mode, &format!("temperature : {t}"));
    }
    if let Some(allow) = &persona.frontmatter.allowed_tools {
        raw_println(raw_mode, &format!("allowed     : {}", allow.join(", ")));
    }
    if let Some(deny) = &persona.frontmatter.denied_tools {
        raw_println(raw_mode, &format!("denied      : {}", deny.join(", ")));
    }
    if let Some(p) = &persona.source_path {
        raw_println(raw_mode, &format!("source      : {}", p.display()));
    } else {
        raw_println(raw_mode, "source      : (builtin default)");
    }
}

async fn reload_persona(state: &Arc<ReplState>, input_tx: &mpsc::Sender<AgentInput>, raw_mode: bool) {
    let resolution = match persona::resolve(
        state.cli_persona_path.as_deref(),
        &state.persona_file_setting,
        &state.agents_dir,
        state.name.as_deref(),
    ) {
        Ok(r) => r,
        Err(e) => {
            raw_eprintln(raw_mode, &format!("[error] {e}"));
            return;
        }
    };
    let prompt = resolution.persona.to_system_prompt();
    {
        let mut guard = state.persona.write().await;
        *guard = resolution.persona;
    }
    if let Err(e) = input_tx.send(AgentInput::SetSystemPrompt(prompt)).await {
        raw_eprintln(raw_mode, &format!("[error] {e}"));
        return;
    }
    let p = state.persona.read().await;
    raw_println(raw_mode, &format!(
        "persona reloaded from {}",
        match &p.source_path {
            Some(path) => path.display().to_string(),
            None => "(builtin default)".to_string(),
        }
    ));
}

fn peer_summary(arg: &str, registry_dir: &Path, raw_mode: bool) {
    let key = arg.trim();
    if key.is_empty() {
        raw_eprintln(raw_mode, "usage: /peer <id_or_name>");
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
        Err(e) => raw_eprintln(raw_mode, &format!("[error] {e}")),
    }
}

#[cfg(test)]
mod tests {
    //! Regression tests for FR-13 "App termination" input loop (`/quit` and Ctrl+D=EOF),
    //! plus unit tests for InputState history navigation (FR-03).
    use super::*;
    use crate::persona::Persona;
    use crate::theme::{has_sgr, strip_sgr};
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    /// InputState unit tests (FR-03 / FR-04: history navigation and draft preservation).
    #[test]
    fn input_state_navigate_up_from_empty_history() {
        let mut s = InputState::new();
        s.line = "hello".to_string();
        s.navigate_up(&[]);
        assert_eq!(s.line, "hello");
        assert!(s.history_index.is_none());
        assert!(s.saved_draft.is_none());
    }

    #[test]
    fn input_state_navigate_up_saves_draft_and_moves_to_newest() {
        let history = vec!["cmd1".to_string(), "cmd2".to_string(), "cmd3".to_string()];
        let mut s = InputState::new();
        s.line = "current draft".to_string();
        s.navigate_up(&history);
        assert_eq!(s.line, "cmd3");
        assert_eq!(s.history_index, Some(2));
        assert_eq!(s.saved_draft, Some("current draft".to_string()));
    }

    #[test]
    fn input_state_navigate_up_then_down_restores_draft() {
        let history = vec!["cmd1".to_string(), "cmd2".to_string()];
        let mut s = InputState::new();
        s.line = "my draft".to_string();
        s.navigate_up(&history);
        assert_eq!(s.line, "cmd2");
        s.navigate_down(&history);
        assert_eq!(s.line, "my draft");
        assert!(s.history_index.is_none());
    }

    #[test]
    fn input_state_navigate_up_twice_then_enter_submits_correct_entry() {
        let history = vec!["first".to_string(), "second".to_string(), "third".to_string()];
        let mut s = InputState::new();
        s.line = String::new();
        s.navigate_up(&history);
        assert_eq!(s.line, "third");
        s.navigate_up(&history);
        assert_eq!(s.line, "second");
        s.navigate_up(&history);
        assert_eq!(s.line, "first");
        s.navigate_up(&history);
        assert_eq!(s.line, "first");
        assert_eq!(s.history_index, Some(0));
    }

    #[test]
    fn input_state_down_at_bottom_is_noop() {
        let history = vec!["cmd1".to_string()];
        let mut s = InputState::new();
        s.line = "hello".to_string();
        s.navigate_down(&history);
        assert_eq!(s.line, "hello");
        assert!(s.history_index.is_none());
    }

    #[test]
    fn input_state_escape_exits_history_and_restores_draft() {
        let history = vec!["old".to_string()];
        let mut s = InputState::new();
        s.line = "typing".to_string();
        s.navigate_up(&history);
        assert_eq!(s.line, "old");
        s.exit_history();
        assert_eq!(s.line, "typing");
        assert!(s.history_index.is_none());
    }

    #[test]
    fn input_state_insert_char_at_cursor() {
        let mut s = InputState::new();
        s.line = "ac".to_string();
        s.cursor = 1;
        s.insert_char('b');
        assert_eq!(s.line, "abc");
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn input_state_backspace_deletes_before_cursor() {
        let mut s = InputState::new();
        s.line = "abc".to_string();
        s.cursor = 2;
        s.backspace();
        assert_eq!(s.line, "ac");
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn input_state_delete_deletes_at_cursor() {
        let mut s = InputState::new();
        s.line = "abc".to_string();
        s.cursor = 1;
        s.delete();
        assert_eq!(s.line, "ac");
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn input_state_cursor_movement() {
        let mut s = InputState::new();
        s.line = "hello".to_string();
        s.cursor = 3;
        s.move_left();
        assert_eq!(s.cursor, 2);
        s.move_right();
        assert_eq!(s.cursor, 3);
        s.move_home();
        assert_eq!(s.cursor, 0);
        s.move_end();
        assert_eq!(s.cursor, 5);
    }

    // --- Existing integration tests (non-interactive, line-oriented mode) ---

    /// FR-03-1-2 / design doc 4.3C: `collapse_thinking_text` behavior.
    #[test]
    fn collapse_thinking_text_keeps_short_single_line_intact() {
        assert_eq!(collapse_thinking_text("hello"), "hello");
    }

    #[test]
    fn collapse_thinking_text_truncates_long_single_line() {
        let long: String = std::iter::repeat_n('a', 200).collect();
        let collapsed = collapse_thinking_text(&long);
        assert!(collapsed.ends_with("..."));
        assert_eq!(collapsed.chars().count(), 83);
    }

    #[test]
    fn collapse_thinking_text_truncates_to_first_line() {
        let multi = "step 1: analyze\nstep 2: act";
        let collapsed = collapse_thinking_text(multi);
        assert_eq!(collapsed, "step 1: analyze...");
    }

    #[test]
    fn collapse_thinking_text_handles_blank_input() {
        assert_eq!(collapse_thinking_text(""), "...");
        assert_eq!(collapse_thinking_text("\n\n"), "...");
    }

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
            cancel: Arc::new(CancelToken::default()),
            suppress: Arc::new(AtomicUsize::new(0)),
            indicator: Arc::new(std::sync::Mutex::new(StatusIndicator::disabled())),
            commands_dir: dir.to_path_buf(),
            commands: RwLock::new(Vec::new()),
            config_source: ConfigSource {
                path: dir.join("config.toml"),
                from_explicit: false,
            },
            group: None,
            theme: Theme::plain(),
        })
    }

    /// Test helper: returns an unused approval channel.
    fn dummy_approval_rx() -> mpsc::Receiver<ApprovalRequest> {
        let (_tx, rx) = mpsc::channel::<ApprovalRequest>(4);
        rx
    }

    #[tokio::test]
    async fn input_loop_terminates_on_eof() {
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_idle_tx, idle_rx) = mpsc::channel::<()>(8);
        let shutdown_observer = shutdown_rx.clone();

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
        assert!(
            *shutdown_observer.borrow(),
            "EOF should propagate as shutdown=true"
        );
    }

    #[tokio::test]
    async fn input_loop_terminates_on_quit_command() {
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_idle_tx, idle_rx) = mpsc::channel::<()>(8);
        let shutdown_observer = shutdown_rx.clone();

        let (mut writer, reader) = tokio::io::duplex(64);
        writer.write_all(b"/quit\n").await.unwrap();

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
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_idle_tx, idle_rx) = mpsc::channel::<()>(8);

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
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (idle_tx, idle_rx) = mpsc::channel::<()>(8);

        let (mut writer, reader) = tokio::io::duplex(1024);
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

        let msg1 = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("first prompt timeout")
            .expect("input_rx closed");
        match msg1 {
            AgentInput::UserPrompt(s) => assert_eq!(s, "first"),
            other => panic!("expected UserPrompt(\"first\"), got {:?}", other),
        }

        let blocked = tokio::time::timeout(Duration::from_millis(300), input_rx.recv()).await;
        assert!(
            blocked.is_err(),
            "second prompt should not arrive while input loop is Pending"
        );

        idle_tx.send(()).await.unwrap();

        let msg2 = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("second prompt timeout")
            .expect("input_rx closed");
        match msg2 {
            AgentInput::UserPrompt(s) => assert_eq!(s, "second"),
            other => panic!("expected UserPrompt(\"second\"), got {:?}", other),
        }

        drop(writer);
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    #[tokio::test]
    async fn stale_idle_signal_is_drained_before_pending() {
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (idle_tx, idle_rx) = mpsc::channel::<()>(8);

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

        let msg = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("prompt timeout")
            .expect("input_rx closed");
        match msg {
            AgentInput::UserPrompt(s) => assert_eq!(s, "only"),
            other => panic!("unexpected: {:?}", other),
        }

        writer.write_all(b"should-not-pass\n").await.unwrap();
        let blocked = tokio::time::timeout(Duration::from_millis(300), input_rx.recv()).await;
        assert!(
            blocked.is_err(),
            "stale idle signals should have been drained, leaving the loop Pending"
        );

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

    /// FR-13 / T-507: `/exit` triggers termination equivalent to `/quit`.
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

    /// FR-04-1 / T-506: Approval request arrives -> "y" input -> oneshot receives true and transitions to Pending;
    /// user input does not leak to agent during this time.
    #[tokio::test]
    async fn approval_y_resolves_true_and_blocks_user_prompt() {
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (idle_tx, idle_rx) = mpsc::channel::<()>(8);
        let (approval_tx, approval_rx) = mpsc::channel::<ApprovalRequest>(4);

        let (mut writer, reader) = tokio::io::duplex(1024);

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

        let (resp_tx, resp_rx) = oneshot::channel::<bool>();
        approval_tx
            .send(ApprovalRequest {
                tool_name: "bash".into(),
                args: serde_json::json!({"command": "echo hi"}),
                response: resp_tx,
            })
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        writer.write_all(b"some text\n").await.unwrap();
        let leaked = tokio::time::timeout(Duration::from_millis(200), input_rx.recv()).await;
        assert!(
            leaked.is_err(),
            "approval-mode input must not reach agent as UserPrompt"
        );
        let approved = tokio::time::timeout(Duration::from_secs(2), resp_rx)
            .await
            .expect("oneshot timeout")
            .expect("oneshot dropped");
        assert!(!approved, "non-y input should resolve to false");

        let (resp_tx2, resp_rx2) = oneshot::channel::<bool>();
        approval_tx
            .send(ApprovalRequest {
                tool_name: "bash".into(),
                args: serde_json::json!({"command": "echo hi"}),
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

    /// FR-04-1: When exiting AwaitingApproval via the shutdown route, oneshot receives false (denied).
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

        let (resp_tx, resp_rx) = oneshot::channel::<bool>();
        approval_tx
            .send(ApprovalRequest {
                tool_name: "bash".into(),
                args: serde_json::json!({}),
                response: resp_tx,
            })
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;

        shutdown_clone.send(true).unwrap();

        let resp = tokio::time::timeout(Duration::from_secs(2), resp_rx)
            .await
            .expect("oneshot timeout");
        match resp {
            Ok(b) => assert!(!b, "shutdown should deny pending approval"),
            Err(_) => panic!("oneshot was dropped without sending; agent would hang"),
        }

        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    // --- Wrap-aware cursor layout (FR-01/FR-02/FR-03) ---

    #[test]
    fn layout_cursor_ascii_no_wrap() {
        // prompt "> " (2) + "abc" (3); cursor after "abc" (3). width 80.
        // total=5, cursor_abs=5 -> row 0, col 5.
        let (end_row, row, col) = layout_cursor(2, 3, 5, 80);
        assert_eq!((end_row, row, col), (0, 0, 5));
    }

    #[test]
    fn layout_cursor_cjk_no_wrap() {
        // prompt "> " (2) + "あいう" (6); cursor after first char (display 2).
        // total=8, cursor_abs=2+2=4 -> row 0, col 4. Confirms width-based math.
        let (end_row, row, col) = layout_cursor(2, 2, 8, 80);
        assert_eq!((end_row, row, col), (0, 0, 4));
    }

    #[test]
    fn layout_cursor_just_before_boundary() {
        // width 10, total content 9 columns, cursor at end (9).
        // total%w != 0 -> end_row = 0; cursor row 0 col 9.
        let (end_row, row, col) = layout_cursor(2, 7, 9, 10);
        assert_eq!((end_row, row, col), (0, 0, 9));
    }

    #[test]
    fn layout_cursor_exactly_fills_width() {
        // width 10, total content exactly 10 columns, cursor at end (10).
        // Phantom last column: end_row = 0 (cursor stays on row 0).
        // cursor_abs=10 -> cursor_row = 1, col 0 (fresh wrapped row).
        let (end_row, row, col) = layout_cursor(2, 8, 10, 10);
        assert_eq!((end_row, row, col), (0, 1, 0));
    }

    #[test]
    fn layout_cursor_wraps_past_boundary() {
        // width 10, total 23 columns, cursor at end (23).
        // end_row = 23/10 = 2; cursor row 2, col 3.
        let (end_row, row, col) = layout_cursor(2, 21, 23, 10);
        assert_eq!((end_row, row, col), (2, 2, 3));
    }

    #[test]
    fn layout_cursor_midline_on_wrapped_row() {
        // width 10, total 23, cursor at absolute column 12 -> row 1, col 2.
        let (end_row, row, col) = layout_cursor(2, 10, 23, 10);
        assert_eq!((end_row, row, col), (2, 1, 2));
    }

    #[test]
    fn layout_cursor_zero_width_terminal_is_safe() {
        // width 0 is clamped to 1; must not divide by zero.
        let (_end_row, _row, _col) = layout_cursor(2, 0, 2, 0);
    }

    #[test]
    fn layout_cursor_empty_line() {
        // Empty prompt + empty line.
        let (end_row, row, col) = layout_cursor(0, 0, 0, 80);
        assert_eq!((end_row, row, col), (0, 0, 0));
    }

    // --- handle_key control-key behavior (FR-07/FR-08) ---

    fn key(code: KeyCode, ctrl: bool) -> KeyEvent {
        let mods = if ctrl {
            KeyModifiers::CONTROL
        } else {
            KeyModifiers::NONE
        };
        KeyEvent::new(code, mods)
    }

    #[test]
    fn ctrl_c_on_empty_line_signals_eof() {
        // Raw mode suppresses SIGINT, so Ctrl-C on an empty line must exit.
        let mut s = InputState::new();
        let action = handle_key(key(KeyCode::Char('c'), true), &mut s, &[]);
        assert!(matches!(action, Some(KeyAction::Eof)));
    }

    #[test]
    fn ctrl_c_with_text_clears_line() {
        // Ctrl-C with pending text clears the line instead of exiting.
        let mut s = InputState::new();
        s.insert_char('h');
        s.insert_char('i');
        let action = handle_key(key(KeyCode::Char('c'), true), &mut s, &[]);
        assert!(matches!(action, Some(KeyAction::ClearLine)));
        assert_eq!(s.line, "");
    }

    #[test]
    fn ctrl_d_on_empty_line_signals_eof() {
        let mut s = InputState::new();
        let action = handle_key(key(KeyCode::Char('d'), true), &mut s, &[]);
        assert!(matches!(action, Some(KeyAction::Eof)));
    }

    #[test]
    fn esc_when_not_browsing_clears_line() {
        let mut s = InputState::new();
        s.insert_char('x');
        let action = handle_key(key(KeyCode::Esc, false), &mut s, &[]);
        assert!(matches!(action, Some(KeyAction::ClearLine)));
        assert_eq!(s.line, "");
    }

    // --- First-section-header newline suppression (FR-15 / Defect #7) ---

    #[test]
    fn first_section_header_has_no_leading_newline() {
        // First header of a turn sits directly under the echoed input.
        assert_eq!(section_header_text("[answer]", false), "[answer]");
        assert_eq!(section_header_text("[thinking]", false), "[thinking]");
    }

    #[test]
    fn subsequent_section_headers_keep_leading_newline() {
        // Later headers separate from preceding streamed content.
        assert_eq!(section_header_text("[answer]", true), "\n[answer]");
    }

    #[test]
    fn display_state_section_flag_resets_each_turn() {
        let mut s = DisplayState::new(false);
        assert!(!s.section_printed);
        s.section_header("[answer]", Role::AnswerMarker);
        assert!(s.section_printed);
        s.reset();
        assert!(!s.section_printed);
    }

    #[test]
    fn esc_when_browsing_exits_history_and_restores_draft() {
        let history = vec!["old".to_string()];
        let mut s = InputState::new();
        s.insert_char('d');
        s.navigate_up(&history);
        assert_eq!(s.line, "old");
        let action = handle_key(key(KeyCode::Esc, false), &mut s, &history);
        assert!(matches!(action, Some(KeyAction::Continue)));
        assert_eq!(s.line, "d"); // draft restored
        assert!(s.history_index.is_none());
    }

    // --- Pending-state cancel key classification ---

    #[test]
    fn is_cancel_key_esc_returns_true() {
        assert!(is_cancel_key(&key(KeyCode::Esc, false)));
    }

    #[test]
    fn is_cancel_key_ctrl_c_returns_true() {
        assert!(is_cancel_key(&key(KeyCode::Char('c'), true)));
    }

    #[test]
    fn is_cancel_key_other_keys_return_false() {
        assert!(!is_cancel_key(&key(KeyCode::Char('a'), false)));
        assert!(!is_cancel_key(&key(KeyCode::Char('c'), false)));
        assert!(!is_cancel_key(&key(KeyCode::Enter, false)));
        assert!(!is_cancel_key(&key(KeyCode::Backspace, false)));
        assert!(!is_cancel_key(&key(KeyCode::Char('d'), true)));
    }

    // --- Forced return to the prompt (Esc during a turn) ---

    #[test]
    fn suppression_decision_passes_events_through_when_nothing_is_cancelled() {
        assert_eq!(suppression_decision(false, 0), (false, false, 0));
        // A terminal event releases the input loop from Pending as before.
        assert_eq!(suppression_decision(true, 0), (false, true, 0));
    }

    #[test]
    fn suppression_decision_drops_the_cancelled_turns_tail() {
        // Non-terminal events of the cancelled turn are dropped, and the count stays.
        assert_eq!(suppression_decision(false, 1), (true, false, 1));
        // Its terminal event is dropped too, consumes the cancellation, and — crucially —
        // sends no idle, so it cannot release a later turn from Pending.
        assert_eq!(suppression_decision(true, 1), (true, false, 0));
        // After that the next turn displays normally again.
        assert_eq!(suppression_decision(false, 0), (false, false, 0));
    }

    #[test]
    fn suppression_decision_counts_repeated_cancels() {
        assert_eq!(suppression_decision(true, 2), (true, false, 1));
        assert_eq!(suppression_decision(true, 1), (true, false, 0));
        assert_eq!(suppression_decision(true, 0), (false, true, 0));
    }

    // --- Turn progress indicator (activity line + spinner/elapsed) ---

    /// Capture sink standing in for the terminal, so the indicator's output can
    /// be asserted without a TTY.
    #[derive(Clone, Default)]
    struct CaptureSink(Arc<std::sync::Mutex<Vec<u8>>>);

    impl CaptureSink {
        fn taken(&self) -> String {
            let mut buf = self.0.lock().unwrap();
            let out = String::from_utf8_lossy(&buf).to_string();
            buf.clear();
            out
        }
    }

    impl std::io::Write for CaptureSink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// An indicator writing into `sink`, enabled, with a fixed 40-column width.
    fn test_indicator(sink: &CaptureSink) -> StatusIndicator {
        let mut ind = StatusIndicator::new(Box::new(sink.clone()), true, false);
        ind.width_override = Some(40);
        ind
    }

    #[test]
    fn format_elapsed_switches_from_seconds_to_minutes() {
        assert_eq!(format_elapsed(Duration::from_millis(0)), "0.0s");
        assert_eq!(format_elapsed(Duration::from_millis(430)), "0.4s");
        assert_eq!(format_elapsed(Duration::from_millis(12_440)), "12.4s");
        // Truncated, never rounded up into a bogus "60.0s".
        assert_eq!(format_elapsed(Duration::from_millis(59_990)), "59.9s");
        assert_eq!(format_elapsed(Duration::from_secs(60)), "1m00s");
        assert_eq!(format_elapsed(Duration::from_secs(123)), "2m03s");
    }

    #[test]
    fn spinner_frame_cycles_through_the_frames() {
        assert_eq!(spinner_frame(0), SPINNER_FRAMES[0]);
        assert_eq!(spinner_frame(3), SPINNER_FRAMES[3]);
        // Wraps around rather than panicking on a long turn.
        assert_eq!(spinner_frame(SPINNER_FRAMES.len()), SPINNER_FRAMES[0]);
        assert_eq!(
            spinner_frame(usize::MAX),
            SPINNER_FRAMES[usize::MAX % SPINNER_FRAMES.len()]
        );
    }

    #[test]
    fn one_line_tool_call_matches_the_permanent_line_and_stays_on_one_line() {
        let args = serde_json::json!({ "command": "echo hi" });
        let line = one_line_tool_call("bash", &args);
        assert_eq!(line, r#"[tool-call] bash {"command":"echo hi"}"#);
        assert!(!line.contains('\n'));
    }

    #[test]
    fn event_ends_at_line_start_tracks_the_cursor_per_event() {
        let mode = ShowThinkingMode::Collapsed;
        // Whole-line events always leave the cursor at column 0.
        for ev in [
            AgentEvent::Done,
            AgentEvent::Error { message: "e".into() },
            AgentEvent::Info { message: "i".into() },
            AgentEvent::ToolResult { name: "bash".into(), ok: true, output: "o".into() },
            AgentEvent::ToolCall { name: "bash".into(), args: serde_json::json!({}) },
        ] {
            assert!(event_ends_at_line_start(&ev, mode, false, false), "{ev:?}");
        }
        // Streamed text only when the delta ends with a newline.
        let mid = AgentEvent::Text { delta: "partial".into() };
        assert!(!event_ends_at_line_start(&mid, mode, false, true));
        let ended = AgentEvent::Text { delta: "line\n".into() };
        assert!(event_ends_at_line_start(&ended, mode, false, false));
        // An empty delta prints nothing, so the previous state stands.
        let empty = AgentEvent::Text { delta: String::new() };
        assert!(event_ends_at_line_start(&empty, mode, false, true));
        assert!(!event_ends_at_line_start(&empty, mode, false, false));
    }

    #[test]
    fn event_ends_at_line_start_respects_the_thinking_mode() {
        let ev = AgentEvent::Thinking { text: "reasoning".into() };
        // Shown: printed without a trailing newline, so the cursor is mid-row.
        assert!(!event_ends_at_line_start(&ev, ShowThinkingMode::Collapsed, false, true));
        assert!(!event_ends_at_line_start(&ev, ShowThinkingMode::Expanded, false, true));
        // Hidden: nothing is printed, so the previous state stands.
        assert!(event_ends_at_line_start(&ev, ShowThinkingMode::Hidden, false, true));
        assert!(!event_ends_at_line_start(&ev, ShowThinkingMode::Hidden, false, false));
        // The turn boundary prints nothing either.
        let start = AgentEvent::TurnStart;
        assert!(event_ends_at_line_start(&start, ShowThinkingMode::Collapsed, false, true));
        assert!(!event_ends_at_line_start(&start, ShowThinkingMode::Collapsed, false, false));
    }

    #[test]
    fn indicator_paints_the_spinner_row_headed_by_the_frame() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        ind.paint(true);
        let out = sink.taken();
        // One row, no newline: it sits directly under the line describing what
        // is running and is redrawn in place.
        assert!(!out.contains('\n'), "the row must not break the line: {out:?}");
        assert!(
            out.starts_with(SPINNER_FRAMES[0]),
            "spinner must head the row: {out:?}"
        );
        assert!(out.ends_with('s'), "elapsed time expected: {out:?}");
        assert!(crate::editor::str_display_width(&out) <= 39);
    }

    #[test]
    fn indicator_does_not_paint_while_the_cursor_is_mid_row() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        ind.paint(false);
        assert_eq!(sink.taken(), "", "nothing may be drawn mid-row");
        // A tick cannot introduce a block either.
        ind.tick();
        assert_eq!(sink.taken(), "");
    }

    #[test]
    fn indicator_tick_repaints_in_place() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        ind.paint(true);
        let _ = sink.taken();
        ind.tick();
        let out = sink.taken();
        // Move back to the block origin, clear it, then draw the next frame.
        assert!(out.contains("\u{1b}["), "an erase sequence is expected: {out:?}");
        assert!(
            out.contains(SPINNER_FRAMES[1]),
            "the spinner must have advanced: {out:?}"
        );
    }

    #[test]
    fn indicator_clear_removes_the_block_once() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        ind.paint(true);
        let _ = sink.taken();
        ind.clear();
        assert!(!sink.taken().is_empty(), "the block must be erased");
        // Already erased: a second clear writes nothing.
        ind.clear();
        assert_eq!(sink.taken(), "");
    }

    #[test]
    fn indicator_finish_leaves_exactly_one_completion_line() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        ind.paint(true);
        let _ = sink.taken();
        ind.finish(Mark::Ok);
        let out = sink.taken();
        let tail = out.rsplit('\u{1b}').next().unwrap_or(&out);
        assert!(tail.contains('✔'), "completion mark expected: {out:?}");
        assert_eq!(out.matches('✔').count(), 1);
        assert!(out.ends_with('\n'), "the line must be terminated: {out:?}");
        // The turn is over: a second terminal event adds nothing.
        ind.finish(Mark::Ok);
        assert_eq!(sink.taken(), "");
    }

    #[test]
    fn indicator_finish_marks_a_failed_turn() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        ind.paint(true);
        let _ = sink.taken();
        ind.finish(Mark::Err);
        let out = sink.taken();
        assert!(out.contains('✗'), "failure mark expected: {out:?}");
        assert!(!out.contains('✔'));
    }

    // --- Colour scheme (theme applied to the display) ---

    /// A theme that colours both streams, for asserting the styled output.
    fn colour_theme() -> Theme {
        Theme {
            stdout: true,
            stderr: true,
        }
    }

    #[test]
    fn tool_call_line_is_styled_as_name_plus_arguments() {
        let theme = colour_theme();
        let line = r#"[tool-call] bash {"command":"echo hi"}"#;
        let styled = style_tool_call_line(&theme, line);
        // The name carries the activity colour, the arguments the dim grey, and
        // the two are separate sequences with the space outside both.
        assert_eq!(
            styled,
            format!(
                "{} {}",
                crate::theme::paint(Role::ToolName, "[tool-call] bash"),
                crate::theme::paint(Role::ToolArgs, r#"{"command":"echo hi"}"#),
            )
        );
        // Nothing but the styling changed.
        assert_eq!(strip_sgr(&styled), line);
    }

    #[test]
    fn a_tool_call_line_cut_before_its_arguments_is_styled_whole() {
        let theme = colour_theme();
        // A narrow terminal can cut the line before the second space.
        let cut = crate::editor::truncate_display("[tool-call] ba", 14);
        let styled = style_tool_call_line(&theme, &cut);
        assert_eq!(styled, crate::theme::paint(Role::ToolName, &cut));
        assert_eq!(strip_sgr(&styled), cut);
    }

    #[test]
    fn a_plain_theme_leaves_the_tool_call_line_untouched() {
        let line = r#"[tool-call] bash {"command":"echo hi"}"#;
        assert_eq!(style_tool_call_line(&Theme::plain(), line), line);
    }

    #[test]
    fn multi_row_messages_are_styled_row_by_row() {
        let theme = colour_theme();
        // An elided tool result spans several rows; a colour must not span the
        // line breaks, since the rows below may be erased and redrawn.
        let text = "[tool-result ok] bash: one\ntwo\n… +3 more lines";
        let styled = style_rows(&theme, Role::ToolOutput, text);
        assert_eq!(styled.lines().count(), 3);
        for row in styled.lines() {
            assert!(
                row.starts_with("\u{1b}[") && row.ends_with("\u{1b}[0m"),
                "every row must open and close its own sequence: {row:?}"
            );
        }
        assert_eq!(strip_sgr(&styled), text);
        // Disabled: not a byte changes.
        assert_eq!(style_rows(&Theme::plain(), Role::ToolOutput, text), text);
    }

    #[test]
    fn indicator_styles_its_rows_without_changing_their_number_or_width() {
        let plain_sink = CaptureSink::default();
        let mut plain = indicator_with_thinking(&plain_sink, 30);
        plain.paint(true);
        let plain_out = plain_sink.taken();

        let sink = CaptureSink::default();
        let mut ind = indicator_with_thinking(&sink, 30);
        ind.theme = colour_theme();
        ind.paint(true);
        let out = sink.taken();

        assert_ne!(out, plain_out, "the coloured block must carry sequences");
        // The escape sequences of the *block* are the only difference: the rows
        // themselves, and therefore the erase maths, are unchanged.
        assert_eq!(strip_sgr(&out), strip_sgr(&plain_out));
        assert_eq!(ind.painted_rows(), plain.painted_rows());
        for row in strip_sgr(&out).split('\n') {
            assert!(crate::editor::str_display_width(row) <= 39);
        }
    }

    #[test]
    fn indicator_marks_a_finished_turn_in_the_outcome_colour() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.theme = colour_theme();
        ind.start();
        ind.paint(true);
        let _ = sink.taken();
        ind.finish(Mark::Ok);
        let ok = sink.taken();
        assert!(ok.contains("\u{1b}[1;32m"), "green bold expected: {ok:?}");
        assert!(ok.trim_end().ends_with("\u{1b}[0m"), "must close: {ok:?}");
        assert!(strip_sgr(&ok).contains('✔'));

        let mut ind = test_indicator(&sink);
        ind.theme = colour_theme();
        ind.start();
        ind.paint(true);
        let _ = sink.taken();
        ind.finish(Mark::Err);
        let err = sink.taken();
        assert!(err.contains("\u{1b}[1;31m"), "red bold expected: {err:?}");
        assert!(strip_sgr(&err).contains('✗'));
    }

    #[test]
    fn a_plain_indicator_writes_no_colour_of_its_own() {
        let sink = CaptureSink::default();
        let mut ind = indicator_with_thinking(&sink, 30);
        ind.paint(true);
        ind.finish(Mark::Ok);
        let out = sink.taken();
        // Only cursor movement and erasing: with colour off the block is drawn
        // exactly as it was before the scheme existed.
        assert!(!has_sgr(&out), "a plain indicator must not colour: {out:?}");
    }

    #[test]
    fn indicator_abandons_a_cancelled_turn_without_a_completion_line() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        ind.paint(true);
        let _ = sink.taken();
        ind.abandon_run();
        let out = sink.taken();
        assert!(!out.contains('✔') && !out.contains('✗'), "no mark: {out:?}");
        // The run is gone, so nothing more is drawn.
        ind.paint(true);
        ind.tick();
        assert_eq!(sink.taken(), "");
    }

    #[test]
    fn indicator_paints_nothing_while_suspended_and_resumes_afterwards() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        ind.paint(true);
        let _ = sink.taken();
        // The input loop takes the bottom rows (cancel notice, approval prompt).
        ind.suspend();
        assert!(!sink.taken().is_empty(), "suspend must erase the block");
        ind.paint(true);
        ind.tick();
        assert_eq!(sink.taken(), "", "nothing may be drawn while suspended");
        ind.resume();
        ind.paint(true);
        assert!(!sink.taken().is_empty(), "painting resumes after the prompt");
    }

    #[test]
    fn indicator_start_lifts_a_suspension_from_the_previous_turn() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        ind.paint(true);
        ind.suspend();
        let _ = sink.taken();
        // A cancel suspended the indicator; the next turn must draw again.
        ind.start();
        ind.paint(true);
        assert!(sink.taken().contains(SPINNER_FRAMES[0]));
    }

    #[test]
    fn disabled_indicator_writes_nothing_at_all() {
        let sink = CaptureSink::default();
        let mut ind = StatusIndicator::new(Box::new(sink.clone()), false, false);
        ind.width_override = Some(40);
        ind.start();
        ind.paint(true);
        ind.tick();
        ind.paint(true);
        ind.finish(Mark::Ok);
        ind.clear();
        assert_eq!(sink.taken(), "", "a disabled indicator must be a no-op");
    }

    #[test]
    fn tool_call_line_is_cut_to_one_row_while_the_indicator_is_drawing() {
        let long = serde_json::json!({ "command": "cargo test ".repeat(40) });
        // Without the indicator the line keeps its full, historical form.
        let mut plain = DisplayState::new(false);
        assert!(!plain.compact_output);
        // With it, the activity line is cut to a single terminal row so the
        // spinner stays directly beneath it.
        plain.compact_output = true;
        let cut = crate::editor::truncate_display(
            &one_line_tool_call("bash", &long),
            PromptRenderer::terminal_width().saturating_sub(1),
        );
        assert!(cut.ends_with('…'));
        assert!(!cut.contains('\n'));
        assert!(
            crate::editor::str_display_width(&cut) < PromptRenderer::terminal_width()
        );
    }

    // --- Live `thinking` view under the spinner ---

    /// An indicator with a fixed 40x24 terminal and reasoning already streamed.
    fn indicator_with_thinking(sink: &CaptureSink, lines: usize) -> StatusIndicator {
        let mut ind = test_indicator(sink);
        ind.height_override = Some(24);
        ind.start();
        let text: String = (0..lines)
            .map(|i| format!("reasoning line {i}\n"))
            .collect();
        ind.push_thinking(&text);
        ind
    }

    #[test]
    fn thinking_view_shows_the_last_rows_and_counts_the_rest() {
        let sink = CaptureSink::default();
        let ind = indicator_with_thinking(&sink, 30);
        let rows = ind.thinking_rows();
        // One marker row plus the capped number of reasoning rows.
        assert_eq!(rows.len(), THINKING_ROWS + 1);
        assert!(rows[0].contains("+20 more"), "marker row: {:?}", rows[0]);
        assert!(rows[0].contains("click to expand"));
        // The tail is what is shown: the newest reasoning, not the oldest.
        assert!(rows[1].contains("reasoning line 20"), "{:?}", rows[1]);
        assert!(rows[THINKING_ROWS].contains("reasoning line 29"));
    }

    #[test]
    fn thinking_view_without_overflow_has_no_marker() {
        let sink = CaptureSink::default();
        let ind = indicator_with_thinking(&sink, 3);
        let rows = ind.thinking_rows();
        assert_eq!(rows.len(), 3);
        assert!(rows[0].contains("reasoning line 0"));
        assert!(!rows[0].contains("click"));
    }

    #[test]
    fn thinking_view_is_empty_before_any_reasoning_arrives() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        assert!(ind.thinking_rows().is_empty());
        ind.push_thinking("   \n  ");
        assert!(ind.thinking_rows().is_empty(), "blank reasoning stays hidden");
    }

    #[test]
    fn expanding_shows_more_rows_but_never_more_than_the_screen() {
        let sink = CaptureSink::default();
        let mut ind = indicator_with_thinking(&sink, 30);
        assert_eq!(ind.thinking_rows().len(), THINKING_ROWS + 1);
        ind.toggle_expanded();
        let rows = ind.thinking_rows();
        // 24-row terminal: the activity line, the spinner, the marker and two
        // rows of context stay visible, so 19 reasoning rows are shown.
        assert_eq!(rows.len(), 19 + 1);
        assert!(rows[0].contains("+11 more"), "{:?}", rows[0]);
        assert!(rows[0].contains("click to collapse"));
        // Toggling back restores the short view.
        ind.toggle_expanded();
        assert_eq!(ind.thinking_rows().len(), THINKING_ROWS + 1);
    }

    #[test]
    fn thinking_rows_stay_within_one_terminal_row_each() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.height_override = Some(24);
        ind.start();
        ind.push_thinking(&"x".repeat(500));
        for row in ind.thinking_rows() {
            assert!(
                crate::editor::str_display_width(&row) <= 39,
                "row wider than the terminal: {row:?}"
            );
        }
    }

    #[test]
    fn painting_the_thinking_view_tracks_the_block_height() {
        let sink = CaptureSink::default();
        let mut ind = indicator_with_thinking(&sink, 4);
        let _ = sink.taken();
        ind.paint(true);
        let out = sink.taken();
        // Spinner row + four reasoning rows, and the erase must know it.
        assert_eq!(out.matches('\n').count(), 4, "block rows: {out:?}");
        assert_eq!(ind.painted_rows(), 5);
        ind.clear();
        assert_eq!(ind.painted_rows(), 0);
        assert!(sink.taken().contains("\u{1b}["), "the block must be erased");
    }

    #[test]
    fn a_finished_turn_drops_the_thinking_view() {
        let sink = CaptureSink::default();
        let mut ind = indicator_with_thinking(&sink, 4);
        ind.paint(true);
        let _ = sink.taken();
        ind.finish(Mark::Ok);
        let out = sink.taken();
        assert!(!out.contains("reasoning line"), "the live view is not kept: {out:?}");
        assert!(out.contains('✔'));
        assert!(ind.thinking_rows().is_empty());
    }

    #[test]
    fn mouse_reporting_stays_off_when_reasoning_is_not_shown() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.raw_mode = true;
        ind.thinking_view = false;
        ind.start();
        ind.push_thinking("reasoning");
        // Nothing to click, so the terminal keeps its own mouse handling and
        // the reasoning is not collected either.
        assert!(sink.taken().is_empty(), "no mouse-capture escape expected");
        assert!(ind.thinking_rows().is_empty());
    }

    #[test]
    fn mouse_reporting_is_enabled_for_the_turn_and_released_at_its_end() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.raw_mode = true;
        ind.start();
        assert!(sink.taken().contains("1000h"), "capture on for the turn");
        ind.paint(true);
        let _ = sink.taken();
        ind.finish(Mark::Ok);
        assert!(sink.taken().contains("1000l"), "capture released at the end");
    }

    #[test]
    fn thinking_is_ignored_outside_a_turn_and_by_a_disabled_indicator() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        // No turn in flight: nothing to attach the reasoning to.
        ind.push_thinking("stray");
        assert!(ind.thinking_rows().is_empty());
        let mut off = StatusIndicator::disabled();
        off.start();
        off.push_thinking("reasoning");
        off.toggle_expanded();
        assert!(off.thinking_rows().is_empty());
    }

    #[test]
    fn click_hits_indicator_covers_exactly_the_painted_block() {
        // A five-row block whose last row is screen row 20 covers rows 16..=20.
        assert!(click_hits_indicator(5, 20, 16));
        assert!(click_hits_indicator(5, 20, 20));
        assert!(!click_hits_indicator(5, 20, 15));
        assert!(!click_hits_indicator(5, 20, 21));
        // A single-row block is just the cursor row.
        assert!(click_hits_indicator(1, 7, 7));
        assert!(!click_hits_indicator(1, 7, 6));
        // Nothing painted: no click can hit it.
        assert!(!click_hits_indicator(0, 7, 7));
        // Near the top of the screen the block cannot start above row 0.
        assert!(click_hits_indicator(5, 2, 0));
    }

    #[test]
    fn captured_thinking_leaves_the_cursor_where_it_was() {
        let ev = AgentEvent::Thinking { text: "reasoning".into() };
        // Captured by the indicator: nothing is printed, so the previous state stands.
        assert!(event_ends_at_line_start(&ev, ShowThinkingMode::Collapsed, true, true));
        assert!(!event_ends_at_line_start(&ev, ShowThinkingMode::Collapsed, true, false));
        // Not captured: printed inline, so the cursor sits mid-row.
        assert!(!event_ends_at_line_start(&ev, ShowThinkingMode::Collapsed, false, true));
    }

    #[test]
    fn display_event_skips_thinking_that_the_indicator_shows() {
        let mut state = DisplayState::new(false);
        state.capture_thinking = true;
        // Nothing is printed and no `[thinking]` section is opened.
        display_event(
            AgentEvent::Thinking { text: "reasoning".into() },
            ShowThinkingMode::Collapsed,
            &mut state,
        );
        assert!(!state.thinking_printed, "no inline thinking section");
        assert!(!state.section_printed);
    }

    // --- Elided tool results ---

    #[test]
    fn a_short_tool_result_is_printed_verbatim() {
        let out = compact_tool_result("ok", "bash", "{\"exit_code\":0}", 80, 5);
        assert_eq!(out, "[tool-result ok] bash: {\"exit_code\":0}");
        assert!(!out.contains('…'));
    }

    #[test]
    fn a_long_tool_result_is_cut_to_the_row_budget_with_a_count() {
        let output: String = (0..30).map(|i| format!("line {i}\n")).collect();
        let out = compact_tool_result("ok", "bash", &output, 80, 5);
        let rows: Vec<&str> = out.split('\n').collect();
        // Five rows of output plus the marker.
        assert_eq!(rows.len(), 6, "{out:?}");
        assert!(rows[0].starts_with("[tool-result ok] bash: line 0"));
        // The head is shown: a result is read from the top.
        assert!(rows[4].contains("line 4"));
        assert_eq!(rows[5], "… +26 more lines");
    }

    #[test]
    fn a_wide_tool_result_counts_wrapped_rows_too() {
        // One very long line still occupies many rows on a narrow terminal.
        let output = "x".repeat(500);
        let out = compact_tool_result("ok", "bash", &output, 40, 5);
        let rows: Vec<&str> = out.split('\n').collect();
        assert_eq!(rows.len(), 6);
        for row in &rows[..5] {
            assert!(
                crate::editor::str_display_width(row) <= 40,
                "row wider than the terminal: {row:?}"
            );
        }
        assert!(rows[5].starts_with("… +"), "{:?}", rows[5]);
    }

    #[test]
    fn a_failed_tool_result_keeps_its_marker() {
        let out = compact_tool_result("ERR", "bash", &"e\n".repeat(20), 80, 5);
        assert!(out.starts_with("[tool-result ERR] bash:"));
        assert!(out.contains("… +"));
    }

    #[test]
    fn display_event_only_elides_tool_results_while_the_indicator_draws() {
        let long: String = (0..30).map(|i| format!("line {i}\n")).collect();
        // The historical form is kept when nothing is drawing beneath it.
        let verbatim = format!("[tool-result ok] bash: {long}");
        assert_eq!(
            compact_tool_result("ok", "bash", &long, 80, usize::MAX),
            verbatim
        );
        let mut state = DisplayState::new(false);
        assert!(!state.compact_output, "off unless the indicator is enabled");
        state.compact_output = true;
    }

    #[test]
    fn begin_cancel_raises_the_token_and_counts_the_outstanding_turn() {
        let cancel = CancelToken::default();
        let suppress = AtomicUsize::new(0);
        assert_eq!(begin_cancel(&cancel, &suppress), 1);
        assert!(cancel.is_cancelled());
        assert_eq!(begin_cancel(&cancel, &suppress), 2);
        assert_eq!(suppress.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn handle_key_eof_on_empty_line_is_state_independent() {
        // handle_key is stateless — it does not know about PromptState.
        // Ctrl-C on empty line always returns Eof regardless of caller state.
        let mut s = InputState::new();
        let action = handle_key(key(KeyCode::Char('c'), true), &mut s, &[]);
        assert!(matches!(action, Some(KeyAction::Eof)));
        // Verify it does not depend on history content
        let mut s2 = InputState::new();
        let action2 = handle_key(key(KeyCode::Char('c'), true), &mut s2, &["cmd".to_string()]);
        assert!(matches!(action2, Some(KeyAction::Eof)));
    }

    /// Build a ReplState whose `commands` are discovered from `dir`.
    async fn build_state_with_commands(dir: &Path) -> Arc<ReplState> {
        let state = build_state(dir);
        let cmds = custom_commands::discover(dir);
        let mut guard = state.commands.write().await;
        *guard = cmds;
        drop(guard);
        state
    }

    #[tokio::test]
    async fn custom_command_sends_prompt() {
        let tmp = TempDir::new().unwrap();
        // Create a custom command file.
        std::fs::write(
            tmp.path().join("hello.md"),
            "Hello from custom command: $ARGUMENTS",
        )
        .unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);

        let result = handle_repl_command("hello world", &input_tx, &state, false).await;
        assert!(matches!(result, CommandResult::SubmittedPrompt), "got {result:?}");

        let msg = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("timeout")
            .expect("input_rx closed");
        match msg {
            AgentInput::UserPrompt(s) => {
                assert_eq!(s, "Hello from custom command: world");
            }
            other => panic!("expected UserPrompt, got {other:?}"),
        }
        // No unexpected second message.
        let extra = tokio::time::timeout(Duration::from_millis(100), input_rx.recv()).await;
        assert!(extra.is_err(), "no extra input expected");
    }

    #[tokio::test]
    async fn builtin_takes_precedence_over_custom() {
        let tmp = TempDir::new().unwrap();
        // A custom command named `help` must NOT shadow the built-in /help.
        std::fs::write(tmp.path().join("help.md"), "SHOULD NOT BE SENT").unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);

        let result = handle_repl_command("help", &input_tx, &state, false).await;
        // Built-in help returns Continue (no prompt sent).
        assert!(matches!(result, CommandResult::Continue), "got {result:?}");
        // No UserPrompt should have been sent.
        let none = tokio::time::timeout(Duration::from_millis(100), input_rx.recv()).await;
        assert!(none.is_err(), "built-in /help must not send a UserPrompt");
    }

    #[tokio::test]
    async fn unknown_command_no_file_sends_nothing() {
        let tmp = TempDir::new().unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);

        let result = handle_repl_command("does-not-exist", &input_tx, &state, false).await;
        assert!(matches!(result, CommandResult::Continue), "got {result:?}");
        let none = tokio::time::timeout(Duration::from_millis(100), input_rx.recv()).await;
        assert!(none.is_err(), "unknown command must not send a UserPrompt");
    }

    #[tokio::test]
    async fn reload_commands_picks_up_new_file() {
        let tmp = TempDir::new().unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);

        // Initially empty.
        {
            let cmds = state.commands.read().await;
            assert!(cmds.is_empty());
        }
        // Add a file, then reload.
        std::fs::write(tmp.path().join("late.md"), "late arrival").unwrap();
        let result = handle_repl_command("reload-commands", &input_tx, &state, false).await;
        assert!(matches!(result, CommandResult::Continue), "got {result:?}");
        {
            let cmds = state.commands.read().await;
            assert_eq!(cmds.len(), 1);
            assert_eq!(cmds[0].name, "late");
        }
    }

    #[tokio::test]
    async fn commands_lists_custom_commands() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("alpha.md"), "first line alpha\nmore").unwrap();
        std::fs::write(tmp.path().join("beta.md"), "first line beta").unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);

        // /commands returns Continue (no prompt sent); we just verify it does
        // not panic and lists both commands by reading the shared state.
        let result = handle_repl_command("commands", &input_tx, &state, false).await;
        assert!(matches!(result, CommandResult::Continue), "got {result:?}");
        let cmds = state.commands.read().await;
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[tokio::test]
    async fn custom_command_at_file_expansion() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("data.txt");
        std::fs::write(&target, "FILE-DATA").unwrap();
        // The command file references @<target>.
        let template = format!("Read this: @{}", target.display());
        std::fs::write(tmp.path().join("show.md"), &template).unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);

        let result = handle_repl_command("show", &input_tx, &state, false).await;
        assert!(matches!(result, CommandResult::SubmittedPrompt), "got {result:?}");
        let msg = input_rx.recv().await.expect("input_rx closed");
        match msg {
            AgentInput::UserPrompt(s) => assert!(s.contains("FILE-DATA"), "{}", s),
            other => panic!("expected UserPrompt, got {other:?}"),
        }
    }

    #[test]
    fn slash_candidates_empty_prefix_lists_all_builtins_first() {
        let customs = vec!["hello".to_string(), "ai".to_string()];
        let cands = slash_candidates("", &customs);
        // Built-ins come first; "ai" custom appended after, sorted with other customs.
        assert!(cands.starts_with(&[
            "quit".to_string(),
            "exit".to_string(),
            "help".to_string(),
        ]));
        assert!(cands.contains(&"commands".to_string()));
        assert!(cands.contains(&"ai".to_string()));
        assert!(cands.contains(&"hello".to_string()));
        // Built-in section precedes all customs.
        let first_custom_idx = cands.iter().position(|c| c == "ai").unwrap();
        let last_builtin_idx = cands.iter().rposition(|c| *c == "reload-commands").unwrap();
        assert!(last_builtin_idx < first_custom_idx);
    }

    #[test]
    fn slash_candidates_prefix_filters() {
        let customs = vec!["hello".to_string(), "history-extra".to_string()];
        // "he" matches built-in "help" and custom "hello"; "history" starts with "hi".
        let cands_he = slash_candidates("he", &customs);
        assert_eq!(cands_he, vec!["help", "hello"]);
        // "hi" matches built-in "history" and custom "history-extra".
        let cands_hi = slash_candidates("hi", &customs);
        assert_eq!(cands_hi, vec!["history", "history-extra"]);
    }

    #[test]
    fn slash_candidates_custom_colliding_with_builtin_is_dropped() {
        // A custom named "help" collides with the built-in; built-in wins, custom dropped.
        let customs = vec!["help".to_string(), "hello".to_string()];
        let cands = slash_candidates("he", &customs);
        assert_eq!(cands, vec!["help", "hello"]);
    }

    #[test]
    fn command_suggestion_returns_none_when_no_match() {
        assert_eq!(command_suggestion("zzz", &["hello".to_string()]), None);
    }

    #[test]
    fn command_suggestion_returns_none_when_sole_exact_match() {
        // Fully typed, single match: nothing more to suggest.
        assert_eq!(command_suggestion("help", &[]), None);
    }

    #[test]
    fn command_suggestion_lists_matches_with_slash() {
        let customs = vec!["hello".to_string()];
        let sug = command_suggestion("he", &customs).unwrap();
        assert_eq!(sug, "/help  /hello");
    }

    #[test]
    fn truncate_suggestion_adds_ellipsis() {
        assert_eq!(truncate_suggestion("abcdef", 4), "abc…");
        assert_eq!(truncate_suggestion("abc", 10), "abc");
    }

    // --- Tab completion ---

    #[test]
    fn longest_common_prefix_of_candidates() {
        let cands = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            longest_common_prefix(&cands(&["reload-persona", "reload-commands"])),
            "reload-"
        );
        assert_eq!(longest_common_prefix(&cands(&["help", "hello"])), "hel");
        assert_eq!(longest_common_prefix(&cands(&["quit", "list"])), "");
        assert_eq!(longest_common_prefix(&cands(&["only"])), "only");
        assert_eq!(longest_common_prefix(&[]), "");
    }

    #[test]
    fn tab_completes_a_unique_match_and_adds_a_space() {
        // `/sen` matches only /send, so it completes and leaves room for the
        // argument.
        assert_eq!(command_completion("/sen", &[]).unwrap(), "/send ");
    }

    #[test]
    fn tab_extends_to_the_common_prefix_when_ambiguous() {
        // /reload-persona and /reload-commands agree up to "reload-".
        assert_eq!(command_completion("/rel", &[]).unwrap(), "/reload-");
        // "/re" also spans /reset, so there the shared prefix is just what was
        // typed and Tab has nothing to add.
        assert_eq!(command_completion("/re", &[]), None);
    }

    #[test]
    fn tab_completes_custom_commands_too() {
        let customs = vec!["review".to_string()];
        assert_eq!(command_completion("/rev", &customs).unwrap(), "/review ");
    }

    #[test]
    fn tab_is_a_noop_when_nothing_can_be_added() {
        // Ambiguous with no shared extension: /re* already spans reload-* and
        // nothing longer is common between them and /reset.
        assert_eq!(command_completion("/reload-", &[]), None);
        // No candidate at all.
        assert_eq!(command_completion("/zzz", &[]), None);
        // Not a slash command.
        assert_eq!(command_completion("hello", &[]), None);
        // Argument already started: the command name is settled.
        assert_eq!(command_completion("/send bob", &[]), None);
    }

    #[test]
    fn tab_on_a_fully_typed_unique_command_only_adds_the_space() {
        assert_eq!(command_completion("/quit", &[]).unwrap(), "/quit ");
        // Idempotent: a second Tab has an argument position, so it does nothing.
        assert_eq!(command_completion("/quit ", &[]), None);
    }

    // --- Candidate hint row bookkeeping (rendered above the prompt) ---

    #[test]
    fn suggestion_occupies_one_row_when_shown() {
        assert_eq!(suggestion_rows(Some("/help  /hello"), 80), 1);
    }

    #[test]
    fn suggestion_occupies_no_row_when_absent_or_terminal_too_narrow() {
        assert_eq!(suggestion_rows(None, 80), 0);
        assert_eq!(suggestion_rows(Some(""), 80), 0);
        assert_eq!(suggestion_rows(Some("/help"), 7), 0);
    }

    // --- FR-17: slash commands recorded in history ---

    #[tokio::test]
    async fn slash_command_recorded_in_history() {
        let tmp = TempDir::new().unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);

        // Execute a built-in command: /help
        let _ = handle_repl_command("help", &input_tx, &state, false).await;
        let hist = state.history.read().await;
        assert!(hist.contains(&"/help".to_string()), "history should contain /help, got {:?}", hist);
        drop(hist);

        // Execute another built-in: /tools
        let _ = handle_repl_command("tools", &input_tx, &state, false).await;
        let hist = state.history.read().await;
        assert!(hist.contains(&"/tools".to_string()), "history should contain /tools, got {:?}", hist);
    }

    #[tokio::test]
    async fn custom_command_recorded_in_history() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("hello.md"), "Hello $ARGUMENTS").unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);

        let _ = handle_repl_command("hello world", &input_tx, &state, false).await;
        let hist = state.history.read().await;
        assert!(hist.contains(&"/hello world".to_string()), "history should contain /hello world, got {:?}", hist);
    }

    #[tokio::test]
    async fn quit_not_recorded_in_history() {
        let tmp = TempDir::new().unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);

        let _ = handle_repl_command("quit", &input_tx, &state, false).await;
        let hist = state.history.read().await;
        assert!(!hist.contains(&"/quit".to_string()), "/quit should not be in history, got {:?}", hist);
    }

    // --- FR-18: auto-execute single-candidate prefix match ---

    #[tokio::test]
    async fn auto_execute_single_prefix_match() {
        let tmp = TempDir::new().unwrap();
        // Only one custom command starting with "he": "hello"
        std::fs::write(tmp.path().join("hello.md"), "Hello from hello").unwrap();
        // "hi" doesn't start with "he", so "he" uniquely matches "hello"
        std::fs::write(tmp.path().join("hi.md"), "Hello from hi").unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);

        // "he" should auto-execute "hello"
        let result = handle_repl_command("he", &input_tx, &state, false).await;
        assert!(matches!(result, CommandResult::SubmittedPrompt), "got {result:?}");

        let msg = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("timeout")
            .expect("input_rx closed");
        match msg {
            AgentInput::UserPrompt(s) => assert_eq!(s, "Hello from hello"),
            other => panic!("expected UserPrompt, got {other:?}"),
        }

        // Verify it was recorded in history with the resolved name
        let hist = state.history.read().await;
        assert!(hist.contains(&"/hello".to_string()), "auto-executed command should be in history as /hello, got {:?}", hist);
    }

    #[tokio::test]
    async fn ambiguous_prefix_lists_candidates() {
        let tmp = TempDir::new().unwrap();
        // Two commands starting with "he": "hello" and "helpx"
        std::fs::write(tmp.path().join("hello.md"), "Hello from hello").unwrap();
        std::fs::write(tmp.path().join("helpx.md"), "Hello from helpx").unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);

        // "he" matches both "hello" and "helpx" → ambiguous
        let result = handle_repl_command("he", &input_tx, &state, false).await;
        assert!(matches!(result, CommandResult::Continue), "ambiguous prefix should return Continue, got {result:?}");

        // No UserPrompt should be sent
        let none = tokio::time::timeout(Duration::from_millis(100), input_rx.recv()).await;
        assert!(none.is_err(), "ambiguous prefix should not send UserPrompt");
    }

    #[tokio::test]
    async fn no_prefix_match_unknown_command() {
        let tmp = TempDir::new().unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);

        let result = handle_repl_command("does-not-exist", &input_tx, &state, false).await;
        assert!(matches!(result, CommandResult::Continue), "got {result:?}");
    }

    #[tokio::test]
    async fn exact_match_takes_precedence_over_prefix() {
        let tmp = TempDir::new().unwrap();
        // Command named exactly "he" and another starting with "he" ("hello")
        std::fs::write(tmp.path().join("he.md"), "Exact match").unwrap();
        std::fs::write(tmp.path().join("hello.md"), "Hello match").unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);

        // "/he" should match exactly "he", not prefix-match "hello"
        let result = handle_repl_command("he", &input_tx, &state, false).await;
        assert!(matches!(result, CommandResult::SubmittedPrompt), "got {result:?}");

        let msg = tokio::time::timeout(Duration::from_secs(2), input_rx.recv())
            .await
            .expect("timeout")
            .expect("input_rx closed");
        match msg {
            AgentInput::UserPrompt(s) => assert_eq!(s, "Exact match"),
            other => panic!("expected UserPrompt with exact match content, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn builtin_takes_precedence_over_prefix_match() {
        let tmp = TempDir::new().unwrap();
        // A custom command "hello" that starts with "he" — but built-in "help" takes
        // precedence when typing the exact built-in name.
        std::fs::write(tmp.path().join("hello.md"), "SHOULD NOT BE SENT").unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);

        // "help" is a built-in command; built-in takes precedence
        let result = handle_repl_command("help", &input_tx, &state, false).await;
        assert!(matches!(result, CommandResult::Continue), "built-in /help should return Continue, got {result:?}");
        // No UserPrompt sent
        let none = tokio::time::timeout(Duration::from_millis(100), input_rx.recv()).await;
        assert!(none.is_err(), "built-in /help must not send UserPrompt");
    }
}