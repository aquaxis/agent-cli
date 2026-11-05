# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the display corrections — during mouse scrolling the `Message below`
marker stands above the prompt, not below it; and during a running turn a
mouse selection no longer makes the elapsed display vanish nor drop the
selection one row" (FR-39, FR-40)
(`.aiprj/instructions.md`, rewritten 2026-11-05 09:31 (+0900))
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

`.aiprj/instructions.md` was rewritten at 2026-11-05 09:31:00 +0900
(595 bytes, sha256 `eca9d4fc…`, §2) and replaces the 2026-11-04 17:08
text this ledger recorded. Its method sentence and its heading stand;
its two asks are new: **the display corrections** — (a) during mouse
scrolling the `Message below` marker is to stand **above** the prompt,
not beneath it; (b) during a running turn a mouse selection must not
make the elapsed display vanish and must not drop the selected row by
one line.

These are corrections to the display the previous cycle delivered. The
previous cycle (FR-37 a/b/c, FR-38 — the four display regions and the
bare-`Enter` guard) is delivered, was re-certified at this ledger's tip
`fb46cc0`, and stands: its asks are not withdrawn here, they are the
ground the two corrections are measured against (§12, §13). The two new
asks are recorded as **FR-39** and **FR-40** (§5) with their baselines
verified in the tree at this update (BL-M40, BL-M41, §3), the design
decisions D-60/D-61 (`AI_PRJ_DESIGN.md` §1) and the tasks T-82–T-85
(`AI_PRJ_TASKS.md`). No product source is changed by this update: the
requirements are recorded and the tasks open (C-13, §13).

The standing method applies (C-12): investigation and repair are to be
performed by multi-angle subagent analysis, with every load-bearing
behaviour pinned (NFR-13).

## 2. Source instruction

`.aiprj/instructions.md` as reloaded in this session (rewritten
2026-11-05 09:31:00.875090395 +0900, 595 bytes; sha256
`eca9d4fc943914aeaab42fd7052660d76ee14b51f92c6559b1f9239d675e8988`;
verified byte-exact):

````text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する

## 最優先事項

### 表示の修正

マウススクロール中に"Message below"が表示されますがプロンプトの下に表示するのではなく、プロンプトの上に表示するように修正して下さい

### マウス選択の修正

エージェントが実行時にマウス選択すると動作中を示す時間経過の行が消えて、マウスで選択した位置が1行下がります、経過時間は表示のまま1行下がらないように修正して下さい
````

(Translation: **Feature additions to agent-cli. The work is to be
performed by multi-angle subagent analysis. Top priority — the display
corrections:** (1) *while mouse scrolling, `Message below` is shown; show
it above the prompt rather than below the prompt.* (2) *when the agent is
running and the user mouse-selects, the row showing the elapsed time of
the running display disappears and the position the user selected drops
by one line; keep the elapsed time displayed and do not let it drop by
one line.*)

The instruction names no third ask and no removal: both corrections are
about **where an existing row stands** (the marker; the elapsed row and
the selected row), not about new regions, new faces or new keys (§3.1).

## 3. Baseline — verified in the tree at this update

Verified 2026-11-05 at HEAD `fb46cc0` (the `main` tip; the worktree
carrying only this update's three documents; `Cargo.toml` 0.39.0, the tag
`v0.39.0` on `4f7fad6`). Every citation below is the tree's own line. The
version was carried to **0.40.0** after the delivery (§12); the baselines
below are the shapes the corrections were measured against and keep the
line numbers they were read at.

| ID | Fact | Evidence |
|---|---|---|
| BL-M40 | **The shipped shape, superseded by D-60 and FR-39.** The scroll marker was painted **beneath** the prompt block on the scrolled screen's prompt pin. The marker's text rode the prompt pin's `text` field, was set from `overlay.view_active()` in the prompt-pinning draw, and was painted by the renderer's marker arm after the block's last row: with the caret parked, `k = (end_row.max(cursor_row) − cursor_row) + 1` and `MoveDown(k)` landed the row **one below** the block's last row. The row's budget was already reserved above that arithmetic (`scrolled_marker_rows` `:1378–1380`, `1` off the bottom and `0` at it), so the ask was the row's **place**, not the row's count. The delivered place is D-60's (the row immediately above the block's first row, `:1201–1213`); the tip's arithmetic is TPR-23a's. | `src/app.rs` at `fb46cc0` (the pre-delivery tree; the delivered shape supersedes it) |
| BL-M41 | **The shipped shape, superseded by D-61 and FR-40.** During a running turn the scrolled screen's pin was the **clock alone**, while the live screen's bottom composition was the prompt span with the clock above it — the two coordinate spaces disagreed by the prompt's rows. The pin choice was `draw_scrolled`'s: when `overlay_status_row()` answered a row it drew a clock-only `ScrollBottom::Status` variant, whose budget arm was `1 + peer_lines`, which painted the clock at the bottom row and never the prompt span. The live screen's mid-turn composition is the opposite order (T-76: the span at the bottom rows, the clock block above it, the peers above that — `zone_beat_plan` `:1628–1636`). The mouse arms measured their body from the **`Pending` branch** of `bottom_block_rows`, `view_moved ? 2 : 1 + peer_lines`, which counted the clock's row alone and no prompt span, and both arms gated `view_moved` on the turn's absence — so mid-turn the body was short of any composition carrying the span and the marker's row was never counted at all. Both symptoms the instruction names followed: the elapsed row's place was taken by the pin's bottom row (it "disappears" from where it stood) and the selected position landed one row lower (AS-60). The delivered composition is D-61's; the tip's arithmetic is TPR-23b's. | `src/app.rs` at `fb46cc0` (the pre-delivery tree; the delivered shape supersedes it) |
| BL-M42 | The previous cycle is delivered and stands: FR-37 (a, b, c) and FR-38 landed at `39afeb2` ("During a turn the terminal composes the four display regions and a bare Enter is ignored"), its sha was named by the tracked ledger's follow-up `fb46cc0`, and the ledger was re-certified at that tip (the previous cycle's BL-M36–BL-M39). The FR-37a mechanism — the armed turn zone (`static TURN_ZONE` `:1458`, the struct `:1414`, the arm `:1503`, the beat `:1664–1756` over `zone_beat_plan` `:1628–1636`, the handoff `zone_disarm` `:1536` over `zone_disarm_plan` `:1650–1662`) — is the ground both corrections are measured against, and it is untouched by them. The cycle published no release of its own; the version was afterwards carried to 0.40.0 (§12). | `git log`/`git show`/`git status` at 2026-11-05, re-read at the delivered tip |

## 3.1 What the instruction does NOT ask for

- **No new region, face or key.** Both corrections move rows that already
  exist: the marker's place (BL-M40) and the running turn's bottom
  composition as the mouse arms measure it (BL-M41). The four regions,
  the peer rows' content, the `[queued: N]` notice, the outcome rows and
  every key keep their shipped meaning (FR-37, FR-38 stand).
- **No transcript change.** Neither the marker nor the clock row is
  recorded into the transcript today (§7 of the shipped contract, the
  `elapsed_clock_survives_scrolling_during_a_running_turn` pin
  `src/app.rs:7735–7772`); the corrections do not change what is
  recorded, only where the display rows stand.
- **No queue-contract change** — FR-35's acceptor, enqueue, count display,
  queued-prompt semantics and drain stand untouched.
- **No bare-`Enter` change** — FR-38's guard (`:2620–2632`) stands; the
  correction surface is the display, not the input path.
- **No headless or piped-stdin change** — where `show_progress = false`
  or the streams are piped there is no indicator block and no overlay;
  the ask is the interactive terminal's display.
- **No mouse-capture or clipboard contract change** — the session-wide
  capture, the drag-selection and the `clip::copy` routes stand; FR-40
  corrects the **rows** a selection resolves against, not the selection
  mechanism.

## 4. Scope

### 4.1 In scope

- **FR-39** — the `Message below` marker's place on the scrolled screen's
  prompt pin: the row above the prompt block, not the row beneath it,
  with the block's rows and the body's budget following it.
- **FR-40** — the running turn's display under a mouse gesture: the
  elapsed row stays displayed where the four-region composition puts it,
  and the row a selection resolves against does not shift; the mouse
  arms' budget and the scrolled screen's mid-turn pin are brought to
  agreement with the delivered live composition.
- The pinned behaviour for both, and the docs/ledger updates.

### 4.2 Out of scope

- The §3.1 list; any change to the queue contracts, the provider
  contracts, the wire shapes, the transcript format, the exit seams or
  the shipped teardown.

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-39 | On the scrolled screen with the prompt pinned and the view off the log's bottom, the `Message below` marker stands on the row **above** the prompt block — the row the four-region order reserves between the run/subagent rows and the bottom-most prompt line — and never below the prompt's last row. The prompt block's rows, the body's slice and the wheel/drag budgets move together with it, so the marker's place, the block's place and the body's end agree in every mixed state; at the log's bottom the marker is absent as shipped. |
| FR-40 | While a turn runs, a mouse gesture on the scrolled screen keeps the running display's elapsed row **displayed and in place**: the scrolled screen composes the running turn's bottom rows as the live screen does (the prompt span at the bottom-most rows, the elapsed row above it, the subagent rows above that), and the row a press, a drag and a release resolve against is the row the screen actually shows — the selected position does not move by a row when the gesture opens the scrolled screen, and the elapsed display is not displaced or withheld. The scroll marker of FR-39 keeps its place within the same composition, and the transcript's recording rules are unchanged (the beat rows stay out of the log). |

## 6. Boundary: FR-39/FR-40 vs the shipped contracts

The shipped contracts stand (NFR-14): the queue semantics (FR-35), the
bare-`Enter` guard (FR-38), the cancel keys' meaning and the `Esc`-to-
live-view path, the approval takeover, the outcome rows' recording, the
four exit seams and the teardown/drain, the mouse-capture and clipboard
contracts, the peer rows' face and stacking (FR-37b). Two shipped
**display** statements are superseded within this cycle's scope — each a
specification change, recorded as such (NFR-13):

- the marker's **beneath-the-block** place on the prompt pin
  (BL-M40; D-28's "the block takes the row below it"), superseded by
  FR-39's above-the-block place;
- the running turn's **clock-only pin** on the scrolled screen and the
  mouse arms' clock-only `Pending` budget (BL-M41; the D-27 gate's
  "during a turn the Status pin carries no marker, one row"), superseded
  by FR-40's agreement with the live four-region composition.

Both are geometry statements about rows the display already owns: no key
path, no recording rule and no wire shape changes (NFR-14).

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | Behaviour-binding pins: every load-bearing behaviour is pinned by a test first made to fail against the shipped shape. For this cycle the pins that bound the **superseded** geometry were re-derived with the corrections and are green at the delivered tip (`the_scrolled_prompt_pin_carries_the_marker_row`, `src/app.rs:7539–7607`; `the_scrolled_screen_swaps_the_clock_for_the_marker`, renamed to `the_scrolled_screen_keeps_the_elapsed_row_and_adds_the_marker`, `:7610–7656`; `the_clock_row_is_not_the_subagent_lines_price`, `:7392–7405`; `the_line_survives_the_intersections_and_the_screen_shows_it`, `:7408–7423`; `wheel_range_ends_above_the_clock_row`, `:7775–7798`), each first made to fail against the shape it replaces, and FR-39's and FR-40's behaviours are pinned new (`the_prompt_span_holds_the_bottom_rows_with_subagents_pinned`, `:7466–7498`; `elapsed_clock_survives_scrolling_during_a_running_turn`, `:7735–7772`; `finishing_a_turn_while_scrolled_leaves_the_outcome_line`, `:7801–7828`). |
| NFR-14 | The changes stay additive to the shipped contracts (the boundary of §6). |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism on the installed binary, not by reading alone. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression: unit + integration counts stay green; clippy output stays identical to the pre-change baseline. Certified at this update against `fb46cc0`: **645 unit + 6 integration, 0 failed**, and the same four clippy warning classes outside the cycle's diff (`commands.rs:137`, `update.rs:303`, `commands.rs:1296`/`:1317–1318`, `send_to.rs:320`/`:335`/`:412`). |
| C-12 | The instruction's method — multi-angle subagent analysis — is performed for the investigation and the repair. |
| C-13 | Write surface for the `.aiprj` documents: the three documents plus the required work logs. This update records the cycle and leaves the product tree unchanged; the corrections are the next execution's deliverable (FR-39/FR-40 are coding asks), recorded here without attribution. |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-34 | FR-39 and FR-40 implemented and pinned: with the prompt pinned and the view off the bottom, the `Message below` marker stands above the prompt block and the budgets agree with it (the wheel's range, the drag's body and the draw's rows on one arithmetic); during a run the scrolled screen keeps the elapsed row displayed in the four-region place and a press/drag/release resolves against the rows the screen shows (no one-row drop); the superseded geometry pins re-derived, the new pins green and failure-first; gates green (unit + integration, 0 failed) and clippy identical to the baseline; the ledger consistent, the work logs written, and the delivery committed per the user's direction. |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-59 | 「プロンプトの上」 names the row **immediately above** the prompt block's first row (the bottom-most region boundary), not the top of the screen and not the space above a multi-row wrapped block: the marker is the last row before the prompt, so a wrapped prompt keeps the marker directly above its first row. | Grounds FR-39's place; read from the instruction and verified against BL-M40's arithmetic. |
| AS-60 | The instruction's second ask is one defect seen twice: the elapsed row "disappearing" and the selection dropping one row are the same disagreement between the scrolled screen's clock-only pin and the live screen's four-region composition (BL-M41), not two unrelated faults — the single correction FR-40 asks for repairs both. | Grounds FR-40's scope; verified at BL-M41 (the `Pending` branch `:2170–2191` over `scrolled_bottom_rows` `:2144–2162` vs the live order `zone_beat_plan` `:1628–1636`), and repaired by one plan at the delivered tip (`scrolled_block_plan` `:2115–2131`). |
| AS-61 | "マウススクロール中" is the wheel-actuated scrolled screen with the prompt pinned — the only place the marker text is produced (`render_prompt_with_run`'s `view_active().then(…)`, `:5347–5350` over `SCROLLED_MARK` `:1372`). The shipped clock-only pin's marker (the clock's row swapped for the text) was the FR-25a face; under FR-40's composition the marker is the block's own row above the prompt span, and the elapsed row stands above it rather than in its place. | Grounds FR-39's surface; read from the instruction and verified at `:5347–5350`, `:1201–1213`. |
| AS-62 | The gesture's rows are read in the scrolled screen's own coordinate space, so "the selected position does not move" means: the content row under the pointer at the press is the content row selected at the release, and opening the scrolled screen from the live screen's four regions does not re-map the pointer by the prompt's rows. | Grounds FR-40's acceptance; verified at the drag arm's `body` (`:3326–3335`) and the release's offset read (`:3373`), both on the one `scrolled_bottom_rows` (`:2144–2162`) the drawer also asks (`:2295–2303`). |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-39 | design §1 (D-60), §2 (TPR-23a), §3, §4 (W-60) | T-82, T-83 |
| FR-40 | design §1 (D-61), §2 (TPR-23b), §3, §4 (W-61) | T-84, T-85 |
| NFR-13 | design §1 (D-60/D-61's pin consequences), §4 (W-62) | T-83, T-85 |
| NFR-14 | design §3 (the boundary angle) | T-82, T-84 |
| AC-34 | design §1 – §4 | T-82, T-83, T-84, T-85 |

## 12. The previous cycles' record

**"the terminal's display regions — from the top, the scrollable and
mouse-selection-copyable log, the spinner-and-elapsed run display, the
running subagents' situation (the operating time and the processing
content), the bottom-most line reserved for the prompt — and a bare
`Enter` ignored" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05, delivered at
`39afeb2` ("During a turn the terminal composes the four display regions
and a bare Enter is ignored"), the sha named by the tracked ledger's
follow-up `fb46cc0`; the ledger re-certified at that tip (BL-M36–BL-M39,
BL-M42). The mechanism: the transcript-backed turn zone as the live
screen's sole writer during a run (`static TURN_ZONE` `src/app.rs:1458`,
the beat `:1664–1756` over `zone_beat_plan` `:1628–1636`, the handoff
`zone_disarm` `:1536` over `zone_disarm_plan` `:1650–1662`), the peer rows' processing
content over the measured registry transport (`RegistryEntry.turn`,
`src/ipc/registry.rs:45`), and the bare-`Enter` guard at the editor's arm
(`:2620–2632`). The pins stand (`:6905`ff, `:7126`ff, `:7285`ff); the
gates at the close **645 unit + 6 integration, 0 failed**, clippy at the
baseline.

**FR-36's display ask** — superseded by FR-37a's record and delivered
with it; the FR-36 cycle never published a version or a tag.

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35, a/b/c)** — closed,
2026-11-04: delivered at commit `c02862d` ("Prompts typed while a turn
runs are queued and run in order when it ends"), `Cargo.toml` **0.38.0**,
the tag `v0.38.0`, the GitHub release published
**2026-11-04T03:52:18Z**; the gates at the close **633 unit + 6
integration, 0 failed**, clippy identical to the baseline. The mechanism
stands in the tree and survives this cycle untouched: the `Pending`
branch's acceptor feeding the shared `InputState` in both REPL loops,
`Enter` enqueueing `AgentInput::UserPrompt` onto the shipped channel, the
helpers `queued_count`/`queue_release_allowed`/`paint_queue_status` (the
one-line `[queued: N]` face, `src/app.rs:1312`/`:1327`/`:1337`), the drain
as shipped, the approval-draft snapshot
(`DraftSnapshot`/`snapshot_draft`/`restore_draft`, `:2592–2605`), the
pins rewritten.

**The post-close ledger events (2026-11-04 – 2026-11-05):** the `.aiprj`
ledger brought under git tracking — commit `881eda9` "Track the .aiprj
project ledger: the three documents, work logs, rules"; the version
released as **v0.39.0** — commit `4f7fad6` "Release 0.39.0: the version
carries the ledger tracking", tag `v0.39.0` pushed, the GitHub release
published **2026-11-04T04:24:19Z**; the records riding the tracked
ledger — `96a576f`, `3af66d4`, `5336a03`; then the FR-36 documents'
rewrite `5257729`, the FR-37/FR-38 cycle's ledger rewrite `c2f78b3`, the
delivery `39afeb2` and the follow-up `fb46cc0` (BL-M42); then the
FR-39/FR-40 delivery `db68d3f`, its ledger follow-up `306f425` naming it,
and the version carried to **0.40.0** by the release commit `505097a`
"Release 0.40.0: the display corrections carry the version" (§13).

**Corrections standing in the ledger:** **BL-M30** (the typed-during-a-
turn prompt was lost, not queued — the shipped contract FR-35
overturned), **BL-M31** (FR-34's close-out re-recorded as v0.37.0 — the
release commit `5a2d7f0`, the tag on it, the CHANGELOG `[0.37.0]`),
**BL-M32** (no `v0.35.0` tag exists; the lightweight tag `v0.36.0`
dereferences to `ddeaf67`; FR-32 `7fe1370` and FR-33 `af48be5` are the
in-tree versions 0.35.0/0.36.0, never tagged, no releases) — each with its
evidence in the cycles' records and work logs.

**"the subagents' auto-exit — permanent (FR-32), configurable and
default-on (FR-33); the 'prompt too long' repair (FR-34)"** — closed,
2026-11-03. FR-32 (commit `7fe1370`), FR-33 (commit `af48be5`), FR-34
(commit `ddeaf67`, released as **v0.37.0** — the release commit
`5a2d7f0`).

**"the multi-angle subagent analysis (standing) and the subagents'
self-exit on completion" (FR-31)** — closed, 2026-11-03 (no release —
investigation and report). Superseded by the implementation (FR-32/
FR-33).

**"the multi-angle subagent analysis, added to the configuration and
verified executable" (FR-29/FR-30)** — closed, 2026-11-03 (no release):
`spawn` in the live `[tools] enabled`, the shipped `[spawn]` bounds.

**"the click rule's exclusion of the elapsed display; the
cancelled-by-user display" (FR-27/FR-28)** — closed, **v0.34.0**
(`c022909`; the tag's final target `745dbb2`). **"the scroll marker's face
— 「Message below」…" (FR-26)** — closed, v0.33.0 (`6cc6661`); its face
and its gate stand, its place on the prompt pin is FR-39's correction.
**"the agent's running display, the subagent status display…"** —
closed, v0.32.0 (`e8ad594` + `e0ecb1b`). **"the mouse defects…"** —
v0.31.0 (`052e230`). **"add strata…"** — v0.30.0–0.30.2 (`b706889`,
`70e6b5b`, `a766757`). **v0.29.0** (`045d569`). **v0.28.0** (`d237a7a`).
**v0.27.0** (`afcbe70`). **v0.25.0** (`3884d16`) with the addenda
**v0.26.0** (`251a5bb`).

## 13. Delivery status

| Item | State |
|---|---|
| FR-39 (the marker above the prompt) | **Delivered.** The marker paints on the row immediately above the prompt block's first row, in one composition with the running turn's rows: the renderer's `above_rows` counts it and folds it into `cursor_row` so the next erase climbs past it (`src/app.rs` `render` `:1156–1260`, the marker arm `:1201–1213`, the fold `:1258`, `clear_block` `:1078–1102`); the text is produced from `overlay.view_active()` at `:5347–5350` and handed over as the pin's `text` field (`:5343–5350`), read back by `marker_row()` (`:2073–2075`). One pure plan answers the place and the count for the drawer and both arms (`scrolled_block_plan` `:2115–2131`, `scrolled_bottom_rows` `:2144–2162`, delegated to by `bottom_block_rows` `:2170–2191`); the wheel arm (`:3250–3256`) and the drag arm (`:3327–3333`) ask that same budget. Pinned by `the_scrolled_prompt_pin_carries_the_marker_row` (`:7539–7607`), re-derived failure-first (NFR-13); T-82/T-83 ✅. |
| FR-40 (the running turn's elapsed row and the selection's rows) | **Delivered.** `ScrollBottom` is a single struct carrying `run_row` beside the prompt's fields (`:2050–2062`, the accessor `:2077–2082`), so the scrolled screen composes the live order in every state: the prompt span at the bottom-most rows, the elapsed row directly above it, the marker above that, the peers above that (`run_row_rows` `:2088–2090`, `scrolled_block_plan` `:2115–2131`). `draw_scrolled` (`:5379–5412`) takes the row from `overlay_status_row()` (`:4779–4786`) and hands it to `render_prompt_with_run` (`:5303–5374`); the renderer paints it above the block (`:1189–1199`), display-only (`finish_line` `:1108–1139` is the only recorder). Both mouse arms pass `view_moved = overlay.view_active()` with no turn gate (`:3249`, `:3326`), so a press, a drag and a release resolve against the rows the screen shows (AS-62). Pinned by `the_scrolled_screen_keeps_the_elapsed_row_and_adds_the_marker` (`:7610–7656`), `the_prompt_span_holds_the_bottom_rows_with_subagents_pinned` (`:7466–7498`), `elapsed_clock_survives_scrolling_during_a_running_turn` (`:7735–7772`) and the re-derived `wheel_range_ends_above_the_clock_row` (`:7775–7798`), `the_clock_row_is_not_the_subagent_lines_price` (`:7392–7405`), `the_line_survives_the_intersections_and_the_screen_shows_it` (`:7408–7423`), `drag_release_resolves_at_the_release_time_offset` (`:7096–7123`); T-84/T-85 ✅. |
| FR-37 / FR-38 (the previous cycle) | Closed and standing (BL-M42, commit `39afeb2`, named by `fb46cc0`). Neither ask is withdrawn; both corrections were measured against their delivered shape and neither is touched by them. |
| State at exit | The instruction is re-verified byte-exact (595 bytes, sha256 `eca9d4fc…`, mtime 2026-11-05 09:31:00.875090395 +0900); the cycle is recorded and delivered in the three documents (FR-39/FR-40, BL-M40–BL-M42, D-60/D-61, TPR-23a/b, T-82–T-85, W-60–W-62, NFR-13/NFR-14, AC-34, AS-59–AS-62, C-13) and in the product tree. The gates are green at the delivered tip: **645 unit + 6 integration, 0 failed** (the baseline held); clippy the same four warning classes on the same sites, all outside the cycle's diff (C-09). The cycle published no release of its own; afterwards, on the user's direction, the version was carried **0.39.0 → 0.40.0** — commit `505097a` "Release 0.40.0: the display corrections carry the version" (`Cargo.toml` and `Cargo.lock` alone, the gates green at it). The work logs `2026-11-05_002.md` (the ledger's recording), `2026-11-05_003.md` (the execution) and `2026-11-05_005.md` (the version carry) carry the cycle; the delivery is committed per the user's direction (AC-34) and named by the ledger's own follow-up, not by itself. |
