# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the mouse-click display defects repaired — the click leaves every
row where it stood and displays no prompt" (FR-48 a/b/c/d — **delivered**)
(`.aiprj/instructions.md`, reloaded 2026-11-06 17:03 +0900 — sha256
`aedf46b46f0c3e36638b687b3cbc62e140f19c2c778372e3b5c4b854d762e015`, 520
bytes, mtime 2026-11-06 17:06:41 +0900, the two-clause ask; recorded in §2)
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

`.aiprj/instructions.md` was rewritten at 2026-11-06 17:01:39 +0900 and
extended twice during this pass — finally at 2026-11-06 17:06:41 +0900
(520 bytes, sha256 `aedf46b4…`, §2). The two previously standing 最優先事
項 sections — 「ターミナル表示の修正」 (FR-46 a/b/c) and 「ワンショットモー
ドの追加」 (FR-47 a/b/c) — are **gone** from the instruction: both stand
delivered (FR-46 at `ebe0aca`, v0.45.0; FR-47 at `77e8a29`, v0.46.0 with
the release verified Latest; §12), and the document now carries one active
最優先事項 section with two clauses:

**「マウスクリックすると1行ズレた表示になる — マウスクリックすると1行ズレ
た表示になる、ズレない表示に改修する」** and **「マウスクリックするとプロ
ンプトを表示しないようにする」** — a mouse click shifts the display by one
row, the display is to be reworked so it does not shift, and a mouse click
is not to display the prompt. This cycle is the defect's cycle: **FR-48a**
the display holds its rows across a mouse click (the displacement
mechanism named in the user's report, evidenced at the click path's own
sites and reworked there); **FR-48b** the click path displays no prompt —
the second clause; **FR-48c** the documentation follows; **FR-48d** the
minor version carry 0.46.0 → **0.47.0** with the commit, the push, and the
release standing latest. The ask is **delivered** (§13): the live probes
first (NFR-15) found neither symptom at any tested target while naming the
decisive contract — a bare log click ended its drain pass at the tail's live
prompt re-render (`render_prompt`, the same face a wheel notch or a drag
takes), the click path's own prompt display (FR-48b, AS-86) and the only
write a no-op gesture performed; the fix removes that write (a click costs
the screen nothing: the press records the drag's anchor without painting,
the release repaints only when the gesture took a screen, and an empty
gesture's clipboard copy is skipped), the repaired face carries its pins
(the requirements' NFR-13), and the delivery sentence's completion rode the
same shape as every prior cycle: the docs follow (FR-48c), the version
carried 0.46.0 → **0.47.0**, the delivery committed and pushed, and the
release `v0.47.0` published as the latest version (FR-48d).

The standing instruction header is unchanged: the work runs 多角サブエー
ジェント解析 (the multi-angle subagent analysis, C-12), and a delivered
addition carries its documentation, the example renewal, the minor version
bump, and the commit/push/release-to-latest (C-14).

The cycle's own work stands delivered: the re-load passes (17:03/17:18
+0900) pinned the click surface at its file:line sites (BL-M58) and
re-derived the ground gates; the execution session (2026-11-06, work log
`_013`) evidenced the mechanisms (the multi-angle record, design §3),
delivered the fixes (the requirements' FR-48a/FR-48b), completed the docs,
the version carry and the release (FR-48c/FR-48d — `f9ec9ae`, v0.47.0
Latest; §13), and the FR-46/FR-47 records stand closed in §12. The pass wrote the ledgers and its work log only — no product
file, no commit (C-15, Art. 3).

## 2. Source instruction

`.aiprj/instructions.md` reloaded for this execution (sha256
`aedf46b46f0c3e36638b687b3cbc62e140f19c2c778372e3b5c4b854d762e015`,
520 bytes, mtime 2026-11-06 17:06:41 +0900; verified byte-exact at the
2026-11-06 17:03 +0900 re-load pass and again at the 2026-11-06 17:18 +0900
re-load pass's exit, byte-exact both times):

```text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する
機能追加した場合は、ドキュメント更新、example更新、マイナーバージョンアップ、commit, push, リリースを最新バージョンにする

## 最優先事項

### マウスクリックすると1行ズレた表示になる

マウスクリックすると1行ズレた表示になる、ズレない表示に改修する
マウスクリックするとプロンプトを表示しないようにする
```

The standing delivery sentence supplies this cycle's completion shape
(documentation, example, minor version bump, commit, push, the latest
release), and the newest 最優先事項 section names the defects it completes.

## 3. Baseline — verified in the tree at this cycle's start

Verified 2026-11-06 in the working tree at `8c26ecc` (HEAD of `main`,
≡ `origin/main`; the FR-47 delivery `77e8a29` and its ledger follow-ups
`1b3c7e1`/`8c26ecc` behind it):

| ID | Fact | Evidence |
|---|---|---|
| BL-M58 | **The mouse's click surface, as the shipped tree builds it.** Mouse reporting is on for the whole session when the wheel scrollback is enabled (`EnableMouseCapture` at the input loop's start, `src/app.rs:3071`; `DisableMouseCapture` once at the raw-mode guard's drop, `:1130`). The pump thread is the session's single crossterm reader; the REPL's mouse arm drains the queued burst behind the first event (`:3310–3315`) and applies three gates: the **wheel** (`state.scroll` and ScrollUp/Down, `:3317–3355` — the view moves, the prompt stays pinned), the **progress-indicator click** (`prompt_state.is_pending()` + `!overlay.is_active()` + Left `Down`, `:3361–3399` — the armed rule `zone_click_hits_clock` `:1433–1448` answers from the zone's own beat bookkeeping, else the pump round-trip `cursor_position_via_pump` `:2979` + `click_hits_indicator` `:1420–1431`; a hit calls `toggle_expanded` `:4645–4654`), and the **selection drag** (`state.scroll && state.mouse_select` + no `Shift`, `:3405–3491` — a `Down` at `row < body` begins a selection, a `Drag` extends it clamped to `body−1`, an `Up` takes it and copies it). Every mouse pass ends at the drain's repaint: `scroll_dirty` drives the alternate screen's enter/leave (`ScrollOverlay::enter` `:2149–2164`, `leave` `:2166–2179`, with `zone_hide`/`zone_show` inside each), the scrolled redraw (`draw_scroll_overlay` `:2432`, `draw_scrolled` `:5409`), or **the live prompt's re-render** (`:3807–3819`), and the copied `[clip]` report lands after the restore (`:3820–3845`). The row budgets the arms count with: `bottom_block_rows` `:2320–2335` (the arms'; the suggestion row excluded), `scrolled_bottom_rows` `:2294` (the drawer's; the suggestion row included), `scrolled_block_plan` `:2265–2288`, and `crate::scroll::body_rows` (`src/scroll.rs:255–257`). The shipped pins cover the two click rules' hit ranges and the wheel's clamps (`click_hits_indicator_covers_exactly_the_painted_block` `src/app.rs:8384+`, `wheel_range_ends_above_the_clock_row` `src/app.rs:7674+`, the scroll/select pins in `src/scroll.rs`/`src/select.rs`) — **no pin covers the display's own row positions across a click**: the suite was green while the user-side symptoms (the click shifting the display one row; the click displaying the prompt) stood user-reported, and neither symptom had an in-tree reproduction at the cycle's ground (the delivery's own record carries what the fix and its probes then established). | `src/app.rs`, `src/scroll.rs`, `src/select.rs` at `8c26ecc`; the line citations above; the suite/clippy re-run of this pass |
| BL-M59 | **The 0.46.0 ground.** `Cargo.toml`/`Cargo.lock` at 0.46.0; CHANGELOG `[0.46.0]` latest section (`:5`), the empty `[Unreleased]` holding section standing above it (`:32`); the release `v0.46.0 — the one-shot subcommand` verified **Latest** (re-read at the 13:07 +0900 pass; the product tree unchanged since); gates re-derived at this pass **663 unit + 6 integration, 0 failed**; clippy standing **three** notices (the empty-line-after-doc-comment note at `src/app.rs:967` — the delivered one-shot's doc-comment block — plus the two pre-existing dead-store notes; re-read this pass). | `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` `:5`/`:32`, the releases API, this pass's suite/clippy runs |

## 3.1 What the instruction did NOT ask for

- **No change to what the clicks and the wheel do.** The thinking view's
  toggle (its meaning and its block), the drag's copy-and-report, the
  wheel's scrollback and its marker — all stand; the asks are the display's
  face *around* them: no row may move because a click happened, and the
  click path itself may not paint the prompt.
- **The shipped standing compositions are not the ask.** The turn zone's
  clock/prompt flip (FR-46's bottom line) and the scrolled view's pinned
  block (FR-25/FR-39) are drawn by their own hands — the zone's beat and
  the drawer — not by the click path's handling; the second clause is read
  against the click path (the remedy direction AS-86), and the cycle's
  evidence-first pass (NFR-11/NFR-15) pins the exact sites before any
  repair conclusion stands.
- **No new config surface.** No new `[ui]` key is asked, and the example
  writes nothing unless the repair's own surface requires one (checked
  during the cycle).
- **No change to the REPL/peer/one-shot/provider contracts** beyond the
  click path's own face (`NFR-14`); no queueing return; no history rewrite.

## 4. Scope

### 4.1 In scope (delivered)

- **FR-48a** — the display-shifting fix: every row of the screen stands
  exactly where it stood before the click, for every click the tree
  handles (a selection's press and release, the progress block's toggle
  mid-turn, a click that starts nothing), with the click's own meaningful
  outcomes unchanged.
- **FR-48b** — the second clause: the click path displays no prompt (the
  click's own handling paints no prompt row; AS-86's remedy direction).
- **FR-48c** — the documentation follows: the CHANGELOG's `[0.47.0]` entry
  in user language naming the fixes; the README/`doc/usage.md` rows whose
  shipped wording names the affected behavior; the example check (§3.1).
- **FR-48d** — the completion: the minor version carry 0.46.0 → **0.47.0**
  (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading), the delivery
  commit(s) (the trail rule governs the recording), the push to
  `origin/main`, and the release published as the **latest** version.

### 4.2 Out of scope

- The §3.1 list: the clicks' semantics, the wheel's contract, the zone's
  flip and the scrolled pin (their own hands), the REPL/peer/one-shot/
  provider surfaces, and every shipped contract of §6 outside the click
  path's face.
- A git tag beyond the release's own.

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-48a | The display does not shift on a mouse click: after any click the tree handles — on the log (a selection's press, its drag, its release), on the progress block mid-turn (the thinking view's toggle), or a click that starts nothing — every row of the screen stands exactly where it stood before the click (the pre-click face re-derived; no row displaced by one in either direction), and the click's own meaningful outcomes (the toggle, the copy with its `[clip]` report, the wheel's scrollback) are unchanged. The displacement mechanism is evidenced at its file:line sites before any repair conclusion (NFR-11), reproduced live first (NFR-15), and the fixed behavior carries its own verifiable pin (NFR-13). — **Delivered** (T-112/T-113; design §1 D-84, the pins). |
| FR-48b | A mouse click displays no prompt: the click path's own handling — the gates' handlers and the drain's repaint tail a mouse event reaches (`src/app.rs:3777–3819`) — paints no prompt row, and the display's prompt face returns only through the hands the shipped flow already owns (the turn's end and the approval's answer; the keyboard's returns) — per AS-86's reading of the second clause. The click's own meaningful outcomes stand (FR-48a's clause). — **Delivered** (T-113; design §1 D-84). |
| FR-48c | The documentation follows: the CHANGELOG's `[0.47.0]` entry records the fixes in user language (the empty `[Unreleased]` heading stands above it); the README's and `doc/usage.md`'s rows whose shipped wording names the affected behavior (the mouse rows `README.md`, `doc/usage.md`) are updated only where the wording changes materially; the example check runs (§3.1). Every statement bound to the code it names. — **Delivered** (T-114; design §4 W-72). |
| FR-48d | The completion: the minor version carry 0.46.0 → **0.47.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's heading), the delivery commit(s) of the fixes and the records (the trail rule governs sha naming), the push to `origin/main`, and the release published and made the **latest** version (the tag verified on the delivery commit). — **Delivered** (T-114; design §4 W-72): the version carried 0.46.0 → **0.47.0** (`Cargo.toml`, `Cargo.lock` via `cargo update -p agent-cli`, the CHANGELOG's heading), the delivery commit `f9ec9ae` (\"Fix the mouse-click display: a click leaves every row where it stood and paints no prompt\") pushed (`8c26ecc..f9ec9ae`), the tag `v0.47.0` verified on it, and the release `v0.47.0 — the mouse-click display fixed` published as **Latest** (published 2026-11-06T11:37:01Z). |

## 6. Boundary: FR-48 and the shipped contracts

The shipped contracts stand (NFR-14): the four-region screen (FR-37,
FR-46), the prompt/clock flip and its bottom-most line, the wheel
scrollback and the scroll marker (FR-39), the drag select-and-copy with
its shift-override and `[clip]` report, the thinking view's toggle and its
click rules (FR-27/FR-28), the subagents' transients (FR-20's faces), the
approval takeover, the peer/IPC/registry surfaces, the one-shot mode
(FR-47) — none of them changes. The repairs are the click path's own face:
they may not re-shape what those contracts deliver, only remove the
one-row displacement a click leaves the display with and the click path's
own painting of the prompt row.

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | The repairs stand pinned: the click-path fixes carry their own pin (the display-holds-its-rows case and the no-prompt-paint case, verifiable without a live terminal wherever the mechanism allows), the shipped click/wheel pins re-derive, and the gates re-derive at the changed tree — the suite green, clippy standing its three notices (C-09). |
| NFR-14 | The changes stay bounded by the boundary of §6: the click path's face and the docs follow; no other shipped contract changes. |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism. This cycle's probes: the suite/clippy runs at the changed tree; **the live click probe** — a scripted click sequence against a live session, the screen's rows compared pre- and post-click (the displacement and the prompt-paint reproduced at the cycle's ground, then the fixed face); the release-latest/tag verification at the close. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression standing: unit + integration green, clippy standing its pre-existing notices, re-derived at this cycle's tree — **re-executed at the ground: 663 unit + 6 integration, 0 failed**; clippy re-derived at the execution session's ground (the product tree unchanged since the FR-48 delivery `f9ec9ae`): **five** distinct notices — the empty-line-after-doc-comment note at the delivered one-shot's doc-comment block `src/app.rs:967`, the two pre-existing dead-store notes `src/app.rs:8324`/`:8582`, plus the two further pre-existing notices the earlier passes' record undercounted (`unnecessary_sort_by` at `src/app.rs:2382`, `collapsible_match` at `src/agent.rs:2101` — both present verbatim at the `8c26ecc` ground and untouched by the FR-48 fix). |
| C-12 | The instruction's method — multi-angle subagent analysis — stands, and the standing method sentence survives in the instruction verbatim. The repairs' analysis runs multi-angle before the fix decision is taken (design §3). |
| C-14 | The delivery clause — the minor version carry, the commit, the push, the latest release — executed after the fixes and their documentation stand complete (the gates green, the CHANGELOG entry written, the live probe unshifted). The delivery commits name the work they deliver. |
| C-15 | Write surface: the three `.aiprj` documents, the work log, and the product files the delivery commit's own stat carries (for this cycle: the click path's surfaces the fixes name — `src/app.rs` beside `src/scroll.rs`/`src/select.rs`/`src/editor.rs`/`src/theme.rs` as the mechanism's evidence names them — plus `CHANGELOG.md`, the README/usage rows the fixes name, `Cargo.toml`, `Cargo.lock`). The transient release-notes file is removed after the `gh release create` call. No document carries any mark of authorship or tooling (NFR-12). |
| C-16 | The update passes write the three `.aiprj` documents and the work log only — no product file, no commit (the AI Update Guidelines' Article 3); the delivered trail is nameable only from a commit that follows the pass's writes (the trail rule). |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-41 | FR-48 a/b/c/d met: the mechanisms evidenced at their sites (the file:line record) and reproduced live first, the fixes delivered there, the suite green and clippy standing its three at the changed tree (C-09), the live probe showing the click leave every row where it stood and display no prompt, the documentation written and bound to the code (the CHANGELOG `[0.47.0]` entry, the README/usage rows the fixes name), the version at **0.47.0** with the release files carried, the delivery committed and **pushed to `origin/main`**, and the release published as the **latest** version; the three `.aiprj` documents mutually consistent (C-08), the work log written per Art. 6/7, and the instruction re-verified byte-exact at exit (NFR-12 throughout). — **Met** (§13). |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-82 | 「マウスクリックすると1行ズレた表示になる」 names the display's displacement: after a mouse click the screen's rows sit (approximately) one row away from their pre-click positions — the whole face shifted — and 「ズレない表示に改修する」 asks the unshifted face back. What the clicks themselves do (the thinking view's toggle, the drag's copy, the wheel's scrollback) is not the complaint and stands (§3.1). | Read from the instruction and the shipped click face (BL-M58); applied in FR-48a. |
| AS-86 | 「マウスクリックするとプロンプトを表示しないようにする」 is read as the remedy direction of the same report: the click path's own handling re-displays the prompt (the live re-render the drain's tail performs for the click's pass, `src/app.rs:3807–3819`), and the ask removes that painting — the prompt face stands as the shipped flow's hands drew it, not as the click's pass leaves it. The turn zone's flip and the scrolled pin (their own hands; §3.1) are not the clause's target; the cycle's evidence-first pass pins the exact sites (NFR-11/NFR-15) before the repair conclusion stands. | Read from the instruction (§2), the standing compositions' contracts (§6) and the click path's sites (BL-M58); applied in FR-48b. |
| AS-83 | 「マイナーバージョンアップ」 is the minor bump 0.46.0 → **0.47.0** — the trail carries the standing sentence's shape (0.45→0.46→0.47), and 「リリースを最新バージョンにする」 is the release published so it is the latest (FR-46c/FR-47c's verified shape). | Read from the instruction, the tree's trail (BL-M59) and the delivered cycles' records; applied in FR-48d. |
| AS-76 | (The FR-47 record — closed: delivered.) 「ワンショットで完結するモード」 names a mode of running one's own agent process: one prompt through the configured backend to its terminal event, with the process exiting after — not a peer-relative send, and not a transport variant. | Delivered in FR-47a (`77e8a29`; the design §5 record). |
| AS-77 | (The FR-47 record — closed: delivered.) The one-shot shares `run`'s startup options through the existing `RunArgs`; `--quiet` is the one new flag. | Delivered in FR-47a. |
| AS-78 | (The FR-47 record — closed: delivered.) 0.45.0 → **0.46.0** minor; the release published latest. | Delivered in FR-47c. |
| AS-79 | (The FR-47 record — closed: delivered.) The `--quiet` flag as delivered is **inert**: the one-shot's body prints the answer text alone (errors to stderr) with or without it; the flag stands parsed-and-not-yet-read. | Delivered in FR-47a (design §5, D-82). |
| AS-80 | (The FR-47 record — closed: delivered.) The delivered one-shot's answer path is **inline** in `run_oneshot`'s event loop (`AgentEvent::Text` flushed per delta; errors to stderr at the turn's end) — no display task, no `Theme`, no banner exists in the arm. | Re-derived at the 13:07 +0900 re-load pass (design §5, D-79). |
| AS-81 | (The FR-47 record — closed: delivered.) `doc/architecture.md` was **not** written by the FR-47 delivery — the delivered doc rows are the README's (`:29`, `:362`) and usage's (`:20`), and the CHANGELOG's `[Unreleased]` heading stands above `[0.46.0]`. | Re-derived at the 13:07 pass (design §5, D-83). |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-48a | design §1 (D-84), §2 (TPR-30), §3, §4 (W-72) | T-112 ✅, T-113 ✅ |
| FR-48b | design §1 (D-84), §2 (TPR-30), §3 | T-113 ✅ |
| FR-48c | design §3 (the docs angle), §4 (W-72) | T-114 ✅ |
| FR-48d | design §4 (W-72) | T-114 ✅ |
| NFR-13 | design §3 (the contracts angle), §4 | T-113 ✅, T-117 ✅ |
| NFR-15 | design §3 (the live-verdict angle), §4 | T-113, T-114 ✅, T-117 ✅ |
| C-09 | design §4 (W-72) | T-113, T-114 ✅, T-117 ✅ |
| C-14 | design §4 (W-72) | T-114 ✅ |
| AS-82, AS-86 | design §1 | T-116 ✅ |
| C-08 (this pass) | design §1 – §4 | T-116 ✅ |
| AC-41 | design §1 – §4 | T-113, T-114, T-116, T-117 |
| FR-47 (delivered cycle) — closed | design §5 | (closed) |
| FR-46 (delivered cycle) — closed | design §5 | (closed) |

## 12. The previous cycles' record

**"the one-shot mode added" (FR-47 a/b/c)** — closed, 2026-11-06:
delivered at `77e8a29` ("Add the one-shot subcommand: one prompt in, the
printed answer out, then exit"), the version carried 0.45.0 → **0.46.0**,
the tag `v0.46.0` verified on it, the GitHub release `v0.46.0 — the
one-shot subcommand` published and verified **Latest** (published
01:16:50Z, created 01:16:40Z); gates **663 unit + 6 integration, 0
failed** (re-derived green at the 13:07 pass and at this pass); clippy
standing its three notes. The delivered shape (the design §5 record
carries D-78–D-83): `Command::OneShot { prompt, quiet }`
(`src/cli.rs:36–46`), the dispatch (`src/main.rs:71–74`), `run_oneshot`
(`src/app.rs:969–1095`) — the headless assembly minus the IPC face, the
transient registry entry (an empty socket field, `registry_handle.
cleanup()` at both exits — D-81), the inline event loop printing the
`Text` deltas (D-82, AS-80), the parse-but-inert `--quiet` (AS-79); the
docs: the CHANGELOG's `[0.46.0]`/`### Added` entry (+12 lines;
`[Unreleased]` standing above), the README's Scriptable bullet (`:29`)
and CLI table row (`:362`), the usage row (`:20`) — `doc/architecture.md`
untouched (D-83, AS-81); the live probe returned `OK-ONEShot` and exited.
The recording follow-ups `1b3c7e1`/`8c26ecc`. The re-load passes
`2026-11-06_009.md`–`_010.md`; the cycle's ledger record `_008`.

**"the terminal display fix" (FR-46 a/b/c)** — closed, 2026-11-06:
delivered at `ebe0aca` ("Fix the terminal's bottom line: the prompt when
input is possible, the running clock while the agent runs"), the version
carried 0.44.0 → **0.45.0**, the tag `v0.45.0`, the release published and
verified **Latest** (`createdAt` 2026-11-06T00:55:05Z); gates 663+6. The
trail's passes `2557f7f`/`92c43fd` and the follow-up `5978dc9`. Work logs
`_005`–`_007`.

**"the prompt-input queueing feature's removal" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c` (v0.44.0), the follow-ups
`6f0390f`/`0d45242` recording it; gates 663+6. Work logs `_002`–`_004`.

**"the codex-code backend …" (FR-44 a/b/c/d, FR-44e)** — closed,
2026-11-05: `122f320`/`4fee899`, versions 0.42.0/0.43.0, the release
`v0.43.0` verified latest; gates 669+6. Work logs `_010`–`_012`
(2026-11-05).

**"the codex access investigation" (FR-43)** — closed, 2026-11-05 (no
release): work log `_008` (2026-11-05). **"the refactoring … the prompt's
bottom row made prompt-only" (FR-41, FR-42)** — closed, 2026-11-05:
`eec12f9`/`064ce50`, v0.41.0; gates 647+6. Work logs `_006`/`_007`.
**"the display corrections…" (FR-39, FR-40)** — closed, 2026-11-05:
`db68d3f`/`306f425`, v0.40.0 (`505097a`, named by `856c2bf`). **"the four
display regions" (FR-37, FR-38)** — `39afeb2`, named by `fb46cc0`; gates
645+6. Later records: the prompt-input queueing (FR-35, v0.38.0 —
delivered, then **removed** by FR-45); FR-32–FR-34 (v0.37.0 on
`5a2d7f0`); FR-31; FR-29/FR-30; FR-27/FR-28 (v0.34.0 on `c022909`); FR-26
(v0.33.0 on `6cc6661`); FR-22–FR-24 (v0.32.0); the mouse (v0.31.0); add
strata (v0.30.0–0.30.2); v0.29.0–v0.25.0 and the 機能追加 addenda (v0.26.0).

## 13. Delivery status

| Item | State |
|---|---|
| FR-48 (the mouse-click display fixes: the unshifted display, the click's no-prompt paint, the docs, the version 0.47.0, the commit/push, the latest release) | **Delivered** (2026-11-06): `f9ec9ae`, the tag `v0.47.0`, the release `v0.47.0 — the mouse-click display fixed` **Latest** (published 2026-11-06T11:37:01Z); gates at the fixed tree **665 unit + 6 integration, 0 failed** (the two new pins included — `a_bare_click_costs_the_screen_nothing`, `a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt`); clippy standing its three notices; the live verdicts: an at-rest click byte-identical, a real drag copies `[clip] copied 2 lines (77 bytes) via osc52` and returns to the live face, a mid-turn click (the turn provably alive) shows no prompt and no cursor drift, a scrolled click (the `Message below` marker up) paints nothing; the docs delivered per FR-48c. — details in the execution-session row below. |
| FR-48 (the mouse-click display fixes: the unshifted display, the click's no-prompt paint, the docs, the version 0.47.0, the commit/push, the latest release) | **Delivered** (2026-11-06): the click path reworked at its sites — the press records the drag's anchor without painting (`src/app.rs`, the selection gate), the release repaints only when the gesture actually put a screen up (`overlay.is_active() || overlay.view_active()` gating `scroll_dirty`; the slice read before the restore — D-84), an empty gesture skips the clipboard call, and the drain tail's unconditional prompt re-render no longer runs for a no-op click (the FR-48b face; the FR-48a face the same zero-touch pass serves); the fixed faces pinned (`a_bare_click_costs_the_screen_nothing`, `a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt`); the shipped click/wheel/drag pins re-derived green; gates at the changed tree **665 unit + 6 integration, 0 failed**, clippy standing its three notices; the live probes re-run against the fixed binary — the bare click at rest byte-identical (`identical=True`; the only captured difference the clock's own elapsed text, row 29 mid-turn, the zone's standing pose), the mid-turn A/B (control then click inside one live turn) prompt-not-displayed and cursor-fixed at `0 29` in every window, the scrolled-click (`Message below` standing) byte-identical with the marker kept, the real drag copying (`[clip] copied 2 lines (77 bytes) … osc52`) and returning to the live view, Esc returning to live as shipped; the docs: the CHANGELOG's `[0.47.0]`/`### Fixed` entry (+15 lines, surgical — the earlier 1123-deletion truncation caught at the diff and restored from git before the entry re-applied; `[Unreleased]` standing above), the README `:22`/`:550` and `doc/usage.md` `:357`/`:430` rows unchanged — the doc surface verified already truthful (the toggle's "does nothing"-row wording, the `[clip]` "selects nothing says so and copies nothing" wording, the block's own hands named); the example check ran (no config surface asked; C-15's verified absence). The version carried 0.46.0 → **0.47.0** (`Cargo.toml`/`Cargo.lock` via `cargo update -p agent-cli`/the heading); the delivery commit `f9ec9ae` ("Fix the mouse-click display: a click leaves every row where it stood and paints no prompt") pushed (`8c26ecc..f9ec9ae`); the tag `v0.47.0` verified on it; the GitHub release `v0.47.0 — the mouse-click display fixed` published as **Latest** (published 2026-11-06T11:37:01Z, verified via `gh release list` + `gh api` (`isDraft:false`, `isPrerelease:false`)). |
| The 2026-11-06 17:03 +0900 re-load pass | **Done** (this update pass): the rewritten instruction verified (441 bytes at the pass's read; sha256 `33844bff…`), then found **extended during the pass** — twice, finally at 17:06:41 +0900 (520 bytes, sha256 `aedf46b4…`; the second clause 「マウスクリックするとプロンプトを表示しないようにする」 added) — and re-verified byte-exact at the pass's exit (§2): the delivered state re-verified from the tree (HEAD `8c26ecc`, `main` ≡ `origin/main`; the version files 0.46.0; the CHANGELOG's `[Unreleased]` standing above `[0.46.0]`) with fresh gate runs (**663 unit + 6 integration, 0 failed**; clippy's three notices re-read) and the release-Latest verification carried from the 13:07 pass re-read; the mouse-click surface pinned file:line (BL-M58); the three ledgers rewritten around the pending FR-48 — both clauses scoped (FR-48 a/b/c/d, AS-86 the second clause's reading) — with the FR-46/FR-47 records closed into §12 and the concurrent writers' inconsistent ID schemes resolved (C-08); **no product file written and no commit made** (the write surface: the three `.aiprj` documents + the work log; C-15/C-16, Art. 3); the work log `2026-11-06_011.md`. |
| The 2026-11-06 17:18 +0900 re-load pass | **Done** (this update pass, the re-verification): the instruction re-read **byte-exact** (520 bytes, sha256 `aedf46b4…`, unchanged since the 17:03 pass's exit), the FR-48 pending state re-verified against the tree — HEAD `8c26ecc`, `main` ≡ `origin/main`, the version files 0.46.0, the product tree unchanged since the FR-47 delivery — with the gates re-derived live (**663 unit + 6 integration, 0 failed**; clippy's three notices re-read); the ledgers re-checked together (C-08: no live inconsistency — the 17:03 pass's FR-48 a/b/c/d scheme stands carried); **no product file written and no commit made** (the write surface: the three `.aiprj` documents + the work log; C-15/C-16, Art. 3); the work log `2026-11-06_012.md`. |
| The 2026-11-06 20:45 +0900 re-load pass | **Done** (this update pass): the instruction re-read **byte-exact** (520 bytes, sha256 `aedf46b4…`, mtime 2026-11-06 17:06:41 +0900 — unchanged since the 17:03/17:18 passes' exits and the execution session's exit); the tree re-derived at the delivered state — HEAD `e1b9118` (the FR-48 cycle's ledger follow-up, committing the ledgers and the work logs `_010`–`_013` behind the delivery `f9ec9ae`), `main` ≡ `origin/main`, the version files 0.47.0, the CHANGELOG's `[0.47.0]`/`### Fixed` entry standing under the empty `[Unreleased]` heading — and the working tree clean; the ledgers' stale exit record (the session's writes standing uncommitted, HEAD named `8c26ecc`) resolved into the committed trail (C-08: all three documents now name the trail `8c26ecc` → `f9ec9ae` → `e1b9118`, HEAD; the traceability's T-116 row supplied in the task list against its dangling reference); no product file written and no commit made (the write surface: the three `.aiprj` documents + the work log; C-15/C-16, Art. 3); the work log `2026-11-06_014.md`. |
| The 2026-11-06 execution session | **Done** (this session): T-112's probe battery (≈25 live scenarios, ollama tmux + SGR bytes — no shift/drift/prompt at any tested target; the control run held the zone's standing pose) and the mechanism decided at the static sites (D-84: the drain's live prompt re-render ran for every plain log click; any recomposition mismatch is the one-row shift face); T-113's fixes delivered at `src/app.rs` with the two pins; T-114's completion carried; the trail `8c26ecc` → `f9ec9ae` (the delivery, `v0.47.0`); gates **665 unit + 6 integration, 0 failed**; the work log `2026-11-06_013.md`. |
| The 2026-11-06 verification session | **Done** (this session): the instruction re-read byte-exact (520 bytes, sha256 `aedf46b4…`, unchanged since the delivered cycle's exits); the delivered state verified live — the tag `v0.47.0` pointing at the delivery commit `f9ec9ae`, the Releases API (`/releases/latest`) resolving `v0.47.0 — the mouse-click display fixed` as the repo's **latest** release (id 436429139, published 2026-11-06T11:37:01Z, `isDraft:false`/`isPrerelease:false`), HEAD `e1b9118` ≡ `origin/main` with the version files 0.47.0 and the CHANGELOG's `[0.47.0]`/`### Fixed` entry standing under the empty `[Unreleased]` heading; the gates re-derived at the unchanged tree — **665 unit + 6 integration, 0 failed**, clippy standing **five** distinct notices (C-09's corrected record: the two further pre-existing notices at `src/app.rs:2382`/`src/agent.rs:2101` verified verbatim at the `8c26ecc` ground); no new repair object, so the multi-angle analysis clause had no new verdict to take (the standing record: design §3, work log `_013`); no product file written, no release made (nothing added); the work log `2026-11-06_015.md`. |
| FR-47 (the one-shot mode) | **Delivered and closed** (2026-11-06; §12): `77e8a29`, v0.46.0 Latest, the follow-ups `1b3c7e1`/`8c26ecc` recording it. |
| FR-46 (the terminal display fix) / FR-45 / FR-44 / earlier | **Delivered and closed** (§12). |
| State at exit | The instruction as extended 2026-11-06 17:06:41 +0900 verified byte-exact (520 bytes, sha256 `aedf46b4…`) at the 17:03/17:18 passes' exits and **re-verified byte-exact at the execution session's exit** (§2); the FR-48 cycle **delivered** (`f9ec9ae`, v0.47.0 Latest) and recorded in the three documents (FR-48 a/b/c/d Delivered, BL-M58–BL-M59, AS-82/AS-86/AS-83, AC-41 **Met**, TPR-30, D-84, W-72 ✅, T-112–T-114 ✅, C-08/C-09/C-12/C-14/C-15/C-16). The session's ledger changes and the work logs `2026-11-06_011.md`–`_013.md` stand committed in the ledger follow-up `e1b9118` ("Record the mouse-click display fix's delivery in the ledger: the trail's follow-up", HEAD, `main` ≡ `origin/main`), per the trail rule; the delivered trail stands `77e8a29` → `1b3c7e1` → `8c26ecc` → `f9ec9ae` (the FR-48 delivery) → `e1b9118` (the ledger follow-up naming it); the verification session's clause re-verified the delivery live (the release Latest, the tag on `f9ec9ae`, the gates green). |
