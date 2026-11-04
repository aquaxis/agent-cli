# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the bottom-most line during a running turn — the editable prompt on
the bottom row; the spinner-and-elapsed display kept above it" (FR-36)
(`.aiprj/instructions.md`, rewritten 2026-11-04 14:06 (+0900))
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

The previous cycle delivered prompt input during a running turn — accepted
into the queue and run in order once the turn completes (FR-35, closed at
**v0.38.0**; the delivery record stands in §12 and the task list). The
current instruction (rewritten 2026-11-04 14:06:59 +0900; reloaded
byte-exact) carries one display ask for that same running turn:

1. **The running display stays** — while the agent works, the spinner and
   the elapsed time (the indicator's status row) keep being displayed.
2. **The bottom-most line is the prompt** — the screen's bottom-most row
   shows the prompt the user can type into (the editable input line), not
   the clock's row.

Today's shipped geometry pins the opposite: during a running turn the
clock's row **is** the screen's bottom row (the `Pending` budget is
`1 + peer_lines`, "the clock stays the screen's bottom row",
`src/app.rs:1730`; the geometry pins bind exactly one row,
`src/app.rs:6396–6421`, `src/app.rs:6718–6726`), and the FR-35 acceptor's
typed line renders wherever the cursor stands with no pin on its location
during the run — the display task's 100 ms ticker repaints the indicator's
block from that same region (`TICK_INTERVAL`, `src/app.rs:3745`; the
`tick`/`repaint`/`clear` chain, `src/app.rs:4106–4141, 4152–4208`), so the
typed line and the spinner race for the bottom rows and nothing binds the
prompt's presence there. The ask closes exactly that gap (FR-36).

The standing method applies (C-12): investigation and repair are to be
performed by multi-angle subagent analysis, with every load-bearing
behaviour pinned (NFR-13).

## 2. Source instruction

`.aiprj/instructions.md` as reloaded in this session (rewritten
2026-11-04 14:06:59 +0900, 258 bytes; sha256
`d311c050af4ecf0c9ba49ed28fb2e43061f71b27ee97ed75f186911a0516d553`;
verified byte-exact):

````text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する

## 最優先事項

エージェントが実行時にクルクルと経過時間が表示されるが一番、下の行は入力できるプロンプトを表示する
````

(Translation: **Feature additions to agent-cli. The work is to be performed
by multi-angle subagent analysis. Top priority:** while the agent runs, a
spinner and the elapsed time are displayed, but the bottom-most line is to
display the prompt one can input into.)

## 3. Baseline — verified in the tree at this update

Verified 2026-11-04 at HEAD `5336a03` (worktree clean, `main` =
`origin/main`; `Cargo.toml` 0.39.0):

| ID | Fact | Evidence |
|---|---|---|
| BL-M33 | During a running turn the clock's row is the screen's bottom row: the shared geometry's `Pending` budget is `1 + peer_lines` (the clock's row plus the subagent rows — no prompt row), the scrolled screen pins the same one-row clock when a run is tracked, and the geometry pins bind the one-row contract. The FR-35 acceptor's typed line renders at the live cursor position with the indicator neither suspended nor re-anchored for those renders (only the mid-turn prints suspend/resume), while the display task's ticker repaints the indicator's block in place every 100 ms — the two writers race for the bottom rows and no pin binds the typed line's location during a run. | `src/app.rs:1526–1538` (the `Pending` budget `1 + peer_lines; 2 + peer_lines` scrolled), `src/app.rs:1730` ("the clock stays the screen's bottom row"), the pins `the_clock_row_is_not_the_subagent_lines_price` (`src/app.rs:6396–6421` — `pending_clock == 1`, additive peer rows) and the wheel pin "the tracked turn pins one row" (`src/app.rs:6718–6726`), the scrolled pin's clock preference (`src/app.rs:4657–4680`), the acceptor's render sites `src/app.rs:2905/2933/2958` against the shipped suspend sites `:2876` (the cancel print) and `:2942–2944` (the submit echo), `TICK_INTERVAL` (`src/app.rs:3745`) with the `tick`/`repaint`/`clear` chain (`src/app.rs:4106–4141`, `:4152–4208`), the scrolled beat (`SCROLL_REFRESH`, 100 ms, `src/app.rs:1361–1365`; the refresh arm `:3182`) |

## 3.1 What the instruction does NOT ask for

- **Not a queue change** — FR-35's acceptance, enqueue, count display and
  drain stand untouched; the ask is the display composition during the
  running turn, not the queue's semantics.
- **Not a spinner removal** — the spinner and the elapsed time must keep
  being displayed; nothing is taken away from the run's status row.
- **Not an outcome-line change** — the `✔ <elapsed>` / `✗ <elapsed>`
  transcript lines and their recording rules stand.
- **Not a headless or piped-stdin change** — where `show_progress = false`
  or the streams are piped there is no indicator block; the ask is the
  interactive screens' bottom composition while the spinner shows.
- **Not an approval-prompt change** — `AwaitingApproval` keeps owning the
  bottom of the screen exactly as shipped (the shipped suspend around the
  approval prompt stands).
- **Not a reordering of the faces above the prompt line beyond what the
  run's block needs** — the subagent rows' shipped stacking and the
  `[queued: N]` notice keep their faces and their recording rules
  (recorded in the assumptions; the design maps the stacking).

## 4. Scope

### 4.1 In scope

- **FR-36a** — the editable prompt at the bottom-most row during a running
  turn (the live screen and the scrolled pin), the typed line persisting
  through the display beats.
- **FR-36b** — the spinner-and-elapsed display (the run's status block)
  kept and shown above the prompt block; the turn's end handing the bottom
  back to the prompt as the outcome line is recorded.
- The pinned behaviour for both — the superseded clock-geometry pins
  updated in place — and the docs/ledger updates.

### 4.2 Out of scope

- The §3.1 list; any change to the queue contracts, the provider
  contracts, the wire shapes, the exit seams, or the shipped teardown.

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-36 | While a turn runs and the interactive prompt is `Pending`, the terminal's bottom-most row is the editable prompt line and the run's spinner-and-elapsed display keeps being shown (above the prompt line). The two displays coexist with fixed places: the ticker's repaints and the acceptor's key renders stop competing for the same rows; the typed line survives the display beats. |
| FR-36a | The bottom row is the prompt: during the run, typed text renders on the screen's bottom-most line and persists — edits land there, the display task's beat never erases or overwrites the prompt block. The scrolled screen's pinned bottom block composes the same way: the prompt at the very bottom, the run's clock (and the subagent rows) above it, the marker's row riding the shipped gate. |
| FR-36b | The run's display kept: the status row (the spinner and the elapsed time) shows during the run, directly above the prompt block; the thinking rows when captured and the subagent rows keep their shipped faces and stacking; at the turn's end the block gives way to the transcript's outcome line with the prompt in place (the handoff leaves no residue on the bottom rows). |

## 6. Boundary: FR-36 vs the shipped contracts

The shipped contracts stand (NFR-14): the queue semantics (FR-35 — the
acceptor, the enqueue, the `[queued: N]` count's face and its suspended
print dance, the drain), the cancel keys' meaning, the approval takeover,
the outcome rows' recording, the four exit seams and the teardown/drain.
One shipped contract is superseded within this cycle's scope: the clock's
own-the-bottom-row geometry during a run (FR-22/FR-26a's standing record)
— on the interactive screens, while the prompt is `Pending`, the bottom
row moves to the prompt and the clock relocates above it. The shared
budgets re-derive from the same geometry helpers
(`bottom_block_rows`/`prompt_block_rows`/`body_rows`), and the superseded
pins update in place — a specification change, recorded as such (NFR-13).

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | Behaviour-binding pins: every load-bearing behaviour is pinned by a test first made to fail against the shipped shape (the geometry pins `src/app.rs:6396–6421` and `:6718–6726` bind today's one-row clock bottom — they are updated in place in the same commit that delivers the feature), then kept green. |
| NFR-14 | The changes stay additive to the shipped contracts (the boundary of §6). |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism on the installed binary, not by reading alone. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression: unit + integration counts stay green; clippy output stays identical to the pre-change baseline — re-certified at each of the cycle's commits. |
| C-12 | The instruction's method — multi-angle subagent analysis — is performed for the investigation and the repair. |
| C-13 | Write surface for the `.aiprj` documents: the three documents plus the required work logs; the product tree changes are the cycle's own instructed deliverable (FR-36 is a coding ask), recorded here without attribution. |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-33 | FR-36 implemented (a, b) and pinned; the superseded clock-geometry pins updated in place and the new pins green; gates green (unit + integration, 0 failed) and clippy identical to the baseline; the ledger consistent, the work logs written, and the delivery committed per the user's direction. |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-51 | The ask concerns the interactive screens' bottom composition while a turn runs and the prompt is `Pending`; the queued-prompt semantics (FR-35), the headless/piped paths and the approval takeover stand untouched. | Grounds FR-36's scope; verified against the shipped shapes at file:line (BL-M33). |
| AS-52 | The shipped block internals keep their faces: the status row's spinner-and-elapsed composition, the thinking rows, the subagent rows' faces and stacking, the `[queued: N]` notice, and the transcript recording rules are unchanged; the cycle re-stacks them relative to the prompt line. | Grounds FR-36b's design; verified at `src/app.rs:4127–4137` (the status row), `:4063–4101` (the thinking rows), `:1142–1148` (the peer rows). |
| AS-53 | The display's beat machinery — the 100 ms `TICK_INTERVAL` tick and the scrolled beat's `needs_refresh` — is the repaint driver the design re-anchors; no new event type and no new state machine are needed. | Grounds FR-36a's design; verified at `src/app.rs:3745`, `:1361–1365`, `:1427–1435`. |
| AS-54 | "クルクルと経過時間が表示される" names the shipped indicator block's face (the spinner-and-elapsed status row); the block's own internal order (the status row on top, the thinking rows beneath) is the shipped shape and is kept, only relocated above the prompt block. | Grounds FR-36/FR-36b; read from the instruction and verified against `src/app.rs:4152–4186`. |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-36 | design §1 (D-52), §2 | T-76 |
| FR-36a | design §1 (D-52, D-53), §3 | T-76, T-77 |
| FR-36b | design §1 (D-52, D-54), §3 | T-76 |
| NFR-13 | design §1 (D-54), §3, §4 | T-76, T-77 |
| NFR-14 | design §2, §4 | T-76 |
| AC-33 | design §1 – §5 | T-76, T-77 |

## 12. The previous cycles' record

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)** — closed, delivered at
commit `c02862d` ("Prompts typed while a turn runs are queued and run in
order when it ends"), `Cargo.toml` **0.38.0**, the tag `v0.38.0`, the
GitHub release published **2026-11-04T03:52:18Z**; the gates at the close:
**633 unit + 6 integration, 0 failed**, clippy identical to the baseline.
The mechanism: the `Pending` branch's acceptor feeding the shared
`InputState` in both REPL loops, `Enter` enqueueing
`AgentInput::UserPrompt` onto the shipped channel by the same
`send().await`, the helpers `queued_count`/`queue_release_allowed`/
`paint_queue_status` (the one-line `[queued: N]` face), the drain standing
as shipped (the loop consumes one input per iteration; the prompt returns
only when the queue is empty), the approval-draft snapshot
(`DraftSnapshot`/`snapshot_draft`/`restore_draft`), the pins rewritten —
`a_prompt_typed_during_a_turn_is_queued_and_arrives_while_it_runs`
(superseding the shipped pending-loss pin),
`stale_idle_signals_do_not_withhold_a_prompt_typed_during_a_turn`,
`approval_snapshot_roundtrip_preserves_the_half_typed_draft`,
`a_queued_prompt_survives_a_cancelled_turn_in_front_of_it`,
`queue_release_gate_answers_every_combination`,
`queued_count_tracks_a_live_channel_without_receiving`, with the FR-32 pin
`a_queued_prompt_keeps_a_flagged_agent_alive` inheriting the feature
untouched. **The cycle's post-close ledger events (2026-11-04):** the
`.aiprj` ledger brought under git tracking — commit `881eda9` "Track the
.aiprj project ledger: the three documents, work logs, rules";
the version released as **v0.39.0** — commit `4f7fad6` "Release 0.39.0:
the version carries the ledger tracking", tag `v0.39.0` pushed, the
GitHub release published **2026-11-04T04:24:19Z**; the records themselves
riding the tracked ledger — commit `96a576f` "Record the ledger-tracking
release close: the ledger events ride the tracked documents" (from here on
every ledger update travels as a normal commit), then `3af66d4` "Extend
the ledger's trail with the records-carry commit" and `5336a03` "Correct
the version trail's release labels: BL-M32" (the BL-M32 correction — the
version trail's release labels for FR-32/FR-33 were false; see below).

**Corrections standing in the ledger:** **BL-M30** (the typed-during-a-turn
prompt was lost, not queued — the shipped contract FR-35 overturned),
**BL-M31** (FR-34's close-out re-recorded as v0.37.0 — the release commit
`5a2d7f0`, the tag on it, the CHANGELOG `[0.37.0]`; the v0.37.0 GitHub
publication corrected to 2026-11-03T14:33:12Z), **BL-M32** (no `v0.35.0`
tag exists; the lightweight tag `v0.36.0` dereferences to `ddeaf67`; FR-32
`7fe1370` and FR-33 `af48be5` are the in-tree versions 0.35.0/0.36.0,
never tagged, no releases; BL-M31's deletion of the "tag `v0.36.0` on
`ddeaf67`" fact was itself corrected — that record was the true tag state
and stands restored) — each with its evidence in the cycles' records and
work logs.

**"the subagents' auto-exit — permanent (FR-32), configurable and
default-on (FR-33); the 'prompt too long' repair (FR-34)"** — closed,
2026-11-03. FR-32 (commit `7fe1370`, the version 0.35.0 delivered in-tree,
never tagged — no release — BL-M32), FR-33 (commit `af48be5`,
2026-11-03 21:36:00 +0900, version 0.36.0, equally untagged — BL-M32),
FR-34 (commit `ddeaf67`, released as **v0.37.0** — the weighted token
estimate (CJK ×3, tool-call arguments, reasoning content, the
per-message envelope) and the kept-tail truncation
(`truncate_tail_tool_results` at half the token budget, marker-annotated);
the release commit `5a2d7f0`, tag pushed, 2026-11-03 23:28:17 +0900; the
tag `v0.36.0` on `ddeaf67` stands restored — BL-M32).

**"the multi-angle subagent analysis (standing) and the subagents' self-exit
on completion" (FR-31)** — closed, 2026-11-03 (no release — investigation
and report): the four exit seams enumerated, the persistence default and
the live self-exit measured, the design space named. Superseded by the
implementation (FR-32/FR-33).

**"the multi-angle subagent analysis, added to the configuration and
verified executable" (FR-29/FR-30)** — closed, 2026-11-03 (no release):
`spawn` in the live `[tools] enabled`, the shipped `[spawn]` bounds; the
executability verdict grounded by live round trips; R-1 carried; R-2
withdrawn as false.

**"the click rule's exclusion of the elapsed display; the
cancelled-by-user display" (FR-27/FR-28)** — closed, **v0.34.0**
(`c022909`; the tag's final target `745dbb2`). **"the scroll marker's face
— 「Message below」…"** — closed, v0.33.0 (`6cc6661`). **"the agent's
running display, the subagent status display…"** — closed, v0.32.0
(`e8ad594` + `e0ecb1b`). **"the mouse defects…"** — v0.31.0 (`052e230`).
**"add strata…"** — v0.30.0–0.30.2 (`b706889`, `70e6b5b`, `a766757`).
**v0.29.0** (`045d569`). **v0.28.0** (`d237a7a`). **v0.27.0** (`afcbe70`).
**v0.25.0** (`3884d16`) with the addenda **v0.26.0** (`251a5bb`).

## 13. Delivery status

| Item | State |
|---|---|
| FR-36 (the bottom-most line during a running turn: the editable prompt; the spinner-and-elapsed kept above) | Open — the cycle's ask, recorded at this update from the rewritten instruction. The design stands (D-52–D-54, TPR-21); the tasks T-76/T-77 are ⬜, waiting for the execution session (the multi-angle subagent analysis, C-12, at the repair stage). |
| State at exit | The update session's ledger reload: the instruction reloaded byte-exact (258 bytes, sha256 `d311c050…`, mtime 2026-11-04 14:06:59 +0900), the three documents rewritten to the FR-36 cycle and mutually consistent (C-08), the work log written; the product tree untouched (`Cargo.toml` 0.39.0 — the changes are the execution session's deliverable, C-13). The ledger's updates ride this session's own normal commit on `5336a03` (the tracked ledger's rule; the sha nameable only from the ledger commit that follows it). |