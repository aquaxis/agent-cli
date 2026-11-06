# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the terminal display fix — the terminal's bottom-most line reads the
prompt while input is possible and the spinner-and-elapsed clock while the
agent runs" (FR-46 a/b/c; the standing delivery sentence of the instruction,
whose 最優先事項 names the fix)
(`.aiprj/instructions.md`, reloaded 2026-11-06 09:16:49 +0900 — sha256
recorded in §2)
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

`.aiprj/instructions.md` was rewritten twice at 2026-11-06 — first at
09:00:12 +0900 (487 bytes, sha256 `baaf4cf4…`, §2), then at 09:16:49 +0900
(699 bytes, sha256 `5cb53102…`), whose refinement sentences shape the ask's
final form. The delivery sentence 「機能追加した場合は、ドキュメント更新、
example更新、マイナーバージョンアップ、commit, push, リリースを最新バージョン
にする」 and the method sentence 「作業は多角サブエージェント解析で実施する」
stand verbatim; the ask is new: **「ターミナルの最下位行を入力が可能なときは
プロンプト表示、エージェントが実行中のときはクルクルと経過時間に修正する」**
with the refinements 「最下位行は必ず、…を表示しなければいけない」 and
「プロンプト表示は入力下の状態のときのみ表示する」 — fix the terminal's
bottom-most line: the prompt display while input is possible, the spinner
(クルクル) and elapsed time while the agent is running; the bottom-most line
always shows one of the two faces; the prompt display only in input-possible
states. The ask is **delivered** (§13): the mid-turn standing shape flipped
(FR-46a), the documents followed (FR-46b), and the completion carried (FR-46c)
— the version 0.44.0 → **0.45.0**, the delivery commit `ebe0aca` pushed, the
tag `v0.45.0`, and the release `v0.45.0 — the terminal's bottom line shows the
prompt when input is possible and the running clock while the agent runs`
published and verified **Latest**.

The previous cycle (FR-45, the prompt-input queueing feature's removal)
stands delivered and closed (§12): delivered at `b5deb9c` (v0.44.0), the
trail's follow-ups `6f0390f`/`0d45242`/`2557f7f`/`92c43fd` naming their
predecessors per the trail rule.

## 2. Source instruction

`.aiprj/instructions.md` reloaded for this execution (final revision — sha256
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

The standing delivery sentence supplies this cycle's completion shape
(documentation, example, minor version bump, commit, push, the latest
release), and the 最優先事項 names the change it completes.

## 3. Baseline — verified in the tree at this cycle's start

Verified 2026-11-06 in the working tree at `92c43fd` (the FR-45 delivery
`b5deb9c` and its ledger-trail follow-ups), before the fix wrote anything.
The baseline's full inventory stands in the delivered cycle's record
(BL-M54/BL-M55 in the tasks document's history; the file:line sites named
there — `zone_arm` `src/app.rs:1453`, `zone_beat_plan` `:1599`, `zone_beat`
`:1635`, `zone_paint_span` `:1797`, `zone_disarm_plan` `:1621`, the
arm/disarm sites in both loops, the geometry pins, the docs sentences) —
verified again at this execution's start against the same tree tip.

| ID | Fact | Evidence |
|---|---|---|
| BL-M54 | The shipped mid-turn composition pinned the prompt span at the bottom and painted the clock row above it; the span took no key input after FR-45 (the Pending key branch honours only `Esc`/`Ctrl-C` — 「All other keys during Pending are ignored」), making it a vestigial display. | `src/app.rs` at `92c43fd`; the line citations in the tasks' history record |
| BL-M55 | The 0.44.0 ground: the release `v0.44.0` Latest (`createdAt` 2026-11-05T23:18:45Z), the version files 0.44.0, the CHANGELOG's `[Unreleased]` empty; the docs sentences naming the shipped shape (README `:13`/`:26`, `doc/usage.md:77–79`/`:418`, the `/help` sentence, the architecture paragraphs); the recorded gates **663 unit + 6 integration, 0 failed**; clippy's two pre-existing dead-store warnings. | The releases API, `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, the named doc lines, the FR-45 close's recorded gates |

## 3.1 What the instruction did NOT ask for

- **No change to the provider backends, the turn machinery's face, the
  scrolled screen's shipped composition, the shutdown drain, the peer-prompt
  queue, or the approval prompt's takeover shape** (AS-73's working readings,
  with the Shell-state and scrolled-screen strict readings recorded as the
  delivery-time fold-checks the delivery satisfied by keeping those states'
  shipped shapes). Verified: the writes touched `src/app.rs`, the CHANGELOG,
  the README/usage sentences, and the version files — nothing else.
- **No history rewrite**: the CHANGELOG's earlier entries stand as history;
  the closed ledgers' records stand closed.

## 4. Scope

### 4.1 In scope (delivered)

- **FR-46a** — the bottom-most line's composition per state: while the agent
  runs, the clock block (the spinner and elapsed time, the thinking rows above
  it) ends at the screen's bottom-most row and **no prompt display paints**
  (the input-possible-states clause); while input is possible (`Ready`, the
  approval prompt), the prompt is the bottom-most line, as shipped. The zone
  arms spanless, the approval takeover keeps its span while the question
  stands, the disarm restores the prompt at every handoff, the click rule's
  exclusion pivots to the block's last row, and the geometry pins re-derive.
- **FR-46b** — the documentation follows: the CHANGELOG's `[0.45.0]` entry
  records the swap in user language; the README/usage/help sentences naming
  the shipped shape corrected; no history rewrite.
- **FR-46c** — the completion: the minor version carry 0.44.0 → **0.45.0**
  (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading), the delivery commit,
  the push, and the release published as the **latest** version.

### 4.2 Out of scope

- The §3.1 list. A git tag beyond the release's own.

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-46a | The terminal's bottom-most line is fixed per state: **while the agent runs** the bottom-most line is the clock row (spinner + elapsed, the `status_row` face) and no prompt display paints; **while input is possible** (`Ready`/`AwaitingApproval`) the prompt is the bottom-most line. The zone arms spanless (`span_top = height`), the approval prompt keeps its span while the question stands (an input-possible state — the `zone_paint_span` call stands for `Role::Confirm` only) and retires it on the answer (`zone_span_retire`), the beat's sweep covers the whole armed region, the disarm frees the clock's bottom row for the prompt at the handoff, the click rule excludes the block's LAST row (the status row) and hits the reasoning rows above it, and the pins re-derive at the flipped shape. Every site bound to its file:line evidence (BL-M54). — **Delivered** (T-105, design D-75). |
| FR-46b | The documentation follows the fix: the CHANGELOG's `[0.45.0]` entry records in user language what changed at the screen's bottom (the running clock on the bottom-most line while the agent runs, no prompt display mid-turn, the prompt back when input is possible); the shipped sentences naming the old owner corrected (`README.md:13`/`:26`, `doc/usage.md:77–79`/`:418`, the `/help` sentence `src/app.rs:5307–5308`); the `[0.40.0]`/`[0.42.0]` entries stand as history. — **Delivered** (T-106, design D-76). |
| FR-46c | The completion: the minor version carry 0.44.0 → **0.45.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's `[0.45.0]` heading), the delivery commit `ebe0aca` ("Fix the terminal's bottom line: the prompt when input is possible, the running clock while the agent runs") pushed to `origin/main` (`92c43fd..ebe0aca`), the tag `v0.45.0` verified on it, and the release published and made the **latest** version (`createdAt` 2026-11-06T00:55:05Z, `gh release list`'s Latest label). — **Delivered** (T-106, design D-77). |
| FR-45 | The previous cycle's removal — delivered and closed (§12). |

## 6. Boundary: FR-46 and the shipped contracts

The shipped contracts stand (NFR-14): the bare-`Enter` guard (FR-38), the
cancel keys and the `Esc`-to-live-view path, the approval takeover (its span
kept while the question stands, retired on the answer), the outcome rows'
recording, the four exit seams and the teardown/drain, the mouse-capture and
clipboard contracts, the peer rows' face and stacking (FR-37b), FR-42's
prompt-only rows at rest, the codex-code backend, the queueing feature's
removal (FR-45's pre-queue contract), and the scrolled screen's shipped
composition (FR-39/FR-40 — AS-73). **The four-region shape's bottom tenant
mid-turn is the one shipped statement this cycle superseded** (the
boundary's clause): the instruction's 最優先事項 named the bottom-most line's
owner, and no other shipped statement — a key path, a recording rule, the
clock's face — changed. Verified by the boundary write-surface list and the
residual suite.

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carried file:line or a live measurement before any conclusion. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | The fix stands pinned: the geometry pins re-derived at the flipped shape (the zone click rule's pin re-bound — the exclusion pivoting to the block's last row), the suite re-derived its counts at the delivery close (**663 unit + 6 integration, 0 failed**), clippy standing its two pre-existing dead-store notes in test code. |
| NFR-14 | The changes stayed bounded by the boundary of §6. |
| NFR-15 | Live-verdict (standing): the suite + clippy runs at the changed tree, the release-latest verification (`gh release list`'s Latest label + the release API's `createdAt` read for `v0.45.0`), the tag-target verification (`git rev-parse v0.45.0^{commit}` → `ebe0aca`). |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression standing: unit + integration green, clippy standing its two pre-existing dead-store notes, re-derived at the delivery tree — executed: **663 unit + 6 integration, 0 failed**. |
| C-12 | The instruction's method — multi-angle subagent analysis — stands; the analysis ran multi-angle (the design's five angles) before the sites were named, and the execution re-read the angles as the implementation demanded (the spanless-arm decision, the approval span's retention). |
| C-14 | The delivery clause executed after the fix and its documentation stood complete (the gates green first, then the CHANGELOG/version/commit/push/tag/release). |
| C-15 | Write surface: the three `.aiprj` documents, the work log, and the product files the ask names through the standing delivery sentence (`src/app.rs`, the docs sentences, `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`). Nothing else was written (one session artifact — the release notes' temp file and an edit-script remnant — was removed before the push; the amended delivery commit carries the seven product files only). |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-39 | FR-46 a/b/c met: the bottom-most line reads the prompt while input is possible and the spinner-plus-elapsed clock while the agent runs with no prompt display mid-turn, the pins re-bound (the suite green, clippy standing), the documentation written (the CHANGELOG `[0.45.0]` entry, the sentence surfaces), the version at **0.45.0**, the delivery committed and **pushed**, the release published as the **latest** version; the three `.aiprj` documents mutually consistent (C-08), the work log written per Art. 1 of the Operation Guidelines, and the instruction re-verified byte-exact at exit (NFR-12 throughout). — **Met** (§13). |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-73 | The state clauses: 「入力が可能なとき」 names `Ready` and `AwaitingApproval`; 「エージェントが実行中」 names the turn in flight. The `!` shell run keeps its shipped shape (streaming output, no prompt — sentence 3's clause already satisfied there, no clock indicated); the scrolled screen keeps its shipped composition (FR-39/FR-40). Recorded as the working reading, with the strict-universal reading noted in the design's scope angle. | Read from the instruction and the `PromptState` arms; applied in FR-46a as delivered. |
| AS-74 | The refinement sentences name the prompt display for the input-possible states only, so mid-turn no prompt display appears — the span retires from the armed composition (the vestigial affordance after FR-45). The typed line is held by the editor and re-displayed at every handoff. | Read from the final instruction text; applied in FR-46a as delivered. |
| AS-75 | 「マイナーバージョンアップ」 is the minor bump 0.44.0 → **0.45.0**; 「リリースを最新バージョンにする」 is the release published so it is the latest. | Read from the instruction and the trail; applied in FR-46c as delivered. |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-46a | design §1 (D-75), §2 (TPR-28), §3 | T-105 ✅ |
| FR-46b | design §1 (D-76), §2 (TPR-28), §3, §4 (W-70) | T-106 ✅ |
| FR-46c | design §1 (D-77), §4 (W-70) | T-106 ✅ |
| NFR-13 | design §3, §4 | T-105, T-106 ✅ |
| NFR-15 | design §3 | T-106 ✅ |
| C-09 | design §4 (W-70) | T-105, T-106 ✅ |
| C-14 | design §1 (D-77), §4 (W-70) | T-106 ✅ |
| AC-39 | design §1 – §4 | T-105, T-106, T-107 ✅ |
| FR-45 (delivered cycles) — closed | — | (closed) |

## 12. The previous cycles' record

**"the prompt-input queueing feature's removal" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c`, the version carried to **0.44.0**, the
release verified Latest, gates **663 unit + 6 integration, 0 failed**; the
trail's follow-ups `6f0390f`/`0d45242`/`2557f7f`/`92c43fd` naming their
predecessors per the trail rule. Work logs `2026-11-06_002.md`–`_006.md`.

Earlier cycles stand as recorded: the codex-code backend (FR-44 a–e,
2026-11-05, delivered at `122f320`/`4fee899`, v0.42.0/0.43.0, the release
latest); the codex access investigation (FR-43, 2026-11-05); the
refactoring and the prompt-only rows (FR-41/FR-42, 2026-11-05, v0.41.0);
the display corrections (FR-39/FR-40, 2026-11-05); the four display regions
(FR-37 a/b/c, FR-38, 2026-11-05, v0.40.0); the prompt-input queueing
(FR-35, v0.38.0 — removed by FR-45; the history record stands); the
subagents' auto-exit (FR-32–FR-34, v0.37.0); FR-31, FR-29/FR-30,
FR-27/FR-28 (v0.34.0), FR-26 (v0.33.0), FR-22–FR-24 (v0.32.0), the mouse
(v0.31.0), add strata (v0.30.0–0.30.2), and the earlier trail.

## 13. Delivery status

| Item | State |
|---|---|
| FR-46 (the terminal display fix: the bottom-most line's state-driven composition; the docs; the version 0.45.0; the commit/push; the latest release) | **Delivered** (2026-11-06): the mid-turn standing shape flipped (the zone arms spanless, the clock block's last row lands on the screen's bottom row, no prompt display mid-turn; the approval prompt keeps its span while the question stands and retires it on the answer; the disarm frees the bottom row for the prompt at every handoff; the click rule's exclusion pivots to the block's last row) per FR-46a (T-105); the CHANGELOG `[0.45.0]` entry written and the README/usage/help sentences corrected per FR-46b (T-106); the version carried 0.44.0 → **0.45.0** and the delivery commit `ebe0aca` pushed (`92c43fd..ebe0aca`), the tag `v0.45.0` verified on it (`git rev-parse v0.45.0^{commit}`), and the release `v0.45.0 — the terminal's bottom line shows the prompt when input is possible and the running clock while the agent runs` published as **Latest** (`createdAt` 2026-11-06T00:55:05Z, `gh release list`'s Latest label) per FR-46c (T-106). Gates at the close: **663 unit + 6 integration, 0 failed**, clippy standing its two pre-existing notes. |
| FR-45 (the queue's removal, the previous cycle) | **Delivered and closed** (2026-11-06; §12). |
| State at exit | The instruction (699 bytes, sha256 `5cb53102f2821fb1e2ea4da3b6e7bc335fe5f5b6a816785826e54d09beb738cb`) re-verified byte-exact at this execution's exit; the cycle is recorded in the three documents (FR-46 a/b/c, BL-M54–BL-M55, TPR-28, D-75–D-77, T-105/T-106/T-107, W-70, AC-39, AS-73–AS-75, C-08/C-09/C-12/C-14/C-15) and in the work log `2026-11-06_007.md` (embedding the instruction verbatim). The trail's follow-up commit (the one that names `ebe0aca`) stands ahead per the trail rule — a commit's sha is nameable only from the commit that follows it. |
