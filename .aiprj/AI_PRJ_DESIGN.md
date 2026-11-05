# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the display corrections — during mouse scrolling the `Message below`
marker stands above the prompt, not below it; and during a running turn a
mouse selection no longer makes the elapsed display vanish nor drop the
selection one row" (FR-39, FR-40)
(`.aiprj/instructions.md`, rewritten 2026-11-05 09:31 (+0900))
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What is decided, and what it costs

The instruction was rewritten at 2026-11-05 09:31 (+0900) and names two
**display corrections** to the shape the previous cycle delivered. The
previous cycle's decisions stand untouched: D-52–D-54 (the running turn's
composition, carried as FR-37a's mechanism), D-56 (the four-region
composition as the standing shape), D-58 (the peer rows' content over the
measured registry transport) and D-59 (the bare-`Enter` guard). What this
cycle decides is **where two rows stand** when the display the previous
cycle built is exercised by the mouse:

- **D-60 — the scroll marker takes the row above the prompt block
  (delivered).** The marker's text is produced only on the scrolled
  screen's prompt pin — the `text` field of the one `ScrollBottom` struct
  (`src/app.rs:2050–2062`, the accessor `marker_row` `:2073–2075`), set
  from `overlay.view_active()` in the prompt-pinning draw (`:5347–5350`
  over `SCROLLED_MARK` `:1372`). The shipped shape painted it *after* the
  block's last row — `k = (end_row.max(cursor_row) − cursor_row) + 1`,
  `MoveDown(k)` — which put it on the row **below** the block. The
  correction is the row's **place**, not the row's count: the budget
  already reserved it (`scrolled_marker_rows` `:1378–1380`). The marker
  now paints on the row immediately above the prompt block's first row —
  the renderer's marker arm before the block's rows (`render`
  `:1156–1260`, the running row `:1189–1199`, the marker arm
  `:1201–1213`, the fold `:1258`), with `above_rows` counting it so the
  tracked region's top owns it and the erase reaches it (`clear_block`
  `:1078–1102`). The three arithmetic sites agree on the place through
  one pure plan: the drawer's budget (`scrolled_bottom_rows`
  `:2144–2162`), the wheel arm's body (`:3249–3257`) and the drag arm's
  body (`:3326–3335`) all call it, and `scrolled_block_plan`
  (`:2115–2131`) answers the marker's row, the prompt span's top and the
  block's whole footprint (AS-59). No new field, no new flag, no new
  state machine — `ScrollBottom` lost its enum and its second variant
  instead of gaining one.

- **D-61 — during a run the scrolled screen composes the live screen's
  bottom rows, and the mouse arms measure the composition that is drawn
  (delivered).** Two statements the previous cycle shipped are what the
  second ask found broken. First, the pin choice: while a turn runs, the
  shipped `draw_scrolled` asked `overlay_status_row()` and, when it
  answered, drew a **clock-only** variant — the elapsed row alone at the
  screen's bottom row, with no prompt span and the marker swapped into
  its row. The live screen's mid-turn composition is the opposite order:
  the prompt span owns the bottom rows and the clock block stands
  directly above it, the peers above that (`zone_beat_plan`
  `:1628–1636`, the beat `:1664–1756`). Opening the scrolled screen
  mid-turn therefore re-stacked the bottom rows — the elapsed row moved
  from the row above the prompt to the screen's last row, and the prompt
  span the user was typing into left the display. That is the
  instruction's "the elapsed row disappears" and its "the selection
  drops one row" in one statement: the composition changed under the
  pointer. Second, the mouse arms' budget: the wheel arm (`:3228–3262`,
  its body `:3249–3257`) and the drag arm (`:3312–3398`, its body
  `:3326–3335`) both derive `body` from a `Pending` branch that counted
  the clock's row alone, and both gated `view_moved` on the turn's
  absence — so during a turn the marker's row was never counted at all.
  The corrected decision, as delivered, is that the scrolled screen
  mid-turn **composes as the live screen does** — the prompt span at the
  bottom-most rows, the elapsed row above it, the marker of D-60 above
  that, the peer rows above that — and that the wheel and drag arms
  measure that same composition, so the row a press, a drag and a release
  resolve against is the row the screen shows (AS-62). One struct and one
  arithmetic carry it: `ScrollBottom` (`:2050–2062`) is a single struct
  with `run_row` beside `role`/`prompt`/`input`/`suggestion`/`peer_lines`/
  `text`, `draw_scrolled` (`:5379–5412`) hands it the row
  `overlay_status_row()` answers (`:4779–4786`), and
  `scrolled_bottom_rows` (`:2144–2162`) is the one budget the drawer
  (`:2295–2303`), the wheel arm (`:3250–3256`) and the drag arm
  (`:3327–3333`) all ask. Both arms now pass `view_moved =
  overlay.view_active()` with no turn gate (`:3249`, `:3326`). The
  transcript's recording rules are untouched: the beat rows stay out of
  the log, and the elapsed row is still display-only —
  `renderer.render` writes rows, `finish_line` (`:1108–1139`) is the
  recorder — bound green by
  `elapsed_clock_survives_scrolling_during_a_running_turn`
  (`:7735–7772`). No new event type; the zone keeps the live screen
  (AS-53 stands).

The two decisions are separable in the tree but not in the display: D-60
places the marker inside the composition D-61 fixes, so the marker's row
is counted once, in one place, by both.

## 2. The touchpoints this cycle touches (TPR-23)

| # | Touchpoint | Evidence (the tree at `fb46cc0`) |
|---|---|---|
| TPR-23a | The scroll marker's place on the prompt pin — the renderer's marker arm, the text's production site, the row's budget and the three arithmetic sites that must agree | `src/app.rs` (`ScrollBottom.text` `:2050–2062`, `marker_row` `:2073–2075`; the renderer `:1156–1260`, the marker arm `:1201–1213`, the fold `:1258`, `clear_block` `:1078–1102`; the producer `:5347–5350` over `SCROLLED_MARK` `:1372`; `scrolled_marker_rows` `:1378–1380`; the pure plan `scrolled_block_plan` `:2115–2131`; the shared budget `scrolled_bottom_rows` `:2144–2162`; the drawer's call `:2295–2303`; the wheel arm's call `:3249–3257`; the drag arm's call `:3326–3335`) |
| TPR-23b | The running turn's scrolled composition and the mouse arms' budget — the pin choice, the elapsed row's transport, the live four-region order, and the two arms that derive the body | `src/app.rs` (`draw_scrolled` `:5379–5412`, the row `:5392–5395`; the single `ScrollBottom` `:2050–2062` with `run_row` `:2080–2082`; `run_row_rows` `:2088–2090`; `scrolled_block_plan` `:2104–2131`; `scrolled_bottom_rows` `:2144–2162`; the drawer `:2282–2352`; `zone_beat_plan` `:1628–1636`, `zone_beat` `:1664–1756`; the wheel arm `:3228–3262`, its body `:3249–3257`; the drag arm `:3312–3398`, its body `:3326–3335`, the press `:3339`, the clamp `:3353`, the release's slice `:3373–3375`; `overlay_status_row` `:4779–4786`) |

## 3. Analysis verdicts (C-12) — the investigation stage

- **The marker-place angle.** The marker is produced at exactly one site
  (`render_prompt_with_run`'s `overlay.view_active().then(…)`,
  `:5347–5350` over `SCROLLED_MARK` `:1372`) and consumed at exactly one
  place (`draw_scroll_overlay`'s one arm, `:2334–2350`, passing
  `bottom.marker_row()` `:2349` into `renderer.render`). The shipped
  renderer arm computed `k` from the caret's own row and moved **down**, so
  the row was *always* one below the block's last row. The delivered arm
  inverts the direction: the marker paints **before** the block's rows
  (`:1201–1213`), the running row before it (`:1189–1199`), both counted in
  `above_rows` (`:1194`, `:1198`, `:1212`), and the fold `:1258` folds them
  into `cursor_row` — so the tracked region's top owns the marker's row and
  `clear_block` (`:1078–1102`) climbs past it. The budget is unchanged
  (`scrolled_marker_rows` `:1378–1380` answers `1` off the bottom and `0`
  at it), so the drawer's rows
  (`scrolled_bottom_rows` `:2144–2162`), the wheel's body (`:3250–3256`)
  and the drag's body (`:3327–3333`) needed no new arithmetic — only the
  place moved, and the place is now read from one plan
  (`scrolled_block_plan` `:2115–2131`: `marker_row` / `prompt_top` /
  `rows`).
- **The composition-agreement angle.** The live screen's mid-turn order
  is decided in one pure function (`zone_beat_plan`, `:1628–1636`:
  `block_home = span_top − block_n`, `peers_top = block_home − peers_n`)
  and painted by one writer (`zone_beat`, `:1664–1756`). The scrolled
  screen's mid-turn order was decided in another (the shipped
  `draw_scrolled`'s pin choice) and painted by another (the shipped
  `Status` arm). The two disagreed in both order and membership: the live
  screen had the span at the bottom, the scrolled screen had the clock at
  the bottom and no span; the live screen counted the span's rows in its
  budget, the scrolled screen's mouse arms counted one row. A pointer
  coordinate therefore meant different content rows in the two spaces, and
  opening the scrolled screen re-mapped it. The delivered correction makes
  the scrolled screen's mid-turn pin the **prompt pin carrying the running
  turn's elapsed row**: `draw_scrolled` (`:5379–5412`) takes
  `overlay_status_row()` (`:4779–4786`) and hands it as
  `render_prompt_with_run`'s `run_row`, so both screens compose the same
  four regions in the same order and the arms' `body` shrinks by the
  composition's true height.
- **The mouse-arms angle.** Both arms derive `body` from
  `bottom_block_rows` (`:3250–3256` wheel, `:3327–3333` drag), which
  delegates to the one `scrolled_bottom_rows` (`:2170–2191`). The shipped
  arms gated `view_moved` on the turn's absence
  (`overlay.view_active() && !prompt_state.is_pending()`), so during a turn
  the marker's row was never counted and the `Pending` branch answered the
  clock's row alone. The delivered arms pass `view_moved =
  overlay.view_active()` with no turn gate (`:3249`, `:3326`), and the
  `Pending` branch now counts the prompt span's rows, the elapsed row and
  the marker. The drag arm then clamps a drag and resolves a release
  against that body (`row < body` at `:3339`, `row.min(body − 1)` at
  `:3353`, the release's `body_slice_at` at `:3374`). One budget, shared by
  the drawer and both arms, is the only shape that cannot drift
  (`body_rows` `src/scroll.rs:255`, the shipped single-source pattern) —
  and it is what the delivered `scrolled_bottom_rows` is.
- **The recording angle.** The elapsed row is display-only and never
  recorded, bound by
  `elapsed_clock_survives_scrolling_during_a_running_turn`
  (`:7735–7772`). The pin change does not touch that: the prompt pin's
  renderer path writes rows only (`renderer.render` `:1156–1260`;
  `finish_line` `:1108–1139` is the recorder), and the delivered comment at
  the drawer states it (`:2334–2339`). It was re-verified green rather than
  assumed (W-62).
- **The superseded-pins angle.** Five pins bound the geometry the two
  corrections changed. Each was re-derived failure-first and is green in
  the delivered tree: `the_scrolled_prompt_pin_carries_the_marker_row`
  (`:7539–7607`, now the above-the-block place and the fold),
  `the_scrolled_screen_swaps_the_clock_for_the_marker` → renamed
  `the_scrolled_screen_keeps_the_elapsed_row_and_adds_the_marker`
  (`:7610–7656`, the shipped single-row swap replaced by the three-row
  composition), `the_clock_row_is_not_the_subagent_lines_price`
  (`:7392–7405`, the `Pending` budget now `2` and `4`),
  `the_line_survives_the_intersections_and_the_screen_shows_it`
  (`:7408–7423`, `bottom = 2` and `bottom + peers`), and
  `wheel_range_ends_above_the_clock_row` (`:7775–7798`, the wheel pin's
  `bottom == 2`). The old text was the fold check that the correction
  actually happened (NFR-13).

## 4. Verification of the cycle (the gate table W-60–W-62)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-60 | FR-39 — the marker above the prompt block: the renderer's marker arm places it on the row above the block's first row, `cursor_row`/`clear_block` account for it, the drawer's rows and the wheel/drag budgets agree, and the marker is absent at the log's bottom | FR-39, NFR-13, NFR-14 | TPR-23a | ✅ |
| W-61 | FR-40 — the running turn's scrolled composition: the mid-turn pin composes the prompt span at the bottom rows, the elapsed row above it and the peers above that; the press, drag and release resolve against the rows the screen shows; the elapsed row stays displayed and in place; the transcript's recording rules hold | FR-40, NFR-13, NFR-14 | TPR-23b | ✅ |
| W-62 | The pins and the gates — the five superseded-geometry pins re-derived failure-first and green, the new pins (the marker's place, the mid-turn composition's plan, the arms' body agreement, the no-record rule) green, unit + integration 0 failed, clippy identical to the baseline, the `.aiprj` documents mutually consistent, the work logs written, the delivery committed per the user's direction | FR-39, FR-40, NFR-13, C-08, C-09, NFR-12, AC-34 | TPR-23a/23b | ✅ |

*The rows stand ✅ at the delivered tip — the same gates the recording
session re-executed against the untouched tree before the execution began
(**645 unit + 6 integration, 0 failed**, clippy the same four warning
classes on the same sites), re-run after the corrections landed and
unchanged: the baseline held exactly, and the pins bound the corrected
geometry rather than the superseded one (NFR-13).*

## 5. The previous cycles (recorded after their documents were retired)

**"the terminal's display regions — from the top, the scrollable and
mouse-selection-copyable log, the spinner-and-elapsed run display, the
running subagents' situation (the operating time and the processing
content), the bottom-most line reserved for the prompt — and a bare
`Enter` ignored" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05: delivered at
commit `39afeb2` ("During a turn the terminal composes the four display
regions and a bare Enter is ignored"), its sha named by the tracked
ledger's follow-up `fb46cc0`, the ledger re-certified at that tip
(BL-M36–BL-M42). The mechanism stands and this cycle's corrections are
measured against it: the transcript-backed turn zone as the live screen's
sole writer during a run (`static TURN_ZONE` `src/app.rs:1458`, the struct
`:1414`, the arm `:1503`, the beat `:1664–1756` over the pure
`zone_beat_plan` `:1628–1636`, the span paint `zone_paint_span` `:1810`,
the re-anchor `:1762`, the park `:1592`, the hide/show `:1777`/`:1787`,
the handoff `zone_disarm` `:1536` over `zone_disarm_plan` `:1650–1661`);
the peer rows' processing content over the measured registry transport
(`RegistryEntry.turn`, `src/ipc/registry.rs:45`, the writes `:215`/`:231`,
the reader `live_children_states` `:2221–2243`, the face
`peer_status_rows` `:2201–2215`); and the bare-`Enter` guard at the
editor's arm (`:2620–2632`, the empty check `:2627–2629`). Its decisions
D-56/D-58/D-59 are the ground D-60/D-61 stand on; its superseded display
statements (the marker's beneath-the-block place, the clock-only mid-turn
pin) are the two this cycle corrects, recorded as specification changes in
the requirements' §6 (NFR-13). The cycle published no release of its own;
the version was afterwards carried to **0.40.0** by the release commit
`505097a` "Release 0.40.0: the display corrections carry the version"
(the requirements' §12/§13).

**FR-36's display ask** — superseded by FR-37a's record and delivered with
it; the FR-36 cycle never published a version or a tag. Its worktree block
(`src/app.rs:1328–1714` at `5257729`) was carried to delivery as T-76's
transcript-backed turn zone (AS-58, the previous cycle's BL-M39).

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35, a/b/c)** — closed,
2026-11-04: delivered at commit `c02862d` ("Prompts typed while a turn
runs are queued and run in order when it ends"), `Cargo.toml` **0.38.0**,
the tag `v0.38.0`, the GitHub release published **2026-11-04T03:52:18Z**;
the gates at the close: **633 unit + 6 integration, 0 failed**, clippy
identical to the baseline. The mechanism stands in the tree and survives
this cycle untouched (the boundary §6 of the requirements): the `Pending`
branch's acceptor feeding the shared `InputState` in both REPL loops,
`Enter` enqueueing `AgentInput::UserPrompt` onto the shipped channel, the
helpers `queued_count`/`queue_release_allowed`/`paint_queue_status` (the
one-line `[queued: N]` face, `src/app.rs:1312`/`:1327`/`:1337`), the drain
as shipped, the approval-draft snapshot
(`DraftSnapshot`/`snapshot_draft`/`restore_draft`, `src/app.rs:2592–2605`),
the pins rewritten. The full cycle history (the post-close ledger events
`881eda9` → v0.39.0 `4f7fad6` → `96a576f` → `3af66d4` → `5336a03` →
`5257729` → `c2f78b3` → `39afeb2` → `fb46cc0` → `db68d3f` → `306f425` →
`85f9ce9` → v0.40.0 `505097a`; the BL-M30/BL-M31/BL-M32 corrections)
stands in the requirements document's §12 and the task list's history
section — one shared record, not duplicated here (C-08).

**"the subagents' auto-exit — permanent (FR-32), configurable and
default-on (FR-33); the 'prompt too long' repair (FR-34)"** — closed,
2026-11-03: FR-32 (commit `7fe1370`, the version 0.35.0 never tagged),
FR-33 (commit `af48be5`, version 0.36.0, equally never tagged), FR-34
(commit `ddeaf67`, released as **v0.37.0** — the release commit
`5a2d7f0`; the tag `v0.36.0` on `ddeaf67` stands restored — BL-M32, with
BL-M31 narrowed). Gates at the close: **629 unit + 6 integration,
0 failed**; clippy at the baseline.

**"the multi-angle subagent analysis, added to the configuration and
verified executable" (FR-29/FR-30)** — closed, 2026-11-03 (no release):
`spawn` in the live `[tools] enabled` (the shipped default omits it),
the shipped `[spawn]` bounds; live round trips grounding the verdict;
R-1 carried, R-2 withdrawn as false.

**"the click rule's exclusion of the elapsed display; the
cancelled-by-user display" (FR-27/FR-28)** — closed, **v0.34.0** (commit
`c022909`, the tag's final target `745dbb2`): the click rule's strict
top excluding the elapsed row, the query-error fallback disarmed; the
three cancel prints `[info] cancelled by user` styled `Role::Info`.
While the zone is armed the hit is judged against the zone's own clock
extent (`zone_click_hits_clock`, `src/app.rs:1294`, over the pure
`click_hits_clock_block` `:1311`), the shipped pump-query path standing
when it is not. D-61's composition keeps the block where the click rule
expects it — the beat's own `clock_top`/`clock_rows` bookkeeping
(`:1438–1439`, set at `:1747–1754`) is unchanged.

**"the scroll marker's face — 「Message below」…" (FR-26)** — closed,
v0.33.0 (`6cc6661`). Its face and its `view_active` gate stand; its
**place** on the prompt pin is FR-39's correction (D-60). **"the agent's
running display, the subagent status display, and the prompt's resumption
after a cancel" (FR-22–FR-24)** — closed, v0.32.0 (`e8ad594` + `e0ecb1b`;
the clock as the screen's bottom row then — the geometry FR-37a
superseded, and the scrolled screen's clock-only pin D-61 now brings into
line with the live composition). **"the mouse defects…"** — v0.31.0
(`052e230`), its drag/release-offset rule
(`drag_release_resolves_at_the_release_time_offset`,
`src/app.rs:7096–7123`) standing and re-bound by D-61. **"add strata as a connection target…"** — v0.30.0–0.30.2
(`b706889`, `70e6b5b`, `a766757`). **"a received peer prompt sometimes
never starts processing…"** — closed, v0.29.0 (`045d569`). **"…left
unprocessed"** — v0.28.0 (`d237a7a`). **"…never progresses"** —
v0.27.0 (`afcbe70`). **"mouse input turns into escape sequences, and
typing freezes"** — v0.25.0 (`3884d16`) with the three 機能追加 addenda
closed as v0.26.0 (`251a5bb`).
