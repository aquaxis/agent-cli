# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the codex-code backend — the installed codex CLI is driven the way the
claude-code backend drives `claude`, as a ninth `provider.kind` with its own
`[provider.codex-code]` section, and it is documented, version-carried and
pushed"
(FR-44 a/b/c)
(`.aiprj/instructions.md`, rewritten 2026-11-05 15:55 (+0900), then
2026-11-05 16:46 (+0900))
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What is decided, and what it costs

The instruction carries one ask clause, 「実装してください」, and the
investigation cycle (FR-43, the previous records) built the design it needs:
**D-65** (the verdict — feasible as a delegation backend — with the
mapping and the costs) and **D-66** (the shape a future implementation
would take — the sibling provider, the kind, the config surface). This
cycle builds D-66's shape from D-65's constraints, on the probes
BL-M46/BL-M47 measured, and the delivery clause (ドキュメント、config
sample 更新、マイナーバージョンアップ、commit, push) is satisfied in §2–§4
below. The method sentence 「作業は多角サブエージェント解析で実施する」
stands; the subagent analysis of the investigation cycle is the analysis
the implementation is measured against (AS-66), and the reconciliation is
recorded with the probes' own transcripts.

- **D-67 — the backend is `CodexCodeProvider`, a delegation-only,
  per-turn-spawn CLI provider.** One `codex exec` spawn per turn — **no
  resident child** (codex exposes no resident stdin protocol on the probed
  surface, so the claude-code resident mode D-52 has no codex counterpart).
  Two transports over the same argv: `transport = "items"` (the default)
  runs `codex exec --json`, maps `item.completed` items to
  `ProviderEvent::Text` (whole text per item — no token deltas on the probed
  surface, AS-68's stated cost), pins `thread.started`'s `thread_id` and
  resumes it from the second turn through `codex exec resume <thread_id>
  --json`; `transport = "oneshot"` runs a bare `codex exec` with
  `--output-last-message <file>` and reads the reply back from the file,
  sessionless by design (no `--json`, no thread id is ever seen).
  `session = "persistent"` (the default; the claude-code `persistent`
  behaviour D-52 reached by one spawn per turn) and `session =
  "ephemeral"` (`--ephemeral`, each conversation stands alone) are the
  session knobs. The transcript is never re-sent on a resume leg: only the
  user turns codex has not seen ride the turn's stdin (P2/P7), and the
  system text rides the fresh leg's payload (a persona change forces a
  fresh conversation). `capabilities().tool_use` is **false** — codex's
  own tools arrive already executed inside codex (`command_execution`
  items, P9/P9v), so the delegation rule's never-re-run mapping holds and
  the REPL's approval prompt is never involved. A **gateway mode is not
  offered** (no `--tools ""` counterpart — D-65's recorded gap), and no
  dedicated system-prompt flag exists, so the persona rides the fresh leg's
  turn payload, as D-65 names the gap.

- **D-68 — the argv geometry is flags-first, and the probes decided
  it.** `build_args` pins the order: exec-level flags first (`--json` /
  `--output-last-message`, `-s <sandbox>`, `--add-dir`, `-c
  approval_policy=...`, `--dangerously-bypass-approvals-and-sandbox`,
  `-m <model>`, `--skip-git-repo-check`, `--ephemeral`, `extra_args`),
  then the `resume <thread_id>` subcommand — live probes falsified the
  alternative geometry («resume first, flags after») when `codex exec
  resume <id> --json -s workspace-write` failed with `error: unexpected
  argument '-s' found` under `codex exec resume [OPTIONS] [SESSION_ID]
  [P guillotine PROMPT]`, so the live E2E probe (the release binary, the
  config's kind, two conversational turns on one thread) drove the
  flags-first rewrite, and the corrected geometry survives in a pinning
  test (`resume_leg_carries_the_subcommand_first`). The sandbox default is
  **`workspace-write`**, not codex's own read-only (P12's live read-only
  rejection is the evidence: delegation must run commands); the trust
  gate's mapping is `skip_git_repo_check = true` by default, emitting
  `--skip-git-repo-check` (the probe-backed refusal without it is the
  reason, not an inference). `stderr` is drained continuously (an
  unbounded channel); at turn end the tail is drained to EOF and the last
  `STDERR_TAIL_LINES` lines quote in errors — the resume-not-found
  signature («no rollout found for thread id», code -32600) clears the
  recorded thread and the next turn re-enters fresh. EOF with rc=0 is the
  normal turn end (no "result" line exists); a nonzero exit is one
  `ProviderEvent::Error` + Done. The timeout kills the child and drops the
  recorded thread (900 s default, `turn_timeout_secs`).

- **D-69 — the delivery shape is additive, and the version carry is
  D-66's build.** The wiring is additive over the shipped contracts: one
  dispatch arm (`src/ai/mod.rs`), one `[provider.codex-code]` entry type
  (`src/config.rs`, the shared `ProviderEntry` fields the claude-code and
  codex entries also carry), one `providers()`/`doctor()` arm
  (`src/commands.rs`), one kind-list addition (`src/cli.rs`, `spawn.rs`),
  the sample config block, and the docs set (README ×2, the
  `doc/providers/codex-code.md` guide, `doc/config.md` §2/§3, CHANGELOG).
  No shipped backend is modified; the version carries **0.41.0 → 0.42.0**
  (the minor bump 「マイナーバージョンアップ」 — the ask's own
  terminology), and the CHANGELOG's `[0.42.0]` entry carries a
  user-language summary of the codex-code backend. The two commits are the
  release pattern's: the delivery commit carries the code, the docs and
  this rewrite, and the carry commit afterwards names it from behind; the
  push sends both.

The instruction's clauses map one-for-one onto the decisions: 実装 →
D-67/D-68; ドキュメント & config sample → D-69 and the docs set; the
method C-12 → AS-66 (the analysis the implementation is measured against);
マイナーバージョンアップ → D-69's minor bump.

## 2. The touchpoints this cycle touches (TPR-26)

| # | Touchpoint | Evidence (the files read in this cycle) |
|---|---|---|
| TPR-26 | The provider surface: `src/ai/codex_code.rs` (~1,390 lines with the stub-based pins), the dispatch `src/ai/mod.rs` (`build()` arm + `SUPPORTED`), the config `src/config.rs` (the `codex_code` slot, `provider_entry`/`_mut` arms, the shared `ProviderEntry` fields: `sandbox`, `add_dir`, `approval_policy`, `bypass_sandbox_and_approvals`, `skip_git_repo_check`; the sample `[provider.codex-code]` in `DEFAULT_CONFIG`), the CLI's `--provider` list (`src/cli.rs`), `src/tools/spawn.rs`'s provider list, `src/commands.rs` (`providers()`'s codex-code bin arm, `doctor()`'s codex-code arm — the binary + `--version` check, 20 s timeout) | The files' own lines, read in this cycle; the delivery's docs set (README ×2, `doc/providers/codex-code.md`, `doc/config.md`, CHANGELOG) rides the same commit |

## 3. Analysis verdicts (C-12) — the subagent analysis, the implementation measured against it

- **The access-point angle.** The design children's verdict — a sibling
  provider behind the same `Provider` trait, a distinct `codex-code` kind,
  additive wiring — was carried by `src/ai/codex_code.rs` + the dispatch
  arm + the config/CLI wiring; no shipped backend is modified, and the
  claude-code backend stands untouched (NFR-14's boundary).
- **The mechanism angle.** `codex exec` per turn, `--json` default, the
  resume subcommand as the session — the shape D-65 named stands built;
  the transport split (`items`/`oneshot`) matches the claude-code
  `stream`/`oneshot` pair with codex's own granularity (turn-grain
  items).
- **The delegation angle.** `command_execution` items never map to
  `ToolUse`, `capabilities().tool_use = false`, the REPL's approval
  prompt not involved — D-65's delegated-tools rule is enforced at the
  parser's site and in `capabilities()`.
- **The surface angle.** No token deltas (whole texts per item), no
  gateway mode, no system-prompt flag — the recorded gaps stand as
  documented costs in the docs set; the parser ignores unknown lines and
  unparsable lines rather than failing the turn (the claude-code
  module's own rule).
- **The live-verdict angle (NFR-15).** The E2E probes' transcripts are
  the delivery's evidence: fresh + resume legs on one thread through the
  wired kind (P1–P12 in-process; the release E2E AIPROBE-92 chain).
- **The docs angle (C-13).** The ledger's identifiers were read and the
  three documents written from the evidence; no identifier was invented
  ahead of its evidence.

## 4. Verification of the cycle (the gate table W-67)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-67 | FR-44 — the backend implemented (the delivery: `src/ai/codex_code.rs`, the dispatch, the config surface, the CLI/provider lists, the sample config, the docs set), the delivery clause satisfied in full — docs, config sample, the minor version carry to **0.42.0** | FR-44a, FR-44b, FR-44c, NFR-11, NFR-14, NFR-15, AS-66, AS-67, AS-68, C-14, TPR-26 | T-97, T-98, T-99 | ✅ |

*The row stands ✅ at the recorded tip: the gates ran green at the close
(**669 unit + 6 integration, 0 failed**; clippy **0 warnings**), the live
E2E (the release binary's wired kind: fresh+resume legs, AIPROBE-92)
carried the backend's runtime shape, and the release commits stand as
recorded in the requirements' §13.*

## 5. The previous cycles (recorded after their documents were retired)

**"the codex access investigation — the verdict, the mapping, the costs
and the shape a future implementation would take" (FR-43)** — closed,
2026-11-05 (no release — investigation and report; the tree untouched,
BL-M48). The verdict **feasible for delegation-style access** (D-65), the
shape recorded unbuilt (D-66), the probes BL-M46/BL-M47 the shared
evidence, T-95/T-96 ✅; the work log `2026-11-05_008.md`. Its
identifiers stand as this cycle's ground: the implementation is D-66
delivered. (The investigation cycle's instruction — 241 bytes, sha256
`1d813156…` — was superseded by the implementation ask, then re-written
again with the delivery clause, `.aiprj/instructions.md` at
2026-11-05 16:46.)

**"the refactoring — contradictions and waste in the source are removed,
`src/ai`'s provider error chain and the tools' test context each collapse
to one implementation, clippy falls silent, and the documents follow the
source; and the prompt's bottom row is prompt-only" (FR-41, FR-42)** —
closed, 2026-11-05, delivered at `eec12f9` (the refactoring) and
`064ce50` (the display fix), the version carried to 0.41.0 by `25fe13c`,
named by the follow-up `3badc04`. Gates at the close **647 unit + 6
integration, 0 failed**, clippy silent.

**"the display corrections — the `Message below` marker above the prompt;
the elapsed display kept and the selection unshifted" (FR-39, FR-40)** —
closed, 2026-11-05, delivered at `db68d3f`, named by `306f425`; the
version afterwards carried to 0.40.0 by `505097a`, recorded by `856c2bf`.

**"the terminal's display regions…" (FR-37 a/b/c, FR-38)** — closed,
2026-11-05, delivered at `39afeb2`, named by `fb46cc0`; gates **645 unit
+ 6 integration, 0 failed**.

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)** — closed, 2026-11-04,
delivered at `c02862d`, v0.38.0, the tag `v0.38.0`, gates **633 unit + 6
integration, 0 failed**.

Later records stand in the requirements document's §12: the
subagents' auto-exit (FR-32–FR-34, v0.37.0 on `5a2d7f0`); the multi-angle
subagent analysis cycles (FR-31, FR-29/FR-30); the click rule's
exclusion and the cancelled-by-user display (FR-27/FR-28, v0.34.0); the
scroll marker's face (FR-26, v0.33.0); the running display (FR-22–FR-24,
v0.32.0); the mouse defects (v0.31.0); **add strata** (FR-13–FR-17,
v0.30.0–0.30.2); v0.29.0–v0.25.0 and the 機能追加 addenda (v0.26.0).
