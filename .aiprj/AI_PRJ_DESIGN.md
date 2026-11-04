# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the terminal's display regions — from the top, the scrollable and
mouse-selection-copyable log, the spinner-and-elapsed run display, the
running subagents' situation (the operating time and the processing
content), the bottom-most line reserved for the prompt — and a bare
`Enter` ignored" (FR-37, FR-38)
(`.aiprj/instructions.md`, rewritten 2026-11-04 17:08 (+0900))
Paired with: `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What is decided, and what it costs

The previous cycle's design (D-52–D-54, recorded at `5257729`) decided
the running turn's composition — the prompt block owning the screen's
bottom rows during a run, the run's status block relocating above it,
every budget re-derived from the shared geometry helpers. The current
instruction widens the ask to the terminal's four display regions and
adds the empty-`Enter` guard, so the design **carries** D-52–D-54
(mechanically, the FR-36 mechanisms are FR-37a's mechanism) and decides
the two new parts:

- **D-56 — the four-region composition is the standing shape the
  carried mechanism delivers.** The instruction's four regions are the
  shipped mechanisms' contents (AS-51/AS-52): the log region (the
  transcript the scrolled view and the raw writers both ride —
  `src/scroll.rs:1`, the `raw_*` writers' tee `src/app.rs:298–329`, the
  wrap `src/scroll.rs:170–174`), the run display's block (the
  spinner-and-elapsed `status_row` `src/app.rs:4127–4139`, the frames
  `SPINNER_FRAMES` `:3742`, `format_elapsed` `:3764–3774`, the 100 ms
  beat `TICK_INTERVAL` `:3745`, the `tick` `:4106–4116` and the
  clear/repaint `:4152–4208`), the subagent rows (the face
  「peer <name> · <elapsed>」 `peer_status_rows` `:1561–1573`, the
  stacking FR-23/AS-26 — `live_children_states` `:1590–1601`, the
  painter above the block `:1142–1148`), and the prompt line. The ask is
  their composition: on the live screen during a run, the log region
  streams above, the status block and the peer rows stand above the
  bottom-most prompt line — exactly what the carried mechanism delivers
  (the re-stack `bottom_block_rows`' `Pending` branch `:1526–1539`; the
  anchor split D-53; the handoff D-54). On the wheel-actuated scrolled
  screen the shipped composition stands (FR-37c): `draw_scroll_overlay`
  `:1641–1733` slicing the transcript (`body_slice_at`; `ScrollBottom`
  `:1475–1487`, the `Status` budget arm `:1666–1671`), the pinned status
  row (`overlay_status_row` `:4143–4150`, `draw_scrolled`'s pin
  `:4655–4680`, `pinned_status_text` `:1305`), the subagent rows'
  painting inside that arm `:1724–1752`, the wheel arm `:2637–2659`, the
  selection's copy site `:2770–2788`, the beats `SCROLL_REFRESH`
  `:1361–1365` and `SCROLL_IDLE_REFRESH` `:1367–1370` (`needs_refresh`
  `:1427–1435`). The design adds no state machine and no new event type
  (AS-53).
- **D-58 — the processing content rides the shipped peer-row face, one
  measured-transport extension.** The peer rows' face is
  「peer <name> · <elapsed>」 (`peer_status_rows`, `src/app.rs:1561–1573`);
  the ask (FR-37b) adds the processing content while keeping the
  operating time ("経過時間と処理内容など" — the elapsed stays), the row
  staying one line cut to the width (the shipped `truncate_display`
  `src/editor.rs:46`, called at `:1567–1568`). The shipped data sources
  carry no turn state (BL-M37(b)): the registry entries
  (`src/ipc/registry.rs:11–37` — name, model, persona summary, pid,
  started_at) and the IPC message set (`src/ipc/mod.rs:16` —
  Prompt/PromptReply/Ack/Error/Ping/Pong/Context) have no turn-state
  field. The decision: the content's transport — a status extension of
  the shipped IPC set versus a registry-side field — is the execution
  stage's measurement and record (the design's §3′′ placeholder); the
  face degrades to the shipped name-and-operating-time when the state is
  unavailable, and no wire-shape change happens without §6's boundary
  recorded (NFR-14's additivity). The content is the peer's actual
  current processing state (AS-55), not the persona metadata (the
  summary `src/persona.rs:34–39` is what the registry already carries).
- **D-59 — the bare-`Enter` guard sits at the editor boundary, before
  the submit and the serving arms.** The raw editor's `Enter` arm
  submits unconditionally today (`handle_key`'s arm
  `src/app.rs:2039–2042`; the single `InputState::submit`
  `src/editor.rs:283–290` — it consumes the line, zeroes the cursor and
  drops the saved draft), and the serving arms' shipped shape already
  discards or refuses an empty trim downstream (the Ready arm's
  blank-line redraw `:3104–3108`, the Pending acceptor's
  `[info] nothing to queue` refusal `:2913–2930`, the line-mode loop's
  silent discard `:3444–3446`). The guard's place is **before** the
  submit: the arm returns without `submit()` when the editor's line is
  empty (empty-string-only; a whitespace-only line falls through to the
  serving arms' shipped trims) — no state touched, no history write, no
  queue effect, in every prompt state the arm serves. The approval
  `y/N` arm's empty-reads-as-deny is the prompt state's arm (`:3031–3035`),
  not the editor's, and stands untouched (AS-56); the scrolled overlay's
  `Esc|Enter` back-to-live branch `:2813–2816` is consumed before the
  editor's arm and is also untouched. The guard is one arm's addition:
  no existing key path altered, the line loop and every other key
  preserved (NFR-14).
- **D-54 carried — the turn-end handoff and the superseded geometry
  pins.** When the turn's terminal event arrives, the indicator's
  `finish` records the outcome line (`✔/✗ <elapsed>`, the `finish`
  `:4212`, the `record_line` `:4230`) — the status block gives way to
  the transcript, and the prompt block is already in place beneath. The
  one-row clock bottom the shipped pins bind is thereby a specification
  change: the pins `the_clock_row_is_not_the_subagent_lines_price`
  (`src/app.rs:6397–6421`, the Pending face) and the wheel pin "the
  tracked turn pins one row" (`:6711–6730`) are updated in place to the
  new composition in the same commit that delivers the feature (NFR-13),
  as the FR-35 cycle did with its pending-loss pin.

## 2. The touchpoints this cycle touches (TPR-22)

| # | Touchpoint | Planned evidence |
|---|---|---|
| TPR-22a | The live screen's running composition — the prompt block at the bottom rows, the status block above it, the peer rows above the block, the log region streaming above them; the ticker's erase/repaint anchored off the prompt's rows; the acceptor's renders unchanged at their shipped call sites (the carried D-52–D-54 mechanism) | `src/app.rs` (`bottom_block_rows` `:1526–1539`; the acceptor's caller `:2908`, the refusal dance `:2928–2930`, the echo `:2941–2943`; `TICK_INTERVAL` `:3745` with `tick` `:4106–4116`, `repaint` `:4152–4208`; `suspend/resume` `:4246–4253`; the worktree's `TurnZone` block `:1328–1714` — worktree numbers) |
| TPR-22b | The scrolled screen — the shipped pinned composition standing as FR-37c inside the four-region ask (the log's viewport slice, the pinned status row, the subagent rows' band, the prompt block pinned bottom-most) | `src/app.rs` (`draw_scroll_overlay` `:1641–1733`, `ScrollBottom` `:1475–1487`, the `Status` arms `:1666–1671` and `:1724–1752`; `draw_scrolled` `:4649–4680`; `overlay_status_row` `:4143–4150`; the wheel arm `:2637–2659`; the copy site `:2770–2788`; the beats `:1361–1370`) |
| TPR-22c | The peer rows' content — the face 「peer <name> · <elapsed>」 extended with the processing content, the transport measured against the shipped transports and the boundary recorded | `src/app.rs` (`peer_status_rows` `:1561–1573`, `live_children_states` `:1590–1617`), `src/ipc/registry.rs:11–37, 140–185` (the entries and the read seams), `src/ipc/mod.rs:16` (the message set), `src/persona.rs:34–39` (the persona summary) |
| TPR-22d | The bare-`Enter` guard — the editor's Enter arm, its submit and the serving arms' shipped discards | `src/app.rs` (`handle_key` `:2030` with the arm `:2039–2042`, the callers `:2908`/`:3024`, the shipped empty branches `:3104–3108`, `:2913–2930`, `:3444–3446`), `src/editor.rs:283–290` (`submit`) |

## 3. Analysis verdicts (C-12) — the design-stage record, re-verified at the repair stage

- **The composition angle (carried)** — the bottom composition is
  decided in one pure function (`bottom_block_rows`, `src/app.rs:1526`)
  whose callers derive the body's and the drawer's row budgets from it
  (the press arm's `:2724–2736`, the pins `:6397–6421`); the prompt
  arithmetic the `Ready` branch needs already exists
  (`prompt_block_rows`, `:1331`, pinned at `:6266`), so the re-stack is
  a budget change plus an anchor change, not a renderer rewrite.
- **The two-writers angle (BL-M33's race, carried)** — the display
  task's tick erases exactly the painted rows ending at the cursor and
  repaints the status block there (`repaint`, `:4152–4208`); the FR-35
  acceptor paints the typed line through the same region, suspending the
  indicator only around its transient prints (the refusal's
  `:2928–2930`, the echo's `:2941–2943`) — so today the two writers race
  on the bottom rows. With the anchor split (D-53 carried) each writer
  owns its rows: the tick never clears below the status block's top, the
  acceptor's renders never paint above the prompt's rows, and the
  shipped `suspend`/`resume` (`:4246–4253`) keeps mediating the REPL's
  own transient prints (the notices, the echo, the cancel line).
- **The faces angle** — the four regions' contents are the shipped
  faces (AS-51/AS-52): the status row (`:4127–4139`), the thinking rows
  (the `thinking_rows` `:4063`, `THINKING_ROWS` `:3750`), the peer rows
  (`:1561–1573`, painted above the block at `:1142–1148`), the prompt
  block (`prompt_block_rows` `:1331`), the `[queued: N]` notice — none
  of their faces change; the composition re-stacks them, and FR-37b adds
  the processing content to the peer rows (D-58's measured transport).
- **The input-path angle** — the guard's blast radius is exactly the
  empty-`Enter` path: one arm's addition (`:2039–2042`), the serving
  arms' shipped discards keeping their shape (the three shipped empty
  branches `:3104–3108`, `:2913–2930`, `:3444–3446`), the approval arm's
  empty-reads-as-deny untouched (`:3031–3035`), the overlay's
  `Esc|Enter` branch consumed before the editor's arm (`:2813–2816`).
- **The transport angle** — the shipped IPC set enumerates
  Prompt/PromptReply/Ack/Error/Ping/Pong/Context (`src/ipc/mod.rs:18–58`)
  and the registry entries carry no turn state (`src/ipc/registry.rs:11–37`);
  the content's transport is measured at the execution stage (§3′′), with
  the boundary (§6) recorded alongside it and the degrade face standing
  in when the state is unavailable.
- **The FR-36 interplay angle** — the worktree's uncommitted block
  (+386) is the carried mechanism's implementation in flight (§3′): the
  design's D-56 statement and the carried D-52–D-54 are one ask, and the
  execution session continues or re-bases the block against the current
  documents (AS-58).

## 3′. The in-flight FR-36 work block (the worktree's uncommitted +386 lines)

The worktree block (`src/app.rs:1328–1714`, additive, uncommitted at
this update — BL-M38) is the carried mechanism's implementation in
flight: the `TurnZone` struct (the doc phrase "the prompt block parked
at the user's input cursor … the turn zone re-homes everything the
running turn writes to the rows above that block", the struct doc at
`:1331–1347`), the parked cursor (`prompt_top`/`park_col`), the
`DECSTBM` scroll margin, and the peers' reserved band composed at the
screen's top rows (`peers`, `zone_compose_peers`). The block's shape
matches D-52's re-stack, D-53's anchor split and FR-23's peer band. Its
line numbers are **worktree** numbers; the HEAD line numbers stand in
the tables above. The execution session continues or re-bases the block
against the current documents (§3 of the tasks list: T-76; AS-58).

## 3′′. The transport measurement's record (the execution-stage record, ⬜)

D-58's transport measurement is the execution stage's record: which
standing transport carries the processing content (an IPC-set status
extension or a registry-side field) is measured and recorded here when
executed, with the §6 boundary item recorded with it. Until then the
face's degrade rule stands: the shipped name-and-operating-time face.

## 4. Verification of the cycle (planned — the gate table, W-57–W-59)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-57 | FR-37a — the composition: the prompt block on the bottom-most rows during a run (the live screen and the scrolled pin), the budgets re-derived from the helpers, the typed line persisting through the display beats | FR-37, FR-37a, NFR-13 | design TPR-22a/22b | ⬜ |
| W-58 | FR-37b — the spinner-and-elapsed kept above the prompt block; the peer rows carrying the operating time and the processing content (degrading to the shipped face); the thinking rows, the subagent rows' stacking and the `[queued: N]` notice keeping their shipped faces; the turn-end handoff leaving no residue | FR-37b, NFR-14 | TPR-22a/22c | ⬜ |
| W-58′ | The boundaries — the queue contracts (FR-35), the cancel keys, the approval takeover, the headless/piped paths, the mouse-capture and clipboard contracts unchanged | FR-37, FR-38, NFR-14 | — | ⬜ |
| W-59 | The pins and the gates — the superseded geometry pins updated in place, the new pins (the prompt-at-bottom pins for both screens, the line-persists pin, the clock-above pin, the handoff pin, the content pin, the bare-`Enter` pin) green; unit + integration 0 failed, clippy identical to the baseline; the `.aiprj` documents mutually consistent, the work logs written, the delivery committed per the user's direction | FR-37, FR-38, NFR-13, C-08, C-09, NFR-12, AC-33 | TPR-22a/22b/22c/22d | ⬜ |

*Executed rows are transcribed (after execution) into `AI_PRJ_TASKS.md` §4
(the task list's execution record).*

## 5. The previous cycles (recorded after their documents were retired)

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35, a/b/c)** — closed,
2026-11-04: delivered at commit `c02862d` ("Prompts typed while a turn
runs are queued and run in order when it ends"), `Cargo.toml` **0.38.0**,
the tag `v0.38.0`, the GitHub release published **2026-11-04T03:52:18Z**;
the gates at the close: **633 unit + 6 integration, 0 failed**, clippy
identical to the baseline. The mechanism stands in the tree and survives
this cycle untouched (AS-51; the boundary §6): the `Pending` branch's
acceptor feeding the shared `InputState` in both REPL loops, `Enter`
enqueueing `AgentInput::UserPrompt` onto the shipped channel, the
helpers `queued_count`/`queue_release_allowed`/`paint_queue_status` (the
one-line `[queued: N]` face), the drain as shipped, the approval-draft
snapshot (`DraftSnapshot`/`snapshot_draft`/`restore_draft`,
`src/app.rs:2013–2029`), the pins rewritten
(`a_prompt_typed_during_a_turn_is_queued_and_arrives_while_it_runs`
superseding the shipped pending-loss pin,
`stale_idle_signals_do_not_withhold_a_prompt_typed_during_a_turn`,
`approval_snapshot_roundtrip_preserves_the_half_typed_draft`,
`a_queued_prompt_survives_a_cancelled_turn_in_front_of_it`,
`queue_release_gate_answers_every_combination`,
`queued_count_tracks_a_live_channel_without_receiving`; the FR-32 pin
`a_queued_prompt_keeps_a_flagged_agent_alive` inheriting the feature
untouched). The full cycle history (the post-close ledger events
`881eda9` → v0.39.0 `4f7fad6` → `96a576f` → `3af66d4` → `5336a03` →
`5257729`; the BL-M30/BL-M31/BL-M32 corrections) stands in the
requirements document's §12 and the task list's history section — one
shared record, not duplicated here (C-08).

**FR-36's in-flight execution record (the worktree's uncommitted +386
block, `src/app.rs:1328–1714`)** — an execution session opened the
carried mechanism (the prompt block parked at the input cursor, the
turn's writes re-homed to the rows above it, a `DECSTBM` scroll margin,
the subagent band composed at the top rows) and has not delivered it;
the block stands as the worktree's in-flight state and is continued or
re-based by the execution session against the current documents (§3′,
AS-58).

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

**"the scroll marker's face — 「Message below」…" (FR-26)** — closed,
v0.33.0 (`6cc6661`). **"the agent's running display, the subagent status
display, and the prompt's resumption after a cancel" (FR-22–FR-24)** —
closed, v0.32.0 (`e8ad594` + `e0ecb1b`; the clock as the screen's bottom
row then — the geometry this cycle's FR-37a supersedes, the marker face
riding its gate). **"the mouse defects…"** — v0.31.0 (`052e230`).
**"add strata as a connection target…"** — v0.30.0–0.30.2 (`b706889`,
`70e6b5b`, `a766757`). **"a received peer prompt sometimes never starts
processing…"** — closed, v0.29.0 (`045d569`). **"…left unprocessed"** —
v0.28.0 (`d237a7a`). **"…never progresses"** — v0.27.0 (`afcbe70`).
**"mouse input turns into escape sequences, and typing freezes"** —
v0.25.0 (`3884d16`) with the three 機能追加 addenda closed as v0.26.0
(`251a5bb`).