use super::*;

/// The scrolled view of the session log.
///
/// While it is active it owns the screen: the terminal is switched to the
/// alternate screen, the transcript slice is drawn there with the prompt block
/// pinned beneath it, and the switch is undone when the view returns to the
/// bottom. The main screen is never written to in between, so leaving restores
/// it — and the terminal's own scrollback with it — exactly as it was.
pub(super) struct ScrollOverlay {
    pub(super) view: crate::scroll::ScrollView,
    /// The alternate screen is currently ours.
    active: bool,
    /// draws the body rows, so the highlight costs no change to any signature
    /// between the input loop and `draw_scroll_overlay`.
    selection: Option<crate::select::Selection>,
    /// When the screen was last drawn, so a running turn's pinned spinner can
    /// be refreshed without repainting on every 50ms poll.
    last_draw: std::time::Instant,
}

/// How often a scrolled screen is redrawn while a turn runs: often enough for
/// the pinned spinner to live, rare enough not to flicker.
pub(super) const SCROLL_REFRESH: Duration = Duration::from_millis(100);

/// How often it is redrawn when no turn is running. Nothing should be writing
/// underneath it then — but a peer's prompt can start a turn the REPL did not,
/// so the screen is repaired on a slow beat rather than never.
pub(super) const SCROLL_IDLE_REFRESH: Duration = Duration::from_secs(1);

impl ScrollOverlay {
    pub(super) fn new() -> Self {
        Self {
            view: crate::scroll::ScrollView::default(),
            active: false,
            selection: None,
            last_draw: std::time::Instant::now(),
        }
    }

    /// Whether the overlay owns the screen.
    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    /// Whether the view has moved off the bottom of the log.
    pub(super) fn view_active(&self) -> bool {
        self.view.is_active()
    }

    /// Whether a drag is in progress. The overlay owns the screen while one is,
    /// even at the bottom of the log, because the live screen cannot be
    /// repainted with a highlight on it.
    pub(super) fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    /// Start a drag at `at`, on whatever the view is showing now.
    pub(super) fn begin_selection(&mut self, at: crate::select::Point) {
        self.selection = Some(crate::select::Selection::begin(at, self.view.offset()));
    }

    /// Move the loose end of the drag. Returns whether anything moved, so a
    /// burst of drag reports costs one repaint.
    pub(super) fn extend_selection(&mut self, to: crate::select::Point) -> bool {
        match self.selection.as_mut() {
            Some(sel) if sel.cursor != to => {
                sel.extend(to);
                true
            }
            _ => false,
        }
    }

    /// End the drag and hand back what was selected.
    pub(super) fn take_selection(&mut self) -> Option<crate::select::Selection> {
        self.selection.take()
    }

    /// A wheel report: move the view by one notch. Returns whether the screen
    /// needs redrawing, so a burst of reports costs one repaint.
    pub(super) fn wheel(&mut self, up: bool, cols: usize, body: usize) -> bool {
        let max = crate::scroll::with(|t| crate::scroll::max_offset(t, cols, body)).unwrap_or(0);
        let step = crate::scroll::STEP as isize;
        self.view.scroll(if up { step } else { -step }, max)
    }

    pub(super) fn needs_refresh(&self, turn_running: bool) -> bool {
        let interval = if turn_running {
            SCROLL_REFRESH
        } else {
            SCROLL_IDLE_REFRESH
        };
        self.active && self.last_draw.elapsed() >= interval
    }

    pub(super) fn enter(&mut self, stdout: &mut std::io::Stdout) {
        if self.active {
            // Already on the alternate screen: reset it (the drawn rows stand
            // from the last view) and hand the zone back its margin.
            self.view.reset();
            return;
        }
        let _ = stdout.execute(terminal::EnterAlternateScreen);
        // The alternate screen owns the terminal while the overlay draws: the
        // armed turn zone stops writing (its bookkeeping keeps running) so a
        // turn's in-flight writes cannot land on the overlay's screen.
        zone_hide();
        self.active = true;
    }

    /// Return to the live screen. Idempotent, so every path out of the REPL can
    /// call it without checking first.
    pub(super) fn leave(&mut self, stdout: &mut std::io::Stdout) {
        self.view.reset();
        self.selection = None;
        if !self.active {
            return;
        }
        let _ = stdout.execute(terminal::LeaveAlternateScreen);
        self.active = false;
        // Back on the live screen: re-issue the armed turn zone's own margin
        // (the alternate screen the overlay just left consumed it), replay the
        // transcript tail into the region, and recompose the band + the park.
        zone_show();
    }
}

impl Drop for ScrollOverlay {
    /// A panic, a signal or any early return must not leave the terminal on the
    /// alternate screen.
    fn drop(&mut self) {
        if self.active {
            let _ = std::io::stdout().execute(terminal::LeaveAlternateScreen);
        }
    }
}

/// What is pinned to the bottom of the scrolled screen: one block, composed
/// the way the live screen composes — the prompt span at the bottom-most rows,
/// the running turn's elapsed row directly above it, the subagent run-state
/// rows above that, and the scroll marker on the row immediately above the
/// prompt's first row.
///
/// One composition for every state is what keeps a pointer honest: the row a
/// press lands on is the row the screen shows, whether a turn runs or not.
pub(super) struct ScrollBottom<'a> {
    pub(super) role: Role,
    pub(super) prompt: &'a str,
    pub(super) input: &'a InputState,
    pub(super) suggestion: Option<&'a str>,
    pub(super) peer_lines: Vec<String>,
    /// The running turn's elapsed row, painted above the block — display-only,
    /// never recorded.
    pub(super) run_row: Option<String>,
    /// `Message below` while the view is off the log's bottom, `None` at it
    /// (painted above the block, never beneath it).
    pub(super) text: Option<String>,
}

impl ScrollBottom<'_> {
    /// The subagent rows the block carries (empty when none).
    fn peer_lines(&self) -> Vec<String> {
        self.peer_lines.clone()
    }

    /// The scroll marker row, `None` when the block carries none: the block
    /// paints `Message below` on the row immediately above its first row while
    /// the view is off the bottom.
    fn marker_row(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// The running turn's elapsed row, `None` when no turn is tracked: the
    /// block paints it above itself so the elapsed display stays on screen
    /// while the scrolled view is exercised.
    fn run_row(&self) -> Option<&str> {
        self.run_row.as_deref()
    }
}

/// Rows the running turn's elapsed row adds above the prompt span: one while a
/// turn is tracked, none otherwise. Pure so the drawer and both mouse arms
/// share the arithmetic.
pub(super) fn run_row_rows(running: bool) -> u16 {
    u16::from(running)
}

/// The scrolled screen's pinned block, read top-down as one plan: one row per
/// live subagent (oldest on top — the painter walks downward), the running
/// turn's elapsed row, the scroll marker, then the prompt span's own rows at
/// the bottom. Rows are block-relative, so the pin binds the order without a
/// terminal.
///
/// The order is the live screen's, with the marker inserted between the run
/// display and the prompt: the elapsed row stands directly above the prompt
/// span and the peer rows above it, exactly as `zone_beat_plan` stacks the live
/// screen's regions. The marker is the last row before the prompt, so a wrapped
/// prompt keeps it directly above the block's first row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScrolledBlockPlan {
    /// The running turn's elapsed row, when a turn is tracked.
    pub(super) run_row: Option<u16>,
    /// The marker's row, when the view is off the log's bottom.
    pub(super) marker_row: Option<u16>,
    /// The prompt span's first row.
    pub(super) prompt_top: u16,
    /// The block's whole footprint, the prompt span's rows included.
    pub(super) rows: u16,
}

pub(super) fn scrolled_block_plan(
    prompt_rows: u16,
    peer_lines: usize,
    running: bool,
    view_moved: bool,
) -> ScrolledBlockPlan {
    let peer_rows = peer_lines as u16;
    let run_rows = run_row_rows(running);
    let marker_rows = scrolled_marker_rows(view_moved);
    let prompt_top = peer_rows + run_rows + marker_rows;
    ScrolledBlockPlan {
        run_row: running.then_some(peer_rows),
        marker_row: view_moved.then_some(peer_rows + run_rows),
        prompt_top,
        rows: prompt_top + prompt_rows,
    }
}

/// The scrolled screen's pinned block, counted once: the prompt span's rows,
/// the running turn's elapsed row above them, the scroll marker's row above
/// that and one row per live subagent (additive to the block). The suggestion
/// row takes the same additive slot when the drawer counts it — the mouse arms
/// leave it out, so their budget stays a subset of the drawer's true count in
/// every mixed state.
///
/// One function is the whole point: the drawer's `bottom_rows`, the wheel
/// arm's body and the drag arm's body cannot disagree about where the log
/// ends, so a pointer row means the same content row the screen shows.
pub(super) fn scrolled_bottom_rows(
    prompt: &str,
    input_state: &InputState,
    width: usize,
    sug_rows: u16,
    peer_lines: usize,
    running: bool,
    view_moved: bool,
) -> u16 {
    let prompt_cols = crate::editor::str_display_width(prompt);
    let prompt_rows = prompt_block_rows(
        prompt_cols,
        input_state.display_cursor(),
        prompt_cols + input_state.display_width(),
        sug_rows,
        width,
    );
    scrolled_block_plan(prompt_rows, peer_lines, running, view_moved).rows
}

/// Rows the pinned bottom block occupies, as the mouse arms measure it: the
/// same composition the drawer paints, without the suggestion row the arms
/// have never counted. A running turn keeps the prompt span on screen and
/// carries its elapsed row above it, so the arms' body shrinks by exactly the
/// rows the screen shows — the press, the drag and the release all resolve
/// against what is drawn.
pub(super) fn bottom_block_rows(
    prompt_state: &PromptState,
    input_state: &InputState,
    width: usize,
    peer_lines: usize,
    view_moved: bool,
) -> u16 {
    let p = if prompt_state.is_awaiting_approval() {
        APPROVAL_PROMPT
    } else {
        PROMPT
    };
    scrolled_bottom_rows(
        p,
        input_state,
        width,
        0,
        peer_lines,
        prompt_state.is_pending(),
        view_moved,
    )
}

/// The peer's situation face: one row per live direct child of this session,
/// naming the peer, its operating time and — when the peer reports one
/// (through its registry entry) — its processing content. Pure over the
/// children list, so the pins compose rows without a registry. The content's
/// absence degrades to the name-and-operating-time face.
///
/// `glyph`, when the caller resolves one, is the Terminal Spinner the face
/// leads with while the child runs (`<spinner> peer <name> · <elapsed>[ ·
/// <turn>]`). The caller resolves it from the same spinner clock the status
/// row rides (the run's frame when a turn runs, the beat-advanced
/// display-only clock when idle), so every spinner row on the screen animates
/// in phase; `None` composes the row without the leading glyph.
///
/// The whole list rides on one terminal row when every row fits; `width` is the
/// budget the truncation shares.
pub(super) fn peer_status_rows(
    children: &[(String, std::time::Duration, Option<String>)],
    width: usize,
    glyph: Option<char>,
) -> Vec<String> {
    children
        .iter()
        .map(|(name, age, turn)| {
            let face = match turn.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
                Some(t) => format!(
                    "{}peer {name} · {} · {t}",
                    glyph_prefix(glyph),
                    format_elapsed(*age)
                ),
                None => format!(
                    "{}peer {name} · {}",
                    glyph_prefix(glyph),
                    format_elapsed(*age)
                ),
            };
            crate::editor::truncate_display(&face, width.saturating_sub(1))
        })
        .collect()
}

/// The face's leading spinner: the caller's glyph and a separating space, or
/// nothing when the composer runs without a phase.
fn glyph_prefix(glyph: Option<char>) -> String {
    glyph.map_or(String::new(), |g| format!("{g} "))
}

/// The live direct children of this session: the display name/id and the
/// operating time (`now − started_at`), read from the shared registry.
/// Liveness is the registry's own reap-on-read — a reaped entry is a finished
/// subagent, and the removal is decided here, per read.
pub(super) fn live_children_states(
    registry_dir: &Path,
    self_id: &crate::id::AgentId,
) -> Vec<(String, std::time::Duration, Option<String>)> {
    let mut children = crate::ipc::registry::children_of(registry_dir, self_id).unwrap_or_default();
    // The rows stack from the bottom line upward with the newest nearest the
    // bottom, and the painter walks downward over this list, so it is the
    // spawn order itself: the registry's read order is the filesystem's, not
    // the spawn order — sort oldest-first, which puts the newest on the row
    // just above the block (or the clock) that holds the bottom.
    children.sort_by_key(|a| a.started_at);
    children
        .into_iter()
        .map(|e| {
            let age = chrono::Utc::now()
                .signed_duration_since(e.started_at)
                .to_std()
                .unwrap_or_default();
            (peer_display_name(&e), age, e.turn)
        })
        .collect()
}

pub(super) fn peer_display_name(e: &crate::ipc::registry::RegistryEntry) -> String {
    e.name.clone().unwrap_or_else(|| e.id.to_string())
}

/// The peer lines the live screen's bottom band carries now, as painted text:
/// the registry read and the composition in one call. `glyph` is the Terminal
/// Spinner the running rows carry (R3.1); `SPINNER_FRAMES[0]` keeps the pure
/// faces the pins assert byte-for-byte equal to the shipped ones.
pub(super) fn live_peer_lines(
    registry_dir: &Path,
    self_id: &crate::id::AgentId,
    width: usize,
    glyph: char,
) -> Vec<String> {
    peer_status_rows(
        &live_children_states(registry_dir, self_id),
        width,
        Some(glyph),
    )
}

/// The transcript offset the scrolled body is sliced at, given the view's
/// `offset` and the marker rows the pinned block carries.
///
/// The view's offset counts rows from the live screen, whose body has no
/// marker row. The marker takes the body's last row, so the slice ends that
/// many rows further up: the rows above the marker stay exactly where the live
/// screen had them, moved by the notch and nothing else. Without this the
/// first notch off the live view moved the log by `STEP` minus the marker
/// rows while every later notch moved it by `STEP`.
pub(super) fn slice_offset(offset: usize, marker_rows: usize) -> usize {
    offset + marker_rows
}

/// The body rows a screen of `body` rows shows at `offset`, padded at the top
/// so that index *i* is screen row *i*.
///
/// A log shorter than the screen sits against the bottom block, where it would
/// be on a terminal that has not scrolled yet; the padding rows are empty, so a
/// selection over them contributes nothing. Drawing and copying share this so
/// the highlight and the clipboard can never disagree about which row is which.
pub(super) fn body_slice_at(width: usize, body: usize, offset: usize) -> Vec<crate::scroll::Row> {
    let rows = crate::scroll::with(|t| crate::scroll::marked_rows(t, width, body, offset))
        .unwrap_or_default();
    let top = body.saturating_sub(rows.len());
    let mut out = Vec::with_capacity(body);
    out.resize(
        top,
        crate::scroll::Row {
            text: String::new(),
            continues: false,
        },
    );
    out.extend(rows);
    out
}

/// Draw the scrolled screen: the transcript slice for the current offset, one
/// row per terminal row, with the bottom block pinned beneath it. Every row is
/// cleared as it is written, so a redraw never flickers and output a running
/// turn wrote underneath is repaired.
pub(super) fn draw_scroll_overlay(
    overlay: &mut ScrollOverlay,
    stdout: &mut std::io::Stdout,
    renderer: &mut PromptRenderer,
    theme: &Theme,
    bottom: ScrollBottom<'_>,
) {
    use crossterm::cursor::MoveTo;
    use crossterm::style::Print;

    let width = PromptRenderer::terminal_width();
    let height = terminal_height();
    let peer_lines = bottom.peer_lines();
    let bottom_rows = scrolled_bottom_rows(
        bottom.prompt,
        bottom.input,
        width,
        suggestion_rows(bottom.suggestion, width),
        peer_lines.len(),
        bottom.run_row().is_some(),
        bottom.marker_row().is_some(),
    );
    let body = crate::scroll::body_rows(height, bottom_rows);
    let marker_rows = usize::from(bottom.marker_row().is_some());
    let rows = body_slice_at(
        width,
        body,
        slice_offset(overlay.view.offset(), marker_rows),
    );
    // A drag made on a different offset describes rows that are no longer
    // there, so it is not drawn over the ones that are.
    let selection = overlay
        .selection
        .filter(|sel| sel.offset == overlay.view.offset());
    for r in 0..body {
        let _ = crossterm::queue!(
            stdout,
            MoveTo(0, r as u16),
            terminal::Clear(ClearType::UntilNewLine)
        );
        if let Some(row) = rows.get(r) {
            let painted = match selection.as_ref().and_then(|sel| {
                let w = crate::editor::str_display_width(&crate::theme::strip_sgr(&row.text));
                crate::select::row_span(sel, r, w)
            }) {
                Some(span) => std::borrow::Cow::Owned(crate::select::highlight(&row.text, span)),
                None => std::borrow::Cow::Borrowed(row.text.as_str()),
            };
            let _ = crossterm::queue!(stdout, Print(painted));
        }
    }
    let _ = crossterm::queue!(stdout, MoveTo(0, body as u16));
    let _ = stdout.flush();

    // The prompt goes through its own renderer, so the cursor arithmetic —
    // wrapping, multibyte widths, the suggestion row — has one implementation.
    renderer.reset();
    // One composition for every state: the prompt goes through its own
    // renderer, so wrapping, multibyte widths and the suggestion row keep one
    // implementation, and the running turn's elapsed row and the scroll
    // marker are printed above it by the same path. Nothing here is recorded:
    // only `finish_line` writes the transcript, so the elapsed row stays
    // display-only.
    renderer.render(
        stdout,
        theme,
        bottom.role,
        bottom.prompt,
        bottom.input,
        bottom.suggestion,
        &peer_lines,
        bottom.run_row(),
        bottom.marker_row(),
    );
    overlay.last_draw = std::time::Instant::now();
}
