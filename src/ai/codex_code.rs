//! Codex CLI provider.
//!
//! Drives the locally installed `codex` CLI as an agent-cli backend, so a
//! user who starts `agent-cli` gets OpenAI Codex underneath, wrapped in
//! agent-cli's own layers (REPL, personas, peer IPC, logging). This module is
//! the codex-CLI sibling of [`crate::ai::claude_code`]: the same delegation
//! rule, the same stream-driven child, a deliberately smaller flag surface —
//! codex exposes no external tool list and no system-prompt flags, and a
//! different session mechanism (`thread.started`'s thread id feeding
//! `codex exec resume`).
//!
//! Observed against Codex CLI `0.166.0` (ChatGPT auth). The JSONL event
//! shapes (`thread.started` / `turn.started` / `item.*` / `turn.failed`) are
//! an implementation detail of that CLI; parsing ignores unknown types and
//! unparsable lines rather than failing the turn.
//!
//! ## The delegation rule
//!
//! Codex runs its own tools and keeps its own session. Its
//! `command_execution` items are **never** mapped to
//! [`ProviderEvent::ToolUse`]; by the time an item completes the command has
//! already run inside codex, so converting it would make the agent loop
//! execute the same command a second time. `capabilities().tool_use` is
//! false accordingly. (Probed on this cycle's session: typed
//! `command_execution` `item.started`/`item.completed` with
//! `command`/`aggregated_output`/`exit_code`/`status`, on fresh *and* resume
//! legs.)
//!
//! ## Sandbox and trust flags
//!
//! Codex's default sandbox is **read-only**: a workspace write without
//! `-s workspace-write` was rejected on the probed CLI ("writing is blocked
//! by read-only sandbox"). The claude-code backend has `--permission-mode`;
//! its counterpart knobs live on `[provider.codex-code]`: `sandbox` → `-s`
//! (`read-only` | `workspace-write` | `danger-full-access`; the backend
//! default is `workspace-write`, since delegation is exactly the shape that
//! must run commands), `add_dir` → repeated `--add-dir` (exec-level flags
//! reach a resume leg — probed), `approval_policy` →
//! `-c approval_policy=...`, `bypass_sandbox_and_approvals` →
//! `--dangerously-bypass-approvals-and-sandbox`, and `skip_git_repo_check`
//! (default `true`, emitting the probe-backed `--skip-git-repo-check`; set
//! `false` to honor codex's own trust gate — without it codex refuses to run
//! outside a trusted git directory, probed). `session = "ephemeral"` adds
//! `--ephemeral` every turn: codex persists no session files, so each
//! conversation stands alone and a `codex exec resume <tid>` after an
//! ephemeral conversation fails (probed).
//!
//! ## Session
//!
//! Every turn is one `codex exec` spawn; there is no resident child — codex
//! has no resident stdin protocol on the probed surface. Persistence across
//! turns is codex-side: a fresh run emits `thread.started` (thread id), later
//! turns spawn `codex exec resume <thread_id> --json` (probe-verified
//! geometry: exec-level flags precede the `resume` subcommand, and the
//! sandbox and model knobs reach the resumed run). The transcript is never
//! re-sent; codex holds the history — on the probed CLI, five turns on one
//! thread rode stdin prompts (`-`, closed after each write) while
//! input_tokens climbed 13,094 → 52,496, mostly cached.
//!
//! ## Errors
//!
//! A turn fails on codex-side conditions (bad model, trust gate, bogus
//! resume id) with rc=1, empty stdout and the message on stderr, or
//! mid-stream as a `turn.failed` JSONL line. There is no "result" line on
//! the codex wire — a **normal** turn ends at stdout EOF with rc=0 — so EOF
//! with a nonzero exit is the error path and EOF with success simply ends
//! the turn (an error already delivered by `turn.failed` is never
//! duplicated: that line ends the stream). The resume-leg failure the probes
//! recorded — "no rollout found for thread id … (code -32600)" — names the
//! thread on stderr, so the provider treats exactly that signature as
//! *session lost* and clears its thread id: the next turn re-enters with a
//! fresh conversation (the recovery a lost or ephemeral thread needs), while
//! the surfaced error still tells the user what happened. A codex-side
//! failure that does not match the signature (bad model, trust gate) never
//! discards a perfectly good thread.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::Lines;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::ai::opencode::flatten_history;
use crate::ai::{
    Capabilities, EventStream, Message, Provider, ProviderContext, ProviderEvent, ToolSpec,
};
use crate::config::{Config, ConfigSource, ProviderEntry};
use crate::error::{AppError, Result};

const DEFAULT_BIN: &str = "codex";
const DEFAULT_TURN_TIMEOUT_SECS: u64 = 900;
/// Delegation must run commands; codex's own default is read-only, which
/// would block every tool it tries to run for the agent.
const DEFAULT_SANDBOX: &str = "workspace-write";
/// Lines of the child's stderr retained for error messages.
const STDERR_TAIL_LINES: usize = 20;

/// Output granularity. Codex emits whole texts per completed item — no
/// token-level deltas on the probed exec surface — but `--json` streams
/// items as they happen; `oneshot` buffers the whole turn and yields the
/// final reply once (read back from `--output-last-message`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transport {
    /// `--json`: items as they arrive (turn-grain, not token-grain).
    Items,
    /// No `--json`: the whole reply at end of turn via `-o`.
    OneShot,
}

/// Whether codex's session persists between turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionMode {
    /// `codex exec resume <thread_id>` from the second turn on.
    Persistent,
    /// `--ephemeral`: each conversation stands alone.
    Ephemeral,
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
        "codex-code",
        format!("[provider.codex-code] {key}: unknown value \"{raw}\" (accepted: {accepted})"),
    ))
}

/// Resolved, validated configuration.
#[derive(Debug, Clone)]
pub(crate) struct Settings {
    pub bin: PathBuf,
    pub model: Option<String>,
    pub transport: Transport,
    pub session: SessionMode,
    pub sandbox: String,
    pub add_dir: Vec<String>,
    pub approval_policy: Option<String>,
    pub bypass_sandbox_and_approvals: bool,
    pub skip_git_repo_check: bool,
    pub turn_timeout: Duration,
    pub extra_args: Vec<String>,
}

impl Settings {
    fn from_entry(entry: &ProviderEntry, bin: PathBuf) -> Result<Self> {
        Ok(Self {
            bin,
            model: entry.model.clone(),
            transport: parse_enum(
                "transport",
                entry.transport.as_deref(),
                &[("items", Transport::Items), ("oneshot", Transport::OneShot)],
                Transport::Items,
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
            sandbox: entry
                .sandbox
                .clone()
                .unwrap_or_else(|| DEFAULT_SANDBOX.to_string()),
            add_dir: entry.add_dir.clone().unwrap_or_default(),
            approval_policy: entry.approval_policy.clone(),
            bypass_sandbox_and_approvals: entry.bypass_sandbox_and_approvals.unwrap_or(false),
            // Default true, probe-backed: agent-cli runs codex from its own
            // possibly-untrusted working directory, and delegation is exactly
            // the shape that must not ask before each tool (the claude-code
            // delegation path behaves the same way).
            skip_git_repo_check: entry.skip_git_repo_check.unwrap_or(true),
            turn_timeout: Duration::from_secs(
                entry.turn_timeout_secs.unwrap_or(DEFAULT_TURN_TIMEOUT_SECS),
            ),
            extra_args: entry.extra_args.clone().unwrap_or_default(),
        })
    }

    /// True when delegation persists its codex-side session across turns.
    fn persistent(&self) -> bool {
        self.session == SessionMode::Persistent
    }

    /// Which turn shape the argv is built for, given the recorded thread.
    fn session_action(&self, resume: Option<&str>) -> SessionAction {
        match (self.persistent(), resume) {
            (true, Some(id)) => SessionAction::Resume(id.to_string()),
            (true, None) => SessionAction::New,
            (false, _) => SessionAction::None,
        }
    }
}

/// Which turn shape the argv is built for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionAction {
    /// `codex exec` — a fresh conversation whose thread later turns resume.
    New,
    /// `codex exec resume <id>` — a later turn of the same thread.
    Resume(String),
    /// `codex exec --ephemeral` — sessionless (no resume exists or is wanted).
    None,
}

/// What varies per turn: the prompt text (stdin) and the session shape.
#[derive(Debug, Clone)]
pub(crate) struct TurnPlan {
    pub prompt: String,
    pub session: SessionAction,
}

/// Build the child's argument vector. Pure: no I/O, so the flag set of every
/// session/transport combination is unit-testable, mirroring claude-code's
/// `build_args`.
///
/// Flag order is fixed (and asserted by tests) so that changes are visible in
/// diffs, and follows the probe-verified geometry: exec-level flags come
/// first — the `resume` subcommand must come **after** them, because
/// `codex exec resume [OPTIONS] [SESSION_ID] [PROMPT]` rejects exec-level
/// flags placed past it (observed live: `resume tid --json …` fails with
/// "unexpected argument '-s' found", while flags placed on `exec` before the
/// subcommand reach the resumed run). Session shape, `extra_args`, then the
/// prompt tail — stdin marker `-` for the items transport, argv text for
/// one-shot.
pub(crate) fn build_args(s: &Settings, plan: &TurnPlan) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();

    if s.transport == Transport::Items {
        a.push("--json".into());
    }
    if s.transport == Transport::OneShot {
        a.push("--output-last-message".into());
        a.push(oneshot_file().to_string());
    }

    a.push("-s".into());
    a.push(s.sandbox.clone());
    for dir in &s.add_dir {
        a.push("--add-dir".into());
        a.push(dir.clone());
    }
    if let Some(policy) = &s.approval_policy {
        a.push("-c".into());
        a.push(format!("approval_policy={policy}"));
    }
    if s.bypass_sandbox_and_approvals {
        a.push("--dangerously-bypass-approvals-and-sandbox".into());
    }
    if let Some(model) = &s.model {
        a.push("-m".into());
        a.push(model.clone());
    }
    if s.skip_git_repo_check {
        a.push("--skip-git-repo-check".into());
    }
    if plan.session == SessionAction::None {
        a.push("--ephemeral".into());
    }
    a.extend(s.extra_args.iter().cloned());

    if let SessionAction::Resume(id) = &plan.session {
        a.push("resume".into());
        a.push(id.clone());
    }

    if s.transport == Transport::Items {
        // The prompt rides stdin (`-`), keeping user text out of argv and out
        // of process listings.
        a.push("-".into());
    } else {
        // One-shot: the prompt is the argv tail (`codex exec [OPTIONS]
        // [PROMPT]`, P5b's geometry).
        a.push(plan.prompt.clone());
    }
    a
}

/// The file one-shot turns write the reply to. Fixed relative name, dropped
/// after the turn: one child per turn, no concurrency within a provider.
pub(crate) fn oneshot_file() -> &'static str {
    ".agent-cli-codex-code-last.txt"
}

/// Parser state across one turn's JSONL lines.
#[derive(Default)]
pub(crate) struct LineState {
    thread: Option<String>,
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

/// Map one stdout line of `codex exec --json` to agent-cli events.
///
/// `thread.started` is pinned into the state (the caller carries it to
/// `codex exec resume` on the next turn) and never surfaced to the REPL.
/// Unknown line types (`turn.started`, `turn.completed`, item types a later
/// release adds) and unparsable lines are ignored — the wire shapes are the
/// CLI's implementation detail, as in the claude-code parser.
pub(crate) fn map_line(line: &str, st: &mut LineState) -> LineOutcome {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return LineOutcome::empty();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => map_value(&v, st),
        Err(e) => {
            // A stray non-JSON line (a warning, a banner) must not kill a turn.
            tracing::debug!(target: "agent_cli::ai::codex_code", error = %e, line = %trimmed, "unparsable line ignored");
            LineOutcome::empty()
        }
    }
}

pub(crate) fn map_value(v: &Value, st: &mut LineState) -> LineOutcome {
    let mut events = Vec::new();
    match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
        // Pin the thread id the moment it appears; the session resumes from
        // it on the next turn. Never surfaced to the REPL.
        "thread.started" => {
            if let Some(id) = v.get("thread_id").and_then(|t| t.as_str()) {
                st.thread = Some(id.to_string());
            }
        }
        "item.completed" => {
            let item = v.get("item").cloned().unwrap_or(Value::Null);
            match item.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                // NEVER converted to ProviderEvent::ToolUse: codex has already
                // executed it, so handing it to the agent loop would run the
                // same command a second time.
                "command_execution" => {
                    tracing::debug!(
                        target: "agent_cli::ai::codex_code",
                        command = item.get("command").and_then(|c| c.as_str()).unwrap_or(""),
                        exit_code = item.get("exit_code").and_then(|c| c.as_i64()).unwrap_or(-1),
                        "tool executed by codex"
                    );
                }
                "agent_message" => {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        if !text.is_empty() {
                            events.push(ProviderEvent::Text {
                                delta: text.to_string(),
                            });
                        }
                    }
                }
                "error" => {
                    let msg = item
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("codex reported an error")
                        .to_string();
                    events.push(ProviderEvent::Error { message: msg });
                }
                // Other item types (reasoning, file changes, …) and any type a
                // later CLI release adds: ignored, as claude-code tolerates
                // unknown line types.
                _ => {}
            }
        }
        // A mid-turn failure (a bad model name, for instance). `turn.failed`
        // ends the turn; the child then exits nonzero, and EOF is handled by
        // the turn driver.
        "turn.failed" => {
            let msg = v
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("codex turn failed");
            events.push(ProviderEvent::Error {
                message: msg.to_string(),
            });
            events.push(ProviderEvent::Done);
            return LineOutcome {
                events,
                turn_done: true,
            };
        }
        // turn.started / turn.completed / anything a later release adds.
        _ => {}
    }
    LineOutcome {
        events,
        turn_done: false,
    }
}

/// The stderr signature of a resume leg whose thread no longer exists
/// (probed: rc=1, empty stdout,
/// `Error: thread/resume: ... no rollout found for thread id ... (code -32600)`).
pub(crate) fn is_resume_not_found(stderr: &str) -> bool {
    stderr.contains("no rollout found for thread id")
}

/// User messages beyond `sent`, joined. Assistant and tool-result entries are
/// skipped: codex's own thread already holds them, and this backend never
/// emits `ToolUse`, so no `ToolResult` can originate here.
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

/// The turn text for a leg where codex holds nothing yet: the persona/system
/// text (there is no system-prompt flag to carry it) followed by the whole
/// transcript, flattened.
pub(crate) fn fresh_prompt(messages: &[Message]) -> String {
    let (system, body) = flatten_history(messages);
    match system {
        Some(sys) if body.is_empty() => sys,
        Some(sys) => format!("{sys}\n\n{body}"),
        None => body,
    }
}

/// System messages joined into one string, for change detection (a persona
/// change re-enters through a fresh conversation, as the claude-code resident
/// path respawns).
fn system_text(messages: &[Message]) -> Option<String> {
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
        "codex-code",
        format!(
            "Codex CLI executable not found: \"{raw}\".\n  \
             Install the Codex CLI, or set [provider.codex-code] bin to its full path."
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

/// The child's piped ends, plus a background drain of stderr. Every stderr
/// line is forwarded over a channel, so the turn's end can await the drain's
/// completion (stderr EOF) before quoting the tail — `wait()` alone races
/// with the reader, which can still be mid-line when the child's exit status
/// is already known.
struct ChildParts {
    child: Child,
    stdout: Lines<BufReader<ChildStdout>>,
    stderr_tail: tokio::sync::mpsc::UnboundedReceiver<String>,
}

/// Spawn one `codex exec` turn: write the prompt to stdin and close the pipe
/// (`-` reads to EOF — a pipe held open would keep the run waiting for a
/// prompt that never completes, as observed on the probed CLI).
async fn spawn(settings: &Settings, args: &[String], prompt: &str) -> Result<ChildParts> {
    let mut cmd = Command::new(&settings.bin);
    cmd.arg("exec")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Guarantees no orphan survives agent-cli exiting or the provider
        // being dropped mid-turn.
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| {
        AppError::provider(
            "codex-code",
            format!("failed to spawn {}: {e}", settings.bin.display()),
        )
    })?;
    {
        let mut stdin: ChildStdin = child.stdin.take().ok_or_else(|| {
            AppError::provider("codex-code", "child stdin unavailable".to_string())
        })?;
        // A BrokenPipe on the prompt write is not a failure of this turn: it
        // means the child stopped reading before the prompt was complete —
        // which is exactly what codex does when it refuses a run before
        // reading its input (the trust gate, a bad flag) or when the process
        // exits early. The turn's own outcome is carried by the exit status
        // and the stderr tail, which the EOF path reads; failing the turn
        // here instead would surface "broken pipe" and hide the real reason.
        match stdin.write_all(prompt.as_bytes()).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                tracing::debug!(target: "agent_cli::ai::codex_code", "child closed stdin before the prompt finished: deferring to its exit status");
            }
            Err(e) => {
                return Err(AppError::provider(
                    "codex-code",
                    format!("failed to send the prompt: {e}"),
                ));
            }
        }
        // Close: the prompt is complete. Same BrokenPipe tolerance — the pipe
        // closing under us is the child exiting, not a failed send.
        if let Err(e) = stdin.shutdown().await {
            if e.kind() != std::io::ErrorKind::BrokenPipe {
                return Err(AppError::provider(
                    "codex-code",
                    format!("failed to close the prompt pipe: {e}"),
                ));
            }
        }
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::provider("codex-code", "child stdout unavailable"))?;
    // Drained continuously: an undrained pipe would block the child once the
    // buffer fills, and the tail is what error messages quote. Lines ride an
    // unbounded channel (send never blocks), so the drain also finishes on
    // its own once stderr hits EOF.
    let (stderr_tx, stderr_tail) = tokio::sync::mpsc::unbounded_channel();
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if stderr_tx.send(line).is_err() {
                    break;
                }
            }
        });
    }
    Ok(ChildParts {
        child,
        stdout: BufReader::new(stdout).lines(),
        stderr_tail,
    })
}

/// Consume the stderr channel to EOF: the caller reads this only after the
/// child has exited, so the pipe (already closing) drains promptly. The last
/// `STDERR_TAIL_LINES` lines are kept — errors quote the tail, not the whole
/// stream.
async fn stderr_text(tail: &mut tokio::sync::mpsc::UnboundedReceiver<String>) -> String {
    use std::collections::VecDeque;
    let mut lines = VecDeque::new();
    while let Some(line) = tail.recv().await {
        if lines.len() == STDERR_TAIL_LINES {
            lines.pop_front();
        }
        lines.push_back(line);
    }
    lines.into_iter().collect::<Vec<_>>().join("\n")
}

pub struct CodexCodeProvider {
    pub(crate) settings: Settings,
    context: ProviderContext,
    /// The thread id to `resume` after the first turn of a conversation.
    resume_id: Mutex<Option<String>>,
    /// History entries already delivered to codex (a fresh leg's flattened
    /// conversation or the turns a resume leg sent): only newer user turns go
    /// out again.
    sent: Mutex<usize>,
    /// The system text the current thread was opened with; a change forces a
    /// fresh conversation (the persona would otherwise be lost).
    current_system: Mutex<Option<String>>,
}

impl CodexCodeProvider {
    pub fn from_config(cfg: &Config, source: &ConfigSource) -> Result<Self> {
        // An absent [provider.codex-code] section is fine: every key has a
        // default, and the backend needs no credentials of its own (codex
        // carries its own authentication, as the claude-code backend does).
        let entry = cfg.provider.codex_code.clone().unwrap_or_default();
        let bin = resolve_bin(entry.bin.as_deref().unwrap_or(DEFAULT_BIN))?;
        let settings = Settings::from_entry(&entry, bin)?;
        Ok(Self {
            settings,
            context: ProviderContext::new(source, None, None),
            resume_id: Mutex::new(None),
            sent: Mutex::new(0),
            current_system: Mutex::new(None),
        })
    }

    fn error(&self, stage: &str, detail: impl fmt::Display) -> String {
        format!(
            "{stage}\n  binary     : {}\n  transport  : {}\n  config     : {}\n  detail     : {detail}",
            self.settings.bin.display(),
            match self.settings.transport {
                Transport::Items => "items",
                Transport::OneShot => "oneshot",
            },
            self.context.config_path.display(),
        )
    }

    /// Drop the thread: the next turn re-enters fresh and re-flattens.
    async fn clear_thread(&self) {
        *self.resume_id.lock().await = None;
        *self.sent.lock().await = 0;
        *self.current_system.lock().await = None;
    }

    /// The prompt text for this turn, and how the session advances.
    ///
    /// A fresh persistent leg carries the whole transcript flat (with the
    /// system text prepended — codex is seeing this conversation for the
    /// first time) and records the thread it opens; a resume leg sends only
    /// the user turns codex has not seen; an ephemeral leg re-flattens every
    /// turn (each conversation stands alone). A persona change re-enters
    /// fresh, as the claude-code resident path respawns on one.
    async fn plan_for_turn(&self, messages: &[Message]) -> Result<TurnPlan> {
        let system = system_text(messages);
        {
            let recorded = self.current_system.lock().await;
            if *recorded != system {
                drop(recorded);
                self.clear_thread().await;
            }
        }
        let action = {
            let guard = self.resume_id.lock().await;
            self.settings.session_action(guard.as_deref())
        };
        let mut sent = self.sent.lock().await;
        let prompt = match &action {
            SessionAction::Resume(_) => new_user_text(messages, *sent),
            // Fresh and sessionless legs both carry the conversation flat:
            // codex holds nothing of it yet.
            _ => fresh_prompt(messages),
        };
        *sent = messages.len();
        *self.current_system.lock().await = system;
        Ok(TurnPlan {
            prompt,
            session: action,
        })
    }

    /// One child per turn: `--json` streams the items, one-shot buffers the
    /// turn and reads `-o`'s file afterwards.
    async fn turn(&self, messages: &[Message]) -> Result<EventStream<'_>> {
        match self.settings.transport {
            Transport::Items => self.items_turn(messages).await,
            Transport::OneShot => self.oneshot_turn(messages).await,
        }
    }

    async fn items_turn(&self, messages: &[Message]) -> Result<EventStream<'_>> {
        let plan = self.plan_for_turn(messages).await?;
        let args = build_args(&self.settings, &plan);
        let had_thread = matches!(plan.session, SessionAction::Resume(_));
        let parts = spawn(&self.settings, &args, &plan.prompt).await?;
        let deadline = Instant::now() + self.settings.turn_timeout;
        let stream = async_stream::stream! {
            let mut parts = parts;
            let mut st = LineState::default();
            loop {
                match tokio::time::timeout_at(deadline, parts.stdout.next_line()).await {
                    Err(_) => {
                        // A timed-out child is killed and the thread cleared:
                        // the next turn starts clean rather than resuming a
                        // turn whose state nobody knows.
                        let _ = parts.child.start_kill();
                        let _ = parts.child.wait().await;
                        self.clear_thread().await;
                        yield ProviderEvent::Error {
                            message: self.error("codex-code: turn timed out", format!("no result within {:?}", self.settings.turn_timeout)),
                        };
                        yield ProviderEvent::Done;
                        return;
                    }
                    Ok(Err(e)) => {
                        self.clear_thread().await;
                        yield ProviderEvent::Error { message: self.error("codex-code: read failed", e) };
                        yield ProviderEvent::Done;
                        return;
                    }
                    Ok(Ok(None)) => {
                        // stdout closed: the turn is over. Codex emits no
                        // result line — EOF **is** the normal end (probed:
                        // rc=0 runs end at EOF) — so only a nonzero exit is
                        // the error path. A lost thread (the resume-not-found
                        // signature) also clears the recorded id, letting the
                        // next turn re-enter fresh.
                        let status = parts.child.wait().await;
                        let stderr = stderr_text(&mut parts.stderr_tail).await;
                        // One read of the status: success ends the turn quietly
                        // (EOF is codex's normal end), anything else is an
                        // error citing the exit state and the stderr tail.
                        let (ok, exit_text) = match &status {
                            // `ExitStatus`' Display already says "exit code N"
                            // (or "signal: ..."), so the message must not add
                            // its own "exit" again.
                            Ok(s) => (s.success(), s.to_string()),
                            Err(e) => (false, format!("wait failed: {e}")),
                        };
                        if had_thread && is_resume_not_found(&stderr) {
                            self.clear_thread().await;
                        }
                        if ok {
                            yield ProviderEvent::Done;
                        } else {
                            yield ProviderEvent::Error {
                                message: self.error(
                                    "codex-code: ended without a result",
                                    format!("{exit_text} {stderr}"),
                                ),
                            };
                            yield ProviderEvent::Done;
                        }
                        return;
                    }
                    Ok(Ok(Some(line))) => {
                        let outcome = map_line(&line, &mut st);
                        if let Some(tid) = st.thread.take() {
                            self.record_thread(&tid).await;
                        }
                        for ev in outcome.events {
                            yield ev;
                        }
                        if outcome.turn_done {
                            return;
                        }
                    }
                }
            }
        };
        Ok(Box::pin(stream))
    }

    async fn record_thread(&self, tid: &str) {
        self.resume_id.lock().await.replace(tid.to_string());
    }

    /// One child per turn, no `--json`: the prompt rides argv, the reply is
    /// read back from `-o`'s file after the child exits. A one-shot
    /// conversation is sessionless by design — without `--json` no thread id
    /// is ever seen, so the turn always ends with the thread cleared.
    async fn oneshot_turn(&self, messages: &[Message]) -> Result<EventStream<'_>> {
        let plan = self.plan_for_turn(messages).await?;
        let args = build_args(&self.settings, &plan);
        // build_args leaves the `-` marker off for this transport: the prompt
        // is the argv tail (claude-code's `plan.prompt` geometry, P5b).

        let output = tokio::time::timeout(self.settings.turn_timeout, {
            let mut cmd = Command::new(&self.settings.bin);
            cmd.arg("exec")
                .args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .output()
        })
        .await;

        let events = match output {
            Err(_) => vec![
                ProviderEvent::Error {
                    message: self.error(
                        "codex-code: turn timed out",
                        format!("no result within {:?}", self.settings.turn_timeout),
                    ),
                },
                ProviderEvent::Done,
            ],
            Ok(Err(e)) => vec![
                ProviderEvent::Error {
                    message: self.error("codex-code: failed to run", e),
                },
                ProviderEvent::Done,
            ],
            Ok(Ok(out)) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if !out.status.success() {
                    vec![
                        ProviderEvent::Error {
                            message: self.error(
                                &format!("codex-code: exited with {}", out.status),
                                stderr.trim().to_string(),
                            ),
                        },
                        ProviderEvent::Done,
                    ]
                } else {
                    match tokio::fs::read_to_string(oneshot_file()).await {
                        Ok(text) if !text.trim().is_empty() => vec![
                            ProviderEvent::Text {
                                delta: text.trim_end().to_string(),
                            },
                            ProviderEvent::Done,
                        ],
                        _ => vec![
                            ProviderEvent::Error {
                                message: self.error(
                                    "codex-code: no last message",
                                    format!("{} absent or empty", oneshot_file()),
                                ),
                            },
                            ProviderEvent::Done,
                        ],
                    }
                }
            }
        };
        // Sessionless by design (no thread id without `--json`).
        self.clear_thread().await;
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

#[async_trait]
impl Provider for CodexCodeProvider {
    fn name(&self) -> &'static str {
        "codex-code"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            // Items, not token-level deltas: text arrives whole per completed
            // item (probed) — the display is stream-ordered but turn-grain.
            // One-shot buffers the whole turn.
            streaming: self.settings.transport == Transport::Items,
            // Tools are codex's own, executed inside it (probed: typed
            // command_execution items on fresh and resume legs); nothing is
            // offered to the agent loop.
            tool_use: false,
            thinking: false,
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
        // `_tools` is unused by design: `codex exec` accepts no external tool
        // definitions (see the module header), so agent-cli's registry cannot
        // be offered to it — the same delegation rule claude-code's module
        // header records.
        self.turn(messages).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            bin: PathBuf::from("/usr/bin/codex"),
            model: None,
            transport: Transport::Items,
            session: SessionMode::Persistent,
            sandbox: DEFAULT_SANDBOX.to_string(),
            add_dir: Vec::new(),
            approval_policy: None,
            bypass_sandbox_and_approvals: false,
            skip_git_repo_check: true,
            turn_timeout: Duration::from_secs(900),
            extra_args: Vec::new(),
        }
    }

    fn plan(session: SessionAction) -> TurnPlan {
        TurnPlan {
            prompt: "hi".into(),
            session,
        }
    }

    #[test]
    fn new_leg_args_are_probe_verifyable() {
        let args = build_args(&settings(), &plan(SessionAction::New));
        assert_eq!(
            args,
            vec![
                "--json",
                "-s",
                "workspace-write",
                "--skip-git-repo-check",
                "-"
            ]
        );
    }

    #[test]
    fn resume_leg_flags_precede_the_subcommand() {
        // Live-verified geometry: `codex exec resume [OPTIONS] [SESSION_ID]`
        // rejects exec-level flags placed after the subcommand ("unexpected
        // argument '-s' found", probe of the wired backend), while flags on
        // the exec level before it reach the resumed run.
        let args = build_args(&settings(), &plan(SessionAction::Resume("tid".into())));
        assert_eq!(
            args,
            vec![
                "--json",
                "-s",
                "workspace-write",
                "--skip-git-repo-check",
                "resume",
                "tid",
                "-"
            ]
        );
    }

    #[test]
    fn ephemeral_leg_adds_the_flag_and_never_resumes() {
        let mut s = settings();
        s.session = SessionMode::Ephemeral;
        let args = build_args(&s, &plan(SessionAction::None));
        assert!(args.contains(&"--ephemeral".to_string()));
        assert!(!args.contains(&"resume".to_string()));
    }

    #[test]
    fn sandbox_model_and_policy_flags_are_forwarded() {
        let mut s = settings();
        s.sandbox = "danger-full-access".into();
        s.model = Some("gpt-5.6-terra".into());
        s.approval_policy = Some("never".into());
        s.add_dir = vec!["/tmp".into()];
        let args = build_args(&s, &plan(SessionAction::New));
        assert!(args.windows(2).any(|w| w == ["-s", "danger-full-access"]));
        assert!(args.windows(2).any(|w| w == ["-m", "gpt-5.6-terra"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["-c", "approval_policy=never"]));
        assert!(args.windows(2).any(|w| w == ["--add-dir", "/tmp"]));
    }

    #[test]
    fn bypass_and_trust_flags_are_honored() {
        let mut s = settings();
        s.bypass_sandbox_and_approvals = true;
        s.skip_git_repo_check = false;
        let args = build_args(&s, &plan(SessionAction::New));
        assert!(args
            .contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
        assert!(!args.contains(&"--skip-git-repo-check".to_string()));
    }

    #[test]
    fn oneshot_uses_the_output_file_and_argv_prompt() {
        let mut s = settings();
        s.transport = Transport::OneShot;
        let args = build_args(&s, &plan(SessionAction::None));
        assert!(args.windows(2).any(|w| w == ["--output-last-message", oneshot_file()]));
        assert!(!args.contains(&"--json".to_string()));
        assert_eq!(args.last().unwrap(), "hi");
    }

    #[test]
    fn unknown_enum_values_are_rejected_with_the_accepted_set() {
        let mut entry = ProviderEntry {
            transport: Some("socket".into()),
            ..Default::default()
        };
        let err = Settings::from_entry(&entry, PathBuf::from("codex")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("transport"), "{msg}");
        assert!(msg.contains("items"), "{msg}");

        entry.transport = None;
        entry.session = Some("hybrid".into());
        let err = Settings::from_entry(&entry, PathBuf::from("codex")).unwrap_err();
        assert!(err.to_string().contains("session"));
    }

    #[test]
    fn defaults_match_the_documented_ones() {
        let s = Settings::from_entry(&ProviderEntry::default(), PathBuf::from("codex")).unwrap();
        assert_eq!(s.transport, Transport::Items);
        assert_eq!(s.session, SessionMode::Persistent);
        assert_eq!(s.sandbox, "workspace-write");
        assert!(s.skip_git_repo_check);
        assert!(!s.bypass_sandbox_and_approvals);
        assert_eq!(s.turn_timeout, Duration::from_secs(900));
        assert!(s.extra_args.is_empty());
    }

    fn events(lines: &[&str]) -> (Vec<ProviderEvent>, bool, Option<String>) {
        let mut st = LineState::default();
        let mut out = Vec::new();
        let mut done = false;
        for l in lines {
            let outcome = map_line(l, &mut st);
            out.extend(outcome.events);
            if outcome.turn_done {
                done = true;
                break;
            }
        }
        (out, done, st.thread.clone())
    }

    fn text_of(evs: &[ProviderEvent]) -> String {
        evs.iter()
            .filter_map(|e| match e {
                ProviderEvent::Text { delta } => Some(delta.clone()),
                _ => None,
            })
            .collect()
    }

    /// The probe-recorded fresh-leg transcript, verbatim shapes.
    #[test]
    fn fresh_leg_stream_maps_to_text_and_pins_the_thread() {
        let (evs, done, thread) = events(&[
            r#"{"type":"thread.started","thread_id":"01a1aa4b-7794-7982-a688-b57decc9a63d"}"#,
            r#"{"type":"turn.started"}"#,
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"P1OK"}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":12702,"cached_input_tokens":9984,"cache_write_input_tokens":0,"output_tokens":7,"reasoning_output_tokens":0}}"#,
        ]);
        assert_eq!(text_of(&evs), "P1OK");
        assert!(!done); // EOF ends the turn, not turn.completed
        assert_eq!(thread.as_deref(), Some("01a1aa4b-7794-7982-a688-b57decc9a63d"));
        assert!(!evs.iter().any(|e| matches!(e, ProviderEvent::Done)));
    }

    /// The probe-recorded delegation transcript: command_execution items are
    /// observed but never become tool events.
    #[test]
    fn command_execution_items_never_become_tool_events() {
        let (evs, done, _) = events(&[
            r#"{"type":"thread.started","thread_id":"tid"}"#,
            r#"{"type":"turn.started"}"#,
            r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"I’ll run the requested command."}}"#,
            r#"{"type":"item.started","item":{"id":"item_1","type":"command_execution","command":"/bin/bash -lc 'echo BIPROBE9'","aggregated_output":"","exit_code":null,"status":"in_progress"}}"#,
            r#"{"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"/bin/bash -lc 'echo BIPROBE9'","aggregated_output":"BIPROBE9\n","exit_code":0,"status":"completed"}}"#,
            r#"{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"DELEG1"}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":26343}}"#,
        ]);
        assert_eq!(text_of(&evs), "I’ll run the requested command.DELEG1");
        assert!(!evs
            .iter()
            .any(|e| matches!(e, ProviderEvent::ToolUse { .. })));
        assert!(!done);
    }

    #[test]
    fn turn_failed_yields_error_then_done_and_ends_the_turn() {
        let (evs, done, _) = events(&[
            r#"{"type":"thread.started","thread_id":"tid"}"#,
            r#"{"type":"turn.started"}"#,
            r#"{"type":"error","message":"The 'no-such-model-xyz' model is not supported when using Codex with a ChatGPT account."}"#,
            r#"{"type":"turn.failed","error":{"message":"The 'no-such-model-xyz' model is not supported when using Codex with a ChatGPT account."}}"#,
            r#"{"type":"turn.completed"}"#,
        ]);
        assert!(done);
        assert!(matches!(
            &evs[0],
            ProviderEvent::Error { message } if message.contains("no-such-model-xyz")
        ));
        assert!(matches!(evs.last(), Some(ProviderEvent::Done)));
    }

    #[test]
    fn item_error_is_forwarded_without_ending_the_turn() {
        let (evs, done, _) = events(&[
            r#"{"type":"item.completed","item":{"id":"item_0","type":"error","message":"Model metadata for `no-such-model-xyz` not found."}}"#,
        ]);
        assert!(!done);
        assert!(matches!(
            &evs[0],
            ProviderEvent::Error { message } if message.contains("no-such-model-xyz")
        ));
    }

    #[test]
    fn unknown_and_unparsable_lines_are_ignored() {
        let (evs, done, _) = events(&[
            r#"{"type":"turn.started"}"#,
            "not json at all",
            "",
            r#"{"type":"something_new","payload":1}"#,
        ]);
        assert!(evs.is_empty());
        assert!(!done);
    }

    #[test]
    fn resume_not_found_matches_only_the_probe_recorded_signature() {
        assert!(is_resume_not_found(
            "Error: thread/resume: thread/resume failed: no rollout found for thread id 00000000-0000-0000-0000-000000000000 (code -32600)"
        ));
        assert!(!is_resume_not_found(
            "Not inside a trusted directory and --skip-git-repo-check was not specified."
        ));
        assert!(!is_resume_not_found(
            "The 'no-such-model-xyz' model is not supported when using Codex with a ChatGPT account."
        ));
    }

    #[test]
    fn new_user_text_sends_only_undelivered_turns() {
        let messages = vec![
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
        assert_eq!(new_user_text(&messages, 2), "second");
        assert_eq!(new_user_text(&messages, 3), "");
    }

    #[test]
    fn fresh_prompt_prepends_the_system_text() {
        let messages = vec![
            Message::System {
                content: "be terse".into(),
            },
            Message::User {
                content: "hello".into(),
            },
            Message::Assistant {
                content: "hi".into(),
                tool_calls: vec![],
                reasoning_content: None,
            },
            Message::User {
                content: "again".into(),
            },
        ];
        let text = fresh_prompt(&messages);
        assert!(text.starts_with("be terse\n\n"), "{text}");
        assert!(text.contains("User: hello"));
        assert!(text.contains("Assistant: hi"));
        assert!(text.trim_end().ends_with("User: again"));
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

        /// Write an executable stub that stands in for `codex`.
        fn stub_bin(dir: &std::path::Path, body: &str) -> PathBuf {
            let path = dir.join("codex-stub.sh");
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

        fn provider(bin: PathBuf, tweak: impl FnOnce(&mut Settings)) -> CodexCodeProvider {
            let mut settings = super::tests::settings();
            settings.bin = bin;
            tweak(&mut settings);
            CodexCodeProvider {
                settings,
                context: ProviderContext {
                    config_path: PathBuf::from("/tmp/config.toml"),
                    api_key_env: None,
                    api_key_mask: None,
                },
                resume_id: Mutex::new(None),
                sent: Mutex::new(0),
                current_system: Mutex::new(None),
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
        /// (a provider spawn failure surfaces as `Err`, a one-shot spawn
        /// failure as an `Error` event).
        async fn run_turn(p: &CodexCodeProvider, messages: &[Message]) -> Vec<ProviderEvent> {
            for _ in 0..50 {
                match p.complete_stream(messages, &[]).await {
                    Ok(mut stream) => {
                        let evs = drain(&mut stream).await;
                        drop(stream);
                        let busy = matches!(
                            evs.first(),
                            Some(ProviderEvent::Error { message }) if message.contains("Text file busy")
                        );
                        if !busy {
                            return evs;
                        }
                    }
                    Err(e) if e.to_string().contains("Text file busy") => {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    Err(e) => panic!("complete_stream failed: {e}"),
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            panic!("stub stayed busy");
        }

        /// Two turns on one stub conversation: the first leg is `exec`, the
        /// second `resume` — and the prompt rode stdin on both.
        #[tokio::test]
        async fn second_leg_resumes_the_thread_the_first_opened() {
            let dir = tempfile::tempdir().unwrap();
            let bin = stub_bin(
                dir.path(),
                r#"
resume=no
for a in "$@"; do [ "$a" = resume ] && resume=yes; done
if [ "$resume" = yes ]; then
  printf '{"type":"thread.started","thread_id":"tid-a"}\n'
  printf '{"type":"item.completed","item":{"id":"i","type":"agent_message","text":"leg2"}}\n'
else
  printf '{"type":"thread.started","thread_id":"tid-a"}\n'
  printf '{"type":"item.completed","item":{"id":"i","type":"agent_message","text":"leg1"}}\n'
fi
"#,
            );
            let p = provider(bin, |_| {});
            let evs = run_turn(&p, &[user("one")]).await;
            assert_eq!(super::tests::text_of(&evs), "leg1");
            assert_eq!(*p.resume_id.lock().await, Some("tid-a".into()));

            let history = vec![
                user("one"),
                Message::Assistant {
                    content: "leg1".into(),
                    tool_calls: vec![],
                    reasoning_content: None,
                },
                user("two"),
            ];
            let evs = run_turn(&p, &history).await;
            assert_eq!(super::tests::text_of(&evs), "leg2");
        }

        #[tokio::test]
        async fn non_zero_exit_yields_one_error_then_done() {
            let dir = tempfile::tempdir().unwrap();
            let bin = stub_bin(
                dir.path(),
                "echo 'no rollout found for thread id x (code -32600)' >&2\nexit 1",
            );
            let p = provider(bin, |_| {});
            let evs = run_turn(&p, &[user("hi")]).await;
            assert_eq!(evs.len(), 2);
            assert!(
                matches!(&evs[0], ProviderEvent::Error { message } if message.contains("no rollout found"))
            );
            assert!(matches!(evs[1], ProviderEvent::Done));
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
            // The dead thread was cleared, so the next turn starts fresh.
            assert!(p.resume_id.lock().await.is_none());
        }

        #[tokio::test]
        async fn oneshot_reads_the_output_file_the_stub_wrote() {
            let dir = tempfile::tempdir().unwrap();
            let bin = stub_bin(
                dir.path(),
                "cat > .agent-cli-codex-code-last.txt <<'EOF'\nwhole reply\nEOF",
            );
            let p = provider(bin, |s| s.transport = Transport::OneShot);
            let evs = run_turn(&p, &[user("hi")]).await;
            assert_eq!(super::tests::text_of(&evs), "whole reply");
            assert!(matches!(evs.last(), Some(ProviderEvent::Done)));
            // Sessionless by design: no thread survives the one-shot leg.
            assert!(p.resume_id.lock().await.is_none());
        }

        #[tokio::test]
        async fn eof_with_success_ends_the_turn_even_without_a_result_line() {
            let dir = tempfile::tempdir().unwrap();
            // No JSONL at all: codex's normal run shape ends at EOF.
            let bin = stub_bin(dir.path(), "# nothing on stdout");
            let p = provider(bin, |_| {});
            let evs = run_turn(&p, &[user("hi")]).await;
            assert!(evs.len() == 1 || evs.len() == 2, "{evs:?}");
            assert!(
                !evs.iter().any(|e| matches!(e, ProviderEvent::Error { .. })),
                "a successful EOF must not be an error: {evs:?}"
            );
            assert!(matches!(evs.last(), Some(ProviderEvent::Done)));
        }
    }
}
