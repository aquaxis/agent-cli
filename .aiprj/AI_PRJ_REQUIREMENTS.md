# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the codex-code backend — the installed codex CLI is driven the way the
claude-code backend drives `claude`, as a ninth `provider.kind` with its own
`[provider.codex-code]` section, and it is documented, version-carried and
pushed"
(FR-44 a/b/c)
(`.aiprj/instructions.md`, rewritten 2026-11-05 16:46 (+0900))
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

`.aiprj/instructions.md` was rewritten at 2026-11-05 16:46:10 +0900
(365 bytes, sha256 `918d14a7…`, §2) and replaces the 241-byte investigation
ask the previous cycle's ledger recorded (FR-43, §12). Its method sentence
「作業は多角サブエージェント解析で実施する」 and its heading 「# agent-cliの
機能追加」 stand; its ask is now the implementation itself: **codex access
implemented like the claude-code access, the documentation and the
config.toml template updated, and — after completion — a minor version
bump, a commit and a push**. The cycle is the D-66 shape the previous
cycle's ledger recorded as "stand ready for the next cycle" — delivered
now as the `codex-code` backend.

The ask has its ready ground in the tree:

- the **precedent and the probe record** (BL-M46–BL-M48, §3 of the
  previous cycle): the claude-code backend's delegation semantics, the
  codex CLI's `exec` surface probed live on 0.166.0, and the verdict
  D-65/D-66 recorded — the implementation cycle builds exactly what that
  verdict named (the sibling provider, the thread-id session, the
  turn-grain items, the sandbox/trust knobs, no gateway mode).
- the **implementation** the session's own work has already carried
  (BL-M49, §3): `src/ai/codex_code.rs` — the `CodexCodeProvider` behind
  the same `Provider` trait, wired at the dispatch (`src/ai/mod.rs`), the
  config (`[provider.codex-code]` with the sandbox/approval knobs), the
  CLI's `--provider`/`spawn` provider lists, `providers`/`doctor` — live
  E2E-probed fresh **and** resume turns, and gated **669 unit + 6
  integration, 0 failed** at the pre-doc tree.
- the **remaining ask** (FR-44a): the two READMEs, the per-backend guide
  `doc/providers/codex-code.md` and `doc/config.md` §6's kind/section
  rows; the config template; then the version carry to **0.42.0**, the
  commit and the push the completion clause asks (C-14).

The previous cycles are not withdrawn: FR-43 (the investigation) stands
recorded in the closed ledger, and the implementation work rides this
cycle's own record (§12).

## 2. Source instruction

`.aiprj/instructions.md` as reloaded in this session (rewritten
2026-11-05 16:46:10.974056706 +0900, 365 bytes; sha256
`918d14a7d104579e62a53ecc914cb103e592c2f2e1b2dfe0e124a4cc77f1bb22`;
verified byte-exact at this session's gates and re-verified at exit):

````text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する

## 最優先事項

claude codeへのアクセスと同様にcodexへのアクセスを実装してください
ドキュメント、config.tomlのテンプレートなどの更新を実施して下さい
完成後、マイナーバージョンアップ、commit, pushする
````

(Translation: **Feature additions to agent-cli. The work is to be performed
by multi-angle subagent analysis. Top priority — implement codex access
the same way claude code access is implemented. Carry out the updates of
the documentation, the config.toml template, and so on. After completion,
a minor version bump, a commit, a push.**)

The instruction names three asks in order: **実装** (implement, the
claude-code-like codex access), **更新** (update the docs and the
config.toml template), and **完成後** (after completion — the minor
version bump, the commit, the push). All three are this cycle's
deliverables (FR-44a/b/c).

## 3. Baseline — verified in the tree at this update

Verified 2026-11-05 in the working tree ahead of `3badc04` — the session's
own implementation work standing uncommitted (the delivery commits this
cycle then make, per the trail rule, name it in the ledger's follow-up):

| ID | Fact | Evidence |
|---|---|---|
| BL-M49 | **The implementation stands in the tree, wired and probed.** `src/ai/codex_code.rs` (untracked, ~1,380 lines) carries `CodexCodeProvider` — the codex sibling of `claude_code.rs`: `Transport::Items` (`--json`, prompt on stdin, thread id from `thread.started`, later turns `codex exec resume <id>`; probe-verified flag geometry, exec-level flags *before* the `resume` subcommand) / `Transport::OneShot` (no `--json`, prompt as argv tail, reply read from `--output-last-message` → `.agent-cli-codex-code-last.txt`, deleted after the turn); `SessionMode::Persistent`/`Ephemeral` (`--ephemeral` every turn, sessionless by design); delegation only — `command_execution` items **never** mapped to `ProviderEvent::ToolUse`, `capabilities().tool_use == false`; the sandbox knobs (`sandbox` → `-s`, default `workspace-write` because codex's own read-only default blocks delegation; `add_dir`, `approval_policy`, `bypass_sandbox_and_approvals`, `skip_git_repo_check` default true); the resume-not-found signature clearing the thread; the timeout killing the child and clearing the thread; stderr drained via a channel. Wired: `src/ai/mod.rs` (the dispatch arm, `SUPPORTED` += `codex-code`), `src/config.rs` (`codex_code` slot, `provider_entry[_mut]` arms, the sample `[provider.codex-code]` in `DEFAULT_CONFIG`, the `codex-code` keys on the shared `ProviderEntry`), `src/cli.rs` (`--provider`/`spawn` lists), `src/tools/spawn.rs` (`:60`), `src/commands.rs` (`providers()` resolves the bin; `doctor()` checks the binary + `--version`), `.gitignore` (the oneshot artifact). Live E2E on the release build: fresh leg `AIPROBE-92`, resume leg answering the same token from memory, the codex rollout file on disk (`0.166.0`). | `src/ai/codex_code.rs`, `src/ai/mod.rs`, `src/config.rs`, `src/cli.rs`, `src/tools/spawn.rs`, `src/commands.rs`, `.gitignore` in the working tree; the live E2E transcript (fresh + resume, `AIPROBE-92`) |
| BL-M50 | **The gates: green, and the one real defect they caught is fixed.** A full solo run after the argv-geometry rewrite: **669 unit + 6 integration, 0 failed**, clippy **0 warnings**; repetition then exposed a flake (`second_leg_resumes_the_thread_the_first_opened`, ~1 in 15 full runs) and diagnosed it — `failed to send the prompt: Broken pipe (os error 32)`, the prompt write racing a child that already exited (the stub codex reading no stdin and the trust-refusal case behave alike) — and the cycle fixed the source: the prompt's write/shutdown tolerate EPIPE on `src/ai/codex_code.rs`'s spawn, letting the turn's normal EOF/status/stderr path report the real outcome; after the fix **15/15 full-suite repetitions ran green** (and the solo 669+6/clippy run as above). The earlier `non_zero_exit…`/`eof_with_success…` failures were concurrency ETXTBSY of the stub harness (two cargo processes at once), never reproduced solo. | The session's runs (the solo full run 669+6 green, the clippy run clean) |
| BL-M51 | **The docs before this cycle's writes.** `README.md` line 11 "Eight backends" (the nine kinds not yet listed), the backend table without codex-code, the capability table without a codex-code column, no claude-code-style section; `README_ja.md` the same shape; `doc/config.md` §2's layout comment and §3's `kind` row without codex-code, no `[provider.codex-code]` item reference; no `doc/providers/codex-code.md`; `CHANGELOG.md` latest `[0.41.0]`; `Cargo.toml` 0.41.0 (the release commit `25fe13c`). | `README.md`, `README_ja.md`, `doc/config.md`, `CHANGELOG.md`, `Cargo.toml` at the pre-edit tree |

## 3.1 What the instruction does NOT ask for

- **No change to the shipped codex wire backend.** `provider.kind = "codex"`
  (`src/ai/codex.rs`), its `[provider.codex]`/`[provider.strata]` handling
  and its tests stand untouched — the instruction's ask is the CLI-driven
  access, a sibling, not a change of the wire.
- **No change to the `claude-code` backend.** It is the measuring stick —
  its files stand untouched.
- **No release or tag machine is asked beyond the version.** The
  完成後 clause names the minor bump, the commit and the push; a GitHub
  release and a tag are not asked (the trail rule governs the commits).

## 4. Scope

### 4.1 In scope

- **FR-44a** — the implementation and its wiring (carried in the tree per
  BL-M49): the `codex-code` backend, the config table with the codex-side
  knobs, the `DEFAULT_CONFIG` sample block, the CLI surfaces.
- **FR-44b** — the documentation: the kind/name surfaces in the two
  READMEs (backend count, the backend table, the claude-code-style
  section, the capability table, the per-backend links), the new
  per-backend guide `doc/providers/codex-code.md`, `doc/config.md`'s §2
  layout, §3 `kind` row and §6-style `[provider.codex-code]` item
  reference, the config template's `codex-code` block (already in
  `DEFAULT_CONFIG`), and the CHANGELOG's release entry.
- **FR-44c** — the completion: the minor version carry (0.41.0 → 0.42.0:
  `Cargo.toml` + `Cargo.lock` + the CHANGELOG heading), the commit, the
  push.

### 4.2 Out of scope

- The §3.1 list: the shipped codex wire backend, the claude-code backend,
  and every shipped contract of §6.
- A git tag and a GitHub release (not asked).

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-44a | The implementation, standing in the tree per BL-M49 and named in the ledger: the `codex-code` backend behind the same `Provider` trait — delegation only (command_execution items never become agent-cli tool calls, capabilities carried accordingly), session continuity by codex's own thread id with the probe-verified exec-flag geometry, transports `items`/`oneshot`, session `persistent`/`ephemeral`, the sandbox/approval knobs codex-side, the resume-not-found and timeout recoveries, no gateway mode (recorded as not implementable, D-65's verdict carried), no system-prompt flag (the persona rides the turn payload). Wired end to end: `build()`, `provider.kind = "codex-code"`, the `[provider.codex-code]` table, the `--provider`/`spawn` lists, `providers`/`doctor`. |
| FR-44b | The documentation updated: the two READMEs (「Eight backends」→「Nine」, the `codex-code` row in the backend table, the capability table's codex-code column, the claude-code-style section, the per-backend link), the guide `doc/providers/codex-code.md` (the prerequisites, the configuration, the key table, the delegation vs gateway question, the process model, the supported features, the verification, the known limitations, the troubleshooting — the claude-code guide's own shapes), `doc/config.md` (§2 the layout comment; §3 the kind row; the `[provider.codex-code]` item reference), and the CHANGELOG's 0.42.0 entry. Every statement bound to the code or a live probe. |
| FR-44c | The completion: the minor version carry 0.41.0 → 0.42.0 (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's `[0.42.0]` heading), the delivery commit(s) of the implementation and the documents (the trail rule — a commit's sha is nameable only from the commit that follows it — governs the naming), and the push to `origin/main`. |

## 6. Boundary: FR-44 vs the shipped contracts

The shipped contracts stand (NFR-14): the queue semantics (FR-35), the
bare-`Enter` guard (FR-38), the cancel keys and the `Esc`-to-live-view
path, the approval takeover, the outcome rows' recording, the four exit
seams and the teardown/drain, the mouse-capture and clipboard contracts,
the peer rows' face and stacking (FR-37b), the four display regions and
the geometry corrections FR-39/FR-40, the prompt-only rows FR-42, the
refactoring's collapsed shapes FR-41. FR-44 adds the codex-code backend
beside them: it reuses the provider dispatch, the config entry shapes and
the CLI surfaces without changing any of their shipped behaviour; the
claude-code and codex wire backends stand untouched (§3.1). No shipped
statement is superseded, and nothing changes a key path, a recording rule
or a wire shape (NFR-14).

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | The implementation re-derives its own pins: the `codex_code.rs` stub-based tests (the two-leg resume, the resume-not-found recovery, the timeout recovery, the one-shot file, the EOF-with-success end, the argv geometry with the resume subcommand first, the fresh/resume prompt shapes) pin the probed shapes; the suite stands green. |
| NFR-14 | The changes stay additive to the shipped contracts (the boundary of §6). |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism. This cycle's probes: the argv-geometry falsification (resume-leg flags order), the E2E fresh + resume legs. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression standing: unit + integration green, clippy silent, re-run after this cycle's tree changes — 669 unit + 6 integration, 0 failed; clippy 0 warnings. |
| C-12 | The instruction's method — multi-angle subagent analysis — is performed, and the standing method sentence survives in the instruction verbatim. The implementation's record names it: the design was reconciled against the live probes (§12's method sentence; FR-43's C-12 record stands behind D-65/D-66). |
| C-13 | Write surface: the three `.aiprj` documents, the work log, and the product files the ask itself names (the implementation, the docs, the config template, the version files). Nothing else is written; no document carries any mark of authorship or tooling (NFR-12). |
| C-14 | The 完成後 clause — the minor version carry, the commit, the push — is executed after the implementation and documentation stand complete (the gates green, the docs written). The delivery commits name the work they deliver. |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-37 | FR-44 a/b/c met: the `codex-code` backend stands implemented, wired and E2E-probed (BL-M49) with the suite green and clippy silent (C-09); the documentation stands written (the two READMEs, the guide, `doc/config.md`, the CHANGELOG) with every statement bound to the code or a probe; the version stands at **0.42.0** with the release files carried; the delivery is committed and **pushed to `origin/main`**; the three `.aiprj` documents are mutually consistent (C-08), the work log written per Art. 6/7, and the instruction re-verified byte-exact at exit (NFR-12 throughout). |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-66 | 「同様に」 carries the previous cycle's verdict forward as the measuring stick: the delegation semantics, the session continuity, the per-turn shape and the capabilities the claude-code backend gives the project (AS-64 of FR-43) are what the codex-code backend is built to match, within the codex CLI's own knobs (the sandbox/trust/approval flags, the thread-id session) and the recorded costs (no deltas, no gateway, no system-prompt flag). | Carried from D-65's verdict clauses and the requirements §3 of FR-43's cycle; applied in FR-44a. |
| AS-67 | 「マイナーバージョンアップ」 is the minor bump 0.41.0 → **0.42.0** — the new backend is a feature (SemVer's minor), and the project's own trail carried 0.39→0.40→0.41 the same way. | Read from the tree's trail (BL-M51) and from SemVer; applied in FR-44c. |
| AS-68 | The completion clause's 「commit, push」 means the delivery commits of this cycle's tree (implementation, docs, version carry, ledger) pushed to `origin/main`; a tag and a GitHub release are not named and stand unasked. | Read from the instruction and the tree's trail (§12's trail: releases are carries of a version the user names); applied in FR-44c. |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-44a | design §1 (D-67), §2 (TPR-26), §3 | T-97 |
| FR-44b | design §1 (D-68), §2 (TPR-26), §3, §4 (W-67) | T-98 |
| FR-44c | design §1 (D-69), §4 (W-67) | T-99 |
| NFR-13 | design §3 (the pins angle), §4 | T-97 |
| NFR-15 | design §3 (the live-verdict angle, the argv falsification) | T-97 |
| C-09 | design §4 (W-67) | T-97 |
| C-14 | design §1 (D-69), §4 (W-67) | T-99 |
| AC-37 | design §1 – §4 | T-97, T-98, T-99 |

## 12. The previous cycles' record

**"the codex access investigation — driving the installed codex CLI the
way the `claude-code` backend drives `claude` is probed live and
reported" (FR-43 a/b/c)** — closed, 2026-11-05 (no release — the
investigation; the ledger rewrote and stood uncommitted by the trail's
rule until this cycle's delivery commits). The verdict **feasible for
delegation-style access** (D-65): the codex CLI's `exec` surface answers
the claude-code pattern's mechanisms — construction (a `[provider.*]`
kind reaches a CLI backend through `build()`, no credential needed where
ChatGPT auth stands), session continuity (`thread.started`'s `thread_id`
feeding `codex exec resume <thread_id> --json`, history accumulating
codex-side), tool ownership (`command_execution` items arrived already
executed — delegation), streaming granularity (**no deltas on the probed
exec surface** — text whole per `item.completed`), persona (no
append/replace flag), trust/approvals (`--skip-git-repo-check`, the
sandbox flags). The future shape (D-66 — a sibling `CodexCodeProvider`
behind the same trait, its own kind, the codex-side knobs) **is this
cycle's implemented deliverable** (FR-44a; design §1, D-67). The method
ran (FR-43c): the subagent verdict was reconciled against the live
probes, the probe deciding. Work log `2026-11-05_008.md`.

**"the refactoring — contradictions and waste removed, the documents
followed to the source; the prompt's bottom row made prompt-only" (FR-41,
FR-42)** — closed, 2026-11-05: delivered at `eec12f9` and `064ce50`, the
version carried to **0.41.0** by `25fe13c` and named by the ledger's
follow-up `3badc04` (the delivery 064ce50's own ledger rewrite standing
uncommitted until then). Gates **647 unit + 6 integration, 0 failed**,
clippy 0 warnings; work logs `2026-11-05_006.md` and `_007.md`.

**"the terminal's display regions…" (FR-37 a/b/c, FR-38)** — closed,
2026-11-05: delivered at `39afeb2`, named by the tracking follow-up
`fb46cc0`; gates 645+6. FR-39/FR-40 closed at `db68d3f`/`306f425`; the
version carried to 0.40.0 (`505097a`, recorded by `856c2bf`).

**"prompt input during a running turn…" (FR-35)** — closed, 2026-11-04
(`c02862d`, v0.38.0, the tag and the release; gates 633+6).
**"the subagents' auto-exit…" (FR-32–FR-34)** — closed, 2026-11-03
(FR-34 released **v0.37.0**, the release commit `5a2d7f0`; FR-32
`7fe1370`, FR-33 `af48be5` — the versions 0.35.0/0.36.0 never tagged;
the tag `v0.36.0` on `ddeaf67` stands restored — BL-M32). **FR-31** —
closed, 2026-11-03 (no release). **FR-29/FR-30** — closed, 2026-11-03
(no release). **FR-27/FR-28** — closed, **v0.34.0** (`c022909`; the
tag's final target `745dbb2`). **FR-26** — v0.33.0 (`6cc6661`).
**FR-22–FR-24** — v0.32.0 (`e8ad594` + `e0ecb1b`). **the mouse** —
v0.31.0 (`052e230`). **add strata** — v0.30.0–0.30.2 (`b706889`,
`70e6b5b`, `a766757`). **v0.29.0** (`045d569`). **v0.28.0** (`d237a7a`).
**v0.27.0** (`afcbe70`). **v0.25.0** (`3884d16`) with the addenda
**v0.26.0** (`251a5bb`).

## 13. Delivery status

| Item | State |
|---|---|
| FR-44 (the codex-code backend: implementation, docs, version, commit, push) | **Delivered.** The backend stands behind the same `Provider` trait with the probed delegation semantics (FR-44a; BL-M49; D-67), the documentation stands written everywhere the ask names (FR-44b; T-98), the version carried to **0.42.0** (FR-44c; the release files + the CHANGELOG `[0.42.0]`), the gates green at the close (**669 unit + 6 integration, 0 failed; clippy 0 warnings**), and the delivery committed and pushed (AC-37). The trail: the delivery commit(s) of this cycle's implementation tree, named in the ledger's follow-up per the trail rule. |
| FR-43 (the investigation, previous cycle) | Closed and standing (§12); its D-65/D-66 verdict is the shape this cycle implemented (D-67). |
| State at exit | The instruction is re-verified byte-exact (365 bytes, sha256 `918d14a7…`, mtime 2026-11-05 16:46:10.974056706 +0900); the cycle is recorded in the three documents (FR-44 a/b/c, BL-M49–BL-M51, TPR-26, D-67–D-69, T-97–T-99, W-67, AC-37, AS-66–AS-68, C-12/C-14) and in the work log `2026-11-05_009.md` (embedding the instruction verbatim). The product tree stands at the version 0.42.0 with the gates green; the delivery commits exist and are pushed (C-14, FR-44c); the ledger's rewrite rides its own commit per the trail rule. |
