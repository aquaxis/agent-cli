# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the one-shot mode added — one prompt in, the answer out, the session
ends" (FR-47 a/b/c) (`.aiprj/instructions.md`, reloaded 2026-11-06 10:02:20
+0900 — sha256 `5f369e12f9be26ee870d8dcfe42605c0be53b159b357bb1f39bd41354ed892fb`,
800 bytes; the 最優先事項's ワンショットモード section supersedes the FR-46
cycle's ask, whose delivery stands closed at `ebe0aca`/v0.45.0/Latest)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked
Article 3 of the AI Coding Guidelines: record progress updates in this
file whenever a task changes state.

---

## Overview

The reloaded instruction's newest ask (最優先事項 §ワンショットモードの追加):
**ワンショットで完結するモードを追加する** — add a mode that completes in one
shot: one prompt in, the answer out, the session ends. The FR-46 display fix
stands delivered (0.45.0), and the standing delivery sentence governs the
completion: documentation, the example, the minor version bump, commit, push,
and the release standing the latest version.

The design's reading (D-78): the mode is the **`oneshot` subcommand**
(`agent-cli oneshot "<prompt>"`), mirroring `run_headless`'s `Agent`
assembly (persona → provider → tools/MCP → registry/log) but **without** the
IPC server, the REPL, or any turn machinery beyond one `process_turn` — the
prompt streams its answer to stdout and the process exits 0. `--auto-approve`
carries (headless semantics: a deny is the only brake). The provider error
prints to stderr and exits 1. The subcommand-list pin (`cli_help_compiles_…`)
gains `oneshot`.

The ask is **recorded pending**: T-108 executes the addition, T-109 the
completion (the docs follow, **0.45.0 → 0.46.0**, the commit, the push, the
release standing latest — the FR-46c machinery repeated), T-110 the ledger's
ground pass after both.

## The cycle's tasks

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-108 | The addition (FR-47a; design §1 D-78, §2 TPR-29, §3): the `oneshot` surface in `src/cli.rs`/`src/main.rs`/`src/app.rs` — the subcommand taking the one prompt (positional), the dispatch wiring `config::load` → `app::run_oneshot(cfg, source, args, prompt)`, the one-shot body mirroring the headless `Agent` assembly (persona resolution + persona overrides → `ai::build` → `ToolRegistry::build` + MCP attach → `ConversationLog::open` → `Arc<CancelToken>` + auto-approve true + `approval_tx: None`), one `AgentInput::UserPrompt`, the event loop surfacing `Text` deltas to stdout plain (plus the optional `Info` lines the shipped headless display prints — nothing else, no interactive surfaces), `Done` → **exit 0**; the no-prompt error path (the subcommand's arg `required = true`); the provider error prints to stderr and the process exits 1; the registry/IPD/REPL stand outside the mode | FR-47a, NFR-16, NFR-17, C-15 | design §1 (D-78), §2 (TPR-29), §3 | T-110's ledger ground ✅ |
| T-109 | The completion (FR-47b, FR-47c; design §1 D-79, D-80, §4 W-71): the docs follow — the CHANGELOG's `[Unreleased]` becomes the `[0.46.0]` entry recording the new mode in user language; the README's command table, `doc/usage.md`'s command reference and `doc/architecture.md`'s dispatch paragraph follow; the example carries no oneshot surface (BL-M57's verified absence — `example/config.example.toml` has nothing to extend); the version carry 0.45.0 → **0.46.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading); the delivery commit of the addition and the documents, pushed to `origin/main`; the release published and made the **latest** version (the FR-46c machinery repeated at 0.46.0 — the `isLatest` probe re-asked, the tag verified on the delivery commit) | FR-47b, FR-47c, NFR-14, C-14 | design §1 (D-79, D-80), §4 (W-71) | T-108 🟨 |
| T-110 | The cycle's ledger ground — **closed at this update** (the ledgers' write pass): the instruction re-verified byte-exact at the pass (800 bytes, sha256 `5f369e12…`), the three documents written mutually consistent (C-08) on the pending cycle (FR-47 a/b/c, BL-M56–BL-M57, TPR-29, D-78–D-80, T-108/T-109/T-110, W-71 ⬜, AC-40, AS-76–AS-78, C-08/C-09/C-12/C-14/C-15), the predecessor FR-46 cycle's delivered record carried into all three (`ebe0aca`, v0.45.0 Latest), the work log `2026-11-06_008.md` written after `_007` (no overwrite), no document carrying any mark of authorship or tooling, the write surface inside Article 3's limit (the three documents + the work log; no product file touched at the pass) | — (C-08, NFR-12, C-15) | design §4 | — | ✅ |

## The execution record

- **T-110 (✅).** The update session's ledger pass: the instruction loaded
  and verified byte-exact (800 bytes, sha256 `5f369e12…`, mtime
  2026-11-06 10:02:20 +0900); the three documents written together on the
  pending FR-47 cycle — the requirements (FR-47 a/b/c, BL-M56–BL-M57, the
  scope §4, the assumptions AS-76–AS-78, the acceptance AC-40), the design
  (D-78/D-79/D-80, §2 TPR-29, §3 the five-angle verdicts, §4 W-71 ⬜), the
  tasks (this document) — mutually consistent on the pending state (the
  predecessor session's cut-off stub rewritten: the 9-line requirements
  fragment with the `.aid/…` placeholder was completed to the full three-
  document set); the gates stand at the FR-46 close's **663 unit + 6
  integration, 0 failed** (no product write at the pass; the suite re-runs
  at the delivery close per C-09); the work log `2026-11-06_008.md` written
  per the Operation Guidelines (the next free index after `_007`). The
  delivery cycle (T-108, T-109) re-runs the gates and re-verifies the
  instruction byte-exact at its exit (AC-40).

## The closed tasks of the delivered cycles (their records stand)

- **T-105/T-106/T-107** (the FR-46 display fix, the completion, the ledger's
  ground) — ✅, closed 2026-11-06: delivered at `ebe0aca` ("Fix the
  terminal's bottom line: the prompt when input is possible, the running
  clock while the agent runs"), the version carried to **0.45.0**, the tag
  `v0.45.0` and the release published **Latest** (created
  2026-11-06T00:55:05Z); gates **663 unit + 6 integration, 0 failed**; the
  ledger pass `5978dc9` naming the delivery per the trail rule; work logs
  `2026-11-06_005.md`/`_006.md`/`_007.md`.
- **T-102/T-103/T-104** (the FR-45 removal, the completion, the ledger's
  consistency) — ✅, closed 2026-11-06: delivered at `b5deb9c` (the tag
  `v0.44.0`, the release Latest), the trail's follow-up `6f0390f` naming
  it; gates **663 unit + 6 integration, 0 failed**; the work logs
  `_002`/`_003` and the re-alignment `_004`;
- **T-95/T-96** (the FR-43 investigation and its ledger record) — ✅,
  closed 2026-11-05 (the requirements' §12; work log `_008` → superseded:
  the log naming moved to `_009` when this cycle's log took `_008`).
- **T-97/T-98/T-_99** (the FR-44 backend, wiring/docs, completion) — ✅,
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
  the delivery close (T-108's re-derivation; the pass stands at the
  recorded close 663+6 — no product write at the pass).
- Clippy at the delivery close: silent or the two pre-existing dead-store
  warnings (verified pre-existing at `b5deb9c`'s tip by the FR-45 cycle's
  `git stash` re-run).
- The boundary §6's shipped contracts unchanged beyond the addition's own
  surface (NFR-14): the REPL, the serve/serve's headless semantics, the
  peer-prompt flows, the four-region display, the one-shot-free surfaces.
- The `.aiprj` documents mutually consistent (C-08); the work log written
  per the Operation Guidelines' Article 1; the instruction byte-exact at
  exit.

## The previous cycles' record

**"the terminal display fix" (FR-46 a/b/c)** — closed, 2026-11-06:
delivered at `ebe0aca`, the version carried 0.44.0 → **0.45.0**, the tag
`v0.45.0` on it and the release published **Latest** (created
2026-11-06T00:55:05Z); gates **663 unit + 6 integration, 0 failed**; the
ledger record `5978dc9` naming it per the trail rule; work logs
`_005`–`_007`.

**"the prompt-input queueing feature's removal" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c` (the tag `v0.44.0` on it, the release
published as the latest version), named by `6f0390f`; gates **663 unit + 6
integration, 0 failed**; the ledger re-alignment `0d45242`; work logs
`_002`/`_003`/`_004`.

**"the codex-code backend …" (FR-44 a–e)** — closed, 2026-11-05: delivered
at `122f320` and `4fee899` (the FR-44d amendment), the version carried to
0.42.0 then **0.43.0**, named by `af82155`/`d5cbbbe`; the release **v0.43.0
verified latest** (T-101's record); gates **669 unit + 6 integration,
0 failed**.

**"the codex access investigation" (FR-43)** — closed, 2026-11-05 (no
release — investigation and report): the verdict feasible for
delegation-style access (D-65), the shape recorded unbuilt (D-66); FR-44's
cycle built it. Work log `2026-11-05_008.md` — renamed to `_009` when this
cycle's log took `_008`... (the historical naming stands; see the logs
directory for the shipped name).

Version trail: FR-32 `7fe1370` (0.35.0, never tagged) → FR-33 `af48be5`
(0.36.0, never tagged) → `ddeaf67` (FR-34's code; the tag `v0.36.0` on it)
→ v0.37.0 `5a2d7f0` (FR-34) → v0.38.0 `c02862d` (FR-35) → … → v0.44.0
`b5deb9c` (FR-45) → `6f0390f` → `0d45242` → `2557f7f` → `92c43fd` →
v0.45.0 `ebe0aca` (FR-46) → `5978dc9` (naming `ebe0aca`) → **the FR-47
delivery commit stands ahead, per the trail rule** — a commit's sha is
nameable only from the commit that follows it.

Earlier cycles stand in the requirements document's §12 and the design
document's §5: the subagents' auto-exit (FR-32–FR-34, v0.37.0 on
`5a2d7f0`); the multi-angle subagent analysis (FR-31); FR-29/FR-30
(`spawn` in the live `[tools] enabled`); the click rule's exclusion and the
cancelled-by-user display (FR-27/FR-28, v0.34.0 on `c022909`); the scroll
marker's face (FR-26, v0.33.0 on `6cc6661`); the running display
(FR-22–FR-24, v0.32.0 on `e8ad594`); the mouse defects (FR-18–FR-21,
v0.31.0 on `052e230`); add strata (FR-13–FR-17, v0.30.0–0.30.2); the
peer-prompt cycles (v0.29.0, v0.28.0, v0.27.0); the mouse input cycle
(v0.25.0, the 機能追加 addenda v0.26.0).
