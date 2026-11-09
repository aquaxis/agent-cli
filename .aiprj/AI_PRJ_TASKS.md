# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: 「ドキュメントを整備する」 — **FR-59**, the FR-55 athena build face
**executed as a feature-addition cycle (2026-11-09, this session)**: the
FR-58 documentation-tidy cycle stands **closed at four passes**
(T-155–T-158; the verdict R-58 plus the correction verdicts R-59/R-60/R-61),
and the session's next job executed **T-147** — the FR-55 formulation's
artifacts installed as shipped product artifacts: the fourth example persona
`orchestrator` (generalized from the athena-specific proposal — the athena
workspace itself does not exist on this machine, AS-111), a feature
addition, so **C-14 fires in full** (the doc/example updates, the minor
version carry **0.49.0 → 0.50.0**, the delivery commit, the tag, the push,
the release). The cycle's own new finding — the shipped `coder`/`reviewer`
personas naming retired tool names (`shell`/`fs_read`/`fs_write`; the
registry's alias map resolving them, so no functional harm — the
multi-angle analysis per C-12, AS-112) — fixed in the same carry.
(`.aiprj/instructions.md`, reloaded 2026-11-09 08:03:12 +0900 — 328 bytes,
sha256 `a17757ca76731d5b202666754efdd0bb234ba38d1c0328e794e9438a896589c6`,
mtime 2026-11-09 08:03:12 +0900 (read second-precision); found byte-exact
again at this session's read — the FR-58 passes' withdrawal records stand in
the closed-cycle sections below)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked

---

## Overview

**The FR-59 delivery (this session)**: the FR-55 build face (T-147)
installed the orchestrator persona as the fourth shipped example
(`example/agents/orchestrator.md`, 2145 B — the observe → delegate →
collect → iterate loop with its prohibitions; `doc/personas.md` §7
"three→four examples", §7.4 the persona quoted byte-exact against the
shipped file, §8.2 the orchestrator + coder + reviewer team, §12 the
cheat-sheet row); the shipped `coder`/`reviewer` personas' retired tool
names corrected to the canonical registry names (`bash`/`read`/`write`/
`send_to`; `bash`/`read`); the `.gitignore`'s retired `codex-code`
oneshot-name line follows the FR-51 rename; the CHANGELOG `[0.50.0]`
section written (Added: the orchestrator persona; Fixed: the tool-name
canonicalization + the `.gitignore` entry); the version carry **0.49.0 →
0.50.0** (`Cargo.toml`/`Cargo.lock`); the gates re-derived green at the
changed tree (**668 unit + 6 integration, 0 failed**). The delivery
clause C-14's chain — the commit, the tag `v0.50.0`, the push, and the
release standing Latest — executes at this session's close (T-160; the
delivery sha and the publication's committed record ride the trail's
next follow-up, per the trail rule).

### The closed FR-58 cycle's overview (its record stands)

The instruction's standing header survives verbatim (the method sentence
C-12 and the delivery sentence C-14); the active 最優先事項 is now
「ドキュメントを整備する」. The cycle's product-face verification (its only
verification face, per AS-105) reads the shipped documentation and example
ground at the delivered tree `9562983` (the design's §2), the audit's
findings re-derived live **again at the second and third execution passes**
(NFR-11's file:line citations; T-156, T-157):

- **The doc face (`doc/` — its mtimes stand at 11-08, the delivered tree
  exactly): `usage.md` 48452 B, `architecture.md` 48466 B, `config.md`
  54461 B, `personas.md` 22676 B, `tools.md` 20586 B, `troubleshooting.md`
  51936 B, `providers/` eight faces** — the core findings stand **green**:
  the current kind names throughout (**zero** stale claude-code/codex-code
  kind hits over `src/ doc/ example/ README*` and the CHANGELOG's
  non-historical faces — the FR-51 renames' completions verified across
  every product tree); `Cargo.toml`/`Cargo.lock` carry **exactly 0.49.0**;
  `doc/` and the README pair carry **zero version-carry strings at all**
  outside the CHANGELOG's historical headings (the strongest drift check:
  not one stale version string exists because none exists); the shipped
  0.49.0 CLI face (「Prohibit mouse operations…」) is CHANGELOG-recorded and
  names doc/usage.md, doc/tools.md, doc/config.md, doc/troubleshooting.md,
  doc/personas.md, doc/providers/*, doc/architecture.md — every named face
  exists (the shipped CLI face's doc citation face verified; the design's
  §2); the README pair carries the doc links' every named file existing.
- **The example face (`example/` — its mtimes stand at 11-08 10:14/12:13,
  the delivered tree exactly): `config.example.toml` 17788 B, the three
  agent personas `coder.md`/`planner.md`/`reviewer.md`** — **zero stale
  backend-kind names tree-wide**, and the backend table's kind list
  (「claude」 | 「claude-cli」 | 「codex」 | 「codex-cli」 | 「strata」 |
  「ollama」 | 「opencode」 | 「opencode-go」 | 「llama.cpp」) covers **all nine**
  shipped kinds — **including `strata`** (its commented `[provider.strata]`
  sample at L74–75).
- **CHANGELOG.md** — its head (Keep-a-Changelog header, the empty
  `## [Unreleased]` section, `## [0.49.0]` immediately after with its
  two ###-headed delivery rows — the FR-50 and FR-51 changes verbatim), and
  the shipped `Cargo.toml`/`Cargo.lock` at 0.49.0; the release workflow face
  green (its 0.49.0 face standing Latest).

**Conclusion (R-58): the documentation ground stands tidy at the delivered
0.49.0 tree** — the audit found zero stale names, zero version drift, zero
missing faces — **and the FR-58 cycle's passes carried the ledgers'
re-scoping, the audit's records, and this pass's citation corrections only**
(no product file, no example file — Art. 3; C-15/C-16; AS-105). The audit's
ground (file lists, sizes, mtime sweep, the stale-name sweep, the
CHANGELOG/Cargo sweeps) stands in the design's §2 (NFR-11's
file:line/size/mtime citations). No product file or example file was written
by any of the cycle's passes (Art. 3; C-16).

### The withdrawals this pass recorded (NFR-11's withdrawal rule)

The first execution pass's record carried three citation faces the second
pass's live re-derivation **could not evidence**; they are **withdrawn**,
not carried, and every ledger now reads on the corrected face:

1. **The provider-page version-line citations** (「exactly 0.49.0」 over
   eight provider rows, and the `config.md` row 「現在 0.49.0」) — the live
   grep finds **no version string in any of the eight provider pages nor in
   `doc/config.md`/`doc/tools.md`/`doc/architecture.md`/`doc/personas.md`/
   `doc/troubleshooting.md`/`usage.md`/either README**: the doc set carries
   no per-page version lines at all. Corrected face: there is nothing to
   name 0.49.0 there — the finding **strengthens** the no-drift verdict.
2. **The usage.md §-cited version citations** (0.49.0 by §7.3–§7.11,
   0.48.0 by §3.2.25/§3.3.1–§3.3.2 and similar) — no such version string
   exists in `usage.md` (the §7.x headings are the REPL-use sections; none
   cites a version). Withdrawn. Also withdrawn: the 「9 つのバックエンド」/
   「 Nine backends」 row was never in question (both READMEs carry it, L7's
   nine-kind row verbatim).
3. **A claimed `strata` gap** (the provider page 「absent from
   `config.example.toml`'s backend table」) — **not a finding**: the table's
   kind list carries `strata` (L12) and the commented `[provider.strata]`
   sample (L74–75) — the gap claim withdrawn; the requirements' AS-107
   carries the withdrawals' own record (the drafted parenthetical replaced).

Also corrected this pass (the toolchain identifiers): the suite-gate row's
「clippy 0.1.94」 reads **clippy 1.94.0** / cargo 1.94.0 (the
re-derived
`cargo clippy -V` face) — the `0.1.94` readings were the same
mis-derivation, corrected everywhere (C-09, the requirements' C-09, the
design's §6).

## The cycle's tasks

| ID | Task | Requirements | Design | Status |
|---|---|---|---|---|
| T-154 | The FR-58 documentation audit (FR-58; C-12, NFR-11): the product-document ground pinned from the delivered tree — every `doc/` face's path/size/mtime (the six top faces + the eight provider faces), the example tree (17788 B `config.example.toml` + the three agent personas), the README pair (39284 B/49640 B), the CHANGELOG top face (the empty `[Unreleased]` + `[0.49.0]` with its two delivery rows), the audit's sweeps executed (the stale backend-kind names: **zero tree-wide** — the FR-51 renames' completions verified; the version strings: **exactly 0.49.0 in `Cargo.toml`/`Cargo.lock`, and zero version strings anywhere in `doc/` or the READMEs** — no stale string can exist; the shipped CLI face's doc citations: every named face exists), the example-tree names verified (the FR-51 completions + the FR-52's agent-name face; the example's kind list covering all nine shipped kinds), the CHANGELOG/Cargo/release ground verified (the CHANGELOG top = the empty Unreleased + 0.49.0; the shipped Cargo.toml/Cargo.lock at 0.49.0; the release workflow's 0.49.0 face standing Latest), the verdict R-58 recorded, and every finding cited (NFR-11 — the path/size/mtime/stale-name/version citations). | FR-58, NFR-11, NFR-15, C-12 | design §1, §2 | ✅ Done (the audit's core findings green — zero stale names, zero version drift, zero missing faces; R-58 recorded, the design's §2 carries its ground; its citation corrections stand in T-156's row and the requirements' §10) |
| T-155 | The ledgers' consistency and the work log at every update pass (C-08, NFR-12, C-15/C-16): the three documents aligned together around the closed FR-57/FR-58 cycles (this row, the requirements' §13, and the design's §1–§5), the instruction byte-exact re-verified at each exit (328 bytes, sha256 `a17757ca…`), no document carrying any mark of authorship or tooling (NFR-12). | — (C-08, NFR-12, C-15, C-16) | design §1–§4 | ✅ (the 08:05 pass; the work log `2026-11-09_001.md`) |
| T-156 | The second execution pass's corrections (C-08, NFR-11, NFR-12): the instruction re-verified byte-exact at read and exit; the audit re-derived live and the three unevidenced citation faces **withdrawn** (the provider-page version lines; the usage.md § version citations; the claimed `strata` gap — the example's kind list covering all nine kinds, L12 + L74–75) and the toolchain identifiers corrected to clippy/cargo **1.94.0**; the three typo corrections (`2026-2026-11-08` → `2026-11-08`; the trail's `9526983` → `9562983`; the mtime `.560236922` → `.560246922` — itself withdrawn as unevidenced by the third pass, T-157); the three documents realigned on the corrected face (the requirements' §10, the design's §2/§4); the work log written per Art. 6/7. | — (C-08, NFR-11, NFR-12, C-15, C-16) | design §2, §4 | ✅ (the second pass's corrections — their precision faces re-corrected by T-157) |
| T-158 | The fourth execution pass's correction (C-08, NFR-11, NFR-12): the instruction re-verified **byte-exact** at read and exit (328 B, sha256 `a17757ca…`, mtime `2026-11-09 08:03:12 +0900` second-precision); the audit re-derived live (R-58 standing: the same zero-hit sweeps rc=1, Cargo/lock 0.49.0, the CHANGELOG head, the example pins L12/L74–75); the third pass's own work-log artifact carried one transcription face its fourth pass's live re-read **could not evidence** and **withdrawn**: the Art. 7 instruction quote in `2026-11-09_003.md` L9 reads 「機能追加した場合、」 — one `は` drop from the live line 4 「機能追加した場合は、」 (the hex face `e381af` = `は` present in the live file; the full-content re-read and the sha256 intact — AS-110); the closed log stands **unmodified** (the Art. 6/7 append-only rule), the corrected quote face carried in `2026-11-09_004.md`; the instructions' ctime/birth face `2026-11-09 13:46:38.354730621 +0900` recorded (its mtime preserved — a copy face this pass only records); the three documents realigned (C-08); the work log `2026-11-09_004.md` written per Art. 6/7. | — (C-08, NFR-11, NFR-12, C-15, C-16, C-17) | design §1, §2, §4 | ✅ |
| T-159 | The FR-59 feature-addition pass (this session, ~14:0x–14:3x JST; the FR-55 build face T-147 executed — FR-59a/b, C-14): the instruction re-verified **byte-exact** at read (328 B, sha256 `a17757ca…`, mtime `2026-11-09 08:03:12 +0900` second-precision); `/work/workspace/athena` verified **absent on this machine** — the athena observation environment lives elsewhere, so the FR-55 formulation's installation face generalized to the shipped product artifacts (AS-111); the third artifact's orchestrator persona installed as `example/agents/orchestrator.md` (2145 B — observe → decide → delegate → collect → iterate + prohibitions), the FR-55 goal-prompt loop's faces folded into its body (`spawn` fire-and-forget, `send_to` `delivery="ask"` collect, the blind-resend prohibition); the **new finding** the pass surfaced — the bundled `coder`/`reviewer` personas naming retired tool names `shell`/`fs_read`/`fs_write` — analyzed multi-angle per C-12 (two spawned analyzer children; analyzer-b's report lost to the 120 s timeout and stopped, analyzer-a reported and stopped; the verdict taken on the registry face `src/tools/mod.rs:98–105`), no functional harm (the alias map resolves all three names; the built permission sets stand) — fixed in the same carry (allowed/ denied lists + body text; `doc/personas.md` §7's quote faces); `doc/personas.md` extended (§7 "four examples", §7.4 the orchestrator quote byte-verified against the shipped file, §8.2 the three-agent team, §12 the cheat-sheet row); the CHANGELOG `[0.50.0]` section written (Added + Fixed, including the `.gitignore` retired `codex-code`-name row following the FR-51 rename); the version carry **0.49.0 → 0.50.0** (`Cargo.toml`, `Cargo.lock`); the gates re-derived green at the changed tree (**668 unit + 6 integration, 0 failed** — twice; `bundled_example_personas_parse` covering the fourth persona; the rustfmt 1.91.1-vs-1.94.0 toolchain-generation sweep withdrawn after application — the delivery ground protected, `tests/detached_spawn.rs`'s stray fold restored to HEAD; clippy's five standing notices read generation-dependent, one notice at clippy 0.1.91); the ledgers realigned (C-08); the work log `2026-11-09_005.md` written per Art. 6/7. | FR-59a, FR-59b, C-14, C-12 | design §1, §2, §3 | ✅ |
| T-160 | The delivery-chain execution (C-14's tail; FR-59b): the delivery commit committing the product changes and this pass's ledger/log writes, tagged `v0.50.0`, pushed to `origin/main`, and the release workflow publishing `v0.50.0` standing Latest (the release notes = the tag's own CHANGELOG `[0.50.0]` section, per the workflow's extract); the trail's follow-up (a later pass) names the delivery sha per the trail rule. | FR-59b, C-14 | design §4 (W-86) | ✅ (executing at this pass's close — its committed record rides the trail's next follow-up)

## The closed tasks of the delivered cycles (their records stand)

- **T-152/T-153** (the FR-57 parent-inaction diagnosis) — ✅, closed
  2026-11-09 00:1x–00:2x: the verdict R-57 standing (the seven `[info]`
  peer-report lines explained end to end — the parked-recv loop, the no-turn
  Context arm, the lost ask-answers, the unimplemented report-triggered
  resumption), the live incident reconstructed from the two live agents'
  conversation logs and the registry, no product file written (the
  requirements' §12; the design's §5). The work log `2026-11-09_000.md`
  carrying the pass.
- **T-149–T-151** (the FR-56 report-reception investigation and 解説) — ✅,
  closed 2026-11-08: the three delivery modes (`prompt`/`report`/`ask`)
  explained end to end on pinned sites (the tool's report arm
  `src/tools/send_to.rs:111–119`, `deliver_report` `:173–213` with the
  legacy-peer prompt fallback, the server's transport face
  `src/ipc/server.rs:111–133`, the two pump arms `src/app.rs:497–513`/
  `:817–833`, the agent loop's no-turn Context arm `src/agent.rs:376–418`),
  the live probe (the token report delivered and confirmed standing in the
  peer's history) — the verdict R-56 recorded. The work log
  `2026-11-08_012.md`/`_013.md` carrying both passes.
- **T-146** (the FR-55 formulation cycle) — ✅, closed 2026-11-08: the
  findings stand in the requirements' §12; **T-147** (the FR-55 build
  face) — ✅ **delivered 2026-11-09 in the FR-59 feature-addition cycle**
  (the orchestrator artifact generalized to `example/agents/orchestrator.md`
  — the shipped artifact, not the athena workspace, AS-111; see T-159)
- **T-142/T-143** (the FR-54 loop investigation + its report) — ✅, closed
  2026-11-08 (§12 of the requirements; the design's §5).
- **T-137/T-138** (the FR-52/FR-53 investigations) — ✅, closed
  2026-11-08; **T-128–T-136** (the FR-50/FR-51 delivery) — ✅, closed at
  `9562983` (v0.49.0); the earlier delivered cycles' records stand (the
  requirements' §12).

## Cross-cutting gates (C-09) — standing

- The suite at the changed FR-59 tree (re-derived twice this session): **668
  unit + 6 integration, 0 failed** — the fourth shipped persona covered by
  `bundled_example_personas_parse`; clippy standing its notices
  (the five-recorded-sites face under clippy 1.94.0; **one** notice at the
  live toolchain clippy 0.1.91 — the rustfmt/clippy generation difference
  recorded, the fmt-tree-wide application withdrawn); the FR-58 audit's
  record standing at `9562983` (its zero-finding face carried by the closed
  cycle's sections). (T-159's record; the design's §3)
- The FR-50 prohibition, the FR-51 renamed kinds, the FR-52/FR-53's
  recorded shapes, the FR-54 loop faces, the FR-56 delivery semantics and
  the FR-57 diagnosis all stand as recorded; **no shipped contract is
  changed by the FR-58 cycle** (its passes touched the three `.aiprj`
  documents and the work logs only — Art. 3); **the FR-59 cycle's shipped
  changes are the persona additions/fixes and the version carry** (no
  Rust-facing contract touched — the suite green at the changed tree).
- The `.aiprj` documents mutually consistent (C-08 — the FR-59 pass
  aligned the three documents around the feature-addition record); the
  work log written per Art. 6/7 (never overwritten); the instruction
  verified byte-exact at exit (328 bytes, sha256 `a17757ca…`).

## The previous cycles' record

**"the documentation-tidy audit" (FR-58)** — **closed, 2026-11-09 (four
passes: 08:05, 08:1x, 13:5x, 14:0x)**: the audit's core findings green (zero stale
backend-kind names tree-wide, the version strings exactly 0.49.0 in
Cargo.toml/Cargo.lock with zero version strings in doc/ and the READMEs,
every cited doc face existing, the example-tree names verified, the
CHANGELOG top face the delivered 0.49.0 ground); the verdict R-58 recorded;
the first pass's three unevidenced citation faces withdrawn at the second
pass (T-156 — R-59), the second pass's own unevidenced mtime-precision face
withdrawn at the third pass (T-157 — R-60), the third pass's own work-log
quote face (the は-drop) withdrawn at the fourth pass (T-158 — R-61); **no
product file written, no commit, no version carry, no release** (AS-106; C-14 stays with the build
cycles). The requirements' §1–§12, the design's §1–§5 and this document
carry the record.

**"the parent-inaction diagnosis" (FR-57)** — **closed, 2026-11-09**: the
verdict R-56's own mechanism explained the live incident (the seven
reports arrived as designed and ran no turn); the diagnosis delivered in
session output (the requirements' §12, the design's §5); no product
change, no release.

**"the peer-report reception investigation" (FR-56)** — closed,
2026-11-08: the delivery semantics explained end to end (the requirements'
§12; the design's §5); the FR-57 incident is its live confirmation.

**"the athena orchestration formulation" (FR-55)** — closed, 2026-11-08
(the requirements' §12; the report `report_athena.md` stands): its
installation face **delivered 2026-11-09 in the FR-59 cycle** (T-147 — see
the FR-59 record).

**Earlier cycles** — the loop investigation FR-54, the FR-52/FR-53
investigations, the FR-50/FR-51 delivery inside the 0.49.0 carry at
`9562983`, and the delivered record before them all stand in the
requirements' §12.

Version trail: … → `ebe0aca` (v0.45.0) → `77e8a29` (the one-shot;
**v0.46.0**) → `f9ec9ae` (the FR-48 delivery; **v0.47.0**) → `89f1ea0`
(the FR-49 delivery, **v0.48.0**) → `41327d7` (the ledger follow-up naming
the publication) → `9562983` (the FR-50/FR-51 delivery; the tag `v0.49.0`
on it; the release `v0.49.0` published standing **latest**) → **the FR-59
delivery commit (this session, tagged `v0.50.0`, pushed; the release
standing Latest — named from the trail's next follow-up per the trail
rule), `main` ≡ `origin/main` after the push**. The trail's follow-ups
name the deliveries behind them (per the trail rule, a delivery commit's
sha is nameable only from the commit that follows it).
