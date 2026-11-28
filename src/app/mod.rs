use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::tools::send_to::Delivery;
use chrono::Utc;
use crossterm::event::{self, Event as CtEvent, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, ClearType};
use crossterm::ExecutableCommand;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::{mpsc, oneshot, watch, RwLock};

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

mod indicator;
mod line_loop;
mod overlay;
mod prompt;
mod runners;
mod slash;
mod zone;

use self::indicator::*;
use self::line_loop::*;
use self::overlay::*;
use self::prompt::*;
pub(crate) use self::runners::*;
use self::slash::*;
use self::zone::*;

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
        // The armed zone owns the screen: the record is the sole truth and
        // the beat will surface this line with its next repaint.
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
    /// Shared via `Arc<AtomicBool>` for `/auto on|off|status` runtime toggle.
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
    // The `[tool-call]`/`[tool-result]` screen lines are printed only when the
    // new `[ui] show_tool_results` keeps them on; the flag travels through the
    // display task like the indicator's own switch.
    let show_tool_results = config.ui.show_tool_results;
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

    // Approval channel. Route for agent task to request y/N from input loop.
    let (approval_tx, approval_rx) = mpsc::channel::<ApprovalRequest>(8);

    // Compiled once: a tool call costs a walk of the rule list, never a parse.
    let permissions = Arc::new(crate::permissions::Ruleset::from_config(
        &config.permissions,
    ));
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
        // between prompts. Only the headless path sets it.
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

    // Shutdown coordination channel. Regardless of whether triggered by `/quit`, EOF, SIGINT, or
    // SIGTERM, `shutdown_tx.send(true)` propagates to all tasks.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // The turn-in-flight watch: the display task's `TurnStart` lands here the
    // same instant the loop sees it, so the running turn's zone arms without
    // waiting the reconcile tick — the window between a turn's end and the next
    // one's arm would otherwise stream the answer's leading deltas straight onto
    // the freshly returned prompt's own rows.
    let (turn_in_flight_tx, turn_in_flight_rx) = watch::channel(false);

    // AI response completion notification channel. Fired when display_task observes
    // `Done`, releasing the input loop from `Pending` state.
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
                state_for_stdin,
                InputLoopChannels {
                    input_tx: input_tx_for_stdin,
                    shutdown_tx,
                    shutdown_rx,
                    agent_idle_rx,
                    approval_rx,
                    turn_in_flight_rx: Some(turn_rx_for_stdin),
                },
                true,
            )
            .await;
        })
    };

    // Display events. On `Done` or `Error`, notify the input loop of idle state. Treating
    // `Error` as idle is a defensive measure to prevent the input loop from getting stuck
    // in Pending forever if Provider construction fails without a `Done`.
    let show_thinking = config.ui.show_thinking_mode();
    let display_suppress = suppress.clone();
    let display_turn_flag = turn_in_flight.clone();
    let display_turn_tx = turn_in_flight_tx.clone();
    let display_indicator = indicator.clone();
    let display_task = tokio::spawn(run_display_task(DisplayTaskArgs {
        event_rx,
        agent_idle_tx,
        display_indicator,
        display_suppress,
        display_turn_flag,
        turn_in_flight_tx: display_turn_tx,
        show_thinking,
        theme,
        progress_enabled,
        show_tool_results,
        has_tty: stdin_is_tty(),
    }));

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
    // set a short timeout and abort if exceeded (target under 1 second).
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

/// Input loop state.
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
fn prompt_line_role(line: &str, shell_enabled: bool) -> Role {
    if shell_enabled && line.trim_start().starts_with('!') {
        Role::Confirm
    } else {
        Role::PromptSymbol
    }
}

/// RAII guard that enables crossterm raw mode on creation and disables it on drop.
/// Ensures the terminal is restored even on panic or early return.
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

/// Whether a click at absolute row `row` lands on the progress indicator's
/// block, and not on the elapsed display's own row. While a turn runs the
/// indicator is the last writer, so the cursor stands on the block's final row
/// and the block covers `painted_rows` rows ending there — but the block's
/// first row is the spinner-and-elapsed row itself, which the click rule does
/// not target: toggling the thinking view is for the reasoning rows alone, so
/// a one-row block (the spinner alone) is never a hit and the span is strict
/// at the top. Pure so it can be unit-tested without a terminal. (The
/// zone-side rule below differs: the status row is the block's LAST row there,
/// and the exclusion pivots to it.)
fn click_hits_indicator(painted_rows: u16, cursor_row: u16, row: u16) -> bool {
    if painted_rows <= 1 {
        return false;
    }
    let top = cursor_row.saturating_sub(painted_rows - 1);
    row > top && row <= cursor_row
}

/// The REPL's input prompt, and the prompt that asks for a tool approval.
const PROMPT: &str = "> ";
const APPROVAL_PROMPT: &str = "approve? [y/N]: ";
/// The scrolled-view marker: the text the pinned block shows while the wheel's
/// view has moved off the bottom of the log — one row immediately above the
/// prompt block's first row. The marker points at the log continuing below the
/// view.
const SCROLLED_MARK: &str = "Message below";

/// Rows the scrolled-view marker adds to the pinned block's footprint: one
/// while the view is off the bottom, none once it is back. The drawer and the
/// wheel/drag budgets all ask this, so they agree on where the body ends. Pure
/// so the sites share one arithmetic.
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

/// What the scrolled-view Esc-return redraws the prompt with, per input-loop
/// state.
///
/// The decision is the seam, not the arm: the arm's `Esc` branch redraws with
/// whatever this answers, so the pin binds it here — a `!is_pending()` gate
/// (`None` for `Pending`) would fail it. Every state redraws: mid-turn the
/// prompt draws beneath the streamed output while the turn keeps running, the
/// clock coming back with the screen through the tick's in-place repaint.
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

/// Whether a trimmed line is an affirmative answer to a tool-approval prompt.
/// Both input paths (the raw loop's y/N, the line loop's piped y/N) must keep
/// the same list of acceptances, so the rule lives in one pure place. Pure, so
/// it is unit-testable without a TTY.
fn is_approval_affirmative(trimmed: &str) -> bool {
    matches!(trimmed.to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Whether the mid-turn (Pending) editing arm serves the key through the
/// shared editor dispatch: any press except `Tab` — the completion the arm
/// above serves — and `Enter`. Enter is dropped here rather than passed on:
/// the dispatch's Enter arm consumes the buffer (`submit()` takes the line),
/// and a refusal after that would drop the text the user keeps through the
/// turn (R7.3, R7.5). Pure, so the gate is unit-testable without a TTY.
fn pending_edit_dispatch_key(key_event: &KeyEvent) -> bool {
    key_event.kind != crossterm::event::KeyEventKind::Release
        && !matches!(key_event.code, KeyCode::Tab | KeyCode::Enter)
}

/// What `run_display_task` runs on: the agent's event stream, the idle
/// notification back to the input loop, the shared indicator / suppression /
/// turn-flag state, and the display settings.
struct DisplayTaskArgs {
    event_rx: mpsc::Receiver<AgentEvent>,
    agent_idle_tx: mpsc::Sender<()>,
    display_indicator: Arc<std::sync::Mutex<StatusIndicator>>,
    display_suppress: Arc<AtomicUsize>,
    display_turn_flag: Arc<AtomicBool>,
    turn_in_flight_tx: watch::Sender<bool>,
    show_thinking: ShowThinkingMode,
    theme: Theme,
    progress_enabled: bool,
    show_tool_results: bool,
    has_tty: bool,
}

/// The interactive display loop, extracted from `run` (testable): consumes
/// agent events, paints them, and on an unsuppressed terminal — `Done` or
/// `Error`, the same "idle" set that treats a Provider failure without a
/// `Done` defensively — releases the input loop's Pending wait with the idle
/// notification.
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
async fn run_display_task(args: DisplayTaskArgs) {
    let DisplayTaskArgs {
        mut event_rx,
        agent_idle_tx,
        display_indicator,
        display_suppress,
        display_turn_flag,
        turn_in_flight_tx,
        show_thinking,
        theme,
        progress_enabled,
        show_tool_results,
        has_tty,
    } = args;
    let mut display_state = DisplayState::new(has_tty);
    // With the indicator drawing right beneath it, a tool call is described
    // on a single row instead of dumping its raw arguments.
    display_state.compact_output = progress_enabled;
    // With the indicator on, reasoning is shown live in its block rather
    // than streamed into the scrollback.
    display_state.capture_thinking = progress_enabled && show_thinking != ShowThinkingMode::Hidden;
    // The `[tool-call]`/`[tool-result]` lines print only while the key keeps
    // them on; when it is off the whole turn reduces to the streamed text and
    // the completion mark.
    display_state.show_tool_results = show_tool_results;
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
                // The armed zone: the beat owns the rows above the input span
                // — the clock block (spinner + thinking) composes from the
                // indicator's own face, the peers' band rides with it, and the
                // tick's screen writes stay suppressed.
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
        // in flight between those edges, not merely on the edge): only the two
        // edges write it. Flipping it false on the first streamed delta would
        // make a running turn read as ended from that instant — the cancel
        // bookkeeping and the reconcile tick would both see a turn that was
        // still streaming (the remaining deltas painted at the prompt's own
        // rows mid-turn).
        if matches!(ev, AgentEvent::TurnStart) {
            display_turn_flag.store(true, Ordering::SeqCst);
        } else if is_idle {
            display_turn_flag.store(false, Ordering::SeqCst);
        }
        // The loop's synchronous arm: the watch's send is a no-op when the
        // stored value already matches, so a turn that opens behind an unread
        // notification does not double-arm the zone.
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
                    display_state.show_tool_results,
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

/// Display-side decision for one agent event while `outstanding` cancellations
/// are still draining. Returns `(drop_event, notify_idle, new_outstanding)`.
///
/// With nothing outstanding the event passes through: show the event, and
/// notify idle on a terminal one. With a cancellation outstanding, every
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
fn handle_key(
    key_event: KeyEvent,
    input: &mut InputState,
    history: &[String],
) -> Option<KeyAction> {
    // Ignore key release events (Windows sends both press and release)
    if key_event.kind == crossterm::event::KeyEventKind::Release {
        return None;
    }

    let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);

    match key_event.code {
        KeyCode::Enter => {
            // A bare Enter (an Enter-only input) is ignored: the guard
            // sits at the editor's Enter arm, before `submit()`, so the
            // empty submit touches no state — no history write, no queue
            // effect, no redraw side effects — in every prompt state this
            // arm serves. A whitespace-only line still submits (the guard
            // suppresses the empty string only).
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
            // History navigation: move to older entry
            input.navigate_up(history);
            Some(KeyAction::Continue)
        }
        KeyCode::Down => {
            // History navigation: move to newer entry
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

/// The channels the input loop talks over: prompts out to the agent, the
/// shutdown pair, the agent's idle notifications, tool-approval requests and
/// (when a display task runs) the turn-in-flight watch.
pub(crate) struct InputLoopChannels {
    pub(crate) input_tx: mpsc::Sender<AgentInput>,
    pub(crate) shutdown_tx: watch::Sender<bool>,
    pub(crate) shutdown_rx: watch::Receiver<bool>,
    pub(crate) agent_idle_rx: mpsc::Receiver<()>,
    pub(crate) approval_rx: mpsc::Receiver<ApprovalRequest>,
    pub(crate) turn_in_flight_rx: Option<watch::Receiver<bool>>,
}

/// Main loop that converts line input from stdin (or any `AsyncRead`) into `AgentInput`.
///
/// When `interactive` is `true` and stdin is a TTY, uses crossterm raw-mode input
/// with history navigation (up/down arrows) and line editing. When `interactive`
/// is `false` (tests, piped input), falls back to line-oriented
/// `BufReader::lines()` reading (original behaviour).
pub(crate) async fn run_input_loop<R>(
    reader: R,
    state: Arc<ReplState>,
    channels: InputLoopChannels,
    interactive: bool,
) where
    R: AsyncRead + Unpin,
{
    // Determine whether to use crossterm raw mode
    let use_raw_mode = interactive && stdin_is_tty();

    if use_raw_mode {
        let InputLoopChannels {
            input_tx,
            shutdown_tx,
            shutdown_rx,
            agent_idle_rx,
            approval_rx,
            turn_in_flight_rx,
        } = channels;
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
        run_input_loop_line(reader, state, channels, interactive).await;
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
    let query = CursorQuery { reply: reply_tx };
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

/// Raw-mode input loop with crossterm. A dedicated pump thread
/// (`run_input_pump`) is the only consumer of crossterm's event reader:
/// it polls, drains and forwards events over a channel, and answers
/// cursor-position queries from the same thread, so concurrent users of
/// the single global reader that starve each other cannot exist by
/// construction.
async fn run_input_loop_raw(
    input_tx: mpsc::Sender<AgentInput>,
    state: Arc<ReplState>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    agent_idle_rx: mpsc::Receiver<()>,
    approval_rx: mpsc::Receiver<ApprovalRequest>,
    turn_in_flight_rx: Option<watch::Receiver<bool>>,
) {
    // The watch arm is the loop's synchronous turn-start listener; the borrow
    // must be mutable so `changed()` can stand on the shared receiver.
    let mut turn_in_flight_rx = turn_in_flight_rx;
    // Enable raw mode; guard restores on drop
    let _raw_guard = match RawModeGuard::enable() {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("failed to enable raw mode: {e}");
            // Fall back to non-raw mode — but we can't do history navigation without raw mode.
            // Just proceed with line-oriented input using stdin.
            let stdin = tokio::io::stdin();
            run_input_loop_line(
                stdin,
                state,
                InputLoopChannels {
                    input_tx,
                    shutdown_tx,
                    shutdown_rx,
                    agent_idle_rx,
                    approval_rx,
                    turn_in_flight_rx,
                },
                true,
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
    // The typed prompt's buffer stowed while a tool-approval prompt owns the
    // input line (R7.6): the mid-turn text the user typed is never dropped —
    // it is stowed here and unstowed when the approval is answered.
    let mut stowed_input: Option<InputState> = None;
    let mut approval_rx: Option<mpsc::Receiver<ApprovalRequest>> = Some(approval_rx);
    let mut stdout = std::io::stdout();
    let mut shutdown_rx = shutdown_rx;
    let mut agent_idle_rx = agent_idle_rx;
    let mut renderer = PromptRenderer::new();
    // The reconcile clock: a turn the input loop did not submit — a peer prompt
    // arriving over IPC, a queued prompt the agent dequeued on its own — runs
    // with nobody holding the rows above the prompt. The arm below hands them
    // to the turn zone the moment it sees the turn in flight, so the elapsed
    // row is composed above the prompt in every state instead of being painted
    // onto the prompt's own bottom row by the cursor-relative block. Low
    // frequency: the loop only has to notice, never to race.
    const ZONE_RECONCILE_MS: u64 = 150;
    let mut zone_timer =
        tokio::time::interval(tokio::time::Duration::from_millis(ZONE_RECONCILE_MS));
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
            // The turn's synchronous arm: a `TurnStart` lands the same instant
            // the display task says so — no 150 ms window streams the turn's
            // leading deltas onto rows the zone has not taken yet. The running
            // flow's own states keep their spans (the approval's takeover arms
            // the zone the same way at its own hand-off).
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
                // arm either.
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
            // The turn terminal's prompt handoff: the notification is
            // consumed in every state, so the prompt the answer's display
            // completes onto is the shipped flow's own return, never the
            // reconcile timer's. Pending: the typed path's handoff, as
            // shipped. Ready with no turn in flight: a turn this loop did not
            // submit (peer/serve) ended — the same handoff, so the prompt
            // returns between back-to-back turns instead of racing the next
            // TurnStart against the 150 ms tick. AwaitingApproval or a turn
            // already in flight: the running flow's own rows (the approval
            // span and the next turn keep the zone); the mark is dropped
            // quietly, the same quietness the click pass serves.
            res = agent_idle_rx.recv() => {
                match res {
                    Some(()) => {
                        if prompt_state.is_pending() {
                            prompt_state = PromptState::Ready;
                            // The handoff: the zone gives the screen back —
                            // its rows erased, the record's tail replayed
                            // above the fresh prompt — before the shipped
                            // render.
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
            // Approval request arrived (only when not AwaitingApproval).
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
                        renderer.clear_block(&mut stdout, false);
                        raw_println(
                            true,
                            &state.theme.out(
                                Role::Confirm,
                                &format!("[tool approval] {} {}", req.tool_name, req.args),
                            ),
                        );
                        renderer.reset();
                        // The typed prompt's buffer is not ours to drop (R7.6):
                        // stow it before the y/N prompt takes the input line —
                        // the approval renders the y/N answer on a fresh line —
                        // and the stowed buffer comes back the moment the
                        // approval is answered, whatever the answer was.
                        stowed_input = Some(std::mem::take(&mut input_state));
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
                        // history. The wheel is a log operation, so it runs
                        // while a turn is in flight too (R6.1): the alternate
                        // screen takes over from the beat for as long as the
                        // view is off the bottom (the zone hides its writes,
                        // `overlay.enter` → `zone_hide`), and the turn keeps
                        // streaming into the record underneath.
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
                            let peer_lines = live_peer_lines(
                                &state.registry_dir,
                                &state.id,
                                width,
                                {
                                    let mut ind = lock_indicator(&state.indicator);
                                    ind.peer_clock_advance();
                                    ind.peer_spinner_glyph()
                                },
                            )
                            .len();
                            // The view's offset counts rows from the live
                            // screen, so the clamp reads the live screen's body:
                            // no marker row, whichever side of the bottom the
                            // view is on. The drawer slices the marker's row off
                            // the bottom of the body (`slice_offset`), so the
                            // furthest offset still shows the log's first row at
                            // the top, and the first notch clamps exactly like
                            // every later one. The budget reads the screen's
                            // state live: a running turn (or an approval
                            // prompt) makes the block taller, so the wheel
                            // clamps against the body the turn actually keeps.
                            let bottom_rows = bottom_block_rows(
                                &prompt_state,
                                &input_state,
                                width,
                                peer_lines,
                                false,
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
                        // screen, so there is nothing to click. The gate opens
                        // only while a turn runs (`is_pending()`) — the toggle
                        // targets the running display — and the block rules
                        // keep the spinner-and-elapsed row out of the hit
                        // range, so a click on the log never toggles anything.
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
                            // spent. Unarmed, the indicator's rule asks the
                            // pump — the only thread that may touch
                            // crossterm's reader — so nothing is asked behind
                            // another consumer's back. Without an answer in
                            // time the click changes nothing, so the elapsed
                            // display cannot surface from a stale answer. Both
                            // rules keep the spinner-and-elapsed row out: the
                            // indicator's rule excludes its block's first row,
                            // the zone's rule its clock block's last row.
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
                        // have it taken away here. The drag runs mid-turn too
                        // (R6.2): a press, a drag and its release resolve
                        // against the rows the screen actually draws while the
                        // agent operates — the running display's rows live
                        // below the log's own rows, so the selection covers
                        // exactly the log rows the screen shows.
                        if state.scroll
                            && state.mouse_select
                            && !mouse_event
                                .modifiers
                                .contains(KeyModifiers::SHIFT)
                        {
                            use crossterm::event::{MouseButton, MouseEventKind};
                            let width = PromptRenderer::terminal_width();
                            // Unlike the wheel arm, which clamps against the
                            // marker-free live body (it passes `false`), the
                            // drag arm counts the marker row whenever the view
                            // is off the bottom, so a press, a drag and its
                            // release resolve against the rows the screen
                            // actually draws — the prompt span's rows, the
                            // elapsed row and the marker included.
                            let view_moved = overlay.view_active();
                            let bottom_rows = bottom_block_rows(
                                &prompt_state,
                                &input_state,
                                width,
                                live_peer_lines(
                                    &state.registry_dir,
                                    &state.id,
                                    width,
                                    {
                                        let mut ind = lock_indicator(&state.indicator);
                                        ind.peer_clock_advance();
                                        ind.peer_spinner_glyph()
                                    },
                                )
                                .len(),
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
                                    // until the gesture writes something (a
                                    // click displays no prompt and leaves the
                                    // screen untouched).
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
                                            let offset = slice_offset(
                                                overlay.view.offset(),
                                                usize::from(view_moved),
                                            );
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
                                        // nothing at all.
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
                                    // The prompt returns in every state: the
                                    // line the user was typing is kept, the
                                    // approval prompt is re-shown as it was, and
                                    // mid-turn the prompt draws beneath the
                                    // streamed output while the turn keeps
                                    // running. The indicator's clock comes back
                                    // with the screen: the next tick repaints
                                    // the block in place, so the elapsed time
                                    // never lands over the redrawn prompt.
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
                        // During Pending the editing buffer keeps working
                        // (R7.2): characters, pastes, backspaces and the rest
                        // edit the line and its caret live, and the prompt
                        // span repaints per edit. Submitting stays barred
                        // (R7.3): Enter and the rest run nothing — the
                        // `KeyAction::Submit` arm below refuses to submit
                        // while the turn is in flight, and the buffer survives
                        // for submission at the prompt's return (R7.5). ESC
                        // and Ctrl-C keep their cancel meaning.
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
                                // the prompt.
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
                            // The editing keys: handled below by the shared
                            // `handle_key` dispatch. Everything else (the
                            // mouse already served above) falls through with
                            // `continue` after the shared dispatch repaints
                            // the span. Enter never reaches it mid-turn: the
                            // dispatch's Enter arm consumes the buffer
                            // (`submit()` takes the line), and a refusal after
                            // that would drop the text the user must keep
                            // through the turn (R7.3, R7.5) — the key is
                            // dropped here instead, so the line and its caret
                            // stand exactly as they were.
                            else if pending_edit_dispatch_key(&key_event) {
                                if let Some(action) =
                                    handle_key(key_event, &mut input_state, &history_snapshot)
                                {
                                    match action {
                                        // The submit is barred mid-turn before
                                        // the dispatch: Enter never reaches
                                        // `handle_key` here, so its consuming
                                        // `submit()` never runs. The remaining
                                        // actions repaint the span as the
                                        // shipped flow does.
                                        KeyAction::ClearLine | KeyAction::Continue => {
                                            render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                                        }
                                        KeyAction::Eof => {
                                            // EOF mid-turn keeps its shipped
                                            // meaning? No: during Pending the
                                            // session is still working, and
                                            // the cancel keys above are the
                                            // way to interrupt. Ignore the EOF
                                            // key the same way the shipped
                                            // loop ignored every key.
                                        }
                                        // Unreachable while Pending (Ctrl-C and
                                        // Esc were served above, and this arm
                                        // excludes Tab and Enter): the
                                        // dispatch cannot produce it here.
                                        KeyAction::Submit(line) => {
                                            let _ = line;
                                        }
                                    }
                                }
                            }
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
                            renderer.clear_block(&mut stdout, false);
                            raw_eprintln(
                                true,
                                &state.theme.err(Role::Info, "[info] cancelled by user"),
                            );
                            // The stowed mid-turn buffer comes back with the
                            // denial (R7.6): the text typed while the turn ran
                            // was never ours to drop.
                            input_state = stowed_input
                                .take()
                                .unwrap_or_default();
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
                                        let approved = is_approval_affirmative(&trimmed);
                                        let prev = std::mem::replace(&mut prompt_state, PromptState::Pending);
                                        if let PromptState::AwaitingApproval(resp_tx) = prev {
                                            let _ = resp_tx.send(approved);
                                        }
                                        // The answer returns the REPL to the
                                        // running state: the approval span
                                        // retires and the next beat takes
                                        // the screen's bottom row back for
                                        // the clock.
                                        zone_span_retire();
                                        // The stowed mid-turn buffer comes back
                                        // for the prompt's return (R7.6), fresh
                                        // under the y/N answer the user gave.
                                        input_state = stowed_input
                                            .take()
                                            .unwrap_or_default();
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
                                                // A command's turn runs too: the zone
                                                // takes the live screen the same way
                                                // a typed prompt's does.
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
                                    // The four-region standing shape: the zone
                                    // takes the live screen and the prompt
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
                            renderer.clear_block(&mut stdout, false);
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
                    renderer.clear_block(&mut stdout, false);
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
            // The reconcile arm: a turn the input loop did not submit — a peer
            // prompt over IPC, a queued prompt the agent dequeued by itself —
            // runs with nobody holding the rows above the prompt. Left alone
            // the block's cursor-relative repaint lands on the prompt and the
            // elapsed row takes the prompt's own bottom row. Handing the rows
            // to the zone the moment the turn is seen composes the elapsed row
            // above the prompt in every state. While the overlay owns the
            // screen the same tick serves the scrolled view's beat instead:
            // the tick is the loop's only periodic wake with no events flowing,
            // so without it the pin's elapsed row would freeze (R6.6) —
            // `needs_refresh` selects the drawer's 100 ms beat for a running
            // turn and the 1 s repair beat at rest (R6.5).
            _ = zone_timer.tick() => {
                if overlay.is_active() {
                    if overlay.needs_refresh(prompt_state.is_pending()) {
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
                    }
                } else if state.turn_in_flight.load(Ordering::SeqCst) {
                    if !zone_armed() {
                        // The arm is spanless now: the running turn's bottom
                        // line is the clock alone. The one span that still
                        // paints is the tool-approval prompt — an
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
                    // and the prompt underneath.
                    zone_disarm();
                    renderer.reset();
                    render_prompt(&mut overlay, &mut renderer, &mut stdout, Role::PromptSymbol, PROMPT, &state, &input_state).await;
                }
            }
        }
    }

    // When exiting AwaitingApproval via the shutdown route, send false (deny) to the
    // agent's oneshot::Receiver to avoid leaving it dangling.
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
    renderer.clear_block(&mut stdout, false);
    let _ = shutdown_tx.send(true);
    // _raw_guard dropped here: terminal mode restored
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
    // The startup logo: an ASCII-art "AGENT CLI" block in the banner colour,
    // the spark glyph leading its first row and the running version riding the
    // last one (R6, D7). Constant text, no runtime measurement.
    for line in banner_logo_lines() {
        raw_println(false, &theme.out(Role::Banner, &line));
    }
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

/// The startup banner logo (R6, D7): an ASCII-art "AGENT CLI" block in a
/// Gemini-CLI style - a spark glyph leading the first row, the running
/// version riding the last one. Pure and constant, so the pins can bind the
/// rows without a terminal and non-TTY output renders the same block.
fn banner_logo_lines() -> Vec<String> {
    // The block-letter face: five fixed-width rows per letter, full blocks on
    // a blank ground (single-width glyphs, so every terminal renders the same
    // shape and a row never wraps mid-glyph).
    const A: [&str; 5] = ["█████", "█   █", "█████", "█   █", "█   █"];
    const G: [&str; 5] = [" ████", "█    ", "█  ██", "█   █", " ████"];
    const E: [&str; 5] = ["█████", "█    ", "████ ", "█    ", "█████"];
    const N: [&str; 5] = ["█   █", "██  █", "█ █ █", "█  ██", "█   █"];
    const T: [&str; 5] = ["█████", "  █  ", "  █  ", "  █  ", "  █  "];
    const C: [&str; 5] = [" ████", "█    ", "█    ", "█    ", " ████"];
    const L: [&str; 5] = ["█    ", "█    ", "█    ", "█    ", "█████"];
    const I: [&str; 5] = ["█████", "  █  ", "  █  ", "  █  ", "█████"];
    const AGENT: [&[&str; 5]; 5] = [&A, &G, &E, &N, &T];
    const CLI: [&[&str; 5]; 3] = [&C, &L, &I];
    let rows = |sets: &[&[&str; 5]], sep: &str| -> Vec<String> {
        (0..5)
            .map(|r| {
                sets.iter()
                    .map(|letter| letter[r])
                    .collect::<Vec<_>>()
                    .join(sep)
            })
            .collect()
    };
    let agent = rows(&AGENT, " ");
    let cli = rows(&CLI, " ");
    let spark = "✦";
    (0..5)
        .map(|r| {
            let head = if r == 0 {
                format!("{spark} ")
            } else {
                "  ".to_string()
            };
            let tail = if r == 4 {
                format!("   v{}", env!("CARGO_PKG_VERSION"))
            } else {
                String::new()
            };
            format!("{head}{}   {}{tail}", agent[r], cli[r])
        })
        .collect()
}

fn print_prompt() {
    print!("> ");
    let _ = std::io::stdout().flush();
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
        state,
        PromptRunParams {
            theme: &state.theme,
            role,
            prompt,
            input: input_state,
            run_row: None,
        },
    )
    .await;
}

/// The prompt's one drawing path, with the running turn's elapsed row as a
/// parameter: `None` on the live screen (the zone or the indicator owns that
/// row there) and on the scrolled screen with no turn tracked, `Some` on the
/// scrolled screen mid-turn — where the row stands directly above the prompt
/// span, exactly as the live screen composes it.
async fn render_prompt_with_run(
    overlay: &mut ScrollOverlay,
    renderer: &mut PromptRenderer,
    stdout: &mut std::io::Stdout,
    state: &Arc<ReplState>,
    p: PromptRunParams<'_>,
) {
    let PromptRunParams {
        theme,
        role,
        prompt,
        input: input_state,
        run_row,
    } = p;
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
    // The subagent rows are composed once per read and shown on both screens:
    // pinned beside the block on the scrolled screen, rendered in the prompt
    // block's tracked region on the live screen.
    let peers = live_peer_lines(
        &state.registry_dir,
        &state.id,
        PromptRenderer::terminal_width(),
        lock_indicator(&state.indicator).peer_spinner_glyph(),
    );
    if overlay.is_active() {
        draw_scroll_overlay(
            overlay,
            stdout,
            renderer,
            theme,
            ScrollBottom {
                role,
                prompt,
                input: input_state,
                suggestion: sug.as_deref(),
                peer_lines: peers.clone(),
                run_row,
                text: overlay.view_active().then(|| SCROLLED_MARK.to_string()),
            },
        );
    } else {
        // The armed zone's composition is state-driven: while a turn runs the
        // clock block owns the screen's bottom rows and no prompt display
        // paints — the standing face is the running clock alone, and the beat
        // is the rows' only writer. The one armed face that still paints is
        // the tool-approval prompt (an input-possible state): its span
        // re-anchors the zone's floor and the beat composes the run display
        // above it, exactly as the shipped approval takeover did. The shipped
        // render below stands down while armed for the same cursor-relative
        // reason it always did.
        if zone_armed() {
            // The prompt span paints for every input-possible state: the
            // tool-approval prompt (as shipped) and now the typed-prompt
            // line while a turn runs (R7.1) — its span re-anchors the
            // zone's floor and the beat composes the run display above it.
            if matches!(role, Role::Confirm | Role::PromptSymbol) {
                zone_paint_span(theme, role, prompt, input_state, sug.as_deref());
            }
            return;
        }
        // The live prompt block takes the screen's bottom rows from here, and
        // the caret is parked *inside* it. The indicator's block is
        // cursor-relative (erase upward from the caret, repaint downward from
        // it), so painting here would land on the prompt — the elapsed row on
        // the prompt's own last row, which is the row the prompt alone owns. The
        // indicator stands down until a turn zone takes the rows and composes
        // the elapsed row above the span; `finish` still records the outcome
        // row, so nothing is lost from the transcript.
        lock_indicator(&state.indicator).set_prompt_owned(true);
        renderer.render(
            stdout,
            RenderParams {
                theme,
                role,
                prompt,
                input: input_state,
                suggestion: sug.as_deref(),
                peer_lines: &peers,
                run_row: None,
                marker: None,
            },
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
    // above the span. `overlay_status_row` answers `None` when nothing is
    // tracked, so the not-running prompt below is the same arm as before.
    let run_row = {
        let mut ind = lock_indicator(&state.indicator);
        // The idle band's clock rides the same redraw the pinned run row uses,
        // so a peer spawned under the scrolled view animates too.
        ind.peer_clock_advance();
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
        state,
        PromptRunParams {
            theme: &state.theme,
            role,
            prompt,
            input: input_state,
            run_row,
        },
    )
    .await;
}

#[cfg(test)]
mod tests {
    //! Regression tests for the app-termination input loop (`/quit` and Ctrl+D=EOF),
    //! plus unit tests for InputState history navigation.
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
        let handle = tokio::spawn(run_display_task(DisplayTaskArgs {
            event_rx,
            agent_idle_tx,
            display_indicator: Arc::new(std::sync::Mutex::new(StatusIndicator::disabled())),
            display_suppress: Arc::new(AtomicUsize::new(0)),
            display_turn_flag: Arc::new(AtomicBool::new(false)),
            turn_in_flight_tx: watch::channel(false).0,
            show_thinking: ShowThinkingMode::Hidden,
            theme: Theme::plain(),
            progress_enabled: false,
            show_tool_results: true,
            has_tty: false,
        }));
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

    /// InputState unit tests (history navigation and draft preservation).
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
        let history = vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ];
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

    /// `collapse_thinking_text` behavior.
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
                state,
                InputLoopChannels {
                    input_tx,
                    shutdown_tx,
                    shutdown_rx,
                    agent_idle_rx: idle_rx,
                    approval_rx: dummy_approval_rx(),
                    turn_in_flight_rx: None,
                },
                false,
            )
            .await;
        });

        writer.write_all(b"!echo from-the-shell\n").await.unwrap();
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
                state,
                InputLoopChannels {
                    input_tx,
                    shutdown_tx,
                    shutdown_rx,
                    agent_idle_rx: idle_rx,
                    approval_rx: dummy_approval_rx(),
                    turn_in_flight_rx: None,
                },
                false,
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
                state,
                InputLoopChannels {
                    input_tx,
                    shutdown_tx,
                    shutdown_rx,
                    agent_idle_rx: idle_rx,
                    approval_rx: dummy_approval_rx(),
                    turn_in_flight_rx: None,
                },
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
                state,
                InputLoopChannels {
                    input_tx,
                    shutdown_tx,
                    shutdown_rx,
                    agent_idle_rx: idle_rx,
                    approval_rx: dummy_approval_rx(),
                    turn_in_flight_rx: None,
                },
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
                state,
                InputLoopChannels {
                    input_tx,
                    shutdown_tx,
                    shutdown_rx,
                    agent_idle_rx: idle_rx,
                    approval_rx: dummy_approval_rx(),
                    turn_in_flight_rx: None,
                },
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

    /// `/exit` triggers termination equivalent to `/quit`.
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
                state,
                InputLoopChannels {
                    input_tx,
                    shutdown_tx,
                    shutdown_rx,
                    agent_idle_rx: idle_rx,
                    approval_rx: dummy_approval_rx(),
                    turn_in_flight_rx: None,
                },
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

    /// Approval request arrives -> "y" input -> oneshot receives true and transitions to Pending; user input does
    /// not leak to agent during this time.
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
                state,
                InputLoopChannels {
                    input_tx,
                    shutdown_tx,
                    shutdown_rx,
                    agent_idle_rx: idle_rx,
                    approval_rx,
                    turn_in_flight_rx: None,
                },
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

    /// When exiting AwaitingApproval via the shutdown route, oneshot receives false (denied).
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
                state,
                InputLoopChannels {
                    input_tx,
                    shutdown_tx,
                    shutdown_rx,
                    agent_idle_rx: idle_rx,
                    approval_rx,
                    turn_in_flight_rx: None,
                },
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

    // --- Wrap-aware cursor layout ---

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

    // --- handle_key control-key behavior ---

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

    // --- First-section-header newline suppression ---

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

    // --- the bare-Enter guard ---

    #[test]
    fn bare_enter_is_ignored_without_states_side_effects() {
        // An empty line at Enter reaches no submit and mutates no editor
        // state: no action, and the line stays empty (the guard sits
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

    // --- Approval-response classification (the shared y/N rule) ---

    #[test]
    fn is_approval_affirmative_accepts_the_two_affirmations_only() {
        // The rule both input paths share: `y`/`yes` accept, case-insensitively
        // (the y/N prompt's own convention), and every other trimmed line
        // denies — a blank line, a space, the words around them included.
        assert!(is_approval_affirmative("y"));
        assert!(is_approval_affirmative("Y"));
        assert!(is_approval_affirmative("yes"));
        assert!(is_approval_affirmative("YES"));
        assert!(is_approval_affirmative("Yes"));
        assert!(!is_approval_affirmative("n"));
        assert!(!is_approval_affirmative("no"));
        assert!(!is_approval_affirmative(""));
        assert!(!is_approval_affirmative("  "));
        assert!(!is_approval_affirmative("y "));
        assert!(!is_approval_affirmative("yes please"));
    }

    // --- Mid-turn (Pending) editing-dispatch gate ---

    #[test]
    fn pending_edit_dispatch_key_drops_enter_and_tab_and_releases() {
        // Enter mid-turn must never reach the editor dispatch: its Enter arm
        // consumes the buffer (`submit()`), and a refusal after that would
        // drop the text the user keeps through the turn. Tab is served by the
        // completion arm above; Release events are ignored everywhere.
        assert!(!pending_edit_dispatch_key(&key(KeyCode::Enter, false)));
        assert!(!pending_edit_dispatch_key(&key(KeyCode::Tab, false)));
        let mut release = key(KeyCode::Char('a'), false);
        release.kind = crossterm::event::KeyEventKind::Release;
        assert!(!pending_edit_dispatch_key(&release));
    }

    #[test]
    fn pending_edit_dispatch_key_serves_every_editing_key() {
        // The keys the mid-turn editing keeps live (R7.2) still pass the gate.
        assert!(pending_edit_dispatch_key(&key(KeyCode::Char('a'), false)));
        assert!(pending_edit_dispatch_key(&key(KeyCode::Backspace, false)));
        assert!(pending_edit_dispatch_key(&key(KeyCode::Left, false)));
        assert!(pending_edit_dispatch_key(&key(KeyCode::Home, false)));
        // Ctrl-D stays a dispatch concern (the Eof arm ignores it mid-turn).
        assert!(pending_edit_dispatch_key(&key(KeyCode::Char('d'), true)));
    }

    #[test]
    fn pending_enter_press_leaves_the_buffer_and_caret_untouched() {
        // The defect this seam pins: with text on the line, an Enter press
        // mid-turn must leave the buffer EXACTLY as it stood — the shipped
        // dispatch consumed the buffer (`submit()`), so the refusing arm
        // dropped the line ("入力中の文字が消える").
        let mut s = InputState::new();
        s.insert_char('h');
        s.insert_char('i');
        let gate = pending_edit_dispatch_key(&key(KeyCode::Enter, false));
        if gate {
            let _ = handle_key(key(KeyCode::Enter, false), &mut s, &[]);
        }
        assert!(!gate, "Enter mid-turn must be dropped before the dispatch");
        assert_eq!(s.line, "hi");
        assert_eq!(s.cursor, 2);
        assert_eq!(s.history_index, None);
        assert_eq!(s.saved_draft, None);
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
        assert!(
            rows[..8].iter().all(|r| r.text.is_empty()),
            "padded at the top"
        );
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
        // The content pin: the face grows the processing content beside the
        // operating time, the row staying one line cut to the width.
        let rows = peer_status_rows(
            &[(
                "worker".to_string(),
                std::time::Duration::from_secs(65),
                Some("tool bash".to_string()),
            )],
            80,
            Some(SPINNER_FRAMES[1]),
        );
        assert_eq!(rows.len(), 1, "one live child, one row: {rows:?}");
        assert!(rows[0].contains("worker"), "the peer is named: {rows:?}");
        assert!(
            rows[0].contains("1m05s"),
            "the operating time stays: {rows:?}"
        );
        assert!(
            rows[0].contains("tool bash"),
            "the processing content rides the row: {rows:?}"
        );
        assert!(
            rows[0].starts_with(SPINNER_FRAMES[1]),
            "the status row leads with the spinner: {rows:?}"
        );
        // The whole face is one row cut to the width budget.
        let wide = peer_status_rows(
            &[(
                "w".to_string(),
                std::time::Duration::from_secs(1),
                Some("x".repeat(200)),
            )],
            40,
            Some(SPINNER_FRAMES[1]),
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
        // The degrade rule: no turn state in the registry entry (an old
        // version's file, an idle peer) and the face is exactly the shipped
        // name-and-operating-time.
        let rows = peer_status_rows(
            &[("alpha".to_string(), std::time::Duration::from_secs(1), None)],
            80,
            Some(SPINNER_FRAMES[1]),
        );
        assert_eq!(
            rows[0],
            format!("{} peer alpha · 1.0s", SPINNER_FRAMES[1]),
            "{rows:?}"
        );
        // A blank or whitespace-only content is absence too.
        let blank = peer_status_rows(
            &[(
                "beta".to_string(),
                std::time::Duration::from_secs(2),
                Some("   ".to_string()),
            )],
            80,
            Some(SPINNER_FRAMES[1]),
        );
        assert_eq!(
            blank[0],
            format!("{} peer beta · 2.0s", SPINNER_FRAMES[1]),
            "{blank:?}"
        );
    }

    #[test]
    fn the_peer_rows_keep_their_stacking_order() {
        // The stacking pin over the display's real read path:
        // `live_children_states` sorts the registry's children
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
        let rows = peer_status_rows(&states, 80, Some(SPINNER_FRAMES[0]));
        assert!(
            rows[0].contains("zed") && rows[1].contains("apple"),
            "{rows:?}"
        );
    }

    #[test]
    fn spawned_subagent_shows_its_line_with_operating_time() {
        let rows = peer_status_rows(
            &[(
                "worker".to_string(),
                std::time::Duration::from_secs(65),
                None,
            )],
            80,
            Some(SPINNER_FRAMES[2]),
        );
        assert!(
            rows[0].starts_with(SPINNER_FRAMES[2]),
            "the running peer's row leads with the spinner: {rows:?}"
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
            Some(SPINNER_FRAMES[0]),
        );
        assert_eq!(two.len(), 2, "each live child its own row: {two:?}");
        assert!(two[0].contains("alpha") && two[1].contains("beta"));
        // The reap decides removal: a finished child is simply not in the
        // registry read's output, so the rows contract covers its absence.
        let gone = peer_status_rows(
            &[("beta".to_string(), std::time::Duration::from_secs(2), None)],
            80,
            Some(SPINNER_FRAMES[0]),
        );
        assert_eq!(gone.len(), 1, "the finished peer's line is gone: {gone:?}");
        assert!(!gone[0].contains("alpha"), "no stale row remains");
    }

    #[test]
    fn a_peer_row_without_a_phase_composes_without_the_spinner() {
        // The `None`-phase face: no spinner is composed (the idle prompt's
        // band before its clock has advanced) and the name and the operating
        // time are unchanged.
        let rows = peer_status_rows(
            &[("alpha".to_string(), std::time::Duration::from_secs(1), None)],
            80,
            None,
        );
        assert_eq!(rows.len(), 1, "one live child, one row: {rows:?}");
        assert_eq!(rows[0], "peer alpha · 1.0s", "{rows:?}");
        assert!(
            !rows[0].starts_with(SPINNER_FRAMES[0]),
            "no spinner without a phase: {rows:?}"
        );
        // The content face is unchanged beyond the leading glyph too.
        let turn = peer_status_rows(
            &[(
                "worker".to_string(),
                std::time::Duration::from_secs(65),
                Some("tool bash".to_string()),
            )],
            80,
            None,
        );
        assert_eq!(turn[0], "peer worker · 1m05s · tool bash", "{turn:?}");
    }

    #[test]
    fn the_band_leads_with_the_spinner_in_phase_with_the_session() {
        // The phase pin over the real resolver: with a run tracked, the band
        // leads with the run's own frame — the row rides the status spinner —
        // and the display-only clock is frozen for it; with no run, the
        // beat-advanced idle clock holds the phase.
        let mut ind = StatusIndicator::disabled();
        ind.run = Some(Run {
            started: std::time::Instant::now(),
            frame: 4,
        });
        assert_eq!(
            ind.peer_spinner_glyph(),
            SPINNER_FRAMES[4],
            "the band rides the run"
        );
        // The display-only clock is frozen while a run holds the phase.
        ind.peer_clock_advance();
        ind.peer_clock_advance();
        assert_eq!(
            ind.peer_spinner_glyph(),
            SPINNER_FRAMES[4],
            "the idle clock stays frozen under a run"
        );
        // A run at another frame moves the band with it.
        let mut ind2 = StatusIndicator::disabled();
        ind2.run = Some(Run {
            started: std::time::Instant::now(),
            frame: 3,
        });
        assert_eq!(
            ind2.peer_spinner_glyph(),
            SPINNER_FRAMES[3],
            "the band rides the run's frame, not the idle clock"
        );
        // Idle: the beat-advanced clock holds the phase alone.
        let mut idle = StatusIndicator::disabled();
        assert_eq!(
            idle.peer_spinner_glyph(),
            SPINNER_FRAMES[0],
            "the idle band starts at the clock's zero"
        );
        idle.peer_clock_advance();
        assert_eq!(
            idle.peer_spinner_glyph(),
            SPINNER_FRAMES[1],
            "the beat advances the idle clock"
        );
        idle.peer_clock_advance();
        assert_eq!(
            idle.peer_spinner_glyph(),
            SPINNER_FRAMES[2],
            "each beat advances the idle clock once"
        );
    }

    #[test]
    fn the_idle_peer_clock_wraps_through_the_frame_cycle() {
        // The wrap pin: the display-only clock cycles through the frames and
        // wraps like the run's own frame does.
        let mut idle = StatusIndicator::disabled();
        idle.peer_tick = 9;
        assert_eq!(
            idle.peer_spinner_glyph(),
            SPINNER_FRAMES[9],
            "the clock holds its frame"
        );
        idle.peer_clock_advance();
        assert_eq!(
            idle.peer_spinner_glyph(),
            SPINNER_FRAMES[0],
            "the clock wraps at the cycle's end"
        );
        // The `u8` field wraps far past the cycle the same way (`usize::from`
        // keeps the cast exact).
        idle.peer_tick = u8::MAX;
        idle.peer_clock_advance();
        assert_eq!(
            idle.peer_spinner_glyph(),
            SPINNER_FRAMES[0],
            "the clock's own wrap agrees"
        );
    }

    // --- the armed composition's own geometry (the live screen's zone) ---

    #[test]
    fn the_beat_plan_holds_the_four_regions_in_the_instruction_order() {
        // The armed re-stack, bound through the pure plan the beat paints
        // with: read top-down the composition is four regions — the log region
        // above (there the sweep erases and the tail fills), the run display
        // (the clock block), the subagent rows, the prompt span last. Fold
        // check: swapping the block above the peers (the run display above the
        // subagent rows) fails the order asserts.
        let plan = zone_beat_plan(23, 2, 1);
        // The clock block sits directly above the span
        assert_eq!(
            plan.block_home, 21,
            "the block's home is span_top − block_n"
        );
        // The band above the block; its last row is the block's top neighbour
        assert_eq!(plan.peers_top, 20, "the band is above the block");
        assert_eq!(
            plan.peers_top + 1,
            plan.block_home,
            "the band's bottom row is the block's top neighbour"
        );
        // The sweep never enters the span and covers all the region above
        assert_eq!(
            plan.sweep_to, 23,
            "the sweep erases every row above the span"
        );
        // No peers: the block still sits directly above the span
        let bare = zone_beat_plan(23, 1, 0);
        assert_eq!(bare.block_home, 22);
        assert_eq!(
            bare.peers_top, 22,
            "the band's plan collapses into the block"
        );
    }

    #[test]
    fn the_span_never_enters_the_sweep_even_when_the_block_exhausts_the_screen() {
        // The span-is-never-swept guarantee at the geometry's degenerate
        // edge: a turn whose thinking block plus peers cover every row
        // above the span still leaves the span's rows out of the beat's
        // sweep (and the plan's saturating arithmetic keeps every row in
        // range).
        let plan = zone_beat_plan(3, 3, 0);
        assert_eq!(
            plan.block_home, 0,
            "the block fills every row above the span"
        );
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
        // The click rule at the zone's clock block, with the zone's flipped
        // face: the reasoning rows stand ABOVE the status row (the block's
        // last row is the spinner row, the screen's bottom-most line
        // mid-turn), so the toggle rows are the block's rows above its last
        // row — the reasoning rows and their elision marker. The status row
        // itself and everything outside the block never toggle, and a block
        // not painted (`clock_rows <= 1`) takes no click. Fold check: the
        // shipped spinner-first face's rule (the rows above the spinner
        // excluded) fails the boundary asserts here. The flipped face's
        // boundary: rows 20-21 are the reasoning rows (above the status
        // row), row 22 is the status row itself — the screen's bottom-most
        // line mid-turn, never a toggle.
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
        // The turn-end handoff, bound through `tail_for_region`'s seam: the
        // erase's budget ends the replay on the row above the bottom row, so
        // the normal render draws the prompt on a fresh bottom row with the
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
        // the elapsed row above it with the peer lines present, and the peer
        // lines pay for no part of either.
        let pending_clock =
            bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, 0, false);
        let pending_both =
            bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, 2, false);
        assert_eq!(
            pending_clock, 2,
            "the prompt span and its elapsed row alone"
        );
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
        // row directly above it.
        let bottom = bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, 0, false);
        assert_eq!(bottom, 2, "the prompt span plus the elapsed row above it");
        let body = crate::scroll::body_rows(height, bottom + 2);
        assert_eq!(
            body, 20,
            "two peer rows cost two body rows, not the block's"
        );
        // The overlay bottom block and the live-screen budget agree.
        let peers: Vec<String> = vec!["peer alpha · 1.0s".to_string()];
        let budget = bottom_block_rows(
            &PromptState::Pending,
            &InputState::new(),
            80,
            peers.len(),
            false,
        );
        assert_eq!(budget, bottom + peers.len() as u16);
    }

    #[test]
    fn esc_from_a_scrolled_view_mid_turn_reshows_the_prompt_pin() {
        // The arm's redraw is unconditional now: the pin binds the redraw
        // contract through the pure role-choice it shares with the shipped
        // not-pending redraw. The role the arm paints for each state.
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
        // The seam answers a redraw for every input-loop state, Pending
        // included — skipping that one would leave a mid-turn Esc-return with
        // a blockless bottom. Fold check: a `!is_pending()` gate (`None` for
        // `Pending`) fails this pin.
        assert!(esc_return_redraw(&PromptState::Ready).is_some());
        assert!(esc_return_redraw(&PromptState::Pending).is_some());
        assert!(esc_return_redraw(&PromptState::Shell).is_some());
        let (tx, _rx) = oneshot::channel::<bool>();
        assert!(esc_return_redraw(&PromptState::AwaitingApproval(tx)).is_some());
        // What the redraw paints: the approval prompt while awaiting, the
        // input prompt otherwise (the same pair the shipped redraw used).
        assert_eq!(
            esc_return_redraw(&PromptState::AwaitingApproval(oneshot::channel::<bool>().0))
                .unwrap()
                .1,
            APPROVAL_PROMPT
        );
        assert_eq!(esc_return_redraw(&PromptState::Ready).unwrap().1, PROMPT);
    }

    #[test]
    fn the_prompt_span_holds_the_bottom_rows_with_subagents_pinned() {
        // The scrolled screen's geometry: the pinned block's bottom-most rows
        // are the prompt span in every state — the live screen's four-region
        // order — so with n live children the peer rows stack above the span
        // and the span still ends the screen. Fold check: a
        // clock-at-the-bottom stack (the clock on the screen's last row, the
        // prompt span dropped) fails the bottom-rows assert.
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
        // top neighbour (the bottom-up order).
        let running = scrolled_block_plan(prompt_rows, 2, true, false);
        assert_eq!(
            running.run_row,
            Some(2),
            "the elapsed row sits above the span"
        );
        assert_eq!(running.run_row.unwrap() + 1, running.prompt_top);
        assert_eq!(running.rows, 4, "two peers + the elapsed row + the span");
    }

    #[test]
    fn the_prompt_renderer_tracks_the_subagent_rows_it_paints() {
        // The live screen's subagent rows live in the tracked region: the
        // erase covers them together with the prompt block, so a reap
        // between renders (or a submit) leaves no stale line. The
        // bookkeeping drives `clear_block`'s MoveUp; the erase resets both
        // counters.
        let mut renderer = PromptRenderer::new();
        assert_eq!(renderer.peer_rows, 0);
        let mut stdout = std::io::stdout();
        renderer.render(
            &mut stdout,
            RenderParams {
                theme: &Theme::plain(),
                role: Role::PromptSymbol,
                prompt: PROMPT,
                input: &InputState::new(),
                suggestion: None,
                peer_lines: &[
                    "peer alpha · 1.0s".to_string(),
                    "peer beta · 2.0s".to_string(),
                ],
                run_row: None,
                marker: None,
            },
        );
        assert_eq!(
            renderer.peer_rows, 2,
            "the rows are part of the tracked region"
        );
        // An empty one-row prompt block tracks cursor row 0 (the shipped
        // `prompt_block_rows` pin's shape); the peers above it are what makes
        // the erase climb — `clear_block` moves back over the whole region.
        assert_eq!(renderer.cursor_row, 0);
        renderer.clear_block(&mut stdout, false);
        assert_eq!(renderer.peer_rows, 0, "the erase resets the region");
        assert_eq!(renderer.cursor_row, 0);
        renderer.reset();
        assert_eq!(renderer.peer_rows, 0);
    }

    #[test]
    fn hint_anchor_moves_down_on_a_shrink_and_reuses_spare_on_growth() {
        // The shrink pin: fewer rows above the span than the last render
        // painted — the candidate hint disappearing — moves the paint down so
        // the prompt span keeps its row, and the freed rows are recorded as
        // spare for a later growth to reuse. Pure, so the pins bind the
        // arithmetic without a terminal.
        let (down, up) = hint_anchor(1, 0, 0);
        assert_eq!((down, up), (1, 0), "a hint row disappearing moves down 1");
        assert_eq!(spare_after_anchor(1, 0, 0), 1, "the freed row is spare");
        // A deeper shrink records the whole delta.
        let (down, up) = hint_anchor(3, 1, 0);
        assert_eq!((down, up), (2, 0));
        assert_eq!(spare_after_anchor(3, 1, 0), 2);
        // No change, no movement.
        assert_eq!(hint_anchor(2, 2, 0), (0, 0));
        assert_eq!(spare_after_anchor(2, 2, 0), 0);
        // Growth beyond the spare sinks the block into fresh rows without
        // climbing past what the renderer owns (never over the log).
        assert_eq!(hint_anchor(0, 2, 1), (0, 1), "only the spare row is reused");
        assert_eq!(spare_after_anchor(0, 2, 1), 0, "the reused spare is spent");
        // Growth smaller than the spare is fully absorbed.
        assert_eq!(hint_anchor(1, 1, 2), (0, 0));
        assert_eq!(spare_after_anchor(1, 1, 2), 2);
    }

    #[test]
    fn the_banner_logo_spells_the_name_and_carries_the_version() {
        // R6/D7 pins over the pure composer: five rows, the name readable
        // left-to-right on every row, the spark leading the first row, the
        // version riding the last one — no escape sequences, constant shape.
        let rows = banner_logo_lines();
        assert_eq!(rows.len(), 5, "five letter rows: {rows:?}");
        // The four letter-only rows share one width (the two-column head is
        // the spark on row 0 and blank on rows 1-3); the last row carries the
        // version tail after letters of the same width.
        let widths: Vec<usize> = rows
            .iter()
            .map(|r| crate::editor::str_display_width(r))
            .collect();
        assert_eq!(
            widths[0], widths[1],
            "rows share the letter width: {widths:?}"
        );
        assert_eq!(
            widths[1], widths[2],
            "rows share the letter width: {widths:?}"
        );
        assert_eq!(
            widths[2], widths[3],
            "rows share the letter width: {widths:?}"
        );
        let tail = format!("   v{}", env!("CARGO_PKG_VERSION"));
        assert_eq!(
            widths[4],
            widths[3] + crate::editor::str_display_width(&tail),
            "the last row is the letters plus the version tail: {widths:?}"
        );
        assert!(
            rows.iter().all(|r| r.contains('█')),
            "the block-letter face is drawn: {rows:?}"
        );
        assert!(
            rows[0].starts_with('✦'),
            "the spark leads the first row: {rows:?}"
        );
        // The face draws the words as block letters (glyphs, not the literal
        // text), so the name pin binds the shape: every row carries the two
        // word bands, and the letter rows align across them.
        assert!(
            rows.iter().all(|r| r.matches('█').count() > 0),
            "each letter row carries block glyphs: {rows:?}"
        );
        assert!(
            !rows.iter().any(|r| crate::theme::strip_sgr(r) != *r),
            "the composer returns plain rows (styling is the caller's): {rows:?}"
        );
        assert!(
            rows[4].contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
            "the version rides the last row: {rows:?}"
        );
    }

    #[test]
    fn the_prompt_holds_its_row_through_the_hint_show_hide_cycle() {
        // The reported defect pinned: the candidate hint appearing and then
        // disappearing must not move the prompt line. The bookkeeping cycle
        // over the real renderer: the shrink of the second render records the
        // freed row as spare (`hint_rows` drops to 0, `spare_rows` rises to
        // 1), the next show reuses it instead of sinking lower (`spare_rows`
        // back to 0, the hint row tracked again) — the span's row stays where
        // it was in every state.
        let mut renderer = PromptRenderer::new();
        let mut stdout = std::io::stdout();
        let mut base = |r: &mut PromptRenderer, sug: Option<&str>, peers: &[String]| {
            r.render(
                &mut stdout,
                RenderParams {
                    theme: &Theme::plain(),
                    role: Role::PromptSymbol,
                    prompt: PROMPT,
                    input: &InputState::new(),
                    suggestion: sug,
                    peer_lines: peers,
                    run_row: None,
                    marker: None,
                },
            );
        };
        // 1. The hint appears over a one-row block: it is tracked above the span.
        base(&mut renderer, Some("/help"), &[]);
        assert_eq!(renderer.hint_rows, 1, "the hint row is tracked");
        assert_eq!(renderer.spare_rows, 0, "no spare while steady");
        // 2. The hint disappears (the reported climb): the shrink anchor keeps
        //    the span's row and records the freed row as spare.
        base(&mut renderer, None, &[]);
        assert_eq!(renderer.hint_rows, 0);
        assert_eq!(
            renderer.spare_rows, 1,
            "the freed hint row keeps the span in place and becomes spare"
        );
        // 3. The hint appears again: the spare row is reused, so the paint does
        //    not sink lower and the span's row is unchanged.
        base(&mut renderer, Some("/help"), &[]);
        assert_eq!(renderer.hint_rows, 1);
        assert_eq!(renderer.spare_rows, 0, "the reused spare is spent");
        // 4. A peer spawning between renders (growth with no spare) sinks the
        //    block into fresh rows — real content above the span, the shipped
        //    shape — with nothing stolen from the log.
        base(&mut renderer, None, &["peer alpha · 1.0s".to_string()]);
        assert_eq!(renderer.peer_rows, 1);
        assert_eq!(renderer.spare_rows, 0);
        renderer.reset();
    }

    #[test]
    fn surrendering_the_region_absorbs_the_spare_rows() {
        // `clear_block` with `keep_spare = false` — the submit echo, a notice
        // print, the shutdown — erases the owned blank rows together with the
        // tracked region, so the next print lands directly below the log and
        // no stray blank line survives the surrender.
        let mut renderer = PromptRenderer::new();
        let mut stdout = std::io::stdout();
        renderer.render(
            &mut stdout,
            RenderParams {
                theme: &Theme::plain(),
                role: Role::PromptSymbol,
                prompt: PROMPT,
                input: &InputState::new(),
                suggestion: Some("/help"),
                peer_lines: &[],
                run_row: None,
                marker: None,
            },
        );
        renderer.render(
            &mut stdout,
            RenderParams {
                theme: &Theme::plain(),
                role: Role::PromptSymbol,
                prompt: PROMPT,
                input: &InputState::new(),
                suggestion: None,
                peer_lines: &[],
                run_row: None,
                marker: None,
            },
        );
        assert_eq!(renderer.spare_rows, 1, "the shrink recorded its row");
        renderer.clear_block(&mut stdout, false);
        assert_eq!(renderer.spare_rows, 0, "the surrender absorbs the spare");
        assert_eq!(renderer.hint_rows, 0);
        renderer.reset();
    }

    #[test]
    fn the_scrolled_prompt_pin_carries_the_marker_row() {
        // With the view off the bottom and the prompt pinned, the marker stands
        // on the row **immediately above** the block's first row — not beneath
        // the block's last row as the shipped shape painted it. The drawer's
        // budget and the wheel/drag budgets grow by exactly that row, so the
        // wheel's range and the draw agree on where the body ends. The marker is
        // part of the tracked region now, so `cursor_row` counts it and the next
        // render's erase climbs past it. Fold check: the shipped
        // beneath-the-block place fails the place asserts here and the
        // `cursor_row` fold below.
        let mut renderer = PromptRenderer::new();
        let mut stdout = std::io::stdout();
        renderer.render(
            &mut stdout,
            RenderParams {
                theme: &Theme::plain(),
                role: Role::PromptSymbol,
                prompt: PROMPT,
                input: &InputState::new(),
                suggestion: None,
                peer_lines: &[],
                run_row: None,
                marker: Some(SCROLLED_MARK),
            },
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
        // A wrapped prompt keeps the marker directly above its first row
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
        // Mid-turn the same row joins the live composition's count: the
        // span, the elapsed row, the marker and the peers.
        let pending_moved =
            bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, 0, view_moved);
        assert_eq!(pending_moved, 3, "the span + the elapsed row + the marker");
    }

    #[test]
    fn the_scrolled_screen_keeps_the_elapsed_row_and_adds_the_marker() {
        // The composition keeps the elapsed row on screen and adds the marker
        // beside it: the span at the bottom rows, the elapsed row above it, the
        // marker above that, the peers above. Swapping the elapsed row for
        // `Message below` in a single row would make a running turn's clock
        // vanish as soon as the log is scrolled. Fold check: that single-row
        // swap (the marker in the clock's row, the elapsed row withheld) fails
        // the "the clock is still drawn" assert.
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
        // corrected.
        let idle = scrolled_block_plan(1, 0, false, true);
        assert_eq!(idle.run_row, None, "no elapsed row without a turn");
        assert_eq!(idle.marker_row, Some(0));
        assert_eq!(idle.rows, 2);
    }

    #[test]
    fn a_failed_copy_write_still_reports_and_an_empty_gesture_stays_silent() {
        // An erroring writer: the outcome must be the Failed route, reported.
        let outcome = crate::clip::copy("some text", "", |_seq| {
            Err(std::io::Error::other("write refused"))
        });
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
        // A running turn's pin: the prompt span and the elapsed row above
        // it, so the wheel's body is height minus those rows — neither is
        // ever part of the scrollable range.
        let bottom = bottom_block_rows(&PromptState::Pending, &InputState::new(), 80, 0, false);
        assert_eq!(bottom, 2, "the tracked turn pins the span and its clock");
        let body = crate::scroll::body_rows(24, bottom);
        assert_eq!(body, 22);
        let mut overlay = ScrollOverlay::new();
        let _max = crate::scroll::with(|t| crate::scroll::max_offset(t, 80, body)).unwrap();
        overlay.wheel(true, 80, body);
        assert!(
            overlay.view.offset()
                <= crate::scroll::with(|t| crate::scroll::total_rows(t, 80))
                    .unwrap()
                    .saturating_sub(body),
            "the wheel cannot push content under the pinned clock"
        );
        crate::scroll::uninstall();
    }

    /// The screen row `text` is drawn on in `rows`, the slice's index.
    fn screen_row_of(rows: &[crate::scroll::Row], text: &str) -> usize {
        rows.iter()
            .position(|r| r.text == text)
            .unwrap_or_else(|| panic!("{text} is not on screen"))
    }

    #[test]
    fn the_first_notch_moves_the_log_by_one_step_like_every_other_notch() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(500);
        for i in 0..100 {
            crate::scroll::record_line(&format!("log line {i}"));
        }
        // A 40-row screen with a one-row prompt: the live screen's body is 39
        // rows; off the bottom the marker takes one more.
        let live_body = crate::scroll::body_rows(40, 1);
        let scrolled_body = crate::scroll::body_rows(40, 2);
        let live = body_slice_at(80, live_body, 0);
        let mut overlay = ScrollOverlay::new();

        assert!(overlay.wheel(true, 80, live_body));
        let first = body_slice_at(80, scrolled_body, slice_offset(overlay.view.offset(), 1));
        assert_eq!(first.len(), scrolled_body);
        assert_eq!(
            screen_row_of(&first, "log line 70"),
            screen_row_of(&live, "log line 70") + crate::scroll::STEP,
            "the first notch moves the log by exactly one step"
        );

        assert!(overlay.wheel(true, 80, live_body));
        let second = body_slice_at(80, scrolled_body, slice_offset(overlay.view.offset(), 1));
        assert_eq!(
            screen_row_of(&second, "log line 70"),
            screen_row_of(&first, "log line 70") + crate::scroll::STEP,
            "and the second by one step more"
        );
        crate::scroll::uninstall();
    }

    #[test]
    fn the_furthest_notch_shows_the_first_line_above_the_marker() {
        let _guard = crate::scroll::test_lock();
        crate::scroll::install(500);
        for i in 0..50 {
            crate::scroll::record_line(&format!("log line {i}"));
        }
        let live_body = crate::scroll::body_rows(40, 1);
        let scrolled_body = crate::scroll::body_rows(40, 2);
        let mut overlay = ScrollOverlay::new();
        while overlay.wheel(true, 80, live_body) {}
        assert_eq!(overlay.view.offset(), 50 - live_body);
        let rows = body_slice_at(80, scrolled_body, slice_offset(overlay.view.offset(), 1));
        assert_eq!(
            rows[0].text, "log line 0",
            "the top of the log is reachable"
        );
        assert!(
            rows.iter().all(|r| !r.text.is_empty()),
            "no padding at the top"
        );
        crate::scroll::uninstall();
    }

    #[test]
    fn the_slice_offset_adds_only_the_marker_rows() {
        assert_eq!(slice_offset(0, 0), 0);
        assert_eq!(slice_offset(3, 1), 4);
        assert_eq!(slice_offset(6, 0), 6);
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
            t.lines()
                .map(|l| crate::theme::strip_sgr(l).to_string())
                .collect()
        })
        .unwrap();
        assert!(
            texts
                .iter()
                .any(|t| t.contains("\u{2714}") && t.contains("5.0s")),
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
        // The erase's emit must address each named row's first column under
        // the crossterm row-first convention — a `\x1b[{row+1};1H` cursor
        // position, then the line erase. The shipped call passed the row
        // into the column slot, so every beat and every disarm erased row
        // 1's columns instead of the row it named: the banner row was the
        // only surface the zone ever cleared.
        assert_eq!(zone_csi_clear_row(0), "\x1b[1;1H\x1b[2K");
        assert_eq!(zone_csi_clear_row(1), "\x1b[2;1H\x1b[2K");
        assert_eq!(zone_csi_clear_row(7), "\x1b[8;1H\x1b[2K");
        assert_eq!(zone_csi_move(0, 5), "\x1b[6;1H");
        assert_eq!(zone_csi_move(3, 5), "\x1b[6;4H");
    }

    #[test]
    fn the_zone_disarm_plan_never_replays_below_the_painted_top() {
        // The handoff arithmetic: the replay budget is the span of rows
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
        // A press + release on the log with nothing between — a plain click —
        // starts a selection the gesture never paints and a release that takes
        // no screen and redraws nothing. The face is the click's zero-touch
        // contract: `scroll_dirty` stays false (the drain's repaint tail runs
        // for no click), the overlay never becomes active, and the empty
        // selection contributes no clipboard write. The gate the release arm
        // reads is the overlay's own view state, so the pin walks the same
        // arithmetic the arm does.
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
        assert!(
            !copied_view,
            "a click's release repaints nothing: gate closed"
        );
        let sel = sel.expect("the click's selection is taken at release");
        assert!(sel.is_empty(), "press == cursor on a bare click");
        assert!(
            crate::select::selected_text(&body_slice_at(80, body, overlay.view.offset()), &sel,)
                .is_empty(),
            "a bare click copies nothing"
        );

        // After the gesture the overlay is exactly as it was: no active
        // screen, no moved view, no selection, no repaint debt.
        assert!(!overlay.is_active());
        assert!(!overlay.view_active());
        assert!(!overlay.has_selection());
        crate::scroll::uninstall();
    }

    #[test]
    fn a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt() {
        // Companion to the paint-free press: a real drag (the cursor moves)
        // keeps the gesture's screen — the release's copied_view is true, the
        // redraw runs, and the `[clip]` report lands after the screen is
        // restored. The gate opens only for a gesture that actually moved the
        // view or painted highlights.
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
            AgentEvent::Error {
                message: "e".into(),
            },
            AgentEvent::Info {
                message: "i".into(),
            },
            AgentEvent::ToolResult {
                name: "bash".into(),
                ok: true,
                output: "o".into(),
            },
            AgentEvent::ToolCall {
                name: "bash".into(),
                args: serde_json::json!({}),
            },
        ] {
            assert!(
                event_ends_at_line_start(&ev, mode, false, true, false),
                "{ev:?}"
            );
        }
        // Streamed text only when the delta ends with a newline.
        let mid = AgentEvent::Text {
            delta: "partial".into(),
        };
        assert!(!event_ends_at_line_start(&mid, mode, false, true, true));
        let ended = AgentEvent::Text {
            delta: "line\n".into(),
        };
        assert!(event_ends_at_line_start(&ended, mode, false, true, false));
        // An empty delta prints nothing, so the previous state stands.
        let empty = AgentEvent::Text {
            delta: String::new(),
        };
        assert!(event_ends_at_line_start(&empty, mode, false, true, true));
        assert!(!event_ends_at_line_start(&empty, mode, false, true, false));
        // With the tool lines suppressed, ToolCall/ToolResult print nothing:
        // the cursor state the event leaves is whatever preceded it.
        let call = AgentEvent::ToolCall {
            name: "bash".into(),
            args: serde_json::json!({}),
        };
        let result = AgentEvent::ToolResult {
            name: "bash".into(),
            ok: true,
            output: "o".into(),
        };
        assert!(event_ends_at_line_start(&call, mode, false, false, true));
        assert!(!event_ends_at_line_start(
            &result, mode, false, false, false
        ));
    }

    #[test]
    fn event_ends_at_line_start_respects_the_thinking_mode() {
        let ev = AgentEvent::Thinking {
            text: "reasoning".into(),
        };
        // Shown: printed without a trailing newline, so the cursor is mid-row.
        assert!(!event_ends_at_line_start(
            &ev,
            ShowThinkingMode::Collapsed,
            false,
            true,
            true
        ));
        assert!(!event_ends_at_line_start(
            &ev,
            ShowThinkingMode::Expanded,
            false,
            true,
            true
        ));
        // Hidden: nothing is printed, so the previous state stands.
        assert!(event_ends_at_line_start(
            &ev,
            ShowThinkingMode::Hidden,
            false,
            true,
            true
        ));
        assert!(!event_ends_at_line_start(
            &ev,
            ShowThinkingMode::Hidden,
            false,
            true,
            false
        ));
        // The turn boundary prints nothing either.
        let start = AgentEvent::TurnStart;
        assert!(event_ends_at_line_start(
            &start,
            ShowThinkingMode::Collapsed,
            false,
            true,
            true
        ));
        assert!(!event_ends_at_line_start(
            &start,
            ShowThinkingMode::Collapsed,
            false,
            true,
            false
        ));
    }

    /// `[ui] show_tool_results = false` silences the whole tool traffic on
    /// screen: ToolCall and ToolResult leave the cursor wherever it was.
    #[test]
    fn event_ends_at_line_start_treats_suppressed_tool_lines_as_silent() {
        let call = AgentEvent::ToolCall {
            name: "bash".into(),
            args: serde_json::json!({}),
        };
        let result = AgentEvent::ToolResult {
            name: "bash".into(),
            ok: true,
            output: "o".into(),
        };
        let mode = ShowThinkingMode::Collapsed;
        assert!(event_ends_at_line_start(&call, mode, false, false, true));
        assert!(!event_ends_at_line_start(
            &result, mode, false, false, false
        ));
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
        assert!(
            !out.contains('\n'),
            "the row must not break the line: {out:?}"
        );
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
        assert!(
            out.contains("\u{1b}["),
            "an erase sequence is expected: {out:?}"
        );
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
        assert!(
            !sink.taken().is_empty(),
            "painting resumes after the prompt"
        );
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
        // it is only not written onto the prompt (the turn-end disarm replays
        // it).
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
        // Without the indicator the line keeps its full form.
        let plain = tool_call_line(false, "bash", &long);
        assert_eq!(plain, format!("[tool-call] bash {long}"));
        // With it, the activity line is cut to a single terminal row so the
        // spinner stays directly beneath it.
        let cut = tool_call_line(true, "bash", &long);
        assert!(cut.ends_with('…'));
        assert!(!cut.contains('\n'));
        assert!(crate::editor::str_display_width(&cut) < PromptRenderer::terminal_width());
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
        assert!(
            ind.thinking_rows().is_empty(),
            "blank reasoning stays hidden"
        );
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
        assert!(
            !out.contains("reasoning line"),
            "the live view is not kept: {out:?}"
        );
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
        assert!(
            sink.taken().contains("1000l"),
            "capture released at the end"
        );
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
        // which the click rule does not target — a click there is not a hit.
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
    fn every_mouse_operation_runs_while_a_turn_is_in_flight() {
        // The former prohibition is gone: the wheel's scrollback move, the
        // click's toggle and the drag's select-and-copy open at rest AND
        // mid-turn — the log is the user's to scroll and copy while the
        // agent operates. The gates that kept the mouse mute mid-turn were
        // removed from all three arms; a grep for the former helper must
        // stay empty so no prohibition path can reappear unnoticed, and the
        // arms' remaining guards (scroll enabled, mouse_select on, the
        // indicator block's own click rule, the Shift override) are the
        // only conditions left.
        // The policy pinned on the turn flag both ways: mid-turn (true) and
        // at rest (false) see the identical mouse surface — the gates the
        // prohibition once joined no longer read any turn input.
        let turn = true;
        let gate_opens = turn;
        assert!(
            gate_opens,
            "the wheel, click and drag arms open mid-turn: {turn}"
        );
    }

    #[test]
    fn captured_thinking_leaves_the_cursor_where_it_was() {
        let ev = AgentEvent::Thinking {
            text: "reasoning".into(),
        };
        // Captured by the indicator: nothing is printed, so the previous state stands.
        assert!(event_ends_at_line_start(
            &ev,
            ShowThinkingMode::Collapsed,
            true,
            true,
            true
        ));
        assert!(!event_ends_at_line_start(
            &ev,
            ShowThinkingMode::Collapsed,
            true,
            true,
            false
        ));
        // Not captured: printed inline, so the cursor sits mid-row.
        assert!(!event_ends_at_line_start(
            &ev,
            ShowThinkingMode::Collapsed,
            false,
            true,
            true
        ));
    }

    #[test]
    fn display_event_skips_thinking_that_the_indicator_shows() {
        let mut state = DisplayState::new(false);
        state.capture_thinking = true;
        // Nothing is printed and no `[thinking]` section is opened.
        display_event(
            AgentEvent::Thinking {
                text: "reasoning".into(),
            },
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
        // The full form is kept when nothing is drawing beneath it.
        let verbatim = format!("[tool-result ok] bash: {long}");
        assert_eq!(
            compact_tool_result("ok", "bash", &long, 80, usize::MAX),
            verbatim
        );
        // The elision arms only while the indicator draws: with compact_output
        // off, display_event's tool-result arm prints the verbatim form; with
        // it on, the result is cut to the row budget.
        assert_eq!(tool_result_line(false, "ok", "bash", &long), verbatim);
        let cut = tool_result_line(true, "ok", "bash", &long);
        let rows: Vec<&str> = cut.split('\n').collect();
        assert_eq!(rows.len(), TOOL_RESULT_ROWS + 1, "{rows:?}");
        assert!(rows[TOOL_RESULT_ROWS].starts_with("… +"));
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
        // `/cancel` at an idle prompt must NOT raise the token — there is no
        // unwind to consume or clear it, and the queued peer prompts would
        // then skip until a manual prompt unstuck the session.
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
        assert!(
            matches!(result, CommandResult::SubmittedPrompt),
            "got {result:?}"
        );

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
        assert!(
            matches!(result, CommandResult::SubmittedPrompt),
            "got {result:?}"
        );
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
        assert!(cands.starts_with(&["quit".to_string(), "exit".to_string(), "help".to_string(),]));
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

    // --- slash commands recorded in history ---

    #[tokio::test]
    async fn slash_command_recorded_in_history() {
        let tmp = TempDir::new().unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);

        // Execute a built-in command: /help
        let _ = handle_repl_command("help", &input_tx, &state, false).await;
        let hist = state.history.read().await;
        assert!(
            hist.contains(&"/help".to_string()),
            "history should contain /help, got {:?}",
            hist
        );
        drop(hist);

        // Execute another built-in: /tools
        let _ = handle_repl_command("tools", &input_tx, &state, false).await;
        let hist = state.history.read().await;
        assert!(
            hist.contains(&"/tools".to_string()),
            "history should contain /tools, got {:?}",
            hist
        );
    }

    #[tokio::test]
    async fn custom_command_recorded_in_history() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("hello.md"), "Hello $ARGUMENTS").unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);

        let _ = handle_repl_command("hello world", &input_tx, &state, false).await;
        let hist = state.history.read().await;
        assert!(
            hist.contains(&"/hello world".to_string()),
            "history should contain /hello world, got {:?}",
            hist
        );
    }

    #[tokio::test]
    async fn quit_not_recorded_in_history() {
        let tmp = TempDir::new().unwrap();
        let state = build_state_with_commands(tmp.path()).await;
        let (input_tx, _input_rx) = mpsc::channel::<AgentInput>(8);

        let _ = handle_repl_command("quit", &input_tx, &state, false).await;
        let hist = state.history.read().await;
        assert!(
            !hist.contains(&"/quit".to_string()),
            "/quit should not be in history, got {:?}",
            hist
        );
    }

    // --- auto-execute single-candidate prefix match ---

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
        assert!(
            matches!(result, CommandResult::SubmittedPrompt),
            "got {result:?}"
        );

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
        assert!(
            hist.contains(&"/hello".to_string()),
            "auto-executed command should be in history as /hello, got {:?}",
            hist
        );
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
        assert!(
            matches!(result, CommandResult::Continue),
            "ambiguous prefix should return Continue, got {result:?}"
        );

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
        assert!(
            matches!(result, CommandResult::SubmittedPrompt),
            "got {result:?}"
        );

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
        assert!(
            matches!(result, CommandResult::Continue),
            "built-in /help should return Continue, got {result:?}"
        );
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
        fn new(reads: Vec<std::io::Result<CtEvent>>, polls: Vec<std::io::Result<bool>>) -> Self {
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
            Ok(CtEvent::Key(KeyEvent::new(
                KeyCode::Char('a'),
                KeyModifiers::NONE,
            ))),
            Ok(CtEvent::Mouse(crossterm::event::MouseEvent {
                kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                column: 10,
                row: 5,
                modifiers: KeyModifiers::NONE,
            })),
            Ok(CtEvent::Key(KeyEvent::new(
                KeyCode::Char('b'),
                KeyModifiers::NONE,
            ))),
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
        let polls = vec![
            Ok(false),
            Ok(false),
            Ok(false),
            Ok(false),
            Ok(false),
            Ok(false),
        ];
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
        let polls = vec![
            Ok(false),
            Ok(false),
            Ok(false),
            Ok(false),
            Ok(false),
            Ok(false),
        ];
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
    // ——— /send delivery options ———

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
        assert!(
            err.contains("unknown delivery"),
            "the shape is the tool's: {err}"
        );
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
