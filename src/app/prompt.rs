use super::*;

/// The parameters of [`render_prompt_with_run`]: the prompt identity
/// (`theme`, `role`, `prompt`, `input`) plus the optional running turn's
/// elapsed row (`run_row`), which only the scrolled mid-turn face carries.
pub(super) struct PromptRunParams<'a> {
    pub(super) theme: &'a Theme,
    pub(super) role: Role,
    pub(super) prompt: &'a str,
    pub(super) input: &'a InputState,
    pub(super) run_row: Option<String>,
}

/// The parameters of [`PromptRenderer::render`], bundled so the method's
/// signature stays under the argument-count lint (the 0.53.2 convention:
/// long parameter lists became small structs). `theme` colours the drawn
/// block; `input` is the editing buffer the line is drawn from; the rest
/// name the optional rows painted above it.
pub(super) struct RenderParams<'a> {
    pub(super) theme: &'a Theme,
    pub(super) role: Role,
    pub(super) prompt: &'a str,
    pub(super) input: &'a InputState,
    pub(super) suggestion: Option<&'a str>,
    pub(super) peer_lines: &'a [String],
    pub(super) run_row: Option<&'a str>,
    pub(super) marker: Option<&'a str>,
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
pub(super) fn layout_cursor(
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
pub(super) struct PromptRenderer {
    /// Physical row offset of the cursor from the top of the last rendered block.
    pub(super) cursor_row: u16,
    /// Rows the subagent status block occupies above the rendered block (or
    /// above the suggestion row when one is shown). They are part of the
    /// tracked region, so `clear_block` erases them together with the block
    /// and a finished subagent's row vanishes at the next render rather than
    /// leaving a stale line on the live screen.
    pub(super) peer_rows: u16,
    /// Transient rows the last render placed above the prompt span but below
    /// the peer rows: the running turn's elapsed row, the scroll marker and
    /// the candidate hint. Unlike `peer_rows` and `cursor_row` this counter
    /// never drives the erase; it feeds the shrink anchor below.
    pub(super) hint_rows: u16,
    /// Blank rows the renderer owns between the log tail and the prompt block.
    /// A block that shrinks (the candidate hint's row disappearing) keeps the
    /// freed row instead of painting one row higher — the prompt line never
    /// moves up — and records the blank here; a block that grows back reuses
    /// these rows before sinking lower. `clear_block` erases them too whenever
    /// the caller surrenders the region, so nothing stray survives a notice
    /// print or a submit.
    pub(super) spare_rows: u16,
}

impl PromptRenderer {
    pub(super) fn new() -> Self {
        Self {
            cursor_row: 0,
            peer_rows: 0,
            hint_rows: 0,
            spare_rows: 0,
        }
    }

    /// Mark the cursor as being on a fresh, empty line (no previous block above).
    /// Call this after emitting a newline outside the renderer (e.g. after submit
    /// or printing an out-of-band message).
    pub(super) fn reset(&mut self) {
        self.cursor_row = 0;
        self.peer_rows = 0;
        self.hint_rows = 0;
        self.spare_rows = 0;
    }

    pub(super) fn terminal_width() -> usize {
        terminal::size()
            .map(|(w, _)| w as usize)
            .unwrap_or(80)
            .max(1)
    }

    /// Move to the top-left of the previously rendered block and clear it
    /// (including any wrapped rows below). Leaves the cursor at column 0 and
    /// resets the tracked row to 0.
    ///
    /// `keep_spare` decides what happens to the owned blank rows between the
    /// log tail and the block: the render path keeps them (the hint's slot
    /// survives the erase and the prompt line holds its row), while callers
    /// that surrender the region — a notice print, a submit echo, the
    /// shutdown — absorb them, so the next print lands directly below the log.
    pub(super) fn clear_block(&mut self, stdout: &mut std::io::Stdout, keep_spare: bool) {
        use crossterm::cursor::{MoveToColumn, MoveUp};
        // The armed zone: the beat owns the rows above the span and the
        // shared cursor is parked on the pinned prompt's caret — the
        // relative repair (MoveUp + clear downward) would cut into the wrong
        // rows. The flags reset quietly and the screen is left to the zone;
        // the next armed render repaints the span in place.
        if zone_armed() {
            self.cursor_row = 0;
            self.peer_rows = 0;
            self.hint_rows = 0;
            self.spare_rows = 0;
            return;
        }
        // The whole tracked region — plus the owned blank rows above it when
        // the caller surrenders them — climbed from the cursor. The erase
        // (`Clear(FromCursorDown)` from the region top) covers the rest.
        let rows = if keep_spare {
            self.peer_rows + self.cursor_row
        } else {
            self.peer_rows + self.cursor_row + self.spare_rows
        };
        if rows > 0 {
            let _ = stdout.execute(MoveUp(rows));
        }
        let _ = stdout.execute(MoveToColumn(0));
        let _ = stdout.execute(terminal::Clear(ClearType::FromCursorDown));
        self.cursor_row = 0;
        self.peer_rows = 0;
        self.hint_rows = 0;
        self.spare_rows = 0;
        let _ = stdout.flush();
    }

    /// Finalize a submitted line: clear the in-progress (cursor-positioned)
    /// render, then re-emit `prompt` + the submitted `line` as a static, fully
    /// visible line followed by a newline — like a normal shell echoing input on
    /// Enter. Leaves the cursor at column 0 of a fresh line.
    pub(super) fn finish_line(
        &mut self,
        stdout: &mut std::io::Stdout,
        theme: &Theme,
        prompt_role: Role,
        prompt: &str,
        line: &str,
    ) {
        use crossterm::style::Print;
        // The armed zone: the echo belongs to the record and the beat
        // surfaces it — writing here would cut into or scroll the standing
        // shape the zone owns.
        if zone_armed() {
            self.clear_block(stdout, false);
            let styled_prompt = theme.out(prompt_role, prompt);
            let styled_line = theme.out(Role::InputText, line);
            crate::scroll::record_line(&format!("{styled_prompt}{styled_line}"));
            return;
        }
        // Remove the interactive render (cursor may be mid-line) before echoing.
        // The region is surrendered here: the spare rows are absorbed, so the
        // echo lands directly below the log.
        self.clear_block(stdout, false);
        let styled_prompt = theme.out(prompt_role, prompt);
        let styled_line = theme.out(Role::InputText, line);
        // The echo is what stays on screen — the block being edited is not.
        crate::scroll::record_line(&format!("{styled_prompt}{styled_line}"));
        let _ = stdout.execute(Print(&styled_prompt));
        let _ = stdout.execute(Print(&styled_line));
        // Advance to a fresh line; CR+LF is required in raw mode.
        let _ = stdout.execute(Print("\r\n"));
        self.cursor_row = 0;
        let _ = stdout.flush();
    }

    /// Render `prompt` + the editor buffer, wrapping correctly and positioning the
    /// cursor at the logical insertion point. When `suggestion` is `Some`, a
    /// one-line candidate hint is printed **above** the prompt line (used for live
    /// slash-command completion), so the line being typed always stays at the
    /// bottom of the block where the eye already is. The hint occupies one extra
    /// row at the top of the block, which `clear_block` removes on the next render
    /// (it moves to the block top and clears downward). The subagent status rows
    /// (`peer_lines`) print above the whole tracked region and are erased with it.
    /// When `run_row` is `Some`, the running turn's elapsed row is printed above
    /// the block's rows — display-only, never recorded — and when `marker` is
    /// `Some`, the scroll state row (`Message below`) is printed on the row
    /// immediately above the block's first row. Both are part of the tracked
    /// region, so the fold below counts them and the next render's erase reaches
    /// past them.
    pub(super) fn render(&mut self, stdout: &mut std::io::Stdout, p: RenderParams) {
        use crossterm::cursor::{MoveDown, MoveToColumn, MoveUp};
        use crossterm::style::Print;

        let RenderParams {
            theme,
            role: prompt_role,
            prompt,
            input: state,
            suggestion,
            peer_lines,
            run_row,
            marker,
        } = p;
        let width = Self::terminal_width();

        // Measure the coming block before touching the screen: the render is
        // bottom-anchored against shrinkage, so the rows the block will place
        // above the prompt span must be known before the previous block is
        // erased. Pure arithmetic — no terminal queries beyond the width —
        // so the measurement cannot observe or disturb what follows.
        let above_rows = u16::from(run_row.is_some()) + u16::from(marker.is_some());
        let sug_rows = suggestion_rows(suggestion, width);
        let new_above = peer_lines.len() as u16 + above_rows + sug_rows;

        // 1. Clear the previous block (handles multi-row renders + suggestion).
        // The erase covers the tracked region only — the spare rows above it
        // survive this step, because a growing block reuses them instead of
        // painting over the log.
        let old_above = self.peer_rows + self.hint_rows;
        self.clear_block(stdout, true);
        // Anchor against the block's shrinkage: the rows above the span
        // (peers, elapsed row, marker, hint) decide where the span's bottom
        // row lands. Fewer rows than the last render painted means the block
        // would sit one row higher — the prompt line would climb — so the
        // paint starts lower instead: the span keeps its row and the freed
        // rows become the renderer's own blank rows (`spare_rows`). More rows
        // means the block grows downward; it reuses the spare rows first,
        // before sinking into fresh space below the block. Pure arithmetic,
        // decided by the helper below so the pins can bind it without a
        // terminal.
        let (move_down, move_up) = hint_anchor(old_above, new_above, self.spare_rows);
        if move_down > 0 {
            let _ = stdout.execute(MoveDown(move_down));
        }
        if move_up > 0 {
            let _ = stdout.execute(MoveUp(move_up));
        }
        self.spare_rows = spare_after_anchor(old_above, new_above, self.spare_rows);
        self.hint_rows = above_rows + sug_rows;

        // 2. Print the subagent status rows above the block's rows, oldest
        //    first (the painter works downward), so the newest stands on the
        //    row nearest the block — the instruction's bottom-up ordering. The
        //    block owns the screen's bottom rows, so these stack from the row
        //    above it.
        for line in peer_lines {
            let _ = stdout.execute(Print(theme.err(Role::Peer, line)));
            // CR+LF is required in raw mode; this lands at column 0 of the
            // next row, where the following peer line or the block begins.
            let _ = stdout.execute(Print("\r\n"));
        }
        self.peer_rows = peer_lines.len() as u16;

        // 2.5 The running turn's elapsed row: display-only, printed
        //     above the block's rows — the row the live screen stands directly
        //     above the prompt span, so the scrolled screen keeps it in sight
        //     while a selection is made. It is never recorded here: only
        //     `finish_line` writes the transcript.
        let mut above_rows = 0u16;
        if let Some(run_row) = run_row {
            let _ = stdout.execute(Print(theme.err(Role::Progress, run_row)));
            let _ = stdout.execute(Print("\r\n"));
            above_rows += 1;
        }

        // 2.6 The scroll marker: the row immediately above the block's
        //     first row, so the prompt the eye is on stays the last thing on
        //     the screen and the marker names what the view has left below.
        //     It is part of the tracked region now, so the fold at the end
        //     counts it and the next render's erase climbs past it.
        if let Some(marker) = marker {
            let _ = stdout.execute(Print(theme.out(
                Role::Scrolled,
                &crate::editor::truncate_display(marker, width.saturating_sub(1)),
            )));
            let _ = stdout.execute(Print("\r\n"));
            above_rows += 1;
        }

        // 3. Print the candidate hint first, on its own row above the prompt block.
        //    Skipped when the terminal is too narrow to be useful (e.g. a pty
        //    with no reported size), so we never render a lone ellipsis.
        let sug_rows = suggestion_rows(suggestion, width);
        if sug_rows > 0 {
            let sug = truncate_suggestion(suggestion.unwrap_or(""), width - 1);
            let _ = stdout.execute(Print(theme.out(Role::Hint, &sug)));
            // CR+LF is required in raw mode; this lands at column 0 of the
            // prompt row, one row below the hint.
            let _ = stdout.execute(Print("\r\n"));
        }

        // 4. Print prompt + line; the terminal auto-wraps long content.
        let _ = stdout.execute(Print(theme.out(prompt_role, prompt)));
        let _ = stdout.execute(Print(theme.out(Role::InputText, &state.line)));

        // 5. Compute the target cursor position within the prompt rows. The
        //    measurements are taken from the *plain* text: a style adds no
        //    printable column, but its escape sequence would be counted.
        let prompt_cols = crate::editor::str_display_width(prompt);
        let total_cols = prompt_cols + state.display_width();
        let cursor_cols = state.display_cursor();
        let (end_row, cursor_row, cursor_col) =
            layout_cursor(prompt_cols, cursor_cols, total_cols, width);

        // 6. Move from the post-print position (end_row) to the cursor row.
        //    Both are relative to the prompt row, so the hint above does not
        //    change this step — only the block-relative bookkeeping in 7.
        if end_row > cursor_row {
            let _ = stdout.execute(MoveUp((end_row - cursor_row) as u16));
        } else if cursor_row > end_row {
            // Cursor belongs on a fresh wrapped row past the printed content
            // (cursor at the end of a line that exactly fills the width).
            let _ = stdout.execute(MoveDown((cursor_row - end_row) as u16));
        }
        let _ = stdout.execute(MoveToColumn(cursor_col as u16));

        // 6. Remember where the cursor ended up for the next render, counting
        //    the hint row so `clear_block` moves all the way back to the top
        //    of the tracked region (the peer rows included), plus the rows
        //    printed above the block — the running turn's elapsed row and the
        //    scroll marker — so the next erase climbs past them and neither
        //    leaves a stale row behind.
        self.cursor_row = above_rows + sug_rows + cursor_row as u16;
        let _ = stdout.flush();
    }
}

/// Number of rows the candidate hint occupies above the prompt: 1 when there is
/// something to show and the terminal is wide enough, 0 otherwise. Pure so the
/// renderer's row bookkeeping can be unit-tested without a TTY.
pub(super) fn suggestion_rows(suggestion: Option<&str>, width: usize) -> u16 {
    match suggestion {
        Some(s) if !s.is_empty() && width >= 8 => 1,
        _ => 0,
    }
}

/// The shrink anchor's pure half: the rows to move before the block paints.
///
/// When the block's rows above the prompt span shrink (the candidate hint's row
/// disappears, a subagent's status row vanishes) the shipped top-anchored paint
/// would draw the span one row higher — the prompt line climbs. The fix moves
/// the writer down by the shrink amount first, so the span's bottom row keeps
/// its absolute place and the freed rows become the renderer's own blank rows.
/// When the block grows back (the hint reappears), the move goes up but only
/// over the blank rows the renderer itself owns (`spare`), never over the log.
///
/// Returns `(move_down, move_up)`; both are never positive at once, and the
/// up-move is clamped to `spare`. Pure so the pins bind the arithmetic without
/// a terminal.
pub(super) fn hint_anchor(old_above: u16, new_above: u16, spare: u16) -> (u16, u16) {
    if new_above < old_above {
        (old_above - new_above, 0)
    } else {
        let grow = new_above - old_above;
        (0, grow.min(spare))
    }
}

/// The shrink anchor's bookkeeping half: how many blank rows the renderer owns
/// after the anchor moved. A shrink frees rows into the ownership (they are
/// blank between the log and the newly anchored block); a grow that reused some
/// of them gives them back to the log's side. Pure, paired with
/// [`hint_anchor`].
pub(super) fn spare_after_anchor(old_above: u16, new_above: u16, spare: u16) -> u16 {
    if new_above < old_above {
        spare.saturating_add(old_above - new_above)
    } else {
        spare - (new_above - old_above).min(spare)
    }
}
