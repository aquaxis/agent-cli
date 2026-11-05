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

The reloaded instruction's ask was one removal: **delete the prompt-input
queueing feature** — the feature FR-35 delivered (v0.38.0, `c02862d`):
prompts typed while a turn runs are accepted, edited and enqueued onto the
agent's input channel, and run in order once the running turn completes
(the CHANGELOG's `[0.38.0]` record). The removal surface the requirements'
§4/§5 named: the Pending submit arm's echo + `UserPrompt` enqueue + the
`!`/`/` refusals, the `[queued: N]` notice, `queued_count`/`queue_release_allowed`,
the approval-time draft pair, the queue's four unit pins, the
approval-snapshot pin and the agent-side boundary pin. The queue channel
itself (`AgentInput` FIFO), the shutdown drain answering queued deliveries
truthfully, the peer-prompt queue and the four-region screen stand
untouched; the pre-queue contract is restored. The delivery rode the
standing sentence: the CHANGELOG's `[0.44.0]` entry, the minor carry
0.43.0 → **0.44.0**, the commit, the push, and the release standing the
latest version (design D-73/D-74, §4 W-69).

**The ask is delivered.** The removal stands in the tree at the delivery
commit `b5deb9c` ("Remove the prompt-input queueing feature", pushed
`d5cbbbe..b5deb9c`), the tag `v0.44.0` stands verified on it, and the
GitHub release `v0.44.0 — the prompt-input queueing feature removed` stands
published as **Latest**. Gates at the close: **663 unit + 6 integration,
0 failed** (669 − the six deleted pins), clippy carrying only the two
dead-store warnings verified pre-existing at the previous tip (a `git
stash` re-run at `d5cbbbe` produced the identical warnings).

## The cycle's tasks

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-102 | The removal (FR-45a; design §1 D-72, §2 TPR-27, §3): delete the queue's mechanism at its named sites — the raw loop's Pending submit arm (the enqueue, the `!`/`/` refusals), the piped loop's mirror arms' mid-turn acceptance, the helpers `queued_count`/`queue_release_allowed`/`paint_queue_status`/`QUEUED_NOTICE_PREFIX`, the idle release arms' queue conjuncts on both loops, the park/pending_since machinery on both loops, the approval-time draft pair (`DraftSnapshot`/`snapshot_draft`/`restore_draft` and their call arms), the four unit pins, the approval-snapshot pin and `src/agent.rs`'s boundary pin — during a running turn the loop ignores plain typed input (only `Esc`/`Ctrl-C` act) and `Pending` releases on the turn's terminal event; the suite re-derives its counts (669 − 6) and stands green | FR-45a, NFR-13, NFR-14, NFR-15, C-09 | design §1 (D-72), §2 (TPR-27), §3 | T-104's ledger ground | ✅ |
| T-103 | The completion (FR-45b, FR-45c; design §1 D-73, D-74, §4 W-69): the CHANGELOG's `[Unreleased]` became the `[0.44.0]` entry recording the removal in user language (the shipped READMEs/usage carried no queue sentence to correct — BL-M53's verified absence); the version carry 0.43.0 → **0.44.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading); the delivery commit of the removal and the records, pushed to `origin/main`; the release published and made the **latest** version (the `isLatest` probe re-asked at 0.44.0) | FR-45b, FR-45c, NFR-11, NFR-15, C-14 | design §1 (D-73, D-74), §4 (W-69) | T-102 | ✅ |
| T-104 | The ledger's own consistency and the byte-exact exit check (C-08, NFR-12, C-13, AC-38; design §4): the three documents stand mutually consistent (written this cycle; re-checked at the delivery close — W-69 marked ✅ on T-102's/T-103's records), the work log written per Art. 6/7 after `2026-11-06_002.md` (no overwrite), the instruction re-verified byte-exact at exit (349 bytes, sha256 `06620319…`), no document carrying any mark of authorship or tooling, the write surface inside Article 3's limit — **closed this update** (the ledgers' final pass; the trail's follow-up rule leaves `b5deb9c`'s own naming to the next cycle's commit, per the trail rule) | — (C-08, NFR-12, C-13, AC-38) | design §4 | T-102, T-103 | ✅ |

## The execution record

- **T-102 (✅).** The removal wrote `src/app.rs` + `src/agent.rs` and
  nothing else in the product tree:
  - the helpers `queued_count`, `queue_release_allowed`,
    `paint_queue_status` and the `QUEUED_NOTICE_PREFIX` constant deleted
    with their FR-35 comment frames;
  - the `DraftSnapshot`/`snapshot_draft`/`restore_draft` pair deleted, and
    the three arms that used it restored to the pre-queue shape (the
    approval arm resets the editor and takes nothing; the Esc-deny arm and
    the raw y/N arm reset the editor plainly, as the pre-FR-35 tree at
    `c02862d`^ carries them);
  - the raw loop's Pending submit arm restored to the pre-queue contract —
    during a turn only `Esc`/`Ctrl-C` are honoured (the cancel's forced
    return with the zone handoff D-54 kept), every other key ignored
    (`continue`); the `[info] nothing to queue`/「queue accepts prompts
    only…」 notices, the mid-turn echo and the `UserPrompt` enqueue are
    gone;
  - the raw and piped loops' idle release arms restored to the plain
    terminal-event release (`prompt_state = Ready` on the notification, no
    queue conjunct, no `pending_since` stamp), the raw arm keeping the
    zone handoff and repaint the pre-FR-35 tree had;
  - both park arms deleted with their `PARK_GRACE_MS`/`park_timer`
    declarations (the self-healing fallback served only the queue's wait);
  - the piped loop's stdin gate restored to `Ready || AwaitingApproval`
    (mid-turn lines are not read);
  - the queue's four unit pins, the approval-snapshot roundtrip pin and
    `agent.rs`'s queued-prompt-survives-cancel boundary pin deleted;
  - residual check: zero occurrences of every deleted identifier and
    sentence in `src/`; zero `FR-35` mentions in `src/`.
  Gates at the tree: the suite **663 unit + 6 integration, 0 failed**
  (669 − 6, the deleted pins' own harnesses), clippy's only warnings the
  two dead-store notes in test code proven pre-existing (a `git stash`
  rebuild at the unmodified tip gave the identical warnings).
- **T-103 (✅).** The CHANGELOG's `[Unreleased]` became the
  `[0.44.0]`/`### Removed` entry recording the queue's deletion in user
  language; `Cargo.toml` and `Cargo.lock` carried 0.44.0; the delivery
  commit `b5deb9c` "Remove the prompt-input queueing feature" was pushed
  (`d5cbbbe..b5deb9c  main -> main`); the tag `v0.44.0` was created and
  pushed and verified pointing at `b5deb9c` (`git rev-parse
  v0.44.0^{commit}`); the GitHub release `v0.44.0 — the prompt-input
  queueing feature removed` was published and verified **Latest**
  (`gh release list`'s Latest label; the release API read,
  `https://github.com/aquaxis/agent-cli/releases/tag/v0.44.0`).
- **T-104 (✅).** The three documents stand mutually consistent (C-08) on
  the delivered record; the work log `2026-11-06_003.md` embeds the
  instruction verbatim (Art. 7); the instruction stands byte-exact at exit
  (349 bytes, sha256 `06620319…`); no document carries any mark of
  authorship or tooling (NFR-12); the write surface: `src/app.rs`,
  `src/agent.rs`, `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, the three
  `.aiprj` documents, the work log — inside C-13's list.

## The closed tasks of the delivered cycles (their records stand)

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
  the delivery commit `4fee899`.

## Cross-cutting gates (C-09, the boundary) — closed ✅

- Unit + integration at the removal tree: **663 unit + 6 integration,
  0 failed** — the counts re-derived (669 − 6 deleted pins) and the suite
  green (C-09's re-execution clause).
- Clippy: the two dead-store warnings in test code stand pre-existing
  (verified against the unmodified tip by a `git stash` rebuild); no new
  warning from the removal.
- The boundary §6's shipped contracts unchanged: the queue **channel**
  (`AgentInput` FIFO), the shutdown drain's truthful answers
  (`doc/architecture.md` §7 — untouched), the peer-prompt queue, the
  approval takeover, the four-region screen, the cancel keys' force-return
  path (kept; the zone handoff D-54 rides it), `--exit-when-idle` (FR-32)
  and every closed cycle's delivered shape stand — the removal touched
  only the queue feature itself (NFR-14, verified by the residual scan's
  zero hits and the write-surface list).
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
delivered feature is the ask this cycle deleted** (FR-45a; the boundary
§6's supersession clause): the removal reinstated the pre-queue contract
the CHANGELOG's `[0.38.0]` record and the queue's pins described, and the
feature's history record stands (the CHANGELOG's `[0.38.0]` section). The
FR-35 cycle's own record stands closed as history; the removal is the
delivery cycle's ask (FR-45), not a rewrite of the closed record.

Version trail: FR-32 `7fe1370` (version 0.35.0, never tagged) → FR-33
`af48be5` (0.36.0, never tagged) → `ddeaf67` (FR-34's code; the tag
`v0.36.0` on it) → v0.37.0 `5a2d7f0` (FR-34) → v0.38.0 `c02862d` (FR-35)
→ `881eda9` → v0.39.0 `4f7fad6` → `96a576f` → `3af66d4` → `5336a03` →
`5257729` → `c2f78b3` → `39afeb2` → `fb46cc0` → `db68d3f` → `306f425` →
`85f9ce9` → v0.40.0 `505097a` → `856c2bf` → `eec12f9` → `064ce50` →
v0.41.0 `25fe13c` → `3badc04` → `122f320` (the codex-code backend) →
`af82155` (naming `122f320`) → `4fee899` (the FR-44d amendment; the tag
`v0.43.0` on it) → `d5cbbbe` (naming `4fee899`) → **`b5deb9c` (the
queue's removal; the tag `v0.44.0` on it)** → the trail's follow-up (the
commit that names `b5deb9c`) stands ahead, per the trail rule — a
delivery commit's sha is nameable only from the commit that follows it.

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
