use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use crossterm::event::{self, Event as CtEvent, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, ClearType};
use crossterm::ExecutableCommand;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::{mpsc, oneshot, watch, RwLock};

use crate::agent::{Agent, AgentEvent, AgentInput, ApprovalRequest};
use crate::ai;
use crate::cli::RunArgs;
use crate::config::{Config, ConfigSource, ShowThinkingMode};
use crate::editor::InputState;
use crate::error::Result;
use crate::id::AgentId;
use crate::ipc::registry::{RegistryEntry, RegistryHandle};
use crate::ipc::server::IpcServer;
use crate::ipc::IpcMessage;
use crate::log::ConversationLog;
use crate::persona::{self, Persona, PersonaResolution};
use crate::tools::ToolRegistry;

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

pub async fn run(mut config: Config, source: ConfigSource, args: RunArgs) -> Result<()> {
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

    // Reflect persona-derived model / temperature into Provider settings
    config.apply_persona_overrides(
        resolution.persona.frontmatter.model.as_deref(),
        resolution.persona.frontmatter.temperature,
    );

    // Build provider (pre-connection validation)
    let provider = ai::build(&config, &source)?;
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

    // Logging
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

    // Approval channel (FR-04-1 / design doc 4.3A). Route for agent task to request y/N from input loop.
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
    let display_task = tokio::spawn(async move {
        let mut display_state = DisplayState::new();
        while let Some(ev) = event_rx.recv().await {
            let is_idle = matches!(ev, AgentEvent::Done | AgentEvent::Error { .. });
            display_event(ev, show_thinking, &mut display_state);
            if is_idle {
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
        let _ = terminal::disable_raw_mode();
    }
}

/// Redraw the prompt line: clear current line, print `> ` prefix and the editor content,
/// and position the cursor at the correct column.
fn redraw_prompt(stdout: &mut std::io::Stdout, state: &InputState) {
    use crossterm::cursor::MoveToColumn;
    use crossterm::style::Print;
    let prompt: &str = "> ";
    // Move to start of line, clear from cursor to end of line
    let _ = stdout.execute(MoveToColumn(0));
    let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
    // Print prompt prefix + line content
    let _ = stdout.execute(Print(format!("{}{}", prompt, state.line)));
    // Position cursor: prompt length + cursor offset
    let col = prompt.len() + state.cursor;
    let _ = stdout.execute(MoveToColumn(col as u16));
    let _ = stdout.flush();
}

/// Redraw the prompt line in approval mode: show `approve? [y/N]: ` with the current content.
fn redraw_approval_prompt(stdout: &mut std::io::Stdout, state: &InputState) {
    use crossterm::cursor::MoveToColumn;
    use crossterm::style::Print;
    let prompt: &str = "approve? [y/N]: ";
    let _ = stdout.execute(MoveToColumn(0));
    let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
    let _ = stdout.execute(Print(format!("{}{}", prompt, state.line)));
    let col = prompt.len() + state.cursor;
    let _ = stdout.execute(MoveToColumn(col as u16));
    let _ = stdout.flush();
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
                // Ctrl+C: clear current line
                input.clear_line();
                Some(KeyAction::ClearLine)
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
            // No tab completion for now; ignore
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

    // Draw initial prompt
    redraw_prompt(&mut stdout, &input_state);

    loop {
        // Read current history for navigation
        let history_snapshot: Vec<String> = {
            let h = state.history.read().await;
            h.clone()
        };

        // Decide if we should poll for terminal events
        let can_read = prompt_state.is_ready() || prompt_state.is_awaiting_approval();

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
                        // Redraw prompt after AI response
                        redraw_prompt(&mut stdout, &input_state);
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
                        // Print approval request (go to new line since we're in raw mode)
                        let _ = stdout.execute(crossterm::cursor::MoveToColumn(0));
                        let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
                        println!("[tool approval] {} {}", req.tool_name, req.args);
                        // Reset input state for the approval prompt
                        input_state = InputState::new();
                        prompt_state = PromptState::AwaitingApproval(req.response);
                        redraw_approval_prompt(&mut stdout, &input_state);
                    }
                    None => {
                        approval_rx = None;
                    }
                }
            }
            // Poll for crossterm key events when Ready or AwaitingApproval
            _ = tokio::task::spawn_blocking(move || {
                // This blocks the calling thread until a key event or timeout.
                // We use a short timeout so tokio::select! can check other branches frequently.
                let _ = event::poll(Duration::from_millis(50));
            }), if can_read => {
                // Drain all pending key events
                while event::poll(Duration::from_millis(0)).unwrap_or(false) {
                    if let Ok(CtEvent::Key(key_event)) = event::read() {
                        let is_awaiting_approval = prompt_state.is_awaiting_approval();
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
                                        // Clear the approval line and move to next line
                                        let _ = stdout.execute(crossterm::cursor::MoveToColumn(0));
                                        let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
                                        println!();
                                        continue;
                                    }

                                    // Clear the current prompt line and move to next line
                                    let _ = stdout.execute(crossterm::cursor::MoveToColumn(0));
                                    let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
                                    println!();

                                    if let Some(rest) = trimmed.strip_prefix('/') {
                                        if !handle_repl_command(rest, &input_tx, &state).await {
                                            break;
                                        }
                                        // Redraw prompt after command
                                        redraw_prompt(&mut stdout, &input_state);
                                        continue;
                                    }
                                    if trimmed.is_empty() {
                                        // Blank line: just redraw prompt
                                        redraw_prompt(&mut stdout, &input_state);
                                        continue;
                                    }

                                    // Save to history (persistent + in-memory)
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
                                    // Discard stale idle notifications from past peer prompts, then enter Pending
                                    while agent_idle_rx.try_recv().is_ok() {}
                                    prompt_state = PromptState::Pending;
                                }
                                KeyAction::ClearLine => {
                                    // Redraw prompt with cleared state
                                    redraw_prompt(&mut stdout, &input_state);
                                }
                                KeyAction::Eof => {
                                    break;
                                }
                                KeyAction::Continue => {
                                    if prompt_state.is_awaiting_approval() {
                                        redraw_approval_prompt(&mut stdout, &input_state);
                                    } else {
                                        redraw_prompt(&mut stdout, &input_state);
                                    }
                                }
                            }
                        }
                    }
                    // Ignore resize and other events in this inner loop
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
    // Ensure prompt line is cleaned up before exit
    let _ = stdout.execute(crossterm::cursor::MoveToColumn(0));
    let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
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
                            println!();
                            println!("[tool approval] {} {}", req.tool_name, req.args);
                            print!("approve? [y/N]: ");
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
                            if !handle_repl_command(rest, &input_tx, &state).await {
                                break;
                            }
                            continue;
                        }
                        if trimmed.is_empty() {
                            continue;
                        }
                        // Save to history (persistent + in-memory)
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

/// `/auto [on|off|status]` handler (FR-04-2 / design doc 4.3A).
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

struct DisplayState {
    thinking_printed: bool,
    answer_printed: bool,
}

impl DisplayState {
    fn new() -> Self {
        Self {
            thinking_printed: false,
            answer_printed: false,
        }
    }

    fn reset(&mut self) {
        self.thinking_printed = false;
        self.answer_printed = false;
    }
}

fn display_event(ev: AgentEvent, show_thinking: ShowThinkingMode, state: &mut DisplayState) {
    match ev {
        AgentEvent::Text { delta } => {
            if !state.answer_printed {
                eprintln!("\n[answer]");
                state.answer_printed = true;
            }
            print!("{delta}");
            let _ = std::io::stdout().flush();
        }
        AgentEvent::Thinking { text } => match show_thinking {
            ShowThinkingMode::Hidden => {}
            ShowThinkingMode::Collapsed => {
                if !state.thinking_printed {
                    eprintln!("\n[thinking]");
                    state.thinking_printed = true;
                } else {
                    eprint!(" ");
                }
                let collapsed = collapse_thinking_text(&text);
                eprint!("{collapsed}");
            }
            ShowThinkingMode::Expanded => {
                if !state.thinking_printed {
                    eprintln!("\n[thinking]");
                    state.thinking_printed = true;
                }
                eprint!("{text}");
            }
        },
        AgentEvent::ToolCall { name, args } => {
            eprintln!("\n[tool-call] {name} {args}");
        }
        AgentEvent::ToolResult { name, ok, output } => {
            let mark = if ok { "ok" } else { "ERR" };
            eprintln!("[tool-result {mark}] {name}: {output}");
        }
        AgentEvent::Done => {
            state.reset();
            println!();
        }
        AgentEvent::Error { message } => {
            state.reset();
            eprintln!("\n[error] {message}");
        }
        AgentEvent::Info { message } => {
            eprintln!("[info] {message}");
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
            // Clear conversation history (keep only system prompt).
            if input_tx.send(AgentInput::ClearHistory).await.is_err() {
                eprintln!("[error] failed to send clear request");
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
    if let Some(t) = &persona.frontmatter.temperature {
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
    //! Regression tests for FR-13 "App termination" input loop (`/quit` and Ctrl+D=EOF),
    //! plus unit tests for InputState history navigation (FR-03).
    use super::*;
    use crate::persona::Persona;
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
                tool_name: "shell".into(),
                args: serde_json::json!({"cmd": "echo hi"}),
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
                tool_name: "shell".into(),
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
}