# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the one-shot mode added — `agent-cli one-shot <prompt>`: one prompt
in, the printed answer out, then exit — no REPL, no IPC server, no registry
entry" (FR-47 a/b/c)
(`.aiprj/instructions.md`, reloaded 2026-11-06 10:02:20 +0900 — sha256
`5f369e12…`, 800 bytes, recorded in requirements §2; the tree carried
v0.45.0, its release `v0.45.0` verified Latest at the cycle's ground)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`.

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What was decided, and what it cost

The instruction's new 最優先事項 section 「ワンショットモードの追加」 names one
addition: 「ワンショットで完結するモードを追加する」 — a mode that completes in
one shot. The FR-46 section above it stands **delivered** (v0.45.0, `ebe0aca`;
the requirements' §13), so this cycle adds only the mode. The design decision
is **D-78 — the one-shot mode is a new subcommand, `agent-cli one-shot
<prompt>`, riding the headless assembly with no IPC server, no REPL and no
registry entry, printing the answer and exiting**. The CLI already holds the
two ends of the scale — `run` (the interactive REPL) and `serve`/headless
(the resident peer) — and the one-shot fills the gap between them: a session
shorter than `serve`, with no IPC surface at all, yet with the full provider/
tools/MCP/persona assembly.

- **D-78 — the subcommand and its pipeline.** `Command::OneShot` carries one
  positional `<PROMPT>` plus a `--quiet` flag (print only the answer text —
  the streamable, parseable surface scripts compose with; the banner and the
  info lines stay on stderr as tracing, so stdout carries the answer alone
  under `--quiet`... the banner is stdout by default and suppressed under
  `--quiet`). The pipeline mirrors `run_headless` (`src/app.rs:630`)'s
  assembly exactly where it can and drops what it cannot name a peer for:
  persona resolution (`persona::resolve` with the same chain — `--persona`,
  `[runtime] persona_file`, the agents dir, the project's own file) →
  `apply_persona_overrides` → `ai::build` → `ConversationLog::open` →
  `ToolRegistry::build` + `mcp::connect_all` (fail-soft, as shipped) →
  `Agent::build_initial_history` → auto-approve headless (`AtomicBool(true)`,
  the shipped headless reading — `[permissions] deny` remains the only
  brake) → the headless cancel token → `Agent { .. approval_tx: None }`.
  The registry entry is **not** written and the IPC server is **not** bound:
  a one-shot session is nobody's addressable peer by design (AS-76), so
  nothing registers or unregisters — no `IpcServer::bind`, no
  `RegistryHandle::register`, no socket file. The agent runs **one**
  `AgentInput::UserPrompt` into the shipped `agent.run(input_rx, event_tx)`
  loop over an unbounded-events channel and the loop's own exit rides the
  shipped quiet-exit (`exit_when_idle`) contract the FR-32 machinery
  carries: one input, the input channel closes, the agent breaks out after
  the turn, the task ends.

- **D-79 — the answer's face and the session's end.** The display path is a
  dedicated `display_task` reading the event channel and printing the turn's
  text events as they arrive (the shipped `display_event` print path, not
  the REPL's indicator) — the answer streams to stdout; the outcome `✔`
  line the indicator writes in the REPL does not exist here (no indicator —
  `Theme::plain()` under `--quiet`, the default theme otherwise). The
  session ends when the agent task ends: `agent_handle` awaited directly —
  no shutdown watch, no drain timer, no teardown beyond dropping the input
  sender and awaiting the handle — and the process exits through `main`'s
  shipped exit. The cost: no queue, no peers, no stop address — the
  completion contract is exactly one turn.

- **D-80 — the documentation follows the addition.** The CHANGELOG's
  `[Unreleased]` became the `[0.46.0]` entry (### Added) recording the
  subcommand in user language; the README's CLI table gains the row; the
  usage doc's command list gains the row (`doc/usage.md:20`); the
  architecture's module-tree entry for `app.rs` names `run_oneshot`; the
  `/help` surface needs nothing (it lists REPL internals, not subcommands).
  The version carries **0.45.0 → 0.46.0** (`Cargo.toml`, `Cargo.lock`, the
  CHANGELOG heading), the delivery commit(s) push to `origin/main`, and the
  release **stands latest** (D-77/FR-45c's verified machinery, repeated).

The instruction's clauses map one-for-one onto the decisions:
「ワンショットモードの追加」+「ワンショットで完結するモードを追加する」 →
D-78/D-79 (the subcommand, the one-prompt/one-answer/exit contract);
ドキュメント更新 → D-80; the method C-12 → §3's angles;
マイナーバージョンアップ + commit, push + リリース最新 → D-80's completion.
「example更新」 names the example configuration: the one-shot mode adds no
config key (BL-M57's verified absence), so no example write exists.

## 2. The touchpoints this cycle touches (TPR-29)

| # | Touchpoint | Evidence (the files read in this cycle) |
|---|---|---|
| TPR-29 | The CLI surface and the headless assembly it borrows: `src/cli.rs` — the `Command` enum (`:30` the `Run`/`Serve` pair the one-shot slots beside), `RunArgs` (`:136` the flattened startup options the one-shot inherits by reuse), the subcommand-list pin (`cli_help_compiles_and_lists_known_subcommands`); `src/main.rs` — the dispatch (`:62` the `match cli.command`), the shipped exit (`:26`); `src/app.rs` — `run_headless` (`:630`, the assembly the one-shot mirrors), the `Agent` struct's fields as headless sets them, `AgentEvent` (`src/agent.rs:139`, the `Text`/`Done`/`Error` events the one-shot prints), the quiet-exit contract (`exit_when_idle`, FR-32), `display_event`'s print path; `src/error.rs` (`AppError::Other`); `README.md:358–369` (the CLI table), `doc/usage.md:19–20` (the command list), `doc/architecture.md:36` (the module tree), `CHANGELOG.md` `[Unreleased]` | The files' own lines, read in this cycle; BL-M56/BL-M57's records |

## 3. Analysis verdicts (C-12) — the multi-angle analysis of the addition

- **The mechanism angle.** The one-shot is the headless assembly minus its
  IPC face: every piece it needs exists (`Agent`, the event channel, the
  quiet-exit), and the only new code is the plumbing that feeds one prompt
  and prints the events. No provider change, no tool change, no new event
  type.
- **The contracts angle (NFR-14).** The shipped contracts stand: `run`'s
  REPL, `serve`'s registry/server, `spawn`/'stop'/'ask'/'send' — none
  touched. The one-shot adds a ninth subcommand; the pin (`cli_help_compiles_
  and_lists_known_subcommands`) re-derives with it. The headless auto-approve
  reading rides unchanged: `[permissions] deny` remains the only brake, as
  in `serve`.
- **The scope angle (§3.1).** **Not** touched: the REPL, the serve loop, the
  registry/IPC surface, the spawn machinery, the FR-46 display fix's
  composition (v0.45.0 delivered), the provider backends, the config keys —
  the one-shot adds no key (BL-M57), so no example write exists.
- **The live-verdict angle (NFR-15).** Executed at this cycle: the full
  suite (**663 unit + 6 integration, 0 failed**), clippy (the two
  pre-existing dead-store notes stand, no new warning from this cycle's
  surface), the CLI parse pin re-derived (the `one-shot` name in the
  subcommand list), the live probe (`agent-cli one-shot "Reply with exactly
  OK-ONEShot and nothing else"` → `OK-ONEShot`, exit), and the release-latest
  read at the close.
- **The docs angle (C-14, D-80).** The ledger's three documents and the work
  log; the products the standing sentence names (the subcommand surface, the
  CHANGELOG entry, the README/usage rows, the architecture tree note, the
  version files); nothing else.

## 4. Verification of the cycle (the gate table W-71)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-71 | FR-47 a/b/c — the one-shot subcommand delivered at D-78's shape (one prompt in, the printed answer out, exit; no IPC/REPL/registry), the parse pin re-derived, the docs written (the CHANGELOG `[0.46.0]` entry, the README/usage rows, the architecture note), the version carry to **0.46.0**, the delivery committed and pushed (`77e8a29`), the release standing latest | FR-47a, FR-47b, FR-47c, NFR-13, NFR-14, NFR-15, AS-76, AS-77, AS-78, TPR-29 | T-108, T-109, T-110 | ✅ |

*W-71's execution record: the suite stands green at the close (**663 unit +
6 integration, 0 failed**; the counts unchanged — the pin set re-derived),
clippy carries only the two dead-store warnings verified pre-existing at the
FR-45 close (a re-run reproduced them; not this cycle's new debt), the live
probe printed the one-shot's answer and exited (`OK-ONEShot`), the
CHANGELOG's integrity was restored after a mis-directed write (the file re-
cut to the surgical 12-line diff; verified 86,557 bytes against the shipped
tail), the tag `v0.46.0` verified on `77e8a29`
(`git rev-parse v0.46.0^{commit}`), the release verified **Latest** — these
clauses are the acceptance criterion AC-40's, which §11 of the requirements
traces here.*

## 5. The previous cycles (recorded after their documents were retired)

**"the terminal display fix — the screen's bottom-most line reads the prompt
while input is possible, and the spinner-and-elapsed clock while the agent
runs" (FR-46 a–c)** — closed, 2026-11-06: delivered at `ebe0aca` ("Fix the
terminal's bottom line…"), the version carried **0.44.0 → 0.45.0**, the tag
`v0.45.0` verified on it, the release published and verified **Latest**
(created 2026-11-06T00:55:20Z); gates **663 unit + 6 integration, 0
failed**, clippy standing its two pre-existing dead-store notes. The
recording pass `2557f7f` and the re-load pass `92c43fd` stand behind the
delivery in the trail; the ledger follow-up `5978dc9` names `ebe0aca`. Work
logs `2026-11-06_005.md`–`_007.md`.

**"the prompt-input queueing feature's removal" (FR-45 a–c)** — closed,
2026-11-06: delivered at `b5deb9c` ("Remove the prompt-input queueing
feature"), the version carried 0.43.0 → **0.44.0**, the tag `v0.44.0`
verified on it, the release `v0.44.0 — the prompt-input queueing feature
removed` published and verified **Latest** (created
2026-11-05T23:18:45Z); gates **663 unit + 6 integration, 0 failed**, the
two dead-store clippy warnings recorded pre-existing. The trail's follow-up
`6f0390f` named the delivery, and the ledger re-alignment pass rode the same
convention (`0d45242`). Work logs `2026-11-06_002.md`–`_004.md`.

**"the codex-code backend…" (FR-44 a–e)** — closed, 2026-11-05: delivered
at `122f320` and `4fee899` (the FR-44d amendment), the versions 0.42.0
then **0.43.0**, named by `af82155`/`d5cbbbe`; the FR-44e release `v0.43.0`
verified latest. Gates **669 unit + 6 integration, 0 failed**. Work logs
`2026-11-05_010.md`–`_012.md`.

**"the codex access investigation" (FR-43)** — closed, 2026-11-05 (no
release — investigation and report): the verdict feasible for
delegation-style access (D-65), the shape recorded unbuilt (D-66); FR-44's
cycle built it (FR-44a, D-67). Work log `2026-11-05_008.md`.

**"the refactoring…; the prompt's bottom row made prompt-only" (FR-41,
FR-42)** — closed, 2026-11-05: delivered at `eec12f9` and `064ce50`, the
version carried to **0.41.0** by `25fe13c`, named by the follow-up
`3badc04`. Gates **647 unit + 6 integration, 0 failed**, clippy 0
warnings. Work logs `2026-11-05_006.md`/`_007.md`.

**"the display corrections…; the elapsed display kept and the selection
unshifted" (FR-39, FR-40)** — closed, 2026-11-05: delivered at `db68d3f`,
named by `306f425`; the version carried to 0.40.0 by `505097a`, recorded by
`856c2bf`.

**"the four display regions" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05:
delivered at `39afeb2`, named by the follow-up `fb46cc0`; gates 645+6.

Earlier cycles stand in the requirements document's §12: the prompt-input
queueing (FR-35, v0.38.0 — delivered, then **removed** by FR-45; the
CHANGELOG's `[0.38.0]` section stands as history); the subagents'
auto-exit (FR-32–FR-34, v0.37.0 on `5a2d7f0`); the multi-angle subagent
analysis cycles (FR-31, FR-29/FR-30); the click rule's exclusion and the
cancelled-by-user display (FR-27/FR-28, v0.34.0 on `c022909`); the scroll
marker's face (FR-26, v0.33.0 on `6cc6661`); the running display
(FR-22–FR-24, v0.32.0 on `e8ad594`); the mouse defects (v0.31.0 on
`052e230`); add strata (FR-13–FR-17, v0.30.0–0.30.2); v0.29.0–v0.25.0 and
the 機能追加 addenda (v0.26.0 on `251a5bb`).
