# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the one-shot mode added — one prompt in, the answer out, the session
ends" (FR-47 a/b/c; the standing delivery sentence of the instruction, whose
最優先事項 adds a second ask beside the FR-46 clause)
(`.aiprj/instructions.md`, reloaded 2026-11-06 10:02:20 +0900 — 800 bytes,
sha256 `5f369e12…`, recorded in §2)
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

`.aiprj/instructions.md` was rewritten at 2026-11-06 10:02:20 +0900 (800
bytes, sha256 `5f369e12…`, §2). The FR-46 display-fix clause stands verbatim
(delivered this trail: `ebe0aca`, v0.45.0, the release Latest), and the ask
is new: **「### ワンショットモードの追加 — ワンショットで完結するモードを追加
する」** — add a one-shot mode: a mode that completes in one shot. The
instruction's standing sentences (the multi-angle method and the delivery
clause 「機能追加した場合は、ドキュメント更新、example更新、マイナーバー
ジョンアップ、commit, push, リリースを最新バージョンにする」) govern: the
addition completes as FR-47 a/b/c with the version carry 0.45.0 → **0.46.0**,
the commit, the push and the release standing latest.

The shipped CLI surface composes conversations through sessions: `run` starts
the REPL (the interactive loop around an `Agent`), `serve`/`spawn` start a
headless IPC peer, and `send`/`ask` target a *running* peer. There is no way
to hand the binary one prompt, get the answer, and be done: the closest
shapes — piping into the REPL (the input loop reads stdin and then waits in
`Pending` until Ctrl-D) and `ask` (a live peer required) — both leave a
session standing. The one-shot mode completes the surface: one prompt in,
the answer streamed to stdout, the session ends by itself.

The previous cycle's ask (FR-46, the terminal display fix) stands delivered
and closed (§12): delivered at `ebe0aca` (v0.45.0, the tag/release verified
Latest — `createdAt` 2026-11-06T00:55:05Z, the gates **663 unit + 6
integration, 0 failed**), the following ledger pass recorded the delivery
(`5978dc9`). The FR-46 text stands verbatim in the instruction — history and
completed delivery, not a re-opened ask.

## 2. Source instruction

`.aiprj/instructions.md` reloaded for this update (sha256
`5f369e12f9be26ee870d8dcfe42605c0be53b159b357bb1f39bd41354ed892fb`, 800
bytes, mtime 2026-11-06 10:02:20 +0900):

```text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する
機能追加した場合は、ドキュメント更新、example更新、マイナーバージョンアップ、commit, push, リリースを最新バージョンにする

## 最優先事項

### ターミナル表示の修正

ターミナルの最下位行を入力が possible なときはプロンプト表示、エージェントが実行中のときはクルクルと経過時間に修正する
最下位行は必ず、「プロンプト表示」か実行中を示す「クルクルと経過時間」を表示しなければいけない
プロンプト表示は入力下の状態のときのみ表示する

### ワンショットモードの追加

ワンショットで完結するモードを追加する
```

The standing delivery sentence supplies this cycle's completion shape
(documentation, example, minor version bump, commit, push, the latest
release), and the new 最優先事項 section names the change it completes. (An
earlier partial write of this document carried a `​.aid/…​` placeholder
artifact; it is recorded here and superseded by this complete record.)

## 3. Baseline — verified in the tree at this cycle's start

Verified 2026-11-06 in the working tree at `5978dc9` (the FR-46 delivery
`ebe0aca` + its ledger record; the tree clean, `main` ≡ `origin/main`),
before the one-shot wrote anything:

| ID | Fact | Evidence |
|---|---|---|
| BL-M56 | **The CLI composition is session-shaped; no one-shot path exists.** `Command` (`src/cli.rs:28+`) enumerates run/serve/spawn/stop/list/groups/send/ask/providers/doctor/update/selftest/config/mcp; `main.rs` dispatches each after `config::load`; the subcommand-list pin (`cli_help_compiles_and_lists_known_subcommands` `src/cli.rs:181`) asserts the fourteen names. `app::run` (`src/app.rs:244`) assembles persona → provider → registry/log/tools → the REPL loops; `run_headless` (`src/app.rs:630`) assembles the same and serves IPC; `commands::ask` (`src/commands.rs:776`-area) sends to a live peer over IPC and prints the reply. Every path leaves a session standing (a REPL awaiting input, or a headless peer in the registry). | `src/cli.rs`, `src/main.rs:62+`, `src/app.rs:244/:630`, `src/commands.rs` |
| BL-M57 | **The 0.45.0 ground.** `Cargo.toml`/`Cargo.lock` at 0.45.0; the CHANGELOG's `[Unreleased]` empty and `[0.45.0]` latest; the release `v0.45.0` verified **Latest** (the releases API read this cycle's start, `createdAt` 2026-11-06T00:55:05Z, the tag on `ebe0aca`); the gates at the FR-46 close **663 unit + 6 integration, 0 failed**, clippy carrying only the two pre-existing dead-store warnings in test code. The docs surface: the README's usage table and `doc/usage.md`'s run/serve sections name the session modes; the example config carries no mode surface (the provider keys only), so no example write exists unless the delivery contradicts it. | `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, the releases API, `gh release list` |
| BL-M58 | **The Agent assembly and the event stream are reusable for a one-shot path.** `run_headless` (`src/app.rs:630+`) assembles persona → provider → ConversationLog → ToolRegistry (+MCP attach) → `Agent` (auto-approve true, `approval_tx: None`, the cancel token, `exit_when_idle`) and spawns it with `(input_tx, input_rx)`/`(event_tx, event_rx)`; the display task consumes `AgentEvent`s (`display_event`, `src/app.rs:2421+`; the Text deltas stream the answer). The `AgentEvent` enum (`src/agent.rs:139+`) carries `TurnStart`/`Text`/`ToolCall`/`ToolResult`/`Thinking`/`Info`/`Error`/`Done`; the display's `display_event` formats each for a plain stream. A one-shot can assemble the same parts, send one `UserPrompt`, replay the events to stdout, and exit on the terminal event. | `src/app.rs:630+`, `src/agent.rs:139+` |

## 3.1 What the instruction did NOT ask for

- **No change to the shipped REPL/IPC machinery**: the REPL loops, the zone
  composition, the peer surface, the providers — none touched by the
  addition; the one-shot path is additive.
- **No deprecation of the session modes**: `run`/`serve`/`spawn` stand as
  shipped; the one-shot is an additional mode.
- **No history rewrite**: the CHANGELOG's entries stand; the closed ledgers'
  records stand closed.

## 4. Scope

### 4.1 In scope (recorded pending)

- **FR-47a** — the one-shot mode: a CLI surface (`agent-cli oneshot "<prompt>"`)
  that resolves config/persona/provider/tools exactly as the session modes
  do, runs **one prompt** through the agent, streams the answer to stdout,
  and ends the process itself. No REPL, no IPC server, no registry entry, no
  session left standing.
- **FR-47b** — the documentation follows: the CHANGELOG's `[Unreleased]`
  entry records the mode in user language and carries to the release
  heading; the README's subcommand list/usage table, `doc/usage.md`'s modes
  sections and `doc/architecture.md`'s entry-point paragraph follow the new
  surface; every statement bound to the code.
- **FR-47c** — the completion: the minor version carry 0.45.0 → **0.46.0**
  (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading), the delivery
  commit(s), the push to `origin/main`, and the release published as the
  **latest** version (the standing sentence's clauses; FR-45c/FR-46c's
  verified machinery).

### 4.2 Out of scope

- The §3.1 list: the REPL/IPC machinery, the provider backends, the FR-46
  display composition, every shipped contract outside the addition.
- A git tag beyond the release's own (the release machinery owns it).

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-47a | The one-shot mode exists: `agent-cli oneshot <PROMPT>` (or the design's named surface) takes exactly one prompt argument, assembles the agent exactly as the session modes do (config → persona → provider → tools/MCP), runs the one prompt, streams the answer (the `Text` deltas) to stdout — the final answer alone or the tool-call/result lines as the design binds — and ends: the process exits 0 on the turn's `Done`, 1 on an error event, with no REPL, no IPC server, no registry entry, no session left standing. The subcommand-list pin and the help surface name it. Every site bound to its file:line evidence before the change (BL-M56/BL-M58). — **Pending** (T-108; design §1 D-78). |
| FR-47b | The documentation follows the addition: the CHANGELOG's `[Unreleased]` entry records the mode in user language (what it is for, how to run it, what it prints and its exit codes) and carries to the release heading; the README's feature list/usage table, `doc/usage.md`'s modes sections and `doc/architecture.md`'s entry-point paragraph follow the new surface. Every statement bound to the code the delivery pins. — **Pending** (T-109; design §1 D-79). |
| FR-47c | The completion: the minor version carry 0.45.0 → **0.46.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's heading), the delivery commit(s) of the mode and the documents (the trail rule governs the sha naming — a commit's sha is nameable only from the commit that follows it), the push to `origin/main`, and the release published and made the **latest** version (the `isLatest` probe re-asked at 0.46.0, the tag verified on the delivery commit). — **Pending** (T-109; design §1 D-80). |

## 6. Boundary: FR-47 and the shipped contracts

The shipped contracts stand (NFR-14): the REPL's full surface (the display
fix FR-46 included — its standing shape and invariant untouched), the
headless serve/spawn/IPC peer machinery, `send`/`ask`'s live-peer contract,
the provider backends (nine kinds), the tools/MCP surface, the permissions
ruleset and the persona resolution. **The one-shot mode is additive**: no
shipped statement — a key path, a provider kind, a wire shape, a display
contract — changes. The subcommand list grows by one; nothing in it changes.

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | The mode stands pinned: pins bind the one-shot's shapes at the changed tree — the pin set re-derives with the addition (the subcommand list pin grows and the mode's own pins ride the delivery: the single-prompt contract, the exit codes, the no-session-standing property). The suite re-derives its counts at the delivery close (C-09). |
| NFR-14 | The changes stay bounded by the boundary of §6: the addition is the one mode; no other shipped contract changes. |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism. This cycle's probes: the one-shot round-trip (the mode executed live against the shipped provider machinery), the suite + clippy at the changed tree, and the release-latest/tag verifications at the close. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression standing: unit + integration green, clippy standing its two pre-existing dead-store notes, re-derived at the delivery close. |
| C-12 | The instruction's method — multi-angle subagent analysis — stands. The mode's analysis ran multi-angle (design §3) before D-78 named the sites. |
| C-14 | The delivery clause executes after the mode and its documentation stand complete (the gates green, the CHANGELOG entry written). The delivery commits name the work they deliver. |
| C-15 | Write surface: the three `.aiprj` documents, the work log, and the product files the ask names through the standing delivery sentence (`src/cli.rs`, `src/main.rs`, `src/app.rs`, the docs the docs clause names, `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`). Nothing else is written; no mark of authorship or tooling (NFR-12). |

## 9. Acceptance criteria

| ID | Criterion |
|---|—|
| AC-40 | FR-47 a/b/c met: the one-shot mode added and live-probed (the round-trip executed, the exit codes verified), the pins stand (the subcommand list pin grown, the mode's own pins green), the suite + clippy stand at the changed tree, the documentation written and bound to the code, the version at **0.46.0** with the release files carried, the delivery committed and **pushed to `origin/main`**, and the release published as the **latest** version; the three `.aiprj` documents mutually consistent (C-08), the work log written per Art. 6/7 of the Operation Guidelines, and the instruction re-verified byte-exact at exit (NFR-12 throughout). — **Pending** (§13). |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-76 | 「ワンショットで完結するモード」 names a mode whose whole cycle is: one prompt in → the answer out → the session over. The shipped session modes (`run`/`serve`) are its contrast. The mode leaves no standing process: no IPC server, no registry entry, no REPL awaiting input. | Read from the instruction's wording and the shipped session modes (BL-M56); applied in FR-47a. |
| AS-77 | The mode shares the session modes' assembly: config (the `--config` chain), persona, provider, tools/MCP, permissions, log — the one difference being the lifecycle (one prompt, then end). The `RunArgs` surface (`--provider`, `--model`, `--persona`, `--name`, `--group`, `--auto-approve-tools`) rides along so the mode accepts the same startup options. | Read from the tree (BL-M58's assembly) and the instruction's own delivery sentence (the example clause); applied in FR-47a's contract. |
| AS-78 | 「マイナーバージョンアップ」 is the minor bump 0.45.0 → **0.46.0**, and 「リリースを最新バージョンにする」 is the release published so it is the latest (the standing machinery, FR-45c/FR-46c's verified shape). | Read from the instruction and the tree's trail; applied in FR-47c. |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-47a | design §1 (D-78), §2 (TPR-29), §3 | T-108 ⬜ |
| FR-47b | design §1 (D-79), §2 (TPR-29), §3, §4 (W-71) | T-109 ⬜ |
| FR-47c | design §1 (D-80), §4 (W-71) | T-109 ⬜ |
| NFR-13 | design §3 (the pins angle), §4 | T-108, T-109 ⬜ |
| NFR-15 | design §3 (the live-verdict angle) | T-108, T-109 ⬜ |
| C-09 | design §4 (W-71) | T-108, T-109 ⬜ |
| C-14/C-15 | design §1 (D-80), §3 (the docs angle) | T-109 ⬜ |
| AC-40 | design §1 – §4 | T-108, T-109, T-110 ⬜ |
| FR-46 (delivered) — closed | design §5 | (closed) |

## 12. The previous cycles' record

**"the terminal display fix" (FR-46 a/b/c)** — closed, 2026-11-06:
delivered at `ebe0aca` ("Fix the terminal's bottom line: the prompt when
input is possible, the running clock while the agent runs"), the version
carried 0.44.0 → **0.45.0**, the tag `v0.45.0` on it, the release published
and verified **Latest** (`createdAt` 2026-11-06T00:55:05Z); gates **663 unit
+ 6 integration, 0 failed**, clippy standing the two pre-existing dead-store
notes. The recording pass `5978dc9` named it. Work logs
`2026-11-06_005.md`–`_007.md`.

**"the prompt-input queueing feature's removal" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c` (v0.44.0, the release Latest), the
follow-up `6f0390f` fulfilling the trail rule, the ledger re-aligned at
`0d45242`. Work logs `2026-11-06_002.md`–`_004.md`.

**"the codex-code backend …" (FR-44 a/b/c/d, FR-44e)** — closed, 2026-11-05:
`122f320`/`4fee899`, the versions 0.42.0/0.43.0, the release `v0.43.0`
verified latest. Gates **669 unit + 6 integration, 0 failed**.

**"the codex access investigation" (FR-43 a/b/c)** — closed, 2026-11-05 (no
release — the investigation): the verdict feasible for delegation-style
access (D-65), the shape recorded unbuilt (D-66); FR-44 built it. Work log
`2026-11-05_008.md`.

**"the refactoring … and the prompt's bottom row made prompt-only" (FR-41,
FR-42)** — closed, 2026-11-05: delivered at `eec12f9` and `064ce50`, the
version carried to 0.41.0 by `25fe13c`, named by `3badc04`. Gates **647 unit
+ 6 integration, 0 failed**.

**"the display regions…" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05:
delivered at `39afeb2`, named by `fb46cc0`; gates 645+6. FR-39/FR-40 closed
at `db68d3f`/`306f425`; the version carried to 0.40.0 (`505097a`, recorded
by `856c2bf`). FR-35 — closed, 2026-11-04 (`c02862d`, v0.38.0); **its
delivered feature was removed by FR-45**. FR-32–FR-34 — closed, 2026-11-03
(v0.37.0 on `5a2d7f0`). Earlier cycles stand in `2026-11-06_007.md`'s
record and the design document's §5.

## 13. Delivery status

| Item | State |
|---|---|
| FR-47 (the one-shot mode: the CLI surface, the single-prompt lifecycle, the docs, the version 0.46.0, the commit/push, the latest release) | **Pending** (recorded 2026-11-06): the ask reloaded at 10:02:20 +0900 with the baselines BL-M56–BL-M58 verified live in the tree at `5978dc9`, the analysis verdicts recorded (design §3) and the sites named (design §1 D-78); the implementation rides T-108, the completion T-109, the ledger's ground T-110. |
| FR-46 (the terminal display fix) | **Delivered and closed** (2026-11-06; §12): `ebe0aca`, v0.45.0, the release Latest, gates **663 + 6, 0 failed**. |
| FR-45 (the prompt-input queueing feature's removal) | **Delivered and closed** (2026-11-06; §12): `b5deb9c`, v0.44.0. |
| State at exit | The instruction as rewritten 2026-11-06 10:02:20 +0900 is re-verified byte-exact (800 bytes, sha256 `5f369e12…`) at this update's exit; the cycle is recorded in the three documents (FR-47 a/b/c, BL-M56–BL-M58, TPR-29, D-78–D-80, T-108/T-109/T-110, W-71, AC-40, AS-76–AS-78, C-08/C-09/C-12/C-14/C-15) and in the work log `2026-11-06_008.md` (embedding the instruction verbatim). The FR-47 delivery commit stands ahead per the trail rule — a delivery commit's sha is nameable only from the commit that follows it. |
