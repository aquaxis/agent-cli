# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the terminal's bottom-most line is fixed — the prompt display when
input is possible, the spinner (クルクル) and its elapsed time while the agent
is running" (FR-46 a/b/c) (`.aiprj/instructions.md`, reloaded 2026-11-06
09:16:49 +0900 — sha256
`5cb53102f2821fb1e2ea4da3b6e7bc335fe5f5b6a816785826e54d09beb738cb`, 699 bytes;
the 最優先事項 supersedes the FR-45 cycle's instruction, whose delivery stands
closed)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked
Article 3 of the AI Coding Guidelines: record progress updates in this
file whenever a task changes state.

---

## Overview

The reloaded instruction's ask was one display fix: **the terminal's
bottom-most line** — 「ターミナルの最下位行を入力が可能なときはプロンプト表示、
エージェントが実行中のときはクルクルと経過時間に修正する」, with the two
refinement sentences (the bottom-most line always reads exactly one of the
two faces; the prompt display only in input-possible states). The ask is
**delivered** (T-105, T-106): the mid-turn standing shape inverted at the
zone/beat geometry, the approval takeover kept as an input-possible state's
span retired on the answer, the pins re-derived, the docs followed
(CHANGELOG `[0.45.0]`, README, usage.md, `/help`), and the completion carried
the version to **0.45.0** at the delivery commit `ebe0aca`, the tag `v0.45.0`
on it, and the GitHub release `v0.45.0 — the terminal's bottom line shows the
prompt when input is possible and the running clock while the agent runs`
published as the **latest version**. Gates at the close: **663 unit + 6
integration, 0 failed**, clippy carrying only the two pre-existing
dead-store warnings in test code.

## The cycle's tasks

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-105 | The bottom-line fix (FR-46a; design §1 D-75, §2 TPR-28, §3): flip the mid-turn standing shape — the clock block's last row on the screen's bottom row while the agent runs (`zone_arm` `src/app.rs:1453` arming spanless, `span_top = height`; `zone_beat` `:1635` composes the block at `block_home = height − block_n`), no prompt display mid-turn (the submit/command/reconcile arms stop painting the span; `render_prompt_with_run` `:5240`'s armed branch paints only the approval span), the approval takeover kept (the y/N span re-anchors; `zone_span_retire` `:1776` restores the spanless anchor on the answer), the release arms (`:2973–2986`, `:3404`, `:3434`) carrying the prompt display back at every input-possible handoff; the click rule's exclusion pivots to the block's last row (`click_hits_clock_block` `:1311`); the pins re-derive at the flipped shape (`:7085`'s click rule pin, the disarm fresh-row pin `:7051` standing, the additive budgets re-reading) | FR-46a, NFR-13, NFR-14, NFR-15, C-15 | design §1 (D-75), §2 (TPR-28), §3 | T-107's ledger ground | ✅ |
| T-106 | The completion (FR-46b, FR-46c; design §1 D-76, D-77, §4 W-70): the docs follow — the CHANGELOG's `[0.45.0]` entry recording the flipped bottom line in user language, the README's peer/wheel bullets, `doc/usage.md`'s Subagents and wheel sentences, the `/help` subagent sentence corrected to the swapped reading; the version carry 0.44.0 → **0.45.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading); the delivery commit `ebe0aca` of the fix and the documents pushed to `origin/main`; the tag `v0.45.0` verified on `ebe0aca` and the release `v0.45.0 — the terminal's bottom line shows the prompt when input is possible and the running clock while the agent runs` published as the **latest version** | FR-46b, FR-46c, NFR-11, NFR-15, C-14 | design §1 (D-76, D-77), §4 (W-70) | T-105 | ✅ |
| T-107 | The cycle's ledger ground — ✅ (the recording passes 2026-11-06 + this delivery cycle's final pass): the instruction re-verified byte-exact at each exit (699 bytes, sha256 `5cb53102…`), the three documents mutually consistent (C-08), the work logs `_005`/`_006`/`_007` (no overwrite), no document carrying any mark of authorship or tooling, the write surfaces inside the Article limits (the recording passes: the three documents + the work log; the delivery cycle: `src/app.rs` + the docs the standing sentence names + the ledgers + the work log) | — (C-08, NFR-12, C-15) | design §4 | — | ✅ |

## The execution record

- **T-105 (✅).** The fix wrote `src/app.rs` alone in the product tree:
  - `zone_arm` (`:1453`) arms spanless: `span_top = height` — the beat's
    region is the whole bottom-anchored composition and the clock block's
    last row lands on the screen's bottom row; no prompt span anchors at
    the arm;
  - `render_prompt_with_run`'s armed branch (`:5237–5246`) paints only the
    approval span (`Role::Confirm`); every other armed render call returns
    without painting the span — the input prompt's display is taken by
    input-possible states only;
  - the reconcile arm (`:3717–3741`) stops the unconditional span paint:
    it paints the span only when `prompt_state.is_awaiting_approval()`;
  - `zone_paint_span` (`:1821`) keeps its shipped shape (the approval span
    re-anchors the zone floor and reports the caret into it);
  - `zone_span_retire` (`:1776`) returns the zone to the spanless anchor on
    the approval answer (`:3514`), so the next beat takes the screen's
    bottom row back for the clock while the turn continues;
  - `zone_beat`/`zone_beat_plan` (`:1635`/`:1599`) stand parameter-neutral
    (the plan re-derives from `span_top`), the composition now ending at
    the screen's bottom row;
  - `zone_block_rows` (`:4608`) re-orders the face: the thinking rows
    above, the spinner-and-elapsed status row **last** — the bottom-most
    tenant carries the クルクルと経過時間 face;
  - `click_hits_clock_block` (`:1311`) re-derives: the exclusion pivots to
    the block's last row (the status row is no click's toggle; the rows
    above it toggle the reasoning view);
  - the disarm keeps the shipped `height−2` replay floor (D-54): the
    handoff erases the clock's bottom row and re-displays the prompt on
    the fresh one (the release arms already render it).
  Pins re-derived: the click-rule pin (`:7085`'s assertion set) re-bound to
  the flipped face; the disarm's fresh-bottom-row pin standing (`:7051`);
  the additive-budget pins re-reading (the Pending budget shape unchanged
  by the arithmetic's neutrality). Gates at the fix tree: **663 unit + 6
  integration, 0 failed**; clippy's two dead-store warnings in test code
  standing pre-existing.

- **T-106 (✅).** The completion: the CHANGELOG's `[Unreleased]` became the
  `[0.45.0]` entry recording the swap in user language; the README's peer
  bullet (`:13`) and wheel bullet (`:26`), `doc/usage.md`'s Subagents
  (`:77–79`) and wheel (`:418`) sentences, and the `/help` subagent sentence
  (`src/app.rs:5307–5308`) follow the swapped reading; the version carry
  0.44.0 → **0.45.0** (`Cargo.toml`, `Cargo.lock` via
  `cargo update -p agent-cli --precise 0.45.0`, the CHANGELOG heading); the
  delivery commit `ebe0aca` ("Fix the terminal's bottom line: the prompt
  when input is possible, the running clock while the agent runs") was
  pushed (`92c43fd..ebe0aca  main -> main`); the tag `v0.45.0` was created
  and verified pointing at `ebe0aca`
  (`ebe0acaae4135a8c230758b233a22dab9f8ccc36`); the GitHub release
  `v0.45.0 — the terminal's bottom line shows the prompt when input is
  possible and the running clock while the agent runs` was published
  (created 2026-11-06T00:55:05Z) and verified **Latest** on `gh release
  list` and the release API read.

- **T-107 (✅).** The ledgers' ground: the instruction byte-exact at each
  pass's exit (699 bytes, sha256 `5cb53102…`); the three documents mutually
  consistent on the delivered record (this pass re-aligned them together);
  the work logs `2026-11-06_005.md`/`_006.md` (the recording passes),
  `_007.md` (this delivery cycle) written at sequential indexes (no
  overwrite); no mark of authorship or tooling anywhere (NFR-12); the
  write surfaces inside the Article limits — the delivery cycle's product
  surface: `src/app.rs`, `CHANGELOG.md`, `README.md`, `doc/usage.md`,
  `Cargo.toml`, `Cargo.lock`, the version tag/release; the `.aiprj`
  surfaces: the three documents + the work log.

## Cross-cutting gates (C-09, the boundary) — ✅ closed at the delivery tree

- Unit + integration at the delivery tree: **663 unit + 6 integration,
  0 failed** — the counts re-derived (the click-rule pin re-bound; every
  other pin standing) and the suite green (C-09's re-execution clause).
- Clippy: the two dead-store warnings in test code stand pre-existing
  (verified at the FR-45 close by that cycle's `git stash` re-run); no new
  warning from the fix or the completion.
- The boundary's shipped contracts unchanged beyond the swapped bottom
  tenant: the four-region order's relative stacking, the bare-`Enter` guard
  (FR-38), the cancel keys' meanings, the approval takeover (kept; the
  input-possible y/N span with its retire-on-answer), the outcome rows'
  recording, the four exit seams and the teardown/drain, the mouse-capture
  and clipboard contracts, FR-39's marker place (the scrolled screen kept
  whole per AS-73), FR-42's prompt-only rows at rest, the queueing
  feature's removal (FR-45's pre-queue contract), the provider backends
  (NFR-14 — verified by the write-surface list and the residual suite).
- The `.aiprj` documents mutually consistent (C-08); the work logs written
  per the Operation Guidelines' Articles 6/7; the instruction byte-exact
  at each exit.

## The previous cycles' record

**"the prompt-input queueing feature is deleted" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c` (the tag `v0.44.0` on it, the release
`v0.44.0 — the prompt-input queueing feature removed` published as the
latest version, created 2026-11-05T23:18:45Z), named by the trail's
follow-up `6f0390f`; the ledger re-alignment `0d45242`; the FR-46 recording
passes `2557f7f` and `92c43fd`; gates at the close **663 unit + 6
integration, 0 failed**; work logs `_002`/`_003`/`_004`/`_005`/`_006`.

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
(`505097a`, recorded by `856c2bf`). **The bottom pair of the four-region
shape was the contract FR-46 superseded mid-turn** (the boundary's
supersession clause).

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
→ `2557f7f` (the FR-46 recording pass, naming `0d45242`) → `92c43fd` (the
re-load pass, naming `2557f7f`) → **`ebe0aca` (the FR-46 delivery; the tag
`v0.45.0` on it)** → the delivery's trail follow-up (the commit that names
`ebe0aca`) stands ahead, per the trail rule — a delivery commit's sha is
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
