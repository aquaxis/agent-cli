//! Session scrollback: the transcript of what the REPL has printed, the
//! arithmetic that decides which part of it a screen shows, and the wheel
//! position.
//!
//! agent-cli prints into the terminal's own scrollback and never redraws what
//! it has written, so scrolling the log while the prompt stays put needs the
//! application to keep its own copy of its output. Every REPL message already
//! passes through the four `raw_*` writers in `app.rs`; they tee into the
//! [`Transcript`] installed here, and nothing else in the tree has to know it
//! exists.
//!
//! Everything in this module except the process-wide handle is pure: the
//! wrapping, the viewport slice and the wheel position are functions of their
//! inputs, so the whole feature is unit-tested without a terminal.

use std::collections::VecDeque;
use std::sync::Mutex;

/// Rows one wheel notch moves the view — what terminals and pagers use.
pub const STEP: usize = 3;

/// Closing sequence written at the end of a wrapped row that still has a
/// colour open, so a style can never bleed into the row (or the prompt) below.
const RESET: &str = "\u{1b}[0m";

/// The lines the REPL has printed, oldest first, capped at `limit`.
///
/// A line is stored exactly as it went to the terminal — styling included — so
/// the scrolled view looks like what was on screen. Terminators are not
/// stored: the CR+LF rewriting the raw writers do happens after recording.
pub struct Transcript {
    lines: VecDeque<String>,
    limit: usize,
    /// The last line has no terminator yet, so the next chunk continues it.
    /// Streamed answer text arrives in fragments, and those fragments are one
    /// line on screen, not one line each.
    open: bool,
}

impl Transcript {
    pub fn new(limit: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            limit,
            open: false,
        }
    }

    /// Record a chunk exactly as it was written. Embedded newlines start new
    /// lines; a chunk that does not end in a newline leaves the last line open
    /// for the next one to continue.
    pub fn record(&mut self, chunk: &str) {
        if self.limit == 0 {
            return;
        }
        let ends_with_newline = chunk.ends_with('\n');
        let segments: Vec<&str> = chunk.split('\n').collect();
        // A trailing newline yields a final empty segment that is a
        // terminator, not a blank line.
        let count = if ends_with_newline {
            segments.len() - 1
        } else {
            segments.len()
        };
        for (i, seg) in segments.iter().take(count).enumerate() {
            if i == 0 && self.open {
                if let Some(last) = self.lines.back_mut() {
                    last.push_str(seg);
                    continue;
                }
            }
            self.push(seg.to_string());
        }
        self.open = !ends_with_newline;
    }

    /// Record one complete line — a prompt echo, the indicator's outcome row —
    /// closing whatever was open before it.
    pub fn record_line(&mut self, line: &str) {
        if self.limit == 0 {
            return;
        }
        self.record(line);
        self.open = false;
    }

    fn push(&mut self, line: String) {
        self.lines.push_back(line);
        while self.lines.len() > self.limit {
            self.lines.pop_front();
        }
    }

    /// Lines currently held. The display reads the transcript through
    /// [`visible_rows`], which counts wrapped rows, so the line count is only
    /// ever asked for in tests.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn lines(&self) -> impl DoubleEndedIterator<Item = &str> {
        self.lines.iter().map(|s| s.as_str())
    }
}

/// An SGR sequence at the start of `s`: its byte length, and whether it resets.
/// Only sequences ending in `m` are colours; a cursor move or an erase is part
/// of the drawing and is left to be treated as text (the transcript never
/// contains one, since only finished messages are recorded).
fn sgr_at(s: &str) -> Option<(usize, bool)> {
    let body = s.strip_prefix("\u{1b}[")?;
    let end = body.find(|c: char| !c.is_ascii_digit() && c != ';')?;
    if body.as_bytes()[end] != b'm' {
        return None;
    }
    let params = &body[..end];
    // `\x1b[m` and `\x1b[0m` (or any all-zero parameter list) close a style.
    let is_reset = params.is_empty() || params.split(';').all(|p| p.chars().all(|c| c == '0'));
    Some(("\u{1b}[".len() + end + 1, is_reset))
}

/// Wrap a possibly styled line to `cols` printable columns.
///
/// SGR sequences cost no column and are never split: the style open at a break
/// is closed at the end of the row and re-opened at the start of the next, so
/// every row is self-closing and a colour cannot bleed into the prompt block.
/// Printable characters are measured exactly as [`crate::editor::wrap_display`]
/// measures them, so a full-width character never straddles two rows.
pub fn wrap_styled(line: &str, cols: usize) -> Vec<String> {
    if cols == 0 {
        return Vec::new();
    }
    let mut rows: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut width = 0usize;
    // The style sequence currently in effect, re-emitted after every break.
    let mut style = String::new();
    let mut i = 0;
    while i < line.len() {
        if let Some((len, is_reset)) = sgr_at(&line[i..]) {
            let seq = &line[i..i + len];
            if is_reset {
                style.clear();
            } else {
                style.push_str(seq);
            }
            current.push_str(seq);
            i += len;
            continue;
        }
        let raw = line[i..].chars().next().unwrap_or(' ');
        i += raw.len_utf8();
        let c = if raw == '\t' || raw.is_control() {
            ' '
        } else {
            raw
        };
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
        if width + w > cols {
            rows.push(close_row(std::mem::take(&mut current), &style));
            current.push_str(&style);
            width = 0;
        }
        current.push(c);
        width += w;
    }
    rows.push(close_row(current, &style));
    rows
}

fn close_row(mut row: String, style: &str) -> String {
    if !style.is_empty() {
        row.push_str(RESET);
    }
    row
}

/// Total wrapped rows the transcript occupies at this width.
pub fn total_rows(t: &Transcript, cols: usize) -> usize {
    t.lines().map(|l| wrap_styled(l, cols).len()).sum()
}

/// The furthest the view can travel from the bottom: everything that does not
/// fit on a screen of `rows` rows.
pub fn max_offset(t: &Transcript, cols: usize, rows: usize) -> usize {
    total_rows(t, cols).saturating_sub(rows)
}

/// The last `want` wrapped rows of the transcript, in order. Walks backwards
/// and stops as soon as it has enough, so a screenful costs a screenful of
/// wrapping however long the session has been.
fn tail_rows(t: &Transcript, cols: usize, want: usize) -> Vec<String> {
    if want == 0 || cols == 0 {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::with_capacity(want);
    for line in t.lines().rev() {
        let mut rows = wrap_styled(line, cols);
        while let Some(row) = rows.pop() {
            out.push(row);
            if out.len() >= want {
                out.reverse();
                return out;
            }
        }
    }
    out.reverse();
    out
}

/// The transcript rows a screen of `rows` rows shows at `offset` — the slice
/// ending `offset` wrapped rows above the end of the transcript. Fewer rows are
/// returned when the transcript is shorter than the screen.
pub fn visible_rows(t: &Transcript, cols: usize, rows: usize, offset: usize) -> Vec<String> {
    if rows == 0 {
        return Vec::new();
    }
    let mut tail = tail_rows(t, cols, rows + offset);
    // Drop the `offset` rows below the view, then keep the last `rows` of what
    // is left.
    let keep = tail.len().saturating_sub(offset);
    tail.truncate(keep);
    if tail.len() > rows {
        tail.drain(..tail.len() - rows);
    }
    tail
}

/// Transcript rows a screen of `height` rows has left once a prompt block of
/// `prompt_rows` rows is pinned to its bottom. Saturates on a terminal too
/// short to hold both, where the prompt alone is drawn.
pub fn body_rows(height: usize, prompt_rows: u16) -> usize {
    height.saturating_sub(prompt_rows as usize)
}

/// Where the view sits, in wrapped rows above the bottom of the transcript.
/// `0` is the live view — the bottom — and is the only position in which no
/// overlay is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScrollView {
    offset: usize,
}

impl ScrollView {
    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn is_active(&self) -> bool {
        self.offset > 0
    }

    /// Move by `delta` rows — positive towards the start of the transcript —
    /// clamped to `0..=max_offset`. Returns whether the position changed, so a
    /// burst of wheel reports costs one repaint rather than one each.
    pub fn scroll(&mut self, delta: isize, max_offset: usize) -> bool {
        let target = if delta >= 0 {
            self.offset.saturating_add(delta as usize)
        } else {
            self.offset.saturating_sub(delta.unsigned_abs())
        }
        .min(max_offset);
        let changed = target != self.offset;
        self.offset = target;
        changed
    }

    /// Back to the live view.
    pub fn reset(&mut self) {
        self.offset = 0;
    }
}

/// The transcript for this process, installed by the interactive REPL when the
/// scrollback is enabled. The `raw_*` writers are free functions called from
/// the display task, the input loop and the command handlers, so the buffer is
/// reached through a handle rather than threaded through every caller. The
/// piped loop and `run_headless` never install one and pay nothing for it.
static TRANSCRIPT: Mutex<Option<Transcript>> = Mutex::new(None);

fn with_lock<R>(f: impl FnOnce(&mut Option<Transcript>) -> R) -> R {
    let mut guard = TRANSCRIPT.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

/// Start recording, keeping at most `limit` lines. A `limit` of 0 records
/// nothing, which is how `[ui] scrollback_lines = 0` disables the feature.
pub fn install(limit: usize) {
    with_lock(|slot| *slot = Some(Transcript::new(limit)));
}

/// Stop recording and drop the buffer. The REPL installs the transcript for
/// the life of the process, so this exists for the tests that share the handle.
#[cfg(test)]
pub fn uninstall() {
    with_lock(|slot| *slot = None);
}

/// Record a chunk if a transcript is installed; a no-op otherwise.
pub fn record(chunk: &str) {
    with_lock(|slot| {
        if let Some(t) = slot.as_mut() {
            t.record(chunk);
        }
    });
}

/// Record one complete line if a transcript is installed.
pub fn record_line(line: &str) {
    with_lock(|slot| {
        if let Some(t) = slot.as_mut() {
            t.record_line(line);
        }
    });
}

/// Read the transcript, if there is one.
pub fn with<R>(f: impl FnOnce(&Transcript) -> R) -> Option<R> {
    with_lock(|slot| slot.as_ref().map(f))
}

/// The process-wide transcript is shared state, and the tests that install one
/// run in the same process as every other test. They take this lock so they
/// cannot interleave with each other.
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::str_display_width;
    use crate::theme::{paint, strip_sgr, Role};

    fn transcript(lines: &[&str]) -> Transcript {
        let mut t = Transcript::new(100);
        for l in lines {
            t.record_line(l);
        }
        t
    }

    #[test]
    fn transcript_records_lines_in_write_order() {
        let t = transcript(&["first", "second", "third"]);
        assert_eq!(
            t.lines().collect::<Vec<_>>(),
            vec!["first", "second", "third"]
        );
    }

    #[test]
    fn a_chunk_without_a_newline_leaves_the_line_open_for_the_next_one() {
        let mut t = Transcript::new(100);
        // How streamed answer text arrives: fragments, then a terminator.
        t.record("the ");
        t.record("answer");
        t.record(" text\n");
        t.record("next line\n");
        assert_eq!(
            t.lines().collect::<Vec<_>>(),
            vec!["the answer text", "next line"]
        );
    }

    #[test]
    fn a_chunk_splits_on_embedded_newlines_without_a_trailing_blank() {
        let mut t = Transcript::new(100);
        t.record("one\ntwo\n");
        assert_eq!(t.lines().collect::<Vec<_>>(), vec!["one", "two"]);
        // A lone newline closes the open line and prints a blank row.
        let mut t2 = Transcript::new(100);
        t2.record("open");
        t2.record("\n");
        t2.record("\n");
        assert_eq!(t2.lines().collect::<Vec<_>>(), vec!["open", ""]);
    }

    #[test]
    fn record_line_closes_the_open_line() {
        let mut t = Transcript::new(100);
        t.record("partial");
        t.record_line(" echoed");
        t.record_line("after");
        assert_eq!(
            t.lines().collect::<Vec<_>>(),
            vec!["partial echoed", "after"]
        );
    }

    #[test]
    fn the_ring_evicts_the_oldest_lines_at_the_cap() {
        let mut t = Transcript::new(3);
        for i in 0..6 {
            t.record_line(&format!("line {i}"));
        }
        assert_eq!(t.len(), 3);
        assert_eq!(
            t.lines().collect::<Vec<_>>(),
            vec!["line 3", "line 4", "line 5"]
        );
    }

    #[test]
    fn a_zero_cap_records_nothing() {
        let mut t = Transcript::new(0);
        t.record_line("dropped");
        t.record("also dropped");
        assert!(t.is_empty());
        assert_eq!(total_rows(&t, 80), 0);
    }

    #[test]
    fn wrap_styled_matches_the_plain_wrap_for_plain_text() {
        for (text, cols) in [("hello world", 5), ("abcdef", 3), ("", 10), ("ab", 10)] {
            assert_eq!(
                wrap_styled(text, cols),
                crate::editor::wrap_display(text, cols),
                "text {text:?} at {cols} cols"
            );
        }
    }

    #[test]
    fn wrap_styled_never_splits_a_wide_character() {
        let rows = wrap_styled("日本語テキスト", 5);
        for row in &rows {
            assert!(str_display_width(row) <= 5);
        }
        assert_eq!(rows.concat(), "日本語テキスト");
    }

    #[test]
    fn wrap_styled_reopens_the_style_on_every_row_and_closes_it() {
        let line = paint(Role::ToolName, "abcdefgh");
        let rows = wrap_styled(&line, 3);
        assert_eq!(rows.len(), 3);
        for row in &rows {
            assert!(row.starts_with('\u{1b}'), "row must re-open the style");
            assert!(row.ends_with(RESET), "row must close its own style");
            // The escape sequences cost no column.
            assert!(str_display_width(&strip_sgr(row)) <= 3);
        }
        assert_eq!(
            rows.iter().map(|r| strip_sgr(r)).collect::<Vec<_>>(),
            vec!["abc", "def", "gh"]
        );
    }

    #[test]
    fn wrap_styled_leaves_a_closed_line_alone() {
        // A line whose style is already closed needs no extra reset.
        let line = format!("{}{}", paint(Role::Info, "ok"), " tail");
        let rows = wrap_styled(&line, 40);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].ends_with(&format!("{RESET}{RESET}")));
        assert_eq!(strip_sgr(&rows[0]), "ok tail");
    }

    #[test]
    fn visible_rows_returns_the_slice_ending_offset_rows_above_the_end() {
        let t = transcript(&["a", "b", "c", "d", "e"]);
        assert_eq!(visible_rows(&t, 80, 3, 0), vec!["c", "d", "e"]);
        assert_eq!(visible_rows(&t, 80, 3, 1), vec!["b", "c", "d"]);
        assert_eq!(visible_rows(&t, 80, 3, 2), vec!["a", "b", "c"]);
        // Past the start there is simply less to show.
        assert_eq!(visible_rows(&t, 80, 3, 4), vec!["a"]);
        assert!(visible_rows(&t, 80, 0, 0).is_empty());
    }

    #[test]
    fn visible_rows_counts_wrapped_rows_not_lines() {
        let t = transcript(&["abcdef", "gh"]);
        // At 3 columns the first line is two rows: "abc", "def".
        assert_eq!(total_rows(&t, 3), 3);
        assert_eq!(visible_rows(&t, 3, 2, 0), vec!["def", "gh"]);
        assert_eq!(visible_rows(&t, 3, 2, 1), vec!["abc", "def"]);
    }

    #[test]
    fn max_offset_is_what_does_not_fit_on_the_screen() {
        let t = transcript(&["a", "b", "c", "d", "e"]);
        assert_eq!(max_offset(&t, 80, 3), 2);
        assert_eq!(max_offset(&t, 80, 5), 0);
        assert_eq!(max_offset(&t, 80, 9), 0);
        // Narrower terminal, more wrapped rows, more to scroll through.
        let wide = transcript(&["abcdef", "ghijkl"]);
        assert_eq!(max_offset(&wide, 3, 2), 2);
    }

    #[test]
    fn body_rows_leaves_room_for_the_prompt_and_saturates_when_short() {
        assert_eq!(body_rows(24, 1), 23);
        assert_eq!(body_rows(24, 3), 21);
        assert_eq!(body_rows(2, 3), 0);
        assert_eq!(body_rows(0, 1), 0);
    }

    #[test]
    fn the_wheel_activates_steps_and_clamps_at_the_top() {
        let mut v = ScrollView::default();
        assert!(!v.is_active());
        assert!(v.scroll(STEP as isize, 10));
        assert_eq!(v.offset(), 3);
        assert!(v.is_active());
        assert!(v.scroll(STEP as isize, 10));
        assert!(v.scroll(STEP as isize, 10));
        assert!(v.scroll(STEP as isize, 10));
        assert_eq!(v.offset(), 10, "clamped to max_offset");
        assert!(
            !v.scroll(STEP as isize, 10),
            "no move means no repaint is needed"
        );
    }

    #[test]
    fn scrolling_back_to_the_bottom_leaves_the_scrolled_view() {
        let mut v = ScrollView::default();
        v.scroll(6, 10);
        assert!(v.scroll(-(STEP as isize), 10));
        assert_eq!(v.offset(), 3);
        assert!(v.scroll(-(STEP as isize), 10));
        assert_eq!(v.offset(), 0);
        assert!(!v.is_active(), "offset 0 is the live view");
        assert!(!v.scroll(-(STEP as isize), 10), "clamped at the bottom");
        v.scroll(3, 10);
        v.reset();
        assert!(!v.is_active());
    }

    #[test]
    fn nothing_to_scroll_means_the_wheel_does_nothing() {
        let t = transcript(&["only", "two"]);
        let mut v = ScrollView::default();
        assert!(!v.scroll(STEP as isize, max_offset(&t, 80, 24)));
        assert!(!v.is_active());
    }

    #[test]
    fn a_width_change_reflows_the_view() {
        let t = transcript(&["aaaaaaaaaa"]);
        assert_eq!(visible_rows(&t, 10, 5, 0), vec!["aaaaaaaaaa"]);
        assert_eq!(
            visible_rows(&t, 4, 5, 0),
            vec!["aaaa", "aaaa", "aa"],
            "the same line reflows when the terminal narrows"
        );
    }

    #[test]
    fn the_process_handle_records_only_while_installed() {
        let _guard = test_lock();
        uninstall();
        record_line("dropped on the floor");
        assert!(with(|t| t.len()).is_none());

        install(10);
        record_line("kept");
        record("partial");
        record(" line\n");
        let lines = with(|t| t.lines().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        assert_eq!(lines, vec!["kept", "partial line"]);
        uninstall();
    }

    #[test]
    fn installing_with_a_zero_cap_keeps_nothing() {
        let _guard = test_lock();
        install(0);
        record_line("dropped");
        assert_eq!(with(|t| t.len()), Some(0));
        uninstall();
    }
}
