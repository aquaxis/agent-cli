# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the terminal's bottom-most line is fixed — the prompt display only
while input is possible, the spinner-and-elapsed clock whenever the agent
runs, one of the two standing on the bottom line at every moment"
(FR-46 a/b/c) (`.aiprj/instructions.md`, reloaded 2026-11-06 09:16:49 +0900 —
sha256 `5cb53102f2821fb1e2ea4da3b6e7bc335fe5f5b6a816785826e54d09beb738cb`,
699 bytes; the 最優先事項 supersedes the FR-45 cycle's instruction, whose
delivery stands closed)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked
Article 3 of the AI Coding Guidelines: record progress updates in this
file whenever a task changes state.

---

## Overview

The reloaded instruction's ask is one display fix with three sentences
(the 最優先事項): 「ターミナルの最下位行を入力が可能なときはプロンプト表示、
エージェントが実行中のときはクルクルと経過時間に修正する」 — the bottom-most
line shows the prompt while input is possible and the spinner-and-elapsed
clock while the agent runs; 「最下位行は必ず、『プロンプト表示』か実行中を
示す『クルクルと経過時間』を表示しなければいけない」 — **at every moment**
the bottom-most line shows one of the two; and 「プロンプト表示は入力下の状態
のときのみ表示する」 — the prompt display is confined to the input-possible
states, so no prompt face stands mid-turn.

The shipped shape runs the first clause the other way mid-turn: the zone's
beat (`src/app.rs:1635`) composes the clock block on the rows directly above
the pinned prompt span (`zone_beat_plan` `src/app.rs:1599`), the prompt span
kept on screen through the turn. The fix inverts it: the clock block takes
the screen's bottom rows while the agent runs, the prompt takes the bottom
back the moment input is possible (the release arms already draw it there),
and mid-turn **no prompt display stands** — the editing affordance withdraws
with the queued-input removal's own contract (FR-45: mid-turn keys are
ignored). The scrolled screen keeps its shipped composition (AS-73).

The ask is **recorded pending**. C-15's write surface keeps this update
session to the ledgers + the work log; the delivery cycle (T-105, T-106)
executes the fix and the completion. The deliverables: the composition flip
at the zone/beat geometry (the span's paint retiring from the armed screens,
the erase floor grown to free the clock's bottom row), the docs follow (the
CHANGELOG's `[0.45.0]` entry, the README/usage/help sentences naming the
shipped shape), the example untouched, the minor carry 0.44.0 → **0.45.0**,
the commit, the push, and the release standing latest
(design D-75/D-76/D-77, §4 W-70).

## The cycle's tasks

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-105 | The bottom-line fix (FR-46a; design §1 D-75, §2 TPR-28, §3): flip the mid-turn standing shape — the clock block's last row on the screen's bottom row while the agent runs (`zone_beat` `src/app.rs:1635`, the plan `:1599`, the beat's floor re-derived), the prompt span's paint retired from the armed screens (`zone_paint_span` `:1797` no longer called mid-turn; no prompt display mid-turn — per AS-74), the release arms (`:2973–2986`) carrying the prompt display back at every input-possible handoff; the disarm erases the zone's whole region and frees the bottom row for the shipped prompt render at the handoff; the pins re-derive at the flipped shape (the beat plan `:6983`, the disarm fresh-row `:7051`, the budgets `:7090`, `:7106`, the scrolled-side pins per AS-73) | FR-46a, NFR-13, NFR-14, NFR-15, C-15 | design §1 (D-75), §2 (TPR-28), §3 | T-107's ledger ground | ⬜ |
| T-106 | The completion (FR-46b, FR-46c; design §1 D-76, D-77, §4 W-70): the shipped docs follow — the CHANGELOG's `[Unreleased]` becomes the `[0.45.0]` entry recording the flipped bottom line in user language (the clock at the bottom mid-turn, no prompt display while the agent runs, the prompt back when input is possible; the `[0.42.0]` entry stands as history), the README's `:13` peer-clock sentence, `doc/usage.md`'s `:77–79` clock sentence and `:418` wheel sentence, and the `/help` subagent sentence (`src/app.rs:5307–5308`) corrected to the flipped reading; the version carry 0.44.0 → **0.45.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading); the delivery commit of the fix and the documents, pushed to `origin/main`; the release published and made the **latest** version (the FR-45c machinery repeated at 0.45.0 — the `isLatest` probe re-asked) | FR-46b, FR-46c, NFR-11, NFR-15, C-14 | design §1 (D-76, D-77), §4 (W-70) | T-105 | ⬜ |
| T-107 | The cycle's ledger ground — **closed at this update** (the ledgers' write pass): the instruction re-verified byte-exact at the pass (699 bytes, sha256 `5cb53102…`), the three documents written mutually consistent (C-08) on the pending cycle (FR-46 a/b/c, BL-M54–BL-M55, TPR-28, D-75–D-77, T-105/T-106/T-107, W-70 ⬜, AC-39, AS-73–AS-75, C-08/C-09/C-12/C-14/C-15), the work log `2026-11-06_005.md` written after `_004` (no overwrite), no document carrying any mark of authorship or tooling, the write surface inside Article 3's limit (the three documents + the work log; no product file touched at the pass) | — (C-08, NFR-12, C-15) | design §4 | — | ✅ |

## The execution record

- **T-107 (✅).** The update session's ledger pass: the instruction loaded
  twice — first at `baaf4cf4…` (487 bytes, 09:00:12), then re-read at
  `5cb53102…` (699 bytes, 09:16:49 +0900) after the user amended the ask
  mid-session with the two tightening sentences (the must-display clause and
  the prompt-only-when-input clause); the ledgers were written against the
  first text, then **re-aligned byte-exact to the second text** (the
  read-back caught the drift). The three documents stand on the final ask:
  the requirements (FR-46 a/b/c, BL-M54–BL-M55, the scope §4, the
  assumptions AS-73–AS-75, the acceptance AC-39), the design (D-75/D-76/
  D-77, §2 TPR-28, §3 the five-angle verdicts, §4 W-70 ⬜), the tasks (this
  document) — mutually consistent on the pending state; the gates stand at
  the FR-45 close's **663 unit + 6 integration, 0 failed** (no product write
  at the pass; the suite re-runs at the delivery close per NFR-13); the work
  log `2026-11-06_005.md` written per the Operation Guidelines (the next
  free index after `_004`), and the pass's record committed and pushed per
  the trail convention (`2557f7f`, naming `0d45242`). A re-load pass
  (2026-11-06, this document's T-107 record re-bound) re-verified the
  instruction byte-exact (699 bytes, `5cb53102…`), the tree clean at
  `2557f7f` with `main` ≡ `origin/main`, and the work log
  `2026-11-06_006.md` written at the next free index after `_005`. The
  delivery cycle (T-105, T-106) re-runs the gates and re-verifies the
  instruction byte-exact at its exit (AC-39).

## The closed tasks of the delivered cycles (their records stand)

- **T-102/T-103/T-104** (the FR-45 removal, the completion, the ledger's
  consistency) — ✅, closed 2026-11-06: delivered at `b5deb9c` (the tag
  `v0.44.0`, the release Latest), the trail's follow-up `6f0390f` naming
  it; gates **663 unit + 6 integration, 0 failed**; the work logs
  `_002`/`_003` and the re-alignment `_004` (the pass whose own record
  rides the trail at `0d45242`, nameable from the commit that follows it).
- **T-95/T-96** (the FR-43 investigation and its ledger record) — ✅,
  closed 2026-11-05 (the requirements' §12; work log `_008`).
- **T-97/T-98/T-99** (the FR-44 backend, wiring/docs, completion) — ✅,
  closed 2026-11-05: delivered at `122f320` (named by `af82155`), the
  version carried to 0.42.0, gates **669 unit + 6 integration, 0 failed**.
- **T-100** (the FR-44d example amendment) — ✅, closed 2026-11-05:
  delivered at `4fee899` (named by the follow-up `d5cbbbe`), the version
  carried to **0.43.0**.
- **T-101** (FR-44e, the latest release) — ✅, closed on the verification
  this cycle's ground carried: the GitHub release `v0.43.0` stood published
  with `isLatest: true` programmatically read from the releases API
  (created 2026-11-05T10:57:38Z), the tag `v0.43.0` verified pointing at
  `4fee899`.

## Cross-cutting gates (C-09, the boundary) — ⬜ pending at the pass

- Unit + integration at the delivery tree: the re-derived counts stand at
  the delivery close (T-105's re-derivation; the pass stands at the
  recorded close 663+6 — no product write at the pass).
- Clippy at the delivery close: silent or the two pre-existing dead-store
  warnings (verified pre-existing at `b5deb9c`'s tip by the FR-45 cycle's
  `git stash` re-run).
- The boundary §6's shipped contracts unchanged beyond the fix's own
  surfaces (NFR-14; the clock's face, the approval takeover, the cancel
  keys, the shutdown drain, the peer-prompt queue).
- The `.aiprj` documents mutually consistent (C-08); the work log written
  per the Operation Guidelines' Article 1; the instruction byte-exact at
  exit.

## The previous cycles' record

**"the prompt-input queueing feature is deleted" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c` (the tag `v0.44.0` on it, the release
`v0.44.0 — the prompt-input queueing feature removed` published as the
latest version, created 2026-11-05T23:18:45Z), named by the trail's
follow-up `6f0390f`; gates at the close **663 unit + 6 integration,
0 failed**; the ledger re-alignment `0d45242` (the pass that bound the
point-in-time claims and corrected the design's `d5cbbde` sha slips);
work logs `_002`/`_003`/`_004`.

**"the codex-code backend — the installed codex CLI is driven the way the
claude-code backend drives `claude`, as a ninth `provider.kind`" (FR-44
a–e)** — closed, 2026-11-05: delivered at `122f320` and `4fee899` (the
FR-44d amendment), the version carried to 0.42.0 then **0.43.0**, named by
the follow-ups `af82155`/`d5cbbbe`; the release **v0.43.0 verified
latest** (T-101's record); gates **669 unit + 6 integration, 0 failed**,
clippy 0 warnings.

**"the codex access investigation" (FR-43 a/b/c)** — closed, 2026-11-05
(no release — the investigation): the verdict feasible for
delegation-style access (D-65), the shape recorded unbuilt (D-66); FR-44's
cycle built it. Work log `2026-11-05_008.md`.

**"the refactoring…; the prompt's bottom row made prompt-only" (FR-41,
FR-42)** — closed, 2026-11-05: delivered at `eec12f9` and `064ce50`, the
version carried to **0.41.0** by `25fe13c`, named by the follow-up
`3badc04`. Gates **647 unit + 6 integration, 0 failed**, clippy 0
warnings.

**"the display regions…" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05:
delivered at `39afeb2`, named by the follow-up `fb46cc0`; gates 645+6.
FR-39/FR-40 closed at `db68d3f`/`306f425`; the version carried to 0.40.0
(`505097a`, recorded by `856c2bf`).

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)** — closed, 2026-11-04
(`c02862d`, v0.38.0, the tag and the release; gates 633+6). **Its
delivered feature was deleted by the FR-45 cycle** (the FR-45a ask; the
CHANGELOG's `[0.38.0]` entry stands as history).

Version trail: FR-32 `7fe1370` (version 0.35.0, never tagged) → FR-33
`af48be5` (0.36.0, never tagged) → `ddeaf67` (FR-34's code; the tag
`v0.36.0` on it) → v0.37.0 `5a2d7f0` (FR-34) → v0.38.0 `c02862d` (FR-35)
→ `881eda9` → v0.39.0 `4f7fad6` → `96a576f` → `3af66d4` → `5336a03` →
`5257729` → `c2f78b3` → `39afeb2` → `fb46cc0` → `db68d3f` → `306f425` →
`85f9ce9` → v0.40.0 `505097a` → `856c2bf` → `eec12f9` → `064ce50` →
v0.41.0 `25fe13c` → `3badc04` → `122f320` (the codex-code backend) →
`af82155` (naming `122f320`) → `4fee899` (the FR-44d amendment; the tag
`v0.43.0` on it) → `d5cbbbe` (naming `4fee899`) → `b5deb9c` (the queue's
removal; the tag `v0.44.0` on it) → `6f0390f` (the trail's follow-up,
naming `b5deb9c`) → `0d45242` (the ledger re-alignment, naming `6f0390f`)
→ `2557f7f` (the FR-46 ledger record, naming `0d45242`) → **the FR-46
delivery commit stands ahead, per the trail rule** — a commit's sha is
nameable only from the commit that follows it.

Earlier cycles stand in the requirements document's §12 and the design
document's §5: the subagents' auto-exit (FR-32–FR-34, v0.37.0 on
`5a2d7f0`); the multi-angle subagent analysis (FR-31); FR-29/FR-30
(`spawn` in the live `[tools] enabled`); the click rule's exclusion and
the cancelled-by-user display (FR-27/FR-28, v0.34.0 on `c022909`); the
scroll marker's face (FR-26, v0.33.0 on `6cc6661`); the running display
(FR-22–FR-24, v0.32.0 on `e8ad594`); the mouse defects (FR-18–FR-21,
v0.31.0 on `052e230`); add strata (FR-13–FR-17, v0.30.0–0.30.2); the
peer-prompt cycles (v0.29.0, v0.28.0, v0.27.0); the mouse input cycle
(v0.25.0, the 機能追加 addenda v0.26.0).
