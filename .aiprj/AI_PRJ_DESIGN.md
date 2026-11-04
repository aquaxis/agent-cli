# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the bottom-most line during a running turn — the editable prompt on
the bottom row; the spinner-and-elapsed display kept above it" (FR-36, a/b)
(`.aiprj/instructions.md`, rewritten 2026-11-04 14:06 (+0900))
Paired with: `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What is decided, and what it costs

- **D-52 — the fixed composition: the prompt block owns the screen's
  bottom-most rows during a run; the run's status block relocates above
  it; the shared budgets re-derive from the same helpers.** Today the
  `Pending` branch of the shared geometry gives the clock one row
  (`1 + peer_lines`, `src/app.rs:1526–1538`) and the
  "the clock stays the screen's bottom row" comment binds it
  (`src/app.rs:1730`); the typed line the FR-35 acceptor renders has no
  geometric place — it paints wherever the cursor stands and the display
  task's 100 ms ticker repaints the indicator's block over the same rows.
  The design rests the composition in one place: with a turn running and
  the prompt `Pending`, the bottom block is the **prompt block**
  (`prompt_block_rows` rows, the subagent rows and the suggestion row
  keeping their shipped faces inside/above it exactly as the `Ready`
  branch already composes) plus the run's status rows **above** it —
  spinner-and-elapsed on the row above the prompt block's top, the
  captured thinking rows above that, the subagent rows above the whole
  status block as FR-23 stacks them. Nothing else changes face or order:
  the status row's composition (`src/app.rs:4127–4137`), the thinking
  rows (`:4063–4101`) and the peer rows (`:1142–1148`) are the shipped
  shapes, only re-anchored. `bottom_block_rows`' `Pending` branch grows
  from `1 (+ marker under the D-27 gate)` to the prompt block's rows +
  the status block's rows + the additive peer rows (`AS-52`), so the
  wheel's range, the drawer's body slice and the click rule all re-derive
  from the same helper without individual touch-ups (the shipped
  derivation shape: `src/app.rs:2724–2734`, `6396–6421`).
- **D-53 — the two writers stop racing: the ticker repaints only the
  status block; the acceptor's renders repaint only the prompt block.**
  The displayed beat stays the shipped 10 Hz tick (`TICK_INTERVAL`,
  `src/app.rs:3745`; the `tick → repaint → clear` chain
  `:4106–4141, 4152–4208`) — the ask keeps the spinner and the elapsed
  time refreshing exactly as shipped — but the block's erase/repaint is
  anchored to the rows **above** the prompt block, never sweeping the
  prompt's rows (the indicator's `clear` walks up `painted_rows` from the
  cursor, so the tick's paint position must account for the prompt block
  standing beneath; the shipped `suspend()`/`resume()` dance remains the
  mediation for the REPL's own writes, `:4257–4264`). The acceptor's key
  renders (`src/app.rs:2905/2933/2958` — the shipped FR-35 call sites)
  keep rendering the prompt block in place; the typed line therefore
  **persists through every display beat** (FR-36a), edits land on the
  bottom rows, and the `[queued: N]` notice and the submit echo keep
  their shipped faces. No new event type and no new state machine — the
  composition is the existing beat machinery with a fixed anchor (AS-53).
- **D-54 — the turn-end handoff and the superseded geometry pins.** When
  the turn's terminal event arrives, the indicator's `finish` records the
  outcome line (`✔/✗ <elapsed>`, `src/app.rs:4212–4243`) — the status
  block gives way to the transcript, and the prompt block is already in
  place beneath, so the handoff leaves no residue and needs no redraw of
  the prompt (the release arm's `render_prompt` at `:2444–2447` stays as
  the `Ready`-state redraw for the queue-emptied prompt). The one-row
  clock bottom the shipped pins bind is thereby a **specification
  change**: the pins `the_clock_row_is_not_the_subagent_lines_price`
  (`src/app.rs:6396–6421`, the Pending face) and the wheel pin "the
  tracked turn pins one row" (`:6718–6726`) — plus the D-27-gated
  pending-marker budget they carry — are updated in place to the new
  composition in the same commit that delivers the feature (NFR-13), as
  the FR-35 cycle did with its pending-loss pin.

## 2. The touchpoints this cycle touches (TPR-21)

| # | Touchpoint | Planned evidence |
|---|---|---|
| TPR-21a | The live screen's `Pending` composition — the prompt block at the bottom rows, the status block above it, the ticker's erase/repaint anchored off the prompt's rows, the acceptor's renders unchanged at their call sites | `src/app.rs` (`bottom_block_rows` `:1526–1538`; the acceptor `:2856–2963`; `TICK_INTERVAL` `:3745` with `tick/repaint/clear` `:4106–4141, 4152–4208`; `suspend/resume` `:4257–4264`) |
| TPR-21b | The geometry helpers and the scrolled screen — the budgets re-derived from `bottom_block_rows`/`prompt_block_rows`; the scrolled `Status` pin composes the prompt block beneath the clock (and the subagent rows), the marker riding the shipped gate | `src/app.rs:1331–1340` (`prompt_block_rows`), `:1526–1538`, the scrolled drawer `:1646–1680` and its status preference `:4657–4680`, the refresh arm `:3182` |
| TPR-21c | The turn-end handoff — the outcome line records and the status block clears while the prompt block stands; the release arm redraws only as shipped | `src/app.rs:4212–4243` (`finish`), the release arm `:2436–2447`, the display task's event arm `:1891–1931` |

## 3. Analysis verdicts (C-12) — the design-stage record, re-verified at the repair stage

- **The geometry angle** — the bottom composition is decided in one pure
  function (`bottom_block_rows`, `src/app.rs:1526`) whose callers derive
  the body's and the drawer's row budgets from it
  (`:2724–2734`, the pins `:6396–6421`); the prompt arithmetic the
  `Ready` branch needs already exists (`prompt_block_rows`,
  `:1331–1340`, pinned at `:6266–6276`), so the re-stack is a budget
  change plus an anchor change, not a renderer rewrite.
- **The two-writers angle (BL-M33's race)** — the display task's tick
  erases exactly `painted_rows` rows ending at the cursor and repaints
  the status block there (`:4191–4208`, `:4152–4186`); the FR-35
  acceptor paints the typed line at the same cursor region without
  suspending the indicator for the pure edit renders (the sites
  `:2905/2933/2958` vs the shipped suspend sites `:2876`, `:2942–2944`)
  — so today the two writers overwrite each other's rows. With the anchor
  split (D-53) each writer owns its rows: the tick never clears below the
  status block's top, the acceptor's renders never paint above the
  prompt's rows, and the shipped suspend/resume continues to mediate the
  REPL's own transient prints (the notices, the echo, the cancel line).
- **The cancel/approval angle** — Esc mid-turn suspends the indicator,
  prints the cancellation and redraws the prompt (`:2866–2884`); the
  approval takeover suspends the indicator and lets the `y/N` prompt own
  the bottom (`:2470–2496`). Both seams keep their shipped shape: their
  suspensions already take the whole block; the composition changes the
  rows those suspensions return to, nothing about their semantics
  (FR-36's §6 boundary; AS-51).
- **The scrolled angle** — `draw_scrolled` pins the status row when a run
  is tracked and the prompt otherwise (`:4657–4680`); under FR-36a the
  scrolled screen composes the same two blocks stacked (prompt at the
  very bottom, clock above it, the subagent rows above that, the marker
  riding its gate), so the wheel's budget shrinks by the prompt block's
  rows exactly as the live screen's does — one arithmetic
  (`bottom_block_rows`' grown branch) feeding both screens, the shipped
  single-source shape.

## 4. Verification of the cycle (planned — the gate table, W-57–W-59)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-57 | FR-36a — the composition: the prompt block on the bottom-most rows during a run (live screen + scrolled pin), the budgets re-derived from the helpers, the typed line persisting through the display beats | FR-36, FR-36a, NFR-13 | design TPR-21a/21b | ⬜ |
| W-58 | FR-36b — the spinner-and-elapsed kept above the prompt block; the thinking rows, the subagent rows and the `[queued: N]` notice keeping their shipped faces; the turn-end handoff leaving no residue | FR-36b, NFR-14 | TPR-21a/21c | ⬜ |
| W-58′ | The boundaries — the queue contracts (FR-35), the cancel keys, the approval takeover, the headless/piped paths unchanged | FR-36, NFR-14 | — | ⬜ |
| W-59 | The pins and the gates — the superseded geometry pins updated in place, the new pins (the prompt-at-bottom pins for both screens, the line-persists pin, the clock-above pin, the handoff pin) green; unit + integration 0 failed, clippy identical to the baseline; the `.aiprj` documents mutually consistent, the work logs written, the delivery committed per the user's direction | FR-36, NFR-13, C-08, C-09, NFR-12, AC-33 | TPR-21c | ⬜ |

*Executed rows are transcribed (after execution) into `AI_PRJ_TASKS.md` §4
(the task list's execution record).*

## 5. The previous cycles (recorded after their documents were retired)

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35, a/b/c)** — closed, 2026-11-04:
delivered at commit `c02862d` ("Prompts typed while a turn runs are queued
and run in order when it ends"), `Cargo.toml` **0.38.0**, the tag
`v0.38.0`, the GitHub release published **2026-11-04T03:52:18Z**; the gates
at the close: **633 unit + 6 integration, 0 failed**, clippy identical to
the baseline. The mechanism stands in the tree and survives this cycle
untouched (AS-51): the `Pending` branch's acceptor feeding the shared
`InputState` in both REPL loops, `Enter` enqueueing
`AgentInput::UserPrompt` onto the shipped channel, the helpers
`queued_count`/`queue_release_allowed`/`paint_queue_status` (the one-line
`[queued: N]` face), the drain as shipped, the approval-draft snapshot,
the pins rewritten (`a_prompt_typed_during_a_turn_is_queued_and_arrives_
while_it_runs` superseding the shipped pending-loss pin,
`stale_idle_signals_do_not_withhold_a_prompt_typed_during_a_turn`,
`approval_snapshot_roundtrip_preserves_the_half_typed_draft`,
`a_queued_prompt_survives_a_cancelled_turn_in_front_of_it`,
`queue_release_gate_answers_every_combination`,
`queued_count_tracks_a_live_channel_without_receiving`; the FR-32 pin
`a_queued_prompt_keeps_a_flagged_agent_alive` inheriting the feature
untouched). The post-close ledger events: `.aiprj` tracked by git —
commit `881eda9` ("Track the .aiprj project ledger: the three documents,
work logs, rules"); the version released as **v0.39.0** — commit `4f7fad6`
("Release 0.39.0: the version carries the ledger tracking"), tag pushed,
the GitHub release published **2026-11-04T04:24:19Z**; the records riding
the tracked ledger — `96a576f` ("Record the ledger-tracking release
close: the ledger events ride the tracked documents", the first ledger
commit), `3af66d4` (the trail extended), `5336a03` (the BL-M32
correction's records; the correction standing in the requirements
document's §12).

The decisions D-49–D-51 (the acceptor, the drain, the count) and the
touchpoints TPR-20a/20b/20c of that cycle stand in its work logs; the
design's §3′ superseded-state records of the FR-34 close-out
(recorded BL-M31, narrowed BL-M32) are summarized in the requirements
document's §12 and the task list's corrections section, mutually
consistent (C-08).

**"the subagents' auto-exit — permanent (FR-32), configurable and
default-on (FR-33); the 'prompt too long' repair (FR-34)"** — closed,
2026-11-03: FR-32 (commit `7fe1370`, the version 0.35.0 never tagged),
FR-33 (commit `af48be5`, version 0.36.0, equally never tagged), FR-34
(commit `ddeaf67`, released as **v0.37.0** — the release commit `5a2d7f0`
"Release 0.37.0: the compaction repair carries the version", the tag on
the release commit; the tag `v0.36.0` on `ddeaf67` stands restored —
BL-M32, with BL-M31 narrowed). Gates at the close: **629 unit +
6 integration, 0 failed**; clippy at the baseline.

**"the multi-angle subagent analysis, added to the configuration and
verified executable" (FR-29/FR-30)** — closed, 2026-11-03 (no release):
`spawn` in the live `[tools] enabled` (the shipped default omits it), the
shipped `[spawn]` bounds; live round trips grounding the verdict; R-1
carried, R-2 withdrawn as false.

**"the click rule's exclusion of the elapsed display; the
cancelled-by-user display" (FR-27/FR-28)** — closed, **v0.34.0** (commit
`c022909`, the tag's final target `745dbb2`): the click rule's strict top
excluding the elapsed row, the query-error fallback disarmed; the three
cancel prints `[info] cancelled by user` styled `Role::Info`.

**"the scroll marker's face — 「Message below」…" (FR-26)** — closed,
v0.33.0 (`6cc6661`). **"the agent's running display, the subagent status
display, and the prompt's resumption after a cancel" (FR-22–FR-24)** —
closed, v0.32.0 (`e8ad594` + `e0ecb1b`; the clock as the screen's bottom
row then — the geometry this cycle re-stacks). **"the mouse defects…"** —
v0.31.0 (`052e230`). **"add strata as a connection target…"** —
v0.30.0–0.30.2 (`b706889`, `70e6b5b`, `a766757`). **"a received peer
prompt sometimes never starts processing…"** — closed, v0.29.0
(`045d569`). **"…left unprocessed"** — v0.28.0 (`d237a7a`). **"…never
progresses"** — v0.27.0 (`afcbe70`). **"mouse input turns into escape
sequences, and typing freezes"** — v0.25.0 (`3884d16`) with the three
機能追加 addenda closed as v0.26.0 (`251a5bb`).