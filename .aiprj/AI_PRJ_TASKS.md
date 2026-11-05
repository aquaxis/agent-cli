# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the display corrections — during mouse scrolling the `Message below`
marker stands above the prompt, not below it; and during a running turn a
mouse selection no longer makes the elapsed display vanish nor drop the
selection one row" (FR-39, FR-40) (`.aiprj/instructions.md`, rewritten
2026-11-05 09:31 (+0900))
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked
Article 3 of the AI Coding Guidelines: record progress updates in this
file whenever a task changes state.

---

## Overview

The instruction (rewritten 2026-11-05 09:31:00 +0900, re-verified
byte-exact at this update; 595 bytes, sha256
`eca9d4fc943914aeaab42fd7052660d76ee14b51f92c6559b1f9239d675e8988`)
replaces the 2026-11-04 17:08 text the previous cycle's ledger recorded.
Its method sentence and its heading stand; its two asks are new: **the
display corrections**. The previous cycle (FR-37 a/b/c, FR-38 — the four
display regions and the bare-`Enter` guard) is delivered at `39afeb2`,
its sha named by the tracked ledger's follow-up `fb46cc0`, and stands:
nothing it asked for is withdrawn (BL-M42). The two corrections are
recorded as **FR-39** and **FR-40** with their baselines verified in the
tree (BL-M40, BL-M41), the design decisions **D-60**/**D-61** and the
tasks **T-82–T-85** below.

**Both corrections are delivered and pinned.** T-82–T-85 stand ✅ in the
task list and their execution record stands below it (the cycle's record).
The version stays 0.39.0; this cycle publishes no release. The tree the
cycle started from was `fb46cc0` (the `main` tip, the previous cycle's
ledger follow-up), the worktree carrying the ledger's recording alone; the
execution then carried the product tree with it. The gates are green at
the delivered tip: **645 unit + 6 integration, 0 failed** (the baseline
held exactly), clippy the same four warning classes on the same sites —
all outside the cycle's diff.

The two asks' baselines, as the tree stood before the execution:

- **The scroll marker was painted beneath the prompt block** (BL-M40, now
  superseded by D-60): the text rode the prompt pin's `text` field, set
  from `overlay.view_active()`, and the renderer's marker arm painted it
  after the block's last row — `k = (end_row.max(cursor_row) −
  cursor_row) + 1`, `MoveDown(k)` — so the row always landed one below the
  block. The row's **budget** was already reserved above that arithmetic
  (`scrolled_marker_rows` `:1378–1380`: `1` off the bottom, `0` at it), so
  FR-39 was the row's **place**, not its count. The delivered place: the
  row immediately above the block's first row, painted by the renderer's
  marker arm **before** the block's rows (`src/app.rs` `render`
  `:1156–1260`, the marker arm `:1201–1213`, the fold `:1258`), counted in
  `above_rows` so `clear_block` (`:1078–1102`) climbs past it.
- **The scrolled screen's mid-turn pin was the clock alone, and the mouse
  arms measured that clock-only budget** (BL-M41, now superseded by D-61):
  `draw_scrolled` drew a clock-only `ScrollBottom::Status` whenever
  `overlay_status_row()` answered, with a `1 + peer_lines` budget, and
  never the prompt span; the live screen's mid-turn order was the opposite
  (`zone_beat_plan` `:1628–1636`: the span at the bottom, the clock above
  it, the peers above that). The wheel and drag arms both derived `body`
  from `bottom_block_rows`'s `Pending` branch (`view_moved ? 2 : 1`,
  `+ peer_lines`) and both gated `view_moved` on the turn's absence, so
  mid-turn the body was short of a composition carrying the span and the
  marker's row was never counted. Both symptoms the instruction names
  followed from that one disagreement (AS-60). The delivered composition:
  `ScrollBottom` is a single struct with `run_row` beside the prompt's
  fields (`:2050–2062`), `scrolled_block_plan` (`:2115–2131`) answers the
  four-region order, `scrolled_bottom_rows` (`:2144–2162`) is the one
  budget the drawer, the wheel arm (`:3249–3257`) and the drag arm
  (`:3326–3335`) all ask, and both arms pass `view_moved =
  overlay.view_active()` with no turn gate.

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-82 | FR-39's implementation: the scroll marker's place moved from the row beneath the prompt block to the row above its first row — the renderer's marker arm painting before the block's rows, `cursor_row`'s fold and `clear_block`'s reach accounting for the marker's row, the drawer's `bottom_rows` and the wheel/drag budgets agreeing with the place they already agree on the count; the marker absent at the log's bottom as shipped | FR-39, NFR-14 | design §1 (D-60), §2 (TPR-23a), §3 | — | ✅ |
| T-83 | FR-39's pins: failure-first against the shipped beneath-the-block place, then green — the marker's row directly above the block's first row, `cursor_row` including it so the next render erases from the right row, the drawer/wheel/drag arithmetic on one budget, the absent-at-bottom case standing | FR-39, NFR-13 | design §1 (D-60), §4 (W-62) | T-82 | ✅ |
| T-84 | FR-40's implementation: the scrolled screen's mid-turn composition brought into line with the live four-region order — the prompt pin carrying the running turn's elapsed row above it (the clock face from `overlay_status_row`, display-only as shipped), the peer rows above that, the marker of FR-39 between the run display and the prompt; the mouse arms' `Pending` budget grown from the clock's row to the prompt span's rows plus the clock's row plus the additive peer rows, so `draw_scrolled`, the wheel arm and the drag arm share one arithmetic; the transcript's recording rules untouched | FR-40, NFR-14 | design §1 (D-61), §2 (TPR-23b), §3 | T-82 | ✅ |
| T-85 | FR-40's pins: failure-first then green — the mid-turn scrolled composition's plan in the four-region order, the elapsed row displayed and in place, a press/drag/release resolving against the rows the screen shows (no one-row drop), the arms' body equal to the drawer's `bottom_rows`, and the clock row still out of the recorded transcript | FR-40, NFR-13 | design §1 (D-61), §4 (W-62) | T-84 | ✅ |

Sections below the table are the execution record (Article 3) — written as
each task's status changes.

## The cycle's record

- **The instruction was rewritten, and the previous cycle's asks stand** —
  the 2026-11-04 17:08 text (567 bytes, sha256 `2eba1d17…`) is replaced by
  the 2026-11-05 09:31 text (595 bytes, sha256 `eca9d4fc…`). The
  previous cycle's FR-37/FR-38 were delivered at `39afeb2` and re-certified
  at `fb46cc0` (BL-M42); this cycle's two asks are corrections to that
  delivered display, not withdrawals of it (the requirements' §1).
- **Both corrections are geometry statements about rows the display
  already owns** — the marker's place (BL-M40) and the mid-turn scrolled
  composition together with the mouse arms' budget (BL-M41). No key path,
  no recording rule and no wire shape changed (NFR-14; the requirements' §6
  records the two superseded display statements as specification changes).
- **The ledger's recording opened as the ledger's alone** (C-13), carried
  by the work log `2026-11-05_002.md`; the execution then carried the
  product tree with it, carried by `2026-11-05_003.md`. Both corrections
  are delivered and pinned, and the gates are green (§ the gates below).

## T-82 — FR-39's implementation (✅)

Delivered. `ScrollBottom` (`src/app.rs:2050–2062`) carries the marker's
text in its `text` field beside the prompt's own, and the renderer prints
it on the row immediately above the block's first row: the marker arm
(`render` `:1156–1260`, the arm `:1201–1213`) now runs **before** the
prompt span's rows — it prints the row and a CR+LF and adds one to
`above_rows` — and the fold `self.cursor_row = above_rows + sug_rows +
cursor_row` (`:1258`) counts it, so the next render's `clear_block`
(`:1078–1102`) climbs past it and leaves no stale row behind. The old
below-the-block arithmetic is gone: the renderer no longer computes
`k = (end_row.max(cursor_row) − cursor_row) + 1` and `MoveDown(k)` to land
the row under the block's last row. Nothing about the row's **count**
moved — `scrolled_marker_rows` (`:1378–1380`) still answers `1` while the
view is off the bottom and `0` at it, and the drawer and the mouse arms
all take that answer through one budget (`scrolled_block_plan`
`:2115–2131`, `scrolled_bottom_rows` `:2144–2162`, `bottom_block_rows`
`:2170–2191`). The marker is produced where it was —
`render_prompt_with_run`'s `overlay.view_active().then(||
SCROLLED_MARK.to_string())` (`:5348–5350`, the text at `:1372`) — and
consumed once, at `draw_scroll_overlay` handing `bottom.marker_row()` to
`renderer.render` (`:2340`ff). AS-59 holds: a wrapped prompt still keeps
the marker directly above the block's first row, because the marker is
printed ahead of the span's first row rather than placed relative to the
caret.

Depends on: nothing (the first task of the cycle).

Design trace: design §1 (D-60), §2 (TPR-23a), §3 (the marker-place and the
superseded-pins angles), §4 (W-60).

## T-83 — FR-39's pins (✅, depends on T-82)

Delivered, green. The place pin
`the_scrolled_prompt_pin_carries_the_marker_row` (`:7539–7607`) was
re-derived from the beneath-the-block place it stated: it now composes the
block through `scrolled_block_plan`, binds the marker's row as the prompt
span's first row minus one, binds `cursor_row` after the render as
including it — so the next `clear_block` climbs past it — and binds the
same place with a wrapped prompt. The absent case rides the same pin: at
the log's bottom `scrolled_marker_rows(false)` answers `0`, so no row is
produced or painted. The agreement the budgets owe is pinned by
`the_clock_row_is_not_the_subagent_lines_price` (`:7392–7405`) and
`the_line_survives_the_intersections_and_the_screen_shows_it`
(`:7408–7423`), which ask `bottom_block_rows` — the function the drawer,
the wheel arm (`:3249–3257`) and the drag arm (`:3326–3335`) all call, so
their `body` answers the same number as the drawer's `bottom_rows` with
the marker present and absent. NFR-13's fold check: the same pin name
carried the old text, so the rewrite of its assertions is the record that
the correction happened, first made to fail against the shipped place.

Depends on: T-82.

Design trace: design §1 (D-60), §3 (the superseded-pins angle), §4 (W-62).

## T-84 — FR-40's implementation (✅, depends on T-82)

Delivered. The scrolled screen no longer swaps in a clock-only pin:
`draw_scrolled` (`:5379–5412`) takes the running turn's face from
`overlay_status_row` (`:4779–4786`) and hands it to
`render_prompt_with_run` (`:5303–5374`) as `run_row`, so the one render
path prints it on the row directly above the prompt span — the live
screen's order (`zone_beat_plan` `:1628–1636`: the span at the bottom,
the clock above it, the peers above that), with the marker of FR-39
between the run display and the prompt (`scrolled_block_plan`
`:2115–2131`). The enum that used to select between a prompt pin and a
clock-only one is gone: `ScrollBottom` (`:2050–2062`) is a single struct
whose fields are the whole composition, so every state — a turn running
or not — draws through one path and the scrolled screen composes the four
regions in the instruction's order.

The arms' budget follows. `bottom_block_rows` (`:2170–2191`) delegates to
`scrolled_bottom_rows` (`:2144–2162`) with `prompt_state.is_pending()` as
`running`, so the pending budget is the prompt span's rows plus the
elapsed row plus the additive peer rows — the shape the `Ready` branch
computes plus the run block — and the wheel arm (`:3249–3257`) and the
drag arm (`:3326–3335`) both pass `view_moved = overlay.view_active()`
with no turn gate. The press's `row < body` (`:3339`), the drag's
`row.min(body − 1)` (`:3353`) and the release's
`body_slice_at(width, body, offset)` (`:3373–3374`) therefore resolve
against exactly the rows the screen shows (AS-62): the selection lands
where the pointer is and the elapsed row keeps its place.

The recording rule is untouched: `renderer.render` writes rows only, and
`finish_line` (`:1108–1139`) remains the sole recorder, so the elapsed row
and the marker never enter the transcript.

Depends on: T-82 (the marker's place is part of the composition T-84
composes).

Design trace: design §1 (D-61), §2 (TPR-23b), §3 (the composition-agreement,
mouse-arms and recording angles), §4 (W-61).

## T-85 — FR-40's pins (✅, depends on T-84)

Delivered, green. The composition pin
`the_prompt_span_holds_the_bottom_rows_with_subagents_pinned`
(`:7466–7498`) reads the order off `scrolled_block_plan` — the peer rows on
top, the elapsed row, the marker, then the prompt span at the bottom rows.
The elapsed-row pin
`the_scrolled_screen_keeps_the_elapsed_row_and_adds_the_marker`
(`:7610–7656`) was re-derived from
`the_scrolled_screen_swaps_the_clock_for_the_marker` and binds the running
display staying on screen with the marker added above it;
`elapsed_clock_survives_scrolling_during_a_running_turn`
(`:7735–7772`) binds the same row while the gesture is live — and binds it
out of the recorded transcript. The no-drop pin is the shipped
`drag_release_resolves_at_the_release_time_offset` (`:7096–7123`),
re-bound: a press, a drag and a release at a given pointer row resolve to
the same content row the screen shows, so opening the scrolled screen from
the live composition does not re-map the pointer. The arms-agreement pin
is `the_line_survives_the_intersections_and_the_screen_shows_it`
(`:7408–7423`), whose budget equality holds in the mixed states (a turn
running, the view at the bottom, the marker present or absent, peers
present or absent). The superseded geometry pins were re-derived with the
correction (NFR-13): `the_clock_row_is_not_the_subagent_lines_price`
(`:7392–7405`, the pending budget growing from the clock's row to the
span's rows plus the elapsed row) and `wheel_range_ends_above_the_clock_row`
(`:7775–7798`, the range ending above the composed block).
`finishing_a_turn_while_scrolled_leaves_the_outcome_line`
(`:7801–7828`) holds the handoff at the turn's end.

Depends on: T-84.

Design trace: design §1 (D-61), §3 (the mouse-arms, recording and
superseded-pins angles), §4 (W-62).

## Cross-cutting gates (✅)

- Unit + integration: green at the delivered tip — **645 unit + 6
  integration, 0 failed**, the baseline held exactly (C-09).
- Clippy: identical to the pre-change baseline — the same four warning
  classes on the same sites (`commands.rs:137`, `update.rs:303`,
  `commands.rs:1296`/`:1317–1318`, `send_to.rs:320`/`:335`/`:412`), all
  outside this cycle's diff.
- The `.aiprj` documents mutually consistent (C-08), the work logs written
  (`2026-11-05_002.md` the recording, `2026-11-05_003.md` the execution;
  Art. 6/7), the delivery committed per the user's direction (AC-34) and
  named by the ledger's own follow-up, not by itself.
- The boundary §6's items (the queue contracts, the bare-`Enter` guard, the
  cancel keys, the approval takeover, the headless/piped paths, the
  mouse-capture and clipboard contracts, the peer rows' face and stacking)
  unchanged — verified by the pins standing green and the shipped flows
  untouched (NFR-14).

## The previous cycles' record

**"the terminal's display regions — from the top, the scrollable and
mouse-selection-copyable log, the spinner-and-elapsed run display, the
running subagents' situation (the operating time and the processing
content), the bottom-most line reserved for the prompt — and a bare
`Enter` ignored" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05, delivered at
commit `39afeb2` ("During a turn the terminal composes the four display
regions and a bare Enter is ignored"), its sha named by the tracked
ledger's follow-up `fb46cc0` ("Name the delivery commit in the ledger: the
trail's follow-up"), the ledger re-certified at that tip (BL-M36–BL-M42).
The mechanism stands in the tree and this cycle's corrections are measured
against it: the transcript-backed turn zone as the live screen's sole
writer during a run (`static TURN_ZONE` `src/app.rs:1458`, the struct
`:1414`, the arm `:1503`, the beat `:1664–1756` over the pure
`zone_beat_plan` `:1628–1636`, the span paint `zone_paint_span` `:1810`,
the re-anchor `:1762`, the park `:1592`, the hide/show
`:1777`/`:1787`, the handoff `zone_disarm` `:1536` over
`zone_disarm_plan` `:1650–1661`); the peer rows' processing content over
the measured registry transport (`RegistryEntry.turn`,
`src/ipc/registry.rs:45`, the writes `:215`/`:231`, the reader
`live_children_states` `:2221–2243`, the face `peer_status_rows`
`:2201–2215`); and the bare-`Enter` guard at the editor's arm
(`:2620–2632`, the empty check `:2627–2629`). Its pins stand (`:6905`ff,
`:7126`ff, `:7285`ff); the gates at the close **645 unit + 6 integration,
0 failed**, clippy at the baseline. The version stayed 0.39.0 — the cycle
published no release. Its two superseded **display** statements (the
marker's beneath-the-block place, the clock-only mid-turn pin) are this
cycle's corrections.

**FR-36's display ask** — superseded by FR-37a's record and delivered with
it; the FR-36 cycle never published a version or a tag. Its worktree block
(`src/app.rs:1328–1714` at `5257729`) was carried to delivery as T-76's
transcript-backed turn zone.

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)** — closed, 2026-11-04,
delivered at commit `c02862d` ("Prompts typed while a turn runs are queued
and run in order when it ends"), `Cargo.toml` **0.38.0**, the tag
`v0.38.0`, the GitHub release published **2026-11-04T03:52:18Z**; the
gates at the close **633 unit + 6 integration, 0 failed**, clippy
identical to the baseline. The mechanism stands in the tree and survives
this cycle untouched (the requirements' §6): the `Pending` branch's
acceptor feeding the shared `InputState` in both REPL loops, `Enter`
enqueueing `AgentInput::UserPrompt`, the helpers `queued_count`/
`queue_release_allowed`/`paint_queue_status` (the `[queued: N]` face,
`src/app.rs:1312`/`:1327`/`:1337`), the drain as shipped, the
approval-draft snapshot `DraftSnapshot`/`snapshot_draft`/`restore_draft`
(`src/app.rs:2592–2605`), the pins rewritten
(`a_prompt_typed_during_a_turn_is_queued_and_arrives_while_it_runs`
superseding the shipped pending-loss pin, and its five companions). T-74/
T-75 stand ✅ in that cycle's closed ledger.

Version trail: FR-32 `7fe1370` (the version 0.35.0, never tagged) → FR-33
`af48be5` (the version 0.36.0, never tagged) → `ddeaf67` (FR-34's code; the
tag `v0.36.0` on it) → v0.37.0 `5a2d7f0` (FR-34) → v0.38.0 `c02862d` (FR-35)
→ `881eda9` (the ledger tracked) → v0.39.0 `4f7fad6` (the version
release) → `96a576f` (the records ride the tracked ledger) → `3af66d4`
(the ledger trail extended; BL-M32 recorded) → `5336a03` (the BL-M32
records rode its commit) → `5257729` (the FR-36 documents' rewrite) →
`c2f78b3` (the FR-37/FR-38 cycle's ledger rewrite) → `39afeb2` (the
delivery commit — the cycle's implementation, records and work log) →
`fb46cc0` (the tracked ledger's follow-up, naming `39afeb2` — the rule that
a delivery commit's sha is nameable only from the commit that follows it) →
`db68d3f` (the FR-39/FR-40 delivery commit — the cycle's corrections, its
records and its work logs).

Earlier cycles stand in the requirements document's §12 and the design
document's §5: **"the subagents' auto-exit — permanent (FR-32),
configurable and default-on (FR-33); the 'prompt too long' repair
(FR-34)"** — closed, 2026-11-03 (FR-34 released v0.37.0, the tag on
`5a2d7f0`; the tag `v0.36.0` on `ddeaf67` stands restored — BL-M32);
**"the multi-angle subagent analysis and the subagents' self-exit"
(FR-31)** — investigation and report; **FR-29/FR-30** — `spawn` in the
live `[tools] enabled`; **"the click rule's exclusion of the elapsed
display; the cancelled-by-user display" (FR-27/FR-28)** — v0.34.0
(`c022909`; the tag's final target `745dbb2`); **"the scroll marker's face
— 「Message below」…" (FR-26)** — v0.33.0 (`6cc6661`), its face and its
`view_active` gate standing, its **place** on the prompt pin this cycle's
FR-39; **the agent's running display cycle (FR-22–FR-24)** — v0.32.0
(`e8ad594` + `e0ecb1b`; the clock as the screen's bottom row then — the
geometry FR-37a superseded and FR-40 now brings the scrolled screen into
line with); **the mouse defects cycle (FR-18–FR-21)** — v0.31.0
(`052e230`), its drag/release-offset rule standing and re-bound by FR-40;
**add strata (FR-13–FR-17)** — v0.30.0–0.30.2 (`b706889`, `70e6b5b`,
`a766757`); **the peer-prompt cycles** — v0.29.0 (`045d569`), v0.28.0
(`d237a7a`), v0.27.0 (`afcbe70`); **the mouse input cycle** — v0.25.0
(`3884d16`) with the three 機能追加 addenda closed as v0.26.0 (`251a5bb`).
