# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: 「ドキュメントを整備する」 — **FR-59**, the FR-55 athena build face
executed as a **feature-addition cycle** (2026-11-09, this session;
FR-59a/b delivered through the delivery chain C-14 — T-159/T-160)
(FR-58 closed at four passes: the audit's verdict R-58 plus the correction
verdicts R-59/R-60/R-61 — its record standing in §12)
(`.aiprj/instructions.md`, **standing unchanged** at 2026-11-09 08:03:12
+0900 — 328 bytes, sha256
`a17757ca76731d5b202666754efdd0bb234ba38d1c0328e794e9438a896589c6`,
verified byte-exact at read and exit; its delivery sentence 「機能追加した
場合は、ドキュメント更新、example更新、マイナーバージョンアップ、commit,
push, リリースを最新バージョンにする」 **fires this cycle** — a feature
was added)
Prior cycles delivered and closed: the FR-58 documentation-tidy (four
passes, R-58/R-59/R-60/R-61); the parent-inaction diagnosis
(FR-57, 2026-11-09 — the verdict R-57, the 解説 delivered); the FR-56
report-reception investigation (R-56); the FR-55 athena-orchestration
formulation (`report_athena.md` — its build face executed as FR-59); the
FR-54 loop investigation (`report_loop.md`); before them, the FR-52/FR-53
investigations, the FR-50/FR-51 deliveries at `9562983`
(0.49.0) — §12
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

## 1. Purpose

The FR-58 documentation-tidy cycle closed at four passes with the audit's
tidy verdict (R-58 + the corrections R-59/R-60/R-61; §12). The session's
next job executed the standing **T-147** — the FR-55 formulation's build
face — as a **feature-addition cycle FR-59**: the instruction's delivery
sentence 「機能追加した場合は、…」 binds (AS-106 read again: the C-14
clause conditions the version/commit/push/release faces on a feature
actually being added — **this cycle adds one**), so the full delivery
chain executes.

**The feature**: the FR-55 formulation's parent-orchestrator persona, in
its generalized shipped-artifact form — `example/agents/orchestrator.md`
(the fourth bundled persona; AS-111 names the athena workspace's absence
on this machine, the observation environment living elsewhere, so the
athena-specific installation face generalized to the product artifacts).
**The same pass's finding**: the bundled `coder`/`reviewer` personas named
retired tool names (`shell`/`fs_read`/`fs_write`) — analyzed multi-angle
per C-12 (AS-112), no functional harm (the registry's alias map resolves
them, `src/tools/mod.rs:98–105`), fixed in the same carry.

The delivery faces (C-14, in the instruction's order): the doc updates
(`doc/personas.md` §7/§7.4/§8.2/§12), the example updates
(`example/agents/{orchestrator,coder,reviewer}.md`), the minor version
carry **0.49.0 → 0.50.0** (`Cargo.toml`/`Cargo.lock`), the CHANGELOG's
`[0.50.0]` section, the delivery commit, the tag `v0.50.0`, the push, and
the release standing latest.

## 2. Source instruction

`.aiprj/instructions.md` reloaded for each execution — **standing
unchanged** through the FR-58 cycle's four passes and this cycle:
328 bytes, sha256
`a17757ca76731d5b202666754efdd0bb234ba38d1c0328e794e9438a896589c6`,
mtime `2026-11-09 08:03:12 +0900` (second-precision). The instruction as
it stands:

```text
# agent-cliのドキュメント整備

作業は多角サブエージェント解析で実施する
機能追加した場合は、ドキュメント更新、example更新、マイナーバージョンアップ、commit, push, リリースを最新バージョンにする

## 最優先事項

ドキュメントを整備する
```

The standing header's method sentence (C-12) and delivery sentence (C-14)
survive verbatim; this cycle executes C-14 on its own condition's firing.

## 3. Baseline — verified live at this cycle's execution

The tree stood at the delivered 0.49.0 ground (BL-M71) entering the pass:
HEAD `9562983` ≡ `origin/main`, the tag `v0.49.0`, the release Latest;
the FR-58 audit's zero-finding record standing (§12). The pass re-pinned
its own baseline live:

- `report_athena.md` (14655 B, 209 lines, repo root — the FR-55
  formulation) re-read in full; **the athena workspace absent on this
  machine** (`/work/workspace/athena` not found — AS-111).
- The shipped personas' tool-name faces re-pinned (`coder.md` 535 B /
  `reviewer.md` 452 B at the delivered tree, both carrying `shell`/
  `fs_read`/`fs_write`); the registry's alias face re-pinned
  (`src/tools/mod.rs:98–105`: shell→bash, fs_read→read, fs_write→write).
- The gates re-derived at the delivered ground pre-change: **668 unit +
  6 integration, 0 failed**; the toolchain face re-pinned live
  (cargo/rustc/clippy **1.91.1**, rust-toolchain.toml channels=stable vs
  the ledgers' clippy 1.94.0 — the generation difference recorded, AS-113).

## 4. Scope

### 4.1 In scope

- **FR-59a** — the feature: the orchestrator example persona shipped
  (`example/agents/orchestrator.md`), the coder/reviewer tool-name
  corrections, and the shipping docs (`doc/personas.md`) carrying them.
- **FR-59b** — the delivery chain (C-14): the version carry 0.49.0 →
  0.50.0, the CHANGELOG's `[0.50.0]` section, the commit, the tag, the
  push, the release standing latest.

### 4.2 Out of scope

- The athena workspace itself (absent on this machine, AS-111); the
  FR-55 goal-prompt and parent-side instructions.md artifacts beyond what
  the shipped persona body carries; any Rust source change (none made);
  the ctime-face tracking and the aiprj parent-side instructions.md face
  (the carried-over investigations, deferred).

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-59a | The FR-55 build face delivered as shipped product artifacts: the fourth example persona `orchestrator` (the observe → decide → delegate → collect → iterate loop with its prohibitions, the FR-55 loop's faces folded in), the `coder`/`reviewer` personas' `allowed_tools`/`denied_tools` and body text naming the canonical registry tool names, and `doc/personas.md` carrying the four-example face (§7 intro, §7.4 the orchestrator quote byte-verified against the shipped file, §8.2 the orchestrator+coder+reviewer team, §12 the cheat-sheet row). |
| FR-59b | The delivery chain executed per C-14: the CHANGELOG's `[0.50.0]` section (Added + Fixed), the version carry 0.49.0 → 0.50.0 (`Cargo.toml`/`Cargo.lock`), the delivery commit, the tag `v0.50.0` on it, the push, and the release `v0.50.0` standing Latest (the notes = the tag's own CHANGELOG section, per the release workflow's extract). |

## 6. Boundary: the shipped contracts stand untouched

Every shipped contract stands (NFR-14): the four-region screen, the
prompt/clock flip, the wheel scrollback, the drag select-and-copy, the
thinking view, the running-turn mouse prohibition, the renamed backend
kinds, the one-shot mode, the peer/IPC/registry faces and the spawn tool,
the answer-display prompt handoff — **the FR-59 cycle touches none of
them**: no Rust source file changed, the suite re-derived green at the
changed tree twice (668 unit + 6 integration, 0 failed).

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named site carries file:line or a log/registry citation before any conclusion; claims the tree and the logs cannot evidence are withdrawn, not carried — this cycle's fmt-tree-wide application withdrawn after the generation-difference face surfaced (AS-113), the stray `tests/detached_spawn.rs` fold identified as this session's own fmt drop and restored to HEAD. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-14 | The changes stay bounded by the boundary of §6: the FR-59 cycle's product writes are the example personas, the doc page, the CHANGELOG, the version manifests, and `.gitignore` — no shipped screen/loop/IPC face. |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism where a mechanism executes — the persona parse verified by the shipped suite's own `bundled_example_personas_parse` (green with the fourth persona), the delivery by the actual chain (commit → tag → push → workflow). |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression standing (re-derived at the changed tree, twice): **668 unit + 6 integration, 0 failed**; clippy's notices generation-dependent (five recorded sites under 1.94.0; one under the live 0.1.91 — AS-113's record). |
| C-12 | The instruction's method — multi-angle subagent analysis — stands; executed this cycle on the tool-name finding (two analyzer children spawned per the FR-58 precedent; the verdict taken on the registry file:line face when one child's report was lost to the 120 s timeout). |
| C-14 | The delivery clause — executed in full this cycle (a feature added): the doc updates, the example updates, the minor version carry 0.49.0 → 0.50.0, the commit, the push, the release latest — its faces delivered at T-159/T-160. |
| C-15 | Write surface (this cycle): the example tree, the doc page, the CHANGELOG, the version manifests, `.gitignore`, the three `.aiprj` documents, the work log. No document carries any mark of authorship or tooling (NFR-12). |
| C-16 | The ledger writes at the pass stand uncommitted until the delivery commit; the delivered trail is nameable only from a commit that follows the pass's writes (the trail rule). |
| C-17 | Live-tree non-interference: the athena-workspace face read-only (it is absent, AS-111); the product writes bounded per C-15. |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-50 | FR-59 a/b met: the fourth persona shipped and parse-verified by the shipped suite (`bundled_example_personas_parse` green), the coder/reviewer tool-name faces canonical, `doc/personas.md` carrying the four-example face byte-verified (§7.4's quote = the shipped file), the CHANGELOG's `[0.50.0]` section standing with the `[Unreleased]` heading above, the version manifests at 0.50.0, the gates green at the changed tree (C-09), the delivery chain executed (commit → tag `v0.50.0` → push → release Latest), the three `.aiprj` documents mutually consistent (C-08), the instruction re-verified byte-exact at exit. |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-105 | 「ドキュメントを整備する」 names an audit over the product documentation surface whose deliverable is the verified state (and any drift it finds) — not a rewrite. | Read from the instruction (§2); carried from the FR-58 cycle (its record §12). |
| AS-106 | The C-14 delivery sentence's 「機能追加した場合」 conditions the version/commit/push/release faces on a feature actually being added. | Read from the standing header (§2); **fired this cycle** — a feature was added (FR-59a); applied in §1, §8 (C-14). |
| AS-107 | The FR-58 audit's citation faces stand exactly as the live tree evidences; the unevidenced ones withdrawn (T-156/T-157/T-158's chain). | Carried from the FR-58 cycle (§12; the closed-cycle sections of the tasks). |
| AS-108 | The FR-58 cycle's ledger-side transcription defects were ledger-side only, not product facts; corrected in-cycle. | Carried from the FR-58 cycle (§12). |
| AS-110 | The FR-58 third pass's work-log quote face (the は-drop) withdrawn at its fourth pass; the closed log stands unmodified per the Art. 6/7 append-only rule. | Carried from the FR-58 cycle (§12). |
| AS-111 | The athena observation workspace does not exist on this machine (`/work/workspace/athena` absent — verified live this pass); the FR-55 formulation's build face therefore installs its **generalized** artifacts into the shipped product tree, not any athena workspace. | Verified live (§3); applied in §1, §4, FR-59a. |
| AS-112 | The retired tool names in the shipped personas present no functional harm: the registry resolves them via the alias map (`src/tools/mod.rs:98–105` — shell→bash, fs_read→read, fs_write→write), so the built permission sets stand; the defect is presentational (the files name tools the registry registers under other names, and `doc/personas.md` §7's quote faces diverged). | Verified live (the registry face; the multi-angle analysis per C-12); applied in FR-59a. |
| AS-113 | The tree's live toolchain (cargo/rustc/clippy 1.91.1, rustfmt 1.8.0-stable; rust-toolchain.toml channels=stable) differs from the recorded clippy/cargo 1.94.0 face: the ledger-recorded gates read under the 1.94.0 generation; the fmt-tree-wide application this pass first applied was withdrawn (the delivery ground protecting against generation-driven churn), the stray `tests/detached_spawn.rs` fold it left behind restored to HEAD; clippy's notice count reads generation-dependent (five at 1.94.0, one at 0.1.91). | Verified live (`cargo -V`/`rustc -V`/`cargo clippy -V`/`cargo fmt --version`; the sweeps); applied in §3, §8 (C-09), NFR-11. |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-58a/FR-58b (closed) | design §2, §3 (the FR-58 audit's record) | T-154 ✅, T-155 ✅, T-156 ✅, T-157 ✅, T-158 ✅ (§12) |
| FR-59a | design §1, §2, §3 | T-159 ✅ |
| FR-59b | design §4 (W-86) | T-159 ✅, T-160 ✅ |
| AS-105–AS-108, AS-110 (carried) | design §2, §3 | §12 (the FR-58 record); T-154–T-158 |
| AS-111, AS-112, AS-113 | design §1, §2, §3 | T-159 ✅ |
| NFR-11, NFR-15, C-12 | design §1 (the method), §2, §3 | T-159 ✅ |
| NFR-14 (the boundary of §6), C-15/C-16/C-17 | design §2, §4 | T-159 ✅, T-160 ✅ |
| C-08 (six ledger passes this date) | design §1–§4 | T-155 ✅–T-158 ✅, T-159 ✅ |
| C-09 | design §3 (the gates' record) | T-159 ✅ |
| AC-49 (the FR-58 tidy) | design §2, §3 | T-154 ✅ (+ the corrections through T-158 ✅) |
| AC-50 (the FR-59 delivery) | design §4 (W-86) | T-159 ✅, T-160 ✅ |
| FR-57 (closed — the precedent) | design §5 | T-152/T-153 ✅ (§12) |
| FR-56 / FR-55 / FR-54 / FR-52 / FR-53 (closed investigations/formulations) | design §5–§6 | T-149 ✅, T-146 ✅, T-142 ✅, T-137 ✅ (§12; FR-55's build face = T-147 ✅ via FR-59) |
| FR-50 / FR-51 (delivered, closed at `9562983`) | design §6 | T-128–T-136 ✅ (§12) |
| FR-49 / FR-48 / FR-47 / FR-46 / FR-45 / FR-44 / FR-43 (closed) | design §6 | (closed) |

## 12. The previous cycles' record

**"the documentation-tidy audit" (FR-58 a/b)** — closed, 2026-11-09 (four
passes 08:05, 08:1x, 13:5x, 14:0x): the verdict R-58 — the product
documentation tidy at the delivered 0.49.0 tree; the correction verdicts
R-59/R-60/R-61 — T-156's three unevidenced citation withdrawals, T-157's
mtime-precision + doc-inventory withdrawals, T-158's work-log quote-face
withdrawal (the は-drop; AS-110). No product file, no commit, no release
(AS-106 then untriggered). The FR-58 cycle's full record stands in the
closed-cycle sections of the tasks and the design's §2–§4.

**"the parent-inaction diagnosis" (FR-57 a/b)** — closed, 2026-11-09
00:1x: the verdict R-57 standing (§12 of the prior pass's records). The
work log `2026-11-09_000.md`.

**"the peer-report reception investigation" (FR-56)** — closed,
2026-11-08 (R-56). **"the athena-orchestration formulation" (FR-55)** —
closed, 2026-11-08 (`report_athena.md`; its build face executed as
FR-59 — T-147 ✅). **"the loop-execution investigation" (FR-54)** —
closed, 2026-11-08 (`report_loop.md`). **"the FR-52/FR-53
investigations"** — closed 2026-11-08. **"the FR-50/FR-51 delivery"** —
closed at `9562983` (v0.49.0 Latest). **Earlier cycles** stand in the
trail.

## 13. Delivery status

| Item | State |
|---|---|
| The 2026-11-09 08:05–14:0x passes (the FR-58 cycle, four executions) | **Done — closed**: the audit's verdict R-58 plus the correction verdicts R-59/R-60/R-61; the withdrawal chain recorded (AS-107/AS-108/AS-110); no product file, no commit, no release (AS-106 untriggered then). |
| **FR-59 (the FR-55 build face as a feature addition)** | **Done — the delivery chain executed** (this session, 2026-11-09): the fourth persona `orchestrator` shipped (2145 B) and parse-green; the coder/reviewer tool-name canonicalization; `doc/personas.md` four-example face; the CHANGELOG `[0.50.0]`; the version carry 0.49.0 → 0.50.0; the gates green at the changed tree twice (668 unit + 6 integration, 0 failed); the commit → tag `v0.50.0` → push → release Latest delivered at T-160 (W-86). |
| FR-57 | **Delivered and closed** (2026-11-09, §12): the verdict R-57 standing. |
| FR-56 / FR-55 / FR-54 / FR-52 / FR-53 | **Delivered and closed** (2026-11-08, §12); FR-55's build face = the FR-59 cycle's T-147 ✅. |
| FR-50 / FR-51 | **Delivered and closed** (2026-11-08, §12): `9562983`, tag `v0.49.0`, release Latest. |
| FR-49 / FR-48 / FR-47 / FR-46 / FR-45 / FR-44 / FR-43 / earlier | **Delivered and closed** (§12; the design's §6 records stand). |
| State at exit (this session) | The instruction verified byte-exact (**328 bytes, sha256 `a17757ca…`, mtime `2026-11-09 08:03:12 +0900`**) at read and re-verified at close; the delivery commit pushed with **tag `v0.50.0`** and the release standing Latest (its sha named from the trail's next follow-up per the trail rule); FR-58 closed (§12); T-147 ✅; the gates standing their record (C-09: 668 unit + 6 integration, 0 failed, at the changed tree; clippy 1.91.1 live face — one notice — vs the recorded 1.94.0 face, AS-113); the three `.aiprj` documents + the work log `_005` joined the delivery commit. |
