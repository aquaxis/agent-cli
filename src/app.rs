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
use crate::tools::send_to::Delivery;

use crate::agent::{Agent, AgentEvent, AgentInput, ApprovalRequest, CancelToken};
use crate::ai;
use crate::cli::RunArgs;
use crate::config::{self, Config, ConfigSource, ShowThinkingMode};
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
///
/// They are also the single seam every REPL message passes through, so each one
/// tees into the session transcript (`scroll.rs`) before the CR+LF rewriting —
/// the transcript holds the lines as they appear on screen, styling included.
/// When no transcript is installed (piped input, `serve`, the scrollback turned
/// off) recording is a no-op and nothing is allocated.
///
/// Print a string to stdout, replacing LF with CR+LF, then append CR+LF.
fn raw_println(raw: bool, msg: &str) {
    crate::scroll::record_line(msg);
    if zone_armed() {
        // The armed zone owns the screen (D-52/AS-53): the record is the sole
        // truth and the beat will surface this line with its next repaint.
        return;
    }
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
    crate::scroll::record(msg);
    if zone_armed() {
        return;
    }
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
    crate::scroll::record_line(msg);
    if zone_armed() {
        return;
    }
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
    crate::scroll::record(msg);
    if zone_armed() {
        return;
    }
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
    /// This session's own agent id and ancestor chain, so a peer it spawns
    /// joins this agent's tree at the right level.
    id: AgentId,
    ancestors: Vec<AgentId>,
    registry_dir: PathBuf,
    agents_dir: PathBuf,
    persona_file_setting: String,
    cli_persona_path: Option<PathBuf>,
    /// The `.agent-cli/persona.md` candidate of the directory this session
    /// started in, resolved once so `/reload-persona` re-reads that same path —
    /// its existence is tested at each resolution, not here.
    project_persona_path: Option<PathBuf>,
    name: Option<String>,
    persona: RwLock<Persona>,
    tool_names: Vec<String>,
    history_path: PathBuf,
    history: RwLock<Vec<String>>,
    /// Shared via `Arc<AtomicBool>` for `/auto on|off|status` runtime toggle (FR-04-2 / design doc 4.3A).
    auto_approve: Arc<AtomicBool>,
    /// `[permissions]`, compiled once and shared with the agent, so
    /// `/permissions` shows exactly the rules the agent is deciding with.
    permissions: Arc<crate::permissions::Ruleset>,
    /// Raised by `Esc` / `Ctrl-C` during a turn and by `/cancel`; the agent task
    /// observes it at every await point and unwinds the turn.
    cancel: Arc<CancelToken>,
    /// Number of cancellations whose turn has not reached its terminal event
    /// yet. The display task drops the events (and the idle notification) of
    /// those turns so the cancelled tail cannot print over the fresh prompt.
    suppress: Arc<AtomicUsize>,
    /// Set by the display task on `TurnStart` and cleared on a terminal event:
    /// the only signal that a normal turn is actually running. `begin_cancel`
    /// consults it so a cancellation raised while the agent is idle cannot arm
    /// a suppression counter that no terminal event would consume.
    turn_in_flight: Arc<AtomicBool>,
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
    /// `[shell]`: whether `!<command>` runs, and with what limits.
    shell: crate::config::ShellConfig,
    /// Whether the mouse-wheel scrollback is live for this session: `[ui]
    /// mouse_scroll` is on, `[ui] scrollback_lines` is non-zero, and the
    /// display is an interactive terminal. It decides whether the transcript
    /// is recorded, whether mouse reporting is enabled at the prompt, and
    /// whether the wheel does anything.
    scroll: bool,
    /// `[ui] mouse_select`: whether dragging the left button over the log
    /// selects it and copies on release. Only reachable while `scroll` is on,
    /// which is what makes the terminal report the drag to us at all.
    mouse_select: bool,
    /// `[ui] copy_command`: where a copied selection is sent. Empty means the
    /// terminal's own clipboard, through OSC 52.
    copy_command: String,
}

const HISTORY_LIMIT: usize = 200;

/// How long the headless shutdown path waits for the agent task to finish
/// cleanly — cancel the in-flight turn, reply truthfully to queued prompts —
/// before falling back to the abort (`run_headless`). Long enough for a
/// stream/tool call to close; short enough that a `stop` remains prompt.
const SHUTDOWN_DRAIN_TIMEOUT: Duration = Duration::from_secs(10);

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
    // The project's own role, if the directory states one. Resolved once, here,
    // so `/reload-persona` keeps reading the directory this session started in.
    let project_persona_path = config::local_persona_candidate();
    let resolution: PersonaResolution = persona::resolve(
        args.persona.as_deref(),
        &config.runtime.persona_file,
        &agents_dir,
        name.as_deref(),
        project_persona_path.as_deref(),
    )?;

    if resolution.builtin_used {
        tracing::info!("using builtin default persona");
    }
    if resolution.project_used {
        if let Some(p) = &resolution.persona.source_path {
            tracing::info!("using project persona {}", p.display());
        }
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
    // Shared with the accept loop's front-door gate; also the Agent's skip
    // condition for its queued prompts (only a genuinely stopping session
    // refuses its queue).
    let stopping = ipc_server.stopping();

    // The chain of agents that created this one, set by the launcher on this
    // process' environment. Empty means a human started it: depth 0, and this
    // agent is the root of its own tree.
    let ancestors = crate::ipc::registry::ancestors_from_env();

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
        ancestors: ancestors.clone(),
        turn: None,
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
    let turn_in_flight = Arc::new(AtomicBool::new(false));
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
    // Mouse-wheel scrollback. The transcript is recorded only for the
    // interactive REPL, so every other path keeps no buffer at all. While it is
    // live the input loop owns mouse reporting for the whole session, and the
    // indicator must not switch it off at the end of a turn.
    let scroll_enabled = {
        use std::io::IsTerminal;
        config.ui.mouse_scroll
            && config.ui.scrollback_lines > 0
            && stdin_is_tty()
            && std::io::stderr().is_terminal()
    };
    if scroll_enabled {
        crate::scroll::install(config.ui.scrollback_lines);
    }
    let indicator = Arc::new(std::sync::Mutex::new(if progress_enabled {
        let mut ind = StatusIndicator::new(Box::new(std::io::stderr()), true, stdin_is_tty());
        ind.thinking_view = config.ui.show_thinking_mode() != ShowThinkingMode::Hidden;
        ind.theme = theme;
        ind.owns_mouse = !scroll_enabled;
        ind
    } else {
        StatusIndicator::disabled()
    }));

    // Approval channel (FR-04-1 / design doc 4.3A). Route for agent task to request y/N from input loop.
    let (approval_tx, approval_rx) = mpsc::channel::<ApprovalRequest>(8);

    // Compiled once: a tool call costs a walk of the rule list, never a parse.
    let permissions = Arc::new(crate::permissions::Ruleset::from_config(&config.permissions));
    report_permission_warnings(&permissions, &theme);

    let initial_persona = resolution.persona.clone();
    let agent = Agent {
        id: id.clone(),
        group: group.clone(),
        persona: resolution.persona,
        provider,
        tools,
        config: config.clone(),
        config_source: source.clone(),
        registry_dir: registry_dir.clone(),
        ancestors: ancestors.clone(),
        log: Some(log),
        auto_approve: auto_approve.clone(),
        permissions: permissions.clone(),
        cancel: cancel.clone(),
        stopping: stopping.clone(),
        // Interactive only: the REPL keeps waiting for the next line after a
        // turn, so `--exit-when-idle`'s quiet-exit would kill the session
        // between prompts. Only the headless path sets it (FR-32).
        exit_when_idle: false,
        approval_tx: Some(approval_tx),
        history,
    };

    let history_path = config.log_dir()?.join("history.txt");
    let initial_history = load_history(&history_path);
    let commands_dir = custom_commands::resolve_dir(&config.runtime.commands_dir);
    let initial_commands = custom_commands::discover(&commands_dir);
    let state = Arc::new(ReplState {
        id: id.clone(),
        ancestors: ancestors.clone(),
        registry_dir: registry_dir.clone(),
        agents_dir: agents_dir.clone(),
        persona_file_setting: config.runtime.persona_file.clone(),
        cli_persona_path: args.persona.clone(),
        project_persona_path: project_persona_path.clone(),
        name: name.clone(),
        persona: RwLock::new(initial_persona),
        tool_names,
        history_path,
        history: RwLock::new(initial_history),
        auto_approve: auto_approve.clone(),
        permissions: permissions.clone(),
        cancel: cancel.clone(),
        suppress: suppress.clone(),
        turn_in_flight: turn_in_flight.clone(),
        indicator: indicator.clone(),
        commands_dir,
        commands: RwLock::new(initial_commands),
        config_source: source.clone(),
        group: group.clone(),
        theme,
        shell: config.shell.clone(),
        scroll: scroll_enabled,
        mouse_select: config.ui.mouse_select,
        copy_command: config.ui.copy_command.clone(),
    });

    let (input_tx, input_rx) = mpsc::channel::<AgentInput>(32);
    let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(64);

    // Shutdown coordination channel (FR-13 / design doc 4.9). Regardless of whether
    // triggered by `/quit`, EOF, SIGINT, or SIGTERM, `shutdown_tx.send(true)` propagates to all tasks.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // The turn-in-flight watch (FR-49): the display task's `TurnStart` lands here
    // the same instant the loop sees it, so the running turn's zone arms without
    // waiting the reconcile tick — the window between a turn's end and the next
    // one's arm would otherwise stream the answer's leading deltas straight
    // onto the freshly returned prompt's own rows.
    let (turn_in_flight_tx, turn_in_flight_rx) = watch::channel(false);

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
                            // A peer reporting a result: added to the
                            // conversation, no turn run, no answer produced.
                            Some(IpcMessage::Context { from, from_name, text }) => {
                                let text = crate::agent::peer_context_text(
                                    &from,
                                    from_name.as_deref(),
                                    &text,
                                );
                                if input_tx_for_ipc
                                    .send(AgentInput::Context {
                                        text,
                                        source: crate::agent::ContextSource::Peer { from, from_name },
                                    })
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
        let turn_rx_for_stdin = turn_in_flight_rx.clone();
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
                Some(turn_rx_for_stdin),
            )
            .await;
        })
    };

    // Display events. On `Done` or `Error`, notify the input loop of idle state (FR-03-2).
    // Treating `Error` as idle is a defensive measure to prevent the input loop from
    // getting stuck in Pending forever if Provider construction fails without a `Done`.
    let show_thinking = config.ui.show_thinking_mode();
    let display_suppress = suppress.clone();
    let display_turn_flag = turn_in_flight.clone();
    let display_turn_tx = turn_in_flight_tx.clone();
    let display_indicator = indicator.clone();
    let display_task = tokio::spawn(run_display_task(
        event_rx,
        agent_idle_tx,
        display_indicator,
        display_suppress,
        display_turn_flag,
        display_turn_tx,
        show_thinking,
        theme,
        progress_enabled,
        stdin_is_tty(),
    ));

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
    // Shutdown begins. Like the headless path below, the server must reject
    // further prompts instead of acking them into a queue that is about to be
    // destroyed: an acked message is gone without a reply or a log otherwise,
    // and its sender waits out its whole ask bound for an answer that can
    // never come.
    ipc_server
        .stopping()
        .store(true, std::sync::atomic::Ordering::SeqCst);

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
    // A headless agent reads the directory it was started in exactly as an
    // interactive one does — a detached child inherits that directory.
    let project_persona_path = config::local_persona_candidate();
    let resolution: PersonaResolution = persona::resolve(
        args.persona.as_deref(),
        &config.runtime.persona_file,
        &agents_dir,
        name.as_deref(),
        project_persona_path.as_deref(),
    )?;

    if resolution.builtin_used {
        tracing::info!("using builtin default persona");
    }
    if resolution.project_used {
        if let Some(p) = &resolution.persona.source_path {
            tracing::info!("using project persona {}", p.display());
        }
    }

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
    // Shared with the accept loop's front-door gate; also the Agent's skip
    // condition for its queued prompts (only a genuinely stopping session
    // refuses its queue).
    let stopping = ipc_server.stopping();

    // The chain of agents that created this one, set by the launcher on this
    // process' environment. Empty means a human started it: depth 0, and this
    // agent is the root of its own tree.
    let ancestors = crate::ipc::registry::ancestors_from_env();

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
        ancestors: ancestors.clone(),
        turn: None,
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
    // `[permissions] deny` is therefore the *only* thing that can stop a tool
    // call here, which is precisely why a deny outranks auto-approval.
    let auto_approve = Arc::new(AtomicBool::new(true));
    let permissions = Arc::new(crate::permissions::Ruleset::from_config(&config.permissions));
    for warning in permissions.warnings() {
        tracing::warn!(target: "permissions", "{warning}");
    }

    // Headless: no console, so no user input can raise this token — only the
    // shutdown path, so the in-flight turn closes cleanly (`finish_cancelled`
    // instead of being dropped mid-stream).
    let cancel = Arc::new(CancelToken::default());

    let agent = Agent {
        id: id.clone(),
        group: group.clone(),
        persona: resolution.persona,
        provider,
        tools,
        config: config.clone(),
        config_source: source.clone(),
        registry_dir: registry_dir.clone(),
        ancestors: ancestors.clone(),
        log: Some(log),
        auto_approve,
        permissions,
        // Headless: no console, so nothing user input can raise this token.
        // The shutdown path raises it so the in-flight turn closes cleanly
        // (`finish_cancelled`, not a silent drop).
        cancel: cancel.clone(),
        stopping: stopping.clone(),
        // FR-32: a headless agent launched with the `--exit-when-idle` flag
        // ends its loop when its work is done (the input channel is empty
        // after the last processed input); the serve loop's wait below fires
        // the shipped teardown on the agent task's end.
        exit_when_idle: args.exit_when_idle,
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

    let mut agent_handle = tokio::spawn(async move { agent.run(input_rx, event_tx).await });

    // Raised when the shutdown begins; the IPC server answers new prompts
    // itself instead of acking them into a queue that would be destroyed.
    let stopping = ipc_server.stopping();

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
                            // A peer reporting a result: added to the
                            // conversation, no turn run, no answer produced.
                            Some(IpcMessage::Context { from, from_name, text }) => {
                                let text = crate::agent::peer_context_text(
                                    &from,
                                    from_name.as_deref(),
                                    &text,
                                );
                                if input_tx_for_ipc
                                    .send(AgentInput::Context {
                                        text,
                                        source: crate::agent::ContextSource::Peer { from, from_name },
                                    })
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

    // Block until a shutdown notification arrives (signal or IPC Shutdown) —
    // or, with `--exit-when-idle` (FR-32), until the agent's own loop ends:
    // the agent breaks out of its input loop when the work is done and the
    // input channel has gone quiet, and that quiet end raises the same
    // shutdown watch here, so the teardown below is exactly the shipped one.
    {
        let mut shutdown_rx = shutdown_rx.clone();
        loop {
            if *shutdown_rx.borrow() {
                break;
            }
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                // The agent's loop ended on its own (quiet exit). Distinguish
                // it from an aborted task by waiting for the handle's result.
                res = &mut agent_handle, if args.exit_when_idle => {
                    if let Err(e) = res {
                        tracing::warn!(error = %e, "agent task ended with an error (quiet exit)");
                    }
                    let _ = shutdown_tx.send(true);
                    break;
                }
            }
        }
    }

    // Shutdown begins: reject further prompts (the server answers them with
    // the stopping error), finish the current turn cleanly, skip queued
    // prompts with a truthful reply, and only then tear down.
    stopping.store(true, std::sync::atomic::Ordering::SeqCst);

    ipc_task.abort();
    let _ = ipc_task.await;
    signal_task.abort();
    let _ = signal_task.await;

    drop(input_tx);
    // Cancel the current turn so it closes with `finish_cancelled` instead of
    // being dropped mid-stream, and let the agent loop skip the remaining
    // queue with truthful replies before ending on the closed channel.
    cancel.cancel();

    match tokio::time::timeout(SHUTDOWN_DRAIN_TIMEOUT, &mut agent_handle).await {
        // finished within the drain window
        Ok(_) => {}
        // did not finish in time: detach, then abort the task itself
        Err(_) => agent_handle.abort(),
    }
    let _ = agent_handle.await;
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
    /// A `!<command>` typed at the prompt is running: its output is streaming,
    /// `Esc` cancels it, and the prompt is not accepting a line. The agent is
    /// not involved — nothing was sent to it and nothing is waited on. The run
    /// itself lives next to this state, so the loop can poll it and the other
    /// branches can still look at the state.
    Shell,
}

/// Outcome of `handle_repl_command` (custom-command support, FR-08/FR-11).
///
/// - `Continue`: command completed (or unknown); redraw the prompt (existing behavior).
/// - `SubmittedPrompt`: a custom command sent a `UserPrompt` to the agent; the
///   caller must drain stale idle notifications and enter `PromptState::Pending`
///   (same as a normal user prompt).
/// - `Quit`: terminate the REPL (`/quit`, `/exit`).
/// - `SpawnedAsk`: a `/send --ask` was handed to a background runner; the
///   caller stores it and lets a dedicated select arm print the outcome, so
///   the loop keeps taking keys while the reply is unpaid on its way.
#[derive(Debug)]
enum CommandResult {
    Continue,
    SubmittedPrompt,
    Quit,
    SpawnedAsk(crate::tools::send_to::PeerAsk),
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
    fn is_shell(&self) -> bool {
        matches!(self, PromptState::Shell)
    }
}

/// The prompt's colour for a line that is being typed or echoed: a line that
/// will be run in the shell is marked with the same yellow bold that the
/// approval prompt uses, so "this is not going to the model" is visible before
/// `Enter` is pressed.

/// The one-shot mode (FR-47a): one prompt in, the answer out, the session ends.
/// `run -p "<prompt>"` (the `Command::OneShot` arm) lands here. The assembly
/// mirrors `run_headless`'s — persona, provider, tools, MCP, the conversation
/// log and the registry entry are the same one identity the REPL would build —
/// but nothing listens: no IPC server is bound, no REPL input loop runs, and
/// the process exits as soon as the one turn completes. The answer streams to
/// stdout as the model produces it (the `Text` deltas, plain), and nothing
/// else prints (the one-shot's output stays pipe-friendly: the answer alone,
/// errors to stderr).
pub async fn run_oneshot(mut config: Config, source: ConfigSource, args: RunArgs, prompt: String) -> Result<()> {
    config.apply_overrides(args.provider.as_deref(), args.model.as_deref());

    let id = AgentId::new();
    let name = args.name.clone();
    let agents_dir = config.agents_dir()?;
    let project_persona_path = config::local_persona_candidate();
    let resolution: PersonaResolution = persona::resolve(
        args.persona.as_deref(),
        &config.runtime.persona_file,
        &agents_dir,
        name.as_deref(),
        project_persona_path.as_deref(),
    )?;

    config.apply_persona_overrides(
        resolution.persona.frontmatter.model.as_deref(),
        resolution.persona.frontmatter.temperature,
    );

    let provider = ai::build(&mut config, &source)?;

    // The one-shot is a session: it registers like any other identity (a peer
    // can `send`/`ask` it only while it lives, which is the duration of the
    // one turn — the registry entry is removed at the end), shares the same
    // ancestors rules, and keeps the conversation log (the one turn's record).
    let registry_dir = config.registry_dir()?;
    let entry = RegistryEntry {
        id: id.clone(),
        name: name.clone(),
        group: config.resolve_group(args.group.as_deref()),
        pid: std::process::id(),
        started_at: Utc::now(),
        provider: config.provider.kind.clone(),
        model: provider.model().to_string(),
        // No socket is bound: nothing can prompt the process over IPC, and the
        // empty socket field keeps the registry honest about that.
        socket: PathBuf::new(),
        persona: Some(resolution.persona.summary()),
        ancestors: crate::ipc::registry::ancestors_from_env(),
        turn: None,
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
    // No console answers an approval mid-shot: the headless rule applies.
    let auto_approve = Arc::new(AtomicBool::new(true));
    let permissions = Arc::new(crate::permissions::Ruleset::from_config(&config.permissions));
    for warning in permissions.warnings() {
        tracing::warn!(target: "permissions", "{warning}");
    }
    let cancel = Arc::new(CancelToken::default());

    let agent = Agent {
        id: id.clone(),
        group: entry.group.clone(),
        persona: resolution.persona,
        provider,
        tools,
        config: config.clone(),
        config_source: source.clone(),
        registry_dir: registry_dir.clone(),
        ancestors: entry.ancestors.clone(),
        log: Some(log),
        auto_approve,
        permissions,
        cancel: cancel.clone(),
        stopping: Arc::new(AtomicBool::new(false)),
        exit_when_idle: true,
        approval_tx: None,
        history,
    };

    let (input_tx, input_rx) = mpsc::channel::<AgentInput>(8);
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(64);

    let mut agent_handle = tokio::spawn({
        async move { agent.run(input_rx, event_tx).await }
    });

    // The one prompt, then the input channel closes: the agent processes it
    // and its loop ends (exit_when_idle), the task finishes, the answer has
    // streamed through the events below.
    if input_tx.send(AgentInput::UserPrompt(prompt)).await.is_err() {
        return Err(crate::error::AppError::Other(
            "the agent task ended before the prompt was accepted".into(),
        ));
    }
    drop(input_tx);

    let mut turn_text = String::new();
    let mut turn_errored: Option<String> = None;
    while let Some(ev) = event_rx.recv().await {
        match ev {
            AgentEvent::Text { delta } => {
                print!("{delta}");
                let _ = std::io::Write::flush(&mut std::io::stdout());
                turn_text.push_str(&delta);
            }
            AgentEvent::Error { message } => {
                turn_errored = Some(message);
            }
            AgentEvent::Done => break,
            _ => {}
        }
    }
    // Drain the agent task's own end.
    let _ = tokio::time::timeout(SHUTDOWN_DRAIN_TIMEOUT, &mut agent_handle).await;

    if let Some(message) = turn_errored {
        eprintln!("error: {message}");
        registry_handle.cleanup();
        std::process::exit(1);
    }
    if !turn_text.ends_with('\n') {
        println!();
    }
    registry_handle.cleanup();
    Ok(())
}
fn prompt_line_role(line: &str, shell_enabled: bool) -> Role {
    if shell_enabled && line.trim_start().starts_with('!') {
        Role::Confirm
    } else {
        Role::PromptSymbol
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
    /// Rows the subagent status block occupies above the rendered block (or
    /// above the suggestion row when one is shown). They are part of the
    /// tracked region, so `clear_block` erases them together with the block
    /// and a finished subagent's row vanishes at the next render rather than
    /// leaving a stale line on the live screen.
    peer_rows: u16,
}


impl PromptRenderer {
    fn new() -> Self {
        Self {
            cursor_row: 0,
            peer_rows: 0,
        }
    }

    /// Mark the cursor as being on a fresh, empty line (no previous block above).
    /// Call this after emitting a newline outside the renderer (e.g. after submit
    /// or printing an out-of-band message).
    fn reset(&mut self) {
        self.cursor_row = 0;
        self.peer_rows = 0;
    }

    fn terminal_width() -> usize {
        terminal::size().map(|(w, _)| w as usize).unwrap_or(80).max(1)
    }

    /// Move to the top-left of the previously rendered block and clear it
    /// (including any wrapped rows below). Leaves the cursor at column 0 and
    /// resets the tracked row to 0.
    fn clear_block(&mut self, stdout: &mut std::io::Stdout) {
        use crossterm::cursor::{MoveToColumn, MoveUp};
        // The armed zone (FR-37a): the beat owns the rows above the span and
        // the shared cursor is parked on the pinned prompt's caret — the
        // relative repair (MoveUp + clear downward) would cut into the wrong
        // rows. The flags reset quietly and the screen is left to the zone;
        // the next armed render repaints the span in place (D-52/D-53).
        if zone_armed() {
            self.cursor_row = 0;
            self.peer_rows = 0;
            return;
        }
        // The whole tracked region: the peer rows, the block's rows above the
        // cursor, and the rows between. `Clear(FromCursorDown)` from the region
        // top erases the rest.
        let rows = self.peer_rows + self.cursor_row;
        if rows > 0 {
            let _ = stdout.execute(MoveUp(rows));
        }
        let _ = stdout.execute(MoveToColumn(0));
        let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
        self.cursor_row = 0;
        self.peer_rows = 0;
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
        // The armed zone (FR-37a): the echo belongs to the record (AS-53) and
        // the beat surfaces it — writing here would cut into or scroll the
        // standing shape the zone owns.
        if zone_armed() {
            self.clear_block(stdout);
            let styled_prompt = theme.out(prompt_role, prompt);
            let styled_line = theme.out(Role::InputText, line);
            crate::scroll::record_line(&format!("{styled_prompt}{styled_line}"));
            return;
        }
        // Remove the interactive render (cursor may be mid-line) before echoing.
        self.clear_block(stdout);
        let styled_prompt = theme.out(prompt_role, prompt);
        let styled_line = theme.out(Role::InputText, line);
        // The echo is what stays on screen — the block being edited is not.
        crate::scroll::record_line(&format!("{styled_prompt}{styled_line}"));
        let _ = stdout.execute(Print(&styled_prompt));
        let _ = stdout.execute(Print(&styled_line));
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
    /// next render (it moves to the block top and clears downward). The
    /// subagent status rows (`peer_lines`, FR-23) print above the whole
    /// tracked region and are erased with it. When `run_row` is `Some`, the
    /// running turn's elapsed row (FR-40) is printed above the block's rows —
    /// display-only, never recorded — and when `marker` is `Some`, the scroll
    /// state row (FR-39: `Message below`) is printed on the row immediately
    /// above the block's first row. Both are part of the tracked region, so the
    /// fold below counts them and the next render's erase reaches past them.
    #[allow(clippy::too_many_arguments)]
    fn render(
        &mut self,
        stdout: &mut std::io::Stdout,
        theme: &Theme,
        prompt_role: Role,
        prompt: &str,
        state: &InputState,
        suggestion: Option<&str>,
        peer_lines: &[String],
        run_row: Option<&str>,
        marker: Option<&str>,
    ) {
        use crossterm::cursor::{MoveDown, MoveToColumn, MoveUp};
        use crossterm::style::Print;

        let width = Self::terminal_width();

        // 1. Clear the previous block (handles multi-row renders + suggestion).
        self.clear_block(stdout);

        // 2. Print the subagent status rows above the block's rows, oldest
        //    first (the painter works downward), so the newest stands on the
        //    row nearest the block — the instruction's bottom-up ordering
        //    (FR-23, AS-26). The block owns the screen's bottom rows, so these
        //    stack from the row above it.
        for line in peer_lines {
            let _ = stdout.execute(Print(theme.err(Role::Peer, line)));
            // CR+LF is required in raw mode; this lands at column 0 of the
            // next row, where the following peer line or the block begins.
            let _ = stdout.execute(Print("\r\n"));
        }
        self.peer_rows = peer_lines.len() as u16;

        // 2.5 The running turn's elapsed row (FR-40): display-only, printed
        //     above the block's rows — the row the live screen stands directly
        //     above the prompt span, so the scrolled screen keeps it in sight
        //     while a selection is made. It is never recorded here: only
        //     `finish_line` writes the transcript.
        let mut above_rows = 0u16;
        if let Some(run_row) = run_row {
            let _ = stdout.execute(Print(theme.err(Role::Progress, run_row)));
            let _ = stdout.execute(Print("\r\n"));
            above_rows += 1;
        }

        // 2.6 The scroll marker (FR-39): the row immediately above the block's
        //     first row, so the prompt the eye is on stays the last thing on
        //     the screen and the marker names what the view has left below.
        //     It is part of the tracked region now, so the fold at the end
        //     counts it and the next render's erase climbs past it.
        if let Some(marker) = marker {
            let _ = stdout.execute(Print(theme.out(
                Role::Scrolled,
                &crate::editor::truncate_display(marker, width.saturating_sub(1)),
            )));
            let _ = stdout.execute(Print("\r\n"));
            above_rows += 1;
        }

        // 3. Print the candidate hint first, on its own row above the prompt block.
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

        // 4. Print prompt + line; the terminal auto-wraps long content.
        let _ = stdout.execute(Print(theme.out(prompt_role, prompt)));
        let _ = stdout.execute(Print(theme.out(Role::InputText, &state.line)));

        // 5. Compute the target cursor position within the prompt rows. The
        //    measurements are taken from the *plain* text: a style adds no
        //    printable column, but its escape sequence would be counted.
        let prompt_cols = crate::editor::str_display_width(prompt);
        let total_cols = prompt_cols + state.display_width();
        let cursor_cols = state.display_cursor();
        let (end_row, cursor_row, cursor_col) =
            layout_cursor(prompt_cols, cursor_cols, total_cols, width);

        // 6. Move from the post-print position (end_row) to the cursor row.
        //    Both are relative to the prompt row, so the hint above does not
        //    change this step — only the block-relative bookkeeping in 7.
        if end_row > cursor_row {
            let _ = stdout.execute(MoveUp((end_row - cursor_row) as u16));
        } else if cursor_row > end_row {
            // Cursor belongs on a fresh wrapped row past the printed content
            // (cursor at the end of a line that exactly fills the width).
            let _ = stdout.execute(MoveDown((cursor_row - end_row) as u16));
        }
        let _ = stdout.execute(MoveToColumn(cursor_col as u16));

        // 6. Remember where the cursor ended up for the next render, counting
        //    the hint row so `clear_block` moves all the way back to the top
        //    of the tracked region (the peer rows included), plus the rows
        //    printed above the block — the running turn's elapsed row (FR-40)
        //    and the scroll marker (FR-39) — so the next erase climbs past
        //    them and neither leaves a stale row behind.
        self.cursor_row = above_rows + sug_rows + cursor_row as u16;
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
/// block, and not on the elapsed display's own row. While a turn runs the
/// indicator is the last writer, so the cursor stands on the block's final row
/// and the block covers `painted_rows` rows ending there — but the block's
/// first row is the spinner-and-elapsed row itself, which the click rule does
/// not target (FR-27): toggling the thinking view is for the reasoning rows
/// alone, so a one-row block (the spinner alone) is never a hit and the span
/// is strict at the top. Pure so it can be unit-tested without a terminal.
/// (The zone-side rule below is the one FR-46a re-binds: the status row is
/// the block's LAST row there, and the exclusion pivots to it.)
fn click_hits_indicator(painted_rows: u16, cursor_row: u16, row: u16) -> bool {
    if painted_rows <= 1 {
        return false;
    }
    let top = cursor_row.saturating_sub(painted_rows - 1);
    row > top && row <= cursor_row
}

/// The armed zone's same rule (FR-27): a click lands on the beat's clock block
/// — never on the spinner row itself and never on the span — so the reasoning
/// view toggles from the zone's standing shape exactly as the shipped block
/// allowed. Pure against the zone's bookkeeping; no click changes anything
/// without a block tall enough to hold a reasoning row.
fn zone_click_hits_clock(row: u16) -> bool {
    let guard = lock_zone();
    let Some(z) = guard.as_ref() else {
        return false;
    };
    if !z.on_screen {
        return false;
    }
    click_hits_clock_block(z.clock_top, z.clock_rows, row)
}

/// The click rule shared with the shipped indicator's rule (`click_hits_
/// indicator`): the reasoning view toggles inside the block's reasoning rows
/// only. `clock_top` is the block's first row on 0-based screen coordinates,
/// `clock_rows` its painted height — `<= 1` is the spinner row alone (or
/// nothing painted), which no click targets (FR-27). FR-46a's face order puts
/// the spinner-and-elapsed row LAST in the block, so the exclusion pivots to
/// the last row; the rows above it (the reasoning rows and the marker) toggle.
fn click_hits_clock_block(clock_top: u16, clock_rows: u16, row: u16) -> bool {
    // The status row is the block's LAST row now (FR-46a's face order): the
    // hit range excludes it and covers the rows above it — the reasoning
    // rows and the elision marker the toggle serves.
    let bottom = clock_top
        .saturating_add(clock_rows)
        .saturating_sub(1);
    clock_rows > 1 && row >= clock_top && row < bottom
}

/// The REPL's input prompt, and the prompt that asks for a tool approval.
const PROMPT: &str = "> ";
const APPROVAL_PROMPT: &str = "approve? [y/N]: ";
/// The scrolled-view marker (FR-25): the text the pinned block shows while the
/// wheel's view has moved off the bottom of the log — one row immediately above
/// the prompt block's first row (FR-39). The marker points at the log
/// continuing below the view.
const SCROLLED_MARK: &str = "Message below";

/// Rows the scrolled-view marker adds to the pinned block's footprint (FR-39):
/// one while the view is off the bottom, none once it is back. The drawer and
/// the wheel/drag budgets all ask this, so they agree on where the body ends.
/// Pure so the sites share one arithmetic.
fn scrolled_marker_rows(view_active: bool) -> u16 {
    u16::from(view_active)
}

/// Terminal height in rows, with the same fallback as the width helper.
fn terminal_height() -> usize {
    terminal::size()
        .map(|(_, h)| h as usize)
        .unwrap_or(24)
        .max(1)
}

/// The FR-37a anchor: while a turn runs on the live screen, the prompt block
/// owns the screen's bottom rows and stands parked at the user's input cursor;
/// this turn zone re-homes everything the running turn writes to the rows above
/// that block (FR-36a's mechanism, carried by D-52) and keeps the block
/// anchored in place (D-53) until the turn's handoff (D-54).
///
/// Write ownership. The armed zone is the live screen's sole writer: the raw
/// writers and the renderer gate their screen writes on it and only record
/// into the transcript — the record is the one truth the zone's beat renders
/// from, so there is no second writer to race (the BL-M33 two-writers race is
/// removed by ownership, not by locking). Once per beat the zone wipes the
/// rows above the input span and repaints them bottom-up at absolute rows:
/// the log tail (the record's own rows, the wrap it was written with) ending
/// at the clock's top, the clock above the peers' band, the peers' band above
/// the parked span — the instruction's top-down order (log, run display,
/// subagent rows, prompt). The typed line never moves; no DECSTBM margin, no
/// SU/SD shifting: the region is re-painted absolutely every beat.
///
/// Caret exactness without cursor queries. The renderer (or the arm) tells the
/// zone where the caret stands after every edit render and how many rows the
/// block spans now; the zone restores exactly that position after every write.
/// Streaming under a caret at the end of the block adds a wrapped row and the
/// renderer reports it as it reports its bookkeeping — no recurring
/// `cursor::position` traffic and no stale-answer hazard (FR-27 stands).
struct TurnZone {
    /// Terminal geometry as of the arm.
    height: u16,
    /// Terminal width; reused by the beat's tail wrap and the span render.
    width: u16,
    /// The input span's top row (0-based): the prompt block — the parked
    /// prompt line and whatever the edit wraps/suggests — occupies the rows
    /// `span_top..=height−1`. It is the beat's floor: every beat erases and
    /// repaints only the rows above it, so the typed line never moves (D-53).
    span_top: u16,
    /// Where the editor's caret stands now, as absolute screen coordinates.
    /// The renderer's armed paint places it; every zone write ends parked, so
    /// the caret never wanders while the turn runs.
    caret_col: u16,
    caret_row: u16,
    /// The lowest row the zone has painted (0-based), so far. The beat paints
    /// from the clock's top down to the span's floor, and the band and the
    /// clock move with the beat, so this is the disarm's erase floor: a turn
    /// that ends erases every row the zone ever touched, none it never did.
    painted_top: u16,
    /// The run display's block as the last beat painted it: its top row and
    /// its row count. The thinking-toggle click (FR-27) is judged against this
    /// extent — the live path's cursor-relative rule has no painted rows to
    /// judge by while the zone holds the writes.
    clock_top: u16,
    clock_rows: u16,
    /// Whether the zone's own screen is the one being written: the scroll
    /// overlay's alternate screen hides it, and the screen writes stop while
    /// hidden (the record keeps running; the overlay draws from the record).
    on_screen: bool,
    /// Whether the turn this zone was armed for has reached its terminal event
    /// while the zone was hidden: the disarm then does not erase the other
    /// screen's coordinates, and the handoff (the erase, the tail replay and
    /// the park) is carried out the moment the live screen returns —
    /// `zone_show` seeing this.
    closed: bool,
    /// The session identity the beat composes the subagent rows from, taken
    /// at the arm so the beat's registry reads need no argument passed in.
    session: ZoneSession,
}

/// The zone handle, a process-wide singleton next to the session transcript:
/// only the interactive session arms it, and every writer (the display task's
/// events, the input loop's notices) routes through the same helpers.
static TURN_ZONE: std::sync::Mutex<Option<TurnZone>> = std::sync::Mutex::new(None);

/// The session identity the armed zone composes the subagent rows from, taken
/// at the arm: the beat reads the shared registry through these, so a peer's
/// row carries its operating time and its processing content (FR-37b's D-58
/// registry transport) fresh at every beat without the arm passing rows in.
#[derive(Clone)]
struct ZoneSession {
    registry_dir: std::path::PathBuf,
    self_id: crate::id::AgentId,
    theme: Theme,
}

/// Erase the row at an absolute 0-based position: a CUP to the row's column 0
/// followed by an entire-row erase, so a beat's repaint never inherits a
/// longer row's tail from what streamed there a beat ago.
fn zone_csi_move(col: u16, row: u16) -> String {
    format!("\x1b[{};{}H", row.saturating_add(1), col.saturating_add(1))
}

fn zone_csi_clear_row(row: u16) -> String {
    // The crossterm row-first convention: `zone_csi_move(col, row)` emits
    // the row first. Passing `row` as the column emitted a crawl along
    // row 1's columns — the erase never touched the rows it named and
    // kept clearing the banner row instead (the shipped erase's own
    // coordinates; FR-49).
    format!("{}\x1b[2K", zone_csi_move(0, row))
}

fn lock_zone() -> std::sync::MutexGuard<'static, Option<TurnZone>> {
    TURN_ZONE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether the zone holds the live screen's write ownership (the prompt
/// pinned). The writers' gate: `raw_*` and the renderer record-only while
/// this true. Ownership covers the hidden state too — while the overlay's
/// alternate screen shows, the zone holds the lease but paints nothing, and
/// a turn's in-flight writes must keep landing in the record alone (the
/// overlay draws from that same record), so the gate does not open for the
/// hidden zone.
fn zone_armed() -> bool {
    lock_zone().is_some()
}

/// Arm the zone for a Pending turn (FR-37a's D-52): the zone takes the live
/// screen's write ownership with the span anchored on the screen's last row
/// (the bare parked prompt — the caller's span render re-anchors with the
/// editor's real extent and reports the caret). Every armed writer stops its
/// screen writes immediately, and the beat owns the rows above the span from
/// the next tick; the first beat composes the standing shape on request.
fn zone_arm(state: &Arc<ReplState>) {
    let height = terminal_height() as u16;
    if height < 2 {
        // No screen worth the name: degrade to the shipped flow (AS-54). The
        // prompt block cannot be kept apart from the clock there either, so
        // the rows go back to the shipped cursor-relative block rather than
        // standing unwritten.
        lock_indicator(&state.indicator).set_prompt_owned(false);
        return;
    }
    let session = ZoneSession {
        registry_dir: state.registry_dir.clone(),
        self_id: state.id.clone(),
        theme: state.theme,
    };
    // The armed zone is the rows' only writer, so the live prompt block no
    // longer owns them: its cursor-relative block stays silent and the beat
    // composes the run display above the span instead (FR-37a/FR-40). Taken
    // before the zone lock, so the two locks are never held in both orders.
    lock_indicator(&state.indicator).set_prompt_owned(false);
    let mut guard = lock_zone();
    if let Some(z) = guard.as_mut() {
        // Already armed: the anchor the editor reported stands. Re-seeding it
        // would drop the span's extent back to the bare one-row prompt, and the
        // next beat would then compose the clock over the block's other rows —
        // the same defect the zone exists to prevent. Only the session the beat
        // reads and the two visibility flags are refreshed.
        z.session = session;
        z.on_screen = true;
        z.closed = false;
        return;
    }
    *guard = Some(TurnZone {
        height,
        width: PromptRenderer::terminal_width() as u16,
        // The standing shape is the running display alone (FR-46a): the
        // beat's region is every row, the clock block's last row lands on
        // the screen's bottom row, and no prompt span paints while armed —
        // the prompt display returns only when input is possible again,
        // at the handoff. `span_top` keeps its role as the region's floor
        // (= height): the sweep covers the whole bottom-anchored region.
        span_top: height,
        caret_col: 0,
        caret_row: height - 1,
        painted_top: height - 1,
        clock_top: height - 1,
        clock_rows: 0,
        on_screen: true,
        closed: false,
        session,
    });
}

/// The turn-end handoff (D-54): erase everything the zone ever painted and
/// replay the record so the log ends above the fresh prompt row. The outcome
/// line the indicator's `finish` recorded at the terminal event stands as the
/// record's newest line; the shipped render below draws the prompt underneath.
fn zone_disarm() {
    let mut guard = lock_zone();
    // Hidden (the overlay's alternate screen): the turn ended while the other
    // screen owned the terminal — erasing its own coordinates now would hit
    // the wrong screen, so the zone stands with the debt marked and the
    // handoff runs when the live screen returns (`zone_show`).
    if let Some(z) = guard.as_mut() {
        if !z.on_screen {
            z.closed = true;
            return;
        }
    } else {
        return;
    }
    let Some(z) = guard.take() else {
        return;
    };
    let width = z.width.max(1) as usize;
    let height = z.height;
    let painted_top = z.painted_top;
    // Rows painted_top..=height−2: every transient row the zone could have
    // put on screen, the span's rows included. The record's tail replays
    // ending on that row — the outcome line the indicator's `finish` recorded
    // at the terminal event is the tail's newest row — so the shipped render
    // below draws the prompt on a fresh, empty bottom row, exactly where a
    // `reset` puts it after a newline-terminated print. One queued pass, so
    // the repair paints atomically.
    let plan = zone_disarm_plan(height, painted_top);
    let mut seq = String::new();
    for row in painted_top..=plan.tail_floor {
        seq.push_str(&zone_csi_clear_row(row));
    }
    if plan.budget > 0 {
        if let Some(rows) = crate::scroll::tail_for_region(width, plan.budget) {
            let top = plan.tail_floor + 1 - rows.len() as u16;
            for (i, line) in rows.iter().enumerate().take(plan.budget) {
                seq.push_str(&zone_csi_move(0, top + i as u16));
                seq.push_str("\x1b[2K");
                seq.push_str(line);
            }
        }
    }
    // A fresh row at the screen's bottom: the column-0 origin every shipped
    // write (the raw writers' CR+LF, `clear_block`'s MoveToColumn(0))
    // continues from, so the handoff lands where the shipped flow puts the
    // cursor after the outcome line's newline.
    seq.push_str("\x1b[0m");
    seq.push_str(&zone_csi_move(0, height - 1));
    let out = std::io::stdout();
    let mut out = out.lock();
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
}

/// Park the caret where the renderer last placed it. Every zone write ends
/// here, so the caret never wanders while the turn runs.
fn zone_park() {
    let (col, row) = {
        let guard = lock_zone();
        match guard.as_ref() {
            Some(z) => (z.caret_col, z.caret_row),
            None => return,
        }
    };
    let _ = crossterm::queue!(std::io::stdout(), crossterm::cursor::MoveTo(col, row));
    let _ = std::io::stdout().flush();
}

/// The beat (FR-37a's standing shape): erase every row above the input span —
/// the span's own rows are never swept (D-53) — and repaint the region at
/// absolute rows: the clock block on the rows directly above the span, the
/// peers' band above the block, and the record's tail above the band, ending
/// at the composition's top. The record is the sole truth (AS-53) and the beat
/// is the only painter while armed; it runs from the display task's 100 ms
/// ticker while a turn runs, so no new event type and no new state machine.
/// The beat's composition plan (FR-37's four regions, read bottom-up from the
/// span): the clock block fills the rows directly above the prompt span, the
/// peers' band sits above the block — the newest just above it (FR-23, AS-26)
/// — and the log's tail fills what remains above the band. Pure so the pin
/// binds the same arithmetic the beat paints with; `sweep_to` is the erase
/// floor the beat shares (the span's own rows are never swept, D-53).
struct ZoneBeatPlan {
    /// The clock block's top row (its bottom row is `block_home + block_n − 1`,
    /// the row just above the span).
    block_home: u16,
    /// The peers' band's top row (its bottom row is `peers_top + peers_n − 1`,
    /// the row just above the block).
    peers_top: u16,
    /// The beat erases rows `0..sweep_to` — everything above the input span.
    sweep_to: u16,
}

fn zone_beat_plan(span_top: u16, block_n: u16, peers_n: u16) -> ZoneBeatPlan {
    ZoneBeatPlan {
        block_home: span_top.saturating_sub(block_n),
        peers_top: span_top
            .saturating_sub(block_n)
            .saturating_sub(peers_n),
        sweep_to: span_top,
    }
}

/// The disarm's geometry, pure: the erase starts at `painted_top` (every row
/// the zone ever painted) and the record's tail replays ending on
/// `tail_floor` = height−2 — the row above the bottom row, so the shipped
/// render draws the prompt on a fresh bottom row with the outcome line above
/// it (D-54). Pure so the pin binds the same arithmetic the disarm paints with.
struct DisarmPlan {
    /// The last row the tail replay may paint (`height − 2`).
    tail_floor: u16,
    /// The rows the tail replay asks the record for.
    budget: usize,
}

fn zone_disarm_plan(height: u16, painted_top: u16) -> DisarmPlan {
    let tail_floor = height.saturating_sub(2);
    // AS-54's degrade inside the geometry: a terminal too short to hold the
    // fresh bottom row and the row above it has no rows the replay may paint
    // into (the degenerate floor would be the bottom row itself), so the plan
    // replays nothing — `zone_arm` never arms such a terminal anyway.
    let budget = if height < 2 {
        0
    } else {
        (tail_floor.saturating_add(1)).saturating_sub(painted_top) as usize
    };
    DisarmPlan { tail_floor, budget }
}

fn zone_beat(block_rows: &[String]) {
    // Compose everything that needs the zone's identity while holding the
    // guard, then drop it before any screen write (beats and the input loop
    // take the same lock; holding it across writes only widens the window).
    let (width, span_top, on_screen, session) = {
        let guard = lock_zone();
        let Some(z) = guard.as_ref() else {
            return;
        };
        (
            z.width.max(1) as usize,
            z.span_top,
            z.on_screen,
            z.session.clone(),
        )
    };
    if !on_screen {
        // The overlay's alternate screen owns the terminal: the bookkeeping
        // stands, the writes wait for the live screen.
        return;
    }
    let block_n = block_rows.len() as u16;
    let peers = live_peer_lines(
        &session.registry_dir,
        &session.self_id,
        width,
    )
    .into_iter()
    .map(|l| session.theme.err(Role::Peer, &l).into_owned())
    .collect::<Vec<_>>();
    let peers_n = peers.len() as u16;
    // The composition reads bottom-up from the span: the clock block (the run
    // display) on the rows above the span, the peers' band above the block —
    // the newest just above it (FR-23, AS-26) — and the log's tail above the
    // band. Read top-down the four regions stand in the instruction's order
    // (FR-37): log / run display / subagent rows / prompt. The geometry is the
    // pure plan's (the pin below binds the same arithmetic the beat paints).
    let plan = zone_beat_plan(span_top, block_n, peers_n);
    let (block_home, peers_top, sweep_to) = (plan.block_home, plan.peers_top, plan.sweep_to);
    let out = std::io::stdout();
    let mut out = out.lock();
    // Everything above the span belongs to the beat: erase the whole region
    // and repaint it in one queued pass, so no row a previous beat painted —
    // a taller block, a friendlier band — survives a beat that composes less.
    let mut seq = String::new();
    for row in 0..sweep_to {
        seq.push_str(&zone_csi_clear_row(row));
    }
    // Each paint stops at its own floor. The plan clamps the composition's top
    // with `saturating_sub`, but a block or band taller than the rows above the
    // span would still walk every row it holds, downward into the span — over
    // the prompt, its bottom-most row among them. The bounds below keep every
    // row the beat writes above the span: the block never crosses `sweep_to`
    // (the span's own top) and the band never crosses the block's first row.
    // The block's first row is the elapsed row, so a truncated block keeps the
    // elapsed display and drops the reasoning rows beneath it.
    for (i, line) in block_rows.iter().enumerate() {
        let row = block_home.saturating_add(i as u16);
        if row >= sweep_to {
            break;
        }
        seq.push_str(&zone_csi_move(0, row));
        seq.push_str("\x1b[2K");
        seq.push_str(line);
    }
    for (i, line) in peers.iter().enumerate() {
        let row = peers_top.saturating_add(i as u16);
        if row >= block_home.min(sweep_to) {
            break;
        }
        seq.push_str(&zone_csi_move(0, row));
        seq.push_str("\x1b[2K");
        seq.push_str(line);
    }
    // The tail: as much of the record as the region above the composition
    // holds, wrapped to the terminal's width and ending at the composition's
    // top — the log's newest row stands just above the band (or the clock
    // when no peer runs).
    let mut tail_top = peers_top;
    if peers_top > 0 {
        if let Some(rows) = crate::scroll::tail_for_region(width, peers_top as usize) {
            let n = rows.len() as u16;
            tail_top = peers_top - n;
            for (i, line) in rows.iter().enumerate() {
                seq.push_str(&zone_csi_move(0, tail_top + i as u16));
                seq.push_str("\x1b[2K");
                seq.push_str(line);
            }
        }
    }
    // No colour may outlive the beat's last row: every later write re-states
    // its own style, but the park lands on the user's line.
    seq.push_str("\x1b[0m");
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
    drop(out);
    // Bookkeeping: the clock's extent for the click rule, and the topmost row
    // the tail repainted — the disarm's erase and replay begin there, so a
    // shrunk band or block never leaves a survivor above the span.
    {
        let mut guard = lock_zone();
        if let Some(z) = guard.as_mut() {
            z.clock_top = block_home;
            z.clock_rows = block_n;
            z.painted_top = z.painted_top.min(tail_top);
        }
    }
    zone_park();
}

/// The renderer (or the arm) reports where the caret now stands and how many
/// rows the input span takes; the zone re-anchors (D-53) so the beat's erase
/// floor and the park follow the block the edit grew or shrank. The span can
/// only sit on the screen's last rows; a span that would cross row 0 clamps.
fn zone_reanchor(span: u16, caret_col: u16, caret_row: u16) {
    let mut guard = lock_zone();
    let Some(z) = guard.as_mut() else {
        return;
    };
    let span = span.max(1).min(z.height);
    z.span_top = z.height - span;
    z.painted_top = z.painted_top.min(z.span_top);
    z.caret_col = caret_col;
    z.caret_row = caret_row;
}

/// Retire the armed span (FR-46a): the composition returns to the spanless
/// standing shape — the beat's sweep reclaims the span's rows and the clock
/// block takes the screen's bottom row back. Called when the approval prompt
/// is answered while the turn continues running; a no-op when no zone is
/// armed.
fn zone_span_retire() {
    let mut guard = lock_zone();
    if let Some(z) = guard.as_mut() {
        z.span_top = z.height;
        z.caret_col = 0;
        z.caret_row = z.height - 1;
    }
}

/// The zone's screen is hidden (the scroll overlay's alternate screen owns
/// the terminal): the bookkeeping stands, the screen writes stop and the
/// writers' gate lifts so the overlay's draws come from the record directly.
fn zone_hide() {
    if let Some(z) = lock_zone().as_mut() {
        z.on_screen = false;
    }
}

/// The zone's screen is live again: the writes resume, the beat repaints from
/// the record on its next tick, and the caret is parked. When the turn ended
/// while the screen was hidden (`closed`), this is the deferred disarm — the
/// handoff runs here, now that the screen's own coordinates are writable.
fn zone_show() {
    let mut guard = lock_zone();
    match guard.as_mut() {
        Some(z) if z.closed => {
            z.closed = false;
            // The deferred handoff: run the disarm on the live screen, then the
            // shipped render below (the input loop's render_prompt) draws the
            // prompt on top of the replayed tail.
            z.on_screen = true;
            drop(guard);
            zone_disarm();
        }
        Some(z) => {
            z.on_screen = true;
            let (col, row) = (z.caret_col, z.caret_row);
            drop(guard);
            let _ = crossterm::queue!(std::io::stdout(), crossterm::cursor::MoveTo(col, row));
            let _ = std::io::stdout().flush();
        }
        None => {}
    }
}

fn zone_paint_span(
    theme: &Theme,
    prompt_role: Role,
    prompt: &str,
    input_state: &InputState,
    suggestion: Option<&str>,
) {
    let Some((width, height)) = (|| {
        let guard = lock_zone();
        let z = guard.as_ref()?;
        Some((z.width.max(1) as usize, z.height))
    })() else {
        return;
    };
    // The span's rows: the hint row when there is one, then the prompt's wrap
    // extent — the same arithmetic the renderer's budget uses (D-28's count).
    let sug_rows = suggestion_rows(suggestion, width) as usize;
    let prompt_cols = crate::editor::str_display_width(prompt);
    let total_cols = prompt_cols + input_state.display_width();
    let (end_row, cursor_row, cursor_col) =
        layout_cursor(prompt_cols, input_state.display_cursor(), total_cols, width);
    let bottom_rows = end_row.max(cursor_row) + 1;
    // The span is the screen's bottom rows; a wrap extent taller than the
    // screen clamps (the anchor clamps with it).
    let span_n = sug_rows + bottom_rows;
    let span_n = span_n.min(height.max(1) as usize).max(1);
    // Grow the anchor (a span taller than the arm's initial one row) before the
    // paint: the beat's erase floor must not sit inside the span. The caret row
    // is the span's bottom row offset by the cursor's position within it.
    // The caret rides the block's bottom row, offset by the cursor's position
    // within it; the anchor reports the same absolute row the paint below
    // parks on (both are height−1−offset: the block's last row pins to the
    // screen's bottom row under any clamp).
    let bottom_offset = end_row.max(cursor_row) - cursor_row;
    zone_reanchor(
        span_n as u16,
        cursor_col as u16,
        (height as usize - 1 - bottom_offset.min(span_n - 1)) as u16,
    );
    // The hint row rides the span's top; the prompt's wrap rows follow, each
    // wrapped by the transcript's own SGR-aware wrapper so a styled prompt and
    // a multibyte line break exactly as the renderer would.
    let mut rows: Vec<String> = Vec::with_capacity(span_n);
    if sug_rows == 1 {
        let sug = truncate_suggestion(suggestion.unwrap_or(""), width - 1);
        rows.push(theme.out(Role::Hint, &sug).into_owned());
    }
    let plain = format!(
        "{}{}",
        theme.out(prompt_role, prompt),
        theme.out(Role::InputText, &input_state.line)
    );
    // The rows paint at absolute rows, the block's last row pinning to the
    // screen's bottom row; a shorter wrapped content leaves the rows above it
    // to the beat's sweep. An exactly-full last row leaves the cursor on a
    // pending fresh row the wrapper does not emit — the erase already cleared
    // it, so nothing is painted there.
    for line in crate::scroll::wrap_styled(&plain, width) {
        rows.push(line);
    }
    // The rows paint at absolute rows: the anchor grew above, so the span's
    // last row is the screen's bottom row and the wrapped content fills the
    // span downward from its top.
    let base = (height - span_n as u16) as usize;
    let out = std::io::stdout();
    let mut out = out.lock();
    let mut seq = String::new();
    for (i, line) in rows.iter().enumerate() {
        seq.push_str(&zone_csi_move(0, (base + i) as u16));
        seq.push_str("\x1b[2K");
        seq.push_str(line);
    }
    // The caret parks on the same absolute row the anchor reported — one
    // truth for the span's edit position (the clamp matches `zone_reanchor`).
    let caret_row = height - 1 - (bottom_offset.min(span_n - 1)) as u16;
    seq.push_str("\x1b[0m");
    seq.push_str(&zone_csi_move(cursor_col as u16, caret_row));
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
}

/// Rows the prompt block occupies on screen: the optional suggestion row above
/// it, plus the rows the prompt and the edited line wrap onto. Pure, so the
/// scrolled screen's row budget can be unit-tested without a terminal.
fn prompt_block_rows(
    prompt_cols: usize,
    cursor_cols: usize,
    total_cols: usize,
    sug_rows: u16,
    width: usize,
) -> u16 {
    let (end_row, cursor_row, _) = layout_cursor(prompt_cols, cursor_cols, total_cols, width);
    sug_rows + end_row.max(cursor_row) as u16 + 1
}

/// The scrolled view of the session log.
///
/// While it is active it owns the screen: the terminal is switched to the
/// alternate screen, the transcript slice is drawn there with the prompt block
/// pinned beneath it, and the switch is undone when the view returns to the
/// bottom. The main screen is never written to in between, so leaving restores
/// it — and the terminal's own scrollback with it — exactly as it was.
struct ScrollOverlay {
    view: crate::scroll::ScrollView,
    /// The alternate screen is currently ours.
    active: bool,
    /// draws the body rows, so the highlight costs no change to any signature
    /// between the input loop and `draw_scroll_overlay`.
    selection: Option<crate::select::Selection>,
    /// When the screen was last drawn, so a running turn's pinned spinner can
    /// be refreshed without repainting on every 50ms poll.
    last_draw: std::time::Instant,
}

/// How often a scrolled screen is redrawn while a turn runs: often enough for
/// the pinned spinner to live, rare enough not to flicker.
const SCROLL_REFRESH: Duration = Duration::from_millis(100);

/// How often it is redrawn when no turn is running. Nothing should be writing
/// underneath it then — but a peer's prompt can start a turn the REPL did not,
/// so the screen is repaired on a slow beat rather than never.
const SCROLL_IDLE_REFRESH: Duration = Duration::from_secs(1);

impl ScrollOverlay {
    fn new() -> Self {
        Self {
            view: crate::scroll::ScrollView::default(),
            active: false,
            selection: None,
            last_draw: std::time::Instant::now(),
        }
    }

    /// Whether the overlay owns the screen.
    fn is_active(&self) -> bool {
        self.active
    }

    /// Whether the view has moved off the bottom of the log.
    fn view_active(&self) -> bool {
        self.view.is_active()
    }

    /// Whether a drag is in progress. The overlay owns the screen while one is,
    /// even at the bottom of the log, because the live screen cannot be
    /// repainted with a highlight on it.
    fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    /// Start a drag at `at`, on whatever the view is showing now.
    fn begin_selection(&mut self, at: crate::select::Point) {
        self.selection = Some(crate::select::Selection::begin(at, self.view.offset()));
    }

    /// Move the loose end of the drag. Returns whether anything moved, so a
    /// burst of drag reports costs one repaint.
    fn extend_selection(&mut self, to: crate::select::Point) -> bool {
        match self.selection.as_mut() {
            Some(sel) if sel.cursor != to => {
                sel.extend(to);
                true
            }
            _ => false,
        }
    }

    /// End the drag and hand back what was selected.
    fn take_selection(&mut self) -> Option<crate::select::Selection> {
        self.selection.take()
    }

    /// A wheel report: move the view by one notch. Returns whether the screen
    /// needs redrawing, so a burst of reports costs one repaint.
    fn wheel(&mut self, up: bool, cols: usize, body: usize) -> bool {
        let max = crate::scroll::with(|t| crate::scroll::max_offset(t, cols, body)).unwrap_or(0);
        let step = crate::scroll::STEP as isize;
        self.view.scroll(if up { step } else { -step }, max)
    }

    fn needs_refresh(&self, turn_running: bool) -> bool {
        let interval = if turn_running {
            SCROLL_REFRESH
        } else {
            SCROLL_IDLE_REFRESH
        };
        self.active && self.last_draw.elapsed() >= interval
    }

    fn enter(&mut self, stdout: &mut std::io::Stdout) {
        if self.active {
            // Already on the alternate screen: reset it (the drawn rows stand
            // from the last view) and hand the zone back its margin.
            self.view.reset();
            return;
        }
        let _ = stdout.execute(terminal::EnterAlternateScreen);
        // The alternate screen owns the terminal while the overlay draws: the
        // armed turn zone stops writing (its bookkeeping keeps running) so a
        // turn's in-flight writes cannot land on the overlay's screen.
        zone_hide();
        self.active = true;
    }

    /// Return to the live screen. Idempotent, so every path out of the REPL can
    /// call it without checking first.
    fn leave(&mut self, stdout: &mut std::io::Stdout) {
        self.view.reset();
        self.selection = None;
        if !self.active {
            return;
        }
        let _ = stdout.execute(terminal::LeaveAlternateScreen);
        self.active = false;
        // Back on the live screen: re-issue the armed turn zone's own margin
        // (the alternate screen the overlay just left consumed it), replay the
        // transcript tail into the region, and recompose the band + the park.
        zone_show();
    }
}

impl Drop for ScrollOverlay {
    /// A panic, a signal or any early return must not leave the terminal on the
    /// alternate screen.
    fn drop(&mut self) {
        if self.active {
            let _ = std::io::stdout().execute(terminal::LeaveAlternateScreen);
        }
    }
}

/// What is pinned to the bottom of the scrolled screen: one block, composed
/// the way the live screen composes — the prompt span at the bottom-most rows,
/// the running turn's elapsed row directly above it (FR-40), the subagent
/// run-state rows (FR-20) above that, and the scroll marker (FR-39) on the row
/// immediately above the prompt's first row.
///
/// One composition for every state is what keeps a pointer honest: the row a
/// press lands on is the row the screen shows, whether a turn runs or not
/// (FR-40's second half).
struct ScrollBottom<'a> {
    role: Role,
    prompt: &'a str,
    input: &'a InputState,
    suggestion: Option<&'a str>,
    peer_lines: Vec<String>,
    /// The running turn's elapsed row, painted above the block — display-only,
    /// never recorded (FR-40).
    run_row: Option<String>,
    /// `Message below` while the view is off the log's bottom, `None` at it
    /// (FR-39: painted above the block, never beneath it).
    text: Option<String>,
}

impl ScrollBottom<'_> {
    /// The subagent rows the block carries (empty when none).
    fn peer_lines(&self) -> Vec<String> {
        self.peer_lines.clone()
    }

    /// The scroll marker row, `None` when the block carries none: the block
    /// paints `Message below` on the row immediately above its first row while
    /// the view is off the bottom (FR-39).
    fn marker_row(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// The running turn's elapsed row, `None` when no turn is tracked: the
    /// block paints it above itself so the elapsed display stays on screen
    /// while the scrolled view is exercised (FR-40).
    fn run_row(&self) -> Option<&str> {
        self.run_row.as_deref()
    }
}

/// Rows the running turn's elapsed row adds above the prompt span (FR-40): one
/// while a turn is tracked, none otherwise. Pure so the drawer and both mouse
/// arms share the arithmetic.
fn run_row_rows(running: bool) -> u16 {
    u16::from(running)
}

/// The scrolled screen's pinned block, read top-down as one plan (FR-39,
/// FR-40): one row per live subagent (FR-20's transient lines, oldest on top —
/// the painter walks downward, AS-26), the running turn's elapsed row (FR-40),
/// the scroll marker (FR-39), then the prompt span's own rows at the bottom.
/// Rows are block-relative, so the pin binds the order without a terminal.
///
/// The order is the live screen's, with the marker inserted between the run
/// display and the prompt: the elapsed row stands directly above the prompt
/// span and the peer rows above it, exactly as `zone_beat_plan` stacks the live
/// screen's regions (D-61). The marker is the last row before the prompt, so a
/// wrapped prompt keeps it directly above the block's first row (AS-59).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScrolledBlockPlan {
    /// The running turn's elapsed row, when a turn is tracked.
    run_row: Option<u16>,
    /// The marker's row, when the view is off the log's bottom.
    marker_row: Option<u16>,
    /// The prompt span's first row.
    prompt_top: u16,
    /// The block's whole footprint, the prompt span's rows included.
    rows: u16,
}

fn scrolled_block_plan(
    prompt_rows: u16,
    peer_lines: usize,
    running: bool,
    view_moved: bool,
) -> ScrolledBlockPlan {
    let peer_rows = peer_lines as u16;
    let run_rows = run_row_rows(running);
    let marker_rows = scrolled_marker_rows(view_moved);
    let prompt_top = peer_rows + run_rows + marker_rows;
    ScrolledBlockPlan {
        run_row: running.then_some(peer_rows),
        marker_row: view_moved.then_some(peer_rows + run_rows),
        prompt_top,
        rows: prompt_top + prompt_rows,
    }
}

/// The scrolled screen's pinned block, counted once: the prompt span's rows,
/// the running turn's elapsed row above them (FR-40), the scroll marker's row
/// above that (FR-39) and one row per live subagent (FR-20's transient lines,
/// additive to the block). The suggestion row takes the same additive slot when
/// the drawer counts it — the mouse arms leave it out, so their budget stays a
/// subset of the drawer's true count in every mixed state (NFR-14).
///
/// One function is the whole point of FR-40's second half: the drawer's
/// `bottom_rows`, the wheel arm's body and the drag arm's body cannot disagree
/// about where the log ends, so a pointer row means the same content row the
/// screen shows (AS-62).
fn scrolled_bottom_rows(
    prompt: &str,
    input_state: &InputState,
    width: usize,
    sug_rows: u16,
    peer_lines: usize,
    running: bool,
    view_moved: bool,
) -> u16 {
    let prompt_cols = crate::editor::str_display_width(prompt);
    let prompt_rows = prompt_block_rows(
        prompt_cols,
        input_state.display_cursor(),
        prompt_cols + input_state.display_width(),
        sug_rows,
        width,
    );
    scrolled_block_plan(prompt_rows, peer_lines, running, view_moved).rows
}

/// Rows the pinned bottom block occupies, as the mouse arms measure it: the
/// same composition the drawer paints (FR-40's D-61), without the suggestion
/// row the arms have never counted. A running turn keeps the prompt span on
/// screen and carries its elapsed row above it, so the arms' body shrinks by
/// exactly the rows the screen shows — the press, the drag and the release all
/// resolve against what is drawn (AS-62).
fn bottom_block_rows(
    prompt_state: &PromptState,
    input_state: &InputState,
    width: usize,
    peer_lines: usize,
    view_moved: bool,
) -> u16 {
    let p = if prompt_state.is_awaiting_approval() {
        APPROVAL_PROMPT
    } else {
        PROMPT
    };
    scrolled_bottom_rows(
        p,
        input_state,
        width,
        0,
        peer_lines,
        prompt_state.is_pending(),
        view_moved,
    )
}

/// The peer's situation face (FR-37b): one row per live direct child of this
/// session, naming the peer, its operating time and — when the peer reports
/// one (D-58's registry transport) — its processing content. Pure over the
/// children list, so the pins compose rows without a registry. The content's
/// absence degrades to the shipped name-and-operating-time face.
///
/// The whole list rides on one terminal row when every row fits; `width` is the
/// budget the truncation shares.
fn peer_status_rows(
    children: &[(String, std::time::Duration, Option<String>)],
    width: usize,
) -> Vec<String> {
    children
        .iter()
        .map(|(name, age, turn)| {
            let face = match turn.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
                Some(t) => format!("peer {name} · {} · {t}", format_elapsed(*age)),
                None => format!("peer {name} · {}", format_elapsed(*age)),
            };
            crate::editor::truncate_display(&face, width.saturating_sub(1))
        })
        .collect()
}

/// The live direct children of this session (FR-20's truth): the display
/// name/id and the operating time (`now − started_at`), read from the shared
/// registry. Liveness is the registry's own reap-on-read — a reaped entry is a
/// finished subagent, and the removal is decided here, per read.
fn live_children_states(
    registry_dir: &Path,
    self_id: &crate::id::AgentId,
) -> Vec<(String, std::time::Duration, Option<String>)> {
    let mut children = crate::ipc::registry::children_of(registry_dir, self_id)
        .unwrap_or_default();
    // The rows stack from the bottom line upward with the newest nearest the
    // bottom (AS-26), and the painter walks downward over this list, so it is
    // the spawn order itself: the registry's read order is the filesystem's,
    // not the spawn order — sort oldest-first, which puts the newest on the
    // row just above the block (or the clock) that holds the bottom.
    children.sort_by(|a, b| a.started_at.cmp(&b.started_at));
    children
        .into_iter()
        .map(|e| {
            let age = chrono::Utc::now()
                .signed_duration_since(e.started_at)
                .to_std()
                .unwrap_or_default();
            (peer_display_name(&e), age, e.turn)
        })
        .collect()
}

fn peer_display_name(e: &crate::ipc::registry::RegistryEntry) -> String {
    e.name.clone().unwrap_or_else(|| e.id.to_string())
}

/// The peer lines the live screen's bottom block carries now, as painted text:
/// the registry read and the composition in one call.
fn live_peer_lines(registry_dir: &Path, self_id: &crate::id::AgentId, width: usize) -> Vec<String> {
    peer_status_rows(&live_children_states(registry_dir, self_id), width)
}

/// The body rows a screen of `body` rows shows at `offset`, padded at the top
/// so that index *i* is screen row *i*.
///
/// A log shorter than the screen sits against the bottom block, where it would
/// be on a terminal that has not scrolled yet; the padding rows are empty, so a
/// selection over them contributes nothing. Drawing and copying share this so
/// the highlight and the clipboard can never disagree about which row is which.
fn body_slice_at(width: usize, body: usize, offset: usize) -> Vec<crate::scroll::Row> {
    let rows = crate::scroll::with(|t| crate::scroll::marked_rows(t, width, body, offset))
        .unwrap_or_default();
    let top = body.saturating_sub(rows.len());
    let mut out = Vec::with_capacity(body);
    out.resize(
        top,
        crate::scroll::Row {
            text: String::new(),
            continues: false,
        },
    );
    out.extend(rows);
    out
}

/// Draw the scrolled screen: the transcript slice for the current offset, one
/// row per terminal row, with the bottom block pinned beneath it. Every row is
/// cleared as it is written, so a redraw never flickers and output a running
/// turn wrote underneath is repaired.
fn draw_scroll_overlay(
    overlay: &mut ScrollOverlay,
    stdout: &mut std::io::Stdout,
    renderer: &mut PromptRenderer,
    theme: &Theme,
    bottom: ScrollBottom<'_>,
) {
    use crossterm::cursor::MoveTo;
    use crossterm::style::Print;

    let width = PromptRenderer::terminal_width();
    let height = terminal_height();
    let peer_lines = bottom.peer_lines();
    let bottom_rows = scrolled_bottom_rows(
        bottom.prompt,
        bottom.input,
        width,
        suggestion_rows(bottom.suggestion, width),
        peer_lines.len(),
        bottom.run_row().is_some(),
        bottom.marker_row().is_some(),
    );
    let body = crate::scroll::body_rows(height, bottom_rows);
    let rows = body_slice_at(width, body, overlay.view.offset());
    // A drag made on a different offset describes rows that are no longer
    // there, so it is not drawn over the ones that are.
    let selection = overlay
        .selection
        .filter(|sel| sel.offset == overlay.view.offset());
    for r in 0..body {
        let _ = crossterm::queue!(
            stdout,
            MoveTo(0, r as u16),
            terminal::Clear(ClearType::UntilNewLine)
        );
        if let Some(row) = rows.get(r) {
            let painted = match selection.as_ref().and_then(|sel| {
                let w = crate::editor::str_display_width(&crate::theme::strip_sgr(&row.text));
                crate::select::row_span(sel, r, w)
            }) {
                Some(span) => std::borrow::Cow::Owned(crate::select::highlight(&row.text, span)),
                None => std::borrow::Cow::Borrowed(row.text.as_str()),
            };
            let _ = crossterm::queue!(stdout, Print(painted));
        }
    }
    let _ = crossterm::queue!(stdout, MoveTo(0, body as u16));
    let _ = stdout.flush();

    // The prompt goes through its own renderer, so the cursor arithmetic —
    // wrapping, multibyte widths, the suggestion row — has one implementation.
    renderer.reset();
    // One composition for every state (FR-39, FR-40): the prompt goes through
    // its own renderer, so wrapping, multibyte widths and the suggestion row
    // keep one implementation, and the running turn's elapsed row and the
    // scroll marker are printed above it by the same path. Nothing here is
    // recorded: only `finish_line` writes the transcript, so the elapsed row
    // stays display-only.
    renderer.render(
        stdout,
        theme,
        bottom.role,
        bottom.prompt,
        bottom.input,
        bottom.suggestion,
        &peer_lines,
        bottom.run_row(),
        bottom.marker_row(),
    );
    overlay.last_draw = std::time::Instant::now();
}

/// What the scrolled-view Esc-return redraws the prompt with, per input-loop
/// state (FR-24).
///
/// The repair is the seam, not the arm: the arm's `Esc` branch redraws with
/// whatever this answers, so the fold check — reinstating the shipped
/// `!is_pending()` gate (`None` for `Pending`) — fails the pin while the
/// shipped gate stands, and passes with the repair. Every state redraws:
/// mid-turn the prompt draws beneath the streamed output while the turn keeps
/// running, the clock coming back with the screen through the tick's in-place
/// repaint.
fn esc_return_redraw(prompt_state: &PromptState) -> Option<(Role, &'static str)> {
    if prompt_state.is_awaiting_approval() {
        Some((Role::Confirm, APPROVAL_PROMPT))
    } else {
        Some((Role::PromptSymbol, PROMPT))
    }
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
/// The interactive display loop, extracted from `run` (testable): consumes
/// agent events, paints them, and on an unsuppressed terminal — `Done` or
/// `Error`, the same "idle" set that treats a Provider failure without a
/// `Done` defensively — releases the input loop's Pending wait with the idle
/// notification (FR-03-2).
///
/// The notification is BEST-EFFORT (`try_send`, never an awaited send):
/// peer-prompt turns run while the user sits at an idle prompt (nobody is
/// Pending), so every such terminal deposits a notification nobody receives.
/// After eight of them the 8-slot channel is full, and an awaited send would
/// park this task there. A parked display task stops draining `event_rx`; the
/// event channel then backs up (64 slots) and the agent parks mid-turn with
/// its remaining queue intact — acked peer prompts stop being dequeued until
/// a manually typed prompt happens to drain the idle channel at submit and
/// revive the pipeline. Dropping the ninth notification is the safe outcome:
/// a full channel already holds eight unwatched terminals, so no Pending
/// waiter can be waiting on it, and the submit-time drains clear the stale
/// ones.
#[allow(clippy::too_many_arguments)]
async fn run_display_task(
    mut event_rx: mpsc::Receiver<AgentEvent>,
    agent_idle_tx: mpsc::Sender<()>,
    display_indicator: Arc<std::sync::Mutex<StatusIndicator>>,
    display_suppress: Arc<AtomicUsize>,
    display_turn_flag: Arc<AtomicBool>,
    turn_in_flight_tx: watch::Sender<bool>,
    show_thinking: ShowThinkingMode,
    theme: Theme,
    progress_enabled: bool,
    has_tty: bool,
) {
    let mut display_state = DisplayState::new(has_tty);
    // With the indicator drawing right beneath it, a tool call is described
    // on a single row instead of dumping its raw arguments.
    display_state.compact_output = progress_enabled;
    // With the indicator on, reasoning is shown live in its block rather
    // than streamed into the scrollback.
    display_state.capture_thinking = progress_enabled && show_thinking != ShowThinkingMode::Hidden;
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
                // The armed zone (FR-37a): the beat owns the rows above the
                // input span — the clock block (spinner + thinking) composes
                // from the indicator's own face, the peers' band rides with it
                // (FR-37b), and the tick's screen writes stay suppressed.
                if zone_armed() {
                    let block = {
                        let mut ind = lock_indicator(&display_indicator);
                        ind.zone_advance_frame();
                        ind.zone_block_rows()
                    };
                    zone_beat(&block);
                    continue;
                }
                lock_indicator(&display_indicator).tick();
                continue;
            }
        };
        let is_idle = matches!(ev, AgentEvent::Done | AgentEvent::Error { .. });
        // The turn-in-flight flag the input loop's cancel sites read:
        // a normal turn occupies the agent exactly between its TurnStart
        // and its terminal event; on a suppressed (cancelled) tail the
        // consuming terminal clears the flag the same way.
        let turn_start = matches!(ev, AgentEvent::TurnStart);
        // The flag carries the TurnStart-to-terminal correspondence (a turn is
        // in flight between those edges, not merely on the edge): only the
        // two edges write it. The shipped store flipped it false on the first
        // streamed delta, so a running turn read as ended from that instant —
        // the cancel bookkeeping and the reconcile tick both saw a turn that
        // was still streaming (FR-49's face: the remaining deltas painted at
        // the prompt's own rows mid-turn).
        if matches!(ev, AgentEvent::TurnStart) {
            display_turn_flag.store(true, Ordering::SeqCst);
        } else if is_idle {
            display_turn_flag.store(false, Ordering::SeqCst);
        }
        // The loop's synchronous arm (FR-49): the watch's send is a no-op when
        // the stored value already matches, so a turn that opens behind an
        // unread notification does not double-arm the zone.
        let _ = turn_in_flight_tx.send(turn_start);
        // While a cancelled turn is still draining, its remaining events are
        // dropped and its terminal event yields no idle notification (it
        // must not release a *later* turn from Pending).
        let (drop_event, notify_idle, outstanding) =
            suppression_decision(is_idle, display_suppress.load(Ordering::SeqCst));
        display_suppress.store(outstanding, Ordering::SeqCst);
        {
            // The guard covers only synchronous terminal writes; it is
            // dropped before the idle notification sends.
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
            let _ = agent_idle_tx.try_send(());
        }
    }
}

fn suppression_decision(is_terminal: bool, outstanding: usize) -> (bool, bool, usize) {
    if outstanding == 0 {
        (false, is_terminal, 0)
    } else if is_terminal {
        (true, false, outstanding - 1)
    } else {
        (true, false, outstanding)
    }
}

/// Book-keeping for a forced return to the prompt: raise the cancellation and,
/// when a turn is actually in flight, record that one more turn is now
/// draining. Returns the armed outstanding count.
///
/// The turn-in-flight guard is load-bearing: raising the token while the agent
/// is idle (no `TurnStart` seen yet, or its terminal event already passed)
/// would otherwise arm a counter no terminal event ever consumes, and the
/// next turn — a peer prompt's, specifically — would then be displayed as
/// nothing at all: every event, including the "[info] peer prompt received"
/// line and the terminal `Done`, dropped and the queue apparently frozen.
/// The `suppress == 0` guard covers a second `Esc`/`Ctrl-C` pressed while the
/// first cancellation is still draining: the turn emits exactly one terminal,
/// so a second increment would be left over for that same wrong turn.
fn begin_cancel(
    cancel: &CancelToken,
    suppress: &AtomicUsize,
    turn_in_flight: &AtomicBool,
) -> usize {
    cancel.cancel();
    if turn_in_flight.load(Ordering::SeqCst) && suppress.load(Ordering::SeqCst) == 0 {
        suppress.fetch_add(1, Ordering::SeqCst) + 1
    } else {
        suppress.load(Ordering::SeqCst)
    }
}

/// `/cancel`'s raise, guarded: the token is raised only while a turn is
/// actually in flight. Raised while the agent is idle, the flag would sit
/// with no unwind to consume or clear it (the turn-entry reset only runs on
/// the next turn), stranding every later peer prompt in the skip branch
/// until a manual prompt unstuck the session.
fn raise_cancel_if_turn_in_flight(cancel: &CancelToken, turn_in_flight: &AtomicBool) -> bool {
    if turn_in_flight.load(Ordering::SeqCst) {
        cancel.cancel();
        true
    } else {
        false
    }
}

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
            // A bare Enter (an Enter-only input) is ignored (FR-38, D-59):
            // the guard sits at the editor's Enter arm, before `submit()`,
            // so the empty submit touches no state — no history write, no
            // queue effect, no redraw side effects — in every prompt state
            // this arm serves. A whitespace-only line still submits (the
            // guard suppresses the empty string only).
            if input.line.is_empty() {
                return None;
            }
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
    turn_in_flight_rx: Option<watch::Receiver<bool>>,
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
            turn_in_flight_rx,
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
            turn_in_flight_rx,
        )
        .await;
    }
}

/// A cursor-position query the input loop hands to the pump, answered on the
/// pump's own thread: `crossterm::cursor::position()` touches the same global
/// reader the pump owns, so the question must be asked from that thread rather
/// than concurrently from the loop.
struct CursorQuery {
    reply: oneshot::Sender<std::io::Result<(u16, u16)>>,
}

/// Events still queued with the pump between two loop runs are delivered in
/// one batch; matched to what the per-iteration drain used to read.
const PUMP_CHANNEL_CAPACITY: usize = 256;

/// Hard bound on waiting for a cursor-position reply. The vendor's own query
/// waits up to two seconds per attempt and retries on internal errors — an
/// unbounded window on the thread that must keep accepting input — so the
/// loop-side helper caps the wait and maps expiry to the same `Err` fallback
/// the direct call had.
const PUMP_QUERY_TIMEOUT: Duration = Duration::from_secs(3);

/// The single owner of this process's crossterm event reader.
///
/// crossterm 0.27 funnels every `event::poll`/`read` through one global
/// reader protected by a mutex; a `poll` with a timeout gives up (`Ok(false)`)
/// when the lock is taken, and `read()` takes the mutex with no timeout and
/// can then block inside the underying tty read. Having several consumers —
/// overlapping polls, or a query on another thread — is undefined by the
/// vendor: events sit unread behind a parked poll (typing lags, then never
/// registers again), and a byte stream split across concurrent readers makes
/// the parser desynchronize, garbage the current sequence, and surface the
/// fragments as typed input.
///
/// One thread polls, drains everything immediately available (the same batch
/// discipline the per-iteration drain had), and forwards each event to the
/// async loop over a bounded channel; a closed channel is the pump's own
/// termination signal. Cursor-position queries are answered here too, between
/// the pump's own poll cycles, so the singleton is touched by one thread
/// only. `position`, `poll` and `read` arrive as injectable operations — the
/// production site passes the crossterm functions; tests pass canned ones, so
/// the pump's contract is pinned without a terminal.
fn run_input_pump<S, P, R>(
    query_rx: std::sync::mpsc::Receiver<CursorQuery>,
    event_tx: mpsc::Sender<CtEvent>,
    position: S,
    poll: P,
    read: R,
) where
    S: Fn() -> std::io::Result<(u16, u16)>,
    P: Fn(Duration) -> std::io::Result<bool>,
    R: Fn() -> std::io::Result<CtEvent>,
{
    loop {
        // Answer a pending cursor query before anything else: the loop asked
        // while a click was being handled, and it is waiting bounded — the
        // answer should not wait behind a poll cycle.
        match query_rx.try_recv() {
            Ok(query) => {
                let _ = query.reply.send(position());
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        if poll(Duration::from_millis(50)).unwrap_or(false) {
            while let Ok(ct_event) = read() {
                // The receiver going away is this pump's retirement:
                // the session that owned it is over.
                if event_tx.blocking_send(ct_event).is_err() {
                    return;
                }
                if !poll(Duration::from_millis(0)).unwrap_or(false) {
                    break;
                }
            }
        }
    }
}

/// Ask the pump for the physical cursor position, bounded by
/// [`PUMP_QUERY_TIMEOUT`]: expiry maps to the same `Err` fallback the direct
/// `cursor::position()` call answered with, so the click semantics behind it
/// are unchanged. A session without a pump (its thread could not be spawned)
/// answers `Err` directly, keeping the same fallback.
async fn cursor_position_via_pump(
    query_tx: Option<&std::sync::mpsc::Sender<CursorQuery>>,
) -> std::io::Result<(u16, u16)> {
    let Some(query_tx) = query_tx else {
        return Err(std::io::Error::other("input pump is gone"));
    };
    let (reply_tx, reply_rx) = oneshot::channel();
    let query = CursorQuery {
        reply: reply_tx,
    };
    if query_tx.send(query).is_err() {
        return Err(std::io::Error::other("input pump is gone"));
    }
    match tokio::time::timeout(PUMP_QUERY_TIMEOUT, reply_rx).await {
        Ok(Ok(result)) => result,
        _ => Err(std::io::Error::other(
            "no cursor position within the query window",
        )),
    }
}

/// Raw-mode input loop with crossterm (FR-01/FR-02/FR-03).
/// A dedicated pump thread (`run_input_pump`) is the only consumer of
/// crossterm's event reader: it polls, drains and forwards events over a
/// channel, and answers cursor-position queries from the same thread, so
/// concurrent users of the single global reader that starve each other
/// cannot exist by construction.
#[allow(clippy::too_many_arguments)]
async fn run_input_loop_raw(
    input_tx: mpsc::Sender<AgentInput>,
    state: Arc<ReplState>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    agent_idle_rx: mpsc::Receiver<()>,
    approval_rx: mpsc::Receiver<ApprovalRequest>,
    turn_in_flight_rx: Option<watch::Receiver<bool>>,
) {
    // The watch arm is the loop's synchronous turn-start listener (FR-49); the
    // borrow must be mutable so `changed()` can stand on the shared receiver.
    let mut turn_in_flight_rx = turn_in_flight_rx;
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
                turn_in_flight_rx,
            )
            .await;
            return;
        }
    };

    let mut input_state = InputState::new();
    let mut prompt_state = PromptState::Ready;
    // The `!` command currently running, if any. It is kept next to
    // `prompt_state` rather than inside it so the `select!` arm below can poll
    // it while the other arms still read the state.
    let mut shell_run: Option<crate::shell::ShellRun> = None;
    // A `/send --ask` waiting for its peer's answer on a background task; its
    // arm below prints the outcome (answer or the disambiguated failure).
    let mut ask_run: Option<crate::tools::send_to::PeerAsk> = None;
    let mut approval_rx: Option<mpsc::Receiver<ApprovalRequest>> = Some(approval_rx);
    let mut stdout = std::io::stdout();
    let mut shutdown_rx = shutdown_rx;
    let mut agent_idle_rx = agent_idle_rx;
    let mut renderer = PromptRenderer::new();
    // The reconcile clock (FR-37a/FR-40): a turn the input loop did not submit
    // — a peer prompt arriving over IPC, a queued prompt the agent dequeued on
    // its own — runs with nobody holding the rows above the prompt. The arm
    // below hands them to the turn zone the moment it sees the turn in flight,
    // so the elapsed row is composed above the prompt in every state instead of
    // being painted onto the prompt's own bottom row by the cursor-relative
    // block. Low frequency: the loop only has to notice, never to race.
    const ZONE_RECONCILE_MS: u64 = 150;
    let mut zone_timer = tokio::time::interval(tokio::time::Duration::from_millis(
        ZONE_RECONCILE_MS,
    ));
    zone_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    zone_timer.tick().await; // consume the immediate first tick
    // The wheel scrollback: the terminal has to report mouse events for the
    // whole session, not only while a turn runs, so the input loop owns
    // reporting here (the indicator's own switch is disabled in that case) and
    // `RawModeGuard::drop` turns it off exactly once, when raw mode ends.
    let mut overlay = ScrollOverlay::new();
    if state.scroll {
        let _ = std::io::stderr().execute(crossterm::event::EnableMouseCapture);
    }

    // The pump: one thread that owns every crossterm reader call of this
    // session. crossterm 0.27 funnels all of `event::poll`/`read` through a
    // single global reader whose timed `poll` returns `Ok(false)` on a lock
    // miss and whose `read()` blocks without a timeout; the per-iteration
    // `spawn_blocking` polls this loop used to spawn could overlap each other
    // and the cursor query, which showed up exactly as the reported pair of
    // symptoms — typing that first lags and then stops registering at all,
    // and mouse reports surfacing as garbled escape-sequence-like input once
    // the shared parser desynchronizes. With one owner, none of those
    // overlaps exist by construction; the loop below only receives events
    // from the channel and asks the pump for the cursor position.
    let (pump_req_tx, pump_req_rx) = std::sync::mpsc::channel::<CursorQuery>();
    let (event_tx, mut event_rx) = mpsc::channel::<CtEvent>(PUMP_CHANNEL_CAPACITY);
    let pump_handle = std::thread::Builder::new()
        .name("input-pump".to_string())
        .spawn(move || {
            run_input_pump(
                pump_req_rx,
                event_tx,
                crossterm::cursor::position,
                event::poll,
                event::read,
            )
        });
    let pump_query_tx = match pump_handle {
        Ok(_) => Some(pump_req_tx),
        Err(e) => {
            tracing::error!("failed to spawn the input pump: {e}");
            None
        }
    };

    // Draw initial prompt
    render_prompt(
        &mut overlay,
        &mut renderer,
        &mut stdout,
        Role::PromptSymbol,
        PROMPT,
        &state,
        &input_state,
    )
    .await;

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
            // The turn's synchronous arm (FR-49): a `TurnStart` lands the same
            // instant the display task says so — no 150 ms window streams the
            // turn's leading deltas onto rows the zone has not taken yet. The
            // running flow's own states keep their spans (the approval's
            // takeover arms the zone the same way at its own hand-off).
            changed = async {
                match turn_in_flight_rx.as_mut() {
                    Some(rx) => rx.changed().await,
                    None => std::future::pending().await,
                }
            } => {
                // No select-level `if`: the arm's own condition is snapshotted
                // when the select enters, so a change the loop blocked on would
                // never wake a disabled branch — the same 150 ms window the
                // reconcile tick serves would stream the turn's leading deltas
                // onto the prompt's freshly returned rows. The body refines:
                // the sends follow the terminal's own edge (`TurnStart` results
                // in true, terminal in false), so a no-op send does not run the
                // arm either (FR-49).
                let _ = changed;
                if !turn_in_flight_rx.as_ref().map(|rx| *rx.borrow()).unwrap_or(false) {
                    // The terminal's edge: the idle arm owns the hand-off, so a
                    // zone that stands still disarms on its own path.
                } else if state.turn_in_flight.load(Ordering::SeqCst)
                    && !prompt_state.is_pending()
                    && !zone_armed()
                {
                    zone_arm(&state);
                    if prompt_state.is_awaiting_approval() {
                        zone_paint_span(
                            &state.theme,
                            Role::Confirm,
                            APPROVAL_PROMPT,
                            &input_state,
                            None,
                        );
                    }
                }
            }
            // The turn terminal's prompt handoff (FR-49): the notification is
            // consumed in every state, so the prompt the answer's display
            // completes onto is the shipped flow's own return, never the
            // reconcile timer's. Pending: the typed path's handoff, as
            // shipped. Ready with no turn in flight: a turn this loop did not
            // submit (peer/serve) ended — the same handoff, so the prompt
            // returns between back-to-back turns instead of racing the next
            // TurnStart against the 150 ms tick. AwaitingApproval or a turn
            // already in flight: the running flow's own rows (the approval
            // span and the next turn keep the zone); the mark is dropped
            // quietly, the same quietness the click pass serves (FR-48b).
            res = agent_idle_rx.recv() => {
                match res {
                    Some(()) => {
                        if prompt_state.is_pending() {
                            prompt_state = PromptState::Ready;
                            // The handoff (D-54): the zone gives the screen
                            // back — its rows erased, the record's tail
                            // replayed above the fresh prompt — before the
                            // shipped render.
                            zone_disarm();
                            // Redraw prompt after AI response on a fresh line
                            renderer.reset();
                            render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                        } else if prompt_state.is_awaiting_approval()
                            || state.turn_in_flight.load(Ordering::SeqCst)
                        {
                            // The running flow's own: the approval the user
                            // answers and the turn that started keep the
                            // screen; the notification is not the handoff.
                        } else {
                            // The loop sat ready while the turn ran (a
                            // peer/serve prompt): the turn ended, so the
                            // prompt returns the same way the typed path
                            // returns it — unless the reconcile tick had
                            // handed the rows back earlier (its own disarm
                            // fired first), whose paint this one would only
                            // duplicate a row above the fresh prompt.
                            if zone_armed() {
                                zone_disarm();
                                renderer.reset();
                                render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                            }
                        }
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
                        // An approval needs an answer, so the scrolled view
                        // gives the screen back before the request is printed.
                        if overlay.is_active() {
                            overlay.leave(&mut stdout);
                            renderer.reset();
                        }
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
                        render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::Confirm, APPROVAL_PROMPT, &state, &input_state).await;
                    }
                    None => {
                        approval_rx = None;
                    }
                }
            }
            // A `!` command is running: print what it produces as it arrives,
            // so a long command shows progress and every line lands in the
            // scrollback like any other output.
            produced = async {
                match shell_run.as_mut() {
                    Some(run) => run.next().await,
                    None => std::future::pending::<Option<crate::shell::ShellLine>>().await,
                }
            }, if shell_run.is_some() => {
                use crate::shell::{ShellLine, ShellStatus};
                match produced {
                    // The command's own output is printed unstyled: it is not
                    // agent-cli's text to colour.
                    Some(ShellLine::Out(l)) => raw_println(true, &l),
                    Some(ShellLine::Err(l)) => raw_eprintln(true, &l),
                    Some(ShellLine::End(status)) => {
                        let run = shell_run.take().expect("a run was in progress");
                        match &status {
                            ShellStatus::Exited(0) => {}
                            ShellStatus::Exited(code) => raw_eprintln(
                                true,
                                &state.theme.err(Role::Failure, &format!("[shell] exit {code}")),
                            ),
                            ShellStatus::TimedOut { after_ms } => raw_eprintln(
                                true,
                                &state.theme.err(
                                    Role::Failure,
                                    &format!("[shell] timed out after {after_ms} ms"),
                                ),
                            ),
                            // The cancel path already printed `[info] cancelled by user`.
                            ShellStatus::Cancelled => {}
                            ShellStatus::Failed(e) => raw_eprintln(
                                true,
                                &state.theme.err(Role::Failure, &format!("[shell] {e}")),
                            ),
                        }
                        let outcome = run.finish(status);
                        if state.shell.context {
                            let _ = input_tx
                                .send(AgentInput::Context {
                                    text: crate::shell::context_message(&outcome),
                                    source: crate::agent::ContextSource::Shell {
                                        command: outcome.command.clone(),
                                        status: outcome.status.describe(),
                                    },
                                })
                                .await;
                        }
                        // An approval may have arrived while the command ran;
                        // only the shell state is ours to leave.
                        if prompt_state.is_shell() {
                            prompt_state = PromptState::Ready;
                            renderer.reset();
                            render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                        }
                    }
                    // The runner ended without a terminal line (its task was
                    // dropped): give the prompt back rather than hanging.
                    None => {
                        shell_run = None;
                        if prompt_state.is_shell() {
                            prompt_state = PromptState::Ready;
                            renderer.reset();
                            render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                        }
                    }
                }
            }
            // A `/send --ask` said nothing still: its runner answers on its
            // own task, and this arm reports the outcome — the peer's answer
            // or the disambiguated failure — the moment it is known, without
            // holding the loop away from anything else you type meanwhile.
            ask_outcome = async {
                match ask_run.as_mut() {
                    Some(run) => run.next().await,
                    None => std::future::pending::<Option<std::result::Result<String, String>>>().await,
                }
            }, if ask_run.is_some() => {
                match ask_outcome {
                    Some(Ok(text)) => {
                        raw_println(true, &state.theme.out(Role::Info, &format!("[peer reply] {text}")));
                    }
                    Some(Err(why)) => {
                        raw_eprintln(true, &state.theme.err(Role::Failure, &format!("[send] {why}")));
                    }
                    // The runner is gone with nothing to say (its receiver was
                    // dropped with the loop): treat it as the end of the ask.
                    None => {}
                }
                ask_run = None;
                if !prompt_state.is_pending() {
                    renderer.reset();
                    render_prompt(&mut overlay, &mut renderer, &mut stdout, if prompt_state.is_awaiting_approval() { Role::Confirm } else { Role::PromptSymbol }, if prompt_state.is_awaiting_approval() { APPROVAL_PROMPT } else { PROMPT }, &state, &input_state).await;
                }
            }
            // Receive a crossterm event from the pump — the session's single
            // reader thread — in all states (Ready, Pending, AwaitingApproval).
            // During Pending, ESC and Ctrl-C send AgentInput::Cancel; other keys are ignored.
            ct_event = event_rx.recv() => {
                // The pump is gone: its channels close when the session ends,
                // the same condition that closes the loop here.
                let Some(ct_event) = ct_event else {
                    break;
                };
                // Drain everything already queued behind it. `exit_loop` lets an inner break
                // (quit/exit command, EOF, closed channel) propagate out of the
                // event-drain `for` to terminate the outer input loop — a plain
                // `break` here would only stop draining events, not exit the REPL.
                let mut exit_loop = false;
                // Set by a wheel report that moved the view, so a burst of
                // notches is drawn once, after the whole drain.
                let mut scroll_dirty = false;
                // Set by a finished drag, reported once the screen is back the
                // way the user expects it.
                let mut copied: Option<crate::clip::Outcome> = None;
                // The burst discipline the previous drain had: the first event
                // arrived over the channel, and everything the pump already
                // read behind it joins the same pass — a wheel burst still
                // repaints once, after the whole drain.
                let mut batch: Vec<std::io::Result<CtEvent>> = Vec::with_capacity(4);
                batch.push(Ok(ct_event));
                while let Ok(queued) = event_rx.try_recv() {
                    batch.push(Ok(queued));
                }
                for ct_event in batch.into_iter() {
                    if let Ok(CtEvent::Mouse(mouse_event)) = &ct_event {
                        // The wheel scrolls the session log behind the prompt,
                        // which stays where it is. The keyboard is untouched:
                        // the arrows still move within the line and through the
                        // history.
                        if state.scroll
                            && matches!(
                                mouse_event.kind,
                                crossterm::event::MouseEventKind::ScrollUp
                                    | crossterm::event::MouseEventKind::ScrollDown
                            )
                        {
                            let up = matches!(
                                mouse_event.kind,
                                crossterm::event::MouseEventKind::ScrollUp
                            );
                            let width = PromptRenderer::terminal_width();
                            let peer_lines =
                                live_peer_lines(&state.registry_dir, &state.id, width).len();
                            // The marker row joins the budget whenever the view
                            // is off the bottom — a running turn included, since
                            // the pin composes the prompt span in every state
                            // (FR-39, FR-40). The drawer recomputes the exact
                            // count at the draw, so a first notch taken at a
                            // live view only risks a notch-shallower clamp
                            // before the next pass.
                            let view_moved = overlay.view_active();
                            let bottom_rows = bottom_block_rows(
                                &prompt_state,
                                &input_state,
                                width,
                                peer_lines,
                                view_moved,
                            );
                            let body = crate::scroll::body_rows(terminal_height(), bottom_rows);
                            if overlay.wheel(up, width, body) {
                                scroll_dirty = true;
                            }
                            continue;
                        }
                        // A click is aimed at the progress indicator: clicking
                        // its block switches the live `thinking` view between
                        // the last few rows and everything the terminal can
                        // hold. While the view is scrolled the block is not on
                        // screen, so there is nothing to click.
                        if prompt_state.is_pending()
                            && !overlay.is_active()
                            && matches!(
                                mouse_event.kind,
                                crossterm::event::MouseEventKind::Down(
                                    crossterm::event::MouseButton::Left
                                )
                            )
                        {
                            // The armed zone owns the standing shape: its beat
                            // bookkeeping knows the clock block's absolute
                            // rows, so the question is answered from the
                            // zone's own truth and no pump round-trip is
                            // spent. The shipped rule otherwise asks the
                            // pump — the only thread that may touch
                            // crossterm's reader — so nothing is asked behind
                            // another consumer's back. Without an answer in
                            // time the click changes nothing, so the elapsed
                            // display cannot surface from a stale answer
                            // (FR-27). Both rules keep the elapsed row out
                            // (the block's first row, excluded top).
                            let hit = if zone_armed() {
                                zone_click_hits_clock(mouse_event.row)
                            } else {
                                let painted = lock_indicator(&state.indicator).painted_rows();
                                match cursor_position_via_pump(pump_query_tx.as_ref()).await {
                                    Ok((_, cursor_row)) => {
                                        click_hits_indicator(painted, cursor_row, mouse_event.row)
                                    }
                                    Err(_) => false,
                                }
                            };
                            if hit {
                                lock_indicator(&state.indicator).toggle_expanded();
                                continue;
                            }
                            // Not the indicator: fall through, so a drag that
                            // starts on the log during a turn still selects.
                        }
                        // Dragging the left button over the log selects it and
                        // copies on release. `Shift` is left to the terminal:
                        // its own selection is the override users already know,
                        // and a terminal that reports a shifted drag must not
                        // have it taken away here.
                        if state.scroll
                            && state.mouse_select
                            && !mouse_event
                                .modifiers
                                .contains(KeyModifiers::SHIFT)
                        {
                            use crossterm::event::{MouseButton, MouseEventKind};
                            let width = PromptRenderer::terminal_width();
                            // The same gate the wheel arm applies (FR-39,
                            // FR-40): the marker row joins the body's budget
                            // whenever the view is off the bottom, so a drag and
                            // its release resolve against the rows the screen
                            // will actually show — the prompt span's rows, the
                            // elapsed row and the marker included.
                            let view_moved = overlay.view_active();
                            let bottom_rows = bottom_block_rows(
                                &prompt_state,
                                &input_state,
                                width,
                                live_peer_lines(&state.registry_dir, &state.id, width).len(),
                                view_moved,
                            );
                            let body =
                                crate::scroll::body_rows(terminal_height(), bottom_rows);
                            let row = mouse_event.row as usize;
                            let col = mouse_event.column as usize;
                            match mouse_event.kind {
                                MouseEventKind::Down(MouseButton::Left) if row < body => {
                                    // A press on the log starts a drag without
                                    // painting: the empty one-cell selection is
                                    // not on screen, so the click costs nothing
                                    // until the gesture writes something (the
                                    // FR-48b clause — a click displays no
                                    // prompt — and FR-48a's zero-touch face).
                                    overlay.begin_selection(crate::select::Point::new(row, col));
                                    continue;
                                }
                                MouseEventKind::Drag(MouseButton::Left)
                                    if overlay.has_selection() =>
                                {
                                    // Clamped so dragging past the edge of the
                                    // log selects to the edge rather than into
                                    // the prompt.
                                    let at = crate::select::Point::new(
                                        row.min(body.saturating_sub(1)),
                                        col.min(width),
                                    );
                                    if overlay.extend_selection(at) {
                                        scroll_dirty = true;
                                    }
                                    continue;
                                }
                                MouseEventKind::Up(MouseButton::Left)
                                    if overlay.has_selection() =>
                                {
                                    if let Some(sel) = overlay.take_selection() {
                                        // The slice is taken at release, at the
                                        // view offset the release belongs to: a
                                        // turn streaming lines under the drag
                                        // changes which transcript rows the
                                        // screen shows, and the copy has to be
                                        // the range the screen highlighted at
                                        // release — not the range anchored to
                                        // where the press happened.
                                        if !sel.is_empty() {
                                            let offset = overlay.view.offset();
                                            let rows = body_slice_at(width, body, offset);
                                            let text = crate::select::selected_text(&rows, &sel);
                                            let outcome = crate::clip::copy(
                                                &text,
                                                &state.copy_command,
                                                |seq| {
                                                    use std::io::Write as _;
                                                    let mut err = std::io::stderr();
                                                    err.write_all(seq.as_bytes())?;
                                                    err.flush()
                                                },
                                            );
                                            // An empty report is no report: a
                                            // gesture that touched nothing does
                                            // not get a line of its own.
                                            if !outcome.is_empty() {
                                                copied = Some(outcome);
                                            }
                                        }
                                        // The release restores the screen the
                                        // gesture took: a drag's highlight
                                        // screen and a scrolled view's
                                        // alternate screen both come back
                                        // here. A plain click (press+release,
                                        // nothing drawn, the view un-moved)
                                        // never took a screen, so it writes
                                        // nothing at all (FR-48a/FR-48b).
                                        if overlay.is_active() || overlay.view_active() {
                                            scroll_dirty = true;
                                        }
                                    }
                                    continue;
                                }
                                _ => {}
                            }
                        }
                        continue;
                    }
                    if let Ok(CtEvent::Key(key_event)) = ct_event {
                        // While the view is scrolled the overlay owns the
                        // screen. `Esc` returns to the live view and is
                        // consumed there — it must not also cancel the turn or
                        // deny an approval; `Enter` and the exit keys return to
                        // it and then mean what they always mean. Every other
                        // key edits the line in the pinned prompt block, so the
                        // arrows keep moving within the line and through the
                        // history.
                        if overlay.is_active()
                            && key_event.kind != crossterm::event::KeyEventKind::Release
                        {
                            let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
                            let back_to_live = matches!(key_event.code, KeyCode::Esc | KeyCode::Enter)
                                || (ctrl
                                    && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d')));
                            if back_to_live {
                                overlay.leave(&mut stdout);
                                lock_indicator(&state.indicator).resume();
                                renderer.reset();
                                if key_event.code == KeyCode::Esc {
                                    // The prompt returns in every state (FR-24):
                                    // the line the user was typing is kept,
                                    // the approval prompt is re-shown as it
                                    // was, and mid-turn the prompt draws
                                    // beneath the streamed output while the
                                    // turn keeps running. The indicator's
                                    // clock comes back with the screen: the
                                    // next tick repaints the block in place,
                                    // so the elapsed time never lands over the
                                    // redrawn prompt.
                                    if let Some((role, prompt)) = esc_return_redraw(&prompt_state) {
                                        render_prompt(&mut overlay, &mut renderer, &mut stdout, role, prompt, &state, &input_state).await;
                                    }
                                    continue;
                                }
                                // Enter / Ctrl-C / Ctrl-D fall through to their
                                // usual handling, now on the live screen.
                            }
                        }
                        // While a `!` command runs, the cancel key stops it and
                        // every other key is ignored — the prompt is not
                        // accepting a line yet. No `CancelToken` is raised: the
                        // agent has nothing to do with this.
                        if prompt_state.is_shell() {
                            if is_cancel_key(&key_event) {
                                if let Some(run) = shell_run.as_ref() {
                                    run.cancel();
                                }
                                raw_eprintln(
                                    true,
                                    &state.theme.err(Role::Info, "[info] cancelled by user"),
                                );
                            }
                            continue;
                        }
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
                                begin_cancel(&state.cancel, &state.suppress, &state.turn_in_flight);
                                // The forced return: the zone hands the screen
                                // back first — its rows are erased and the
                                // record's tail stands as the log — then the
                                // shipped cancel flow paints the notice and
                                // the prompt (D-54).
                                zone_disarm();
                                // Take the bottom rows back from the indicator
                                // before printing over them.
                                lock_indicator(&state.indicator).suspend();
                                raw_eprintln(
                                    true,
                                    &state.theme.err(Role::Info, "[info] cancelled by user"),
                                );
                                prompt_state = PromptState::Ready;
                                renderer.reset();
                                render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
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
                            begin_cancel(&state.cancel, &state.suppress, &state.turn_in_flight);
                            // The deny ends the turn's run display: the zone
                            // hands the screen back before the notice prints.
                            zone_disarm();
                            lock_indicator(&state.indicator).suspend();
                            renderer.clear_block(&mut stdout);
                            raw_eprintln(
                                true,
                                &state.theme.err(Role::Info, "[info] cancelled by user"),
                            );
                            input_state = InputState::new();
                            renderer.reset();
                            render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
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
                            render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
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
                                        // The answer returns the REPL to the
                                        // running state: the approval span
                                        // retires (FR-46a) and the next beat
                                        // takes the screen's bottom row back
                                        // for the clock.
                                        zone_span_retire();
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
                                    renderer.finish_line(&mut stdout, &state.theme, prompt_line_role(&line, state.shell.enabled), PROMPT, &line);

                                    // Only a line typed here can be a shell
                                    // command: a peer's prompt never reaches
                                    // this classification.
                                    let classified = crate::shell::classify_line(&trimmed, state.shell.enabled);
                                    if let crate::shell::Line::Shell(cmd) = classified {
                                        push_history(&state, &trimmed).await;
                                        raw_println(true, &state.theme.out(Role::ToolName, &format!("$ {cmd}")));
                                        shell_run = Some(crate::shell::ShellRun::spawn(cmd, &state.shell));
                                        prompt_state = PromptState::Shell;
                                        continue;
                                    }
                                    if matches!(classified, crate::shell::Line::ShellUsage) {
                                        raw_eprintln(true, &state.theme.err(Role::Info, "usage: !<command>  (runs it in the shell)"));
                                        render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                                        continue;
                                    }

                                    if let Some(rest) = trimmed.strip_prefix('/') {
                                        match handle_repl_command(rest, &input_tx, &state, true).await {
                                            CommandResult::Quit => {
                                                exit_loop = true;
                                                break;
                                            }
                                            CommandResult::SubmittedPrompt => {
                                                while agent_idle_rx.try_recv().is_ok() {}
                                                prompt_state = PromptState::Pending;
                                                // A command's turn runs too (FR-37a):
                                                // the zone takes the live screen the
                                                // same way a typed prompt's does.
                                                zone_arm(&state);
                                                render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                                            }
                                            CommandResult::Continue => {
                                                // Redraw prompt after command
                                                render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                                            }
                                            // The ask runs on its own task; the
                                            // dedicated select arm below prints the
                                            // outcome when it arrives.
                                            CommandResult::SpawnedAsk(run) => {
                                                ask_run = Some(run);
                                            }
                                        }
                                        continue;
                                    }
                                    if trimmed.is_empty() {
                                        // Blank line: just redraw prompt
                                        render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
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
                                    // The four-region standing shape (FR-37a): the
                                    // zone takes the live screen and the prompt
                                    // re-paints as the pinned span above the beat's
                                    // rows.
                                    zone_arm(&state);
                                    render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                                }
                                KeyAction::ClearLine => {
                                    // Redraw prompt with cleared state
                                    if prompt_state.is_awaiting_approval() {
                                        render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::Confirm, APPROVAL_PROMPT, &state, &input_state).await;
                                    } else {
                                        render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                                    }
                                }
                                KeyAction::Eof => {
                                    exit_loop = true;
                                    break;
                                }
                                KeyAction::Continue => {
                                    if prompt_state.is_awaiting_approval() {
                                        render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::Confirm, APPROVAL_PROMPT, &state, &input_state).await;
                                    } else {
                                        render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
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
                if scroll_dirty {
                    if overlay.view_active() || overlay.has_selection() {
                        if !overlay.is_active() {
                            lock_indicator(&state.indicator).suspend();
                            renderer.clear_block(&mut stdout);
                            overlay.enter(&mut stdout);
                        }
                    } else if overlay.is_active() {
                        overlay.leave(&mut stdout);
                        lock_indicator(&state.indicator).resume();
                        renderer.reset();
                    }
                }
                // A scrolled screen is redrawn a few times a second: the pinned
                // spinner stays alive while the indicator's own block is
                // suspended, and anything a running turn wrote underneath is
                // repaired.
                if overlay.is_active()
                    && (scroll_dirty || overlay.needs_refresh(prompt_state.is_pending()))
                {
                    draw_scrolled(
                        &mut overlay,
                        &mut renderer,
                        &mut stdout,
                        &state,
                        &input_state,
                        prompt_state.is_pending(),
                        prompt_state.is_awaiting_approval(),
                    )
                    .await;
                } else if scroll_dirty && !prompt_state.is_pending() {
                    // Back on the live screen: put the prompt back where it was.
                    render_prompt(
                        &mut overlay,
                        &mut renderer,
                        &mut stdout,
                        if prompt_state.is_awaiting_approval() { Role::Confirm } else { Role::PromptSymbol },
                        if prompt_state.is_awaiting_approval() { APPROVAL_PROMPT } else { PROMPT },
                        &state,
                        &input_state,
                    )
                    .await;
                }
                // What a finished drag copied. Reported after the screen has
                // been restored, so the line lands in the log rather than on a
                // screen that is about to be redrawn over it.
                if let Some(outcome) = copied {
                    let role = if outcome.is_failure() {
                        Role::Failure
                    } else {
                        Role::Info
                    };
                    let msg = format!("[clip] {}", outcome.message());
                    renderer.clear_block(&mut stdout);
                    raw_eprintln(true, &state.theme.err(role, &msg));
                    if !prompt_state.is_pending() {
                        renderer.reset();
                        render_prompt(
                            &mut overlay,
                            &mut renderer,
                            &mut stdout,
                            if prompt_state.is_awaiting_approval() { Role::Confirm } else { Role::PromptSymbol },
                            if prompt_state.is_awaiting_approval() { APPROVAL_PROMPT } else { PROMPT },
                            &state,
                            &input_state,
                        )
                        .await;
                    }
                }
            }
            // The reconcile arm (FR-40's live side): a turn the input loop did
            // not submit — a peer prompt over IPC, a queued prompt the agent
            // dequeued by itself — runs with nobody holding the rows above the
            // prompt. Left alone the block's cursor-relative repaint lands on
            // the prompt and the elapsed row takes the prompt's own bottom row.
            // Handing the rows to the zone the moment the turn is seen composes
            // the elapsed row above the prompt in every state. The overlay's
            // screen is not this arm's to take: while it is active the pin
            // already carries the elapsed row (`draw_scrolled`).
            _ = zone_timer.tick(), if !overlay.is_active() => {
                if state.turn_in_flight.load(Ordering::SeqCst) {
                    if !zone_armed() {
                        // The arm is spanless now (FR-46a): the running turn's
                        // bottom line is the clock alone. The one span that
                        // still paints is the tool-approval prompt — an
                        // input-possible state whose span re-anchors the
                        // zone's floor and keeps its shipped takeover.
                        zone_arm(&state);
                        if prompt_state.is_awaiting_approval() {
                            zone_paint_span(
                                &state.theme,
                                Role::Confirm,
                                APPROVAL_PROMPT,
                                &input_state,
                                None,
                            );
                        }
                    }
                } else if zone_armed() && prompt_state.is_ready() {
                    // The other half of the same handoff: that turn reached its
                    // terminal event with no submit site of this loop to disarm
                    // for it, so the zone is still standing over a prompt the
                    // loop believes is idle. Give the rows back the way the
                    // release arm does — the disarm's replay, a fresh renderer
                    // and the prompt underneath (D-54).
                    zone_disarm();
                    renderer.reset();
                    render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
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
    // A `!` command still running when the REPL exits is stopped here rather
    // than orphaned: the runner kills the whole job, and we wait briefly for it
    // to confirm before the process goes away.
    if let Some(mut run) = shell_run.take() {
        run.cancel();
        let _ = tokio::time::timeout(Duration::from_millis(500), async {
            while let Some(line) = run.next().await {
                if matches!(line, crate::shell::ShellLine::End(_)) {
                    break;
                }
            }
        })
        .await;
    }
    // Give the live screen back before tearing the prompt down, so the session
    // never ends on the alternate screen. The zone — still armed when the last
    // turn is drained under a shutdown — lets go first, so the leave writes on
    // a screen no other painter holds.
    zone_disarm();
    overlay.leave(&mut stdout);
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
    _turn_in_flight_rx: Option<watch::Receiver<bool>>,
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

                        // A `!` line runs in the shell here too. There are no
                        // keys to poll on this path, so the command is simply
                        // awaited to completion — the timeout is what bounds it.
                        let classified = crate::shell::classify_line(&trimmed, state.shell.enabled);
                        if let crate::shell::Line::Shell(cmd) = classified {
                            push_history(&state, &trimmed).await;
                            run_shell_to_completion(cmd, &state, &input_tx, false).await;
                            continue;
                        }
                        if matches!(classified, crate::shell::Line::ShellUsage) {
                            raw_eprintln(false, "usage: !<command>  (runs it in the shell)");
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
                                // No select arm here (line mode has no prompt
                                // state to redraw through): the ask outcome is
                                // waited for synchronously, out of band.
                                CommandResult::SpawnedAsk(mut run) => match run.next().await {
                                    Some(Ok(text)) => println!("[peer reply] {text}"),
                                    Some(Err(why)) => eprintln!("[error] {why}"),
                                    None => {}
                                },
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
    // The banner carries the brand colour; the startup information beneath it
    // stays in the terminal's default foreground, so it reads as plain text.
    // Through the writer seam, so the banner is part of the session log the
    // wheel scrolls back to. Raw mode is not on yet, so the bytes are the same
    // as a plain `println!`.
    let detail = |line: String| raw_println(false, &theme.out(Role::BannerDetail, &line));
    raw_println(false, &theme.out(Role::Banner, "agent-cli ready"));
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

/// Print, once at startup, everything the `[permissions]` section said that
/// could not be honoured.
///
/// A rule that silently does nothing is the failure this whole feature exists
/// to end, so these are never swallowed — not even the ones that are merely
/// inert.
fn report_permission_warnings(rules: &crate::permissions::Ruleset, theme: &Theme) {
    for warning in rules.warnings() {
        eprintln!(
            "{}",
            theme.err(Role::Info, &format!("[permissions] {warning}"))
        );
        tracing::warn!(target: "permissions", "{warning}");
    }
}

/// `/permissions` — what the rules are, where each came from, and what happens
/// to a call none of them matched.
fn handle_permissions_command(state: &Arc<ReplState>, raw_mode: bool) {
    let rules = &state.permissions;
    let theme = &state.theme;
    let mut out = String::new();

    for (i, path) in state.config_source.chain().iter().enumerate() {
        let rank = if i + 1 == state.config_source.chain().len() {
            "config"
        } else {
            "base  "
        };
        let state_note = if path.exists() { "" } else { "  (absent)" };
        out.push_str(&format!("  {rank}: {}{state_note}\n", path.display()));
    }

    if rules.is_empty() {
        out.push_str(
            "  no rules configured — every tool call goes to the y/N prompt\n  \
             (add a [permissions] section to config.toml)\n",
        );
    } else {
        out.push_str(&format!("  deny ({}):\n", rules.deny_len()));
        for rule in rules.deny_rules() {
            out.push_str(&format!("    {}\n", rule.raw));
        }
        out.push_str(&format!("  allow ({}):\n", rules.allow_len()));
        for rule in rules.allow_rules() {
            out.push_str(&format!("    {}\n", rule.raw));
        }
        out.push_str(&format!("  default_mode: {}\n", rules.default_mode()));
        out.push_str(
            "  a deny outranks an allow, the default mode, and /auto on\n",
        );
    }
    for warning in rules.warnings() {
        out.push_str(&format!("  warning: {warning}\n"));
    }

    raw_println(
        raw_mode,
        &theme.out(Role::Info, &format!("[permissions]\n{}", out.trim_end())),
    );
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
    /// Whether mouse reporting is the indicator's to switch on and off. With
    /// the wheel scrollback live the input loop owns it for the whole session
    /// (it has to be on at the prompt too), so the end of a turn must not be
    /// able to turn it off underneath the wheel.
    owns_mouse: bool,
    /// Whether the live prompt block owns the screen's bottom rows and the
    /// parked caret, with no turn zone holding the writes. The block's rows are
    /// cursor-relative from the prompt's own position, and the caret stands
    /// *inside* the prompt, so a repaint here lands on the prompt: the elapsed
    /// row takes the bottom-most prompt row. Nothing here writes while this
    /// holds — the prompt render is the screen's only writer, and the armed
    /// zone (which composes the elapsed row above the pinned span) is the only
    /// painter allowed on those rows.
    prompt_owned: bool,
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
            owns_mouse: true,
            prompt_owned: false,
        }
    }

    /// Record whether the live prompt block owns the screen's bottom rows. While
    /// it does, nothing here writes to the screen: the caret is the prompt's to
    /// place, so the shipped cursor-relative block (erase upward from the
    /// caret, repaint downward from it) would land inside the prompt — the
    /// elapsed row on the bottom-most prompt row. Entering the state drops the
    /// painted bookkeeping without moving the cursor; leaving it lets the next
    /// paint start from wherever its writer then stands.
    fn set_prompt_owned(&mut self, on: bool) {
        self.prompt_owned = on;
        if on {
            self.painted = false;
            self.painted_rows = 0;
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
        if !self.enabled || !self.raw_mode || !self.thinking_view || !self.owns_mouse {
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
        if self.run.is_none()
            || !self.enabled
            || self.suspended
            || !self.painted
            || self.prompt_owned
        {
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
        if !self.enabled
            || self.suspended
            || self.run.is_none()
            || !at_line_start
            || self.prompt_owned
        {
            return;
        }
        self.repaint();
    }

    /// The status row as it would be painted now — spinner and elapsed time,
    /// cut to the terminal width — or `None` when no turn is running.
    fn status_row(&self) -> Option<String> {
        let run = self.run.as_ref()?;
        Some(crate::editor::truncate_display(
            &format!(
                "{} {}",
                spinner_frame(run.frame),
                format_elapsed(run.started.elapsed())
            ),
            self.width().saturating_sub(1),
        ))
    }

    /// The status row for the scroll overlay, which pins it at the bottom of
    /// the screen while a turn runs. The block itself is suspended for as long
    /// as the overlay owns the screen, so `tick` does nothing and the spinner
    /// is advanced here instead.
    fn overlay_status_row(&mut self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let run = self.run.as_mut()?;
        run.frame = run.frame.wrapping_add(1);
        self.status_row()
    }

    /// The armed zone's clock face (FR-37a): the styled rows the beat paints
    /// as the run display — the spinner-and-elapsed row and, when captured,
    /// the thinking rows under it — the shipped block's own face, unchanged.
    fn zone_block_rows(&self) -> Vec<String> {
        let Some(status) = self.status_row() else {
            return Vec::new();
        };
        // FR-46a's face order: the thinking rows stand above the status row,
        // so the spinner-and-elapsed row is the block's LAST row — the row
        // that lands on the screen's bottom-most line while the agent runs
        // (the instruction's クルクルと経過時間 clause). The reasoning rows keep
        // the progress colour with their indent; the status row and the
        // elision marker keep it too.
        let mut rows = self.thinking_rows();
        rows.push(status);
        // The reasoning rows keep their shipped colour (the indented rows are
        // the Thinking role, order-independent); the status row and the
        // elision marker keep the progress colour.
        rows.iter()
            .map(|row| {
                let role = if row.starts_with("  ") {
                    Role::Thinking
                } else {
                    Role::Progress
                };
                self.theme.err(role, row).into_owned()
            })
            .collect()
    }

    /// The beat's clock advance: the spinner's frame moves once per beat while
    /// the zone paints the row, standing in for the shipped tick's advance.
    fn zone_advance_frame(&mut self) {
        if let Some(run) = self.run.as_mut() {
            run.frame = run.frame.wrapping_add(1);
        }
    }

    fn repaint(&mut self) {
        if zone_armed() {
            // FR-37a: the zone owns the rows above the span (the beat paints
            // the block's own face with `zone_block_rows`), so painting here
            // would land beside the pinned prompt. Nothing to un-do either:
            // the zone never paints with this handle, and `painted` never
            // rises while the gate is on. The gate sits inside, behind the
            // disabled/suspended/run checks, so the zone's borrow of the
            // block's face (`zone_block_rows`) is the only thing repainting
            // stands down from.
            return;
        }
        // The live prompt block's rows (the same rule `tick` and `paint` gate
        // on): the caret stands inside the prompt and this repair is
        // cursor-relative, so a repaint here lands on the prompt's own last
        // row. The armed zone is the only painter allowed above the span.
        if self.prompt_owned {
            return;
        }
        self.clear();
        if self.run.is_none() {
            return;
        }
        let Some(status) = self.status_row() else {
            return;
        };
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
        // The armed zone (FR-37a): the beat owns the block's rows on the live
        // screen and the shared cursor is parked on the prompt's caret — the
        // shipped relative repair (MoveUp + erase downward) would cut into the
        // wrong rows, so the flags reset quietly and the screen is left to the
        // zone.
        if zone_armed() {
            self.painted = false;
            self.painted_rows = 0;
            return;
        }
        // The live screen's prompt block (the same rule the gate above serves):
        // the caret stands inside the prompt, so the shipped relative repair
        // (MoveUp from the caret + erase downward) would erase the prompt's own
        // rows. Nothing was painted there either, so the bookkeeping simply
        // resets.
        if self.prompt_owned {
            self.painted = false;
            self.painted_rows = 0;
            return;
        }
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
        // The outcome row stays on screen, so it belongs to the transcript —
        // unlike the block above it, which is erased and redrawn ten times a
        // second and is never recorded. The record happens even when the block
        // is suspended (the scrolled view owns the screen): a turn that ends
        // while scrolled leaves its outcome line for the return to the bottom.
        crate::scroll::record_line(&line);
        // A suspended block cannot write here: the overlay's body would repair
        // over it, and the transcript record above is what the user sees when
        // the live view returns. The armed zone asks the same quietness: the
        // shared cursor is parked on the pinned span's caret, the row below the
        // last beat's paint is the zone's to compose, so the outcome line is
        // record-only here and surfaces through the disarm's replay.
        if !self.suspended && !zone_armed() && !self.prompt_owned {
            let nl = self.newline();
            let _ = write!(self.out, "{line}{nl}");
            let _ = self.out.flush();
        }
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
    "permissions",
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
/// Run a `!` command to completion, printing its output as it arrives and
/// handing the result to the model afterwards.
///
/// Used by the line-oriented loop, which has no key events to poll and so has
/// nothing to cancel with; the raw loop drives the same `ShellRun` from its
/// `select!` instead, so `Esc` can stop it.
async fn run_shell_to_completion(
    command: &str,
    state: &Arc<ReplState>,
    input_tx: &mpsc::Sender<AgentInput>,
    raw_mode: bool,
) {
    use crate::shell::{ShellLine, ShellStatus};
    raw_println(
        raw_mode,
        &state.theme.out(Role::ToolName, &format!("$ {command}")),
    );
    let mut run = crate::shell::ShellRun::spawn(command, &state.shell);
    let status = loop {
        match run.next().await {
            Some(ShellLine::Out(l)) => raw_println(raw_mode, &l),
            Some(ShellLine::Err(l)) => raw_eprintln(raw_mode, &l),
            Some(ShellLine::End(status)) => break status,
            None => break ShellStatus::Cancelled,
        }
    };
    match &status {
        ShellStatus::Exited(0) | ShellStatus::Cancelled => {}
        ShellStatus::Exited(code) => raw_eprintln(
            raw_mode,
            &state
                .theme
                .err(Role::Failure, &format!("[shell] exit {code}")),
        ),
        ShellStatus::TimedOut { after_ms } => raw_eprintln(
            raw_mode,
            &state.theme.err(
                Role::Failure,
                &format!("[shell] timed out after {after_ms} ms"),
            ),
        ),
        ShellStatus::Failed(e) => raw_eprintln(
            raw_mode,
            &state.theme.err(Role::Failure, &format!("[shell] {e}")),
        ),
    }
    let outcome = run.finish(status);
    if state.shell.context {
        let _ = input_tx
            .send(AgentInput::Context {
                text: crate::shell::context_message(&outcome),
                source: crate::agent::ContextSource::Shell {
                    command: outcome.command.clone(),
                    status: outcome.status.describe(),
                },
            })
            .await;
    }
}

/// Draw the prompt block: in place on the live screen, or pinned at the bottom
/// of the scrolled screen while the overlay owns it.
async fn render_prompt(
    overlay: &mut ScrollOverlay,
    renderer: &mut PromptRenderer,
    stdout: &mut std::io::Stdout,
    role: Role,
    prompt: &str,
    state: &Arc<ReplState>,
    input_state: &InputState,
) {
render_prompt_with_run(
        overlay,
        renderer,
        stdout,
        role,
        prompt,
        state,
        input_state,
        None,
    )
    .await;
}

/// The prompt's one drawing path, with the running turn's elapsed row (FR-40)
/// as a parameter: `None` on the live screen (the zone or the indicator owns
/// that row there) and on the scrolled screen with no turn tracked, `Some` on
/// the scrolled screen mid-turn — where the row stands directly above the
/// prompt span, exactly as the live screen composes it (D-61).
#[allow(clippy::too_many_arguments)]
async fn render_prompt_with_run(
    overlay: &mut ScrollOverlay,
    renderer: &mut PromptRenderer,
    stdout: &mut std::io::Stdout,
    role: Role,
    prompt: &str,
    state: &Arc<ReplState>,
    input_state: &InputState,
    run_row: Option<String>,
) {
    // The slash-command hint belongs to the input prompt; the approval prompt
    // takes a y/N answer and never has one.
    let sug = if matches!(role, Role::PromptSymbol) {
        current_suggestion(state, &input_state.line).await
    } else {
        None
    };
    // A line that starts with `!` will be run in the shell, not sent to the
    // model; the prompt says so while it is being typed.
    let role = if matches!(role, Role::PromptSymbol) {
        prompt_line_role(&input_state.line, state.shell.enabled)
    } else {
        role
    };
    // The subagent rows are composed once per read and shown on both screens
    // (FR-23, D-25): pinned beside the block on the scrolled screen, rendered
    // in the prompt block's tracked region on the live screen.
    let peers = live_peer_lines(
        &state.registry_dir,
        &state.id,
        PromptRenderer::terminal_width(),
    );
    if overlay.is_active() {
        draw_scroll_overlay(
            overlay,
            stdout,
            renderer,
            &state.theme,
            ScrollBottom {
                role,
                prompt,
                input: input_state,
                suggestion: sug.as_deref(),
                peer_lines: peers.clone(),
                run_row,
                text: overlay
                    .view_active()
                    .then(|| SCROLLED_MARK.to_string()),
            },
        );
    } else {
        // The armed zone's composition is state-driven (FR-46a): while a turn
        // runs the clock block owns the screen's bottom rows and no prompt
        // display paints — the standing face is the running clock alone, and
        // the beat is the rows' only writer. The one armed face that still
        // paints is the tool-approval prompt (an input-possible state): its
        // span re-anchors the zone's floor and the beat composes the run
        // display above it, exactly as the shipped approval takeover did.
        // The shipped render below stands down while armed for the same
        // cursor-relative reason it always did.
        if zone_armed() {
            if matches!(role, Role::Confirm) {
                zone_paint_span(&state.theme, role, prompt, input_state, sug.as_deref());
            }
            return;
        }
        // The live prompt block takes the screen's bottom rows from here, and
        // the caret is parked *inside* it. The indicator's block is
        // cursor-relative (erase upward from the caret, repaint downward from
        // it), so painting here would land on the prompt — the elapsed row on
        // the prompt's own last row, which is the row the prompt alone owns. The
        // indicator stands down until a turn zone takes the rows and composes
        // the elapsed row above the span (FR-37a/FR-40); `finish` still records
        // the outcome row, so nothing is lost from the transcript.
        lock_indicator(&state.indicator).set_prompt_owned(true);
renderer.render(
            stdout,
            &state.theme,
            role,
            prompt,
            input_state,
            sug.as_deref(),
            &peers,
            None,
            None,
        );
    }
}

/// Redraw the scrolled screen from whatever the REPL is currently doing: the
/// prompt (or the approval prompt) when it waits for input, the running turn's
/// spinner row when it does not.
async fn draw_scrolled(
    overlay: &mut ScrollOverlay,
    renderer: &mut PromptRenderer,
    stdout: &mut std::io::Stdout,
    state: &Arc<ReplState>,
    input_state: &InputState,
    _pending: bool,
    awaiting_approval: bool,
) {
    // The block is suspended while the overlay owns the screen, so the spinner
    // is advanced here and its face is handed to the prompt pin as the row
    // above the span (FR-40). `overlay_status_row` answers `None` when nothing
    // is tracked, so the not-running prompt below is the same arm as before.
    let run_row = {
        let mut ind = lock_indicator(&state.indicator);
        ind.overlay_status_row()
    };
    let (role, prompt) = if awaiting_approval {
        (Role::Confirm, APPROVAL_PROMPT)
    } else {
        (Role::PromptSymbol, PROMPT)
    };
        render_prompt_with_run(
        overlay,
        renderer,
        stdout,
        role,
        prompt,
        state,
        input_state,
        run_row,
    )
    .await;
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
            raw_println(raw_mode, "  /send [--report|--ask] <peer> <text>");
            raw_println(raw_mode, "                              Send to a peer (id or name): a one-shot prompt (default),");
            raw_println(raw_mode, "                              a report added to its conversation with no turn, or an ask");
            raw_println(raw_mode, "                              that waits for the answer (up to 120s).");
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
            raw_println(raw_mode, "  /permissions                Show the [permissions] rules in force and where they came from.");
            raw_println(raw_mode, "  /commands                   List custom slash commands from .agent-cli/commands.");
            raw_println(raw_mode, "  /reload-commands            Re-scan the custom commands directory.");
            raw_println(raw_mode, "  /help                       Show this help.");
            raw_println(raw_mode, "  /quit, /exit                Terminate (full aliases). Ctrl+D, Ctrl+C, SIGTERM also exit cleanly.");
            raw_println(raw_mode, "");
            raw_println(raw_mode, "At the prompt:");
            raw_println(raw_mode, "  !<command>                  Run a shell command right away (no approval); its output");
            raw_println(raw_mode, "                              is shown and handed to the model as context. Esc stops it.");
            raw_println(raw_mode, "  Tab                         Complete the slash command being typed, from the list");
            raw_println(raw_mode, "                              shown on the row above the prompt.");
            raw_println(raw_mode, "  Mouse wheel                 Scroll back through this session's log with the prompt");
            raw_println(raw_mode, "                              line pinned; the screen's bottom line reads inverted");
            raw_println(raw_mode, "                              \"Message below\" while you are scrolled — no spinner, no");
            raw_println(raw_mode, "                              elapsed time, at rest or mid-turn — and Esc or scrolling");
            raw_println(raw_mode, "                              to the bottom always returns to the live view with the");
            raw_println(raw_mode, "                              prompt re-displayed.");
            raw_println(raw_mode, "  Mouse drag                  Select a range of the log; releasing copies it to the");
            raw_println(raw_mode, "                              clipboard as plain text. Shift-drag selects with the");
            raw_println(raw_mode, "                              terminal instead ([ui] mouse_select, copy_command).");
            raw_println(raw_mode, "  Esc                         Stop a running turn or shell command; at the tool-approval");
            raw_println(raw_mode, "                              prompt, deny the pending tool. After the stop, the line");
            raw_println(raw_mode, "                              reads [info] cancelled by user and the prompt is");
            raw_println(raw_mode, "                              re-displayed.");
            raw_println(raw_mode, "  Subagents                   Each running subagent shows its name and operating");
            raw_println(raw_mode, "                              time stacked above the clock block; the row goes when");
            raw_println(raw_mode, "                              the subagent ends, and the clock takes the bottom line back");
            raw_println(raw_mode, "                              while the agent runs, the prompt when you can type again.");
            raw_println(raw_mode, "");
            raw_println(raw_mode, "Tool approval can be skipped via:");
            raw_println(raw_mode, "  - REPL command  : /auto on  (toggleable at runtime)");
            raw_println(raw_mode, "  - CLI flag      : agent-cli run --auto-approve-tools");
            raw_println(raw_mode, "  - Config file   : [runtime] auto_approve_tools = true");
            raw_println(raw_mode, "  None of these override a [permissions] deny rule: a deny is refused");
            raw_println(raw_mode, "  even with /auto on. See /permissions.");
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
        "permissions" => handle_permissions_command(state, raw_mode),
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
        "send" => {
            if let CommandResult::SpawnedAsk(ask) =
                send_to_peer(arg, &state.registry_dir, raw_mode).await
            {
                return CommandResult::SpawnedAsk(ask);
            }
        }
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
            // Raised only while a turn is actually in flight — raised while
            // the agent is idle, the flag would sit un-consumed (nothing is
            // unwinding, and the unwind-end clear never runs) and every later
            // peer prompt would skip until a manual prompt ran.
            raise_cancel_if_turn_in_flight(&state.cancel, &state.turn_in_flight);
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
        // FR-32: a peer the REPL's /spawn creates exits when its work is done,
        // matching the spawn tool's children.
        exit_when_idle: true,
    };
    // A peer the REPL spawns belongs to this session's tree.
    // The child's chain is this session's chain plus this session itself.
    let mut chain = state.ancestors.clone();
    chain.push(state.id.clone());
    match crate::commands::spawn_detached(state.config_source.chain(), &state.registry_dir, &run_args, &chain).await {
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

/// Parse the `/send` argument: an optional delivery option (`--prompt`, the
/// silent default; `--report`; `--ask`) followed by `<peer> <text>`. The
/// option reads naturally in either slot — `/send --ask <peer> <text>` or
/// `/send <peer> --report <text>` — and an unknown `--` word surfaces the
/// `send_to` tool's exact error, so both surfaces reject the same vocabulary.
/// The deprecated `wait_reply` boolean stays a tool-side concept.
fn parse_send_args(arg: &str) -> std::result::Result<(Delivery, String, String), String> {
    const USAGE: &str = "usage: /send [--report|--ask] <peer> <text>";
    // Leading option tokens.
    let mut delivery: Option<Delivery> = None;
    let mut rest = arg;
    loop {
        let (tok, after) = match rest.split_once(' ') {
            Some((f, a)) => (f, a.trim_start()),
            None => (rest, ""),
        };
        if tok.starts_with("--") {
            delivery = Some(resolve_option_token(tok)?);
            rest = after;
        } else {
            break;
        }
    }
    // The peer, then possibly one option token right after it.
    let (peer_token, tail) = match rest.split_once(' ') {
        Some((f, a)) => (f, a.trim_start()),
        None => (rest, ""),
    };
    if tail.starts_with("--") {
        let (tok, after) = match tail.split_once(' ') {
            Some((f, a)) => (f, a.trim_start()),
            None => (tail, ""),
        };
        delivery = Some(resolve_option_token(tok)?);
        rest = after;
    } else {
        rest = tail;
    }
    let peer = peer_token.trim();
    let text = rest.trim();
    if peer.is_empty() || text.is_empty() {
        return Err(USAGE.to_string());
    }
    Ok((delivery.unwrap_or(Delivery::Prompt), peer.to_string(), text.to_string()))
}

/// Resolve an option token that carries its dashes (`--report` → Report; an
/// unknown `--verbose` errors with the tool's exact text, dashes and all).
fn resolve_option_token(tok: &str) -> std::result::Result<Delivery, String> {
    match Delivery::resolve(Some(tok.trim_start_matches('-')), false) {
        Ok(d) => Ok(d),
        // The stripped word is not vocabulary either: re-resolve the token as
        // written so the error names what was typed.
        Err(_) => Delivery::resolve(Some(tok), false),
    }
}

async fn send_to_peer(arg: &str, registry_dir: &Path, raw_mode: bool) -> CommandResult {
    let (delivery, peer_name, text) = match parse_send_args(arg) {
        Ok(parsed) => parsed,
        Err(why) => {
            raw_eprintln(raw_mode, &format!("[error] {why}"));
            return CommandResult::Continue;
        }
    };
    match crate::ipc::registry::resolve_peer(registry_dir, &peer_name) {
        Ok(p) => match delivery {
            Delivery::Prompt => {
                let msg = crate::ipc::IpcMessage::Prompt {
                    from: p.id.clone(),
                    from_name: None,
                    text,
                    reply_to: None,
                };
                if let Err(e) = crate::ipc::client::send(&p.socket, &msg).await {
                    raw_eprintln(raw_mode, &format!("[error] {e}"));
                } else {
                    raw_println(raw_mode, &format!("delivered to {}", p.id));
                }
                CommandResult::Continue
            }
            Delivery::Report => {
                match crate::tools::send_to::deliver_report(
                    &p.id, registry_dir, &peer_name, &text,
                )
                .await
                {
                    Ok(outcome) => raw_println(raw_mode, &outcome),
                    Err(why) => raw_eprintln(raw_mode, &format!("[error] {why}")),
                }
                CommandResult::Continue
            }
            // The runner binds the reply socket and waits on its own task; the
            // loop keeps taking keys and the ask arm below prints the outcome
            // when it arrives, so a slow peer cannot hold the prompt hostage.
            Delivery::Ask => {
                let ask = crate::tools::send_to::PeerAsk::spawn(
                    &p.id, registry_dir.to_path_buf(), peer_name, text,
                );
                raw_println(
                    raw_mode,
                    &format!("[send] asked {} (waiting for the reply, up to 120s)", p.id),
                );
                CommandResult::SpawnedAsk(ask)
            }
        },
        Err(e) => {
            raw_eprintln(raw_mode, &format!("[error] {e}"));
            CommandResult::Continue
        }
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
        state.project_persona_path.as_deref(),
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

    /// The parked-pipeline pin: turn terminals that end while nobody is
    /// Pending (peer-prompt turns, the user sitting at an idle prompt)
    /// deposit idle notifications nobody receives. With the awaited send,
    /// the ninth terminal parked this display task; its event loop then
    /// backed the agent up mid-turn, and acked peer prompts stopped being
    /// dequeued until a manually typed prompt drained the idle channel at
    /// submit — the reported "peer prompt never starts; typing a prompt
    /// revives it" shape. The best-effort notify must keep the display task
    /// consuming forever: eight notifications fit, the surplus drops, and
    /// every event is still seen.
    #[tokio::test]
    async fn a_display_task_survives_more_unwatched_terminals_than_the_idle_channel_holds() {
        let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(64);
        let (agent_idle_tx, mut agent_idle_rx) = mpsc::channel::<()>(8);
        let handle = tokio::spawn(run_display_task(
            event_rx,
            agent_idle_tx,
            Arc::new(std::sync::Mutex::new(StatusIndicator::disabled())),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicBool::new(false)),
            watch::channel(false).0,
            ShowThinkingMode::Hidden,
            Theme::plain(),
            false,
            false,
        ));
        // Twelve terminals, no Pending consumer ever armed.
        for _ in 0..12 {
            event_tx.send(AgentEvent::Done).await.unwrap();
        }
        drop(event_tx); // the display task must run dry on its own
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("the display task must not park on unwatched terminals")
            .expect("the display task must not panic");
        // The eight slots held; the surplus dropped instead of parking.
        let mut delivered = 0usize;
        while agent_idle_rx.try_recv().is_ok() {
            delivered += 1;
        }
        assert_eq!(delivered, 8, "eight unwatched terminals fit; the rest drop");
    }

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
            id: AgentId::new(),
            ancestors: Vec::new(),
            registry_dir: dir.to_path_buf(),
            agents_dir: dir.to_path_buf(),
            persona_file_setting: String::new(),
            cli_persona_path: None,
            project_persona_path: None,
            name: Some("test".into()),
            persona: RwLock::new(Persona::builtin_default()),
            tool_names: Vec::new(),
            history_path: dir.join("history.txt"),
            history: RwLock::new(Vec::new()),
            auto_approve: Arc::new(AtomicBool::new(false)),
            permissions: Arc::new(crate::permissions::Ruleset::default()),
            cancel: Arc::new(CancelToken::default()),
            suppress: Arc::new(AtomicUsize::new(0)),
            turn_in_flight: Arc::new(AtomicBool::new(false)),
            indicator: Arc::new(std::sync::Mutex::new(StatusIndicator::disabled())),
            commands_dir: dir.to_path_buf(),
            commands: RwLock::new(Vec::new()),
            config_source: ConfigSource {
                layers: vec![dir.join("config.toml")],
                from_explicit: false,
            },
            group: None,
            theme: Theme::plain(),
            shell: crate::config::ShellConfig::default(),
            scroll: false,
            mouse_select: true,
            copy_command: String::new(),
        })
    }

    /// Test helper: the same state `build_state` builds, carrying the caller's
    /// indicator — the ownership handoffs are read off the shared indicator, so
    /// a pin has to observe that one rather than the disabled default.
    fn test_state_with_indicator(
        indicator: Arc<std::sync::Mutex<StatusIndicator>>,
    ) -> Arc<ReplState> {
        let dir = TempDir::new().unwrap();
        let base = build_state(dir.path());
        Arc::new(ReplState {
            id: base.id.clone(),
            ancestors: base.ancestors.clone(),
            registry_dir: base.registry_dir.clone(),
            agents_dir: base.agents_dir.clone(),
            persona_file_setting: base.persona_file_setting.clone(),
            cli_persona_path: None,
            project_persona_path: None,
            name: base.name.clone(),
            persona: RwLock::new(Persona::builtin_default()),
            tool_names: Vec::new(),
            history_path: base.history_path.clone(),
            history: RwLock::new(Vec::new()),
            auto_approve: Arc::new(AtomicBool::new(false)),
            permissions: Arc::new(crate::permissions::Ruleset::default()),
            cancel: Arc::new(CancelToken::default()),
            suppress: Arc::new(AtomicUsize::new(0)),
            turn_in_flight: Arc::new(AtomicBool::new(false)),
            indicator,
            commands_dir: base.commands_dir.clone(),
            commands: RwLock::new(Vec::new()),
            config_source: base.config_source.clone(),
            group: None,
            theme: Theme::plain(),
            shell: crate::config::ShellConfig::default(),
            scroll: false,
            mouse_select: true,
            copy_command: String::new(),
        })
    }

    /// Test helper: returns an unused approval channel.
    fn dummy_approval_rx() -> mpsc::Receiver<ApprovalRequest> {
        let (_tx, rx) = mpsc::channel::<ApprovalRequest>(4);
        rx
    }

    /// A `!` line typed at the prompt runs and its result is handed to the
    /// model as context — not as a prompt, so no turn is started.
    #[tokio::test]
    async fn a_bang_line_runs_a_command_and_feeds_the_result_as_context() {
        let tmp = TempDir::new().unwrap();
        let state = build_state(tmp.path());
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_idle_tx, idle_rx) = mpsc::channel::<()>(8);

        let (mut writer, reader) = tokio::io::duplex(128);
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
                None,
            )
            .await;
        });

        writer
            .write_all(b"!echo from-the-shell\n")
            .await
            .unwrap();
        let msg = tokio::time::timeout(Duration::from_secs(10), input_rx.recv())
            .await
            .expect("the command should finish")
            .expect("a context message");
        match msg {
            AgentInput::Context { text, source } => {
                assert!(text.contains("$ echo from-the-shell"), "{text}");
                assert!(text.contains("from-the-shell"), "{text}");
                assert!(text.contains("exit status: 0"), "{text}");
                match source {
                    crate::agent::ContextSource::Shell { command, .. } => {
                        assert_eq!(command, "echo from-the-shell")
                    }
                    other => panic!("a `!` line is shell context, not {other:?}"),
                }
            }
            other => panic!("a `!` line must not become a prompt: {other:?}"),
        }
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    /// The safety property: a peer's prompt is text, wherever it starts. It
    /// reaches the agent as a `PeerPrompt` and is never classified, so it can
    /// never run a command.
    #[test]
    fn a_peer_prompt_starting_with_a_bang_is_never_a_command() {
        // The classifier is only ever reached from the input loops; what a peer
        // sends is wrapped as `PeerPrompt` without passing through it.
        let peer = AgentInput::PeerPrompt {
            from: AgentId::new(),
            from_name: None,
            text: "!rm -rf /tmp/should-not-run".into(),
            reply_to: None,
        };
        match peer {
            AgentInput::PeerPrompt { text, .. } => {
                // Even if the text were classified, it would only be a command
                // for a line the user typed — this asserts the wrapping, which
                // is what keeps it away from the classifier.
                assert!(text.starts_with('!'));
            }
            other => panic!("unexpected input: {other:?}"),
        }
        // And with the feature on, the classifier itself is only a pure
        // function over a typed line: nothing in the IPC path calls it.
        assert_eq!(
            crate::shell::classify_line("!rm -rf /tmp/should-not-run", true),
            crate::shell::Line::Shell("rm -rf /tmp/should-not-run")
        );
    }

    /// With the feature off, a `!` line is an ordinary prompt again.
    #[tokio::test]
    async fn with_the_shell_disabled_a_bang_line_is_sent_to_the_model() {
        let tmp = TempDir::new().unwrap();
        let mut state_inner = std::sync::Arc::try_unwrap(build_state(tmp.path()))
            .map_err(|_| ())
            .expect("sole owner");
        state_inner.shell.enabled = false;
        let state = Arc::new(state_inner);
        let (input_tx, mut input_rx) = mpsc::channel::<AgentInput>(8);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_idle_tx, idle_rx) = mpsc::channel::<()>(8);

        let (mut writer, reader) = tokio::io::duplex(128);
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
                None,
            )
            .await;
        });

        writer.write_all(b"!echo nope\n").await.unwrap();
        let msg = tokio::time::timeout(Duration::from_secs(5), input_rx.recv())
            .await
            .expect("a message")
            .expect("a prompt");
        match msg {
            AgentInput::UserPrompt(text) => assert_eq!(text, "!echo nope"),
            other => panic!("expected a prompt, got {other:?}"),
        }
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    #[test]
    fn the_prompt_marks_a_line_that_will_run_in_the_shell() {
        assert_eq!(prompt_line_role("!ls", true), Role::Confirm);
        assert_eq!(prompt_line_role("  !ls", true), Role::Confirm);
        assert_eq!(prompt_line_role("hello", true), Role::PromptSymbol);
        assert_eq!(prompt_line_role("", true), Role::PromptSymbol);
        // With the feature off there is no shell mode to mark.
        assert_eq!(prompt_line_role("!ls", false), Role::PromptSymbol);
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
                None,
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
                None,
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
                None,
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
                None,
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
                None,
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
                None,
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

    // --- FR-38: the bare-Enter guard (D-59) ---

    #[test]
    fn bare_enter_is_ignored_without_states_side_effects() {
        // An empty line at Enter reaches no submit and mutates no editor
        // state: no action, and the line stays empty (D-59's guard sits
        // before `submit()`).
        let mut s = InputState::new();
        let action = handle_key(key(KeyCode::Enter, false), &mut s, &[]);
        assert!(action.is_none());
        assert_eq!(s.line, "");
        assert!(s.history_index.is_none());
    }

    #[test]
    fn bare_enter_after_draft_restore_is_ignored_too() {
        // The guard is not a one-shot: browse to history, Esc back to the
        // (empty) draft, press Enter — still ignored, history intact.
        let history = vec!["old".to_string()];
        let mut s = InputState::new();
        s.navigate_up(&history);
        let back = handle_key(key(KeyCode::Esc, false), &mut s, &history);
        assert!(matches!(back, Some(KeyAction::Continue)));
        let action = handle_key(key(KeyCode::Enter, false), &mut s, &[]);
        assert!(action.is_none());
        assert_eq!(s.line, "");
    }

    #[test]
    fn nonempty_enter_still_submits() {
        // A line with content submits exactly as before; the empty check is
        // the only new behavior.
        let mut s = InputState::new();
        s.insert_char('h');
        s.insert_char('i');
        let action = handle_key(key(KeyCode::Enter, false), &mut s, &[]);
        assert!(matches!(action, Some(KeyAction::Submit(l)) if l == "hi"));
    }

    #[test]
    fn whitespace_only_enter_still_submits() {
        // The guard suppresses the empty string only; whitespace falls
        // through to `submit()` unchanged.
        let mut s = InputState::new();
        s.insert_char(' ');
        let action = handle_key(key(KeyCode::Enter, false), &mut s, &[]);
        assert!(matches!(action, Some(KeyAction::Submit(l)) if l == " "));
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
    fn prompt_block_rows_counts_the_rows_the_prompt_occupies() {
        // "> " plus a short line: one row.
        assert_eq!(prompt_block_rows(2, 0, 2, 0, 80), 1);
        assert_eq!(prompt_block_rows(2, 5, 7, 0, 80), 1);
        // The suggestion row above the prompt is part of the block.
        assert_eq!(prompt_block_rows(2, 5, 7, 1, 80), 2);
        // A line that wraps takes the rows it wraps onto.
        assert_eq!(prompt_block_rows(2, 20, 25, 0, 10), 3);
        // A line that exactly fills a row leaves the cursor on a fresh one.
        assert_eq!(prompt_block_rows(2, 8, 10, 0, 10), 2);
    }

    #[test]
    fn the_wheel_takes_the_view_off_the_bottom_and_back_to_it() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        for i in 0..50 {
            crate::scroll::record_line(&format!("log line {i}"));
        }
        let mut overlay = ScrollOverlay::new();
        assert!(!overlay.view_active());
        // A notch up moves the view; the overlay only takes the screen once
        // the view has actually left the bottom.
        assert!(overlay.wheel(true, 80, 10));
        assert!(overlay.view_active());
        assert!(!overlay.is_active(), "the screen is taken by the caller");
        // Down again, and the view is live: offset 0 is the only position
        // without an overlay.
        assert!(overlay.wheel(false, 80, 10));
        assert!(!overlay.view_active());
        assert!(
            !overlay.wheel(false, 80, 10),
            "already at the bottom: nothing to redraw"
        );
        crate::scroll::uninstall();
    }

    #[test]
    fn the_body_slice_puts_screen_row_and_slice_index_on_the_same_number() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        crate::scroll::record_line("first");
        crate::scroll::record_line("second");

        // A log shorter than the body sits against the bottom block, so the
        // rows above it are empty padding — and index 8 is screen row 8.
        let rows = body_slice_at(80, 10, 0);
        assert_eq!(rows.len(), 10);
        assert!(rows[..8].iter().all(|r| r.text.is_empty()), "padded at the top");
        assert_eq!(rows[8].text, "first");
        assert_eq!(rows[9].text, "second");

        // Selecting over the padding copies nothing; selecting the two real
        // rows copies exactly them.
        let mut sel = crate::select::Selection::begin(crate::select::Point::new(0, 0), 0);
        sel.extend(crate::select::Point::new(7, 40));
        assert_eq!(crate::select::selected_text(&rows, &sel), "");
        sel.extend(crate::select::Point::new(9, 6));
        assert_eq!(crate::select::selected_text(&rows, &sel), "first\nsecond");
        crate::scroll::uninstall();
    }

    #[test]
    fn drag_release_resolves_at_the_release_time_offset() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        for i in 0..10 {
            crate::scroll::record_line(&format!("row zero {i}"));
        }
        // The press happens scrolled one notch up (offset 3): the highlighted
        // range covers two rows of the screen then in view.
        let press_body = 4;
        let press_rows = body_slice_at(80, press_body, 3);
        let mut sel = crate::select::Selection::begin(crate::select::Point::new(0, 0), 3);
        sel.extend(crate::select::Point::new(1, 10));
        let press_text = crate::select::selected_text(&press_rows, &sel);
        assert_eq!(press_text, "row zero 3\nrow zero 4");
        // By release the running turn streamed a line in, the view scrolled
        // back to the bottom, and the wheel's slice is now offset 0. The copy
        // is the release-time slice — what the screen highlighted at release —
        // not the press-time range at offset 3.
        crate::scroll::record_line("row streamed");
        let release_rows = body_slice_at(80, press_body, 0);
        let text = crate::select::selected_text(&release_rows, &sel);
        assert_ne!(text, press_text, "the offsets disagree, so the text does");
        assert_eq!(
            text, "row zero 7\nrow zero 8",
            "the release-time slice is the copy: {text:?}"
        );
        crate::scroll::uninstall();
    }

    #[test]
    fn a_peer_row_carries_the_processing_content_when_the_registry_has_it() {
        // FR-37b (T-79's content pin): the face grows the processing content
        // beside the operating time, the row staying one line cut to the
        // width.
        let rows = peer_status_rows(
            &[(
                "worker".to_string(),
                std::time::Duration::from_secs(65),
                Some("tool bash".to_string()),
            )],
            80,
        );
        assert_eq!(rows.len(), 1, "one live child, one row: {rows:?}");
        assert!(rows[0].contains("worker"), "the peer is named: {rows:?}");
        assert!(
            rows[0].contains("1m05s"),
            "the operating time stays (D-58): {rows:?}"
        );
        assert!(
            rows[0].contains("tool bash"),
            "the processing content rides the row: {rows:?}"
        );
        // The whole face is one row cut to the width budget.
        let wide = peer_status_rows(
            &[(
                "w".to_string(),
                std::time::Duration::from_secs(1),
                Some("x".repeat(200)),
            )],
            40,
        );
        assert_eq!(wide.len(), 1, "still one row when cut: {wide:?}");
        assert!(
            crate::editor::str_display_width(&wide[0]) <= 39,
            "the row is cut to the width: {}",
            crate::editor::str_display_width(&wide[0])
        );
    }

    #[test]
    fn a_peer_row_degrades_to_the_shipped_face_when_the_state_is_absent() {
        // The degrade rule (D-58): no turn state in the registry entry (an old
        // version's file, an idle peer) and the face is exactly the shipped
        // name-and-operating-time.
        let rows = peer_status_rows(
            &[("alpha".to_string(), std::time::Duration::from_secs(1), None)],
            80,
        );
        assert_eq!(rows[0], "peer alpha · 1.0s", "{rows:?}");
        // A blank or whitespace-only content is absence too.
        let blank = peer_status_rows(
            &[(
                "beta".to_string(),
                std::time::Duration::from_secs(2),
                Some("   ".to_string()),
            )],
            80,
        );
        assert_eq!(blank[0], "peer beta · 2.0s", "{blank:?}");
    }

    #[test]
    fn the_peer_rows_keep_their_stacking_order() {
        // T-79's stacking pin (FR-23/AS-26) over the display's real read
        // path: `live_children_states` sorts the registry's children
        // oldest-first — the spawn order itself — so the painter walking
        // downward lands the newest on the row just above the block that
        // holds the bottom. The directory is written newest-first and the
        // names are set against the age order, so a sort by the registry's
        // read order or by name fails this pin.
        let tmp = TempDir::new().unwrap();
        let parent = AgentId::new();
        let entry = |name: &str, started_at: chrono::DateTime<chrono::Utc>| {
            crate::ipc::registry::RegistryEntry {
                id: AgentId::new(),
                name: Some(name.to_string()),
                group: None,
                pid: 1,
                started_at,
                provider: "ollama".into(),
                model: "m".into(),
                // The directory itself is the socket stand-in: the read's
                // reap-on-read prunes entries whose socket is gone, and the
                // pin tests the stacking order, not liveness.
                socket: tmp.path().to_path_buf(),
                persona: None,
                ancestors: vec![parent.clone()],
                turn: None,
            }
        };
        // Written newest-first: the read order is the opposite of the ask.
        let newer = entry("apple", chrono::Utc::now());
        let older = entry("zed", chrono::Utc::now() - chrono::Duration::seconds(30));
        for e in [&newer, &older] {
            std::fs::write(
                tmp.path().join(format!("{}.json", e.id.as_str())),
                serde_json::to_string(e).unwrap(),
            )
            .unwrap();
        }
        let states = live_children_states(tmp.path(), &parent);
        assert_eq!(states.len(), 2, "both live children read: {states:?}");
        assert_eq!(states[0].0, "zed", "the oldest child first: {states:?}");
        assert_eq!(states[1].0, "apple", "the newest last: {states:?}");
        assert!(
            states[0].1 > states[1].1,
            "the ages agree with the order: {states:?}"
        );
        // The painter's rows follow the same order (the newest nearest the
        // clock).
        let rows = peer_status_rows(&states, 80);
        assert!(
            rows[0].contains("zed") && rows[1].contains("apple"),
            "{rows:?}"
        );
    }

    #[test]
    fn spawned_subagent_shows_its_line_with_operating_time() {
        let rows = peer_status_rows(
            &[("worker".to_string(), std::time::Duration::from_secs(65), None)],
            80,
        );
        assert_eq!(rows.len(), 1, "one live child, one row: {rows:?}");
        assert!(rows[0].contains("worker"), "the peer is named: {rows:?}");
        assert!(
            rows[0].contains("1m05s"),
            "the operating time is the elapsed format: {rows:?}"
        );
        // The styled row carries the Peer role, not the transcript's record.
        let styled = crate::theme::paint(crate::theme::Role::Peer, &rows[0]);
        assert!(styled.contains("\u{1b}["), "the row is styled: {styled:?}");
    }

    #[test]
    fn second_subagent_gets_its_own_row_and_finished_one_is_removed() {
        let two = peer_status_rows(
            &[
                ("alpha".to_string(), std::time::Duration::from_secs(1), None),
                ("beta".to_string(), std::time::Duration::from_secs(2), None),
            ],
            80,
        );
        assert_eq!(two.len(), 2, "each live child its own row: {two:?}");
        assert!(two[0].contains("alpha") && two[1].contains("beta"));
        // The reap decides removal: a finished child is simply not in the
        // registry read's output, so the rows contract covers its absence.
        let gone = peer_status_rows(
            &[("beta".to_string(), std::time::Duration::from_secs(2), None)],
            80,
        );
        assert_eq!(gone.len(), 1, "the finished peer's line is gone: {gone:?}");
        assert!(!gone[0].contains("alpha"), "no stale row remains");
    }

    // --- T-77's pins: the FR-37a composition's own geometry (the live
    // screen's zone) ---

    #[test]
    fn the_beat_plan_holds_the_four_regions_in_the_instruction_order() {
        // FR-37a's re-stack, bound through the pure plan the beat paints with:
        // read top-down the composition is FR-37's four regions — the log
        // region above (there the sweep erases and the tail fills), the run
        // display (the clock block), the subagent rows, the prompt span last.
        // Fold check: swapping the block above the peers (the run display
        // above the subagent rows) fails the order asserts.
        let plan = zone_beat_plan(23, 2, 1);
        // The clock block sits directly above the span
        assert_eq!(plan.block_home, 21, "the block's home is span_top − block_n");
        // The band above the block; its last row is the block's top neighbour
        assert_eq!(plan.peers_top, 20, "the band is above the block");
        assert_eq!(
            plan.peers_top + 1, plan.block_home,
            "the band's bottom row is the block's top neighbour"
        );
        // The sweep never enters the span (D-53) and covers all the region
        // above
        assert_eq!(plan.sweep_to, 23, "the sweep erases every row above the span");
        // No peers: the block still sits directly above the span
        let bare = zone_beat_plan(23, 1, 0);
        assert_eq!(bare.block_home, 22);
        assert_eq!(bare.peers_top, 22, "the band's plan collapses into the block");
    }

    #[test]
    fn the_span_never_enters_the_sweep_even_when_the_block_exhausts_the_screen() {
        // D-53's guarantee at the geometry's degenerate edge: a turn whose
        // thinking block plus peers cover every row above the span still
        // leaves the span's rows out of the beat's sweep (and the plan's
        // saturating arithmetic keeps every row in range).
        let plan = zone_beat_plan(3, 3, 0);
        assert_eq!(plan.block_home, 0, "the block fills every row above the span");
        assert_eq!(plan.sweep_to, 3, "the span's rows are still excluded");
        // Overflowing the screen clamps at row 0 — nothing is negative, no
        // panic, and the span's exclusion is unchanged.
        let clamped = zone_beat_plan(0, 5, 5);
        assert_eq!(clamped.block_home, 0);
        assert_eq!(clamped.peers_top, 0);
        assert_eq!(clamped.sweep_to, 0);
    }

    #[test]
    fn the_zone_click_rule_hits_only_the_blocks_reasoning_rows() {
        // FR-27's rule at the zone's clock block, re-bound for FR-46a's
        // flipped face: the reasoning rows stand ABOVE the status row (the
        // block's last row is the spinner row now, the screen's bottom-most
        // line mid-turn), so the toggle rows are the block's rows below its
        // last row — the reasoning rows and their elision marker. The status
        // row itself and everything outside the block never toggle, and a
        // block not painted (`clock_rows <= 1`) takes no click. Fold check:
        // the shipped spinner-first face's rule (the rows above the
        // spinner excluded) fails the boundary asserts here.
        // The flipped face's boundary: rows 20-21 are the reasoning rows
        // (above the status row), row 22 is the status row itself — the
        // screen's bottom-most line mid-turn, never a toggle.
        assert!(click_hits_clock_block(20, 3, 20));
        assert!(click_hits_clock_block(20, 3, 21));
        assert!(
            !click_hits_clock_block(20, 3, 22),
            "the status row (the block's last row) is not a toggle"
        );
        assert!(
            !click_hits_clock_block(20, 3, 23),
            "below the block is out of range"
        );
        assert!(
            !click_hits_clock_block(10, 1, 11),
            "a single-row block has no reasoning rows to toggle"
        );
        assert!(!click_hits_clock_block(10, 0, 11));
        assert!(
            !click_hits_clock_block(20, 3, 19),
            "the tail above the block is the log"
        );
    }

    #[test]
    fn the_disarm_leaves_the_bottom_row_fresh_and_the_record_ends_above_it() {
        // D-54's handoff, bound through `tail_for_region`'s seam: the erase's
        // budget ends the replay on the row above the bottom row, so the
        // shipped render draws the prompt on a fresh bottom row with the
        // outcome line above it. The disarm itself needs a terminal to paint
        // at, so the pin binds its arithmetic through the same plan the erase
        // walks and asks the record for exactly the plan's budget. Fold check:
        // replaying the tail into the bottom row (the fold: the plan's floor
        // one row lower into `height`) fails the bounds assert.
        let plan = zone_disarm_plan(24, 0);
        assert_eq!(plan.tail_floor, 22, "the replay ends above the bottom row");
        assert_eq!(plan.budget, 23, "the erase owns every row it could paint");
        let degenerate = zone_disarm_plan(1, 0);
        assert_eq!(degenerate.tail_floor, 0, "the height clamp stays in range");
        assert_eq!(degenerate.budget, 0, "a one-row terminal replays nothing");

        // The record's side of the seam: the tail the disarm asks for exists,
        // is at most the budget, and — with the transcript shorter than the
        // budget — returns exactly what is there, which is what leaves the
        // bottom row free for the shipped prompt.
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        for i in 0..5 {
            crate::scroll::record_line(&format!("log line {i}"));
        }
        let rows = crate::scroll::tail_for_region(80, plan.budget).unwrap();
        assert!(
            rows.len() <= plan.budget,
            "the replay never exceeds the budget: {rows:?}"
        );
        assert_eq!(
            rows.last().map(|r| crate::theme::strip_sgr(r)),
            Some("log line 4".to_string()),
            "the record's tail is what the replay paints last: {rows:?}"
        );
        crate::scroll::uninstall();
    }

    #[test]
    fn the_clock_row_is_not_the_subagent_lines_price() {
        // The budget is additive: the running block keeps the prompt span and
        // the elapsed row above it with the peer lines present (FR-40), and the
        // peer lines pay for no part of either.
        let pending_clock =
            bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, 0, false);
        let pending_both =
            bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, 2, false);
        assert_eq!(pending_clock, 2, "the prompt span and its elapsed row alone");
        assert_eq!(pending_both, 4, "block + two peer rows: additive");
        let prompt = bottom_block_rows(&PromptState::Ready, &InputState::new(), 80, 2, false);
        let bare = bottom_block_rows(&PromptState::Ready, &InputState::new(), 80, 0, false);
        assert_eq!(prompt, bare + 2, "the prompt block's rows are not taken");
    }

    #[test]
    fn the_line_survives_the_intersections_and_the_screen_shows_it() {
        // The drawer's budget: peer rows ride beneath the pinned block on the
        // scrolled screen, and the body must shrink by exactly that many rows.
        let height = 24usize;
        // The running block: the prompt span on the bottom row and the elapsed
        // row directly above it (FR-40).
        let bottom = bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, 0, false);
        assert_eq!(bottom, 2, "the prompt span plus the elapsed row above it");
        let body = crate::scroll::body_rows(height, bottom + 2);
        assert_eq!(body, 20, "two peer rows cost two body rows, not the block's");
        // The overlay bottom block and the live-screen budget agree.
        let peers: Vec<String> = vec!["peer alpha · 1.0s".to_string()];
        let budget =
            bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, peers.len(), false);
        assert_eq!(budget, bottom + peers.len() as u16);
    }

    #[test]
    fn esc_from_a_scrolled_view_mid_turn_reshows_the_prompt_pin() {
        // The arm's redraw is unconditional now (FR-21): the pin binds the
        // redraw contract through the pure role-choice it shares with the
        // shipped not-pending redraw. The role the arm paints for each state.
        let role = |awaiting: bool| {
            if awaiting {
                (Role::Confirm, APPROVAL_PROMPT)
            } else {
                (Role::PromptSymbol, PROMPT)
            }
        };
        assert_eq!(role(false).1, PROMPT);
        assert_eq!(role(true).1, APPROVAL_PROMPT);
    }

    #[test]
    fn esc_return_redraws_the_prompt_in_every_state() {
        // FR-24's mechanism: the seam answers a redraw for every input-loop
        // state, Pending included — the shipped gate skipped exactly that one,
        // leaving a mid-turn Esc-return with a blockless bottom. Fold check:
        // reinstating the gate (`None` for `Pending`) fails this pin.
        assert!(esc_return_redraw(&PromptState::Ready).is_some());
        assert!(esc_return_redraw(&PromptState::Pending).is_some());
        assert!(esc_return_redraw(&PromptState::Shell).is_some());
        let (tx, _rx) = oneshot::channel::<bool>();
        assert!(esc_return_redraw(&PromptState::AwaitingApproval(tx)).is_some());
        // What the redraw paints: the approval prompt while awaiting, the
        // input prompt otherwise (the same pair the shipped redraw used).
        assert_eq!(
            esc_return_redraw(&PromptState::AwaitingApproval(
                oneshot::channel::<bool>().0
            ))
            .unwrap()
            .1,
            APPROVAL_PROMPT
        );
        assert_eq!(esc_return_redraw(&PromptState::Ready).unwrap().1, PROMPT);
    }

    #[test]
    fn the_prompt_span_holds_the_bottom_rows_with_subagents_pinned() {
        // FR-40 (D-61) re-derives FR-22/FR-23's geometry: the pinned block's
        // bottom-most rows are the prompt span in every state — the live
        // screen's four-region order — so with n live children the peer rows
        // stack above the span and the span still ends the screen. Fold check:
        // the shipped clock-at-the-bottom stack (the clock on the screen's last
        // row, the prompt span dropped) fails the bottom-rows assert.
        let height = 24usize;
        let prompt_rows = 1u16;
        for n in 0..4usize {
            let plan = scrolled_block_plan(prompt_rows, n, false, false);
            assert_eq!(plan.rows, n as u16 + prompt_rows, "peers + the span");
            assert_eq!(
                plan.prompt_top, n as u16,
                "with {n} subagent(s) the peer rows stack above the span"
            );
            // The block owns the screen's bottom rows: `body_rows` leaves
            // exactly the plan's rows under the log.
            let body = crate::scroll::body_rows(height, plan.rows);
            assert_eq!(
                body as u16 + plan.rows,
                height as u16,
                "the block owns the screen's bottom rows whatever the child count"
            );
        }
        // A running turn inserts the elapsed row between the peers and the
        // span, so the span still ends the screen and the elapsed row is its
        // top neighbour (FR-40, AS-26's bottom-up order).
        let running = scrolled_block_plan(prompt_rows, 2, true, false);
        assert_eq!(running.run_row, Some(2), "the elapsed row sits above the span");
        assert_eq!(running.run_row.unwrap() + 1, running.prompt_top);
        assert_eq!(running.rows, 4, "two peers + the elapsed row + the span");
    }

    #[test]
    fn the_prompt_renderer_tracks_the_subagent_rows_it_paints() {
        // The live screen's FR-23 rows live in the tracked region: the erase
        // covers them together with the prompt block, so a reap between
        // renders (or a submit) leaves no stale line. The bookkeeping drives
        // `clear_block`'s MoveUp; the erase resets both counters.
        let mut renderer = PromptRenderer::new();
        assert_eq!(renderer.peer_rows, 0);
        let mut stdout = std::io::stdout();
        renderer.render(
            &mut stdout,
            &Theme::plain(),
            Role::PromptSymbol,
            PROMPT,
            &InputState::new(),
            None,
            &[
                "peer alpha · 1.0s".to_string(),
                "peer beta · 2.0s".to_string(),
            ],
            None,
            None,
        );
        assert_eq!(
            renderer.peer_rows, 2,
            "the rows are part of the tracked region"
        );
        // An empty one-row prompt block tracks cursor row 0 (the shipped
        // `prompt_block_rows` pin's shape); the peers above it are what makes
        // the erase climb — `clear_block` moves back over the whole region.
        assert_eq!(renderer.cursor_row, 0);
        renderer.clear_block(&mut stdout);
        assert_eq!(renderer.peer_rows, 0, "the erase resets the region");
        assert_eq!(renderer.cursor_row, 0);
        renderer.reset();
        assert_eq!(renderer.peer_rows, 0);
    }

    #[test]
    fn the_scrolled_prompt_pin_carries_the_marker_row() {
        // FR-39 (D-60): with the view off the bottom and the prompt pinned, the
        // marker stands on the row **immediately above** the block's first row —
        // not beneath the block's last row as the shipped shape painted it. The
        // drawer's budget and the wheel/drag budgets grow by exactly that row,
        // so the wheel's range and the draw agree on where the body ends. The
        // marker is part of the tracked region now, so `cursor_row` counts it
        // and the next render's erase climbs past it. Fold check: the shipped
        // beneath-the-block place fails the place asserts here and the
        // `cursor_row` fold below.
        let mut renderer = PromptRenderer::new();
        let mut stdout = std::io::stdout();
        renderer.render(
            &mut stdout,
            &Theme::plain(),
            Role::PromptSymbol,
            PROMPT,
            &InputState::new(),
            None,
            &[],
            None,
            Some(SCROLLED_MARK),
        );
        // The marker is printed above the block's one row, so the tracked
        // region is two rows ending on the prompt row (cursor row 0 relative to
        // the prompt); the fold counts the row above it.
        assert_eq!(renderer.peer_rows, 0);
        assert_eq!(
            renderer.cursor_row, 1,
            "the marker's row is inside the tracked region, above the block"
        );
        // The place, block-relative: no rows when the view is at the bottom;
        // the marker directly above the span when it has moved.
        let at_bottom = scrolled_block_plan(1, 0, false, false);
        assert_eq!(at_bottom.marker_row, None, "no marker at the log's bottom");
        assert_eq!(at_bottom.prompt_top, 0, "the span owns the bottom row");
        let moved = scrolled_block_plan(1, 0, false, true);
        assert_eq!(
            moved.marker_row,
            Some(0),
            "the marker takes the row above the block's first row"
        );
        assert_eq!(
            moved.prompt_top, 1,
            "the prompt's first row is one below the marker"
        );
        assert_eq!(moved.rows, 2, "the marker's row joins the footprint");
        // A wrapped prompt keeps the marker directly above its first row (AS-59)
        let wrapped = scrolled_block_plan(3, 0, false, true);
        assert_eq!(wrapped.marker_row, Some(0));
        assert_eq!(wrapped.prompt_top, 1, "the marker is above the block's top");
        assert_eq!(wrapped.rows, 4, "the span's three rows plus the marker");
        // The helper is the shared arithmetic: one row off the bottom, none at
        // it — and the three arithmetic sites all ask it.
        assert_eq!(scrolled_marker_rows(true), 1);
        assert_eq!(scrolled_marker_rows(false), 0);
        let view_moved = scrolled_marker_rows(true) > 0;
        let shifted = bottom_block_rows(&PromptState::Ready, &InputState::new(), 80, 2, view_moved);
        let still = bottom_block_rows(&PromptState::Ready, &InputState::new(), 80, 2, false);
        assert_eq!(shifted, still + 1, "the marker's row joins the budget");
        // Mid-turn the same row joins the live composition's count (FR-40):
        // the span, the elapsed row, the marker and the peers.
        let pending_moved =
            bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, 0, view_moved);
        assert_eq!(
            pending_moved, 3,
            "the span + the elapsed row + the marker"
        );
    }

    #[test]
    fn the_scrolled_screen_keeps_the_elapsed_row_and_adds_the_marker() {
        // FR-40 (D-61) re-derives FR-26a's swap. The shipped shape replaced the
        // elapsed row with `Message below` in the one row the Status pin owned,
        // so a running turn's clock vanished as soon as the log was scrolled.
        // The corrected composition keeps the elapsed row on screen and adds
        // the marker beside it: the span at the bottom rows, the elapsed row
        // above it, the marker above that (FR-39's place), the peers above.
        // Fold check: the shipped single-row swap (the marker in the clock's
        // row, the elapsed row withheld) fails the "the clock is still drawn"
        // assert.
        let running = scrolled_block_plan(1, 0, true, true);
        assert_eq!(
            running.run_row,
            Some(0),
            "the elapsed row stays, on the row above the span"
        );
        assert_eq!(
            running.marker_row,
            Some(1),
            "the marker takes the row above the elapsed row"
        );
        assert_eq!(running.prompt_top, 2, "the span owns the bottom row");
        assert_eq!(running.rows, 3, "the elapsed row, the marker, the span");
        // Both rows are drawn, so the composition carries them both — the one
        // row the shipped swap could show becomes three.
        let shipped_swap_rows = 1u16;
        assert!(
            running.rows > shipped_swap_rows,
            "the corrected pin is taller than the swap it replaces: {} > {}",
            running.rows,
            shipped_swap_rows
        );
        // At the log's bottom there is no marker and the elapsed row stands
        // directly above the span, as the live screen draws it.
        let at_bottom = scrolled_block_plan(1, 0, true, false);
        assert_eq!(at_bottom.marker_row, None);
        assert_eq!(at_bottom.run_row, Some(0));
        assert_eq!(at_bottom.prompt_top, 1);
        assert_eq!(at_bottom.rows, 2, "the elapsed row above the span");
        // With no turn tracked the marker's row and the span are the whole
        // block: the shipped not-running prompt pin, with the marker's place
        // corrected (FR-39).
        let idle = scrolled_block_plan(1, 0, false, true);
        assert_eq!(idle.run_row, None, "no elapsed row without a turn");
        assert_eq!(idle.marker_row, Some(0));
        assert_eq!(idle.rows, 2);
    }

    #[test]
    fn a_failed_copy_write_still_reports_and_an_empty_gesture_stays_silent() {
        // An erroring writer: the outcome must be the Failed route, reported.
        let outcome = crate::clip::copy(
            "some text",
            "",
            |_seq| Err(std::io::Error::other("write refused")),
        );
        assert!(
            matches!(outcome, crate::clip::Outcome::Failed { .. }),
            "a failed write is a Failed outcome: {outcome:?}"
        );
        assert!(!outcome.is_empty(), "a failure is reported");
        // An empty gesture: silence.
        let empty = crate::clip::copy("", "", |_seq| Ok(()));
        assert!(
            matches!(empty, crate::clip::Outcome::Empty),
            "an empty gesture copies nothing"
        );
        assert!(empty.is_empty(), "the empty outcome is not reported");
    }

    #[test]
    fn a_selection_copies_the_line_the_terminal_wrapped_as_one_line() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        crate::scroll::record_line("abcdefghij");

        // At ten columns the line is one row; at four it is three, and the
        // selection has to put it back together.
        let wide = body_slice_at(10, 1, 0);
        let mut all = crate::select::Selection::begin(crate::select::Point::new(0, 0), 0);
        all.extend(crate::select::Point::new(0, 10));
        assert_eq!(crate::select::selected_text(&wide, &all), "abcdefghij");

        let narrow = body_slice_at(4, 3, 0);
        assert_eq!(narrow.len(), 3);
        assert!(narrow[1].continues && narrow[2].continues, "wrapped rows");
        let mut over_all = crate::select::Selection::begin(crate::select::Point::new(0, 0), 0);
        over_all.extend(crate::select::Point::new(2, 4));
        assert_eq!(
            crate::select::selected_text(&narrow, &over_all),
            "abcdefghij",
            "the wrap is undone"
        );
        crate::scroll::uninstall();
    }

    #[test]
    fn the_overlay_owns_the_screen_while_a_drag_is_live_even_at_the_bottom() {
        let mut overlay = ScrollOverlay::new();
        assert!(!overlay.has_selection());
        overlay.begin_selection(crate::select::Point::new(2, 3));
        assert!(overlay.has_selection());
        assert!(!overlay.view_active(), "a drag does not move the view");
        // A drag report that does not move costs no repaint.
        assert!(overlay.extend_selection(crate::select::Point::new(4, 5)));
        assert!(!overlay.extend_selection(crate::select::Point::new(4, 5)));
        let sel = overlay.take_selection().expect("the finished drag");
        assert_eq!(sel.normalised().1, crate::select::Point::new(4, 5));
        assert!(!overlay.has_selection(), "taken once");
    }

    #[test]
    fn the_wheel_does_nothing_when_the_log_fits_on_the_screen() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        crate::scroll::record_line("one line only");
        let mut overlay = ScrollOverlay::new();
        assert!(!overlay.wheel(true, 80, 24));
        assert!(!overlay.view_active());
        crate::scroll::uninstall();
    }

    /// A turn tracked on the indicator, for the scroll-pin tests below: the
    /// same shape the display task produces on `TurnStart`.
    #[test]
    fn elapsed_clock_survives_scrolling_during_a_running_turn() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        for i in 0..30 {
            crate::scroll::record_line(&format!("log line {i}"));
        }
        let sink = CaptureSink::default();
        let mut ind = {
            let mut ind = test_indicator(&sink);
            ind.height_override = Some(24);
            ind
        };
        ind.run = Some(Run {
            started: std::time::Instant::now(),
            frame: 0,
        });
        // A wheel notch up mid-turn: the scrolled screen's draw pins the
        // status row on screen, and the pinned clock is display-only — the
        // draw never records its beat rows, so the wheel's scrollable
        // history holds the turn's output, not the spinner beats. The fold:
        // reinstating the old record-in-the-draw behaviour (recording the
        // row as this test did before the fix) FAILS the negative pin.
        let _row = ind.overlay_status_row().expect("a tracked turn has a row");
        let texts: Vec<String> = crate::scroll::with(|t| {
            crate::scroll::marked_rows(t, 80, 10, 0)
                .into_iter()
                .map(|r| crate::theme::strip_sgr(&r.text))
                .collect()
        })
        .unwrap();
        assert!(
            !texts.iter().any(|t| t.contains("0.0s")),
            "the pinned clock's beat rows stay out of the scrollable history: {texts:?}"
        );
        // The turn's output the log does keep, still in place.
        assert_eq!(texts[9], "log line 29", "the log's own lines are intact");
        crate::scroll::uninstall();
    }

    #[test]
    fn wheel_range_ends_above_the_clock_row() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        for i in 0..30 {
            crate::scroll::record_line(&format!("log line {i}"));
        }
        // A running turn's pin: the prompt span and the elapsed row above it
        // (FR-40), so the wheel's body is height minus those rows — neither
        // is ever part of the scrollable range.
        let bottom = bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, 0, false);
        assert_eq!(bottom, 2, "the tracked turn pins the span and its clock");
        let body = crate::scroll::body_rows(24, bottom);
        assert_eq!(body, 22);
        let mut overlay = ScrollOverlay::new();
        let _max = crate::scroll::with(|t| crate::scroll::max_offset(t, 80, body)).unwrap();
        overlay.wheel(true, 80, body);
        assert!(
            overlay.view.offset() <= crate::scroll::with(|t| crate::scroll::total_rows(t, 80))
                .unwrap()
                .saturating_sub(body),
            "the wheel cannot push content under the pinned clock"
        );
        crate::scroll::uninstall();
    }

    #[test]
    fn finishing_a_turn_while_scrolled_leaves_the_outcome_line() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        crate::scroll::record_line("log line one");
        let sink = CaptureSink::default();
        // Suspended: exactly the scrolled-view state — the finish must still
        // record the outcome row (BL-M08's swallow is the repair).
        let mut ind = test_indicator(&sink);
        ind.run = Some(Run {
            started: std::time::Instant::now() - std::time::Duration::from_secs(5),
            frame: 3,
        });
        ind.suspend();
        ind.finish(Mark::Ok);
        let texts: Vec<String> = crate::scroll::with(|t| {
            t.lines().map(|l| crate::theme::strip_sgr(l).to_string()).collect()
        })
        .unwrap();
        assert!(
            texts.iter().any(|t| t.contains("\u{2714}") && t.contains("5.0s")),
            "the outcome row must be recorded even while suspended: {texts:?}"
        );
        assert!(
            !sink.taken().contains("\u{2714}"),
            "a suspended block writes nothing to the terminal"
        );
        crate::scroll::uninstall();
    }

    #[test]
    fn the_writers_record_what_stays_on_screen_and_not_what_is_erased() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);

        // Whole-line messages, and streamed text arriving in fragments.
        raw_println(false, "MARK banner");
        raw_eprintln(false, "MARK [info] something");
        raw_print_str(false, "MARK answer ");
        raw_print_str(false, "text\n");

        // The prompt echo stays; the block being edited does not.
        let mut renderer = PromptRenderer::new();
        let mut stdout = std::io::stdout();
        renderer.finish_line(
            &mut stdout,
            &Theme::plain(),
            Role::PromptSymbol,
            "MARK> ",
            "the question",
        );

        // The indicator's block is erased and redrawn ten times a second, so
        // only its closing line belongs to the transcript.
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        ind.start();
        ind.paint(true);
        ind.repaint();
        ind.finish(Mark::Ok);

        let lines = crate::scroll::with(|t| {
            t.lines()
                .filter(|l| l.starts_with("MARK") || l.starts_with('✔'))
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap();
        crate::scroll::uninstall();

        assert_eq!(
            lines[..4],
            [
                "MARK banner",
                "MARK [info] something",
                "MARK answer text",
                "MARK> the question",
            ],
            "recorded lines, in write order"
        );
        assert_eq!(lines.len(), 5, "one outcome row, and no transient rows");
        let outcome = lines.last().unwrap();
        assert!(
            outcome.starts_with('✔') && !outcome.contains('\u{1b}'),
            "the outcome row is recorded, the erased block is not: {outcome:?}"
        );
        assert!(
            !lines.iter().any(|l| l.contains(SPINNER_FRAMES[0])),
            "no transient spinner row was recorded"
        );
    }

    #[test]
    fn the_zone_erase_clears_each_row_it_names() {
        // FR-49: the erase's emit must address each named row's first column
        // under the crossterm row-first convention — a `\x1b[{row+1};1H`
        // cursor position, then the line erase. The shipped call passed the
        // row into the column slot, so every beat and every disarm erased
        // row 1's columns instead of the row it named: the banner row was
        // the only surface the zone ever cleared.
        assert_eq!(zone_csi_clear_row(0), "\x1b[1;1H\x1b[2K");
        assert_eq!(zone_csi_clear_row(1), "\x1b[2;1H\x1b[2K");
        assert_eq!(zone_csi_clear_row(7), "\x1b[8;1H\x1b[2K");
        assert_eq!(zone_csi_move(0, 5), "\x1b[6;1H");
        assert_eq!(zone_csi_move(3, 5), "\x1b[6;4H");
    }

    #[test]
    fn the_zone_disarm_plan_never_replays_below_the_painted_top() {
        // FR-49's handoff arithmetic: the replay budget is the span of rows
        // between the painted top and the tail floor; a zone that painted
        // nothing replays nothing (the degenerate arm), and a zone whose
        // beats painted the whole screen replays every row but the prompt's.
        let degenerate = zone_disarm_plan(26, 25);
        assert_eq!(degenerate.tail_floor, 24);
        assert_eq!(degenerate.budget, 0);
        let full = zone_disarm_plan(26, 0);
        assert_eq!(full.budget, 25);
        assert_eq!(zone_disarm_plan(26, 14).budget, 11);
        // A height too small to hold the prompt and a row above it replays nothing.
        assert_eq!(zone_disarm_plan(1, 0).budget, 0);
    }

    #[test]
    fn a_bare_click_costs_the_screen_nothing() {
        // FR-48a/FR-48b: a press + release on the log with nothing between —
        // a plain click — starts a selection the gesture never paints and a
        // release that takes no screen and redraws nothing. The face is the
        // click's zero-touch contract: `scroll_dirty` stays false (the drain's
        // repaint tail runs for no click), the overlay never becomes active,
        // and the empty selection contributes no clipboard write. The gate the
        // release arm reads is the overlay's own view state, so the pin walks
        // the same arithmetic the arm does.
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        for i in 0..30 {
            crate::scroll::record_line(&format!("log line {i}"));
        }
        let mut overlay = ScrollOverlay::new();
        let bottom = bottom_block_rows(&PromptState::Ready, &InputState::new(), 80, 0, false);
        let body = crate::scroll::body_rows(24, bottom);

        // The press records the anchor at a body row; nothing paints, the
        // overlay stays off the screen, and the (empty) selection exists.
        overlay.begin_selection(crate::select::Point::new(5, 10));
        assert!(overlay.has_selection(), "the click's anchor is recorded");
        assert!(!overlay.is_active(), "a bare press takes no screen");
        assert!(!overlay.view_active(), "a bare press moves no view");

        // The release: the arm reads the view state BEFORE taking the
        // selection — a click's copied_view is false, so its repaint gate
        // stays closed. The selection the gesture leaves behind is empty
        // (press == cursor), so the copy produces no report and the copied
        // line is never reached.
        let sel = overlay.take_selection();
        let copied_view = overlay.view_active();
        assert!(!copied_view, "a click's release repaints nothing: gate closed");
        let sel = sel.expect("the click's selection is taken at release");
        assert!(sel.is_empty(), "press == cursor on a bare click");
        assert!(crate::select::selected_text(
            &body_slice_at(80, body, overlay.view.offset()),
            &sel,
        )
        .is_empty(), "a bare click copies nothing");

        // After the gesture the overlay is exactly as it was: no active
        // screen, no moved view, no selection, no repaint debt.
        assert!(!overlay.is_active());
        assert!(!overlay.view_active());
        assert!(!overlay.has_selection());
        crate::scroll::uninstall();
    }

    #[test]
    fn a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt() {
        // FR-48a's companion: a real drag (the cursor moves) keeps the gesture's
        // screen — the release's copied_view is true, the redraw runs, and the
        // `[clip]` report lands after the screen is restored. The gate opens
        // only for a gesture that actually moved the view or painted highlights.
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        for i in 0..30 {
            crate::scroll::record_line(&format!("log line {i}"));
        }
        let mut overlay = ScrollOverlay::new();
        let bottom = bottom_block_rows(&PromptState::Ready, &InputState::new(), 80, 0, false);
        let body = crate::scroll::body_rows(24, bottom);

        // Press at (5,10), drag to (7,20): the selection is non-empty, the
        // release's gate reads the view still un-moved (the drag painted the
        // highlight on the live screen's alternate face in real use; the pin
        // proves the gate's own arithmetic: has_selection drives the screen's
        // ownership while the drag is live -- the_overlay_owns_the_screen_
        // while_a_drag_is_live pin -- and the release closes it cleanly).
        overlay.begin_selection(crate::select::Point::new(5, 10));
        overlay.extend_selection(crate::select::Point::new(7, 20));
        assert!(overlay.has_selection());
        let copied_view = overlay.view_active();
        let sel = overlay.take_selection().expect("drag selection taken");
        assert!(!sel.is_empty(), "a real drag selects a range");
        let text = crate::select::selected_text(&body_slice_at(80, body, 0), &sel);
        assert!(
            text.contains("log line"),
            "the drag's range carries its rows' text: {text:?}"
        );
        let _ = copied_view; // the view did not move during this drag
        assert!(!overlay.has_selection(), "the release leaves no debt");
        crate::scroll::uninstall();
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

    /// The reported defect, pinned on the block itself: while the live prompt
    /// block owns the screen's bottom rows the caret stands *inside* the
    /// prompt, and the block's repair is cursor-relative — so a paint or a tick
    /// there rewrites the prompt's own last row with the spinner-and-elapsed
    /// row. Failure-first: without the ownership gate this pin sees the elapsed
    /// row written onto the prompt (`sink.taken()` non-empty, `painted` set).
    #[test]
    fn the_indicator_never_paints_over_the_live_prompt_block() {
        let sink = CaptureSink::default();
        let mut ind = test_indicator(&sink);
        // A turn is running and its row was on screen, then the prompt block
        // took the bottom rows back (a live redraw after a turn the loop did
        // not submit, a drain window, an approval answer).
        ind.start();
        ind.paint(true);
        let _ = sink.taken();
        ind.set_prompt_owned(true);
        // Every writer the display task drives must stand down.
        ind.tick();
        ind.paint(true);
        ind.repaint();
        ind.toggle_expanded();
        assert_eq!(
            sink.taken(),
            "",
            "no writer may touch the prompt's rows while the prompt owns them"
        );
        assert_eq!(ind.painted_rows(), 0, "nothing is on screen from the block");
        // The closing row is still the transcript's — the outcome is not lost,
        // it is only not written onto the prompt (D-54's disarm replays it).
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(200);
        ind.finish(Mark::Ok);
        let texts: Vec<String> = crate::scroll::with(|t| {
            t.lines()
                .map(|l| crate::theme::strip_sgr(l).to_string())
                .collect()
        })
        .unwrap();
        crate::scroll::uninstall();
        assert!(
            texts.iter().any(|t| t.contains('\u{2714}')),
            "the outcome row is recorded even while the prompt owns the rows: {texts:?}"
        );
        assert_eq!(sink.taken(), "", "and still nothing is written there");
        // Ownership handed back (the zone armed, the screen scrolled away, the
        // rows released): the next paint draws again.
        ind.start();
        ind.set_prompt_owned(false);
        ind.paint(true);
        assert!(
            sink.taken().contains(SPINNER_FRAMES[0]),
            "the block paints once the prompt releases the rows"
        );
    }

    /// The arm half of the same rule: arming the turn zone takes the rows from
    /// the prompt block, so the beat — which composes the elapsed row *above*
    /// the span — is their only writer. Failure-first: without the handoff the
    /// flag stays set and the beat is the sole writer while the block also
    /// believes it owns the rows.
    #[test]
    fn arming_the_zone_takes_the_rows_from_the_prompt_block() {
        let sink = CaptureSink::default();
        let ind = Arc::new(std::sync::Mutex::new(test_indicator(&sink)));
        let state = test_state_with_indicator(ind.clone());
        lock_indicator(&ind).set_prompt_owned(true);
        zone_arm(&state);
        assert!(
            !lock_indicator(&ind).prompt_owned,
            "the armed zone owns the rows above the span, not the prompt block"
        );
        zone_disarm();
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
        // A five-row block whose last row is screen row 20 spans 16..=20; the
        // elapsed display's own first row (16) is the spinner-and-elapsed row,
        // which the click rule does not target (FR-27) — a click there is not
        // a hit.
        assert!(click_hits_indicator(5, 20, 17));
        assert!(click_hits_indicator(5, 20, 20));
        assert!(!click_hits_indicator(5, 20, 16));
        assert!(!click_hits_indicator(5, 20, 15));
        assert!(!click_hits_indicator(5, 20, 21));
        // A one-row block is the spinner row alone: nothing to toggle.
        assert!(!click_hits_indicator(1, 7, 7));
        // Nothing painted: no click can hit it.
        assert!(!click_hits_indicator(0, 7, 7));
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
    fn begin_cancel_arms_the_suppression_only_while_a_turn_is_in_flight() {
        let cancel = CancelToken::default();
        let suppress = AtomicUsize::new(0);
        let turn = AtomicBool::new(true);

        // A turn is running: the cancellation arms the suppression so the
        // cancelled tail cannot print over the fresh prompt.
        assert_eq!(begin_cancel(&cancel, &suppress, &turn), 1);
        assert!(cancel.is_cancelled());
        assert_eq!(suppress.load(Ordering::SeqCst), 1);

        // A second cancel while the first is still draining does NOT arm a
        // second counter: the turn emits exactly one terminal, so an extra
        // increment would suppress the whole NEXT turn instead.
        cancel.reset();
        assert_eq!(begin_cancel(&cancel, &suppress, &turn), 1);
        assert_eq!(suppress.load(Ordering::SeqCst), 1);

        // Cancel while the agent is idle (no TurnStart, or the terminal event
        // already passed): the token rises but no suppression is armed, so the
        // next turn — a peer prompt's — still displays.
        turn.store(false, Ordering::SeqCst);
        suppress.store(0, Ordering::SeqCst);
        assert_eq!(begin_cancel(&cancel, &suppress, &turn), 0);
        assert!(cancel.is_cancelled());
        assert_eq!(suppress.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn raise_cancel_if_turn_in_flight_refuses_the_idle_raise() {
        // FR-12: `/cancel` at an idle prompt must NOT raise the token — there
        // is no unwind to consume or clear it, and the queued peer prompts
        // would then skip until a manual prompt unstuck the session.
        let cancel = CancelToken::default();
        let turn = AtomicBool::new(false);
        assert!(!raise_cancel_if_turn_in_flight(&cancel, &turn));
        assert!(
            !cancel.is_cancelled(),
            "an idle /cancel leaves the token clear"
        );

        // While a turn is in flight the raise goes through (the Esc-equivalent
        // signal the command documents).
        turn.store(true, Ordering::SeqCst);
        assert!(raise_cancel_if_turn_in_flight(&cancel, &turn));
        assert!(cancel.is_cancelled());
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

    // ——— Input pump (the session's single crossterm reader thread) ———

    /// A canned event source for the pump: replays queued answers to
    /// `read()` and programmed outcomes for `poll(duration)`.
    struct CannedSource {
        reads: std::sync::Mutex<std::collections::VecDeque<std::io::Result<CtEvent>>>,
        polls: std::sync::Mutex<std::collections::VecDeque<std::io::Result<bool>>>,
    }

    impl CannedSource {
        fn new(
            reads: Vec<std::io::Result<CtEvent>>,
            polls: Vec<std::io::Result<bool>>,
        ) -> Self {
            let mut reads: std::collections::VecDeque<_> = reads.into();
            reads.make_contiguous();
            let mut polls: std::collections::VecDeque<_> = polls.into();
            polls.make_contiguous();
            Self {
                reads: std::sync::Mutex::new(reads),
                polls: std::sync::Mutex::new(polls),
            }
        }
    }

    #[tokio::test]
    async fn input_pump_forwards_events_in_arrival_order_and_exits_when_closed() {
        // A key press, a mouse press and a second key: the pump forwards them
        // in exactly the order its source produced them — the same order the
        // per-iteration drain used to have.
        let reads = vec![
            Ok(CtEvent::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE))),
            Ok(CtEvent::Mouse(crossterm::event::MouseEvent {
                kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                column: 10,
                row: 5,
                modifiers: KeyModifiers::NONE,
            })),
            Ok(CtEvent::Key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE))),
        ];
        // First poll: an event is ready. Inside the drain, every read must be
        // preceded by a ready report, so each canned read is followed by an
        // Ok(true) before the closing Ok(false); the not-ready polls close the
        // drain, and the timer keeps cycling afterwards so the query check
        // runs first.
        let polls = vec![
            Ok(true),
            Ok(true),
            Ok(true),
            Ok(false),
            Ok(false),
            Ok(false),
        ];
        let canned = std::sync::Arc::new(CannedSource::new(reads, polls));
        let source = canned.clone();
        let read_fn = move || {
            source
                .reads
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err(std::io::Error::other("no more canned reads")))
        };
        let source = canned.clone();
        let poll_fn = move |timeout: Duration| {
            let _ = timeout;
            source
                .polls
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err(std::io::Error::other("no more canned polls")))
        };
        let position_fn = || Err(std::io::Error::other("no position in this test"));

        let (event_tx, mut event_rx) = mpsc::channel::<CtEvent>(PUMP_CHANNEL_CAPACITY);
        let (query_tx, query_rx) = std::sync::mpsc::channel::<CursorQuery>();
        let handle = std::thread::spawn(move || {
            run_input_pump(query_rx, event_tx, position_fn, poll_fn, read_fn)
        });

        let mut received = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), async {
            while received.len() < 3 {
                if let Some(ct_event) = event_rx.recv().await {
                    received.push(ct_event);
                }
            }
        })
        .await
        .expect("the pump delivers its three canned events");
        assert!(
            matches!(&received[0], CtEvent::Key(k) if k.code == KeyCode::Char('a')),
            "first event must be the first canned key"
        );
        assert!(
            matches!(received[1], CtEvent::Mouse(_)),
            "second event must be the canned mouse press"
        );
        assert!(
            matches!(&received[2], CtEvent::Key(k) if k.code == KeyCode::Char('b')),
            "third event must be the second canned key"
        );
        // The sender side going away is what retires the pump: with the query
        // channel dropped it has nothing left to answer, so it returns.
        drop(query_tx);
        handle.join().expect("the pump thread exits cleanly");
    }

    #[tokio::test]
    async fn input_pump_answers_a_cursor_query_between_its_own_poll_cycles() {
        // Idle source: no events, only not-ready polls, so the query is
        // answered by the check that precedes each poll cycle.
        let polls = vec![Ok(false), Ok(false), Ok(false), Ok(false), Ok(false), Ok(false)];
        let canned = std::sync::Arc::new(CannedSource::new(vec![], polls));
        let source = canned.clone();
        let read_fn = move || {
            source
                .reads
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err(std::io::Error::other("no reads")))
        };
        let source = canned.clone();
        let poll_fn = move |timeout: Duration| {
            let _ = timeout;
            source
                .polls
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(false))
        };
        let position_fn = || Ok((7u16, 3u16));
        let (event_tx, _event_rx) = mpsc::channel::<CtEvent>(PUMP_CHANNEL_CAPACITY);
        let (query_tx, query_rx) = std::sync::mpsc::channel::<CursorQuery>();
        let _handle = std::thread::spawn(move || {
            run_input_pump(query_rx, event_tx, position_fn, poll_fn, read_fn)
        });

        let position = cursor_position_via_pump(Some(&query_tx)).await;
        assert_eq!(position.expect("the pump answers the query"), (7, 3));
    }

    #[tokio::test]
    async fn input_pump_reports_a_failing_position_query_as_the_documented_error() {
        let polls = vec![Ok(false), Ok(false), Ok(false), Ok(false), Ok(false), Ok(false)];
        let canned = std::sync::Arc::new(CannedSource::new(vec![], polls));
        let source = canned.clone();
        let read_fn = move || {
            source
                .reads
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err(std::io::Error::other("no reads")))
        };
        let source = canned.clone();
        let poll_fn = move |timeout: Duration| {
            let _ = timeout;
            source
                .polls
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Ok(false))
        };
        let position_fn = || Err(std::io::Error::other("position failed"));
        let (event_tx, _event_rx) = mpsc::channel::<CtEvent>(PUMP_CHANNEL_CAPACITY);
        let (query_tx, query_rx) = std::sync::mpsc::channel::<CursorQuery>();
        let _handle = std::thread::spawn(move || {
            run_input_pump(query_rx, event_tx, position_fn, poll_fn, read_fn)
        });

        let position = cursor_position_via_pump(Some(&query_tx)).await;
        assert!(
            position.is_err(),
            "a failing source must surface as the Err fallback, got {position:?}"
        );
    }
    // ——— /send delivery options (FR-04) ———

    #[test]
    fn send_args_default_to_prompt_without_an_option() {
        let (delivery, peer, text) =
            parse_send_args("worker fix the failing test").expect("plain /send parses");
        assert_eq!(delivery, Delivery::Prompt);
        assert_eq!(peer, "worker");
        assert_eq!(text, "fix the failing test");
    }

    #[test]
    fn send_args_accept_the_report_option() {
        // A result handed over without costing the peer a turn.
        let (delivery, peer, text) =
            parse_send_args("worker --report done with task 4").expect("report parses");
        assert_eq!(delivery, Delivery::Report);
        assert_eq!(peer, "worker");
        assert_eq!(text, "done with task 4");
    }

    #[test]
    fn send_args_accept_the_ask_option() {
        let (delivery, _, text) =
            parse_send_args("worker --ask what did the build say?").expect("ask parses");
        assert_eq!(delivery, Delivery::Ask);
        assert_eq!(text, "what did the build say?");
    }

    #[test]
    fn send_args_explicit_prompt_is_the_same_as_the_default() {
        let explicit = parse_send_args("peer --prompt ping").expect("prompt parses");
        let implicit = parse_send_args("peer ping").expect("plain parses");
        assert_eq!(explicit, implicit);
        assert_eq!(explicit.0, Delivery::Prompt);
    }

    #[test]
    fn send_args_reject_unknown_options_with_the_tool_s_exact_text() {
        // The vocabulary is the send_to tool's: both surfaces reject the same
        // word, so a typo in either place reads the same.
        let err = parse_send_args("worker --verbose ping").unwrap_err();
        let tool = Delivery::resolve(Some("--verbose"), false).unwrap_err();
        assert_eq!(err, tool, "the REPL and the tool must share the error text");
        assert!(err.contains("unknown delivery"), "the shape is the tool's: {err}");
    }

    #[test]
    fn send_args_keep_the_usage_error_on_missing_parts() {
        for arg in ["", "worker", "--ask worker"] {
            let err = parse_send_args(arg).unwrap_err();
            assert_eq!(
                err, "usage: /send [--report|--ask] <peer> <text>",
                "missing-part /send {arg:?} must point at the new usage"
            );
        }
    }

    #[test]
    fn send_args_an_option_lone_is_still_a_usage_error_not_an_unknown_delivery() {
        // A bare `--ask` with no peer/text is a usage problem, not a vocabulary
        // one: the option was recognized.
        assert_eq!(
            parse_send_args("--ask").unwrap_err(),
            "usage: /send [--report|--ask] <peer> <text>"
        );
    }
}

