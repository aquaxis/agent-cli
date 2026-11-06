# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the one-shot mode added — `agent-cli one-shot <prompt>`: one prompt in,
the answer printed as it arrives, and the session ends — no REPL, no IPC
server, no registry entry (the FR-46 display fix stands delivered; the
standing delivery sentence's clauses govern the completion)" (FR-47 a/b/c;
the standing delivery sentence of the instruction, whose 最優先事項 names the
addition)
(`.aiprj/instructions.md`, reloaded 2026-11-06 10:02:20 +0900 — sha256
`5f369e12f9be26ee870d8dcfe42605c0be53b159b357bb1f39bd41354ed892fb`, 800
bytes, recorded in §2)
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

`.aiprj/instructions.md` was extended at 2026-11-06 10:02:20 +0900 (800
bytes, sha256 `5f369e12…`, §2). FR-46's ask stands verbatim and **delivered**
(`ebe0aca`, v0.45.0, the release verified Latest — §12/§13); the newest
最優先事項 section adds **「ワンショットモードの追加」** — 「ワンショットで
完結するモードを追加する」 (add a mode that completes in one shot). The
shipped CLI at 0.45.0 had exactly two ways to run a prompt through the
agent: the interactive REPL (`run`, whose session continues) and a peer
relative (`send`/`ask` against someone else's `serve`/`spawn` process) —
no way to run **one prompt of one's own to completion and exit**. This
cycle adds it: **FR-47a** the `agent-cli one-shot <prompt>` subcommand —
one prompt in, the answer printed as it arrives, exit; **FR-47b** the
documentation follows; **FR-47c** the minor version carry 0.45.0 →
**0.46.0** with the commit, the push, and the release standing latest.
The ask is **delivered** (§13).

## 2. Source instruction

`.aiprj/instructions.md` reloaded for this execution (sha256
`5f369e12f9be26ee870d8dcfe42605c0be53b159b357bb1f39bd41354ed892fb`,
800 bytes, mtime 2026-11-06 10:02:20 +0900):

```text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する
機能追加した場合は、ドキュメント更新、example更新、マイナーバージョンアップ、commit, push, リリースを最新バージョンにする

## 最優先事項

### ターミナル表示の修正

ターミナルの最下位行を入力が可能なときはプロンプト表示、エージェントが実行中のときはクルクルと経過時間に修正する
最下位行は必ず、「プロンプト表示」か実行中を示す「クルクルと経過時間」を表示しなければいけない
プロンプト表示は入力下の状態のときのみ表示する

### ワンショットモードの追加

ワンショットで完結するモードを追加する
```

The standing delivery sentence supplies this cycle's completion shape
(documentation, example, minor version bump, commit, push, the latest
release), and the newest 最優先事項 section names the change it completes.

## 3. Baseline — verified in the tree at this cycle's start

Verified 2026-11-06 in the working tree at `5978dc9` (the FR-46 delivery
`ebe0aca`, v0.45.0 Latest, its ledger follow-up `5978dc9`; the tree clean,
`main` ≡ `origin/main`):

| ID | Fact | Evidence |
|---|---|---|
| BL-M56 | **The shipped surface had no way to run one of one's own prompts to completion and exit.** `Command`'s subcommands (`src/cli.rs:28–134`): `run` (the REPL; `app::run` `src/main.rs:63–66`), `serve` (headless IPC; `run_headless` `:67–70`), `spawn`/`stop`/`list`/`groups` (peer lifecycle; `:71–90`), `send`/`ask` (peer-relative prompts — both take a *running peer's* id/name, `src/cli.rs:77–92`; `commands::ask` resolves against the registry), `providers`/`doctor`/`update`/`selftest`/`config`/`mcp` (`:93–134`). Every agent-carrying entry point other than `run` targets *someone else's* session; `run_headless` (`src/app.rs:630`) stays resident serving prompts over IPC (registering itself in the shared registry, `RegistryEntry` … `registry_handle`), FR-32's `--exit-when-idle` ending it only after queued work drains. The assembly a one-shot needs is exactly `run_headless`'s: config overrides → persona resolution (`persona::resolve`, `src/app.rs:640–650`) → `apply_persona_overrides` → `ai::build` → registry dir/socket → IpcServer bind **(the part a one-shot must not do)** → registry/log/tools+MCP → `Agent { … approval_tx: None, cancel, exit_when_idle }` → `(input_tx, input_rx)`, `(event_tx, event_rx)`, shutdown watch → `agent.run(input_rx, event_tx)` (`:795+`) → the event consumer. The `Agent` struct's fields (`src/agent.rs:227+`) and `AgentEvent` (`:139` — `Text { delta }`, `Done`, `Error { … }`…) are the streaming surface the one-shot prints from. The `Command` help pin (`cli_help_compiles_and_lists_known_subcommands` `src/cli.rs:181–209`) enumerates the expected subcommand names. | `src/cli.rs`, `src/main.rs`, `src/app.rs`, `src/agent.rs` at `5978dc9`; the line citations above |
| BL-M57 | **The 0.45.0 ground.** `Cargo.toml`/`Cargo.lock` at 0.45.0; CHANGELOG `[Unreleased]` empty, `[0.45.0]` latest section; release `v0.45.0 — the terminal's bottom line shows the prompt when input is possible and the running clock while the agent runs` Latest (the releases API read at this cycle's ground, `createdAt` 2026-11-06T00:55:05Z; tag on `ebe0aca`); gates at the FR-46 close **663 unit + 6 integration, 0 failed** (the product tree unchanged since — the counts stand without a re-run at the baseline). The shipped docs' CLI surfaces: `README.md:358–369` (the command table) and `:19` (the quick-start row), `doc/usage.md:19` (the command table row), `doc/architecture.md:36` (the module tree's `app.rs` line) and `:323–336` (the `run`/`run_headless` entry-point paragraphs). The word `oneshot` already names two **other** surfaces (the `[provider.claude-code] transport` key `README.md:115`/`250`, and rust-`oneshot` channel types in the architecture doc) — the new subcommand's name must read distinctly. | `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, the releases API, the README/usage/architecture lines above, the FR-46 close's recorded gates |

## 3.1 What the instruction did NOT ask for

- **No change to the REPL, the headless serve, or any peer surface** — the
  one-shot is a new front end beside them; `run`/`run_headless`/`send`/`ask`
  behave exactly as shipped (NFR-14, verified §13's boundary record).
- **No IPC server, no registry entry for the one-shot process** — it is a
  single-shot tool invocation, not a long-lived peer (D-78's decision).
- **No history rewrite** — `[0.45.0]` and the closed ledgers' records stand.

## 4. Scope

### 4.1 In scope (delivered)

- **FR-47a** — the `agent-cli one-shot <prompt>` subcommand: one prompt in,
  the answer printed to stdout as it arrives, and the process exits at the
  turn's end; no REPL, no IPC server, no registry entry; the same startup
  options as `run` (`--provider`/`--model`/`--persona`/`--name`/`--group`/
  `--auto-approve-tools`) through the shared `RunArgs`; `--quiet` limits the
  output to the answer text.
- **FR-47b** — the documentation follows: the CHANGELOG's `[0.46.0]` entry in
  user language; the README command tables (`:19`, `:358–369`), `doc/usage.md`'s
  command table, and `doc/architecture.md`'s module/entry-point record name the
  new surface.
- **FR-47c** — the completion: the minor version carry 0.45.0 → **0.46.0**
  (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading), the delivery commit,
  the push, and the release published as the **latest** version.

### 4.2 Out of scope

- The §3.1 list: the REPL/headless/peer surfaces, and every shipped contract
  of §6 outside the added subcommand.
- A git tag beyond the release's own.

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-47a | The one-shot mode exists: `agent-cli one-shot <prompt>` runs the one prompt to completion through the configured backend (persona/model/temperature applied, tools and MCP servers attached, `[permissions]` in force), prints the answer text to stdout as it arrives (the `AgentEvent::Text` stream), and exits at the turn's terminal event; the process constructs **no** IPC server, holds **no** registry entry, and joins **no** peer surface; the clap subcommand name is `one-shot` (kebab — distinct from the `strata` transport key and the rust channel types `oneshot` carries elsewhere), and `--quiet` prints the answer text alone. The subcommand-list pin (`cli_help_compiles_and_lists_known_subcommands`) re-derives with the new name. — **Delivered** (T-108; design §1 D-78). |
| FR-47b | The documentation follows: the CHANGELOG's `[0.46.0]` entry records the new surface in user language; the README command rows and `doc/usage.md`'s table name `one-shot` with its shape (one prompt, the answer printed, exit — no REPL/IPC/registry; `--quiet`); `doc/architecture.md`'s module record names `run_oneshot` beside `run`/`run_headless`. Every statement bound to the code the delivery pins. — **Delivered** (T-109; design §1 D-79). |
| FR-47c | The completion: the minor version carry 0.45.0 → **0.46.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's heading), the delivery commit(s) of the mode and the records (the trail rule governs sha naming), the push to `origin/main`, and the release published and made the **latest** version (the `isLatest` probe re-asked at 0.46.0; the tag verified on the delivery commit). — **Delivered** (T-109; design §1 D-80). |

## 6. Boundary: FR-47 and the shipped contracts

The shipped contracts stand (NFR-14): the four-region screen (FR-37/FR-46),
the prompt/clock flip, the cancel keys, the approval takeover, the queueing
removal (FR-45), the codex-code backend (FR-44), the peer/IPC/registry
surfaces, the shutdown drain — none touched by the added subcommand. The
one-shot process never binds a socket, so a `spawn`-family peer list can
never even see it.

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | The addition stands pinned: `run_oneshot`'s assembly mirrors `run_headless`'s shipped one (the same persona/provider/tools/permissions order), and the gates re-derive at the changed tree — the suite green, clippy standing only the two pre-existing dead-store notes (C-09). |
| NFR-14 | The changes stay bounded by the boundary of §6: the new subcommand and the docs follow; no other shipped contract changes. |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism. This cycle's probes: the clippy/suite runs at the changed tree, **`agent-cli one-shot "Reply with exactly OK-ONEShot and nothing else"` executed live returning `OK-ONEShot`** and exiting, the release-latest/tag verifications at the close. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression standing: unit + integration green, clippy standing its two pre-existing dead-store notes in test code, re-derived at this cycle's tree — **re-executed at the fixed tree: 663 unit + 6 integration, 0 failed** (the new-mode code carries no new warning; the `one-shot` pin ran in the suite). |
| C-12 | The instruction's method — multi-angle subagent analysis — stands, and the standing method sentence survives in the instruction verbatim. The mode's analysis ran multi-angle (design §3) before D-78 named the sites. |
| C-14 | The delivery clause — the minor version carry, the commit, the push, the latest release — executed after the mode and its documentation stood complete (the gates green, the CHANGELOG entry written, the live probe returned). The delivery commits name the work they deliver. |
| C-15 | Write surface: the three `.aiprj` documents, the work log, and the product files the ask names through the standing delivery sentence (`src/cli.rs`, `src/main.rs`, `src/app.rs`, `CHANGELOG.md`, `README.md`, `doc/usage.md`, `doc/architecture.md`, `Cargo.toml`, `Cargo.lock`). Nothing else was written (the transient release-notes file removed after the `gh release create` call); no document carries any mark of authorship or tooling (NFR-12). |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-40 | FR-47 a/b/c met: the `one-shot` subcommand delivered at its named sites with the boundary intact (BL-M56's evidence re-verified), the suite green and clippy standing at the changed tree (C-09), the live probe executed (`OK-ONEShot` returned and the process exited), the documentation written and bound to the code (the CHANGELOG `[0.46.0]` entry, the README/usage/architecture rows), the version at **0.46.0** with the release files carried, the delivery committed and **pushed to `origin/main`**, and the release published as the **latest** version; the three `.aiprj` documents mutually consistent (C-08), the work log written per Art. 6/7, and the instruction re-verified byte-exact at exit (NFR-12 throughout). — **Met** (§13). |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-76 | 「ワンショットで完結するモード」 names a **mode of running one's own agent process**: one prompt through the configured backend to its terminal event, with the process exiting after — not a peer-relative send (`send`/`ask` already do prompt-and-optionally-answer against *someone else's* registry entry), and not a transport variant (`strata`'s `oneshot` key already names that). | Read from the instruction and the shipped surface inventory (BL-M56); applied in FR-47a. |
| AS-77 | The one-shot shares `run`'s startup options through the existing `RunArgs` (the provider/model/persona/name/group/auto-approve surface) rather than inventing a second flag set — the ask adds a *mode*, not a parallel configuration. `--quiet` is the one new flag (the mode's own display contract). | Read from `RunArgs`' shipped surface (`src/cli.rs:136+`) and `run_headless`'s reuse of it; applied in FR-47a. |
| AS-78 | 「マイナーバージョンアップ」 is the minor bump 0.45.0 → **0.46.0** — the trail carries the standing sentence's shape (0.44→0.45→0.46), and 「リリースを最新バージョンにする」 is the release published so it is the latest (FR-44e/FR-45c/FR-46c's verified shape). | Read from the instruction, the tree's trail (BL-M57) and the delivered cycles' records; applied in FR-47c. |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-47a | design §1 (D-78), §2 (TPR-29), §3 | T-108 ✅ |
| FR-47b | design §1 (D-79), §2 (TPR-29), §3, §4 (W-71) | T-109 ✅ |
| FR-47c | design §1 (D-80), §4 (W-71) | T-109 ✅ |
| NFR-13 | design §3 (the mechanism angle), §4 | T-108 ✅ |
| NFR-15 | design §3 (the live-verdict angle) | T-108, T-109 ✅ |
| C-09 | design §4 (W-71) | T-108 ✅ |
| C-14 | design §1 (D-80), §4 (W-71) | T-109 ✅ |
| AC-40 | design §1 – §4 | T-108, T-109, T-110 ✅ |
| FR-46 (delivered cycle) — closed | design §5 | (closed) |

## 12. The previous cycles' record

**"the terminal display fix" (FR-46 a/b/c)** — closed, 2026-11-06:
delivered at `ebe0aca` ("Fix the terminal's bottom line: the prompt when
input is possible, the running clock while the agent runs"), the version
carried 0.44.0 → **0.45.0**, the tag `v0.45.0`, the release published and
verified **Latest** (`createdAt` 2026-11-06T00:55:05Z); gates **663 unit + 6
integration, 0 failed**. The pending-cycle record was written by the ledger
passes `2557f7f`/`92c43fd` and the delivery was recorded by the follow-up
`5978dc9` (the trail rule). Work logs `2026-11-06_005.md`–`_007.md`.

**"the prompt-input queueing feature's removal" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c` (v0.44.0), the follow-ups `6f0390f`/
`0d45242` recording it; gates 663+6. Work logs `_001`–`_004`.

**"the codex-code backend …" (FR-44 a/b/c/d, FR-44e)** — closed,
2026-11-05: `122f320`/`4fee899`, versions 0.42.0/0.43.0, the release
`v0.43.0` verified latest; gates 669+6. Work logs `_010`–`_012`.

**"the codex access investigation" (FR-43)** — closed, 2026-11-05 (no
release): work log `_008`. **"the refactoring … the prompt's bottom row
made prompt-only" (FR-41, FR-42)** — closed, 2026-11-05: `eec12f9`/`064ce50`,
v0.41.0; gates 647+6. Work logs `_006`/`_007`. **"the display corrections…"
(FR-39, FR-40)** — closed, 2026-11-05: `db68d3f`/`306f425`, v0.40.0
(`505097a`, named by `856c2bf`). **"the four display regions" (FR-37,
FR-38)** — `39afeb2`, named by `fb46cc0`; gates 645+6. Later records: the
prompt-input queueing (FR-35, v0.38.0 — delivered, then **removed** by
FR-45); FR-32–FR-34 (v0.37.0 on `5a2d7f0`); FR-31; FR-29/FR-30; FR-27/FR-28
(v0.34.0 on `c022909`); FR-26 (v0.33.0 on `6cc6661`); FR-22–FR-24 (v0.32.0);
the mouse (v0.31.0); add strata (v0.30.0–0.30.2); v0.29.0–v0.25.0 and the
機能追加 addenda (v0.26.0).

## 13. Delivery status

| Item | State |
|---|---|
| FR-47 (the one-shot mode: the subcommand, the docs, the version 0.46.0, the commit/push, the latest release) | **Delivered** (2026-11-06): `agent-cli one-shot <prompt>` added (`Command::OneShot` `src/cli.rs`, the dispatch `src/main.rs:71–74`, `run_oneshot` `src/app.rs:978` — the `run_headless` assembly without the IPC server, one prompt → the `AgentEvent::Text` stream printed → exit at `Done`/`Error`); the subcommand-list pin re-derived (`one-shot` in the expected set); gates **663 unit + 6 integration, 0 failed**, clippy standing the two pre-existing dead-store notes, the live probe **`agent-cli one-shot "Reply with exactly OK-ONEShot and nothing else"` → `OK-ONEShot`** and the process exited; the docs follow (the CHANGELOG `[0.46.0]` entry, README `:19`+`:365`, usage `:20`, architecture `:36`+`:323–326`); the version carried 0.45.0 → **0.46.0** (`Cargo.toml`/`Cargo.lock`/the heading); the delivery commit `77e8a29` ("Add the one-shot subcommand: one prompt in, the printed answer out, then exit") pushed (`5978dc9..77e8a29`); the tag `v0.46.0` verified on `77e8a29` (`git rev-parse v0.46.0^{commit}`) and the GitHub release `v0.46.0 — the one-shot subcommand` published as **Latest** (`gh release list`'s Latest label + the release API read, `createdAt` 2026-11-06T01:16:40Z). |
| FR-46 (the terminal display fix) | **Delivered and closed** (2026-11-06; §12): `ebe0aca`, v0.45.0 Latest, the follow-up `5978dc9` recording it. |
| FR-45 (the queueing removal) / FR-44 / earlier | **Delivered and closed** (§12). |
| State at exit | The instruction as extended 2026-11-06 10:02:20 +0900 is re-verified byte-exact (800 bytes, sha256 `5f369e12…`) at the cycle's exit; the cycle is recorded in the three documents (FR-47 a/b/c, BL-M56–BL-M57, TPR-29, D-78–D-80, T-108/T-109/T-110, W-71, AC-40, AS-76–AS-78, C-08/C-09/C-12/C-14/C-15) and in the work log `2026-11-06_008.md` (embedding the instruction verbatim). The FR-47 commit's own naming stands per the trail rule — a commit's sha is nameable only from the commit that follows it. |
