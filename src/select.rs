//! Selecting a range of the session log with the mouse, and the text that
//! comes off it.
//!
//! The REPL keeps mouse reporting on for the whole session so the wheel can
//! scroll the log (see [`crate::scroll`]), which means the terminal's own
//! drag-to-select never reaches the terminal. This module is the replacement:
//! a drag is two points, and everything else — which columns of which rows the
//! range covers, how those rows are drawn while the drag is in progress, and
//! what ends up on the clipboard — is arithmetic over the rows the scrollback
//! already knows how to produce.
//!
//! Two properties matter and are the reason it all lives here rather than in
//! `app.rs`:
//!
//! * **Pure.** Nothing in this module touches a terminal, a clipboard or the
//!   process-wide transcript. Every function is a function of its arguments,
//!   so the whole feature is unit-tested without a screen.
//! * **The transcript is the truth.** The copied text comes from the rows the
//!   transcript produced, not from the screen: the wrapping is undone, so a
//!   line the terminal broke over three rows is pasted as the one line it was,
//!   and the styling is stripped, so no escape sequence ever reaches the
//!   clipboard.

use std::ops::Range;

use crate::scroll::{sgr_at, Row};
use crate::theme::{paint, strip_sgr, Role};

/// A cell of the drawn body: a row of the viewport (0 at the top) and a
/// printable column within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub row: usize,
    pub col: usize,
}

impl Point {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

/// A drag: where the button went down, where the pointer is now, and the
/// scroll offset the drag belongs to.
///
/// The offset is carried so a repaint can tell whether the selection still
/// describes what is on screen — a view that has moved is a different slice of
/// the transcript, and the selection is dropped rather than redrawn over rows
/// it was not made on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Point,
    pub cursor: Point,
    pub offset: usize,
}

impl Selection {
    /// Begin a drag at `anchor`, on the view currently at `offset`.
    pub fn begin(anchor: Point, offset: usize) -> Self {
        Self {
            anchor,
            cursor: anchor,
            offset,
        }
    }

    /// Move the loose end. The anchor never moves, so dragging back past the
    /// start reverses the range rather than shrinking it to nothing.
    pub fn extend(&mut self, cursor: Point) {
        self.cursor = cursor;
    }

    /// The range in reading order, whichever way it was dragged.
    pub fn normalised(&self) -> (Point, Point) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }

    /// A drag that never left its starting cell — a plain click. It selects
    /// nothing, so it copies nothing.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }
}

/// The half-open column range of `row` the selection covers, clamped to
/// `row_width` printable columns — or `None` when the row is outside the
/// selection or contributes nothing.
///
/// The first row starts at the anchor, the last ends at the cursor, and the
/// rows between are whole. A selection that ends at column 0 of a row does not
/// include that row: the pointer is *before* its first character.
pub fn row_span(sel: &Selection, row: usize, row_width: usize) -> Option<Range<usize>> {
    if sel.is_empty() {
        return None;
    }
    let (start, end) = sel.normalised();
    if row < start.row || row > end.row {
        return None;
    }
    let from = if row == start.row { start.col } else { 0 };
    let to = if row == end.row { end.col } else { row_width };
    let from = from.min(row_width);
    let to = to.min(row_width);
    if from >= to {
        return None;
    }
    Some(from..to)
}

/// Re-emit `styled_row` with `span` drawn as selected.
///
/// The row is walked exactly as [`crate::scroll::wrap_styled`] walks it, so the
/// colours it already carries survive: the style in effect is re-opened after
/// the highlight closes, and a full-width character is highlighted whole rather
/// than split down the middle.
pub fn highlight(styled_row: &str, span: Range<usize>) -> String {
    if span.is_empty() {
        return styled_row.to_string();
    }
    let mut out = String::with_capacity(styled_row.len() + 16);
    // The style sequence currently in effect, re-emitted after the highlight
    // closes (the highlight's own reset would otherwise cancel it).
    let mut style = String::new();
    let mut inside = false;
    let mut col = 0usize;
    let mut buf = String::new();
    let mut i = 0;
    while i < styled_row.len() {
        if let Some((len, is_reset)) = sgr_at(&styled_row[i..]) {
            let seq = &styled_row[i..i + len];
            if is_reset {
                style.clear();
            } else {
                style.push_str(seq);
            }
            // A style change inside the highlight is kept, but the highlight
            // has to be re-opened after it so it is not cancelled by a reset.
            if inside {
                buf.push_str(seq);
            } else {
                out.push_str(seq);
            }
            i += len;
            continue;
        }
        let c = styled_row[i..].chars().next().unwrap_or(' ');
        i += c.len_utf8();
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
        let selected = col >= span.start && col < span.end;
        if selected && !inside {
            inside = true;
        } else if !selected && inside {
            out.push_str(&paint(Role::Selection, &buf));
            out.push_str(&style);
            buf.clear();
            inside = false;
        }
        if inside {
            buf.push(c);
        } else {
            out.push(c);
        }
        col += w;
    }
    if inside {
        out.push_str(&paint(Role::Selection, &buf));
        out.push_str(&style);
    }
    out
}

/// The plain text of a selection over `rows`, ready for the clipboard.
///
/// SGR sequences are dropped, and the wrapping is undone: consecutive rows that
/// came from one transcript line are joined with nothing between them, so a
/// line the terminal broke is copied as the line it was. Rows that came from
/// different lines are joined with `\n`.
pub fn selected_text(rows: &[Row], sel: &Selection) -> String {
    let mut out = String::new();
    let mut started = false;
    for (i, row) in rows.iter().enumerate() {
        let plain = strip_sgr(&row.text);
        let width = crate::editor::str_display_width(&plain);
        let Some(span) = row_span(sel, i, width) else {
            continue;
        };
        if started {
            // A wrapped row continues the line above it; anything else is a
            // new line.
            if !row.continues {
                out.push('\n');
            }
        }
        out.push_str(&slice_columns(&plain, span));
        started = true;
    }
    out
}

/// The characters of `plain` occupying the printable columns in `span`. A
/// full-width character is taken whole when the span starts or ends inside it.
fn slice_columns(plain: &str, span: Range<usize>) -> String {
    let mut out = String::new();
    let mut col = 0usize;
    for c in plain.chars() {
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
        let end = col + w;
        // Overlap, so a wide character straddling a boundary is included.
        if end > span.start && col < span.end {
            out.push(c);
        }
        col = end;
        if col >= span.end {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(text: &str) -> Row {
        Row {
            text: text.to_string(),
            continues: false,
        }
    }

    fn wrapped(text: &str) -> Row {
        Row {
            text: text.to_string(),
            continues: true,
        }
    }

    fn drag(from: (usize, usize), to: (usize, usize)) -> Selection {
        let mut s = Selection::begin(Point::new(from.0, from.1), 0);
        s.extend(Point::new(to.0, to.1));
        s
    }

    #[test]
    fn a_range_normalises_whichever_way_it_was_dragged() {
        let forward = drag((1, 2), (3, 4));
        let backward = drag((3, 4), (1, 2));
        assert_eq!(forward.normalised(), backward.normalised());
        assert_eq!(
            forward.normalised(),
            (Point::new(1, 2), Point::new(3, 4)),
            "reading order, top-left first"
        );
        // Same row, dragged right to left.
        let leftward = drag((2, 9), (2, 3));
        assert_eq!(
            leftward.normalised(),
            (Point::new(2, 3), Point::new(2, 9))
        );
        // Up and to the right still orders by row first.
        let upright = drag((5, 1), (2, 40));
        assert_eq!(upright.normalised(), (Point::new(2, 40), Point::new(5, 1)));
    }

    #[test]
    fn row_spans_cover_first_middle_and_last_rows() {
        let sel = drag((1, 3), (3, 5));
        assert_eq!(row_span(&sel, 0, 80), None, "above the selection");
        assert_eq!(row_span(&sel, 1, 80), Some(3..80), "first row from the anchor");
        assert_eq!(row_span(&sel, 2, 80), Some(0..80), "middle rows are whole");
        assert_eq!(row_span(&sel, 3, 80), Some(0..5), "last row up to the cursor");
        assert_eq!(row_span(&sel, 4, 80), None, "below the selection");
    }

    #[test]
    fn a_span_is_clamped_to_the_rows_own_width() {
        let sel = drag((0, 2), (1, 70));
        // A short row contributes only the columns it has, not the 70 the
        // pointer reached.
        assert_eq!(row_span(&sel, 0, 10), Some(2..10));
        assert_eq!(row_span(&sel, 1, 12), Some(0..12));
        // An anchor past the end of its row contributes nothing.
        let past = drag((0, 40), (0, 60));
        assert_eq!(row_span(&past, 0, 10), None);
    }

    #[test]
    fn a_click_selects_nothing() {
        let click = drag((2, 5), (2, 5));
        assert!(click.is_empty());
        assert_eq!(row_span(&click, 2, 80), None);
        assert_eq!(selected_text(&[row("hello")], &click), "");
    }

    #[test]
    fn one_row_selection_takes_just_that_span() {
        let sel = drag((0, 6), (0, 11));
        assert_eq!(selected_text(&[row("hello world!")], &sel), "world");
    }

    #[test]
    fn selected_text_strips_styling() {
        let styled = paint(Role::Info, "coloured");
        let sel = drag((0, 0), (0, 8));
        let text = selected_text(&[row(&styled)], &sel);
        assert_eq!(text, "coloured");
        assert!(!text.contains('\u{1b}'), "no escape may reach the clipboard");
    }

    #[test]
    fn a_wrapped_line_copies_as_one_line() {
        // What `wrap_styled` produces for one long line at a narrow width.
        let rows = vec![row("the quick "), wrapped("brown fox")];
        let sel = drag((0, 0), (1, 9));
        assert_eq!(selected_text(&rows, &sel), "the quick brown fox");
    }

    #[test]
    fn separate_lines_are_joined_with_newlines() {
        let rows = vec![row("first"), row("second"), row("third")];
        let sel = drag((0, 0), (2, 5));
        assert_eq!(selected_text(&rows, &sel), "first\nsecond\nthird");
    }

    #[test]
    fn a_partial_first_row_keeps_the_rest_whole() {
        let rows = vec![row("[answer] hello"), row("second line")];
        let sel = drag((0, 9), (1, 6));
        assert_eq!(selected_text(&rows, &sel), "hello\nsecond");
    }

    #[test]
    fn highlight_marks_the_span_and_leaves_the_row_plain_either_side() {
        let out = highlight("abcdef", 2..4);
        assert_eq!(out, format!("ab{}ef", paint(Role::Selection, "cd")));
        assert_eq!(strip_sgr(&out), "abcdef", "the text itself is untouched");
    }

    #[test]
    fn highlight_preserves_the_rows_own_colours() {
        let row = format!("plain {}", paint(Role::Info, "coloured"));
        let out = highlight(&row, 0..5);
        assert_eq!(
            strip_sgr(&out),
            "plain coloured",
            "highlighting must not eat characters"
        );
        assert!(
            out.contains(&paint(Role::Selection, "plain")),
            "the span is drawn as selected: {out}"
        );
        // The row's own colour sequence is still there, after the highlight.
        assert!(out.contains("\u{1b}[36m") || out.contains("coloured"));
    }

    #[test]
    fn highlight_does_not_split_a_full_width_character() {
        // `あ` is two columns wide; a span ending mid-character takes it whole.
        let out = highlight("あい", 0..1);
        assert_eq!(strip_sgr(&out), "あい");
        assert!(out.contains(&paint(Role::Selection, "あ")));
    }

    #[test]
    fn full_width_characters_are_selected_whole() {
        let sel = drag((0, 0), (0, 3));
        assert_eq!(selected_text(&[row("あいう")], &sel), "あい");
    }

    #[test]
    fn an_empty_span_leaves_the_row_exactly_as_it_was() {
        let styled = paint(Role::Info, "unchanged");
        assert_eq!(highlight(&styled, 0..0), styled);
    }
}
