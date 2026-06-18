//! Line editor with history navigation for the REPL prompt.
//!
//! Pure data model for in-terminal single-line editing with up/down arrow-key
//! history browsing. The terminal I/O (crossterm raw-mode, key events, line
//! redrawing) lives in `app.rs`; this module only manages the edit buffer and
//! history cursor.

/// Display width (number of terminal columns) of an arbitrary string. Full-width
/// CJK characters count as 2 columns; zero-width / combining marks as 0. Unknown
/// characters default to 1. This is the single source of column math shared by
/// the editor and the raw-mode renderer.
pub fn str_display_width(s: &str) -> usize {
    s.chars()
        .map(|c| unicode_width::UnicodeWidthChar::width(c).unwrap_or(1))
        .sum()
}

/// State for the in-terminal line editor.
///
/// `history_index` is `None` when the user is editing a new line (not browsing
/// history). When navigating with ↑/↓, it points into the history vector.
/// `saved_draft` preserves the unfinished text the user was typing before they
/// pressed ↑, and is restored when they press ↓ past the newest entry.
#[derive(Debug)]
pub struct InputState {
    /// Current line content being edited.
    pub line: String,
    /// Cursor position (byte offset) within `line`.
    pub cursor: usize,
    /// Index into the history vector when browsing; `None` when editing a new line.
    pub history_index: Option<usize>,
    /// The line content saved before the first ↑ press; restored on ↓-past-newest.
    pub saved_draft: Option<String>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            line: String::new(),
            cursor: 0,
            history_index: None,
            saved_draft: None,
        }
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        self.line.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Delete the character before the cursor (Backspace).
    /// Returns `true` if a character was deleted.
    pub fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        // Find the previous character boundary.
        let prev = self.line[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.line.drain(prev..self.cursor);
        self.cursor = prev;
        true
    }

    /// Delete the character at the cursor (Delete key).
    /// Returns `true` if a character was deleted.
    pub fn delete(&mut self) -> bool {
        if self.cursor >= self.line.len() {
            return false;
        }
        let next = self.line[self.cursor..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| self.cursor + i)
            .unwrap_or(self.line.len());
        self.line.drain(self.cursor..next);
        true
    }

    /// Move cursor to the beginning of the line (Home / Ctrl+A).
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to the end of the line (End / Ctrl+E).
    pub fn move_end(&mut self) {
        self.cursor = self.line.len();
    }

    /// Move cursor one character left. Returns `true` if the cursor moved.
    pub fn move_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let prev = self.line[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.cursor = prev;
        true
    }

    /// Move cursor one character right. Returns `true` if the cursor moved.
    pub fn move_right(&mut self) -> bool {
        if self.cursor >= self.line.len() {
            return false;
        }
        let next = self.line[self.cursor..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| self.cursor + i)
            .unwrap_or(self.line.len());
        self.cursor = next;
        true
    }

    /// Compute the display width (number of terminal columns) of the text
    /// before the cursor position. This accounts for full-width CJK characters
    /// and other Unicode characters that occupy 2 columns on a terminal.
    pub fn display_cursor(&self) -> usize {
        str_display_width(&self.line[..self.cursor])
    }

    /// Compute the total display width (number of terminal columns) of the
    /// entire line content.
    pub fn display_width(&self) -> usize {
        str_display_width(&self.line)
    }

    /// Navigate up (older) in the history. Saves the current line as a draft
    /// if this is the first ↑ press. Replaces the line with the history entry.
    pub fn navigate_up(&mut self, history: &[String]) {
        if history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                // First ↑ press: save current line as draft, go to newest entry.
                self.saved_draft = Some(std::mem::take(&mut self.line));
                self.history_index = Some(history.len() - 1);
            }
            Some(i) => {
                if i == 0 {
                    // Already at oldest; stay.
                    return;
                }
                self.history_index = Some(i - 1);
            }
        }
        self.line = history[self.history_index.unwrap()].clone();
        self.cursor = self.line.len();
    }

    /// Navigate down (newer) in the history. If past the newest entry,
    /// restore the saved draft.
    pub fn navigate_down(&mut self, history: &[String]) {
        match self.history_index {
            None => {
                // Already at bottom (new input); nothing to do.
                return;
            }
            Some(i) => {
                if i + 1 < history.len() {
                    self.history_index = Some(i + 1);
                    self.line = history[i + 1].clone();
                } else {
                    // Past newest: restore draft.
                    self.history_index = None;
                    self.line = self.saved_draft.take().unwrap_or_default();
                }
            }
        }
        self.cursor = self.line.len();
    }

    /// Exit history browsing mode, restoring the saved draft.
    /// If not browsing, this is a no-op.
    pub fn exit_history(&mut self) {
        if self.history_index.is_some() {
            self.history_index = None;
            self.line = self.saved_draft.take().unwrap_or_default();
            self.cursor = self.line.len();
        }
    }

    /// Clear the current line and reset history browsing state.
    pub fn clear_line(&mut self) {
        self.line.clear();
        self.cursor = 0;
        self.history_index = None;
        self.saved_draft = None;
    }

    /// Submit the current line: return the line content, clear the editor,
    /// and reset history browsing state.
    pub fn submit(&mut self) -> String {
        let line = std::mem::take(&mut self.line);
        self.cursor = 0;
        self.history_index = None;
        self.saved_draft = None;
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_char_appends_at_cursor() {
        let mut s = InputState::new();
        s.insert_char('a');
        s.insert_char('b');
        assert_eq!(s.line, "ab");
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn insert_char_midline() {
        let mut s = InputState::new();
        s.insert_char('a');
        s.insert_char('c');
        s.cursor = 1;
        s.insert_char('b');
        assert_eq!(s.line, "abc");
        assert_eq!(s.cursor, 2);
    }

    #[test]
    fn backspace_deletes_char_before_cursor() {
        let mut s = InputState::new();
        s.insert_char('a');
        s.insert_char('b');
        assert!(s.backspace());
        assert_eq!(s.line, "a");
        assert_eq!(s.cursor, 1);
    }

    #[test]
    fn backspace_at_beginning_is_noop() {
        let mut s = InputState::new();
        assert!(!s.backspace());
    }

    #[test]
    fn delete_removes_char_at_cursor() {
        let mut s = InputState::new();
        s.insert_char('a');
        s.insert_char('b');
        s.cursor = 0;
        assert!(s.delete());
        assert_eq!(s.line, "b");
        assert_eq!(s.cursor, 0);
    }

    #[test]
    fn delete_at_end_is_noop() {
        let mut s = InputState::new();
        s.insert_char('a');
        assert!(!s.delete());
    }

    #[test]
    fn move_left_right_home_end() {
        let mut s = InputState::new();
        s.insert_char('a');
        s.insert_char('b');
        s.insert_char('c');
        assert_eq!(s.cursor, 3);
        s.move_left();
        assert_eq!(s.cursor, 2);
        s.move_right();
        assert_eq!(s.cursor, 3);
        s.move_home();
        assert_eq!(s.cursor, 0);
        s.move_end();
        assert_eq!(s.cursor, 3);
    }

    #[test]
    fn navigate_up_saves_draft_and_selects_newest() {
        let mut s = InputState::new();
        let history = vec!["old1".into(), "old2".into(), "recent".into()];
        s.insert_char('d');
        s.insert_char('r');
        s.insert_char('a');
        s.insert_char('f');
        s.insert_char('t');
        s.navigate_up(&history);
        assert_eq!(s.line, "recent");
        assert_eq!(s.history_index, Some(2));
        assert_eq!(s.saved_draft, Some("draft".to_string()));
        assert_eq!(s.cursor, 6); // cursor at end
    }

    #[test]
    fn navigate_up_then_down_restores_draft() {
        let mut s = InputState::new();
        let history = vec!["old1".into(), "old2".into()];
        s.insert_char('x');
        s.navigate_up(&history);
        assert_eq!(s.line, "old2");
        s.navigate_down(&history);
        assert_eq!(s.line, "x");
        assert_eq!(s.history_index, None);
        assert_eq!(s.saved_draft, None);
    }

    #[test]
    fn navigate_up_multiple_times() {
        let mut s = InputState::new();
        let history = vec!["first".into(), "second".into(), "third".into()];
        s.navigate_up(&history);
        assert_eq!(s.line, "third");
        assert_eq!(s.history_index, Some(2));
        s.navigate_up(&history);
        assert_eq!(s.line, "second");
        assert_eq!(s.history_index, Some(1));
        s.navigate_up(&history);
        assert_eq!(s.line, "first");
        assert_eq!(s.history_index, Some(0));
        // Already at oldest, stays.
        s.navigate_up(&history);
        assert_eq!(s.line, "first");
        assert_eq!(s.history_index, Some(0));
    }

    #[test]
    fn navigate_up_on_empty_history_is_noop() {
        let mut s = InputState::new();
        let history: Vec<String> = vec![];
        s.insert_char('a');
        s.navigate_up(&history);
        assert_eq!(s.line, "a");
        assert_eq!(s.history_index, None);
    }

    #[test]
    fn navigate_down_at_bottom_is_noop() {
        let mut s = InputState::new();
        let history = vec!["old".into()];
        s.navigate_down(&history);
        assert_eq!(s.line, "");
        assert_eq!(s.history_index, None);
    }

    #[test]
    fn submit_clears_state_and_returns_line() {
        let mut s = InputState::new();
        s.insert_char('h');
        s.insert_char('i');
        s.saved_draft = Some("draft".into());
        s.history_index = Some(0);
        let line = s.submit();
        assert_eq!(line, "hi");
        assert_eq!(s.line, "");
        assert_eq!(s.cursor, 0);
        assert_eq!(s.history_index, None);
        assert_eq!(s.saved_draft, None);
    }

    #[test]
    fn clear_line_resets_everything() {
        let mut s = InputState::new();
        s.insert_char('x');
        s.navigate_up(&["old".into()]);
        s.clear_line();
        assert_eq!(s.line, "");
        assert_eq!(s.cursor, 0);
        assert_eq!(s.history_index, None);
        assert_eq!(s.saved_draft, None);
    }

    #[test]
    fn up_then_submit_then_up_starts_from_newest() {
        let mut s = InputState::new();
        let history = vec!["a".into(), "b".into()];
        s.navigate_up(&history);
        assert_eq!(s.line, "b");
        let submitted = s.submit();
        assert_eq!(submitted, "b");
        // After submit, history browsing resets; pressing ↑ again goes to newest.
        s.navigate_up(&history);
        assert_eq!(s.line, "b");
        assert_eq!(s.history_index, Some(1));
    }

    #[test]
    fn up_twice_then_down_once() {
        let mut s = InputState::new();
        let history = vec!["first".into(), "second".into(), "third".into()];
        s.navigate_up(&history); // -> third (index 2)
        s.navigate_up(&history); // -> second (index 1)
        assert_eq!(s.line, "second");
        s.navigate_down(&history); // -> third (index 2)
        assert_eq!(s.line, "third");
    }

    #[test]
    fn exit_history_restores_draft() {
        let mut s = InputState::new();
        let history = vec!["old".into()];
        s.insert_char('x');
        s.navigate_up(&history);
        assert_eq!(s.line, "old");
        s.exit_history();
        assert_eq!(s.line, "x");
        assert!(s.history_index.is_none());
    }

    #[test]
    fn exit_history_when_not_browsing_is_noop() {
        let mut s = InputState::new();
        s.insert_char('x');
        s.exit_history();
        assert_eq!(s.line, "x");
        assert!(s.history_index.is_none());
    }

    // Task 4: Tests for consecutive-duplicate dedup during navigation
    // These tests verify that navigate_up/navigate_down work correctly with
    // deduplicated history snapshots (as constructed by app.rs's dedup_consecutive).

    #[test]
    fn navigate_up_skips_consecutive_duplicates() {
        // After dedup_consecutive, ["a", "a", "b"] becomes ["a", "b"]
        let mut s = InputState::new();
        let history = vec!["a".into(), "b".into()]; // already deduplicated
        s.navigate_up(&history);
        assert_eq!(s.line, "b");
        s.navigate_up(&history);
        assert_eq!(s.line, "a");
        // "a" appears only once in the deduplicated snapshot
    }

    #[test]
    fn navigate_up_all_same_entries() {
        // After dedup_consecutive, ["x", "x", "x"] becomes ["x"]
        let mut s = InputState::new();
        let history = vec!["x".into()]; // already deduplicated
        s.insert_char('d');
        s.navigate_up(&history);
        assert_eq!(s.line, "x");
        // Pressing up again stays at the only entry
        s.navigate_up(&history);
        assert_eq!(s.line, "x");
        assert_eq!(s.history_index, Some(0));
    }

    // display_cursor tests

    #[test]
    fn display_cursor_ascii() {
        let mut s = InputState::new();
        s.insert_char('a');
        s.insert_char('b');
        s.insert_char('c');
        assert_eq!(s.display_cursor(), 3);
    }

    #[test]
    fn display_cursor_midline_ascii() {
        let mut s = InputState::new();
        s.insert_char('a');
        s.insert_char('b');
        s.insert_char('c');
        s.cursor = 1; // between 'a' and 'b'
        assert_eq!(s.display_cursor(), 1);
    }

    #[test]
    fn display_cursor_cjk_fullwidth() {
        let mut s = InputState::new();
        // CJK character 'あ' is 3 bytes in UTF-8, display width 2
        s.line = "あb".to_string();
        s.cursor = 3; // byte offset after 'あ' (3 bytes), before 'b'
        assert_eq!(s.display_cursor(), 2); // 'あ' occupies 2 columns
    }

    #[test]
    fn display_cursor_cjk_at_beginning() {
        let mut s = InputState::new();
        s.line = "あb".to_string();
        s.cursor = 0;
        assert_eq!(s.display_cursor(), 0);
    }

    #[test]
    fn display_cursor_cjk_at_end() {
        let mut s = InputState::new();
        s.line = "あb".to_string();
        s.cursor = s.line.len(); // after 'b'
        assert_eq!(s.display_cursor(), 3); // 2 (あ) + 1 (b) = 3
    }

    #[test]
    fn display_cursor_multiple_cjk() {
        let mut s = InputState::new();
        s.line = "あい".to_string(); // two CJK chars, each width 2
        s.cursor = 3; // after 'あ', before 'い'
        assert_eq!(s.display_cursor(), 2);
    }

    #[test]
    fn display_width_ascii() {
        let mut s = InputState::new();
        s.line = "abc".to_string();
        assert_eq!(s.display_width(), 3);
    }

    #[test]
    fn display_width_cjk() {
        let mut s = InputState::new();
        s.line = "あい".to_string();
        assert_eq!(s.display_width(), 4); // 2 chars × 2 columns each
    }

    #[test]
    fn str_display_width_ascii_cjk_and_mixed() {
        assert_eq!(str_display_width(""), 0);
        assert_eq!(str_display_width("> "), 2);
        assert_eq!(str_display_width("hello"), 5);
        assert_eq!(str_display_width("あいう"), 6); // 3 × 2
        assert_eq!(str_display_width("aあb"), 4); // 1 + 2 + 1
    }

    #[test]
    fn display_width_mixed_ascii_and_cjk() {
        let mut s = InputState::new();
        s.line = "ab漢字cd".to_string();
        // a,b = 2 ; 漢,字 = 4 ; c,d = 2 => 8
        assert_eq!(s.display_width(), 8);
    }
}