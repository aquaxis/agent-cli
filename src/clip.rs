//! Putting text on the system clipboard.
//!
//! Two routes, in this order:
//!
//! * **OSC 52** (the default) — `\x1b]52;c;<base64>` written to the terminal,
//!   which puts it on the clipboard of whoever is *looking at* the session.
//!   That matters: agent-cli commonly runs over SSH or inside a container,
//!   where a helper binary would copy into a clipboard nobody can paste from,
//!   and often there is no helper binary at all.
//! * **`[ui] copy_command`** — the text piped to a command's stdin
//!   (`wl-copy`, `xclip -selection clipboard`, …), for terminals that refuse
//!   OSC 52.
//!
//! OSC 52 is **fire-and-forget**: the terminal answers nothing, so a write can
//! be reported as sent but never as arrived. Everything user-facing here is
//! worded accordingly, and an over-sized selection is refused with its limit
//! named rather than truncated into a silent half-copy.

use std::io::Write;
use std::process::{Command, Stdio};

/// How the text was (or would be) delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Written to the terminal as OSC 52.
    Osc52,
    /// Piped to `[ui] copy_command`.
    Command(String),
}

impl Route {
    pub fn label(&self) -> String {
        match self {
            Route::Osc52 => "osc52".to_string(),
            Route::Command(cmd) => cmd.clone(),
        }
    }
}

/// What happened. Reported to the user in one line; the text itself never
/// appears in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Sent { bytes: usize, lines: usize, route: Route },
    /// A click, or a selection that covered no characters.
    Empty,
    /// Larger than the terminal will accept; refused rather than truncated.
    TooLarge { bytes: usize, limit: usize },
    Failed { route: Route, reason: String },
}

/// The payload limit of an OSC 52 write, in bytes of the text being encoded.
///
/// Terminals cap the escape sequence itself; the common ceiling is 100 000
/// bytes of sequence, and base64 costs four bytes for every three. Staying
/// under three quarters of it leaves room for the prefix, the terminator and
/// tmux's wrapper.
pub const OSC52_LIMIT: usize = 74_000;

impl Outcome {
    /// The one line shown to the user.
    pub fn message(&self) -> String {
        match self {
            Outcome::Sent { bytes, lines, route } => {
                let unit = if *lines == 1 { "line" } else { "lines" };
                format!(
                    "copied {lines} {unit} ({bytes} bytes) to the clipboard via {}",
                    route.label()
                )
            }
            Outcome::Empty => "nothing selected".to_string(),
            Outcome::TooLarge { bytes, limit } => format!(
                "selection too large to copy: {bytes} bytes, limit {limit}. \
                 Select less, or set `[ui] copy_command` to a clipboard command"
            ),
            Outcome::Failed { route, reason } => {
                format!("copy failed via {}: {reason}", route.label())
            }
        }
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, Outcome::TooLarge { .. } | Outcome::Failed { .. })
    }
}

/// Standard base64, no line breaks. Twenty lines rather than a dependency for
/// the one place the project needs it.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The OSC 52 sequence that sets the clipboard to `text`.
///
/// Inside tmux the sequence has to be wrapped in tmux's passthrough or tmux
/// eats it and nothing reaches the terminal — the difference between the
/// feature working and appearing to do nothing at all.
pub fn osc52(text: &str, in_tmux: bool) -> String {
    let payload = base64(text.as_bytes());
    if in_tmux {
        format!("\u{1b}Ptmux;\u{1b}\u{1b}]52;c;{payload}\u{7}\u{1b}\\")
    } else {
        format!("\u{1b}]52;c;{payload}\u{7}")
    }
}

/// Whether this process is running inside tmux.
fn in_tmux() -> bool {
    std::env::var_os("TMUX").is_some()
}

/// Put `text` on the clipboard, by `copy_command` when one is configured and by
/// OSC 52 otherwise. `write` receives the escape sequence for the OSC 52 route;
/// it is the terminal in the REPL and a buffer in the tests.
pub fn copy(text: &str, copy_command: &str, write: impl FnOnce(&str) -> std::io::Result<()>) -> Outcome {
    if text.is_empty() {
        return Outcome::Empty;
    }
    let bytes = text.len();
    let lines = text.lines().count().max(1);
    let cmd = copy_command.trim();
    if !cmd.is_empty() {
        let route = Route::Command(cmd.to_string());
        return match pipe_to_command(cmd, text) {
            Ok(()) => Outcome::Sent { bytes, lines, route },
            Err(reason) => Outcome::Failed { route, reason },
        };
    }
    if bytes > OSC52_LIMIT {
        return Outcome::TooLarge {
            bytes,
            limit: OSC52_LIMIT,
        };
    }
    match write(&osc52(text, in_tmux())) {
        Ok(()) => Outcome::Sent {
            bytes,
            lines,
            route: Route::Osc52,
        },
        Err(e) => Outcome::Failed {
            route: Route::Osc52,
            reason: e.to_string(),
        },
    }
}

/// Run `cmd` through the shell with `text` on its stdin. A failure is returned
/// rather than quietly retried through OSC 52: a `copy_command` that does not
/// work is a setting the user needs to hear about.
fn pipe_to_command(cmd: &str, text: &str) -> Result<(), String> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    }
    // Dropping stdin closes the pipe, which is what lets the command exit.
    drop(child.stdin.take());
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let detail = stderr.lines().next().unwrap_or("").trim().to_string();
    Err(if detail.is_empty() {
        format!("exited with {}", out.status)
    } else {
        format!("exited with {}: {detail}", out.status)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Capture what the OSC 52 route would write.
    fn capture(text: &str, copy_command: &str) -> (Outcome, String) {
        let seen = RefCell::new(String::new());
        let outcome = copy(text, copy_command, |s| {
            seen.borrow_mut().push_str(s);
            Ok(())
        });
        (outcome, seen.into_inner())
    }

    #[test]
    fn base64_matches_the_known_vectors() {
        // RFC 4648 §10.
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_handles_multibyte_text() {
        assert_eq!(base64("あ".as_bytes()), "44GC");
    }

    #[test]
    fn osc52_has_the_expected_shape() {
        assert_eq!(osc52("foo", false), "\u{1b}]52;c;Zm9v\u{7}");
    }

    #[test]
    fn osc52_is_wrapped_for_tmux() {
        let seq = osc52("foo", true);
        assert!(seq.starts_with("\u{1b}Ptmux;\u{1b}"), "tmux passthrough: {seq:?}");
        assert!(seq.ends_with("\u{1b}\\"), "terminated for tmux: {seq:?}");
        assert!(seq.contains("]52;c;Zm9v"), "payload survives: {seq:?}");
    }

    #[test]
    fn copying_writes_the_sequence_and_reports_size_and_route() {
        let (outcome, written) = capture("hello\nworld", "");
        assert_eq!(
            outcome,
            Outcome::Sent {
                bytes: 11,
                lines: 2,
                route: Route::Osc52
            }
        );
        assert!(written.contains("]52;c;"), "the sequence was written: {written:?}");
        let msg = outcome.message();
        assert!(msg.contains("2 lines"), "{msg}");
        assert!(msg.contains("osc52"), "{msg}");
        assert!(!msg.contains("hello"), "the text must not be echoed: {msg}");
    }

    #[test]
    fn an_empty_selection_copies_nothing() {
        let (outcome, written) = capture("", "");
        assert_eq!(outcome, Outcome::Empty);
        assert!(written.is_empty(), "nothing goes to the terminal");
        assert_eq!(outcome.message(), "nothing selected");
    }

    #[test]
    fn an_oversized_selection_is_refused_not_truncated() {
        let big = "x".repeat(OSC52_LIMIT + 1);
        let (outcome, written) = capture(&big, "");
        assert_eq!(
            outcome,
            Outcome::TooLarge {
                bytes: OSC52_LIMIT + 1,
                limit: OSC52_LIMIT
            }
        );
        assert!(written.is_empty(), "a partial copy is worse than none");
        assert!(outcome.message().contains("copy_command"), "the way out is named");
        // One byte under the limit still goes.
        let ok = "x".repeat(OSC52_LIMIT);
        let (outcome, written) = capture(&ok, "");
        assert!(matches!(outcome, Outcome::Sent { .. }));
        assert!(!written.is_empty());
    }

    #[test]
    fn a_configured_command_is_used_instead_of_the_terminal() {
        let file = std::env::temp_dir().join(format!("agent-cli-clip-{}", std::process::id()));
        let _ = std::fs::remove_file(&file);
        let cmd = format!("cat > {}", file.display());
        let (outcome, written) = capture("piped text", &cmd);
        assert!(
            matches!(outcome, Outcome::Sent { route: Route::Command(_), .. }),
            "{outcome:?}"
        );
        assert!(written.is_empty(), "the terminal route is not used as well");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "piped text");
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn a_failing_command_is_reported_rather_than_falling_back() {
        let (outcome, written) = capture("text", "exit 3");
        match &outcome {
            Outcome::Failed { route, reason } => {
                assert_eq!(route, &Route::Command("exit 3".to_string()));
                assert!(reason.contains('3'), "the exit status is named: {reason}");
            }
            other => panic!("expected a failure, got {other:?}"),
        }
        assert!(
            written.is_empty(),
            "a broken copy_command must not silently fall back to the terminal"
        );
        assert!(outcome.is_failure());
    }

    #[test]
    fn a_command_larger_than_the_osc52_limit_is_still_allowed() {
        // The limit belongs to the escape sequence, not to a pipe.
        let big = "x".repeat(OSC52_LIMIT + 10);
        let (outcome, _) = capture(&big, "cat > /dev/null");
        assert!(matches!(outcome, Outcome::Sent { .. }), "{outcome:?}");
    }
}
