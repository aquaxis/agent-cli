use super::*;

pub(super) struct DisplayState {
    pub(super) thinking_printed: bool,
    answer_printed: bool,
    /// Whether any section header (`[answer]`/`[thinking]`/`[tool-call]`) has been
    /// printed in the current turn. The first header of a turn is emitted without a
    /// leading newline so it sits directly under the echoed input line, avoiding an
    /// extra blank line; subsequent headers keep the leading newline to separate
    /// from preceding streamed content.
    pub(super) section_printed: bool,
    /// When true, output is in crossterm raw mode and newlines must use CR+LF.
    raw_mode: bool,
    /// When true the progress indicator is drawing beneath the output, so the
    /// lines around it are kept short: a tool call is cut to a single terminal
    /// row and a tool result to [`TOOL_RESULT_ROWS`] rows, since the raw dumps
    /// would otherwise push the indicator many rows down.
    pub(super) compact_output: bool,
    /// When true, `thinking` text is not printed inline: the progress
    /// indicator shows it live under the spinner instead.
    pub(super) capture_thinking: bool,
    /// When false, the `[tool-call]` and `[tool-result]` lines are not
    /// printed at all (`[ui] show_tool_results`); the conversation log and
    /// the model still see every call and result in full.
    pub(super) show_tool_results: bool,
    /// Colour scheme. Styling is applied at the print sites below, always
    /// *after* the line has been measured and cut, so the column maths in
    /// `crate::editor` never sees an escape sequence.
    pub(super) theme: Theme,
}

impl DisplayState {
    pub(super) fn new(raw_mode: bool) -> Self {
        Self {
            thinking_printed: false,
            answer_printed: false,
            section_printed: false,
            raw_mode,
            compact_output: false,
            capture_thinking: false,
            show_tool_results: true,
            theme: Theme::plain(),
        }
    }

    pub(super) fn reset(&mut self) {
        self.thinking_printed = false;
        self.answer_printed = false;
        self.section_printed = false;
    }

    /// Print a section header (e.g. `[answer]`), prefixing a newline only if a
    /// previous section already printed this turn, styled with `role`.
    pub(super) fn section_header(&mut self, label: &str, role: Role) {
        let styled = self.theme.err(role, label).into_owned();
        self.section_header_styled(&styled);
    }

    /// Same, for a header the caller has already styled — the `[tool-call]`
    /// line, whose name and arguments carry different roles.
    ///
    /// The separating newline is added around the styled text, never inside it,
    /// so a blank row never carries a colour into a region the display erases.
    fn section_header_styled(&mut self, styled: &str) {
        raw_eprintln(
            self.raw_mode,
            &section_header_text(styled, self.section_printed),
        );
        self.section_printed = true;
    }
}

/// Build a section header line: prefix a newline only when a previous section has
/// already printed this turn, so the first header sits directly under the echoed
/// input with no extra blank line.
pub(super) fn section_header_text(label: &str, section_printed: bool) -> String {
    if section_printed {
        format!("\n{label}")
    } else {
        label.to_string()
    }
}

/// Spinner frames, advanced one per tick while a turn runs.
pub(super) const SPINNER_FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Repaint interval of the progress indicator.
pub(super) const TICK_INTERVAL: Duration = Duration::from_millis(100);

/// Rows of live `thinking` text shown under the spinner before the view is
/// elided. Clicking the block shows as much as the terminal can hold instead.
pub(super) const THINKING_ROWS: usize = 10;

/// Rows a tool result is cut to while the progress indicator is drawing, so a
/// large output cannot push the indicator off the screen. The full text is
/// always in the conversation log.
pub(super) const TOOL_RESULT_ROWS: usize = 5;

/// Spinner frame `n`, cycling through [`SPINNER_FRAMES`].
pub(super) fn spinner_frame(n: usize) -> char {
    SPINNER_FRAMES[n % SPINNER_FRAMES.len()]
}

/// Elapsed time of a turn: one decimal below a minute (`0.4s`, `12.4s`),
/// minutes and whole seconds above it (`1m00s`, `2m03s`). Truncated rather
/// than rounded, so the display never briefly shows `60.0s`.
pub(super) fn format_elapsed(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        let tenths = d.as_millis() / 100;
        format!("{}.{}s", tenths / 10, tenths % 10)
    } else {
        format!("{}m{:02}s", secs / 60, secs % 60)
    }
}

/// The `ToolCall` line as `display_event` renders it: cut to a single terminal
/// row when the progress indicator is drawing (`compact_output`), otherwise the
/// full form. Pure, so the tests pin both shapes without a terminal.
pub(super) fn tool_call_line(compact_output: bool, name: &str, args: &serde_json::Value) -> String {
    let line = one_line_tool_call(name, args);
    if compact_output {
        // Cut first, style second: the truncation counts columns, so it
        // must never see an escape sequence.
        crate::editor::truncate_display(&line, PromptRenderer::terminal_width().saturating_sub(1))
    } else {
        format!("[tool-call] {name} {args}")
    }
}

/// The `ToolResult` line as `display_event` renders it: the row-budgeted dump
/// when the progress indicator is drawing (`compact_output`), otherwise the
/// full form. Pure, so the tests pin both shapes without a terminal.
pub(super) fn tool_result_line(
    compact_output: bool,
    mark: &str,
    name: &str,
    output: &str,
) -> String {
    if compact_output {
        compact_tool_result(
            mark,
            name,
            output,
            PromptRenderer::terminal_width(),
            TOOL_RESULT_ROWS,
        )
    } else {
        format!("[tool-result {mark}] {name}: {output}")
    }
}

/// One-line rendering of a tool call for the indicator's activity row. Mirrors
/// the wording of the permanent `[tool-call]` line so the two read the same.
pub(super) fn one_line_tool_call(name: &str, args: &serde_json::Value) -> String {
    crate::editor::flatten_one_line(&format!("[tool-call] {name} {args}"))
}

/// A tool result cut to `max_rows` terminal rows, with a marker counting what
/// was left out — the counterpart of the one-line tool call, so a `bash` dump
/// cannot push the progress indicator far down the screen. The text is wrapped
/// (not cut) at `cols`, so the visible part is complete as far as it goes.
pub(super) fn compact_tool_result(
    mark: &str,
    name: &str,
    output: &str,
    cols: usize,
    max_rows: usize,
) -> String {
    let full = format!("[tool-result {mark}] {name}: {output}");
    let rows = crate::editor::wrap_display(&full, cols);
    if rows.len() <= max_rows {
        return full;
    }
    let hidden = rows.len() - max_rows;
    let mut out = rows[..max_rows].join("\n");
    out.push_str(&format!("\n… +{hidden} more lines"));
    out
}

/// Style a finished `[tool-call] <name> <args>` line: the marker and the tool
/// name carry the activity colour, the arguments are dimmed to grey.
///
/// The split is on the second space, which is exact because the line is built
/// as `"[tool-call] {name} {args}"` and a tool name never contains a space. A
/// line already truncated to the terminal width may end before that point, in
/// which case the whole of it is the name.
pub(super) fn style_tool_call_line(theme: &Theme, line: &str) -> String {
    match line
        .match_indices(' ')
        .nth(1)
        .map(|(i, _)| line.split_at(i + 1))
    {
        // The space between the two parts stays outside both sequences.
        Some((head, args)) => format!(
            "{} {}",
            theme.err(Role::ToolName, head.trim_end()),
            theme.err(Role::ToolArgs, args),
        ),
        None => theme.err(Role::ToolName, line).into_owned(),
    }
}

/// Style a possibly multi-row message row by row, so every row opens and closes
/// its own sequence. A colour must never span a line break: the row below may be
/// erased and redrawn (the progress indicator), or scrolled away.
pub(super) fn style_rows(theme: &Theme, role: Role, text: &str) -> String {
    if !theme.stderr {
        return text.to_string();
    }
    text.split('\n')
        .map(|row| theme.err(role, row).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether the cursor sits at column 0 of a fresh row once `ev` has been
/// displayed. `Text` leaves the cursor mid-row unless its delta ends with a
/// newline, and `Thinking` always does (except when hidden, which prints
/// nothing at all); every other event is printed as whole lines. `prev` is the
/// state before the event, returned unchanged when the event prints nothing.
pub(super) fn event_ends_at_line_start(
    ev: &AgentEvent,
    mode: ShowThinkingMode,
    capture_thinking: bool,
    show_tool_results: bool,
    prev: bool,
) -> bool {
    match ev {
        // Prints nothing: the indicator alone reacts to it.
        AgentEvent::TurnStart => prev,
        // Captured by the indicator, so nothing reaches the cursor.
        AgentEvent::Thinking { .. } if capture_thinking => prev,
        AgentEvent::Text { delta } => {
            if delta.is_empty() {
                prev
            } else {
                delta.ends_with('\n')
            }
        }
        AgentEvent::Thinking { .. } => match mode {
            ShowThinkingMode::Hidden => prev,
            _ => false,
        },
        // Suppressed by the key prints nothing, so the previous state stands.
        AgentEvent::ToolCall { .. } | AgentEvent::ToolResult { .. } if !show_tool_results => prev,
        AgentEvent::ToolCall { .. }
        | AgentEvent::ToolResult { .. }
        | AgentEvent::Info { .. }
        | AgentEvent::Done
        | AgentEvent::Error { .. } => true,
    }
}

/// Terminal mark closing a finished turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    Ok,
    Err,
}

impl Mark {
    fn glyph(self) -> char {
        match self {
            Mark::Ok => '✔',
            Mark::Err => '✗',
        }
    }
}

/// The turn currently being tracked by the indicator.
pub(super) struct Run {
    pub(super) started: std::time::Instant,
    pub(super) frame: usize,
}

/// Live progress indicator for the running turn.
///
/// While a turn runs it owns the bottom row of the output, showing
/// `<spinner> <elapsed>` directly beneath the line describing what is being
/// executed — the echoed question at first, then each `[tool-call]` line, which
/// the display shortens to a single row while the indicator is on. The row is
/// cut to one terminal row so it can never wrap, which keeps the erase
/// (`MoveToColumn(0)` + `Clear(FromCursorDown)`) exact.
///
/// The block is painted only from column 0 of a fresh row: after streamed text
/// has left the cursor mid-row there is no way to erase the block without
/// destroying that row, so the indicator simply stays unpainted until a whole
/// line has been emitted (the flowing text is itself the sign of activity).
///
/// Writes go through an injectable sink so the rendering can be asserted in
/// tests without a TTY. A disabled indicator (not a terminal, `[ui]
/// show_progress = false`, serve mode) never writes a single byte.
pub(super) struct StatusIndicator {
    out: Box<dyn std::io::Write + Send>,
    enabled: bool,
    /// Raw mode is on, so line breaks must be CR+LF.
    pub(super) raw_mode: bool,
    /// Fixed terminal width, used by tests instead of querying the terminal.
    pub(super) width_override: Option<usize>,
    pub(super) run: Option<Run>,
    /// The input loop owns the bottom of the screen (cancel notice, approval
    /// prompt, shutdown); the indicator must not paint until it is resumed.
    suspended: bool,
    /// The block is currently on screen.
    painted: bool,
    /// Number of terminal rows the painted block occupies (1 for the spinner
    /// row, plus the live `thinking` rows below it). Needed to move back to the
    /// top of the block when erasing it.
    painted_rows: u16,
    /// Reasoning text streamed so far this turn, shown live under the spinner.
    thinking: String,
    /// Show the whole `thinking` text (as much as the terminal can hold)
    /// instead of the last [`THINKING_ROWS`] rows. Toggled by clicking the
    /// block; kept across turns, since it is a view preference.
    expanded: bool,
    /// Fixed terminal height, used by tests instead of querying the terminal.
    pub(super) height_override: Option<usize>,
    /// Whether reasoning is shown at all (`[ui] show_thinking` is not
    /// `hidden`). With no `thinking` view there is nothing to click, so mouse
    /// reporting is left alone and text selection keeps working during a turn.
    pub(super) thinking_view: bool,
    /// Colour scheme. Each row is styled after it has been cut to the terminal
    /// width, and separately from its neighbours, so the row bookkeeping and
    /// the erase are unaffected.
    pub(super) theme: Theme,
    /// Whether mouse reporting is the indicator's to switch on and off. With
    /// the wheel scrollback live the input loop owns it for the whole session
    /// (it has to be on at the prompt too), so the end of a turn must not be
    /// able to turn it off underneath the wheel.
    pub(super) owns_mouse: bool,
    /// Whether the live prompt block owns the screen's bottom rows and the
    /// parked caret, with no turn zone holding the writes. The block's rows are
    /// cursor-relative from the prompt's own position, and the caret stands
    /// *inside* the prompt, so a repaint here lands on the prompt: the elapsed
    /// row takes the bottom-most prompt row. Nothing here writes while this
    /// holds — the prompt render is the screen's only writer, and the armed
    /// zone (which composes the elapsed row above the pinned span) is the only
    /// painter allowed on those rows.
    pub(super) prompt_owned: bool,
    /// The peers' band's display-only spinner clock. It exists only where no
    /// run of this session holds a phase for the band to ride (`Run.frame`);
    /// the tick and the scrolled redraw advance it one step per beat, so a
    /// spawned peer's band animates on an idle prompt. Never written to any
    /// record: the band's rows are display-only.
    pub(super) peer_tick: u8,
}

impl StatusIndicator {
    pub(super) fn new(out: Box<dyn std::io::Write + Send>, enabled: bool, raw_mode: bool) -> Self {
        Self {
            out,
            enabled,
            raw_mode,
            width_override: None,
            run: None,
            suspended: false,
            painted: false,
            painted_rows: 0,
            thinking: String::new(),
            expanded: false,
            height_override: None,
            thinking_view: true,
            theme: Theme::plain(),
            owns_mouse: true,
            prompt_owned: false,
            peer_tick: 0,
        }
    }

    /// Record whether the live prompt block owns the screen's bottom rows. While
    /// it does, nothing here writes to the screen: the caret is the prompt's to
    /// place, so the shipped cursor-relative block (erase upward from the
    /// caret, repaint downward from it) would land inside the prompt — the
    /// elapsed row on the bottom-most prompt row. Entering the state drops the
    /// painted bookkeeping without moving the cursor; leaving it lets the next
    /// paint start from wherever its writer then stands.
    pub(super) fn set_prompt_owned(&mut self, on: bool) {
        self.prompt_owned = on;
        if on {
            self.painted = false;
            self.painted_rows = 0;
        }
    }

    /// An indicator that never draws anything: used where there is no
    /// interactive terminal (serve mode, piped stdin, `show_progress = false`).
    pub(super) fn disabled() -> Self {
        Self::new(Box::new(std::io::sink()), false, false)
    }

    fn newline(&self) -> &'static str {
        if self.raw_mode {
            "\r\n"
        } else {
            "\n"
        }
    }

    fn width(&self) -> usize {
        self.width_override
            .unwrap_or_else(PromptRenderer::terminal_width)
    }

    fn height(&self) -> usize {
        self.height_override.unwrap_or_else(|| {
            terminal::size()
                .map(|(_, h)| h as usize)
                .unwrap_or(24)
                .max(1)
        })
    }

    /// Begin tracking a turn. Any suspension from a previous turn is lifted;
    /// the block itself is drawn by the following [`StatusIndicator::paint`].
    pub(super) fn start(&mut self) {
        if !self.enabled {
            return;
        }
        self.suspended = false;
        self.thinking.clear();
        self.run = Some(Run {
            started: std::time::Instant::now(),
            frame: 0,
        });
        // Clicking the block toggles the thinking view, so the terminal has to
        // report mouse events — but only while a turn is running, so ordinary
        // text selection keeps working at the prompt.
        self.set_mouse_capture(true);
    }

    /// Append streamed reasoning text to the live `thinking` view.
    pub(super) fn push_thinking(&mut self, text: &str) {
        if !self.enabled || !self.thinking_view || self.run.is_none() {
            return;
        }
        self.thinking.push_str(text);
    }

    /// Switch the `thinking` view between the last [`THINKING_ROWS`] rows and
    /// as much of the text as the terminal can hold, redrawing the block in
    /// place. Called from the input loop when the block is clicked.
    pub(super) fn toggle_expanded(&mut self) {
        if !self.enabled || self.run.is_none() {
            return;
        }
        self.expanded = !self.expanded;
        if self.painted && !self.suspended {
            self.repaint();
        }
    }

    /// Number of rows the block occupies on screen, `0` when it is not drawn.
    pub(super) fn painted_rows(&self) -> u16 {
        if self.painted {
            self.painted_rows
        } else {
            0
        }
    }

    fn set_mouse_capture(&mut self, on: bool) {
        if !self.enabled || !self.raw_mode || !self.thinking_view || !self.owns_mouse {
            return;
        }
        let _ = if on {
            crossterm::queue!(self.out, crossterm::event::EnableMouseCapture)
        } else {
            crossterm::queue!(self.out, crossterm::event::DisableMouseCapture)
        };
        let _ = self.out.flush();
    }

    /// The rows drawn below the spinner: the tail of the reasoning text, or as
    /// much of it as fits when expanded, with a marker row counting what was
    /// left out.
    pub(super) fn thinking_rows(&self) -> Vec<String> {
        if self.thinking.trim().is_empty() {
            return Vec::new();
        }
        let cols = self.width().saturating_sub(1);
        // Indented by two columns so the block reads as a detail of the
        // spinner row above it.
        let rows = crate::editor::wrap_display(self.thinking.trim_end(), cols.saturating_sub(2));
        // Never take more than the screen can hold: the activity line, the
        // spinner row, the marker row and two rows of context stay visible, so
        // expanding cannot push the whole conversation off the screen.
        let budget = self.height().saturating_sub(5).max(1);
        let visible = if self.expanded {
            budget
        } else {
            THINKING_ROWS.min(budget)
        };
        let hidden = rows.len().saturating_sub(visible);
        let mut out: Vec<String> = Vec::with_capacity(visible + 1);
        if hidden > 0 || self.expanded {
            let hint = if self.expanded {
                "click to collapse".to_string()
            } else {
                "click to expand".to_string()
            };
            let marker = if hidden > 0 {
                format!("… +{hidden} more ({hint})")
            } else {
                format!("… ({hint})")
            };
            out.push(crate::editor::truncate_display(&marker, cols));
        }
        out.extend(
            rows[rows.len() - visible.min(rows.len())..]
                .iter()
                .map(|r| crate::editor::truncate_display(&format!("  {r}"), cols)),
        );
        out
    }

    /// Advance the spinner and refresh the elapsed time, but only where the
    /// row is already on screen — a tick never *introduces* a row, so it
    /// cannot land in the middle of streamed output.
    pub(super) fn tick(&mut self) {
        if self.run.is_none()
            || !self.enabled
            || self.suspended
            || !self.painted
            || self.prompt_owned
        {
            return;
        }
        if let Some(run) = self.run.as_mut() {
            run.frame = run.frame.wrapping_add(1);
        }
        // The idle peers' band advances its own clock here: `tick` runs only
        // where the run's own row is already painted, so the band (painted by
        // the beat or the prompt) would otherwise stand still at rest.
        self.peer_tick = self.peer_tick.wrapping_add(1);
        self.repaint();
    }

    /// Draw the row when allowed to. `at_line_start` is the caller's knowledge
    /// of where the cursor is.
    pub(super) fn paint(&mut self, at_line_start: bool) {
        if !self.enabled
            || self.suspended
            || self.run.is_none()
            || !at_line_start
            || self.prompt_owned
        {
            return;
        }
        self.repaint();
    }

    /// The status row as it would be painted now — spinner and elapsed time,
    /// cut to the terminal width — or `None` when no turn is running.
    fn status_row(&self) -> Option<String> {
        let run = self.run.as_ref()?;
        Some(crate::editor::truncate_display(
            &format!(
                "{} {}",
                spinner_frame(run.frame),
                format_elapsed(run.started.elapsed())
            ),
            self.width().saturating_sub(1),
        ))
    }

    /// The status row for the scroll overlay, which pins it at the bottom of
    /// the screen while a turn runs. The block itself is suspended for as long
    /// as the overlay owns the screen, so `tick` does nothing and the spinner
    /// is advanced here instead.
    pub(super) fn overlay_status_row(&mut self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let run = self.run.as_mut()?;
        run.frame = run.frame.wrapping_add(1);
        self.status_row()
    }

    /// The armed zone's clock face: the styled rows the beat paints as the
    /// run display — the spinner-and-elapsed row and, when captured, the
    /// thinking rows under it — the shipped block's own face, unchanged.
    pub(super) fn zone_block_rows(&self) -> Vec<String> {
        let Some(status) = self.status_row() else {
            return Vec::new();
        };
        // The zone's face order: the thinking rows stand above the status
        // row, so the spinner-and-elapsed row is the block's LAST row — the
        // row that lands on the screen's bottom-most line while the agent
        // runs. The reasoning rows keep the progress colour with their
        // indent; the status row and the elision marker keep it too.
        let mut rows = self.thinking_rows();
        rows.push(status);
        // The reasoning rows keep their own colour (the indented rows are the
        // Thinking role, order-independent); the status row and the elision
        // marker keep the progress colour.
        rows.iter()
            .map(|row| {
                let role = if row.starts_with("  ") {
                    Role::Thinking
                } else {
                    Role::Progress
                };
                self.theme.err(role, row).into_owned()
            })
            .collect()
    }

    /// The beat's clock advance: the spinner's frame moves once per beat while
    /// the zone paints the row, standing in for the shipped tick's advance.
    pub(super) fn zone_advance_frame(&mut self) {
        if let Some(run) = self.run.as_mut() {
            run.frame = run.frame.wrapping_add(1);
        }
    }

    /// The peers' band's own beat: when no run of this session holds a phase
    /// for the band to ride, the display-only clock advances one step per
    /// beat, so a spawned peer's band animates on an otherwise idle prompt.
    pub(super) fn peer_clock_advance(&mut self) {
        if self.run.is_none() {
            self.peer_tick = self.peer_tick.wrapping_add(1);
        }
    }

    /// The peers' band's spinner glyph: in phase with the main run's frame
    /// while a turn runs, riding the beat-advanced display-only clock when
    /// idle. A disabled indicator holds the clock still — the phase freeze in
    /// place is consistent with the band's animation being display-only.
    pub(super) fn peer_spinner_glyph(&self) -> char {
        if let Some(run) = self.run.as_ref() {
            spinner_frame(run.frame)
        } else {
            spinner_frame(usize::from(self.peer_tick))
        }
    }

    pub(super) fn repaint(&mut self) {
        if zone_armed() {
            // The zone owns the rows above the span (the beat paints the
            // block's own face with `zone_block_rows`), so painting here
            // would land beside the pinned prompt. Nothing to un-do either:
            // the zone never paints with this handle, and `painted` never
            // rises while the gate is on. The gate sits inside, behind the
            // disabled/suspended/run checks, so the zone's borrow of the
            // block's face (`zone_block_rows`) is the only thing repainting
            // stands down from.
            return;
        }
        // The live prompt block's rows (the same rule `tick` and `paint` gate
        // on): the caret stands inside the prompt and this repair is
        // cursor-relative, so a repaint here lands on the prompt's own last
        // row. The armed zone is the only painter allowed above the span.
        if self.prompt_owned {
            return;
        }
        self.clear();
        if self.run.is_none() {
            return;
        }
        let Some(status) = self.status_row() else {
            return;
        };
        let mut rows = Vec::with_capacity(1 + THINKING_ROWS);
        rows.push(status);
        rows.extend(self.thinking_rows());
        let painted_rows = rows.len() as u16;
        // Style last, once every row has been cut to the terminal width, and
        // row by row so no sequence spans a line break: the block is erased and
        // redrawn ten times a second, and a colour left open would survive it.
        // The reasoning rows are the indented ones; the status row and the
        // elision marker above them belong to the progress colour.
        let styled: Vec<String> = rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let role = if i > 0 && row.starts_with("  ") {
                    Role::Thinking
                } else {
                    Role::Progress
                };
                self.theme.err(role, row).into_owned()
            })
            .collect();
        let nl = self.newline();
        let _ = write!(self.out, "{}", styled.join(nl));
        let _ = self.out.flush();
        self.painted = true;
        self.painted_rows = painted_rows;
    }

    /// Erase the block, leaving the cursor exactly at its origin (column 0 of
    /// the row where painting began), so the output above is untouched and the
    /// next write continues where it would have without the indicator.
    pub(super) fn clear(&mut self) {
        // The armed zone: the beat owns the block's rows on the live screen
        // and the shared cursor is parked on the prompt's caret — the shipped
        // relative repair (MoveUp + erase downward) would cut into the wrong
        // rows, so the flags reset quietly and the screen is left to the zone.
        if zone_armed() {
            self.painted = false;
            self.painted_rows = 0;
            return;
        }
        // The live screen's prompt block (the same rule the gate above serves):
        // the caret stands inside the prompt, so the shipped relative repair
        // (MoveUp from the caret + erase downward) would erase the prompt's own
        // rows. Nothing was painted there either, so the bookkeeping simply
        // resets.
        if self.prompt_owned {
            self.painted = false;
            self.painted_rows = 0;
            return;
        }
        if !self.painted {
            return;
        }
        use crossterm::cursor::{MoveToColumn, MoveUp};
        // The cursor sits on the last row of the block; go back to its first.
        if self.painted_rows > 1 {
            let _ = crossterm::queue!(self.out, MoveUp(self.painted_rows - 1));
        }
        let _ = crossterm::queue!(
            self.out,
            MoveToColumn(0),
            terminal::Clear(ClearType::FromCursorDown)
        );
        let _ = self.out.flush();
        self.painted = false;
        self.painted_rows = 0;
    }

    /// Close the turn: erase the row and leave one permanent line marking the
    /// outcome and the total elapsed time.
    pub(super) fn finish(&mut self, mark: Mark) {
        self.clear();
        let Some(run) = self.run.take() else {
            return;
        };
        self.thinking.clear();
        self.set_mouse_capture(false);
        let cols = self.width().saturating_sub(1);
        let line = crate::editor::truncate_display(
            &format!("{} {}", mark.glyph(), format_elapsed(run.started.elapsed())),
            cols,
        );
        let role = match mark {
            Mark::Ok => Role::Success,
            Mark::Err => Role::Failure,
        };
        let line = self.theme.err(role, &line);
        // The outcome row stays on screen, so it belongs to the transcript —
        // unlike the block above it, which is erased and redrawn ten times a
        // second and is never recorded. The record happens even when the block
        // is suspended (the scrolled view owns the screen): a turn that ends
        // while scrolled leaves its outcome line for the return to the bottom.
        crate::scroll::record_line(&line);
        // A suspended block cannot write here: the overlay's body would repair
        // over it, and the transcript record above is what the user sees when
        // the live view returns. The armed zone asks the same quietness: the
        // shared cursor is parked on the pinned span's caret, the row below the
        // last beat's paint is the zone's to compose, so the outcome line is
        // record-only here and surfaces through the disarm's replay.
        if !self.suspended && !zone_armed() && !self.prompt_owned {
            let nl = self.newline();
            let _ = write!(self.out, "{line}{nl}");
            let _ = self.out.flush();
        }
    }

    /// Drop the turn without marking it (its output was discarded, e.g. a
    /// cancelled turn draining behind a fresh prompt). Also used when the turn
    /// ends with nothing to mark.
    pub(super) fn abandon_run(&mut self) {
        self.clear();
        if self.run.take().is_some() {
            self.thinking.clear();
            self.set_mouse_capture(false);
        }
    }

    /// The input loop is about to write at the bottom of the screen.
    pub(super) fn suspend(&mut self) {
        self.clear();
        self.suspended = true;
    }

    pub(super) fn resume(&mut self) {
        self.suspended = false;
    }
}

/// Lock the shared indicator, recovering from a poisoned mutex: a panic in one
/// writer must not take the REPL's display down with it.
pub(super) fn lock_indicator(
    indicator: &std::sync::Mutex<StatusIndicator>,
) -> std::sync::MutexGuard<'_, StatusIndicator> {
    indicator.lock().unwrap_or_else(|e| e.into_inner())
}

pub(super) fn display_event(
    ev: AgentEvent,
    show_thinking: ShowThinkingMode,
    state: &mut DisplayState,
) {
    let rm = state.raw_mode;
    match ev {
        // The turn boundary drives the progress indicator only; it prints
        // nothing itself, so every non-interactive path stays byte-identical.
        AgentEvent::TurnStart => {}
        AgentEvent::Text { delta } => {
            if !state.answer_printed {
                state.section_header("[answer]", Role::AnswerMarker);
                state.answer_printed = true;
            }
            // The answer body is the longest thing on screen and is left
            // uncoloured on purpose.
            raw_print_str(rm, &delta);
        }
        // Captured by the progress indicator, which shows it live under the
        // spinner; printing it here too would duplicate it into the scrollback.
        AgentEvent::Thinking { .. } if state.capture_thinking => {}
        AgentEvent::Thinking { text } => match show_thinking {
            ShowThinkingMode::Hidden => {}
            ShowThinkingMode::Collapsed => {
                if !state.thinking_printed {
                    state.section_header("[thinking]", Role::Thinking);
                    state.thinking_printed = true;
                } else {
                    raw_eprint(rm, " ");
                }
                let collapsed = collapse_thinking_text(&text);
                raw_eprint(rm, &state.theme.err(Role::Thinking, &collapsed));
            }
            ShowThinkingMode::Expanded => {
                if !state.thinking_printed {
                    state.section_header("[thinking]", Role::Thinking);
                    state.thinking_printed = true;
                }
                raw_eprint(rm, &state.theme.err(Role::Thinking, &text));
            }
        },
        // `[ui] show_tool_results = false`: neither line is printed at all —
        // the empty arms keep the match exhaustive.
        AgentEvent::ToolCall { name, args } if state.show_tool_results => {
            let line = tool_call_line(state.compact_output, &name, &args);
            let styled = style_tool_call_line(&state.theme, &line);
            state.section_header_styled(&styled);
        }
        AgentEvent::ToolCall { .. } => {}
        AgentEvent::ToolResult { name, ok, output } if state.show_tool_results => {
            let mark = if ok { "ok" } else { "ERR" };
            let line = tool_result_line(state.compact_output, mark, &name, &output);
            raw_eprintln(rm, &style_rows(&state.theme, Role::ToolOutput, &line));
        }
        AgentEvent::ToolResult { .. } => {}
        AgentEvent::Done => {
            state.reset();
            raw_println(rm, "");
        }
        AgentEvent::Error { message } => {
            state.reset();
            let line = format!("[error] {message}");
            // The separating newline stays outside the sequence.
            raw_eprintln(
                rm,
                &format!("\n{}", style_rows(&state.theme, Role::Failure, &line)),
            );
        }
        AgentEvent::Info { message } => {
            let line = format!("[info] {message}");
            raw_eprintln(rm, &style_rows(&state.theme, Role::Info, &line));
        }
    }
}

/// Truncate thinking delta to 1 line / 80 characters when `[ui] show_thinking =
/// "collapsed"`. Prevents models that return long reasoning (e.g. `glm-5.1:cloud`) from
/// flooding the REPL output in a single turn.
pub(super) fn collapse_thinking_text(text: &str) -> String {
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
