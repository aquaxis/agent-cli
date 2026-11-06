# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the one-shot mode added — `agent-cli one-shot <prompt>`: one prompt
in, the answer printed, the session ends" (FR-47 a/b/c)
(`.aiprj/instructions.md`, reloaded 2026-11-06 10:02:20 +0900 — sha256
`5f369e12f9be26ee870d8dcfe42605c0be53b159b357bb1f39bd41354ed892fb`, 800 bytes)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked

---

## Overview

The reloaded instruction's ask (FR-47): 「### ワンショットモードの追加 —
ワンショットで完結するモードを追加する」 — add a mode that completes in one
shot. The delivery: **`agent-cli one-shot <prompt>`**, one prompt in, the
answer printed as it arrives, the session ends — no REPL, no IPC server, no
registry entry. The standing delivery sentence's clauses are all delivered:
the docs (the CHANGELOG's `[0.46.0]` entry, the README's CLI table row, the
usage row, the architecture module/entry surfaces), the example (verified no
surface — the `[ui]`/provider blocks name nothing the one-shot changes), the
minor carry 0.45.0 → **0.46.0**, the delivery commit pushed, and the release
standing the latest version.

**The ask is delivered.** The subcommand ran live: `agent-cli one-shot "Reply
with exactly OK-ONEShot and nothing else"` printed `OK-ONEShot` and exited.
Gates at the close: **663 unit + 6 integration, 0 failed**, clippy's only
warnings the two pre-existing dead-store notes in test code.

## The cycle's tasks

| ID | Task | Requirements | Design | Status |
|---|---|---|---|---|
| T-108 | The one-shot mode (FR-47a; design §1 D-78): the `Command::OneShot { prompt, quiet }` subcommand (`src/cli.rs`), the dispatch arm (`src/main.rs`), and `app::run_oneshot` (`src/app.rs`) — the same persona/provider/tools/MCP assembly `run_headless` performs, then one `AgentInput::UserPrompt`, the text events streamed to stdout (`--quiet` strips the banner), exit on the terminal event; no IPC server is bound and no registry entry is written; the CLI's subcommand-list pin adds `one-shot` | FR-47a, NFR-13, NFR-15, AS-76, AS-77 | design §1 (D-78), §2 (TPR-29), §3 | ✅ |
| T-109 | The completion (FR-47b, FR-47c; design §1 D-79, D-80, §4 W-71): the CHANGELOG's `[Unreleased]` became the `[0.46.0]`/`### Added` entry recording the subcommand in user language; the README's CLI table grew the `one-shot` row and `doc/usage.md`'s command table grew its row (the architecture's module tree names `run_oneshot`); the version carry 0.45.0 → **0.46.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading); the delivery commit pushed to `origin/main`; the tag `v0.46.0` and the release published and made the **latest** version | FR-47b, FR-47c, NFR-11, NFR-14, C-14 | design §1 (D-79, D-80), §4 (W-71) | ✅ |
| T-110 | The ledger's consistency and the byte-exact exit check (C-08, NFR-12, C-15, AC-40; design §4): the three documents re-aligned to the delivered state together (this pass), the work log `2026-11-06_008.md` written embedding the instruction verbatim (the execution session's log — written once, never overwritten), no document carrying any mark of authorship or tooling, the write surface inside the delivery sentence's list + the ledgers + the log | — (C-08, NFR-12, C-15, AC-40) | design §4 | ✅ |

## The execution record

- **T-108 (✅).** `Command::OneShot { prompt, quiet }` added to `src/cli.rs`
  (clap derives the subcommand name `one-shot`; the doc comment names the
  same startup options `run` takes), the dispatch arm `Command::OneShot`
  added to `src/main.rs` (`config::load`, `app::run_oneshot(cfg, source,
  cli.run_args, prompt)`), and `app::run_oneshot` implemented in `src/app.rs`
  beside `run_headless`: the persona resolution, `config::apply_overrides`,
  the provider build, the tools registry with MCP attach, the conversation
  log, the auto-approve/permissions ruleset — mirrors of the headless
  assembly — with **no** `IpcServer::bind`, **no** `RegistryHandle::register`
  and no REPL; one `AgentInput::UserPrompt(prompt)` on a channel the agent
  task drains once, the `AgentEvent::Text` deltas printed to stdout as they
  arrive (the banner only without `--quiet`; `--quiet` prints the answer
  text alone), and the loop ends on the terminal event (`Done`/`Error`) and
  returns — `main`'s `std::process::exit(0)` ends the process. The CLI's
  subcommand-list pin (`cli_help_compiles_and_lists_known_subcommands`)
  grew `one-shot`. Compile fixes en route: the `RegistryHandle` cleanup is
  `cleanup(&self)` (sync) — not an async unregister; the dispatch pattern
  needed the `quiet` field; the bail is `AppError::Other(...)` per the house
  error style. Gates: suite **663 unit + 6 integration, 0 failed**; clippy's
  only warnings the two pre-existing dead-store notes (the empty-doc-comment
  warning the insertion first produced was fixed at the site).
  **Live probe**: `agent-cli one-shot "Reply with exactly OK-ONEShot and
  nothing else"` → the answer `OK-ONEShot` printed, the process returned —
  one prompt in, the answer out, the session ended.
- **T-109 (✅).** The CHANGELOG's `[Unreleased]` became the `[0.46.0]`/
  `### Added` entry (the first full write of the file was caught at the
  diff — 1123 deletions, a truncation — and was restored from git and the
  entry re-applied surgically, +12 lines); the README's CLI table row
  (`agent-cli one-shot <prompt> | Run one prompt to completion…`) added
  beside `serve`'s row; `doc/usage.md`'s command table grew the same row;
  `doc/architecture.md`'s module tree and entry-point paragraphs name
  `run_oneshot` beside `run_headless`. The version carry 0.45.0 → **0.46.0**
  (`Cargo.toml`, `Cargo.lock` via `cargo update -p agent-cli`). The delivery
  commit `77e8a29` "Add the one-shot subcommand: one prompt in, the printed
  answer out, then exit" pushed (`5978dc9..77e8a29 main -> main`); the tag
  `v0.46.0` created and verified pointing at `77e8a29`
  (`git rev-parse v0.46.0^{commit}`); the GitHub release
  `v0.46.0 — the one-shot subcommand` published and verified **Latest**
  (`gh release list`'s Latest label + the API read, `createdAt`
  2026-11-06T01:16:40Z).
- **T-110 (✅).** The three ledgers re-aligned to the delivered state together
  at this pass (the requirements' §13 delivery table, the design's gate row
  W-71 ✅ and §5 record, this document's task table and trail); the work log
  `2026-11-06_008.md` embeds the instruction verbatim (Art. 7); the
  instruction verified byte-exact at exit (800 bytes, sha256 `5f369e12…`);
  no document carries any mark of authorship or tooling (Art. 8, NFR-12);
  the write surface stayed inside the standing delivery sentence's list (the
  product files: `src/cli.rs`, `src/main.rs`, `src/app.rs`, `CHANGELOG.md`,
  `README.md`, `doc/usage.md`, `doc/architecture.md`, `Cargo.toml`,
  `Cargo.lock`) plus the three ledgers and the log.

## The closed tasks of the delivered cycles (their records stand)

- **T-105/T-106/T-107** (the FR-46 terminal display fix, the completion, the
  ledger passes) — ✅, closed 2026-11-06: delivered at `ebe0aca` ("Fix the
  terminal's bottom line…"), the version carried to **0.45.0**, the tag
  `v0.45.0`, the release verified Latest (created 2026-11-06T00:55:05Z);
  gates **663 unit + 6 integration, 0 failed**; the trail's follow-ups
  `5978dc9` and the recording/re-load passes `2557f7f`/`92c43fd`.
- **T-102/T-103/T-104** (the FR-45 removal, the completion, the ledger's
  consistency) — ✅, closed 2026-11-06: delivered at `b5deb9c` (the tag
  `v0.44.0`, the release Latest), the trail's follow-up `6f0390f` naming
  it; gates **663 unit + 6 integration, 0 failed**; the ledger re-alignment
  `0d45242`.
- **T-95/T-96** (the FR-43 investigation and its ledger record) — ✅,
  closed 2026-11-05 (the requirements' §12; work log `_008`).
- **T-97/T-98/T-99** (the FR-44 backend, wiring/docs, completion) — ✅,
  closed 2026-11-05: delivered at `122f320` (named by `af82155`), the
  version carried to 0.42.0, gates **669 unit + 6 integration, 0 failed**.
- **T-100** (the FR-44d example amendment) — ✅, closed 2026-11-05:
  delivered at `4fee899` (named by the follow-up `d5cbbbe`), the version
  carried to **0.43.0**.
- **T-101** (FR-44e, the latest release) — ✅, closed on the verification
  the FR-45 cycle's ground carried: the GitHub release `v0.43.0` stood
  published with `isLatest: true` programmatically read from the releases
  API (created 2026-11-05T10:57:38Z), the tag `v0.43.0` verified pointing
  at `4fee899`.

## Cross-cutting gates (C-09, the boundary) — closed ✅

- Unit + integration at the delivery tree: **663 unit + 6 integration,
  0 failed** — the counts re-derived at the delivery tree (the one-shot
  added no deleted pin; the CLI pin grew a member and passes).
- Clippy at the delivery tree: the two dead-store warnings in test code
  stand pre-existing (verified at the FR-45/FR-46 closes as well); no new
  warning from the one-shot's tree (the empty-doc-comment note the insertion
  first produced was fixed before the gates ran).
- The boundary's shipped contracts unchanged: the REPL (`run`), the headless
  serve (`serve`/`spawn`/`stop`/`list`/`groups`), the peer transport
  (`send`/`ask`), the providers/doctor/config/selftest subcommands, the
  FR-46 display fix, the queueing removal's pre-queue contract — the
  one-shot adds one subcommand and touches none of them (verified: the
  suite's REPL/zone/display pins all pass unchanged; the diff touches only
  the new subcommand's surfaces).
- The `.aiprj` documents mutually consistent (C-08); the work log written
  per Art. 6/7 (never overwritten); the instruction byte-exact at exit.

## The previous cycles' record

**"the terminal's bottom-most line is fixed" (FR-46 a/b/c)** — closed,
2026-11-06: the mid-turn standing shape inverted (the clock block's last row
on the screen's bottom-most line, no prompt display mid-turn, the prompt
back at every input-possible handoff), delivered at `ebe0aca`, the version
carried to **0.45.0**, the release `v0.45.0` published and verified Latest
(created 2026-11-06T00:55:05Z); gates **663 unit + 6 integration, 0
failed**; the recording/re-load passes `2557f7f`/`92c43fd` and the ledger
follow-up `5978dc9`.

**"the prompt-input queueing feature is deleted" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c` (the tag `v0.44.0` on it, the release
`v0.44.0 — the prompt-input queueing feature removed` published as the
latest version, created 2026-11-05T23:18:45Z), named by the trail's
follow-up `6f0390f`; gates at the close **663 unit + 6 integration,
0 failed**; the ledger re-alignment `0d45242`.

**"the codex-code backend…" (FR-44 a–e)** — closed, 2026-11-05: delivered at
`122f320` and `4fee899` (the FR-44d amendment), the version carried to
0.42.0 then **0.43.0**, the release **v0.43.0 verified latest** (T-101's
record); gates **669 unit + 6 integration, 0 failed**.

**"the codex access investigation" (FR-43 a/b/c)** — closed, 2026-11-05 (no
release — the investigation): the verdict feasible for delegation-style
access (D-65), the shape recorded unbuilt (D-66); FR-44's cycle built it.
Work log `2026-11-05_008.md`.

**"the refactoring…; the prompt's bottom row made prompt-only" (FR-41,
FR-42)** — closed, 2026-11-05: delivered at `eec12f9` and `064ce50`, the
version carried to **0.41.0** by `25fe13c`, named by the follow-up
`3badc04`. Gates **647 unit + 6 integration, 0 failed**.

**"the display regions…" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05:
delivered at `39afeb2`, named by the follow-up `fb46cc0`; gates 645+6.
FR-39/FR-40 closed at `db68d3f`/`306f425`; the version carried to 0.40.0.

**"prompt input during a running turn…" (FR-35)** — closed, 2026-11-04
(`c02862d`, v0.38.0). **Its delivered feature was deleted by the FR-45
cycle** (the CHANGELOG's `[0.38.0]` entry stands as history).

Version trail: … → `122f320` → `af82155` → `4fee899` → `d5cbbbe` →
`b5deb9c` → `6f0390f` → `0d45242` → `2557f7f` → `92c43fd` → `5978dc9` →
`ebe0aca` (the FR-46 fix; the tag `v0.45.0`) → `5978dc9` (naming `ebe0aca`)
→ **`77e8a29` (the one-shot; the tag `v0.46.0`)** → the trail's follow-up
stands ahead, per the trail rule — a delivery commit's sha is nameable only
from the commit that follows it.

Earlier cycles stand in the requirements document's §12 and the design
document's §5.
