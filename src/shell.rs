//! Shell commands typed at the REPL prompt (`!<command>`).
//!
//! Three things live here: the pure classification of a submitted line (which
//! is where the feature's safety rule is decided — only a line the user typed
//! can ever be a command), the runner that spawns the command and streams its
//! output back a line at a time, and the rendering of the text the model is
//! given afterwards.
//!
//! The command's stdio is piped and its stdin is closed: the terminal is never
//! handed over, so raw mode stays on, `Ctrl-C` stays a key rather than a signal
//! that would shut the REPL down, and every line of output goes out through the
//! REPL's own writers — where the wheel scrollback can see it. Interactive and
//! full-screen programs are therefore not what `!` is for.

use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Notify};

use crate::config::ShellConfig;

/// Marker prefixed to the model's copy of the output when the beginning of it
/// had to be dropped.
const TRUNCATED_MARK: &str = "…[earlier output truncated]";

/// What a submitted prompt line is.
///
/// The `!` arm is the whole safety rule: it is reachable only from the input
/// loops, which see what the user typed. A peer's prompt arrives as
/// `AgentInput::PeerPrompt` and never passes through here, so no remote agent —
/// and nothing the model produced — can run a command.
#[derive(Debug, PartialEq, Eq)]
pub enum Line<'a> {
    /// `/…` — a REPL command.
    Command(&'a str),
    /// `!…` — a shell command.
    Shell(&'a str),
    /// Anything else — a prompt for the model.
    Prompt(&'a str),
    /// `!` with nothing after it.
    ShellUsage,
    /// An empty line.
    Empty,
}

/// Classify an already-trimmed submitted line.
///
/// With `shell_enabled` false a `!` line is an ordinary prompt, which is
/// exactly what it was before this feature existed.
pub fn classify_line(line: &str, shell_enabled: bool) -> Line<'_> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Line::Empty;
    }
    if let Some(rest) = trimmed.strip_prefix('/') {
        return Line::Command(rest);
    }
    if shell_enabled {
        if let Some(rest) = trimmed.strip_prefix('!') {
            let cmd = rest.trim();
            return if cmd.is_empty() {
                Line::ShellUsage
            } else {
                Line::Shell(cmd)
            };
        }
    }
    Line::Prompt(trimmed)
}

/// How a command ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellStatus {
    Exited(i32),
    TimedOut { after_ms: u64 },
    Cancelled,
    /// The command could not be started at all.
    Failed(String),
}

impl ShellStatus {
    /// One-line rendering for the model and the log.
    pub fn describe(&self) -> String {
        match self {
            ShellStatus::Exited(code) => format!("exit status: {code}"),
            ShellStatus::TimedOut { after_ms } => {
                format!("timed out after {after_ms} ms and was killed")
            }
            ShellStatus::Cancelled => "cancelled by the user before it finished".to_string(),
            ShellStatus::Failed(e) => format!("could not be started: {e}"),
        }
    }
}

/// A finished command: what was run, how it ended, and as much of its output as
/// the model is allowed to see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellOutcome {
    pub command: String,
    pub status: ShellStatus,
    pub output: String,
    /// Output was dropped to fit the cap.
    pub truncated: bool,
}

/// The message appended to the conversation after a command has run.
///
/// One place decides what the model is told, so the wording can be asserted by
/// a test rather than discovered in a transcript.
pub fn context_message(outcome: &ShellOutcome) -> String {
    let mut out = String::with_capacity(outcome.output.len() + 128);
    out.push_str("I ran a shell command in my terminal.\n\n$ ");
    out.push_str(&outcome.command);
    out.push_str("\n\n");
    out.push_str(&outcome.status.describe());
    out.push('\n');
    if outcome.output.trim().is_empty() {
        out.push_str("output: (none)");
    } else {
        out.push_str("output:\n");
        if outcome.truncated {
            out.push_str(TRUNCATED_MARK);
            out.push('\n');
        }
        out.push_str(outcome.output.trim_end());
    }
    out
}

/// Cut `output` to `max_kb` kilobytes, keeping the **tail** — the end of a
/// build log is what carries the error — and reporting whether it had to cut.
/// The cut lands on a character boundary, and on a line boundary when there is
/// one nearby, so the model is never handed half a line of noise.
pub fn cap_output(output: &str, max_kb: u64) -> (String, bool) {
    let max_bytes = (max_kb as usize).saturating_mul(1024);
    if output.len() <= max_bytes {
        return (output.to_string(), false);
    }
    if max_bytes == 0 {
        return (String::new(), true);
    }
    // Keep the last `max_bytes`, moving forward to a character boundary.
    let mut start = output.len() - max_bytes;
    while start < output.len() && !output.is_char_boundary(start) {
        start += 1;
    }
    let tail = &output[start..];
    // Prefer starting at the next line break, as long as that does not throw
    // away most of what was kept.
    let tail = match tail.find('\n') {
        Some(idx) if idx < tail.len() / 4 => &tail[idx + 1..],
        _ => tail,
    };
    (tail.to_string(), true)
}

/// Something a running command produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellLine {
    Out(String),
    Err(String),
    /// The command ended; nothing more will arrive.
    End(ShellStatus),
}

/// A command running in the background of the REPL.
///
/// The lines arrive on a channel so the input loop can print them and still
/// poll for keys: a command never blocks the prompt, which is what makes `Esc`
/// able to cancel it and the timeout able to fire.
pub struct ShellRun {
    rx: mpsc::Receiver<ShellLine>,
    cancel: Arc<Notify>,
    command: String,
    /// The tail of the output, bounded by the configured cap as it grows, so a
    /// command that prints for an hour cannot grow the process.
    collected: VecDeque<String>,
    collected_bytes: usize,
    max_bytes: usize,
    truncated: bool,
}

impl ShellRun {
    /// Start `command` under `bash -lc`, with stdout and stderr piped and
    /// stdin closed, inheriting agent-cli's working directory and environment —
    /// the same execution model the `bash` tool uses.
    pub fn spawn(command: &str, cfg: &ShellConfig) -> Self {
        let (tx, rx) = mpsc::channel(256);
        let cancel = Arc::new(Notify::new());
        let timeout = Duration::from_millis(cfg.timeout_ms);
        tokio::spawn(run_command(
            command.to_string(),
            timeout,
            tx,
            cancel.clone(),
        ));
        Self {
            rx,
            cancel,
            command: command.to_string(),
            collected: VecDeque::new(),
            collected_bytes: 0,
            max_bytes: (cfg.max_output_kb as usize).saturating_mul(1024),
            truncated: false,
        }
    }

    /// The next line the command produced, or `None` once the channel is done.
    pub async fn next(&mut self) -> Option<ShellLine> {
        let line = self.rx.recv().await;
        match &line {
            Some(ShellLine::Out(s)) | Some(ShellLine::Err(s)) => self.record(s),
            _ => {}
        }
        line
    }

    /// Keep the line for the model, dropping from the front to stay inside the
    /// cap. The screen has already been given every line (this is only the
    /// copy the model gets).
    fn record(&mut self, line: &str) {
        if self.max_bytes == 0 {
            self.truncated = true;
            return;
        }
        self.collected_bytes += line.len() + 1;
        self.collected.push_back(line.to_string());
        while self.collected_bytes > self.max_bytes && self.collected.len() > 1 {
            if let Some(dropped) = self.collected.pop_front() {
                self.collected_bytes -= dropped.len() + 1;
                self.truncated = true;
            }
        }
    }

    /// Ask the command to stop: the runner terminates it, then kills it if it
    /// does not go, and ends with [`ShellStatus::Cancelled`].
    pub fn cancel(&self) {
        self.cancel.notify_waiters();
    }

    /// Everything the model is told about this command.
    pub fn finish(self, status: ShellStatus) -> ShellOutcome {
        let joined = self
            .collected
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        // The ring already bounds this; the cap is applied again so the
        // contract holds exactly, whatever the ring's line granularity did.
        let (output, cut) = cap_output(&joined, (self.max_bytes / 1024) as u64);
        ShellOutcome {
            command: self.command,
            status,
            output,
            truncated: self.truncated || cut,
        }
    }
}

/// Run the command, forwarding each line as it arrives, and end with exactly
/// one [`ShellLine::End`].
async fn run_command(
    command: String,
    timeout: Duration,
    tx: mpsc::Sender<ShellLine>,
    cancel: Arc<Notify>,
) {
    let mut cmd = Command::new("bash");
    cmd.arg("-lc")
        .arg(&command)
        // Closed stdin: the terminal is in raw mode and belongs to the REPL, so
        // a command that reads stdin must get EOF rather than the user's keys.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Its own process group, so cancelling can signal the whole job. `bash -lc`
    // usually forks for the command rather than exec'ing it, and signalling
    // only the shell would leave the real work orphaned and running.
    unsafe {
        cmd.pre_exec(|| {
            // Failure here is not fatal: the group simply stays the REPL's, and
            // the direct child is still signalled below.
            libc::setpgid(0, 0);
            Ok(())
        });
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx
                .send(ShellLine::End(ShellStatus::Failed(e.to_string())))
                .await;
            return;
        }
    };

    let mut out = BufReader::new(child.stdout.take().expect("piped")).lines();
    let mut err = BufReader::new(child.stderr.take().expect("piped")).lines();
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);
    let mut out_done = false;
    let mut err_done = false;

    let status = loop {
        tokio::select! {
            biased;
            _ = cancel.notified() => {
                terminate(&mut child).await;
                break ShellStatus::Cancelled;
            }
            // The REPL dropped the run (shutdown, or the prompt moved on):
            // stop the command rather than leaving it behind.
            _ = tx.closed() => {
                terminate(&mut child).await;
                break ShellStatus::Cancelled;
            }
            _ = &mut deadline => {
                terminate(&mut child).await;
                break ShellStatus::TimedOut { after_ms: timeout.as_millis() as u64 };
            }
            line = out.next_line(), if !out_done => match line {
                Ok(Some(l)) => { if tx.send(ShellLine::Out(l)).await.is_err() { break ShellStatus::Cancelled; } }
                _ => out_done = true,
            },
            line = err.next_line(), if !err_done => match line {
                Ok(Some(l)) => { if tx.send(ShellLine::Err(l)).await.is_err() { break ShellStatus::Cancelled; } }
                _ => err_done = true,
            },
            // Both pipes are at EOF, so the command has written everything it
            // is going to: now it is safe to wait for the exit status.
            res = child.wait(), if out_done && err_done => {
                break match res {
                    Ok(st) => ShellStatus::Exited(st.code().unwrap_or(-1)),
                    Err(e) => ShellStatus::Failed(e.to_string()),
                };
            }
        }
    };
    let _ = tx.send(ShellLine::End(status)).await;
}

/// SIGTERM the whole job, then SIGKILL what is left a moment later, so a
/// cancelled command cannot outlive the prompt it was typed at.
///
/// The signal goes to the process *group* (`kill(-pgid)`): the shell forks for
/// the command it was given, so signalling the shell alone would leave the real
/// work running with no parent.
async fn terminate(child: &mut tokio::process::Child) {
    let pid = child.id().map(|p| p as libc::pid_t);
    if let Some(pid) = pid {
        unsafe {
            libc::kill(-pid, libc::SIGTERM);
            libc::kill(pid, libc::SIGTERM);
        }
    }
    if tokio::time::timeout(Duration::from_millis(300), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
    if let Some(pid) = pid {
        // Whatever the shell left behind goes too.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leading_bang_is_a_shell_command() {
        assert_eq!(classify_line("!ls -la", true), Line::Shell("ls -la"));
        // The line arrives trimmed, but leading blanks must not defeat it.
        assert_eq!(classify_line("   !echo hi", true), Line::Shell("echo hi"));
        assert_eq!(classify_line("!  echo hi", true), Line::Shell("echo hi"));
    }

    #[test]
    fn a_bang_alone_asks_for_usage() {
        assert_eq!(classify_line("!", true), Line::ShellUsage);
        assert_eq!(classify_line("!   ", true), Line::ShellUsage);
    }

    #[test]
    fn everything_else_keeps_its_meaning() {
        assert_eq!(classify_line("/help", true), Line::Command("help"));
        assert_eq!(classify_line("", true), Line::Empty);
        assert_eq!(classify_line("   ", true), Line::Empty);
        // A `!` that is not first is ordinary text, not a command.
        assert_eq!(
            classify_line("what does ! mean", true),
            Line::Prompt("what does ! mean")
        );
        assert_eq!(
            classify_line("really?!", true),
            Line::Prompt("really?!")
        );
    }

    #[test]
    fn with_the_feature_off_a_bang_line_is_an_ordinary_prompt() {
        assert_eq!(classify_line("!ls", false), Line::Prompt("!ls"));
        assert_eq!(classify_line("!", false), Line::Prompt("!"));
        // The other arms are unaffected by the switch.
        assert_eq!(classify_line("/help", false), Line::Command("help"));
        assert_eq!(classify_line("hello", false), Line::Prompt("hello"));
    }

    fn outcome(status: ShellStatus, output: &str, truncated: bool) -> ShellOutcome {
        ShellOutcome {
            command: "git status --short".into(),
            status,
            output: output.into(),
            truncated,
        }
    }

    #[test]
    fn the_context_message_carries_the_command_status_and_output() {
        let msg = context_message(&outcome(ShellStatus::Exited(0), " M src/app.rs\n", false));
        assert!(msg.contains("$ git status --short"), "{msg}");
        assert!(msg.contains("exit status: 0"), "{msg}");
        assert!(msg.contains(" M src/app.rs"), "{msg}");
        assert!(!msg.contains(TRUNCATED_MARK));
    }

    #[test]
    fn a_failed_cancelled_or_timed_out_command_is_distinguishable() {
        let failed = context_message(&outcome(ShellStatus::Exited(2), "boom", false));
        assert!(failed.contains("exit status: 2"), "{failed}");
        let cancelled = context_message(&outcome(ShellStatus::Cancelled, "partial", false));
        assert!(cancelled.contains("cancelled by the user"), "{cancelled}");
        let timed = context_message(&outcome(
            ShellStatus::TimedOut { after_ms: 1000 },
            "",
            false,
        ));
        assert!(timed.contains("timed out after 1000 ms"), "{timed}");
        assert!(timed.contains("output: (none)"), "{timed}");
        let bad = context_message(&outcome(ShellStatus::Failed("nope".into()), "", false));
        assert!(bad.contains("could not be started: nope"), "{bad}");
    }

    #[test]
    fn a_truncated_output_says_so() {
        let msg = context_message(&outcome(ShellStatus::Exited(0), "tail lines", true));
        assert!(msg.contains(TRUNCATED_MARK), "{msg}");
        assert!(msg.contains("tail lines"), "{msg}");
    }

    #[test]
    fn cap_output_keeps_the_tail_and_reports_the_cut() {
        let small = "short output";
        assert_eq!(cap_output(small, 1), (small.to_string(), false));

        let big: String = (0..500).map(|i| format!("line {i}\n")).collect();
        let (capped, cut) = cap_output(&big, 1);
        assert!(cut);
        assert!(capped.len() <= 1024);
        assert!(
            capped.contains("line 499"),
            "the end of the output is what matters"
        );
        assert!(!capped.contains("line 0\n"));
        // The cut lands on a line boundary rather than mid-line.
        assert!(capped.starts_with("line "), "{capped:?}");
    }

    #[test]
    fn cap_output_handles_degenerate_caps_and_multibyte_text() {
        let (empty, cut) = cap_output("anything", 0);
        assert!(cut);
        assert!(empty.is_empty());
        // Cutting must land on a character boundary, never inside a code point.
        let cjk = "あ".repeat(2000);
        let (capped, cut) = cap_output(&cjk, 1);
        assert!(cut);
        assert!(capped.chars().all(|c| c == 'あ'));
        assert!(capped.len() <= 1024);
    }

    fn cfg(timeout_ms: u64, max_output_kb: u64) -> ShellConfig {
        ShellConfig {
            enabled: true,
            timeout_ms,
            max_output_kb,
            context: true,
        }
    }

    async fn drain(run: &mut ShellRun) -> (Vec<String>, ShellStatus) {
        let mut lines = Vec::new();
        while let Some(line) = run.next().await {
            match line {
                ShellLine::Out(s) | ShellLine::Err(s) => lines.push(s),
                ShellLine::End(status) => return (lines, status),
            }
        }
        panic!("the run ended without a terminal status");
    }

    #[tokio::test]
    async fn a_command_streams_its_output_and_reports_its_status() {
        let mut run = ShellRun::spawn("echo one; echo two 1>&2; exit 3", &cfg(5_000, 64));
        let (lines, status) = drain(&mut run).await;
        assert_eq!(lines, vec!["one", "two"]);
        assert_eq!(status, ShellStatus::Exited(3));
        let out = run.finish(status);
        assert_eq!(out.command, "echo one; echo two 1>&2; exit 3");
        assert!(out.output.contains("one") && out.output.contains("two"));
        assert!(!out.truncated);
    }

    #[tokio::test]
    async fn a_command_that_overruns_its_timeout_is_killed() {
        let mut run = ShellRun::spawn("sleep 30", &cfg(300, 64));
        let (_, status) = drain(&mut run).await;
        assert_eq!(status, ShellStatus::TimedOut { after_ms: 300 });
    }

    #[tokio::test]
    async fn cancelling_ends_the_run_and_keeps_what_was_printed() {
        let mut run = ShellRun::spawn("echo first; sleep 30", &cfg(30_000, 64));
        // Wait for the first line, then cancel as the input loop would.
        let first = run.next().await;
        assert_eq!(first, Some(ShellLine::Out("first".into())));
        run.cancel();
        let (_, status) = drain(&mut run).await;
        assert_eq!(status, ShellStatus::Cancelled);
        let out = run.finish(ShellStatus::Cancelled);
        assert!(out.output.contains("first"), "partial output is kept");
    }

    #[tokio::test]
    async fn the_model_copy_is_capped_while_the_command_still_runs_to_the_end() {
        // 400 lines of ~20 bytes against a 1 KiB cap: the run must complete and
        // keep only the tail.
        let mut run = ShellRun::spawn("for i in $(seq 1 400); do echo line-$i; done", &cfg(10_000, 1));
        let (lines, status) = drain(&mut run).await;
        assert_eq!(status, ShellStatus::Exited(0));
        assert_eq!(lines.len(), 400, "every line reached the screen");
        let out = run.finish(status);
        assert!(out.truncated, "the model's copy was cut");
        assert!(out.output.len() <= 1024);
        assert!(out.output.contains("line-400"), "the tail is what is kept");
        assert!(!out.output.contains("line-1\n"));
    }
}
