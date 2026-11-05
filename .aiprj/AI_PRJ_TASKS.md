# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the prompt-input queueing feature is deleted — prompts typed during a
running turn are again ignored (only the cancel keys act), the deletion is
documented, the version carries to 0.44.0, and the release stands the latest
version" (FR-45 a/b/c) (`.aiprj/instructions.md`, reloaded 2026-11-06
02:02:15 +0900 — sha256 `066203190b3c5fc30e3a43e1cd09819998bd462886f9eb5a87f4757c591a4e0b`,
349 bytes; the 最優先事項 「プロンプトの入力のキューイング機能を削除する」
supersedes the FR-44 cycle's instruction, whose delivery stands closed)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked
Article 3 of the AI Coding Guidelines: record progress updates in this
file whenever a task changes state.

---

## Overview

The reloaded instruction's ask is one removal: **delete the prompt-input
queueing feature** — the feature FR-35 delivered (v0.38.0, `c02862d`):
prompts typed while a turn runs are accepted, edited and enqueued onto the
agent's input channel, and run in order once the running turn completes
(the CHANGELOG's `[0.38.0]` record). The removal surface the requirements'
§4/§5 name: the Pending submit arm's echo + `UserPrompt` enqueue + the
`!`/`/` refusals, the `[queued: N]` notice, `queued_count`/`queue_release_allowed`,
the approval-time draft pair, the queue's four unit pins and the
agent-side boundary pin. The queue channel itself (`AgentInput` FIFO), the
shutdown drain answering queued deliveries truthfully, the peer-prompt
queue and the four-region screen stand untouched; the pre-queue contract
is restored. The delivery rides the standing sentence: the CHANGELOG's
`[0.44.0]` entry, the minor carry 0.43.0 → **0.44.0**, the commit, the
push, and the release standing the latest version (design D-73/D-74,
§4 W-69).

**The ask is recorded, not yet delivered.** No removal write has reached
the tree at this update (the tree stands at the v0.43.0 ground,
BL-M52/BL-M53): T-102 carries the removal, T-103 the completion, T-104 the
ledger's own consistency at the delivery cycle's exit.

## The cycle's tasks

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-102 | The removal (FR-45a; design §1 D-72, §2 TPR-27, §3): delete the queue's mechanism at its named sites — the raw loop's Pending submit arm (`src/app.rs:3505–3609`, the enqueue `:3595`, the `!`/`/` refusals `:3511`), the piped loop's mirror arms, the helpers `queued_count` (`:1320`)/`queue_release_allowed` (`:1335`)/`paint_queue_status` (`:1345`)/`QUEUED_NOTICE_PREFIX` (`:1364`), the idle release arm's queue conjuncts (`:3070–3098`), the approval-time draft pair (`DraftSnapshot`/`snapshot_draft`/`restore_draft` `:2610–2624`, the arms `:3131–3134`/`:3715`), the four unit pins (`:6634`, `:6707`, `:8815`, `:8836`) and `src/agent.rs`'s boundary pin (`:1419`, the FR-35b comment `:1457`) — during a running turn the loop ignores plain typed input (only `Esc`/`Ctrl-C` act) and `Pending` releases on the turn's terminal event; the suite re-derives its counts (669 − 4 unit) and stands green, clippy silent | FR-45a, NFR-13, NFR-14, NFR-15, C-09 | design §1 (D-72), §2 (TPR-27), §3 | T-104's ledger ground | ⬜ |
| T-103 | The completion (FR-45b, FR-45c; design §1 D-73, D-74, §4 W-69): the CHANGELOG's `[Unreleased]` becomes the `[0.44.0]` entry recording the removal in user language (the shipped READMEs/usage carry no queue sentence to correct — BL-M53's verified absence); the version carry 0.43.0 → **0.44.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading); the delivery commit(s) of the removal and the records, pushed to `origin/main`; the release published and made the **latest** version (the `isLatest` probe re-asked at 0.44.0 per D-74/D-71) | FR-45b, FR-45c, NFR-11, C-14 | design §1 (D-73, D-74), §4 (W-69) | T-102 | ⬜ |
| T-104 | The ledger's own consistency and the byte-exact exit check (— (C-08, NFR-12, C-13, AC-38); design §4): the three documents stand mutually consistent (written this update to the reloaded instruction; re-checked at the delivery close), the work log written per Art. 6/7 after `2026-11-06_001.md` (no overwrite), the instruction re-verified byte-exact at exit (349 bytes, sha256 `06620319…`), no document carrying any mark of authorship or tooling, the write surface inside Article 3's limit | — (C-08, NFR-12, C-13, AC-38) | design §4 | T-102, T-103 | 🟨 |

## The closed tasks of the delivered cycles (their records stand)

- **T-95/T-96** (the FR-43 investigation and its ledger record) — ✅,
  closed 2026-11-05 (the requirements' §12; work log `_008`).
- **T-97/T-98/T-99** (the FR-44 backend, wiring/docs, completion) — ✅,
  closed 2026-11-05: delivered at `122f320` (named by `af82155`), the
  version carried to 0.42.0, gates **669 unit + 6 integration, 0 failed**.
- **T-100** (the FR-44d example amendment) — ✅, closed 2026-11-05:
  delivered at `4fee899` (named by the follow-up `d5cbbbe`), the version
  carried to **0.43.0**.
- **T-101** (FR-44e, the latest release) — ✅, **closed on this update's
  own verification**: the GitHub release `v0.43.0` stands published with
  `isLatest: true` programmatically read from the releases API (created
  2026-11-05T10:57:38Z; `isLatest: false` for `v0.42.0`/`v0.41.0`), the
  tag `v0.43.0` verified pointing at the delivery commit `4fee899`
  (`git rev-parse v0.43.0^{commit}`), URL
  `https://github.com/aquaxis/agent-cli/releases/tag/v0.43.0`. The
  previous ledger carried no programmatic verification; this update's
  record establishes it. FR-44e's requirement is discharged.

## Cross-cutting gates (C-09, the boundary)

- Unit + integration at the removal tree: green, clippy silent (the gates
  re-run at T-102's close; the counts re-derive — 669 − 4 unit pins —
  per NFR-13), clippy **0 warnings**.
- The boundary §6's shipped contracts unchanged: the queue **channel**
  (`AgentInput` FIFO), the shutdown drain's truthful answers
  (`doc/architecture.md` §7 — a sentence the removal must not touch),
  the peer-prompt queue, the approval takeover, the four-region screen,
  and every closed cycle's delivered shape stand; T-102's deletions touch
  only the queue feature itself (NFR-14).
- The `.aiprj` documents mutually consistent (C-08); the work log written
  per Art. 6/7; the instruction byte-exact at exit.

## The previous cycles' record

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

**"the refactoring — contradictions and waste removed, the documents
followed to the source; the prompt's bottom row made prompt-only" (FR-41,
FR-42)** — closed, 2026-11-05: delivered at `eec12f9` and `064ce50`, the
version carried to **0.41.0** by `25fe13c`, named by the follow-up
`3badc04`. Gates **647 unit + 6 integration, 0 failed**, clippy 0
warnings; work logs `2026-11-05_006.md` and `_007.md`.

**"the display regions…" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05:
delivered at `39afeb2`, named by the follow-up `fb46cc0`; gates 645+6.
FR-39/FR-40 closed at `db68d3f`/`306f425`; the version carried to 0.40.0
(`505097a`, recorded by `856c2bf`).

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)** — closed, 2026-11-04
(`c02862d`, v0.38.0, the tag and the release; gates 633+6). **Its
delivered feature is the ask this cycle deletes** (FR-45a; the boundary
§6's supersession clause): the feature ships at v0.43.0 (BL-M52) and the
removal reinstates the pre-queue contract the CHANGELOG's `[0.38.0]`
record and the queue's pins describe. The FR-35 cycle's own record stands
closed as history; the removal is the new cycle's ask (FR-45), not a
rewrite of the closed record.

Version trail: FR-32 `7fe1370` (version 0.35.0, never tagged) → FR-33
`af48be5` (0.36.0, never tagged) → `ddeaf67` (FR-34's code; the tag
`v0.36.0` on it) → v0.37.0 `5a2d7f0` (FR-34) → v0.38.0 `c02862d` (FR-35)
→ `881eda9` → v0.39.0 `4f7fad6` → `96a576f` → `3af66d4` → `5336a03` →
`5257729` → `c2f78b3` → `39afeb2` → `fb46cc0` → `db68d3f` → `306f425` →
`85f9ce9` → v0.40.0 `505097a` → `856c2bf` → `eec12f9` → `064ce50` →
v0.41.0 `25fe13c` → `3badc04` → `122f320` (the codex-code backend) →
`af82155` (naming `122f320`) → `4fee899` (the FR-44d amendment; the tag
`v0.43.0` on it) → `d5cbbbe` (naming `4fee899`). The FR-45 removal's
delivery lands ahead; its sha is nameable only from the commit that
follows it (the trail's rule).

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

## The boundary of the removal (— (NFR-14, C-08))

The FR-43/FR-44 cycles' delivered contracts stand in the boundary §6; the
removal touches only the queue feature itself — not the queue channel,
not the shutdown drain, not the peer-prompt acceptance, not the approval
takeover, not the four-region screen, not any provider surface. The
FR-35 cycle's own closed record is not rewritten — the removal (FR-45)
stands beside the closed records, exactly as FR-44 stood beside FR-43's.
