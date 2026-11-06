# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the one-shot mode added — one prompt in, the answer out, the session
ends" (FR-47 a/b/c)
(`.aiprj/instructions.md`, reloaded 2026-11-06 10:02:20 +0900 — sha256
`5f369e12…`, 800 bytes, recorded in requirements §2; the tree carries
v0.45.0, its release `v0.45.0` verified Latest at the delivery close of the
FR-46 cycle and re-read at this cycle's ground)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`.

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What is decided, and what it costs

The instruction's newest 最優先事項 section is an addition: 「ワンショットで
完結するモードを追加する」 — add a mode that completes in one shot. The
shipped surface today offers three session shapes: the interactive REPL
(`run`), the headless IPC server (`serve`, `spawn`), and the peer-targeting
queries (`send`/`ask` — both need an already-running peer). No shape runs one
prompt locally and exits. The design decision is **D-78 — the one-shot mode
is a new `oneshot` subcommand that assembles an agent exactly as `run_headless`
does, feeds it one prompt, prints the streamed answer to stdout, and exits**
— no IPC server, no registry entry (a transient session that outlives
nothing), no REPL, no display task: the answer is the output. The standing
delivery sentence governs the completion: **D-79** the docs surface,
**D-80** the version/commit/push/release shape.

- **D-78 — the one-shot pipeline, headless-shaped and terminal-bound.**
  In `src/cli.rs`: a new `Command::Oneshot { text, … }` variant — the
  prompt as the positional argument — carrying the same
  provider/model/persona/name overrides `RunArgs` already gives (the mode
  reuses `RunArgs` by flattening, so every override the REPL takes works
  here). In `src/main.rs`: the dispatch arm loads the config exactly like
  `Run`/`Serve`. In `src/app.rs`: a new `pub async fn run_oneshot(...)` —
  the `run_headless` assembly minus the IPC server and the registry write:
  persona resolution (`persona::resolve` with the same project-persona
  read), `config.apply_persona_overrides`, `ai::build`, the tool registry +
  MCP attach, the conversation log, the `Agent` struct with `approval_tx:
  None` and `exit_when_idle: true` semantics by construction. The turn:
  `AgentInput::UserPrompt(text)` sent once, the input channel then closed
  (dropping `input_tx`), and the event loop consumes `Text` deltas printing
  them raw to stdout until `Done`/`Error` — the answer on stdout exactly as
  the model streamed it, with tracing going to stderr only (the shipped
  `init_tracing` writer) so the output stays pipe-clean. Exit: `Ok(())` on
  `Done`, the error path on `Error`. The session ends — no channel left
  open, no server to drain.

  1. **The transcript log still opens.** The conversation log is agent-cli's
     durable record for every session shape; the one-shot opens its own
     (`ConversationLog::open`), so the turn is inspectable after the fact.
     The registry entry is *not* written: the session is process-scoped and
     lives exactly as long as the answer.
  2. **No approval takeover.** `approval_tx: None` with the headless auto-
     approve reading (a deny rule is the only stop); the shipped
     `[permissions]` semantics ride unchanged.
  3. **The cancel token is not exposed.** No interactive user can raise it;
     the process ends with the answer (or an error). SIGINT behaves as any
     CLI's: the default termination.
  4. **The streamed answer owns stdout.** No banner, no spinner, no theme
     colouring of the answer (`Theme::plain()` semantics by default) — the
     mode's output is a composable Unix stream, the same bytes `ask` prints
     for its peer's answer.

- **D-79 — the docs surface follows the new subcommand.** The CHANGELOG's
  `[Unreleased]` becomes the `[0.46.0]` entry (### Added) recording the
  mode in user language: `agent-cli oneshot "<prompt>"` — one prompt, the
  answer on stdout, the session ends; the `--provider`/`--model`/
  `--persona`/`--name` overrides work as in `run`. The README gains the
  one-shot bullet beside the scriptable one (its scriptability bullet
  already names `agent-cli run` piping — the bullet is corrected to name
  `oneshot` as the one-prompt form); `doc/usage.md` gains the mode
  paragraph in the CLI section; `doc/architecture.md`'s process-shape
  paragraph names the third session shape; the `/help` text is not touched
  (the mode is a CLI subcommand, not a REPL command). The example config
  carries no new key (the mode reads the same `[provider]`/`[runtime]`
  surface), so no example write exists.

- **D-80 — the completion repeats the standing delivery machinery
  (AS-78).** The minor version carries **0.45.0 → 0.46.0** (`Cargo.toml`,
  `Cargo.lock`, the CHANGELOG's `[0.46.0]` heading), the delivery
  commit(s) of the mode and the documents are pushed to `origin/main` (the
  trail rule governs the sha naming), and the GitHub release is published
  and made the **latest** version (the FR-45c/FR-46c verified shape: the
  `isLatest`/latest-flag probe re-asked at 0.46.0, the tag verified on the
  delivery commit via `git rev-parse`).

The instruction's clauses map one-for-one onto the decisions:
ワンショットモード → D-78's pipeline; ドキュメント更新 → D-79; the method
C-12 → §3's angles; マイナーバージョンアップ + commit, push + リリース最新 →
D-80. 「example更新」 names the example configuration: the mode adds no
config key (evidence: the D-78 assembly reads only existing keys), so no
example write exists.

## 2. The touchpoints this cycle touches (TPR-29)

| # | Touchpoint | Evidence (the files read in this cycle) |
|---|---|---|
| TPR-29 | The session shapes and the assembly the mode mirrors: `src/cli.rs` (`Command` enum `:28–150`, the subcommand-list pin `:181–207`, the run-args pin `:212–249`); `src/main.rs` (the dispatch `:57–115`); `src/app.rs` — `run_headless` (`:630–790`, the assembly the mode mirrors: the persona resolution `:637–660`, `ai::build` `:665`, the IPC bind `:667–675` (the part the mode drops), the registry entry `:678–700` (dropped), the tool registry + MCP `:706–710`, the `Agent` struct `:730–770`), `AgentEvent` (`src/agent.rs:139+` — `Text`/`Done`/`Error` the event loop consumes), `display_event`'s headless reading (`:2421+`), `commands::ask`'s stdout printing (peer answer shape, `src/commands.rs`); the docs surface — `README.md`'s scriptable bullet, `doc/usage.md`'s CLI sections, `doc/architecture.md`'s session-shape paragraphs; `CHANGELOG.md` `[Unreleased]`; `Cargo.toml`, `Cargo.lock`; the subcommand list the pin asserts | The files' own lines, read in this cycle; BL-M56/BL-M57's records |

## 3. Analysis verdicts (C-12) — the multi-angle analysis of the addition

- **The mechanism angle.** The Agent's pipeline is already state-safe for a
  fire-once session: `process_turn` is driven purely by `AgentInput` values
  over the channel, and closing the channel ends the loop cleanly (the
  FR-32 quiet-exit semantics). The mode is therefore the `run_headless`
  assembly with its server parts subtracted — no new state machine, no new
  event type, no new config key.
- **The contracts angle (NFR-14).** The shipped session shapes stand: the
  REPL (`run`) untouchsed, `serve`/`spawn`'s registry surface untouched,
  `ask`/`send`'s peer contract untouched. The one-shot is an additive
  subcommand; the subcommand-list pin grows by one entry and no existing pin
  changes.
- **The scope angle (§3.1).** **Not** touched: the persona resolution
  (shared code, read-only reuse), the provider builds, the permission
  semantics, the conversation log format, the IPC transport, the REPL's
  entire display stack, and every shipped key meaning. The mode does not
  register in the peer registry and does not answer IPC.
- **The live-verdict angle (NFR-15).** This cycle's probes, executed at the
  delivery: the suite + clippy at the changed tree, the one-shot's own pin
  set (the CLI parse, the dispatch, the event-loop fold checks), and the
  release-latest/tag verifications at the close. The ground probe executed
  at this pass: the release `v0.45.0` Latest re-read (the releases API,
  `createdAt` 2026-11-06T00:55:05Z), the tree clean at `5978dc9`.
- **The docs angle (C-15, D-79).** The ledger's three documents and the
  work log; the products the standing sentence names (the source surfaces,
  the README/usage/architecture paragraphs, the CHANGELOG, the version
  files); nothing else.

## 4. Verification of the cycle (the gate table W-71)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-71 | FR-47 a/b/c — the `oneshot` subcommand added at D-78's named sites (the CLI variant, the dispatch, the `run_oneshot` pipeline: one prompt in, the streamed answer to stdout, the session ends — no IPC server, no registry entry), the shipped session shapes and every other contract intact, the docs rewritten/extended per D-79, the version carry to **0.46.0**, the delivery committed and pushed, the release standing latest | FR-47a, FR-47b, FR-47c, NFR-13, NFR-14, NFR-15, AS-76, AS-77, AS-78, TPR-29 | T-108, T-109, T-110 | ⬜ |

*W-71's execution record stands at the delivery close (T-108/T-109's
record). The ground this cycle verifies: the release `v0.45.0` Latest
(re-read 2026-11-06, `createdAt` 2026-11-06T00:55:05Z), the tree clean with
`main` ≡ `origin/main`, the version files at 0.45.0, the gates at the FR-46
close **663 unit + 6 integration, 0 failed** (the product tree unchanged
since `ebe0aca`).*

## 5. The previous cycles' records (already retired; one shared record with the requirements' §12)

**"the terminal display fix — the bottom-most line's composition" (FR-46
a–c)** — closed, 2026-11-06: the mid-turn standing shape inverted at
`ebe0aca` ("Fix the terminal's bottom line: the prompt when input is
possible, the running clock while the agent runs" — the clock block's last
row the screen's bottom row mid-turn, no prompt display while the agent
runs, the approval span as the one armed input-possible face), the version
carried 0.44.0 → **0.45.0**, the tag `v0.45.0` on the delivery commit, the
release published and verified **Latest** (`createdAt`
2026-11-06T00:55:05Z); gates **663 unit + 6 integration, 0 failed**, the two
dead-store clippy warnings pre-existing. The recording pass (`2557f7f`), the
re-load pass (`92c43fd`) and the delivery-follow-up (`5978dc9`) rode the
trail convention. Work logs `2026-11-06_005.md`–`_007.md`.

**"the prompt-input queueing feature's removal" (FR-45 a–c)** — closed,
2026-11-06: delivered at `b5deb9c`, the version carried 0.43.0 → **0.44.0**,
the tag `v0.44.0`, the release verified **Latest**; gates **663 unit + 6
integration, 0 failed**. Trail: the follow-up `6f0390f`, the ledger
re-alignment `0d45242`. Work logs `2026-11-06_002.md`–`_004.md`.

**"the codex-code backend … and the example's sample models refreshed"
(FR-44 a–e)** — closed, 2026-11-05: delivered at `122f320` and `4fee899`,
the versions 0.42.0/0.43.0, the release `v0.43.0` verified latest; gates
**669 unit + 6 integration, 0 failed**. **"the codex access investigation"
(FR-43)** — closed, 2026-11-05 (no release). **"the refactoring … and the
prompt's bottom row made prompt-only" (FR-41, FR-42)** — closed, 2026-11-05
(`eec12f9`/`064ce50`, the carry to 0.41.0). **"the display corrections"
(FR-39, FR-40)** and **"the four display regions" (FR-37 a/b/c, FR-38)** —
closed, 2026-11-05 (the FR-46 cycle superseded the four-region shape's
bottom pair mid-turn; the relative stacking survives). Earlier records
stand in the requirements document's §12: the prompt-input queueing (FR-35,
v0.38.0 — delivered, then removed by FR-45); the subagents' auto-exit
(FR-32–FR-34, v0.37.0); the multi-angle analysis cycles (FR-31, FR-29/30);
the click rule and cancelled display (FR-27/FR-28, v0.34.0); the scroll
marker (FR-26, v0.33.0); the running display (FR-22–FR-24, v0.32.0); the
mouse defects (v0.31.0); add strata (v0.30.0–0.30.2); v0.29.0–v0.25.0.
