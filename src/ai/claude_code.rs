//! Claude Code provider.
//!
//! Drives the locally installed `claude` CLI as an agent-cli backend, so a
//! user who starts `agent-cli` gets Claude Code underneath, wrapped in
//! agent-cli's own layers (REPL, personas, peer IPC, logging).
//!
//! Two independent knobs select the shape of the integration:
//!
//! * **`mode`** — who owns the agent loop.
//!   - `"delegation"` (default): Claude Code runs its own tools and keeps its
//!     own session. Its `tool_use` blocks are **never** mapped to
//!     [`ProviderEvent::ToolUse`]; by the time such a block is observed the
//!     tool has already run inside Claude Code, so converting it would make
//!     the agent loop execute the same command a second time.
//!   - `"gateway"`: `--tools ""` disables Claude Code's tools and the backend
//!     is used as a chat endpoint. Note that agent-cli's own tools cannot be
//!     offered either — `claude -p` has no way to accept external tool
//!     definitions short of MCP — so gateway mode is chat-only.
//!
//! * **`transport`** — `"stream"` (default) uses `--output-format stream-json`
//!   (`--verbose --include-partial-messages`) for token-level streaming;
//!   `"oneshot"` uses `--output-format json` and yields the whole reply at the
//!   end of the turn.
//!
//! Only `delegation` + `stream` + `session = "persistent"` keeps a resident
//! child process, fed one JSON line per turn on stdin. Every other combination
//! spawns one child per turn: a stateful child that is also re-sent the whole
//! transcript would accumulate a duplicated history.
//!
//! Observed against Claude Code `2.1.233`. The stream-json message shapes are
//! an implementation detail of that CLI, so parsing here ignores unknown line
//! types and unparsable lines rather than failing the turn.

use std::collections::VecDeque;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::ai::claude::{handle_frame, ClaudeParseState};
use crate::ai::opencode::flatten_history;
use crate::ai::{
    Capabilities, EventStream, Message, Provider, ProviderContext, ProviderEvent, ToolSpec,
};
use crate::config::{Config, ConfigSource, ProviderEntry};
use crate::error::{AppError, Result};

const DEFAULT_BIN: &str = "claude";
const DEFAULT_TURN_TIMEOUT_SECS: u64 = 900;
/// Lines of the child's stderr retained for error messages.
const STDERR_TAIL_LINES: usize = 20;

/// Who owns the agent loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Claude Code runs its own tools and keeps its own session.
    Delegation,
    /// `--tools ""`; chat-only.
    Gateway,
}

/// Output format / streaming granularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transport {
    /// `--output-format stream-json`, token-level deltas.
    Stream,
    /// `--output-format json`, whole reply at end of turn.
    OneShot,
}

/// Whether Claude Code's session is reused across turns (delegation only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionMode {
    Persistent,
    Ephemeral,
}

/// How the persona reaches Claude Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SystemPromptMode {
    Append,
    Replace,
}

fn parse_enum<T: Copy>(
    key: &str,
    value: Option<&str>,
    table: &[(&str, T)],
    default: T,
) -> Result<T> {
    let Some(raw) = value else { return Ok(default) };
    let needle = raw.trim().to_ascii_lowercase();
    if let Some((_, v)) = table.iter().find(|(name, _)| *name == needle) {
        return Ok(*v);
    }
    let accepted = table
        .iter()
        .map(|(name, _)| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(" | ");
    Err(AppError::provider(
        "claude-code",
        format!("[provider.claude-code] {key}: unknown value \"{raw}\" (accepted: {accepted})"),
    ))
}

/// Resolved, validated configuration.
#[derive(Debug, Clone)]
pub(crate) struct Settings {
    pub bin: PathBuf,
    pub model: Option<String>,
    pub mode: Mode,
    pub transport: Transport,
    pub session: SessionMode,
    pub tools: Option<Vec<String>>,
    pub allowed_tools: Option<Vec<String>>,
    pub disallowed_tools: Option<Vec<String>>,
    pub permission_mode: Option<String>,
    pub turn_timeout: Duration,
    pub max_budget_usd: Option<f64>,
    pub system_prompt_mode: SystemPromptMode,
    pub extra_args: Vec<String>,
}

impl Settings {
    fn from_entry(entry: &ProviderEntry, bin: PathBuf) -> Result<Self> {
        Ok(Self {
            bin,
            model: entry.model.clone(),
            mode: parse_enum(
                "mode",
                entry.mode.as_deref(),
                &[("delegation", Mode::Delegation), ("gateway", Mode::Gateway)],
                Mode::Delegation,
            )?,
            transport: parse_enum(
                "transport",
                entry.transport.as_deref(),
                &[
                    ("stream", Transport::Stream),
                    ("oneshot", Transport::OneShot),
                ],
                Transport::Stream,
            )?,
            session: parse_enum(
                "session",
                entry.session.as_deref(),
                &[
                    ("persistent", SessionMode::Persistent),
                    ("ephemeral", SessionMode::Ephemeral),
                ],
                SessionMode::Persistent,
            )?,
            tools: entry.tools.clone(),
            allowed_tools: entry.allowed_tools.clone(),
            disallowed_tools: entry.disallowed_tools.clone(),
            permission_mode: entry.permission_mode.clone(),
            turn_timeout: Duration::from_secs(
                entry.turn_timeout_secs.unwrap_or(DEFAULT_TURN_TIMEOUT_SECS),
            ),
            max_budget_usd: entry.max_budget_usd,
            system_prompt_mode: parse_enum(
                "system_prompt_mode",
                entry.system_prompt_mode.as_deref(),
                &[
                    ("append", SystemPromptMode::Append),
                    ("replace", SystemPromptMode::Replace),
                ],
                SystemPromptMode::Append,
            )?,
            extra_args: entry.extra_args.clone().unwrap_or_default(),
        })
    }

    /// True when one child process serves every turn of the conversation.
    pub(crate) fn resident(&self) -> bool {
        self.mode == Mode::Delegation
            && self.transport == Transport::Stream
            && self.session == SessionMode::Persistent
    }

    /// True when Claude Code holds the history and only new messages are sent.
    fn incremental(&self) -> bool {
        self.mode == Mode::Delegation && self.session == SessionMode::Persistent
    }

    /// True when `--include-partial-messages` is in effect, i.e. assistant
    /// text arrives as deltas and the final `assistant` message would be a
    /// duplicate.
    fn partial_messages(&self) -> bool {
        self.transport == Transport::Stream
    }
}

/// Which session flag the turn carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionAction {
    /// No session flag (resident child, or a stateless turn).
    None,
    /// `--session-id <uuid>` — first turn of a resumable one-shot session.
    New(String),
    /// `--resume <uuid>` — later turns of the same session.
    Resume(String),
}

/// What varies per turn.
#[derive(Debug, Clone)]
pub(crate) struct TurnPlan {
    pub system_prompt: Option<String>,
    /// Positional prompt. `None` for a resident child, which is fed on stdin.
    pub prompt: Option<String>,
    pub session: SessionAction,
}

/// Build the child's argument vector. Pure: no I/O, so the flag set of every
/// mode/transport combination is unit-testable.
///
/// Flag order is fixed (and asserted by tests) so that changes are visible in
/// diffs: `-p`, output format, streaming flags, input format, model, system
/// prompt, tool policy, permission mode, session flags, budget, `extra_args`,
/// prompt.
pub(crate) fn build_args(s: &Settings, plan: &TurnPlan) -> Vec<String> {
    let mut a: Vec<String> = vec!["-p".into()];

    match s.transport {
        Transport::Stream => {
            a.push("--output-format".into());
            a.push("stream-json".into());
            // stream-json under --print is rejected without --verbose.
            a.push("--verbose".into());
            a.push("--include-partial-messages".into());
        }
        Transport::OneShot => {
            a.push("--output-format".into());
            a.push("json".into());
        }
    }

    if s.resident() {
        a.push("--input-format".into());
        a.push("stream-json".into());
    }

    if let Some(model) = &s.model {
        a.push("--model".into());
        a.push(model.clone());
    }

    if let Some(sys) = &plan.system_prompt {
        if !sys.is_empty() {
            a.push(match s.system_prompt_mode {
                SystemPromptMode::Append => "--append-system-prompt".into(),
                SystemPromptMode::Replace => "--system-prompt".into(),
            });
            a.push(sys.clone());
        }
    }

    match s.mode {
        // Gateway disables Claude Code's tools outright; the configured tool
        // policy is deliberately ignored so the two cannot half-overlap.
        Mode::Gateway => {
            a.push("--tools".into());
            a.push(String::new());
        }
        Mode::Delegation => {
            if let Some(tools) = &s.tools {
                a.push("--tools".into());
                a.push(tools.join(","));
            }
            if let Some(tools) = &s.allowed_tools {
                a.push("--allowed-tools".into());
                a.push(tools.join(","));
            }
            if let Some(tools) = &s.disallowed_tools {
                a.push("--disallowed-tools".into());
                a.push(tools.join(","));
            }
            if let Some(pm) = &s.permission_mode {
                a.push("--permission-mode".into());
                a.push(pm.clone());
            }
        }
    }

    match &plan.session {
        SessionAction::New(id) => {
            a.push("--session-id".into());
            a.push(id.clone());
        }
        SessionAction::Resume(id) => {
            a.push("--resume".into());
            a.push(id.clone());
        }
        SessionAction::None => {
            // A stateless turn must not leave a session behind: without this
            // every gateway turn would persist a one-message conversation.
            if !s.incremental() {
                a.push("--no-session-persistence".into());
            }
        }
    }

    if let Some(budget) = s.max_budget_usd {
        a.push("--max-budget-usd".into());
        a.push(budget.to_string());
    }

    a.extend(s.extra_args.iter().cloned());

    if let Some(prompt) = &plan.prompt {
        a.push(prompt.clone());
    }
    a
}

/// Parser state carried across the lines of one turn.
#[derive(Default)]
pub(crate) struct LineState {
    claude: ClaudeParseState,
}

pub(crate) struct LineOutcome {
    pub events: Vec<ProviderEvent>,
    pub turn_done: bool,
}

impl LineOutcome {
    fn empty() -> Self {
        Self {
            events: Vec::new(),
            turn_done: false,
        }
    }
}

/// Map one stdout line of `claude` to agent-cli events.
///
/// `partial` reflects `--include-partial-messages`: when set, assistant text
/// already arrived as `stream_event` deltas and the final `assistant` message
/// is ignored to avoid emitting it twice.
pub(crate) fn map_line(line: &str, st: &mut LineState, partial: bool) -> LineOutcome {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return LineOutcome::empty();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => map_value(&v, st, partial),
        Err(e) => {
            // A stray non-JSON line (a warning, a banner) must not kill a turn.
            tracing::debug!(target: "agent_cli::ai::claude_code", error = %e, line = %trimmed, "unparsable line ignored");
            LineOutcome::empty()
        }
    }
}

pub(crate) fn map_value(v: &Value, st: &mut LineState, partial: bool) -> LineOutcome {
    let mut events = Vec::new();
    match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
        // Anthropic SSE, wrapped. Reuse the Claude parser for text/thinking
        // deltas; drop its Done (message_stop ends a message, not a turn —
        // a delegation turn contains several) and its ToolUse (see below).
        "stream_event" => {
            if let Some(event) = v.get("event") {
                let outcome = handle_frame(&event.to_string(), &mut st.claude);
                for ev in outcome.events {
                    if matches!(
                        ev,
                        ProviderEvent::Text { .. } | ProviderEvent::Thinking { .. }
                    ) {
                        events.push(ev);
                    }
                }
            }
        }
        "assistant" => {
            for block in blocks(v) {
                match block.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                    "text" if !partial => {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            events.push(ProviderEvent::Text {
                                delta: text.to_string(),
                            });
                        }
                    }
                    // NEVER converted to ProviderEvent::ToolUse: Claude Code
                    // has already executed it, so handing it to the agent loop
                    // would run the same command a second time.
                    "tool_use" => {
                        tracing::debug!(
                            target: "agent_cli::ai::claude_code",
                            tool = block.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                            "tool executed by Claude Code"
                        );
                    }
                    _ => {}
                }
            }
        }
        // Echo of a tool result Claude Code produced for itself.
        "user" => {
            tracing::debug!(target: "agent_cli::ai::claude_code", "tool result echo");
        }
        "result" => {
            if v.get("is_error").and_then(|e| e.as_bool()).unwrap_or(false) {
                let msg = v
                    .get("result")
                    .and_then(|r| r.as_str())
                    .or_else(|| v.get("subtype").and_then(|s| s.as_str()))
                    .unwrap_or("claude reported an error")
                    .to_string();
                events.push(ProviderEvent::Error { message: msg });
            } else if !partial {
                if let Some(text) = v.get("result").and_then(|r| r.as_str()) {
                    events.push(ProviderEvent::Text {
                        delta: text.to_string(),
                    });
                }
            }
            tracing::debug!(
                target: "agent_cli::ai::claude_code",
                cost_usd = v.get("total_cost_usd").and_then(|c| c.as_f64()).unwrap_or(0.0),
                num_turns = v.get("num_turns").and_then(|n| n.as_u64()).unwrap_or(0),
                "turn complete"
            );
            events.push(ProviderEvent::Done);
            return LineOutcome {
                events,
                turn_done: true,
            };
        }
        // system / rate_limit_event / anything a later release adds.
        _ => {}
    }
    LineOutcome {
        events,
        turn_done: false,
    }
}

fn blocks(v: &Value) -> &[Value] {
    v.get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

/// System messages joined into one prompt.
fn system_prompt(messages: &[Message]) -> Option<String> {
    let mut out = String::new();
    for m in messages {
        if let Message::System { content } = m {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(content);
        }
    }
    (!out.is_empty()).then_some(out)
}

/// User messages beyond `sent`, joined. Assistant and tool-result entries are
/// skipped: Claude Code's own session already holds them, and this backend
/// never emits `ToolUse`, so no `ToolResult` can originate here.
pub(crate) fn new_user_text(messages: &[Message], sent: usize) -> String {
    let mut out = String::new();
    for m in messages.iter().skip(sent) {
        if let Message::User { content } = m {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(content);
        }
    }
    out
}

/// A UUID-v4-shaped identifier for `--session-id`, derived from a ULID so no
/// new dependency is needed.
fn new_session_uuid() -> String {
    let mut b = ulid::Ulid::new().0.to_be_bytes();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: Vec<String> = b.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        hex[0..4].concat(),
        hex[4..6].concat(),
        hex[6..8].concat(),
        hex[8..10].concat(),
        hex[10..16].concat()
    )
}

/// Resolve the configured executable: used as-is when it looks like a path,
/// otherwise searched on `PATH`.
pub(crate) fn resolve_bin(raw: &str) -> Result<PathBuf> {
    let expanded = shellexpand::tilde(raw).to_string();
    let candidate = Path::new(&expanded);
    if expanded.contains(std::path::MAIN_SEPARATOR) {
        return is_executable(candidate)
            .then(|| candidate.to_path_buf())
            .ok_or_else(|| not_found(raw));
    }
    let paths = std::env::var_os("PATH").ok_or_else(|| not_found(raw))?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(&expanded))
        .find(|p| is_executable(p))
        .ok_or_else(|| not_found(raw))
}

fn not_found(raw: &str) -> AppError {
    AppError::provider(
        "claude-code",
        format!(
            "Claude Code executable not found: \"{raw}\".\n  \
             Install Claude Code, or set [provider.claude-code] bin to its full path."
        ),
    )
}

fn is_executable(p: &Path) -> bool {
    if !p.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// The child's piped ends, plus a background drain of stderr.
struct ChildParts {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Lines<BufReader<ChildStdout>>,
    stderr_tail: Arc<StdMutex<VecDeque<String>>>,
}

fn spawn(settings: &Settings, args: &[String], pipe_stdin: bool) -> Result<ChildParts> {
    let mut cmd = Command::new(&settings.bin);
    cmd.args(args)
        .stdin(if pipe_stdin {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Guarantees no orphan survives agent-cli exiting or the provider
        // being dropped mid-turn.
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| {
        AppError::provider(
            "claude-code",
            format!("failed to spawn {}: {e}", settings.bin.display()),
        )
    })?;
    let stdin = if pipe_stdin { child.stdin.take() } else { None };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::provider("claude-code", "child stdout unavailable"))?;
    let stderr_tail = Arc::new(StdMutex::new(VecDeque::new()));
    if let Some(stderr) = child.stderr.take() {
        let tail = Arc::clone(&stderr_tail);
        // Drained continuously: an undrained pipe would block the child once
        // the buffer fills, and the tail is what error messages quote.
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut t) = tail.lock() {
                    if t.len() == STDERR_TAIL_LINES {
                        t.pop_front();
                    }
                    t.push_back(line);
                }
            }
        });
    }
    Ok(ChildParts {
        child,
        stdin,
        stdout: BufReader::new(stdout).lines(),
        stderr_tail,
    })
}

fn tail_text(tail: &Arc<StdMutex<VecDeque<String>>>) -> String {
    tail.lock()
        .map(|t| t.iter().cloned().collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

/// A resident child serving every turn (delegation + stream + persistent).
struct Session {
    parts: ChildParts,
    stdin: ChildStdin,
    /// History entries already delivered to the child.
    sent: usize,
    /// System prompt the child was spawned with; a change forces a respawn.
    system: Option<String>,
}

pub struct ClaudeCodeProvider {
    pub(crate) settings: Settings,
    context: ProviderContext,
    session: Mutex<Option<Session>>,
    /// One-shot delegation: the session id to `--resume` after the first turn.
    resume_id: Mutex<Option<String>>,
}

impl ClaudeCodeProvider {
    pub fn from_config(cfg: &Config, source: &ConfigSource) -> Result<Self> {
        // An absent [provider.claude-code] section is fine: every key has a
        // default, and the backend needs no credentials of its own.
        let entry = cfg.provider.claude_code.clone().unwrap_or_default();
        let bin = resolve_bin(entry.bin.as_deref().unwrap_or(DEFAULT_BIN))?;
        let settings = Settings::from_entry(&entry, bin)?;
        Ok(Self {
            settings,
            context: ProviderContext::new(source, None, None),
            session: Mutex::new(None),
            resume_id: Mutex::new(None),
        })
    }

    fn error(&self, stage: &str, detail: impl fmt::Display) -> String {
        format!(
            "{stage}\n  binary     : {}\n  mode       : {}\n  transport  : {}\n  config     : {}\n  detail     : {detail}",
            self.settings.bin.display(),
            match self.settings.mode {
                Mode::Delegation => "delegation",
                Mode::Gateway => "gateway",
            },
            match self.settings.transport {
                Transport::Stream => "stream",
                Transport::OneShot => "oneshot",
            },
            self.context.config_path.display(),
        )
    }

    /// The prompt text for a non-resident turn, and how many history entries
    /// it covers.
    async fn plan_for_child(&self, messages: &[Message]) -> Result<TurnPlan> {
        let system = system_prompt(messages);
        if self.settings.incremental() {
            // Delegation + oneshot + persistent: Claude Code holds the
            // transcript; carry it across processes with --session-id/--resume.
            let mut guard = self.resume_id.lock().await;
            let action = match guard.clone() {
                Some(id) => SessionAction::Resume(id),
                None => {
                    let id = new_session_uuid();
                    *guard = Some(id.clone());
                    SessionAction::New(id)
                }
            };
            let text = last_user(messages).ok_or_else(|| {
                AppError::provider("claude-code", "no user message to send".to_string())
            })?;
            Ok(TurnPlan {
                system_prompt: system,
                prompt: Some(text),
                session: action,
            })
        } else {
            let (sys, body) = flatten_history(messages);
            Ok(TurnPlan {
                system_prompt: sys.or(system),
                prompt: Some(body),
                session: SessionAction::None,
            })
        }
    }

    /// One child per turn, `--output-format json`.
    async fn oneshot_turn(&self, messages: &[Message]) -> Result<EventStream<'_>> {
        let plan = self.plan_for_child(messages).await?;
        let args = build_args(&self.settings, &plan);
        let output = tokio::time::timeout(
            self.settings.turn_timeout,
            Command::new(&self.settings.bin)
                .args(&args)
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
        .await;

        let events = match output {
            Err(_) => vec![
                ProviderEvent::Error {
                    message: self.error(
                        "claude-code: turn timed out",
                        format!("no result within {:?}", self.settings.turn_timeout),
                    ),
                },
                ProviderEvent::Done,
            ],
            Ok(Err(e)) => vec![
                ProviderEvent::Error {
                    message: self.error("claude-code: failed to run", e),
                },
                ProviderEvent::Done,
            ],
            Ok(Ok(out)) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if !out.status.success() {
                    // V-09b shape: non-zero exit, empty stdout, message on stderr.
                    vec![
                        ProviderEvent::Error {
                            message: self.error(
                                &format!("claude-code: exited with {}", out.status),
                                if stderr.trim().is_empty() {
                                    stdout.trim().to_string()
                                } else {
                                    stderr.trim().to_string()
                                },
                            ),
                        },
                        ProviderEvent::Done,
                    ]
                } else {
                    match serde_json::from_str::<Value>(stdout.trim()) {
                        Ok(v) => {
                            let mut st = LineState::default();
                            let outcome = map_value(&v, &mut st, false);
                            let mut events = outcome.events;
                            if !outcome.turn_done {
                                events.push(ProviderEvent::Done);
                            }
                            events
                        }
                        Err(e) => vec![
                            ProviderEvent::Error {
                                message: self.error(
                                    "claude-code: could not parse output",
                                    format!("{e}: {}", excerpt(&stdout)),
                                ),
                            },
                            ProviderEvent::Done,
                        ],
                    }
                }
            }
        };
        Ok(Box::pin(futures::stream::iter(events)))
    }

    /// One child per turn, `--output-format stream-json` (gateway, or
    /// delegation with an ephemeral session).
    async fn per_turn_stream(&self, messages: &[Message]) -> Result<EventStream<'_>> {
        let plan = self.plan_for_child(messages).await?;
        let args = build_args(&self.settings, &plan);
        let parts = spawn(&self.settings, &args, false)?;
        let deadline = Instant::now() + self.settings.turn_timeout;
        let partial = self.settings.partial_messages();
        let stream = async_stream::stream! {
            let mut parts = parts;
            let mut st = LineState::default();
            loop {
                match tokio::time::timeout_at(deadline, parts.stdout.next_line()).await {
                    Err(_) => {
                        let _ = parts.child.start_kill();
                        yield ProviderEvent::Error {
                            message: self.error("claude-code: turn timed out", format!("no result within {:?}", self.settings.turn_timeout)),
                        };
                        yield ProviderEvent::Done;
                        return;
                    }
                    Ok(Err(e)) => {
                        yield ProviderEvent::Error { message: self.error("claude-code: read failed", e) };
                        yield ProviderEvent::Done;
                        return;
                    }
                    Ok(Ok(None)) => {
                        // stdout closed without a result line.
                        let status = parts.child.wait().await.ok();
                        yield ProviderEvent::Error {
                            message: self.error(
                                "claude-code: ended without a result",
                                format!(
                                    "exit {} {}",
                                    status.map(|s| s.to_string()).unwrap_or_else(|| "unknown".into()),
                                    tail_text(&parts.stderr_tail)
                                ),
                            ),
                        };
                        yield ProviderEvent::Done;
                        return;
                    }
                    Ok(Ok(Some(line))) => {
                        let outcome = map_line(&line, &mut st, partial);
                        for ev in outcome.events { yield ev; }
                        if outcome.turn_done { return; }
                    }
                }
            }
        };
        Ok(Box::pin(stream))
    }

    /// One resident child for the whole conversation, fed a JSON line per turn.
    async fn resident_turn(&self, messages: &[Message]) -> Result<EventStream<'_>> {
        let system = system_prompt(messages);
        let mut guard = self.session.lock().await;

        // Respawn when the child never existed, has exited, or the persona
        // changed (the system prompt is fixed at spawn time).
        let stale = match guard.as_mut() {
            None => true,
            Some(s) => s.system != system || s.parts.child.try_wait().ok().flatten().is_some(),
        };
        if stale {
            *guard = None;
            let plan = TurnPlan {
                system_prompt: system.clone(),
                prompt: None,
                session: SessionAction::None,
            };
            let args = build_args(&self.settings, &plan);
            let mut parts = spawn(&self.settings, &args, true)?;
            let stdin = parts.stdin.take().ok_or_else(|| {
                AppError::provider("claude-code", "child stdin unavailable".to_string())
            })?;
            *guard = Some(Session {
                parts,
                stdin,
                sent: 0,
                system: system.clone(),
            });
        }

        let session = guard.as_mut().expect("session present");
        let text = new_user_text(messages, session.sent);
        let text = if text.is_empty() {
            last_user(messages).ok_or_else(|| {
                AppError::provider("claude-code", "no user message to send".to_string())
            })?
        } else {
            text
        };
        session.sent = messages.len();

        let line = format!("{}\n", user_line(&text));
        if let Err(e) = session.stdin.write_all(line.as_bytes()).await {
            let msg = self.error("claude-code: failed to send the turn", e);
            *guard = None;
            return Err(AppError::provider("claude-code", msg));
        }
        if let Err(e) = session.stdin.flush().await {
            let msg = self.error("claude-code: failed to send the turn", e);
            *guard = None;
            return Err(AppError::provider("claude-code", msg));
        }

        let deadline = Instant::now() + self.settings.turn_timeout;
        let stream = async_stream::stream! {
            // The guard rides along with the stream: one turn at a time per
            // provider, which is what one-process-one-agent already implies.
            let mut guard = guard;
            let mut st = LineState::default();
            loop {
                let session = match guard.as_mut() {
                    Some(s) => s,
                    None => {
                        yield ProviderEvent::Done;
                        return;
                    }
                };
                match tokio::time::timeout_at(deadline, session.parts.stdout.next_line()).await {
                    Err(_) => {
                        let _ = session.parts.child.start_kill();
                        *guard = None;
                        yield ProviderEvent::Error {
                            message: self.error("claude-code: turn timed out", format!("no result within {:?}", self.settings.turn_timeout)),
                        };
                        yield ProviderEvent::Done;
                        return;
                    }
                    Ok(Err(e)) => {
                        *guard = None;
                        yield ProviderEvent::Error { message: self.error("claude-code: read failed", e) };
                        yield ProviderEvent::Done;
                        return;
                    }
                    Ok(Ok(None)) => {
                        let tail = tail_text(&session.parts.stderr_tail);
                        let status = session.parts.child.wait().await.ok();
                        *guard = None;
                        yield ProviderEvent::Error {
                            message: self.error(
                                "claude-code: child exited",
                                format!(
                                    "exit {} {tail}",
                                    status.map(|s| s.to_string()).unwrap_or_else(|| "unknown".into())
                                ),
                            ),
                        };
                        yield ProviderEvent::Done;
                        return;
                    }
                    Ok(Ok(Some(line))) => {
                        let outcome = map_line(&line, &mut st, true);
                        for ev in outcome.events { yield ev; }
                        if outcome.turn_done { return; }
                    }
                }
            }
        };
        Ok(Box::pin(stream))
    }
}

/// The stdin frame for `--input-format stream-json`. Isolated so a change in
/// the CLI's expected shape is a one-line fix.
fn user_line(text: &str) -> String {
    json!({
        "type": "user",
        "message": {
            "role": "user",
            "content": [{"type": "text", "text": text}],
        },
    })
    .to_string()
}

fn last_user(messages: &[Message]) -> Option<String> {
    messages.iter().rev().find_map(|m| match m {
        Message::User { content } => Some(content.clone()),
        _ => None,
    })
}

fn excerpt(s: &str) -> String {
    s.chars().take(200).collect()
}

#[async_trait]
impl Provider for ClaudeCodeProvider {
    fn name(&self) -> &'static str {
        "claude-code"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            streaming: self.settings.transport == Transport::Stream,
            // Tools exist only in delegation mode — and they are Claude Code's
            // own, executed inside it.
            tool_use: self.settings.mode == Mode::Delegation,
            thinking: self.settings.transport == Transport::Stream,
        }
    }

    fn model(&self) -> &str {
        self.settings.model.as_deref().unwrap_or("default")
    }

    async fn complete_stream(
        &self,
        messages: &[Message],
        _tools: &[ToolSpec],
    ) -> Result<EventStream<'_>> {
        // `_tools` is unused by design: `claude -p` accepts no external tool
        // definitions (see the module header), so agent-cli's registry cannot
        // be offered to it in either mode.
        if self.settings.resident() {
            self.resident_turn(messages).await
        } else if self.settings.transport == Transport::Stream {
            self.per_turn_stream(messages).await
        } else {
            self.oneshot_turn(messages).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            bin: PathBuf::from("/usr/bin/claude"),
            model: None,
            mode: Mode::Delegation,
            transport: Transport::Stream,
            session: SessionMode::Persistent,
            tools: None,
            allowed_tools: None,
            disallowed_tools: None,
            permission_mode: None,
            turn_timeout: Duration::from_secs(900),
            max_budget_usd: None,
            system_prompt_mode: SystemPromptMode::Append,
            extra_args: Vec::new(),
        }
    }

    fn plan() -> TurnPlan {
        TurnPlan {
            system_prompt: None,
            prompt: None,
            session: SessionAction::None,
        }
    }

    #[test]
    fn resident_only_for_delegation_stream_persistent() {
        let mut s = settings();
        assert!(s.resident());
        s.session = SessionMode::Ephemeral;
        assert!(!s.resident());
        let mut s = settings();
        s.transport = Transport::OneShot;
        assert!(!s.resident());
        let mut s = settings();
        s.mode = Mode::Gateway;
        assert!(!s.resident());
    }

    #[test]
    fn delegation_stream_args_are_bidirectional_json() {
        let args = build_args(&settings(), &plan());
        assert_eq!(
            args,
            vec![
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--include-partial-messages",
                "--input-format",
                "stream-json",
            ]
        );
    }

    #[test]
    fn oneshot_args_use_json_output_and_no_input_format() {
        let mut s = settings();
        s.transport = Transport::OneShot;
        let mut p = plan();
        p.prompt = Some("hi".into());
        p.session = SessionAction::New("11111111-2222-4333-8444-555555555555".into());
        let args = build_args(&s, &p);
        assert_eq!(
            args,
            vec![
                "-p",
                "--output-format",
                "json",
                "--session-id",
                "11111111-2222-4333-8444-555555555555",
                "hi",
            ]
        );
        assert!(!args.contains(&"--input-format".to_string()));
    }

    #[test]
    fn resume_replaces_session_id_on_later_turns() {
        let mut s = settings();
        s.transport = Transport::OneShot;
        let mut p = plan();
        p.prompt = Some("again".into());
        p.session = SessionAction::Resume("abc".into());
        let args = build_args(&s, &p);
        assert!(args.windows(2).any(|w| w == ["--resume", "abc"]));
        assert!(!args.contains(&"--session-id".to_string()));
        assert_eq!(args.last().unwrap(), "again");
    }

    #[test]
    fn gateway_disables_tools_and_session_persistence() {
        let mut s = settings();
        s.mode = Mode::Gateway;
        // A configured tool policy is ignored in gateway mode.
        s.tools = Some(vec!["Bash".into()]);
        let mut p = plan();
        p.prompt = Some("hello".into());
        let args = build_args(&s, &p);
        let joined = args.join(" ");
        assert!(args.windows(2).any(|w| w == ["--tools", ""]));
        assert!(joined.contains("--no-session-persistence"));
        assert!(!joined.contains("--tools Bash"));
        assert_eq!(args.last().unwrap(), "hello");
    }

    #[test]
    fn delegation_tool_policy_is_forwarded() {
        let mut s = settings();
        s.tools = Some(vec!["Bash".into(), "Read".into()]);
        s.allowed_tools = Some(vec!["Bash(git *)".into()]);
        s.disallowed_tools = Some(vec!["WebFetch".into()]);
        s.permission_mode = Some("acceptEdits".into());
        let args = build_args(&s, &plan());
        assert!(args.windows(2).any(|w| w == ["--tools", "Bash,Read"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--allowed-tools", "Bash(git *)"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--disallowed-tools", "WebFetch"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--permission-mode", "acceptEdits"]));
    }

    #[test]
    fn model_budget_and_extra_args_are_placed_before_the_prompt() {
        let mut s = settings();
        s.model = Some("opus".into());
        s.max_budget_usd = Some(0.5);
        s.extra_args = vec!["--add-dir".into(), "/tmp".into()];
        let mut p = plan();
        p.prompt = Some("go".into());
        let args = build_args(&s, &p);
        assert!(args.windows(2).any(|w| w == ["--model", "opus"]));
        assert!(args.windows(2).any(|w| w == ["--max-budget-usd", "0.5"]));
        let dir = args.iter().position(|a| a == "--add-dir").unwrap();
        assert_eq!(args[dir + 1], "/tmp");
        assert_eq!(args.last().unwrap(), "go");
    }

    #[test]
    fn system_prompt_mode_selects_the_flag() {
        let mut p = plan();
        p.system_prompt = Some("be terse".into());
        let args = build_args(&settings(), &p);
        assert!(args
            .windows(2)
            .any(|w| w == ["--append-system-prompt", "be terse"]));

        let mut s = settings();
        s.system_prompt_mode = SystemPromptMode::Replace;
        let args = build_args(&s, &p);
        assert!(args
            .windows(2)
            .any(|w| w == ["--system-prompt", "be terse"]));
    }

    #[test]
    fn ephemeral_delegation_does_not_persist_a_session() {
        let mut s = settings();
        s.session = SessionMode::Ephemeral;
        let mut p = plan();
        p.prompt = Some("hi".into());
        let args = build_args(&s, &p);
        assert!(args.contains(&"--no-session-persistence".to_string()));
    }

    #[test]
    fn unknown_enum_values_are_rejected_with_the_accepted_set() {
        let mut entry = ProviderEntry {
            mode: Some("hybrid".into()),
            ..Default::default()
        };
        let err = Settings::from_entry(&entry, PathBuf::from("claude")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("mode"), "{msg}");
        assert!(msg.contains("hybrid"), "{msg}");
        assert!(msg.contains("delegation"), "{msg}");

        entry.mode = None;
        entry.transport = Some("socket".into());
        let err = Settings::from_entry(&entry, PathBuf::from("claude")).unwrap_err();
        assert!(err.to_string().contains("transport"));
    }

    #[test]
    fn defaults_match_the_documented_ones() {
        let s = Settings::from_entry(&ProviderEntry::default(), PathBuf::from("claude")).unwrap();
        assert_eq!(s.mode, Mode::Delegation);
        assert_eq!(s.transport, Transport::Stream);
        assert_eq!(s.session, SessionMode::Persistent);
        assert_eq!(s.system_prompt_mode, SystemPromptMode::Append);
        assert_eq!(s.turn_timeout, Duration::from_secs(900));
        assert!(s.extra_args.is_empty());
    }

    fn events(lines: &[&str], partial: bool) -> (Vec<ProviderEvent>, bool) {
        let mut st = LineState::default();
        let mut out = Vec::new();
        let mut done = false;
        for l in lines {
            let outcome = map_line(l, &mut st, partial);
            out.extend(outcome.events);
            if outcome.turn_done {
                done = true;
                break;
            }
        }
        (out, done)
    }

    fn text_of(evs: &[ProviderEvent]) -> String {
        evs.iter()
            .filter_map(|e| match e {
                ProviderEvent::Text { delta } => Some(delta.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn stream_event_text_deltas_become_text() {
        let (evs, done) = events(
            &[
                r#"{"type":"system","subtype":"init","tools":[]}"#,
                r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"He"}}}"#,
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"llo"}}}"#,
                r#"{"type":"result","subtype":"success","is_error":false,"result":"Hello","total_cost_usd":0.002}"#,
            ],
            true,
        );
        assert_eq!(text_of(&evs), "Hello");
        assert!(done);
        assert!(matches!(evs.last(), Some(ProviderEvent::Done)));
    }

    #[test]
    fn message_stop_does_not_end_the_turn() {
        // A delegation turn contains several messages; only `result` ends it.
        let (evs, done) = events(
            &[
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"a"}}}"#,
                r#"{"type":"stream_event","event":{"type":"message_stop"}}"#,
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"b"}}}"#,
            ],
            true,
        );
        assert_eq!(text_of(&evs), "ab");
        assert!(!done);
        assert!(!evs.iter().any(|e| matches!(e, ProviderEvent::Done)));
    }

    #[test]
    fn thinking_deltas_are_forwarded() {
        let (evs, _) = events(
            &[
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"hmm"}}}"#,
            ],
            true,
        );
        assert!(evs
            .iter()
            .any(|e| matches!(e, ProviderEvent::Thinking { text } if text == "hmm")));
    }

    #[test]
    fn tool_use_never_becomes_a_tool_event() {
        // Claude Code has already executed it; emitting ToolUse would make the
        // agent loop run the same command a second time.
        let (evs, _) = events(
            &[
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"ls"}}]}}"#,
                r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"8"}]}}"#,
            ],
            true,
        );
        assert!(!evs
            .iter()
            .any(|e| matches!(e, ProviderEvent::ToolUse { .. })));
        assert!(evs.is_empty());
    }

    #[test]
    fn assistant_text_is_used_only_without_partial_messages() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}"#;
        let (with_partial, _) = events(&[line], true);
        assert_eq!(text_of(&with_partial), "");
        let (without_partial, _) = events(&[line], false);
        assert_eq!(text_of(&without_partial), "hi");
    }

    #[test]
    fn oneshot_result_object_yields_text_then_done() {
        let (evs, done) = events(
            &[
                r#"{"type":"result","subtype":"success","is_error":false,"result":"OK","session_id":"s1"}"#,
            ],
            false,
        );
        assert_eq!(text_of(&evs), "OK");
        assert!(done);
        assert!(matches!(evs.last(), Some(ProviderEvent::Done)));
    }

    #[test]
    fn result_error_yields_error_then_done() {
        let (evs, done) = events(
            &[
                r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"boom"}"#,
            ],
            true,
        );
        assert!(done);
        assert!(matches!(&evs[0], ProviderEvent::Error { message } if message == "boom"));
        assert!(matches!(evs.last(), Some(ProviderEvent::Done)));
    }

    #[test]
    fn unknown_and_unparsable_lines_are_ignored() {
        let (evs, done) = events(
            &[
                r#"{"type":"something_new","payload":1}"#,
                "not json at all",
                "",
            ],
            true,
        );
        assert!(evs.is_empty());
        assert!(!done);
    }

    #[test]
    fn new_user_text_sends_only_undelivered_turns() {
        let messages = vec![
            Message::System {
                content: "sys".into(),
            },
            Message::User {
                content: "first".into(),
            },
            Message::Assistant {
                content: "reply".into(),
                tool_calls: vec![],
                reasoning_content: None,
            },
            Message::User {
                content: "second".into(),
            },
        ];
        assert_eq!(new_user_text(&messages, 0), "first\n\nsecond");
        // After the first turn the child has seen entries 0..3.
        assert_eq!(new_user_text(&messages, 3), "second");
        assert_eq!(new_user_text(&messages, 4), "");
    }

    #[test]
    fn user_line_is_a_single_json_object() {
        let line = user_line("hello");
        assert!(!line.contains('\n'));
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["type"], "user");
        assert_eq!(v["message"]["role"], "user");
        assert_eq!(v["message"]["content"][0]["text"], "hello");
    }

    #[test]
    fn session_uuid_has_the_shape_claude_requires() {
        let id = new_session_uuid();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
        assert_eq!(&parts[2][0..1], "4");
        assert!(matches!(&parts[3][0..1], "8" | "9" | "a" | "b"));
        assert_ne!(id, new_session_uuid());
    }

    #[test]
    fn resolve_bin_rejects_a_missing_executable() {
        let err = resolve_bin("definitely-not-a-real-binary-xyz").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("definitely-not-a-real-binary-xyz"), "{msg}");
        assert!(msg.contains("bin"), "{msg}");
    }

    #[cfg(unix)]
    mod stub {
        use super::*;
        use futures::StreamExt;
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        /// Write an executable stub that stands in for `claude`.
        fn stub_bin(dir: &std::path::Path, body: &str) -> PathBuf {
            let path = dir.join("claude-stub.sh");
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "#!/bin/sh\n{body}").unwrap();
            // Close before exec: a still-open writer makes the exec fail with
            // ETXTBSY.
            drop(f);
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
            path
        }

        /// Start a turn, retrying while the stub is momentarily unexecutable.
        ///
        /// Tests run in parallel, and a child forked by one test inherits the
        /// still-open write handle another test holds on its own stub for the
        /// instant between `fork` and `exec` — which makes exec fail with
        /// ETXTBSY ("Text file busy"). Only the fixture is racy, never the
        /// provider, so the retry lives here.
        async fn start_turn<'a>(
            p: &'a ClaudeCodeProvider,
            messages: &[Message],
        ) -> EventStream<'a> {
            for _ in 0..50 {
                match p.complete_stream(messages, &[]).await {
                    Ok(stream) => return stream,
                    Err(e) if e.to_string().contains("Text file busy") => {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    Err(e) => panic!("complete_stream failed: {e}"),
                }
            }
            panic!("stub stayed busy");
        }

        fn provider(bin: PathBuf, tweak: impl FnOnce(&mut Settings)) -> ClaudeCodeProvider {
            let mut settings = super::tests::settings();
            settings.bin = bin;
            tweak(&mut settings);
            ClaudeCodeProvider {
                settings,
                context: ProviderContext {
                    config_path: PathBuf::from("/tmp/config.toml"),
                    api_key_env: None,
                    api_key_mask: None,
                },
                session: Mutex::new(None),
                resume_id: Mutex::new(None),
            }
        }

        async fn drain(stream: &mut EventStream<'_>) -> Vec<ProviderEvent> {
            let mut out = Vec::new();
            while let Some(ev) = stream.next().await {
                let done = matches!(ev, ProviderEvent::Done);
                out.push(ev);
                if done {
                    break;
                }
            }
            out
        }

        fn user(text: &str) -> Message {
            Message::User {
                content: text.into(),
            }
        }

        /// Run one turn to completion, absorbing the fixture-only ETXTBSY race
        /// described on `start_turn` (the one-shot path reports a spawn failure
        /// as an `Error` event rather than an `Err`).
        async fn run_turn(p: &ClaudeCodeProvider, messages: &[Message]) -> Vec<ProviderEvent> {
            for _ in 0..50 {
                let mut stream = start_turn(p, messages).await;
                let evs = drain(&mut stream).await;
                drop(stream);
                let busy = matches!(
                    evs.first(),
                    Some(ProviderEvent::Error { message }) if message.contains("Text file busy")
                );
                if !busy {
                    return evs;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            panic!("stub stayed busy");
        }

        #[tokio::test]
        async fn resident_child_serves_two_turns() {
            let dir = tempfile::tempdir().unwrap();
            // One turn's worth of events per line read from stdin, numbered by
            // a counter the process keeps. A fresh child per turn would reset
            // it to 1, so "turn2" proves the same process served both turns.
            let bin = stub_bin(
                dir.path(),
                r#"
n=0
while IFS= read -r line; do
  n=$((n+1))
  printf '{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"turn%s"}}}\n' "$n"
  printf '{"type":"result","subtype":"success","is_error":false,"result":"done"}\n'
done
"#,
            );
            let p = provider(bin, |_| {});
            let mut history = vec![user("one")];
            let evs = run_turn(&p, &history).await;
            assert_eq!(super::tests::text_of(&evs), "turn1");

            history.push(Message::Assistant {
                content: "turn1".into(),
                tool_calls: vec![],
                reasoning_content: None,
            });
            history.push(user("two"));
            let evs = run_turn(&p, &history).await;
            assert_eq!(super::tests::text_of(&evs), "turn2");
        }

        #[tokio::test]
        async fn non_zero_exit_yields_one_error_then_done() {
            let dir = tempfile::tempdir().unwrap();
            let bin = stub_bin(
                dir.path(),
                "echo 'No conversation found with session ID: x' >&2\nexit 1",
            );
            let p = provider(bin, |s| s.transport = Transport::OneShot);
            let evs = run_turn(&p, &[user("hi")]).await;
            assert_eq!(evs.len(), 2);
            assert!(
                matches!(&evs[0], ProviderEvent::Error { message } if message.contains("No conversation found"))
            );
            assert!(matches!(evs[1], ProviderEvent::Done));
        }

        #[tokio::test]
        async fn unparsable_oneshot_output_is_reported() {
            let dir = tempfile::tempdir().unwrap();
            let bin = stub_bin(dir.path(), "echo 'not json'");
            let p = provider(bin, |s| s.transport = Transport::OneShot);
            let evs = run_turn(&p, &[user("hi")]).await;
            assert!(
                matches!(&evs[0], ProviderEvent::Error { message } if message.contains("could not parse"))
            );
            assert!(matches!(evs.last(), Some(ProviderEvent::Done)));
        }

        #[tokio::test]
        async fn a_stuck_child_times_out_and_the_provider_recovers() {
            let dir = tempfile::tempdir().unwrap();
            let bin = stub_bin(dir.path(), "sleep 30");
            // Generous enough that spawning the stub under a loaded test run
            // cannot be mistaken for the timeout under test.
            let p = provider(bin, |s| s.turn_timeout = Duration::from_secs(2));
            let evs = run_turn(&p, &[user("hi")]).await;
            assert!(
                matches!(&evs[0], ProviderEvent::Error { message } if message.contains("timed out"))
            );
            assert!(matches!(evs.last(), Some(ProviderEvent::Done)));
            // The dead session was cleared, so the next turn starts a new child.
            assert!(p.session.lock().await.is_none());
        }

        #[tokio::test]
        async fn gateway_turn_streams_without_stdin() {
            let dir = tempfile::tempdir().unwrap();
            let bin = stub_bin(
                dir.path(),
                r#"printf '{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"hi"}}}\n'
printf '{"type":"result","subtype":"success","is_error":false,"result":"hi"}\n'"#,
            );
            let p = provider(bin, |s| s.mode = Mode::Gateway);
            let evs = run_turn(&p, &[user("hello")]).await;
            assert_eq!(super::tests::text_of(&evs), "hi");
            assert!(matches!(evs.last(), Some(ProviderEvent::Done)));
        }
    }
}
