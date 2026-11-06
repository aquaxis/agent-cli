# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the terminal display fix — the terminal's bottom-most line reads the
prompt while input is possible and the spinner-and-elapsed clock while the
agent runs, always one of the two, the prompt display confined to the
input-possible states" (FR-46 a/b/c; the standing delivery sentence of the
instruction, whose 最優先事項 names the fix)
(`.aiprj/instructions.md`, reloaded 2026-11-06 09:00:12 and again
09:16:49 +0900 — the source text and sha256 recorded in §2; the ledgers
stand aligned to the latter revision)
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

`.aiprj/instructions.md` was rewritten twice this session: first at
2026-11-06 09:00:12 +0900 (487 bytes, sha256 `baaf4cf4…`), then again at
09:16:49 +0900 (699 bytes, sha256 `5cb53102…`, §2) — the second revision
keeping the ask's first sentence verbatim and adding two refinement
sentences. The delivery sentence 「機能追加した場合は、ドキュメント更新、
example更新、マイナーバージョンアップ、commit, push, リリースを最新バー
ジョンにする」 and the method sentence 「作業は多角サブエージェント解析で
実施する」 stand verbatim throughout. The ask is the 最優先事項
「ターミナル表示の修正」: **「ターミナルの最下位行を入力が可能なときは
プロンプト表示、エージェントが実行中のときはクルクルと経過時間に修正する」
— fix the terminal's bottom-most line: the prompt display while input is
possible, the spinner (クルクル) and elapsed time while the agent is running
— with the refinements 「最下位行は必ず、「プロンプト表示」か実行中を示す
「クルクルと経過時間」を表示しなければいけない」 (the bottom-most line must
always display one of the two) and 「プロンプト表示は入力下の状態のときのみ
表示する」 (the prompt display is shown only in the input state)**.

The shipped shape at 0.44.0 composes the bottom **oppositely** mid-turn: the
prompt span stays pinned on the screen's last rows through the turn (the
zone's own contract, D-52/D-53) and the clock row (spinner + elapsed) is
painted **directly above the span** (`zone_beat_plan` composes
`block_home = span_top − block_n`; the CHANGELOG's `[0.42.0]` record names
the shape: "The bottom-most line is the prompt's alone, and the elapsed row
stays displayed above it"). This cycle names the flip, and the refinements
make it total: while the agent runs, the **clock row takes the bottom-most
line**, the prompt display does not appear at all (the input-possible-states
clause), and the bottom-most line stands at one of the two faces in every
standing moment (the invariant). The typed line is not lost: the editor
holds it (`InputState`) and the release handoff re-displays it with the
prompt — the shipped arm shapes. The mid-turn span this removes is a
vestigial affordance after FR-45: the Pending key branch honours only
`Esc`/`Ctrl-C` (「All other keys during Pending are ignored」), so the pinned
span already takes no editing input mid-turn. The ask is **recorded
pending** (§13): per the standing delivery sentence the fix completes as the
documentation follows (FR-46b) and the minor version carries 0.44.0 →
**0.45.0** with the commit, the push and the release standing latest
(FR-46c).

The previous cycle's ask (FR-45, the prompt-input queueing feature's
removal) stands delivered and closed (§12/§13): delivered at `b5deb9c`
(v0.44.0, the release verified Latest), the trail's follow-up fulfilled by
`6f0390f`, the ledger re-aligned at `0d45242` (both naming their predecessors
per the trail rule). The queue's removal stands verified live this update:
the release `v0.44.0` Latest (`createdAt` 2026-11-05T23:18:45Z), zero
queue-mechanism identifiers left in `src/` (the `stop queued`
shutdown-drain line `src/commands.rs:178` the separate mechanism §3.1
keeps), the version files at 0.44.0.

## 2. Source instruction

`.aiprj/instructions.md` as it stands at this update's alignment (sha256
`5cb53102f2821fb1e2ea4da3b6e7bc335fe5f5b6a816785826e54d09beb738cb`,
699 bytes, mtime 2026-11-06 09:16:49 +0900):

```text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する
機能追加した場合は、ドキュメント更新、example更新、マイナーバージョンアップ、commit, push, リリースを最新バージョンにする

## 最優先事項

### ターミナル表示の修正

ターミナルの最下位行を入力が可能なときはプロンプト表示、エージェントが実行中のときはクルクルと経過時間に修正する
最下位行は必ず、「プロンプト表示」か実行中を示す「クルクルと経過時間」を表示しなければいけない
プロンプト表示は入力下の状態のときのみ表示する
```

The session's revision record: the instruction stood at 487 bytes (sha256
`baaf4cf4e2c3e394672e70c02b99a0dbd3fb305e40b5bd36aec8562bf8a1e1da`, mtime
09:00:12 +0900) when this session's first ledger write aligned to it, and
the 09:16:49 revision added the invariant sentence and the
prompt-only-when-input-possible sentence. The ledgers were re-aligned to the
final text in the same session (the work logs `2026-11-06_005.md` and
the re-load pass's own `2026-11-06_006.md` carry the two phases; the
byte-exact exit check binds the final revision). The standing delivery sentence supplies this cycle's completion
shape (documentation, example, minor version bump, commit, push, the latest
release), and the 最優先事項 names the change it completes.

## 3. Baseline — verified in the tree at this cycle's start

Verified 2026-11-06 in the working tree at `2557f7f` (the FR-45 delivery
`b5deb9c`, its follow-up `6f0390f`, the ledger re-alignment `0d45242`, and
the FR-46 recording pass `2557f7f` — the tree clean, `main` ≡
`origin/main`), before the fix writes anything:

| ID | Fact | Evidence |
|---|---|---|
| BL-M54 | **The shipped mid-turn composition pins the prompt span at the bottom and paints the clock row directly above it — and the span is an editing affordance with no live editing after FR-45.** The zone takes the live screen at a turn's start (`zone_arm` `src/app.rs:1453` — the span anchored on the screen's last row, `span_top = height − 1`, the beat's erase floor `painted_top = height − 1`) and the span paint pins the block's last row to the screen's bottom row under any clamp (`zone_paint_span` `src/app.rs:1797`, 「the span is the screen's bottom rows」); the beat composes the clock block (the spinner-and-elapsed row first, `zone_block_rows` `src/app.rs:4605`; the face `status_row` `src/app.rs:4577` — `{spinner_frame} {format_elapsed}` with `SPINNER_FRAMES` `:4157`) on the rows **directly above the span** (`zone_beat_plan` `src/app.rs:1599` — `block_home = span_top.saturating_sub(block_n)`, `sweep_to = span_top` — and `zone_beat` `src/app.rs:1635`), erasing everything above the span and never the span itself (D-53's floor). The beat parks the caret inside the span (`zone_park` `src/app.rs:1563` → `z.caret_col/caret_row`) and the span's anchor is re-anchored per edit (`zone_reanchor` `src/app.rs:1749`). The armed span paint is fed by the render path routing: while armed, `render_prompt`'s calls paint the span — the submit arm (`:3578–3579`), the command arm (`:3527–3528`), the reconcile arm (`:3690–3697`), and the piped loop's mirrored arms. **The mid-turn editing the span implies does not exist**: the Pending key branch honours only `Esc`/`Ctrl-C` — 「All other keys during Pending are ignored」 with a bare `continue` — so the pinned span takes no key input while a turn runs; it is a vestigial affordance since the queueing feature's removal (FR-45, v0.44.0). The disarm replays the record ending on `tail_floor = height − 2` (`zone_disarm_plan` `src/app.rs:1621`), leaving the fresh bottom row for the shipped prompt render (D-54) — and the release arms re-display the typed line with the prompt (the idle release `:2973–2986`, the cancel paths `:3404`/`:3434`, the approval deny `:3424–3446` — all call `render_prompt` with the current `input_state`). On the scrolled screen the pinned block keeps the shipped FR-40 composition (`scrolled_block_plan` `src/app.rs:2102` — the run row above the prompt rows) fed by `draw_scrolled` `:5219` and `render_prompt_with_run` `:5134`. The geometry pins assert the current shapes: the beat plan's four-region order (`the_beat_plan_holds_the_four_regions_in_the_instruction_order` `src/app.rs:6983`), the disarm's fresh bottom row (`the_disarm_leaves_the_bottom_row_fresh_and_the_record_ends_above_it` `:7051`), the additive block budget (`the_clock_row_is_not_the_subagent_lines_price` `:7090` — Pending reads `2`, "the prompt span and its elapsed row alone"), the wheel/drag budget agreement (`the_line_survives_the_intersections_and_the_screen_shows_it` `:7106`), and the scrolled plan's elapsed-row place (`the_scrolled_screen_keeps_the_elapsed_row_and_adds_the_marker` `:7308`). | `src/app.rs` at `2557f7f` (the lines verified identical at `0d45242` — the code the pass read); the line citations above |
| BL-M55 | **The 0.44.0 ground and the docs surface that names the current shape.** `Cargo.toml`/`Cargo.lock` at 0.44.0; the CHANGELOG's `[Unreleased]` empty and `[0.44.0]` latest; the release `v0.44.0` verified **Latest** (the releases API read this update, `createdAt` 2026-11-05T23:18:45Z; the tag on `b5deb9c`); the recorded gates at the FR-45 close **663 unit + 6 integration, 0 failed** (the product tree verified unchanged since `b5deb9c` — `src/` and `tests/` identical to the tip — so the recorded counts stand without a re-run at this update). The shipped documents that name the current bottom-line owner (the sentences the fix must follow): `README.md:13` (the peer bullet — 「the spinner's clock stays the screen's bottom row」), `README.md:26` (the wheel bullet — 「the spinner-and-elapsed clock pinned at the bottom while a turn runs」), `doc/usage.md:77–79` (the Subagents section — 「directly above the bottom block … the spinner's clock stays the screen's bottom row」), `doc/usage.md:418` (the wheel section — 「On both screens the pinned block keeps the bottom row」), the `/help` subagent sentence (`src/app.rs:5307–5308` — 「the clock stays the bottom row」), and `doc/architecture.md`'s indicator/turn-zone paragraphs. The CHANGELOG's `[0.40.0]`/`[0.42.0]` entries record the composition's history and stand as history. The example's `[ui]` block carries no bottom-line sentence (`show_progress`'s comment names only 「Activity line + spinner/elapsed while a turn runs」), so no example write exists unless the delivery contradicts it. | `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, the releases API (`createdAt`), the README/usage/architecture/help lines above, the recorded FR-45 close and the tree-identity check |

## 3.1 What the instruction did NOT ask for

- **No change to the provider backends** (the nine-kind surface, codex-code
  included): the fix is a REPL display composition. Verified: the touched
  surfaces the design names live in `src/app.rs`; no provider file appears
  in any baseline citation.
- **No change to the turn machinery itself** — the clock's face (the
  spinner frames, `format_elapsed`), the tick, the zone's transcript
  record, the disarm's replay, the cancel keys, the approval takeover, the
  peer band, the shutdown drain: the fix moves **what** the bottom-most
  line displays per state, not the clock's own drawing or the turn's run.
- **No history rewrite**: the CHANGELOG's `[0.38.0]`/`[0.40.0]`/
  `[0.42.0]`/`[0.44.0]` entries stand as history; the closed ledgers'
  records stand closed.

## 4. Scope

### 4.1 In scope (recorded pending)

- **FR-46a** — the bottom-most line's composition is state-driven and total:
  while the agent runs (a turn tracked mid-turn on the live screen), the
  clock row (spinner + elapsed) is the screen's bottom-most line and **no
  prompt display appears** (the input-possible-only clause — the prompt span
  retires from the armed composition; the typed line is held by the editor
  and returns to view with the prompt at the release); while input is
  possible (the idle prompt, the approval prompt), the prompt is the
  bottom-most line, as shipped; and in every standing moment the bottom-most
  line displays exactly one of the two faces — never blank, never stale,
  never the other state's face (the invariant; the handoff's transient
  redraw instants are below its grain). The anchor/park/caret bookkeeping
  the span carried, the beat plan, the disarm geometry and the pin set
  re-derive with the flip (NFR-13).
- **FR-46b** — the documentation follows: the CHANGELOG's `[Unreleased]`
  entry records the change in user language and carries to the release
  heading; the shipped sentences naming the current owner (BL-M55's list:
  the two README bullets, the usage.md Subagents and wheel paragraphs, the
  `/help` subagent sentence, the architecture paragraphs as the delivery
  reads them) follow the new shape; every statement bound to the code.
- **FR-46c** — the completion: the minor version carry 0.44.0 → **0.45.0**
  (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading), the delivery
  commit(s), the push to `origin/main`, and the release published as the
  **latest** version (the standing sentence's clauses; FR-44e/FR-45c's
  verified machinery).

### 4.2 Out of scope

- The §3.1 list: the provider backends, the turn machinery, and every
  shipped contract of §6 outside the bottom-line composition.
- A git tag beyond the release's own (the release machinery owns it).

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-46a | The terminal's bottom-most line is fixed, state-driven and total: **while the agent runs** (a turn in flight, the zone armed on the live screen) the bottom-most line is the clock row — the spinner (クルクル) and the turn's elapsed time, the block's own face — and **the prompt display does not appear anywhere** (「プロンプト表示は入力下の状態のときのみ表示する」: the prompt span retires from the armed screens, `zone_paint_span` unpainted mid-turn, the typed line carried by the editor and re-displayed with the prompt at the release); **while input is possible** (`Ready`, `AwaitingApproval`) the prompt is the bottom-most line, the shipped shape; and the **invariant** (「最下位行は必ず…表示しなければいけない」) holds in every standing moment: the bottom-most line is always exactly one of the two faces, never blank, never stale, never the other state's face — the zone's arming and the first beat, the handoffs and the state changes each land the correct face (the ≤100 ms beat and the queued handoff passes are the transient instants below the invariant's grain). The span-anchoring bookkeeping (`zone_reanchor`), the caret park (`zone_park`), the beat plan (`zone_beat_plan`'s `block_home`/`sweep_to`), the disarm's fresh-row floor (`zone_disarm_plan`) and the geometry pins re-derive so every redraw agrees one way, and the click rules' geometry follows the moved block. Every site bound to its file:line evidence before the change (BL-M54). — **Pending** (T-105; design §1 D-75). |
| FR-46b | The documentation follows the fix: the CHANGELOG's `[Unreleased]` entry records in user language what changed at the screen's bottom (while the agent runs the spinner-and-elapsed clock is the bottom-most line and no prompt is displayed — the line you typed returns when the turn ends; the bottom-most line always shows either the prompt or the running clock; `Esc`/`Ctrl-C` and every other shipped behaviour unchanged); the shipped sentences naming the current mid-turn owner follow the new shape (BL-M55's surface list; no history rewrite — the `[0.40.0]`/`[0.42.0]` entries stand); `/help`'s sentence follows. Every statement bound to the code the delivery pins. — **Pending** (T-106; design §1 D-76). |
| FR-46c | The completion: the minor version carry 0.44.0 → **0.45.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's heading), the delivery commit(s) of the fix and the documents (the trail rule governs the sha naming — a commit's sha is nameable only from the commit that follows it), the push to `origin/main`, and the release published and made the **latest** version (the `isLatest` probe re-asked at 0.45.0, the tag verified on the delivery commit). — **Pending** (T-106; design §1 D-77). |

## 6. Boundary: FR-46 and the shipped contracts

The shipped contracts stand (NFR-14): the bare-`Enter` guard (FR-38), the
cancel keys and the `Esc`-to-live-view path, the approval takeover, the
outcome rows' recording, the four exit seams and the teardown/drain, the
mouse-capture and clipboard contracts, the peer rows' face and stacking
(FR-37b), the refactoring's collapsed shapes FR-41, the codex-code backend
and the FR-44 additions, and the queueing feature's removal (FR-45's
pre-queue contract — the very contract that makes the mid-turn span
vestigial, BL-M54). **The four-region standing shape's bottom pair (FR-37:
the run display above the prompt span, the span pinned mid-turn) is
superseded by this cycle's FR-46 on the live screen**: the instruction's
最優先事項 names the bottom-most line's owner and its invariant, the
boundary records the flip explicitly, and no other shipped statement — a
key path, a recording rule, the clock's face or a wire shape — changes. The
scroll view's shipped composition (FR-39/FR-40) and the prompt release handoffs
ride per AS-73/AS-74's recorded readings.

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | The fix stands pinned: the geometry pins binding the shipped composition (BL-M54's list) are re-derived at the changed tree — the pins the flip overturns are re-bound to the new composition (the beat-plan order — the clock block's last row at the screen's bottom; the span-retirement's effect on the arm/disarm and park sites), the ones it does not touch stand (the disarm replay's `height−2` floor, the scrolled-side budget pins per AS-73) — and the suite re-derives its counts at the delivery close (C-09), green + clippy standing its two pre-existing dead-store notes in test code. |
| NFR-14 | The changes stay bounded by the boundary of §6: the bottom-line composition and the docs follow; no other shipped contract changes. |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism. This cycle's probes: the interactive bottom-line composition (a running turn's screen — the clock row on the bottom-most line, no prompt display, the caret bookkeeping's new shape; an idle and an approval prompt's shipped bottom line; the typed line's return at the release), the suite + clippy runs at the changed tree, and the release-latest/tag verifications at the close (`isLatest` read for `v0.45.0`, `git rev-parse` on the tag). |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression standing: unit + integration green, clippy standing its two pre-existing dead-store notes in test code, re-derived after this cycle's tree changes — pending re-execution at the fix tree (the recorded ground: **663 unit + 6 integration, 0 failed** at the FR-45 close; the product tree verified unchanged since). |
| C-12 | The instruction's method — multi-angle subagent analysis — stands, and the standing method sentence survives in the instruction verbatim. The fix's analysis ran multi-angle (design §3: the mechanism, contracts, scope, live-verdict and docs angles) before D-75 named the sites; the delivery re-runs the angles as its implementation reads demand (the Shell-state and scrolled-screen fold-checks re-asked there). |
| C-14 | The delivery clause — the minor version carry, the commit, the push, the latest release — executes after the fix and its documentation stand complete (the gates green, the CHANGELOG entry written). The delivery commits name the work they deliver. |
| C-15 | Write surface: the three `.aiprj` documents, the work log, and the product files the ask names through the standing delivery sentence (`src/app.rs` and the sentence surfaces the docs clause names — `README.md`, `doc/usage.md`, `doc/architecture.md` as read, `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`). Nothing else is written; no document carries any mark of authorship or tooling (NFR-12). |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-39 | FR-46 a/b/c met: the bottom-most line reads the prompt while input is possible and the spinner-plus-elapsed clock while the agent runs with no prompt display mid-turn (the typed line preserved and restored at the release), the invariant standing in every redraw (the bottom line always one of the two faces), every site re-derived one coherent way (BL-M54's sites), the pins re-bound and the suite green with clippy standing at the fix tree (C-09, NFR-13), the documentation written and bound to the code (the CHANGELOG entry, the sentence surfaces), the version at **0.45.0** with the release files carried, the delivery committed and **pushed to `origin/main`**, and the release published as the **latest** version; the three `.aiprj` documents mutually consistent (C-08), the work log written per Art. 1 of the Operation Guidelines, and the instruction re-verified byte-exact at exit (NFR-12 throughout). — **Pending** (§13). |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-73 | 「入力が可能なとき」 (and the refinement sentence's 「入力下の状態」) name the REPL states where the user answers at the prompt: `Ready` (the input prompt) and `AwaitingApproval` (the y/N prompt); 「エージェントが実行中」 names the agent's turn in flight — the trigger the zone and indicator already track (`TurnStart` … the terminal event, `turn_in_flight`). A `!` shell run is neither the agent running nor an input-possible state (it has no clock and `PromptState::Shell` arms no zone, `src/app.rs:3502–3507`); the invariant sentence's 必ず read strictly against it would demand a clock under the running command — the **working reading** keeps the shipped shell shape (streaming output, no prompt — sentence 3's only-in-input-states clause is already satisfied there) and records the strict reading (the clock indicating the running command) as the fold-check the delivery re-asks through C-12's read against the shipped shell machinery. The scrolled screen: the fix names the live standing composition; the scrolled view keeps its shipped composition (FR-39/FR-40 — the span pinned, the elapsed row above it) as the **working reading**, with the strict-universal reading (the flip applies there too — the clock ending the composition mid-turn) recorded as the same delivery-time fold-check against FR-40's own just-delivered record (D-61 corrected exactly that screen's clock-must-stay-displayed property). | Read from the instruction (all three sentences), the states' shipped meanings (the `PromptState` arms) and the zone's own trigger; applied in FR-46a's per-state statement and recorded in the design's scope angle with the fold-checks named. |
| AS-74 | **The refinement sentences overturned this assumption's first form.** The 09:00:12 revision (487 bytes) was first read as keeping the typed-line span visible (moved above the clock) mid-turn; the 09:16:49 revision's 「プロンプト表示は入力下の状態のときのみ表示する」 names the prompt display for the input-possible states only, so mid-turn **no prompt display appears** — the span retires from the armed composition and the bottom-most line is the clock's alone. The typed line is not lost: the editor holds it (`InputState`), and the release handoffs already re-display it with the prompt (BL-M54's release arms; the shipped `input_state` ride). The removal costs no editing affordance that exists: after FR-45 the mid-turn span takes no key input (the Pending branch ignores every key but the cancel keys) — it was a vestigial display, and the instruction replaces the vestige with the running clock. Recorded here as the revision history; applied in FR-46a. | Read from the instruction's final text and the tree's post-FR-45 key contract (BL-M54); applied in FR-46a and the design's D-75. |
| AS-75 | 「マイナーバージョンアップ」 is the minor bump 0.44.0 → **0.45.0** — the change the standing delivery sentence completes is carried by the project's trail the same way (0.41→0.42→0.43→0.44→0.45), and 「リリースを最新バージョンにする」 is the release published so it is the latest (FR-44e's own verified shape, repeated here). | Read from the instruction, the tree's trail (BL-M55) and FR-44e/FR-45c's records; applied in FR-46c (pending). |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-46a | design §1 (D-75), §2 (TPR-28), §3 | T-105 ⬜ |
| FR-46b | design §1 (D-76), §2 (TPR-28), §3, §4 (W-70) | T-106 ⬜ |
| FR-46c | design §1 (D-77), §4 (W-70) | T-106 ⬜ |
| NFR-13 | design §3 (the contracts angle), §4 | T-105, T-106 ⬜ |
| NFR-15 | design §3 (the live-verdict angle) | T-105, T-106 ⬜ |
| C-09 | design §4 (W-70) | T-105, T-106 ⬜ |
| C-15 | design §1 (D-76's surface list), §3 (the docs angle) | T-106 ⬜ |
| AC-39 | design §1 – §4 | T-105, T-106, T-107 ⬜ |
| FR-45 (delivered cycles) — closed | design §5 | (closed) |

## 12. The previous cycles' record

**"the prompt-input queueing feature's removal" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c` ("Remove the prompt-input queueing
feature", the version carried to **0.44.0**, the tag `v0.44.0`, the release
published and verified **Latest** — `createdAt` 2026-11-05T23:18:45Z, gates
**663 unit + 6 integration, 0 failed**); the trail rule fulfilled by the
follow-up `6f0390f` ("Record the queue removal's delivery in the ledger"
— it names `b5deb9c` in the three ledgers and the work log
`2026-11-06_003.md`); the ledger re-aligned at `0d45242` ("Re-align the
queue-removal ledger" — it names `6f0390f`), which corrected the design's
three `d5cbbde` sha slips to `d5cbbbe` and closed the pending follow-up
clause; the re-verified live state (the release Latest, the version files,
the residual scan) stands this cycle's ground. Work logs
`2026-11-06_002.md`–`_004.md`.

**"the codex-code backend — the installed codex CLI driven the way the
claude-code backend drives `claude`, as a ninth `provider.kind` … and the
example's sample models refreshed to the newest evidenced ids" (FR-44
a/b/c/d, FR-44e)** — closed, 2026-11-05: the backend delivered at
`122f320` (named by the follow-up `af82155`), the FR-44d amendment at
`4fee899` (named by `d5cbbbe`), the version carried to **0.42.0** then
**0.43.0**, the release `v0.43.0` published and verified **latest**
(`isLatest: true`, the tag on `4fee899` — FR-44e closed), the trail's work
logs `_010`/`_011`/`_012`. Gates at the close **669 unit + 6 integration,
0 failed**, clippy 0 warnings.

**"the codex access investigation" (FR-43 a/b/c)** — closed, 2026-11-05
(no release — the investigation): the verdict D-65 feasible for
delegation-style access, the shape D-66 recorded unbuilt; FR-44's cycle
implemented it (D-67). Work log `2026-11-05_008.md`.

**"the refactoring — contradictions and waste removed… and the prompt's
bottom row made prompt-only" (FR-41, FR-42)** — closed, 2026-11-05:
delivered at `eec12f9` and `064ce50`, the version carried to **0.41.0**
by `25fe13c` and named by the follow-up `3badc04`. Gates **647 unit + 6
integration, 0 failed**, clippy 0 warnings; work logs
`2026-11-05_006.md` and `_007.md`.

**"the display regions…" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05:
delivered at `39afeb2`, named by the follow-up `fb46cc0`; gates 645+6.
FR-39/FR-40 closed at `db68d3f`/`306f425`; the version carried to 0.40.0
(`505097a`, recorded by `856c2bf`).

**"prompt input during a running turn…" (FR-35)** — closed, 2026-11-04
(`c02862d`, v0.38.0, the tag and the release; gates 633+6). **Its
delivered feature was the FR-45 cycle's ask and stands removed** (§12's
FR-45 record): the CHANGELOG's `[0.38.0]` section stands as history.
**FR-32–FR-34** — closed, 2026-11-03 (FR-34 released **v0.37.0**, the
release commit `5a2d7f0`; FR-32 `7fe1370`, FR-33 `af48be5` — the versions
0.35.0/0.36.0 never tagged; the tag `v0.36.0` on `ddeaf67` stands
restored — BL-M32). **FR-31** — closed, 2026-11-03 (no release).
**FR-29/FR-30** — closed, 2026-11-03 (no release). **FR-27/FR-28** —
closed, **v0.34.0** (`c022909`; the tag's final target `745dbb2`).
**FR-26** — v0.33.0 (`6cc6661`). **FR-22–FR-24** — v0.32.0 (`e8ad594` +
`e0ecb1b`). **the mouse** — v0.31.0 (`052e230`). **add strata** —
v0.30.0–0.30.2 (`b706889`, `70e6b5b`, `a766757`). **v0.29.0**
(`045d569`). **v0.28.0** (`d237a7a`). **v0.27.0** (`afcbe70`).
**v0.25.0** (`3884d16`) with the addenda **v0.26.0** (`251a5bb`).

## 13. Delivery status

| Item | State |
|---|---|
| FR-46 (the terminal display fix: the bottom-most line's state-driven composition — the prompt while input is possible, the spinner-and-elapsed clock alone while the agent runs with no prompt display mid-turn, the invariant always-one-of-the-two; the docs; the version 0.45.0; the commit/push; the latest release) | **Pending** (recorded 2026-11-06): the ask reloaded twice this session (09:00:12 — 487 bytes, sha256 `baaf4cf4…`; 09:16:49 — 699 bytes, sha256 `5cb53102…`, the refinement sentences) with the baselines BL-M54/BL-M55 verified live in the tree at `0d45242`, the analysis verdicts recorded (design §3, its fold-checks on the Shell state and the scrolled screen included) and the sites named (design §1 D-75, the span's retirement included per AS-74's revised reading); the implementation rides T-105, the completion T-106, the ledger's ground T-107. |
| FR-45 (the prompt-input queueing feature's removal) | **Delivered and closed** (2026-11-06; §12): the removal at `b5deb9c` (v0.44.0, the tag, the release verified Latest `createdAt` 2026-11-05T23:18:45Z, gates **663 + 6, 0 failed**), the follow-up `6f0390f` fulfilling the trail rule, the ledger re-aligned at `0d45242`. |
| FR-44 (the codex-code backend, its amendment and the latest release) | **Delivered and closed** (2026-11-05; §12): `122f320`/`4fee899`, the versions 0.42.0/0.43.0, the release `v0.43.0` verified latest. |
| State at exit | The instruction as rewritten 2026-11-06 09:16:49 +0900 is re-verified byte-exact (699 bytes, sha256 `5cb53102…`) at each pass's exit — first at the recording pass (the ledgers written against `baaf4cf4…` were re-aligned to the refinement, caught at read-back), now re-verified again at the re-load pass (2026-11-06, the second ledger pass over the same ask: the instruction unchanged, the pending cycle FR-46 stands as recorded, the live ground re-read — the release `v0.44.0` Latest, the version files 0.44.0, the product tree unchanged since `b5deb9c`, the gates standing at the recorded close 663+6). The cycle is recorded in the three documents (FR-46 a/b/c, BL-M54–BL-M55, TPR-28, D-75–D-77, T-105/T-106/T-107, W-70 ⬜, AC-39, AS-73–AS-75, C-08/C-09/C-12/C-14/C-15) and the work logs `2026-11-06_005.md`/`_006.md`. The FR-46 delivery commit stands ahead per the trail rule — a delivery commit's sha is nameable only from the commit that follows it. |
