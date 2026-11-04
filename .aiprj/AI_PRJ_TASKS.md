# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the bottom-most line during a running turn — the editable prompt on
the bottom row; the spinner-and-elapsed display kept above it" (FR-36, a/b)
(`.aiprj/instructions.md`, rewritten 2026-11-04 14:06)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked
Article 3 of the AI Coding Guidelines: record progress updates in this
file whenever a task changes state.

---

## Overview

The instruction (rewritten 2026-11-04 14:06:59 +0900, verified byte-exact at
this update; 258 bytes, sha256 `d311c050af4ecf0c…`) carries one ask in two
parts: while the agent runs, the spinner and the elapsed time keep being
displayed (**FR-36b**) and the screen's bottom-most row is to show the
prompt one can input into (**FR-36a**) — the two displays coexisting with
fixed places during the run. The baseline verification (BL-M33) located
today's gap on the geometry side: during a run the clock's row is the
screen's bottom row (`bottom_block_rows`' `Pending` budget
`1 + peer_lines`, `src/app.rs:1526–1538`; "the clock stays the screen's
bottom row", `src/app.rs:1730`), and the FR-35 acceptor's typed line
renders wherever the cursor stands — the 100 ms ticker (`TICK_INTERVAL`
`src/app.rs:3745`, the `tick`/`repaint`/`clear` chain `:4106–4141,
:4152–4208`) and the acceptor's renders (`:2905/2933/2958`) race for the
bottom rows with no pin binding the prompt's place. The cycle's
deliverable is the re-stack (`bottom_block_rows`' grown `Pending` branch)
plus the anchor split; the shipped block faces and the FR-35 queue
contracts stand untouched (AS-51/AS-52).

One ledger link was carried at the cycle's start: the BL-M32 records (the
previous session's correction — the version trail's release labels for
FR-32/FR-33 were false records) rode commit `5336a03` "Correct the version
trail's release labels: BL-M32" (2026-11-04 14:06:31 +0900, `.aiprj`-only),
which the trail now carries. The FR-36 cycle's records travel with the
cycle (the tracked ledger's rule: every ledger update rides a normal
commit).

Tasks: T-76 the FR-36 implementation (⬜), T-77 the pins (⬜). The cycle
is **not yet delivered** — no version commit, no tag, no release; the
delivery is committed per the user's direction (AC-33) at the execution
session's close.

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-76 | FR-36's implementation: the shared geometry's `Pending` branch grown to the prompt block's rows + the status block's rows + the additive peer rows (the subagent rows keeping their shipped stacking); the display task's ticker anchored to the rows above the prompt block (the beat never sweeps the prompt's rows); the acceptor's key renders unchanged at their shipped call sites; the scrolled `Status` pin composes the prompt block beneath the clock and the subagent rows, the marker riding the shipped D-27 gate; the turn-end handoff — the outcome line records, the status block clears, the prompt block stands | FR-36, FR-36a, FR-36b, NFR-14 | design §1 (D-52, D-53, D-54), §2 (TPR-21a/21b/21c) | — | ⬜ |
| T-77 | The pins and the gates: the superseded clock-geometry pins (`the_clock_row_is_not_the_subagent_lines_price`, the wheel pin "the tracked turn pins one row", the D-27-gated pending-marker budget) updated in place — a specification change, recorded as such; the new pins (the prompt-at-bottom and line-persists pins for both screens, the clock-above pin, the handoff pin) green; gates green (unit + integration, 0 failed; clippy identical to the baseline); the `.aiprj` documents mutually consistent (C-08), the work logs written, the delivery committed per the user's direction (AC-33) | FR-36, FR-36a, FR-36b, NFR-13, NFR-14, AC-33 | design §1 (D-54), §3, §4 (W-57–W-59) | T-76 | ⬜ |

Sections below the table are the execution record (Article 3) — written as
each task's status changes.

## The cycle's ledger corrections (carried at the cycle's start)

- **The BL-M32 records' ride** — the previous session's correction (the
  version trail's release labels for FR-32/FR-33 were false records — no
  `v0.35.0` tag exists, the lightweight tag `v0.36.0` dereferences to
  `ddeaf67`, `7fe1370`/`af48be5` carry the in-tree versions
  0.35.0/0.36.0, never tagged, no releases) rode commit `5336a03`
  ("Correct the version trail's release labels: BL-M32", 2026-11-04
  14:06:31 +0900) — the trail's missing link, carried by this update.
  The FR-36 cycle's records travel with the cycle (the tracked ledger's
  rule: every ledger update rides a normal commit).

## T-76 — FR-36's implementation (⬜, the execution session's deliverable)

Grounded in the baseline (BL-M33) and the design's decisions D-52–D-54.
The shape, verified against the tree before the design moved:

- **The re-stack** (`src/app.rs:1526–1538`, the `Pending` branch):
  `bottom_block_rows`' budget grows from `1 (+ marker)` to the prompt
  block's rows + the status block's rows + the additive peer rows (AS-52)
  — the composition the `Ready` branch already composes, plus the run's
  block above it. The geometry helpers re-derive every budget (the
  wheel's body, the drawer's slice, the click rule — the shipped
  single-source shape).
- **The anchor split** (D-53): the display task's 100 ms tick
  (`TICK_INTERVAL` `src/app.rs:3745`, the `tick → repaint → clear` chain
  `:4106–4141, :4152–4208`) repaints only the status block's rows above
  the prompt block; the acceptor's renders (the FR-35 sites
  `:2905/2933/2958`) keep rendering the prompt block in place; the
  shipped suspend/resume (`:4257–4264`) continues to mediate the REPL's
  own transient prints (the notices, the echo, the cancel line). The
  typed line persists through every display beat (FR-36a).
- **The scrolled screen** (FR-36a): `draw_scrolled` composes the same two
  blocks stacked — the prompt at the very bottom, the clock above it, the
  subagent rows above that, the marker riding the shipped gate; the
  wheel's budget shrinks by the prompt block's rows exactly as the live
  screen's does.
- **The turn-end handoff** (D-54): the outcome line records
  (`src/app.rs:4212–4243`), the status block clears, the prompt block
  stands; the release arm's `render_prompt` (`:2444–2447`) stays as the
  Ready-state redraw for the queue-emptied prompt. No new event type, no
  new state machine (AS-53).

Depends on: nothing (the first task of the cycle).

Design trace: design §1 (D-52 the composition, D-53 the anchor split,
D-54 the handoff and the pin supersession), §2 (TPR-21a/21b/21c),
§3 (the analysis verdicts), §4 (W-57–W-59 the gate rows T-76 executes).

## T-77 — The pins and gates (⬜, depends on T-76)

- **The superseded geometry pins** — a specification change, recorded as
  such: `the_clock_row_is_not_the_subagent_lines_price` (the Pending
  face, `src/app.rs:6396–6421`) and the wheel pin "the tracked turn pins
  one row" (`:6718–6726` — the D-27-gated pending-marker budget beside
  it) are updated in place to the new composition (NFR-13, as FR-35's
  pending-loss pin was).
- **The new pins**, planned ⬜: the prompt-at-bottom and line-persists
  pins (both screens), the clock-above pin, the handoff pin.
- **The gates** (W-59, planned ⬜): unit + integration 0 failed; clippy
  identical to the baseline; the `.aiprj` documents mutually consistent
  (C-08); the delivery committed per the user's direction (AC-33).

Depends on: T-76.

Design trace: design §1 (D-54's pin consequence), §3 (the two-writers and
cancel/approval angles), §4 (W-57–W-59 the gate rows T-77 executes).

## The previous cycle's record (closed before this cycle)

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
`3af66d4` (the trail extended), `5336a03` (the BL-M32 records' ride).

Version trail: FR-32 `7fe1370` (the version 0.35.0, never tagged) → FR-33 `af48be5` (the version 0.36.0) →
`ddeaf67` (FR-34's code; the tag `v0.36.0` on it) → v0.37.0 `5a2d7f0` (FR-34) → v0.38.0 `c02862d` (FR-35) →
`881eda9` (the ledger tracked) → v0.39.0 `4f7fad6` (the version release) →
`96a576f` (the records ride the tracked ledger) → `3af66d4` (the ledger
trail extended; BL-M32 recorded) → `5336a03` (the BL-M32 records rode its
commit; the FR-36 cycle's records travel ahead).

Earlier cycles stand in the requirements document's §12: **"the
subagents' auto-exit — permanent (FR-32), configurable and default-on
(FR-33); the 'prompt too long' repair (FR-34)"** — closed, 2026-11-03
(FR-34 released v0.37.0, the tag `v0.37.0` on `5a2d7f0`; the tag
`v0.36.0` on `ddeaf67` stands restored — BL-M32); **"the multi-angle
subagent analysis and the subagents' self-exit" (FR-31)** — investigation
and report; **FR-29/FR-30** — `spawn` in the live `[tools] enabled`;
**"the click rule's exclusion of the elapsed display; the
cancelled-by-user display" (FR-27/FR-28)** — v0.34.0 (`c022909`; the
tag's final target `745dbb2`); **"the scroll marker's face — 「Message
below」…" (FR-26)** — v0.33.0 (`6cc6661`); **the agent's running display
cycle (FR-22–FR-24)** — v0.32.0 (`e8ad594` + `e0ecb1b`; the clock as the
screen's bottom row then — the geometry this cycle re-stacks); **the
mouse defects cycle (FR-18–FR-21)** — v0.31.0 (`052e230`); **add strata
(FR-13–FR-17)** — v0.30.0–0.30.2 (`b706889`, `70e6b5b`, `a766757`); **the
peer-prompt cycles** — v0.29.0 (`045d569`), v0.28.0 (`d237a7a`), v0.27.0
(`afcbe70`); **the mouse input cycle** — v0.25.0 (`3884d16`) with the
three 機能追加 addenda closed as v0.26.0 (`251a5bb`).