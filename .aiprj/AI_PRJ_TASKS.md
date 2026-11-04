# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the terminal's display regions — from the top, the scrollable and
mouse-selection-copyable log, the spinner-and-elapsed run display, the
running subagents' situation (the operating time and the processing
content), the bottom-most line reserved for the prompt — and a bare
`Enter` ignored" (FR-37, FR-38) (`.aiprj/instructions.md`, rewritten
2026-11-04 17:08)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked
Article 3 of the AI Coding Guidelines: record progress updates in this
file whenever a task changes state.

---

## Overview

The instruction (rewritten 2026-11-04 17:08:59 +0900, verified byte-exact at
this update; 567 bytes, sha256
`2eba1d17d31c7855d1f20dd8a9137cd7cdfad9ec8ca9a4fa9c5f2d825dfbaa11`) carries
the previous cycle's display ask inside a wider composition and adds one
input ask (FR-37, FR-38 below; the FR-36 numbering superseded — the task
IDs continue). **The cycle is now delivered** (this commit, T-76–T-81 ✅
below): the version stays 0.39.0, the trail's head the ledger rewrite
`c2f78b3`.

The tree at the reload stood at `5257729` (the FR-36 rewrite commit, the
`.aiprj`-only ledger), with an execution session's **uncommitted FR-36
implementation work in the worktree** later carried to delivery: the
`TurnZone` mechanism (the prompt block parked at the input cursor, the
turn's writes re-homed to the rows above it; BL-M38) re-based as T-76's
transcript-backed turn zone (the execution record under T-76 below —
the in-flight block's DECSTBM/SD/SU stream bookkeeping retired in favour
of the record-replayer shape the design's AS-53 names).

The new asks' baseline (BL-M36/BL-M37, the requirements' §3):

- **The four regions are shipped in the scrolled composition** — the
  transcript/log viewport (scrollable, mouse-selection-copyable through
  `crate::clip::copy`), the pinned spinner-and-elapsed status row
  (`overlay_status_row` `:4143`), the peer rows' band
  (`peer_status_rows` `:1561` — 「peer <name> · <elapsed>」), the prompt
  pinned bottom-most (`draw_scroll_overlay` `:1641`,
  `ScrollBottom::Prompt` `:1475–1487`); the composition gap is the live
  screen during a run — the FR-36 mechanism (T-76) delivers it (the
  requirements' FR-37a record).
- **The peer rows name the operating time only** (`:1561–1573`); the
  processing content (FR-37b) needs a transport the shipped sources carry
  no turn state for — the registry entries (`src/ipc/registry.rs:11–37`)
  and the IPC set (`src/ipc/mod.rs:16`) — measured at D-58's execution
  stage.
- **The bare `Enter` (FR-38)**: the raw editor's `Enter` arm submits
  unconditionally (`handle_key`, `src/app.rs:2039–2042`; the single
  `input.submit()` `src/editor.rs:283–290`), the serving arms' empty-trim
  branches (the Ready redraw `:3104–3108`, the Pending refusal
  `:2913–2930`, the line-mode discard `:3444–3446`) are downstream of the
  submit — no guard on the key itself.

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-76 | FR-37a's implementation (carrying the FR-36 mechanism): the shared geometry's `Pending` branch grown to the prompt block's rows + the status block's rows + the additive peer rows (the subagent rows keeping their shipped stacking); the display task's ticker anchored to the rows above the prompt block (the beat never sweeps the prompt's rows); the acceptor's key renders unchanged at their shipped call sites; the scrolled `Status` pin composes the prompt block beneath the clock and the subagent rows, the marker riding the shipped D-27 gate; the turn-end handoff — the outcome line records, the status block clears, the prompt block stands | FR-37, FR-37a, FR-37c, NFR-14 | design §1 (D-52–D-54 carried, D-56), §2 (TPR-22a/22b) | — | ✅ |
| T-77 | The pins and the gates for FR-37a: the superseded clock-geometry pins (`the_clock_row_is_not_the_subagent_lines_price`, the wheel pin "the tracked turn pins one row", the D-27-gated pending-marker budget) re-verified in place — the head-to-head read found their text already carrying the additive peer budget, no change needed (the verification recorded); the new pins (the composition plan, sweep-exclusion, zone click-rule, handoff, screen-span pins) green; gates green (unit + integration, 0 failed; clippy identical to the baseline); the `.aiprj` documents mutually consistent (C-08), the work logs written, the delivery committed per the user's direction (AC-33) | FR-37a, NFR-13, NFR-14, AC-33 | design §1 (D-54), §3, §4 (W-57–W-59) | T-76 | ✅ |
| T-78 | FR-37b's implementation: the peer rows' content — the processing content alongside the operating time (AS-55), the face one line cut to the width, degrading to the shipped 「peer <name> · <elapsed>」 when the state is unavailable — the transport measured against the shipped transports (the IPC set vs the registry seams) and the measure recorded (D-58), with §6's boundary item recorded for any wire-shape change | FR-37, FR-37b, NFR-14 | design §1 (D-58), §2 (TPR-22c), §3′′ | — | ✅ |
| T-79 | FR-37b's pins: the content pin (each row carries name, operating time and content, cut to width, the degrade face standing when unavailable) and the stacking pin (the FR-23/AS-26 order unchanged), failure-first then green | FR-37b, NFR-13 | design §1 (D-58), §4 (W-58) | T-78 | ✅ |
| T-80 | FR-38's implementation: the bare-`Enter` guard at the editor's Enter arm (`src/app.rs:2039–2042`) — an empty submit is ignored (no `submit()`, no state touched, no history write, no queue effect) in every prompt state the arm serves; the serving arms' shipped empty branches keep their shape downstream (AS-56); the approval `y/N` and the overlay's `Esc|Enter` branches untouched | FR-38, NFR-14 | design §1 (D-59), §2 (TPR-22d) | — | ✅ |
| T-81 | FR-38's pins: the guard is a failure-first test (an empty Enter reaches no submit and mutates no state; a non-empty Enter submits unchanged; the line loop and the other keys stand), then green with the feature | FR-38, NFR-13 | design §1 (D-59), §4 (W-58′) | T-80 | ✅ |

Sections below the table are the execution record (Article 3) — written as
each task's status changes.

## The cycle's record (carried at this reload)

- **The FR-36 carry-over** — the previous cycle's documents (commit
  `5257729`, 2026-11-04 14:20:41 +0900) recorded the display ask with
  tasks T-76/T-77 ⬜; this cycle restates them (the table above), the
  instruction's four-region wording being the same ask's wider statement
  (the requirements' §12/§13 record; the FR-36 numbering superseded).
  The worktree's uncommitted `TurnZone` block is T-76's mechanism in
  flight — the execution session continues or re-bases it against the
  current documents (AS-58).
- **The FR-36 documents' in-flight state was already forward-recorded** —
  their own commit (`5257729`) wrote T-76/T-77 ⬜ with an execution
  session in flight; this reload verifies the same state (the block
  uncommitted) and carries the tasks unchanged in scope.
- **The new asks' gap records (BL-M36/BL-M37)** ride the requirements' §3;
  the version trail's head stands at `5257729` and the next ledger commit
  carries this cycle's records (the tracked ledger's rule: every ledger
  update rides a normal commit).

## T-76 — FR-37a's implementation (✅, the FR-36 mechanism carried)

Grounded in the carried design (D-52–D-54, the requirements' FR-37a and
§12's record) and D-56's composition statement. The shape, verified
against the tree before the design moved (the requirements' §3
baselines):

- **The re-stack** (`src/app.rs:1526–1539`, the `Pending` branch):
  `bottom_block_rows`' budget grows from `1 (+ marker)` to the prompt
  block's rows + the status block's rows + the additive peer rows (AS-52)
  — the composition the `Ready` branch already composes, plus the run's
  block above it. The geometry helpers re-derive every budget (the
  wheel's body, the drawer's slice, the click rule — the shipped
  single-source shape).
- **The anchor split** (D-53): the display task's 100 ms tick
  (`TICK_INTERVAL` `src/app.rs:3745`, the `tick → repaint → clear` chain
  `:4106–4116`, `:4152–4208`) repaints only the status block's rows above
  the prompt block; the acceptor's renders (the FR-35 sites `:2905`,
  `:2941–2943`, `:3063`) keep rendering the prompt block in place; the
  shipped suspend/resume (`:4246–4253`) continues to mediate the REPL's
  own transient prints. The typed line persists through every display
  beat (FR-37a).
- **The scrolled screen** (FR-37c standing): the shipped
  `draw_scroll_overlay` composition (the log's viewport slice, the pinned
  status row, the peer rows, the prompt block pinned bottom-most) is the
  four-region composition's scrolled face — no change beyond the carried
  mechanism's budget re-derivation.
- **The turn-end handoff** (D-54): the outcome line records
  (`src/app.rs:4212–4243`), the status block clears, the prompt block
  stands; the release arm's `render_prompt` stays as the Ready-state
  redraw for the queue-emptied prompt. No new event type, no new state
  machine (AS-53).

**Execution record (T-76, ✅):** delivered as the **transcript-backed
turn zone** — the worktree's in-flight block continued (AS-58) and
re-based against the carried design. The zone (`static TURN_ZONE`,
`src/app.rs:1416` the struct, `:1454` the singleton) is the live
screen's **sole writer** while a turn runs: the raw writers
(`raw_println/raw_print_str/raw_eprintln/raw_eprint`, the gate
`src/app.rs:42/60/76/92`) and the status indicator's screen paths
(`repaint/clear/finish`) go record-only while armed, the prompt
renderer's `clear_block`/`finish_line` do likewise (`:1085`/`:1120`) —
the transcript record is the sole truth (AS-53). The display task's
100 ms ticker re-composes the rows above the pinned span through
`zone_beat` (`:1666`, the pure plan `zone_beat_plan` `:1630`: the clock
block directly above the span, the peer band above it — newest nearest
the clock, FR-23/AS-26 — the log's tail above the band via
`crate::scroll::tail_for_region`; the sweep never enters the span's
rows, D-53). The arm (`zone_arm` `:1505`, degrading to the shipped flow
when the terminal is under two rows, AS-54) rides the typed submit and
the `/`-command's `SubmittedPrompt` sites (`:3775`/`:3724`); the handoff
(`zone_disarm` `:1538`: every painted row erased, the record's tail
replayed above the fresh bottom row, the shipped renderer resuming)
rides the release arms (`:3049`, the park timer's `:3894`), the
Pending cancel (`:3491`), the approval's Esc-deny (`:3607`) and the
shutdown tail (`:3927`), with the scroll overlay's `enter`/`leave`
hiding and showing the zone (`:2012`/`:2029` — a turn ended while hidden
hands over at `zone_show`). No state machine, no event type added
(AS-53); the line mode keeps the shipped flow (the zone is the live
screen's mechanism only).

Depends on: nothing (the first task of the cycle).

Design trace: design §1 (D-52–D-54 carried, D-56 the composition
statement), §2 (TPR-22a/22b), §3 (the verdicts), §4 (W-57–W-59).

## T-77 — The pins and gates for FR-37a (✅, depends on T-76)

- **The superseded geometry pins** — re-verified in place: the execution's
  head-to-head read found `the_clock_row_is_not_the_subagent_lines_price`
  and the wheel pin `wheel_range_ends_above_the_clock_row` already
  carrying the additive peer budget in HEAD — their text verifiably
  identical either side (the worktree copies `:7384`/`:7696`; the D-27
  gated pending-marker budget beside them standing green too), the
  composition's budget growth being the D-28 re-derivation the shipped
  helpers already express (NFR-13 satisfied by verification, no text
  change recorded as none was needed).
- **The new pins**, green: the composition's plan pin (`the_beat_plan_`↳
  `holds_the_four_regions_in_the_instruction_order`, `src/app.rs:7277`),
  the sweep/exclusion pin (`the_span_never_enters_the_sweep_even_when_`↳
  `the_block_exhausts_the_screen`, `:7303`), the zone click-rule pin
  (`the_zone_click_rule_hits_only_the_blocks_reasoning_rows`, `:7320`),
  the handoff pin (`the_disarm_leaves_the_bottom_row_fresh_and_the_`↳
  `record_ends_above_it`, `:7345`), the additive-budget pin
  (`the_clock_row_is_not_the_subagent_lines_price`, `:7384`) and its
  scrolled-screen companions
  (`the_line_survives_the_intersections_and_the_screen_shows_it`,
  `the_clock_stays_the_bottom_row_with_subagents_pinned`) standing green
  with the FR-37b rows riding the same budgets.
- **The gates** (W-59, ✅): unit **645** + integration **6**, 0 failed;
  clippy identical to the baseline (the same four warning classes on the
  same sites — `commands.rs:137`, `update.rs:303`, `send_to.rs:320/335/
  412`, the `&PathBuf`/`map_or`/`if let` classes — verified by a stashed
  HEAD-side measurement against the worktree's); the `.aiprj` documents
  mutually consistent (C-08, this rewrite); the work logs written
  (Art. 6/7); the delivery committed per the user's direction (AC-33 —
  the session's closing commit).

Depends on: T-76.

Design trace: design §1 (D-54's pin consequence), §3, §4 (W-57–W-59).

## T-78 — FR-37b's implementation (✅)

Grounded in BL-M37(b) and the design's decision D-58. The peer rows'
face 「peer <name> · <elapsed>」 (`peer_status_rows`, `src/app.rs:1561–1573`)
gains the processing content (AS-55: the peer's actual current
turn/processing state at the read — not the persona metadata), the face
staying one line cut to the width and degrading to the shipped face when
the state is unavailable. The transport — an extension of the shipped IPC
message set (`src/ipc/mod.rs:16`, Prompt/PromptReply/Ack/Error/Ping/
Pong/Context, no turn-state variant) versus a registry-side field (the
entries `src/ipc/registry.rs:11–37`; the write seam `:97–139`
`RegistryHandle`/`cleanup`, the reads `:140–185`) — is **measured at the
execution stage and recorded** (the design's §3′′ placeholder), with §6's
boundary item recorded for any wire-shape change (NFR-14).

**Execution record (T-78, ✅):** the transport measured (design §3′′) —
the **registry-side field** wins, `RegistryEntry`'s `turn: Option<String>`
(`src/ipc/registry.rs:45`; `#[serde(default, skip_serializing_if =
"Option::is_none")]`, old registry files load unchanged and the absence is
the degrade rule), written by the peer itself at the turn's boundaries
(turn start → `Some("working")`, `src/agent.rs:479`; each tool call →
`Some("tool <name>")`, `:656`; the clears at the provider-error Done
`:514`, the cancelled-mid-stream unwind `:623` and the normal Done
`:628`) through `set_turn_state`/`clear_turn_state`
(`src/ipc/registry.rs:215/:231`); the shipped IPC set untouched — no
wire-shape change (§6's boundary item holds with nothing recorded against
it, NFR-14). The read side: `live_children_states` (`src/app.rs:2159`)
returns the 3-tuple (name, operating time, content) stacked oldest-first,
and `peer_status_rows` (`:2139`) renders each row
「peer <name> · <elapsed> · <content>」 — the face one line cut to the
width (`truncate_display`), degrading to the shipped
「peer <name> · <elapsed>」 when the entry lacks the state. The live
screen's beat composes the rows through `live_peer_lines` (`:2189`) with
every 100 ms beat, so the content is read fresh at each repaint.

Depends on: nothing.

Design trace: design §1 (D-58), §2 (TPR-22c), §3′′ (the transport
measurement's record), §4 (W-58).

## T-79 — FR-37b's pins (✅, depends on T-78)

- **The content pin** — landed green:
  `a_peer_row_carries_the_processing_content_when_the_registry_has_it`
  (`src/app.rs:7118` — the row carries the name, the operating time
  (`1m05s`) and the content (`tool bash`), cut to the width) and the
  degrade pin `a_peer_row_degrades_to_the_shipped_face_when_the_state_`↳
  `is_absent` (`:7158` — a missing or blank state leaves the shipped
  「peer <name> · <elapsed>」 face).
- **The stacking pin** — `the_peer_rows_keep_their_stacking_order`
  (`:7180`): the rows keep their FR-23/AS-26 oldest-first order through
  the real seam (`live_children_states` reading the registry), a
  name-order or read-order betrayal failing the pin.

Depends on: T-78.

Design trace: design §1 (D-58), §4 (W-58).

## T-80 — FR-38's implementation (✅)

Grounded in BL-M37(c) and the design's decision D-59. The guard is one
arm at `handle_key`'s Enter site (`src/app.rs:2039–2042`): with the
editor's line empty the arm returns without `submit()` — no
`KeyAction::Submit`, no redraw, no state touched, no history write, no
queue effect — in every prompt state the arm serves.

**Execution record (T-80, ✅):** the guard sits at the editor's Enter
arm before `submit()` (`src/app.rs:2631–2643`, the empty check
`:2638–2640`): an empty line returns `None` — no submit, no state
change, no history write, no queue effect — in every prompt state the
arm serves (D-59); a whitespace-only line still submits (the guard
suppresses the empty string only), the approval `y/N` arm's
empty-reads-as-deny and the overlay's `Esc|Enter` branch untouched
(AS-56). `InputState::submit` itself stands unchanged
(`src/editor.rs:283–290`) — all other callers keep their shipped
behaviour.

Depends on: nothing.

Design trace: design §1 (D-59), §2 (TPR-22d), §3 (the input-path angle),
§4 (W-58′).

## T-81 — FR-38's pins (✅, depends on T-80)

- **The guard pin** — green:
  `bare_enter_is_ignored_without_states_side_effects` (`src/app.rs:6897`
  — an empty Enter reaches no submit and mutates no state) and its
  companion `bare_enter_after_draft_restore_is_ignored_too` (`:6909` —
  the restored draft survives an empty Enter).
- **The unchanged-submit pin** — `nonempty_enter_still_submits` (`:6923`).
- **The whitespace boundary pin** — `whitespace_only_enter_still_submits`
  (`:6934` — the guard suppresses the empty string only).

Depends on: T-80.

Design trace: design §1 (D-59), §4 (W-58′).

## Cross-cutting gates (✅)

- Unit + integration: **645 unit + 6 integration, 0 failed** (C-09);
  clippy identical to the pre-change baseline (C-09 — the same four
  warning classes on the same sites, verified HEAD-side via a stash
  round-trip).
- The `.aiprj` documents mutually consistent (C-08, this rewrite), the
  work logs written (Art. 6/7), the delivery committed per the user's
  direction (AC-33 — the session's closing commit).
- The boundary §6's items (the queue contracts, the cancel keys, the
  approval takeover, the headless/piped paths, the mouse-capture and
  clipboard contracts) unchanged — verified by the pins standing green
  (`stale_idle_signals_do_not_withhold_a_prompt_typed_during_a_turn`,
  the queue-combination pin, the bare-`Enter` pins) and the shipped
  cancel/approval flows untouched (W-58′).

## The previous cycles' record (carried at this reload)

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)** — closed, 2026-11-04,
delivered at commit `c02862d` ("Prompts typed while a turn runs are queued
and run in order when it ends"), `Cargo.toml` **0.38.0**, the tag
`v0.38.0`, the GitHub release published **2026-11-04T03:52:18Z**; the
gates at the close **633 unit + 6 integration, 0 failed**, clippy
identical to the baseline. The mechanism stands in the tree and survives
this cycle untouched (AS-51): the `Pending` branch's acceptor feeding the
shared `InputState` in both REPL loops, `Enter` enqueueing
`AgentInput::UserPrompt`, the helpers `queued_count`/
`queue_release_allowed`/`paint_queue_status` (the `[queued: N]` face),
the drain as shipped, the approval-draft snapshot, the pins rewritten
(`a_prompt_typed_during_a_turn_is_queued_and_arrives_while_it_runs`
superseding the shipped pending-loss pin, and its five companions; the
FR-32 pin inheriting the feature untouched). T-74/T-75 stand ✅ in that
cycle's closed ledger. Post-close ledger events (2026-11-04): `881eda9`
(the `.aiprj` ledger tracked), v0.39.0 (`4f7fad6`, the release published
2026-11-04T04:24:19Z), `96a576f` (the records ride the tracked ledger),
`3af66d4` (the trail extended), `5336a03` (the BL-M32 records' ride),
`5257729` (the FR-36 cycle's design-stage rewrite).

Version trail: FR-32 `7fe1370` (the version 0.35.0, never tagged) → FR-33
`af48be5` (the version 0.36.0) → `ddeaf67` (FR-34's code; the tag
`v0.36.0` on it) → v0.37.0 `5a2d7f0` (FR-34) → v0.38.0 `c02862d` (FR-35)
→ `881eda9` (the ledger tracked) → v0.39.0 `4f7fad6` (the version
release) → `96a576f` (the records ride the tracked ledger) → `3af66d4`
(the ledger trail extended; BL-M32 recorded) → `5336a03` (the BL-M32
records rode its commit) → `5257729` (the FR-36 documents' rewrite) →
`c2f78b3` (the FR-37/FR-38 cycle's ledger rewrite) → `39afeb2` (the
delivery commit — "During a turn the terminal composes the four display
regions and a bare Enter is ignored", the cycle's implementation, records
and work log; its sha named by the ledger commit that follows it, the
tracked ledger's rule).

Earlier cycles stand in the requirements document's §12 and the design
document's §5: **"the subagents' auto-exit — permanent (FR-32),
configurable and default-on (FR-33); the 'prompt too long' repair
(FR-34)"** — closed, 2026-11-03 (FR-34 released v0.37.0, the tag on
`5a2d7f0`; the tag `v0.36.0` on `ddeaf67` stands restored — BL-M32);
**"the multi-angle subagent analysis and the subagents' self-exit"
(FR-31)** — investigation and report; **FR-29/FR-30** — `spawn` in the
live `[tools] enabled`; **"the click rule's exclusion of the elapsed
display; the cancelled-by-user display" (FR-27/FR-28)** — v0.34.0
(`c022909`; the tag's final target `745dbb2`); **"the scroll marker's
face — 「Message below」…" (FR-26)** — v0.33.0 (`6cc6661`); **the agent's
running display cycle (FR-22–FR-24)** — v0.32.0 (`e8ad594` + `e0ecb1b`;
the clock as the screen's bottom row then — the geometry the FR-37a
mechanism supersedes); **the mouse defects cycle (FR-18–FR-21)** —
v0.31.0 (`052e230`); **add strata (FR-13–FR-17)** — v0.30.0–0.30.2
(`b706889`, `70e6b5b`, `a766757`); **the peer-prompt cycles** — v0.29.0
(`045d569`), v0.28.0 (`d237a7a`), v0.27.0 (`afcbe70`); **the mouse input
cycle** — v0.25.0 (`3884d16`) with the three 機能追加 addenda closed as
v0.26.0 (`251a5bb`).