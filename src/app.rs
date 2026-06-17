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
        let mut display_state = DisplayState::new(stdin_is_tty());
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
    fn finish_line(&mut self, stdout: &mut std::io::Stdout, prompt: &str, line: &str) {
        use crossterm::style::Print;
        // Remove the interactive render (cursor may be mid-line) before echoing.
        self.clear_block(stdout);
        let _ = stdout.execute(Print(prompt));
        let _ = stdout.execute(Print(line));
        // Advance to a fresh line; CR+LF is required in raw mode.
        let _ = stdout.execute(Print("\r\n"));
        self.cursor_row = 0;
        let _ = stdout.flush();
    }

    /// Render `prompt` + the editor buffer, wrapping correctly and positioning the
    /// cursor at the logical insertion point.
    fn render(&mut self, stdout: &mut std::io::Stdout, prompt: &str, state: &InputState) {
        use crossterm::cursor::{MoveDown, MoveToColumn, MoveUp};
        use crossterm::style::Print;

        let width = Self::terminal_width();

        // 1. Clear the previous block (handles multi-row renders).
        self.clear_block(stdout);

        // 2. Print prompt + line; the terminal auto-wraps long content.
        let _ = stdout.execute(Print(prompt));
        let _ = stdout.execute(Print(&state.line));

        // 3. Compute the target cursor position.
        let prompt_cols = crate::editor::str_display_width(prompt);
        let total_cols = prompt_cols + state.display_width();
        let cursor_cols = state.display_cursor();
        let (end_row, cursor_row, cursor_col) =
            layout_cursor(prompt_cols, cursor_cols, total_cols, width);

        // 4. Move from the post-print position (end_row) to the cursor row.
        if end_row > cursor_row {
            let _ = stdout.execute(MoveUp((end_row - cursor_row) as u16));
        } else if cursor_row > end_row {
            // Cursor belongs on a fresh wrapped row past the printed content
            // (cursor at the end of a line that exactly fills the width).
            let _ = stdout.execute(MoveDown((cursor_row - end_row) as u16));
        }
        let _ = stdout.execute(MoveToColumn(cursor_col as u16));

        // 5. Remember where the cursor ended up for the next render.
        self.cursor_row = cursor_row as u16;
        let _ = stdout.flush();
    }
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
    let mut renderer = PromptRenderer::new();
    const PROMPT: &str = "> ";
    const APPROVAL_PROMPT: &str = "approve? [y/N]: ";

    // Draw initial prompt
    renderer.render(&mut stdout, PROMPT, &input_state);

    loop {
        // Read current history for navigation (deduplicated for smooth up/down browsing)
        let history_snapshot: Vec<String> = {
            let h = state.history.read().await;
            dedup_consecutive(&h)
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
                        // Redraw prompt after AI response on a fresh line
                        renderer.reset();
                        renderer.render(&mut stdout, PROMPT, &input_state);
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
                        // the approval request on a fresh line.
                        renderer.clear_block(&mut stdout);
                        raw_println(true, &format!("[tool approval] {} {}", req.tool_name, req.args));
                        renderer.reset();
                        // Reset input state for the approval prompt
                        input_state = InputState::new();
                        prompt_state = PromptState::AwaitingApproval(req.response);
                        renderer.render(&mut stdout, APPROVAL_PROMPT, &input_state);
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
                // Drain all pending key events. `exit_loop` lets an inner break
                // (quit/exit command, EOF, closed channel) propagate out of the
                // event-drain `while` to terminate the outer input loop — a plain
                // `break` here would only stop draining events, not exit the REPL.
                let mut exit_loop = false;
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
                                        // Echo the approval response, then move to next line
                                        renderer.finish_line(&mut stdout, APPROVAL_PROMPT, &line);
                                        continue;
                                    }

                                    // Echo the submitted line, then move to next line
                                    renderer.finish_line(&mut stdout, PROMPT, &line);

                                    if let Some(rest) = trimmed.strip_prefix('/') {
                                        if !handle_repl_command(rest, &input_tx, &state, true).await {
                                            exit_loop = true;
                                            break;
                                        }
                                        // Redraw prompt after command
                                        renderer.render(&mut stdout, PROMPT, &input_state);
                                        continue;
                                    }
                                    if trimmed.is_empty() {
                                        // Blank line: just redraw prompt
                                        renderer.render(&mut stdout, PROMPT, &input_state);
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
                                        renderer.render(&mut stdout, APPROVAL_PROMPT, &input_state);
                                    } else {
                                        renderer.render(&mut stdout, PROMPT, &input_state);
                                    }
                                }
                                KeyAction::Eof => {
                                    exit_loop = true;
                                    break;
                                }
                                KeyAction::Continue => {
                                    if prompt_state.is_awaiting_approval() {
                                        renderer.render(&mut stdout, APPROVAL_PROMPT, &input_state);
                                    } else {
                                        renderer.render(&mut stdout, PROMPT, &input_state);
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
                            if !handle_repl_command(rest, &input_tx, &state, false).await {
                                break;
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
fn handle_auto_command(arg: &str, state: &Arc<ReplState>, raw_mode: bool) {
    let arg = arg.trim().to_ascii_lowercase();
    match arg.as_str() {
        "on" | "true" | "1" => {
            state.auto_approve.store(true, Ordering::SeqCst);
            raw_println(raw_mode, "[auto] tool approval: on (skipping y/N prompts)");
        }
        "off" | "false" | "0" => {
            state.auto_approve.store(false, Ordering::SeqCst);
            raw_println(raw_mode, "[auto] tool approval: off (will ask y/N for each tool call)");
        }
        "" | "status" => {
            let cur = if state.auto_approve.load(Ordering::SeqCst) {
                "on"
            } else {
                "off"
            };
            raw_println(raw_mode, &format!("[auto] tool approval: {cur}"));
        }
        other => {
            raw_eprintln(raw_mode, &format!("usage: /auto [on|off|status]  (got: {other})"));
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
}

impl DisplayState {
    fn new(raw_mode: bool) -> Self {
        Self {
            thinking_printed: false,
            answer_printed: false,
            section_printed: false,
            raw_mode,
        }
    }

    fn reset(&mut self) {
        self.thinking_printed = false;
        self.answer_printed = false;
        self.section_printed = false;
    }

    /// Print a section header (e.g. `[answer]`), prefixing a newline only if a
    /// previous section already printed this turn (FR-15).
    fn section_header(&mut self, label: &str) {
        raw_eprintln(self.raw_mode, &section_header_text(label, self.section_printed));
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

fn display_event(ev: AgentEvent, show_thinking: ShowThinkingMode, state: &mut DisplayState) {
    let rm = state.raw_mode;
    match ev {
        AgentEvent::Text { delta } => {
            if !state.answer_printed {
                state.section_header("[answer]");
                state.answer_printed = true;
            }
            raw_print_str(rm, &delta);
        }
        AgentEvent::Thinking { text } => match show_thinking {
            ShowThinkingMode::Hidden => {}
            ShowThinkingMode::Collapsed => {
                if !state.thinking_printed {
                    state.section_header("[thinking]");
                    state.thinking_printed = true;
                } else {
                    raw_eprint(rm, " ");
                }
                let collapsed = collapse_thinking_text(&text);
                raw_eprint(rm, &collapsed);
            }
            ShowThinkingMode::Expanded => {
                if !state.thinking_printed {
                    state.section_header("[thinking]");
                    state.thinking_printed = true;
                }
                raw_eprint(rm, &text);
            }
        },
        AgentEvent::ToolCall { name, args } => {
            state.section_header(&format!("[tool-call] {name} {args}"));
        }
        AgentEvent::ToolResult { name, ok, output } => {
            let mark = if ok { "ok" } else { "ERR" };
            raw_eprintln(rm, &format!("[tool-result {mark}] {name}: {output}"));
        }
        AgentEvent::Done => {
            state.reset();
            raw_println(rm, "");
        }
        AgentEvent::Error { message } => {
            state.reset();
            raw_eprintln(rm, &format!("\n[error] {message}"));
        }
        AgentEvent::Info { message } => {
            raw_eprintln(rm, &format!("[info] {message}"));
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
    raw_mode: bool,
) -> bool {
    let mut parts = rest.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("").trim();
    let arg = parts.next().unwrap_or("").trim();
    match cmd {
        "quit" | "exit" => return false,
        "help" => {
            raw_println(raw_mode, "Commands:");
            raw_println(raw_mode, "  /list                       List currently running peers (id / name / provider / model / role).");
            println!(
                "  /send <peer> <text>         Send a one-shot prompt to a peer (id or name)."
            );
            raw_println(raw_mode, "  /tools                      List tools enabled for this agent.");
            raw_println(raw_mode, "  /persona                    Show this agent's persona (role / skills / source path).");
            raw_println(raw_mode, "  /reload-persona             Re-resolve and reload the persona; system prompt is replaced, history kept.");
            raw_println(raw_mode, "  /peer <id_or_name>          Show a peer's persona summary.");
            raw_println(raw_mode, "  /history [n]                Show last n (default 20) user inputs from this session.");
            raw_println(raw_mode, "  /clear, /reset              Clear conversation history (persona / system prompt are kept).");
            raw_println(raw_mode, "  /cancel                     Request cancel of the in-flight AI response or tool call.");
            raw_println(raw_mode, "  /auto [on|off|status]       Toggle tool-approval skip. No arg / 'status' shows current value.");
            raw_println(raw_mode, "  /help                       Show this help.");
            raw_println(raw_mode, "  /quit, /exit                Terminate (full aliases). Ctrl+D, Ctrl+C, SIGTERM also exit cleanly.");
            raw_println(raw_mode, "");
            raw_println(raw_mode, "Tool approval can be skipped via:");
            raw_println(raw_mode, "  - REPL command  : /auto on  (toggleable at runtime)");
            raw_println(raw_mode, "  - CLI flag      : agent-cli run --auto-approve-tools");
            raw_println(raw_mode, "  - Config file   : [runtime] auto_approve_tools = true");
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
                raw_println(raw_mode, &format!("{:>4}  {}", i + 1, line));
            }
            if total == 0 {
                raw_println(raw_mode, "(empty)");
            }
        }
        "list" => list_peers(&state.registry_dir, raw_mode),
        "send" => send_to_peer(arg, &state.registry_dir, raw_mode).await,
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
            let _ = input_tx.send(AgentInput::Cancel).await;
        }
        _ => {
            raw_eprintln(raw_mode, &format!("unknown command: {cmd}"));
        }
    }
    true
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
        s.section_header("[answer]");
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
}