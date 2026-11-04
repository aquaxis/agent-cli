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
IDs continue). The cycle is **not yet delivered** — the version stays
0.39.0, the trail's head `5257729`.

The tree at this reload stands at `5257729` (the FR-36 rewrite commit, the
`.aiprj`-only ledger). An execution session holds **uncommitted FR-36
implementation work in the worktree**: `src/app.rs` +386 additive lines —
the `TurnZone` mechanism (the prompt block parked at the input cursor, the
turn's writes re-homed to the rows above it, a `DECSTBM` scroll margin, the
subagent band composed at the screen's top rows,
`src/app.rs:1328–1714` in the worktree; BL-M38). The tasks below carry
that work as T-76/T-77 (the requirements' FR-37a mechanism) unchanged in
scope, with the execution session continuing or re-basing it against the
current documents (AS-58).

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
| T-76 | FR-37a's implementation (carrying the FR-36 mechanism): the shared geometry's `Pending` branch grown to the prompt block's rows + the status block's rows + the additive peer rows (the subagent rows keeping their shipped stacking); the display task's ticker anchored to the rows above the prompt block (the beat never sweeps the prompt's rows); the acceptor's key renders unchanged at their shipped call sites; the scrolled `Status` pin composes the prompt block beneath the clock and the subagent rows, the marker riding the shipped D-27 gate; the turn-end handoff — the outcome line records, the status block clears, the prompt block stands | FR-37, FR-37a, FR-37c, NFR-14 | design §1 (D-52–D-54 carried, D-56), §2 (TPR-22a/22b) | — | ⬜ |
| T-77 | The pins and the gates for FR-37a: the superseded clock-geometry pins (`the_clock_row_is_not_the_subagent_lines_price`, the wheel pin "the tracked turn pins one row", the D-27-gated pending-marker budget) updated in place — a specification change, recorded as such; the new pins (the prompt-at-bottom and line-persists pins for both screens, the clock-above pin, the handoff pin) green; gates green (unit + integration, 0 failed; clippy identical to the baseline); the `.aiprj` documents mutually consistent (C-08), the work logs written, the delivery committed per the user's direction (AC-33) | FR-37a, NFR-13, NFR-14, AC-33 | design §1 (D-54), §3, §4 (W-57–W-59) | T-76 | ⬜ |
| T-78 | FR-37b's implementation: the peer rows' content — the processing content alongside the operating time (AS-55), the face one line cut to the width, degrading to the shipped 「peer <name> · <elapsed>」 when the state is unavailable — the transport measured against the shipped transports (the IPC set vs the registry seams) and the measure recorded (D-58), with §6's boundary item recorded for any wire-shape change | FR-37, FR-37b, NFR-14 | design §1 (D-58), §2 (TPR-22c), §3′′ | — | ⬜ |
| T-79 | FR-37b's pins: the content pin (each row carries name, operating time and content, cut to width, the degrade face standing when unavailable) and the stacking pin (the FR-23/AS-26 order unchanged), failure-first then green | FR-37b, NFR-13 | design §1 (D-58), §4 (W-58) | T-78 | ⬜ |
| T-80 | FR-38's implementation: the bare-`Enter` guard at the editor's Enter arm (`src/app.rs:2039–2042`) — an empty submit is ignored (no `submit()`, no state touched, no history write, no queue effect) in every prompt state the arm serves; the serving arms' shipped empty branches keep their shape downstream (AS-56); the approval `y/N` and the overlay's `Esc|Enter` branches untouched | FR-38, NFR-14 | design §1 (D-59), §2 (TPR-22d) | — | ⬜ |
| T-81 | FR-38's pins: the guard is a failure-first test (an empty Enter reaches no submit and mutates no state; a non-empty Enter submits unchanged; the line loop and the other keys stand), then green with the feature | FR-38, NFR-13 | design §1 (D-59), §4 (W-58′) | T-80 | ⬜ |

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

## T-76 — FR-37a's implementation (⬜, the FR-36 mechanism carried)

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

Depends on: nothing (the first task of the cycle).

Design trace: design §1 (D-52–D-54 carried, D-56 the composition
statement), §2 (TPR-22a/22b), §3 (the verdicts), §4 (W-57–W-59).

## T-77 — The pins and gates for FR-37a (⬜, depends on T-76)

- **The superseded geometry pins** — a specification change, recorded as
  such: `the_clock_row_is_not_the_subagent_lines_price` (the Pending
  face, `src/app.rs:6397–6421`) and the wheel pin "the tracked turn pins
  one row" (`:6711–6730` — the D-27-gated pending-marker budget beside
  it) are updated in place to the new composition (NFR-13, as FR-35's
  pending-loss pin was).
- **The new pins**, planned ⬜: the prompt-at-bottom and line-persists
  pins (both screens), the clock-above pin, the handoff pin.
- **The gates** (W-59, planned ⬜): unit + integration 0 failed; clippy
  identical to the baseline; the `.aiprj` documents mutually consistent
  (C-08); the delivery committed per the user's direction (AC-33).

Depends on: T-76.

Design trace: design §1 (D-54's pin consequence), §3, §4 (W-57–W-59).

## T-78 — FR-37b's implementation (⬜)

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

Depends on: nothing.

Design trace: design §1 (D-58), §2 (TPR-22c), §3′′ (the transport
measurement's record), §4 (W-58).

## T-79 — FR-37b's pins (⬜, depends on T-78)

- **The content pin** — failure-first: each peer row carries the name,
  the operating time and the processing content, truncated as one to the
  width; when the state is unavailable the shipped face stands — then
  green with the feature.
- **The stacking pin** — the rows keep their FR-23/AS-26 order and their
  recording exclusion.

Depends on: T-78.

Design trace: design §1 (D-58), §4 (W-58).

## T-80 — FR-38's implementation (⬜)

Grounded in BL-M37(c) and the design's decision D-59. The guard is one
arm at `handle_key`'s Enter site (`src/app.rs:2039–2042`): with the
editor's line empty the arm returns without `submit()` — no
`KeyAction::Submit`, no redraw, no state touched, no history write, no
queue effect — in every prompt state the arm serves. `InputState::submit`
consumes the line, zeroes the cursor and drops the saved draft when it
runs (`src/editor.rs:283–290`) — the guard keeps the empty Enter from
reaching it, preserving an untouched half-typed draft. The serving arms'
shipped empty branches (the Ready redraw `:3104–3108`, the Pending
refusal `:2913–2930`, the line-mode discard `:3444–3446`) keep their
shape downstream (AS-56); the approval `y/N` arm's empty-reads-as-deny
(`:3031–3035`) and the scrolled overlay's `Esc|Enter` back-to-live branch
(`:2813–2816`) are untouched.

Depends on: nothing.

Design trace: design §1 (D-59), §2 (TPR-22d), §3 (the input-path angle),
§4 (W-58′).

## T-81 — FR-38's pins (⬜, depends on T-80)

- **The guard pin** — failure-first: with a draft loaded and no text
  typed, an Enter reaches neither the editor's `submit` nor any
  downstream serving arm, and the draft survives — then green with the
  guard in.
- **The unchanged-submit pin** — a non-empty Enter submits exactly as
  shipped (the echo, the classification, the history, the queue).
- **The state-preservation pins** — the approval y/N responses and the
  overlay's `Esc|Enter` behaviour unchanged by the guard.

Depends on: T-80.

Design trace: design §1 (D-59), §4 (W-58′).

## Cross-cutting gates (⬜, after the tasks)

- Unit + integration: 0 failed; clippy identical to the pre-change
  baseline (C-09).
- The `.aiprj` documents mutually consistent (C-08), the work logs
  written (Art. 6/7), the delivery committed per the user's direction
  (AC-33).
- The boundary §6's items (the queue contracts, the cancel keys, the
  approval takeover, the headless/piped paths, the mouse-capture and
  clipboard contracts) unchanged.

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
records rode its commit) → `5257729` (the FR-36 documents' rewrite; this
cycle's records travel with the next ledger commit).

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