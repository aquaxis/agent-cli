# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the terminal's display regions — from the top, the scrollable and
mouse-selection-copyable log, the spinner-and-elapsed run display, the
running subagents' situation (the operating time and the processing
content), the bottom-most line reserved for the prompt — and a bare
`Enter` ignored" (FR-37, FR-38)
(`.aiprj/instructions.md`, rewritten 2026-11-04 17:08 (+0900))
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

The previous cycle (the instruction rewritten 2026-11-04 14:06:59 +0900)
recorded one display ask for the running turn — the spinner-and-elapsed
kept and the bottom-most row the editable prompt — as **FR-36** with
tasks T-76/T-77. Its documents were rewritten to the ledger at commit
`5257729`; the instruction's ask was then rewritten again (2026-11-04
17:08:59 +0900, this session's source, §2), and an execution session has
left an uncommitted implementation block in the worktree (§3, §13) — the
FR-36 cycle's execution in flight, not yet delivered.

The current instruction carries the same display ask inside a wider
composition and adds one input ask:

1. **The terminal's four display regions** (from the top): (i) the
   agent's log display — scrollable, and copyable to the clipboard by
   mouse selection; (ii) the agent's run display — the spinner and the
   elapsed time; (iii) the running subagents' situation display — their
   operating time and processing content; (iv) the bottom-most line,
   reserved for the prompt only. The display ask's FR-36 wording
   (the run's display kept, the prompt on the bottom row) is the (ii)+(iv)
   part of the composition and is carried as **FR-37a** under the
   cycle's record (§12: the FR-36 numbering superseded).
2. **The prompt-input suppression**: a bare `Enter` (Enter only, no
   input) is ignored (**FR-38**).

The shipped terminal already realizes all four regions in the
wheel-actuated scrolled composition (BL-M36) and the log is already
mouse-selection-copyable there; the open gaps are (a) the composition
**outside** the wheel-actuated scrolled region — on the live screen a
running turn still owns the bottom row (the FR-36 ask, carried in flight
by the worktree block), and (b) the peer rows name only the operating
time, not the processing content (BL-M37), plus (c) the bare-`Enter`
guard (BL-M37).

The standing method applies (C-12): investigation and repair are to be
performed by multi-angle subagent analysis, with every load-bearing
behaviour pinned (NFR-13).

## 2. Source instruction

`.aiprj/instructions.md` as reloaded in this session (rewritten
2026-11-04 17:08:59.861673867 +0900, 567 bytes; sha256
`2eba1d17d31c7855d1f20dd8a9137cd7cdfad9ec8ca9a4fa9c5f2d825dfbaa11`;
verified byte-exact):

````text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する

## 最優先事項

### ターミナルの表示領域

上から
エージェントのログなどの表示：スクロール可能、マウス選択でクリップボードにコピー可能
エージェントの実行表示：クルクルと経過時間表示
稼働しているサブエージェントのう状況表示：経過時間と処理内容など
最下位行：プロンプト専用

### プロンプト入力の抑制

エンターのみの入力は無視する
````

(Translation: **Feature additions to agent-cli. The work is to be
performed by multi-angle subagent analysis. Top priority —** **the
terminal's display regions:** from the top, (i) the agent's log display:
scrollable, copyable to the clipboard by mouse selection; (ii) the
agent's run display: the spinner and the elapsed time; (iii) the running
subagents' situation display: the operating time and the processing
content, and so on; (iv) the bottom-most line: reserved for the prompt
only. **The prompt-input suppression:** an Enter-only input is ignored.)

## 3. Baseline — verified in the tree at this update

Verified 2026-11-04 at HEAD `5257729` (the ledger's tip; `main` =
`origin/main`; `Cargo.toml` 0.39.0). The worktree additionally carries an
uncommitted execution-session addition to `src/app.rs` (+386 additive
lines, one block: the FR-36 `TurnZone` mechanism,
`src/app.rs:1328–1714` in the worktree — the previous cycle's
implementation in flight, recorded in §12/§13 and left as it stands):

| ID | Fact | Evidence |
|---|---|---|
| BL-M36 | The shipped terminal already realizes the four regions in the scroll overlay's composition: (i) the log display — the transcript the scrolled view slices and the non-scrolled stream both ride (`src/scroll.rs:1` module doc, the `raw_*` writers' tee `src/app.rs:298–329`; the scroll module pure, the wrap `:170–174`), its rows a mouse selection copies to the clipboard (`src/select.rs:1` module doc, the `Point`/`Selection` arithmetic `:27–62`; `src/clip.rs:166–167` `clip::copy`, the call site `:2775`, the copied face `clip.rs:69`); (ii) the run display — the spinner-and-elapsed status row (`status_row` `src/app.rs:4127–4129`, the frames `:4136`, `format_elapsed` `:3764–3774`) refreshed on the 100 ms tick (`TICK_INTERVAL` `:3745`, `tick` `:4106–4116`, `repaint` `:4152–4208`) and pinned on the scrolled screen (`overlay_status_row` `:4143–4150`); (iii) the subagent rows — one per live direct child, stacked oldest-top (`peer_status_rows` `:1561–1573` — the face 「peer <name> · <elapsed>」, `live_children_states` `:1610–1619`); (iv) the prompt pinned bottom-most in the scrolled composition (`ScrollBottom::Prompt`, `:1762–1770`; `draw_scroll_overlay` `:1641–1712`, the gate `:4552–4860`). | `src/app.rs`, `src/scroll.rs`, `src/select.rs`, `src/clip.rs` at `5257729` |
| BL-M37 | The two gaps against the four regions' faces and the one input gap: (a) the log region's mouse-selection copyable display exists only inside the wheel-actuated scrolled composition — on the live screen the transcript streams into the scrollback without an on-screen selectable viewport; the four-region ask is therefore the composition's extension to the non-wheel-actuated screen (the FR-36 D-52–D-54 mechanism, in flight in the worktree block). (b) the subagent rows display the operating time only — 「peer <name> · <elapsed>」 (`peer_status_rows`, `:1561–1573`); the registry entries (`src/ipc/registry.rs:11–37`) and the IPC message set (`src/ipc/mod.rs:16–…`) carry no turn/processing state, so nothing distinguishes a turn in flight. (c) a bare `Enter` is not ignored: the raw editor's `Enter` arm submits unconditionally (`handle_key`, `src/app.rs:2038–2042`, the single `input.submit()` `src/editor.rs:283–290`), and the submit arms act on the empty trim with no guard on the key itself — the Ready arm's blank-line redraw (`:3104–3108`), the Pending acceptor's `[info] nothing to queue` refusal (`:2917–2930` — already the only face an empty prompt meets) and the line-mode loop's silent discard (`:3444–3446`) are downstream of, not guards on, the submit. | `src/app.rs` at `5257729` (the worktree's `+386` block for (a)'s mechanism) |
| BL-M38 | The previous cycle (FR-36, the instruction rewritten 2026-11-04 14:06:59 +0900) was recorded with tasks T-76/T-77 and superseded by this cycle's records at the instruction's 17:08:59.861673867 +0900 rewrite: the documents' FR-36 numbering stands in the ledger's commit `5257729` (`.aiprj`-only; the FR-36 cycle never published a version or a tag), and its ask is carried under FR-37a's record here. An execution session's uncommitted `src/app.rs` block (+386 additive lines: the `TurnZone` mechanism — the prompt block parked at the input cursor, the turn's writes re-homed to the rows above it, a `DECSTBM` scroll margin, the subagent band recomposed at the screen's top rows, the anchor split of the ticker's erase/repaint) is the cycle's implementation in flight and is verified consistent with the standing shipped shape (`git diff` against `5257729`, worktree-only). | `git log`/`git diff` at 2026-11-04 (HEAD `5257729`, the `src/app.rs` worktree diff) |

## 3.1 What the instruction does NOT ask for

- **No queue-contract change** — FR-35's acceptor, enqueue, count display,
  queued-prompt semantics and drain stand untouched; the bare-`Enter`
  ask shapes the empty-submit path only, before the acceptor's arms
  (§6).
- **No removal of the shipped faces** — the spinner-and-elapsed status
  row, the thinking rows, the peer rows' name-and-operating-time face
  and the `[queued: N]` notice keep their faces; FR-37b only adds the
  processing content to the peer rows.
- **No outcome-line change** — the `✔ <elapsed>` / `✗ <elapsed>`
  transcript lines and their recording rules stand.
- **No headless or piped-stdin change** — where `show_progress = false`
  or the streams are piped there is no indicator block; the ask is the
  interactive terminal's composition.
- **No approval-prompt change** — `AwaitingApproval` keeps owning the
  bottom of the screen as shipped, and the approval `y/N` prompt's
  `Enter` semantics (an empty line reads as deny) are untouched.
- **No mouse-capture or clipboard contract change** — the session-wide
  capture and the `clip::copy` routes (`copy_command`/OSC 52) stand;
  FR-37c keeps the mechanism and its faces.

## 4. Scope

### 4.1 In scope

- **FR-37a** — the four regions' composition as the terminal's standing
  shape: on the live screen during a running turn, the log region above,
  the run display's block and the subagent rows stacked above the
  bottom-most prompt line (the FR-36 mechanism carried — the re-stack,
  the anchor split, the turn-end handoff); on the wheel-actuated
  scrolled screen the shipped pinned composition standing.
- **FR-37b** — the subagent rows' situation face: each live peer row
  carrying its operating time and its processing content.
- **FR-37c** — the log region kept scrollable and
  mouse-selection-copyable, with the shipped `clip::copy` route and the
  copied/cleared faces, inside the four-region composition.
- **FR-38** — the bare-`Enter` suppression (the raw editor's Enter
  path; the line loop and every other key preserved).
- The pinned behaviour for all and the docs/ledger updates.

### 4.2 Out of scope

- The §3.1 list; any change to the queue contracts, the provider
  contracts, the wire shapes beyond the FR-37b state feed's measured
  transport, the exit seams, or the shipped teardown.

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-37 | The interactive terminal composes, from the top: (i) the agent's log region — scrollable and mouse-selection-copyable; (ii) the agent's run display — the spinner-and-elapsed block; (iii) the running subagents' situation — one row per live peer with its operating time and processing content; (iv) the bottom-most line reserved for the prompt. The composition holds as the standing shape on the live screen (FR-37a), and its region contents are the shipped mechanisms' faces (BL-M36). |
| FR-37a | While a turn runs and the interactive prompt is `Pending` on the live screen, the bottom-most row is the editable prompt line and the run's spinner-and-elapsed block keeps being displayed above it, with the subagent rows stacked above the block and the log region streaming above them (the FR-36 D-52–D-54 mechanism carried: the re-stack, the anchor split, the turn-end handoff). No part of the running display writes on the prompt's row; the typed line survives the display beats. On the wheel-actuated scrolled screen the shipped pinned composition stands (FR-37c). |
| FR-37b | In the subagent-situation region each live peer's row carries its operating time and its processing content — what the peer is working on at the read, e.g. the run/turn state (AS-55) — beside the shipped name; the row face stays one line, cut to the width, and the rows keep their shipped FR-23/AS-26 stacking and recording. The content's absence degrades to the shipped name-and-operating-time face. |
| FR-37c | The log region is scrollable and mouse-selection-copyable: the shipped wheel (`ScrollOverlay`), the shipped drag-selection copy (`crate::clip::copy`, the `copy_command`/OSC 52 routes) and the copied/empty `[clip]` faces stand inside the FR-37 composition; nothing about the selection mechanism changes. |
| FR-38 | A bare `Enter` — the editor receiving Enter with no input typed at least once since the last submit — is ignored on the raw editor's Enter path: no submit, no state touched, no history write, no queue effect, in every prompt state the editor's Enter arm serves (`Ready`/`Pending`/`AwaitingApproval`-drafts). Any typed input (whitespace included once) keeps the shipped behaviour through the serving arms; the line-mode loop and every other key preserve their meaning (AS-56). |

## 6. Boundary: FR-37/FR-38 vs the shipped contracts

The shipped contracts stand (NFR-14): the queue semantics (FR-35 — the
acceptor, the enqueue, the `[queued: N]` count's face, the drain; the
`[info] nothing to queue` refusal remains the queue's own empty-prompt
face for a *typed* empty submit, FR-38's guard preceding it), the
cancel keys' meaning and the `Esc`-to-live-view path, the approval
takeover, the outcome rows' recording, the four exit seams and the
teardown/drain, the mouse-capture and clipboard contracts. One shipped
contract is superseded within this cycle's scope (a specification
change, recorded as such, NFR-13): the clock's own-the-bottom-row
geometry on the live screen during a run — the FR-22/FR-26a standing
record, already superseded in-flight by the FR-36 cycle's uncommitted
mechanism, now superseded at the requirements level under FR-37a — the
bottom row moving to the prompt and the run's block relocating above
it. FR-37b extends the peer rows' information content without changing
their face machinery's contract. FR-38 adds one filter on the
empty-`Enter` path: no existing key path altered.

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | Behaviour-binding pins: every load-bearing behaviour is pinned by a test first made to fail against the shipped shape (the clock-geometry pins `src/app.rs:6396–6421` and `:6718–6726` bind the one-row clock bottom — updated in place by the FR-37a delivery; no pin exists yet for FR-38's filter or the FR-37b face — new pins bind them), then kept green. |
| NFR-14 | The changes stay additive to the shipped contracts (the boundary of §6). |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism on the installed binary, not by reading alone. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression: unit + integration counts stay green; clippy output stays identical to the pre-change baseline — re-certified at each of the cycle's commits. |
| C-12 | The instruction's method — multi-angle subagent analysis — is performed for the investigation and the repair. |
| C-13 | Write surface for the `.aiprj` documents: the three documents plus the required work logs; the product tree changes are the cycle's own instructed deliverable (FR-37/FR-38 are coding asks), recorded here without attribution. |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-33 | FR-37 (a, b, c) and FR-38 implemented and pinned: the four-region composition verified on the live screen (the prompt at the bottom-most row during a run, the run's block and the peer rows above it) and standing on the scrolled screen; the peer rows carrying their processing content (degrading to the shipped face); the bare `Enter` ignored on the editor's path in all served states; the superseded clock-geometry pins updated in place and the new pins green; gates green (unit + integration, 0 failed) and clippy identical to the baseline; the ledger consistent, the work logs written, and the delivery committed per the user's direction. |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-51 | The instruction's four regions name the shipped mechanisms' contents (the transcript/log, the spinner-and-elapsed status row, the peer rows, the prompt line), whose composition on the scrolled screen is already shipped (BL-M36); the ask is the composition as the standing shape — not four new windows, not a transcript-format change. | Grounds FR-37/FR-37a's scope; read from the instruction and verified against BL-M36. |
| AS-52 | The shipped block internals keep their faces: the status row's spinner-and-elapsed composition, the thinking rows, the peer rows' name-and-operating-time face and stacking, the `[queued: N]` notice, and the transcript recording rules are unchanged; the cycle re-stacks them relative to the prompt line (carrying the FR-36 cycle's AS-52). | Grounds FR-37a/FR-37b; verified at `src/app.rs:4127–4137` (the status row), `:4063–4101` (the thinking rows), `:1142–1148` (the peer rows), `:1561–1573` (the face). |
| AS-53 | The display's beat machinery — the 100 ms `TICK_INTERVAL` tick and the scrolled beat's `needs_refresh` — is the repaint driver the composition re-anchors; no new event type and no new state machine are needed (carrying the FR-36 cycle's AS-53). | Grounds FR-37a's design; verified at `src/app.rs:3745`, `:1361–1365`, `:1427–1435`. |
| AS-54 | 「クルクルと経過時間」 names the shipped status row's face (`<spinner> <elapsed>`, `format_elapsed`'s tenths/minutes rules) unchanged inside the composition. | Grounds FR-37/FR-37a; read from the instruction and verified at `src/app.rs:4127–4129`, `:3764–3774`. |
| AS-55 | The peer rows' 「処理内容」 (processing content) is the peer's actual current turn/processing state at the read, not a substitute for the operating time (which stays) — the shipped data sources (the registry entries, `src/ipc/registry.rs:11–37`; the IPC message set, `src/ipc/mod.rs:16–…`) carry persona summaries but no turn state, so the content ride is designed against the measured transports (the IPC set or the registry, D-58): the face degrades to name+operating-time when the state is unavailable, and no wire-shape change happens without the standing contracts' boundary (NFR-14) recorded. | Grounds FR-37b's design; verified at BL-M37(b). |
| AS-56 | The bare `Enter` applies to the raw editor's Enter path: `handle_key`'s `Enter` arm (`src/app.rs:2038–2042`) and its submit sites are the suppression's place, keeping the serving arms' shipped behaviour for every non-empty line; the line-mode loop reads lines (no key event) and keeps its shipped empty-line discard (`:3444–3446`); the approval `y/N` arm's empty-reads-as-deny is untouched (the prompt state's arm, not the editor's); the scrolled overlay's `Esc|Enter` back-to-live branch (`:2813`) is not the editor's arm — the design resolves the overlap. | Grounds FR-38's scope; verified at BL-M37(c). |
| AS-57 | The gate rows W-57–W-59 (the design's §4) execute this cycle's verification: T-76/T-77 run W-57–W-59 for the FR-37a composition and the superseded clock-geometry pins; the new asks' pins (the content pin, the bare-`Enter` pin) ride the same rows' gates. | Grounds §11's design-column mapping; read from the design's §4. |
| AS-58 | The worktree's uncommitted `src/app.rs` block (the FR-36 `TurnZone` mechanism, +386 additive lines, BL-M38) is the carried FR-37a mechanism's implementation in flight: the execution session continues it or re-bases it against the current documents; this session leaves the worktree untouched (C-13's boundary — the tree changes are the coding asks' deliverable). | Grounds §3's baseline and §13's state; verified with `git diff` against `5257729` (worktree-only, additive). |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-37 | design §1 (D-56), §2, §3, §4 | T-76, T-78, T-80 |
| FR-37a | design §1 (D-52–D-54 carried, D-56), §2 (TPR-22a/22b), §4 (W-57) | T-76, T-77 |
| FR-37b | design §1 (D-58), §2 (TPR-22c), §3′′, §4 (W-58) | T-78, T-79 |
| FR-37c | design §1 (D-56), §2 (TPR-22b), §4 | T-76, T-77 |
| FR-38 | design §1 (D-59), §2 (TPR-22d), §4 (W-58′) | T-80, T-81 |
| NFR-13 | design §1, §3, §4 | T-77, T-79, T-81 |
| NFR-14 | design §3 (the boundary items) | T-76, T-78, T-80 |
| AC-33 | design §1 – §4 | T-76, T-77, T-78, T-79, T-80, T-81 |

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
as shipped, the approval-draft snapshot
(`DraftSnapshot`/`snapshot_draft`/`restore_draft`), the pins rewritten.

**"the bottom-most line during a running turn — the editable prompt on
the bottom row; the spinner-and-elapsed kept above it" (FR-36)** —
recorded 2026-11-04 at the instruction's 14:06:59 rewrite with tasks
T-76/T-77 and the baseline BL-M33 (the clock's one-row bottom geometry:
`bottom_block_rows`' `Pending` budget `1 + peer_lines`,
`src/app.rs:1526–1538`; "the clock stays the screen's bottom row",
`:1730`; the ticker/acceptor race on the bottom rows), the design
D-52–D-54 and the gate rows W-57–W-59; the instruction's 17:08:59
rewrite (this cycle) names the same display ask inside the four-region
composition, so **FR-36's requirements are carried under FR-37a's
record** (the FR-36 numbering superseded; the design's D-52–D-54
carried), and an execution session's uncommitted `TurnZone` block
(`src/app.rs`, +386 additive) stands in the worktree as the
implementation in flight (BL-M38, §13). T-76/T-77 are restated here as
this cycle's tasks (the task list's table).

**The post-close ledger events (2026-11-04):** the `.aiprj` ledger
brought under git tracking — commit `881eda9` "Track the .aiprj project
ledger: the three documents, work logs, rules"; the version released as
**v0.39.0** — commit `4f7fad6` "Release 0.39.0: the version carries the
ledger tracking", tag `v0.39.0` pushed, the GitHub release published
**2026-11-04T04:24:19Z**; the records themselves riding the tracked
ledger — `96a576f` "Record the ledger-tracking release close: the ledger
events ride the tracked documents", `3af66d4` "Extend the ledger's trail
with the records-carry commit", `5336a03` "Correct the version trail's
release labels: BL-M32", `5257729` "Rewrite the ledger to the FR-36
cycle: the prompt on the bottom row" (the FR-36 cycle's design-stage
rewrite committing the FR-36 documents and T-76/T-77's restatement as
they then stood).

**Corrections standing in the ledger:** **BL-M30** (the typed-during-a-turn
prompt was lost, not queued — the shipped contract FR-35 overturned),
**BL-M31** (FR-34's close-out re-recorded as v0.37.0 — the release commit
`5a2d7f0`, the tag on it, the CHANGELOG `[0.37.0]`; the v0.37.0 GitHub
publication corrected to 2026-11-03T14:33:12Z), **BL-M32** (no `v0.35.0`
tag exists; the lightweight tag `v0.36.0` dereferences to `ddeaf67`;
FR-32 `7fe1370` and FR-33 `af48be5` are the in-tree versions
0.35.0/0.36.0, never tagged, no releases) — each with its evidence in the
cycles' records and work logs.

**"the subagents' auto-exit — permanent (FR-32), configurable and
default-on (FR-33); the 'prompt too long' repair (FR-34)"** — closed,
2026-11-03. FR-32 (commit `7fe1370`), FR-33 (commit `af48be5`), FR-34
(commit `ddeaf67`, released as **v0.37.0** — the release commit
`5a2d7f0`).

**"the multi-angle subagent analysis (standing) and the subagents' self-exit
on completion" (FR-31)** — closed, 2026-11-03 (no release — investigation
and report). Superseded by the implementation (FR-32/FR-33).

**"the multi-angle subagent analysis, added to the configuration and
verified executable" (FR-29/FR-30)** — closed, 2026-11-03 (no release):
`spawn` in the live `[tools] enabled`, the shipped `[spawn]` bounds.

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
| FR-37 (the four display regions; a, b, c) | Open — the cycle's display ask, recorded at this update from the rewritten instruction. The design stands (D-56 wrapping the carried D-52–D-54, D-58); the tasks T-76/T-77 (a: carried in flight) and T-78/T-79 (b) are ⬜. The baseline (BL-M36) carries the shipped mechanisms' file:line evidence. The instruction's four-region wording is the FR-36 display ask's wider statement — the requirements carry it as FR-37a (D-52–D-54 carried), with FR-37b the peer rows' processing-content ask and FR-37c the shipped scrollable/mouse-copyable log region standing. |
| FR-38 (the bare-`Enter` suppression) | Open — recorded with FR-37 from the same instruction; D-59, T-80 and T-81 ⬜; the baseline (BL-M37(c)) carries the shipped submit path's file:line evidence. |
| FR-36 (the previous cycle's display ask) | Superseded by FR-37a's record (BL-M38) — T-76/T-77 restated as this cycle's tasks; the worktree's uncommitted `TurnZone` block is the FR-36 ask's implementation in flight, left untouched by this session and continued or re-based by the execution session against the current documents. |
| State at exit | The update session's ledger reload: the instruction reloaded byte-exact (567 bytes, sha256 `2eba1d17…`, mtime 2026-11-04 17:08:59.861673867 +0900), the three documents rewritten to the FR-37/FR-38 cycle and mutually consistent (C-08), the work log written; the product tree untouched by this session (`Cargo.toml` 0.39.0 — the changes are the execution session's deliverable, C-13). The ledger's updates ride this session's own normal commit on `5257729` (the tracked ledger's rule; the sha nameable only from the ledger commit that follows it). |