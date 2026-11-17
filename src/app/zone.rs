use super::*;

/// The armed zone's same rule: a click lands on the beat's clock block — never
/// on the spinner row itself and never on the span — so the reasoning view
/// toggles from the zone's standing shape exactly as the indicator's block
/// rule allows. Pure against the zone's bookkeeping; no click changes anything
/// without a block tall enough to hold a reasoning row.
pub(super) fn zone_click_hits_clock(row: u16) -> bool {
    let guard = lock_zone();
    let Some(z) = guard.as_ref() else {
        return false;
    };
    if !z.on_screen {
        return false;
    }
    click_hits_clock_block(z.clock_top, z.clock_rows, row)
}

/// The click rule shared with the indicator's rule (`click_hits_
/// indicator`): the reasoning view toggles inside the block's reasoning rows
/// only. `clock_top` is the block's first row on 0-based screen coordinates,
/// `clock_rows` its painted height — `<= 1` is the spinner row alone (or
/// nothing painted), which no click targets. The zone's face order puts the
/// spinner-and-elapsed row LAST in the block, so the exclusion pivots to the
/// last row; the rows above it (the reasoning rows and the marker) toggle.
pub(super) fn click_hits_clock_block(clock_top: u16, clock_rows: u16, row: u16) -> bool {
    // The status row is the block's LAST row (the zone's face order): the
    // hit range excludes it and covers the rows above it — the reasoning
    // rows and the elision marker the toggle serves.
    let bottom = clock_top.saturating_add(clock_rows).saturating_sub(1);
    clock_rows > 1 && row >= clock_top && row < bottom
}

/// The turn zone's anchor: while a turn runs on the live screen, the prompt
/// block owns the screen's bottom rows and stands parked at the user's input
/// cursor; this turn zone re-homes everything the running turn writes to the
/// rows above that block and keeps the block anchored in place until the
/// turn's handoff.
///
/// Write ownership. The armed zone is the live screen's sole writer: the raw
/// writers and the renderer gate their screen writes on it and only record
/// into the transcript — the record is the one truth the zone's beat renders
/// from, so there is no second writer to race (the BL-M33 two-writers race is
/// removed by ownership, not by locking). Once per beat the zone wipes the
/// rows above the input span and repaints them bottom-up at absolute rows:
/// the log tail (the record's own rows, the wrap it was written with) ending
/// at the clock's top, the clock above the peers' band, the peers' band above
/// the parked span — the instruction's top-down order (log, run display,
/// subagent rows, prompt). The typed line never moves; no DECSTBM margin, no
/// SU/SD shifting: the region is re-painted absolutely every beat.
///
/// Caret exactness without cursor queries. The renderer (or the arm) tells the
/// zone where the caret stands after every edit render and how many rows the
/// block spans now; the zone restores exactly that position after every write.
/// Streaming under a caret at the end of the block adds a wrapped row and the
/// renderer reports it as it reports its bookkeeping — no recurring
/// `cursor::position` traffic and no stale-answer hazard.
pub(super) struct TurnZone {
    /// Terminal geometry as of the arm.
    height: u16,
    /// Terminal width; reused by the beat's tail wrap and the span render.
    width: u16,
    /// The input span's top row (0-based): the prompt block — the parked
    /// prompt line and whatever the edit wraps/suggests — occupies the rows
    /// `span_top..=height−1`. It is the beat's floor: every beat erases and
    /// repaints only the rows above it, so the typed line never moves.
    span_top: u16,
    /// Where the editor's caret stands now, as absolute screen coordinates.
    /// The renderer's armed paint places it; every zone write ends parked, so
    /// the caret never wanders while the turn runs.
    caret_col: u16,
    caret_row: u16,
    /// The lowest row the zone has painted (0-based), so far. The beat paints
    /// from the clock's top down to the span's floor, and the band and the
    /// clock move with the beat, so this is the disarm's erase floor: a turn
    /// that ends erases every row the zone ever touched, none it never did.
    painted_top: u16,
    /// The run display's block as the last beat painted it: its top row and
    /// its row count. The thinking-toggle click is judged against this extent
    /// — the live path's cursor-relative rule has no painted rows to judge by
    /// while the zone holds the writes.
    clock_top: u16,
    clock_rows: u16,
    /// Whether the zone's own screen is the one being written: the scroll
    /// overlay's alternate screen hides it, and the screen writes stop while
    /// hidden (the record keeps running; the overlay draws from the record).
    on_screen: bool,
    /// Whether the turn this zone was armed for has reached its terminal event
    /// while the zone was hidden: the disarm then does not erase the other
    /// screen's coordinates, and the handoff (the erase, the tail replay and
    /// the park) is carried out the moment the live screen returns —
    /// `zone_show` seeing this.
    closed: bool,
    /// The session identity the beat composes the subagent rows from, taken
    /// at the arm so the beat's registry reads need no argument passed in.
    session: ZoneSession,
}

/// The zone handle, a process-wide singleton next to the session transcript:
/// only the interactive session arms it, and every writer (the display task's
/// events, the input loop's notices) routes through the same helpers.
pub(super) static TURN_ZONE: std::sync::Mutex<Option<TurnZone>> = std::sync::Mutex::new(None);

/// The session identity the armed zone composes the subagent rows from, taken
/// at the arm: the beat reads the shared registry through these, so a peer's
/// row carries its operating time and its processing content (read from the
/// registry) fresh at every beat without the arm passing rows in.
#[derive(Clone)]
pub(super) struct ZoneSession {
    registry_dir: std::path::PathBuf,
    self_id: crate::id::AgentId,
    theme: Theme,
}

/// Erase the row at an absolute 0-based position: a CUP to the row's column 0
/// followed by an entire-row erase, so a beat's repaint never inherits a
/// longer row's tail from what streamed there a beat ago.
pub(super) fn zone_csi_move(col: u16, row: u16) -> String {
    format!("\x1b[{};{}H", row.saturating_add(1), col.saturating_add(1))
}

pub(super) fn zone_csi_clear_row(row: u16) -> String {
    // The crossterm row-first convention: `zone_csi_move(col, row)` emits
    // the row first. Passing `row` as the column emitted a crawl along
    // row 1's columns — the erase never touched the rows it named and
    // kept clearing the banner row instead (the erase's own coordinates).
    format!("{}\x1b[2K", zone_csi_move(0, row))
}

pub(super) fn lock_zone() -> std::sync::MutexGuard<'static, Option<TurnZone>> {
    TURN_ZONE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether the zone holds the live screen's write ownership (the prompt
/// pinned). The writers' gate: `raw_*` and the renderer record-only while
/// this true. Ownership covers the hidden state too — while the overlay's
/// alternate screen shows, the zone holds the lease but paints nothing, and
/// a turn's in-flight writes must keep landing in the record alone (the
/// overlay draws from that same record), so the gate does not open for the
/// hidden zone.
pub(super) fn zone_armed() -> bool {
    lock_zone().is_some()
}

/// Arm the zone for a Pending turn: the zone takes the live screen's write
/// ownership with the span anchored on the screen's last row (the bare
/// parked prompt — the caller's span render re-anchors with the editor's
/// real extent and reports the caret). Every armed writer stops its screen
/// writes immediately, and the beat owns the rows above the span from the
/// next tick; the first beat composes the standing shape on request.
pub(super) fn zone_arm(state: &Arc<ReplState>) {
    let height = terminal_height() as u16;
    if height < 2 {
        // No screen worth the name: degrade to the unarmed flow. The prompt
        // block cannot be kept apart from the clock there either, so the
        // rows go back to the cursor-relative block rather than standing
        // unwritten.
        lock_indicator(&state.indicator).set_prompt_owned(false);
        return;
    }
    let session = ZoneSession {
        registry_dir: state.registry_dir.clone(),
        self_id: state.id.clone(),
        theme: state.theme,
    };
    // The armed zone is the rows' only writer, so the live prompt block no
    // longer owns them: its cursor-relative block stays silent and the beat
    // composes the run display above the span instead. Taken before the
    // zone lock, so the two locks are never held in both orders.
    lock_indicator(&state.indicator).set_prompt_owned(false);
    let mut guard = lock_zone();
    if let Some(z) = guard.as_mut() {
        // Already armed: the anchor the editor reported stands. Re-seeding it
        // would drop the span's extent back to the bare one-row prompt, and the
        // next beat would then compose the clock over the block's other rows —
        // the same defect the zone exists to prevent. Only the session the beat
        // reads and the two visibility flags are refreshed.
        z.session = session;
        z.on_screen = true;
        z.closed = false;
        return;
    }
    *guard = Some(TurnZone {
        height,
        width: PromptRenderer::terminal_width() as u16,
        // The standing shape is the running display alone: the beat's
        // region is every row, the clock block's last row lands on the
        // screen's bottom row, and no prompt span paints while armed — the
        // prompt display returns only when input is possible again, at the
        // handoff. `span_top` keeps its role as the region's floor (=
        // height): the sweep covers the whole bottom-anchored region.
        span_top: height,
        caret_col: 0,
        caret_row: height - 1,
        painted_top: height - 1,
        clock_top: height - 1,
        clock_rows: 0,
        on_screen: true,
        closed: false,
        session,
    });
}

/// The turn-end handoff: erase everything the zone ever painted and replay the
/// record so the log ends above the fresh prompt row. The outcome line the
/// indicator's `finish` recorded at the terminal event stands as the record's
/// newest line; the shipped render below draws the prompt underneath.
pub(super) fn zone_disarm() {
    let mut guard = lock_zone();
    // Hidden (the overlay's alternate screen): the turn ended while the other
    // screen owned the terminal — erasing its own coordinates now would hit
    // the wrong screen, so the zone stands with the debt marked and the
    // handoff runs when the live screen returns (`zone_show`).
    if let Some(z) = guard.as_mut() {
        if !z.on_screen {
            z.closed = true;
            return;
        }
    } else {
        return;
    }
    let Some(z) = guard.take() else {
        return;
    };
    let width = z.width.max(1) as usize;
    let height = z.height;
    let painted_top = z.painted_top;
    // Rows painted_top..=height−2: every transient row the zone could have
    // put on screen, the span's rows included. The record's tail replays
    // ending on that row — the outcome line the indicator's `finish` recorded
    // at the terminal event is the tail's newest row — so the shipped render
    // below draws the prompt on a fresh, empty bottom row, exactly where a
    // `reset` puts it after a newline-terminated print. One queued pass, so
    // the repair paints atomically.
    let plan = zone_disarm_plan(height, painted_top);
    let mut seq = String::new();
    for row in painted_top..=plan.tail_floor {
        seq.push_str(&zone_csi_clear_row(row));
    }
    if plan.budget > 0 {
        if let Some(rows) = crate::scroll::tail_for_region(width, plan.budget) {
            let top = plan.tail_floor + 1 - rows.len() as u16;
            for (i, line) in rows.iter().enumerate().take(plan.budget) {
                seq.push_str(&zone_csi_move(0, top + i as u16));
                seq.push_str("\x1b[2K");
                seq.push_str(line);
            }
        }
    }
    // A fresh row at the screen's bottom: the column-0 origin every shipped
    // write (the raw writers' CR+LF, `clear_block`'s MoveToColumn(0))
    // continues from, so the handoff lands where the shipped flow puts the
    // cursor after the outcome line's newline.
    seq.push_str("\x1b[0m");
    seq.push_str(&zone_csi_move(0, height - 1));
    let out = std::io::stdout();
    let mut out = out.lock();
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
}

/// Park the caret where the renderer last placed it. Every zone write ends
/// here, so the caret never wanders while the turn runs.
pub(super) fn zone_park() {
    let (col, row) = {
        let guard = lock_zone();
        match guard.as_ref() {
            Some(z) => (z.caret_col, z.caret_row),
            None => return,
        }
    };
    let _ = crossterm::queue!(std::io::stdout(), crossterm::cursor::MoveTo(col, row));
    let _ = std::io::stdout().flush();
}

/// The beat (the armed zone's standing shape): erase every row above the input
/// span — the span's own rows are never swept — and repaint the region at
/// absolute rows: the clock block on the rows directly above the span, the
/// peers' band above the block, and the record's tail above the band, ending
/// at the composition's top. The record is the sole truth and the beat is the
/// only painter while armed; it runs from the display task's 100 ms ticker
/// while a turn runs, so no new event type and no new state machine. The
/// beat's composition plan (four regions, read bottom-up from the span): the
/// clock block fills the rows directly above the prompt span, the peers' band
/// sits above the block — the newest just above it — and the log's tail fills
/// what remains above the band. Pure so the pin binds the same arithmetic the
/// beat paints with; `sweep_to` is the erase floor the beat shares (the span's
/// own rows are never swept).
pub(super) struct ZoneBeatPlan {
    /// The clock block's top row (its bottom row is `block_home + block_n − 1`,
    /// the row just above the span).
    pub(super) block_home: u16,
    /// The peers' band's top row (its bottom row is `peers_top + peers_n − 1`,
    /// the row just above the block).
    pub(super) peers_top: u16,
    /// The beat erases rows `0..sweep_to` — everything above the input span.
    pub(super) sweep_to: u16,
}

pub(super) fn zone_beat_plan(span_top: u16, block_n: u16, peers_n: u16) -> ZoneBeatPlan {
    ZoneBeatPlan {
        block_home: span_top.saturating_sub(block_n),
        peers_top: span_top.saturating_sub(block_n).saturating_sub(peers_n),
        sweep_to: span_top,
    }
}

/// The disarm's geometry, pure: the erase starts at `painted_top` (every row
/// the zone ever painted) and the record's tail replays ending on `tail_floor`
/// = height−2 — the row above the bottom row, so the normal render draws the
/// prompt on a fresh bottom row with the outcome line above it. Pure so the pin
/// binds the same arithmetic the disarm paints with.
pub(super) struct DisarmPlan {
    /// The last row the tail replay may paint (`height − 2`).
    pub(super) tail_floor: u16,
    /// The rows the tail replay asks the record for.
    pub(super) budget: usize,
}

pub(super) fn zone_disarm_plan(height: u16, painted_top: u16) -> DisarmPlan {
    let tail_floor = height.saturating_sub(2);
    // The short-terminal degrade, inside the geometry: a terminal too short
    // to hold the fresh bottom row and the row above it has no rows the
    // replay may paint into (the degenerate floor would be the bottom row
    // itself), so the plan replays nothing — `zone_arm` never arms such a
    // terminal anyway.
    let budget = if height < 2 {
        0
    } else {
        (tail_floor.saturating_add(1)).saturating_sub(painted_top) as usize
    };
    DisarmPlan { tail_floor, budget }
}

pub(super) fn zone_beat(block_rows: &[String]) {
    // Compose everything that needs the zone's identity while holding the
    // guard, then drop it before any screen write (beats and the input loop
    // take the same lock; holding it across writes only widens the window).
    let (width, span_top, on_screen, session) = {
        let guard = lock_zone();
        let Some(z) = guard.as_ref() else {
            return;
        };
        (
            z.width.max(1) as usize,
            z.span_top,
            z.on_screen,
            z.session.clone(),
        )
    };
    if !on_screen {
        // The overlay's alternate screen owns the terminal: the bookkeeping
        // stands, the writes wait for the live screen.
        return;
    }
    let block_n = block_rows.len() as u16;
    let peers = live_peer_lines(&session.registry_dir, &session.self_id, width)
        .into_iter()
        .map(|l| session.theme.err(Role::Peer, &l).into_owned())
        .collect::<Vec<_>>();
    let peers_n = peers.len() as u16;
    // The composition reads bottom-up from the span: the clock block (the run
    // display) on the rows above the span, the peers' band above the block —
    // the newest just above it — and the log's tail above the band. Read
    // top-down the four regions stand in this order: log / run display /
    // subagent rows / prompt. The geometry is the pure plan's (the pin below
    // binds the same arithmetic the beat paints).
    let plan = zone_beat_plan(span_top, block_n, peers_n);
    let (block_home, peers_top, sweep_to) = (plan.block_home, plan.peers_top, plan.sweep_to);
    let out = std::io::stdout();
    let mut out = out.lock();
    // Everything above the span belongs to the beat: erase the whole region
    // and repaint it in one queued pass, so no row a previous beat painted —
    // a taller block, a friendlier band — survives a beat that composes less.
    let mut seq = String::new();
    for row in 0..sweep_to {
        seq.push_str(&zone_csi_clear_row(row));
    }
    // Each paint stops at its own floor. The plan clamps the composition's top
    // with `saturating_sub`, but a block or band taller than the rows above the
    // span would still walk every row it holds, downward into the span — over
    // the prompt, its bottom-most row among them. The bounds below keep every
    // row the beat writes above the span: the block never crosses `sweep_to`
    // (the span's own top) and the band never crosses the block's first row.
    // The block's first row is the elapsed row, so a truncated block keeps the
    // elapsed display and drops the reasoning rows beneath it.
    for (i, line) in block_rows.iter().enumerate() {
        let row = block_home.saturating_add(i as u16);
        if row >= sweep_to {
            break;
        }
        seq.push_str(&zone_csi_move(0, row));
        seq.push_str("\x1b[2K");
        seq.push_str(line);
    }
    for (i, line) in peers.iter().enumerate() {
        let row = peers_top.saturating_add(i as u16);
        if row >= block_home.min(sweep_to) {
            break;
        }
        seq.push_str(&zone_csi_move(0, row));
        seq.push_str("\x1b[2K");
        seq.push_str(line);
    }
    // The tail: as much of the record as the region above the composition
    // holds, wrapped to the terminal's width and ending at the composition's
    // top — the log's newest row stands just above the band (or the clock
    // when no peer runs).
    let mut tail_top = peers_top;
    if peers_top > 0 {
        if let Some(rows) = crate::scroll::tail_for_region(width, peers_top as usize) {
            let n = rows.len() as u16;
            tail_top = peers_top - n;
            for (i, line) in rows.iter().enumerate() {
                seq.push_str(&zone_csi_move(0, tail_top + i as u16));
                seq.push_str("\x1b[2K");
                seq.push_str(line);
            }
        }
    }
    // No colour may outlive the beat's last row: every later write re-states
    // its own style, but the park lands on the user's line.
    seq.push_str("\x1b[0m");
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
    drop(out);
    // Bookkeeping: the clock's extent for the click rule, and the topmost row
    // the tail repainted — the disarm's erase and replay begin there, so a
    // shrunk band or block never leaves a survivor above the span.
    {
        let mut guard = lock_zone();
        if let Some(z) = guard.as_mut() {
            z.clock_top = block_home;
            z.clock_rows = block_n;
            z.painted_top = z.painted_top.min(tail_top);
        }
    }
    zone_park();
}

/// The renderer (or the arm) reports where the caret now stands and how many
/// rows the input span takes; the zone re-anchors so the beat's erase floor
/// and the park follow the block the edit grew or shrank. The span can only
/// sit on the screen's last rows; a span that would cross row 0 clamps.
pub(super) fn zone_reanchor(span: u16, caret_col: u16, caret_row: u16) {
    let mut guard = lock_zone();
    let Some(z) = guard.as_mut() else {
        return;
    };
    let span = span.max(1).min(z.height);
    z.span_top = z.height - span;
    z.painted_top = z.painted_top.min(z.span_top);
    z.caret_col = caret_col;
    z.caret_row = caret_row;
}

/// Retire the armed span: the composition returns to the spanless standing
/// shape — the beat's sweep reclaims the span's rows and the clock block
/// takes the screen's bottom row back. Called when the approval prompt is
/// answered while the turn continues running; a no-op when no zone is armed.
pub(super) fn zone_span_retire() {
    let mut guard = lock_zone();
    if let Some(z) = guard.as_mut() {
        z.span_top = z.height;
        z.caret_col = 0;
        z.caret_row = z.height - 1;
    }
}

/// The zone's screen is hidden (the scroll overlay's alternate screen owns
/// the terminal): the bookkeeping stands, the screen writes stop and the
/// writers' gate lifts so the overlay's draws come from the record directly.
pub(super) fn zone_hide() {
    if let Some(z) = lock_zone().as_mut() {
        z.on_screen = false;
    }
}

/// The zone's screen is live again: the writes resume, the beat repaints from
/// the record on its next tick, and the caret is parked. When the turn ended
/// while the screen was hidden (`closed`), this is the deferred disarm — the
/// handoff runs here, now that the screen's own coordinates are writable.
pub(super) fn zone_show() {
    let mut guard = lock_zone();
    match guard.as_mut() {
        Some(z) if z.closed => {
            z.closed = false;
            // The deferred handoff: run the disarm on the live screen, then the
            // shipped render below (the input loop's render_prompt) draws the
            // prompt on top of the replayed tail.
            z.on_screen = true;
            drop(guard);
            zone_disarm();
        }
        Some(z) => {
            z.on_screen = true;
            let (col, row) = (z.caret_col, z.caret_row);
            drop(guard);
            let _ = crossterm::queue!(std::io::stdout(), crossterm::cursor::MoveTo(col, row));
            let _ = std::io::stdout().flush();
        }
        None => {}
    }
}

pub(super) fn zone_paint_span(
    theme: &Theme,
    prompt_role: Role,
    prompt: &str,
    input_state: &InputState,
    suggestion: Option<&str>,
) {
    let Some((width, height)) = (|| {
        let guard = lock_zone();
        let z = guard.as_ref()?;
        Some((z.width.max(1) as usize, z.height))
    })() else {
        return;
    };
    // The span's rows: the hint row when there is one, then the prompt's wrap
    // extent — the same arithmetic the renderer's budget uses.
    let sug_rows = suggestion_rows(suggestion, width) as usize;
    let prompt_cols = crate::editor::str_display_width(prompt);
    let total_cols = prompt_cols + input_state.display_width();
    let (end_row, cursor_row, cursor_col) =
        layout_cursor(prompt_cols, input_state.display_cursor(), total_cols, width);
    let bottom_rows = end_row.max(cursor_row) + 1;
    // The span is the screen's bottom rows; a wrap extent taller than the
    // screen clamps (the anchor clamps with it).
    let span_n = sug_rows + bottom_rows;
    let span_n = span_n.min(height.max(1) as usize).max(1);
    // Grow the anchor (a span taller than the arm's initial one row) before the
    // paint: the beat's erase floor must not sit inside the span. The caret row
    // is the span's bottom row offset by the cursor's position within it.
    // The caret rides the block's bottom row, offset by the cursor's position
    // within it; the anchor reports the same absolute row the paint below
    // parks on (both are height−1−offset: the block's last row pins to the
    // screen's bottom row under any clamp).
    let bottom_offset = end_row.max(cursor_row) - cursor_row;
    zone_reanchor(
        span_n as u16,
        cursor_col as u16,
        (height as usize - 1 - bottom_offset.min(span_n - 1)) as u16,
    );
    // The hint row rides the span's top; the prompt's wrap rows follow, each
    // wrapped by the transcript's own SGR-aware wrapper so a styled prompt and
    // a multibyte line break exactly as the renderer would.
    let mut rows: Vec<String> = Vec::with_capacity(span_n);
    if sug_rows == 1 {
        let sug = truncate_suggestion(suggestion.unwrap_or(""), width - 1);
        rows.push(theme.out(Role::Hint, &sug).into_owned());
    }
    let plain = format!(
        "{}{}",
        theme.out(prompt_role, prompt),
        theme.out(Role::InputText, &input_state.line)
    );
    // The rows paint at absolute rows, the block's last row pinning to the
    // screen's bottom row; a shorter wrapped content leaves the rows above it
    // to the beat's sweep. An exactly-full last row leaves the cursor on a
    // pending fresh row the wrapper does not emit — the erase already cleared
    // it, so nothing is painted there.
    for line in crate::scroll::wrap_styled(&plain, width) {
        rows.push(line);
    }
    // The rows paint at absolute rows: the anchor grew above, so the span's
    // last row is the screen's bottom row and the wrapped content fills the
    // span downward from its top.
    let base = (height - span_n as u16) as usize;
    let out = std::io::stdout();
    let mut out = out.lock();
    let mut seq = String::new();
    for (i, line) in rows.iter().enumerate() {
        seq.push_str(&zone_csi_move(0, (base + i) as u16));
        seq.push_str("\x1b[2K");
        seq.push_str(line);
    }
    // The caret parks on the same absolute row the anchor reported — one
    // truth for the span's edit position (the clamp matches `zone_reanchor`).
    let caret_row = height - 1 - (bottom_offset.min(span_n - 1)) as u16;
    seq.push_str("\x1b[0m");
    seq.push_str(&zone_csi_move(cursor_col as u16, caret_row));
    let _ = out.write_all(seq.as_bytes());
    let _ = out.flush();
}

/// Rows the prompt block occupies on screen: the optional suggestion row above
/// it, plus the rows the prompt and the edited line wrap onto. Pure, so the
/// scrolled screen's row budget can be unit-tested without a terminal.
pub(super) fn prompt_block_rows(
    prompt_cols: usize,
    cursor_cols: usize,
    total_cols: usize,
    sug_rows: u16,
    width: usize,
) -> u16 {
    let (end_row, cursor_row, _) = layout_cursor(prompt_cols, cursor_cols, total_cols, width);
    sug_rows + end_row.max(cursor_row) as u16 + 1
}
