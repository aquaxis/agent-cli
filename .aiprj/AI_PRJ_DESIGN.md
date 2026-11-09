# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: 「ドキュメントを整備する」 — **FR-59**, the FR-55 athena build face
**executed as a feature-addition cycle (2026-11-09, this session)**: the
FR-58 documentation-tidy cycle closed at four passes (the audit's verdict
R-58 plus the correction verdicts R-59/R-60/R-61, its record standing in
§5–§6 below), and the session's next job executed **T-147** — the FR-55
formulation's artifacts installed as shipped product artifacts: the fourth
example persona `orchestrator` (generalized from the athena-specific
proposal — the athena workspace absent on this machine, AS-111), a feature
addition, so **C-14 fires in full** (the doc/example updates, the version
carry **0.49.0 → 0.50.0**, the delivery commit, the tag `v0.50.0`, the push,
the release standing Latest). The cycle's own new finding — the shipped
`coder`/`reviewer` personas naming retired tool names — analyzed multi-angle
per C-12 and fixed in the same carry (AS-112). The instruction re-verified
byte-exact at this session's read (328 bytes, mtime
2026-11-09 08:03:12 +0900), the header and its 最優先事項 standing verbatim.
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`; requirement IDs
refer to the requirements file.

---

## 1. The cycles' asks and the method (C-12)

**The FR-59 ask (this session)**: the session's next job executed the
standing T-147 — the FR-55 build face — as a **feature-addition cycle**
(the FR-58 tidy cycle having closed at four passes). The delivery sentence
「機能追加した場合は、…」 fired: a feature was added (the orchestrator
persona), so C-14's full chain executed (AS-106 fired). The pass's method
(C-12): the generalization decision taken on the live `stat` read
(`/work/workspace/athena` absent on this machine — AS-111; the athena
observation environment living elsewhere, so the FR-55 formulation's
installation face generalized to the shipped product artifacts); the
**new finding** (the shipped personas' retired tool names) analyzed
multi-angle — two analyzer children spawned (the FR-58 precedent), one
child's report lost to the 120 s timeout and the child stopped, the
verdict taken on the registry's file:line face (`src/tools/mod.rs:98–105`,
the alias map shell→bash/fs_read→read/fs_write→write — AS-112, no
functional harm). The fmt-generation face: the live toolchain reads
cargo/rustc/clippy **1.91.1** (rust-toolchain.toml channels=stable) vs the
ledgers' recorded 1.94.0 — the tree-wide fmt application first applied was
**withdrawn** (the delivery ground protected from generation-driven
churn; AS-113), its stray `tests/detached_spawn.rs` fold restored to HEAD;
clippy's notice count read generation-dependent (five at 1.94.0, one at
0.1.91).

**The FR-58 method (its record standing)**: the ask 「最優先事項:
ドキュメントを整備する」 named a documentation-tidy cycle whose surface is
read-only survey + record (a no-op verdict riding the delivered 0.49.0
ground). The execution pass's method (C-12, refined): re-derive every
citation the prior ledger record carries — the prior pass's citations
re-pinned against the live tree — and **withdraw** any claim the tree
cannot evidence (NFR-11). The execution's ground: the tree at `9562983`
(the trail's HEAD, `main` ≡ `origin/main`; `Cargo.toml`/`Cargo.lock` at
0.49.0) with the ledgers' uncommitted pass-001 writes standing; the audit
re-run live (the renames' face, the version faces, the example faces, the
CHANGELOG's top face, the toolchain face).

## 2. The surveyed ground (T-154's re-derived findings, every fact pinned)

### §2.0 The FR-59 pass's own ground (every fact pinned)

- `report_athena.md` 14655 B / 209 lines (the FR-55 formulation) re-read in
  full; `/work/workspace/athena` **absent** on this machine (AS-111).
- The shipped personas read at the delivered tree: `coder.md` 535 B and
  `reviewer.md` 452 B, both carrying `shell`/`fs_read`/`fs_write`; the
  registry's alias face `src/tools/mod.rs:98–105` re-pinned (shell→bash,
  fs_read→read, fs_write→write; `canonical_tool_name()` applied to every
  build input — enabled/allowed/denied — so the built permission sets stood:
  coder={bash,read,write,send_to}, reviewer={bash,read}); the parse test
  `bundled_example_personas_parse` (`src/persona.rs:427–450`) validating
  parse+role only, no tool-name validation — the defect presentational.
- The new file `example/agents/orchestrator.md` written **2145 B / 55 lines**
  (frontmatter: role "Task orchestrator", allowed bash/read/glob/grep/
  send_to/spawn/list_agents/stop_agent; body: the permissions block, the
  observe→decide→delegate→collect→iterate loop, the prohibitions block).
  `doc/personas.md` §7.4's quote verified **byte-identical** with the shipped
  file (2145 == 2145).
- The toolchain face live: cargo/rustc 1.91.1, clippy 0.1.91, rustfmt
  1.8.0-stable (vs the ledgers' recorded 1.94.0 face — AS-113); the fmt
  tree-wide application applied then **withdrawn**, `tests/detached_spawn.rs`
  folded rows restored to HEAD (the stray fold this session's own fmt drop).
- The gates at the changed tree: **668 unit + 6 integration, 0 failed** (twice
  — pre- and post-fix); the fourth persona covered by
  `bundled_example_personas_parse`.

§2's rows below stand as the FR-58 audit's record (the 0.49.0 delivered
ground):

- **The backend-kind renames' face** — **zero stale backend-kind name**
  anywhere in the product tree (the whole-tree sweep over `src/ doc/
  example/ README*` returning clean; the `claude-cli`/`codex-cli` faces
  standing in the source, config, doc, and example trees). `doc/providers/
  claude-cli.md` 7040 B and `doc/providers/codex-cli.md` 8638 B (mtimes
  12:13 — the 0.49.0 delivery's doc-update rows); `doc/providers/claude.md`,
  `codex.md`, `llamacpp.md`, `ollama.md`, `opencode.md`, `strata.md`
  mtimes Aug 16/Oct 22/Nov 5 19:15/19:25.
- **The version faces** — `Cargo.toml`/`Cargo.lock` carry **exactly 0.49.0**;
  **zero version strings stand in `doc/` or the READMEs** (the live greps:
  `grep -R '0\.49\.0' doc/ README*` → empty; `grep -nE 'v?0\.[0-9]+\.[0-9]+
  '`-shaped sweeps over `doc/` READMEs returning only historical v-faces —
  the `update --ref v0.11.0` example rows, the history citations v0.20.0 /
  v0.22.0 / v0.25.0 / v0.26.0 / v0.11.0 / the `Input pump (v0.25.0)` row —
  all carrying `**` bold in doc/*.md and **zero v0.4x.0 or 0.49.0/0.48.0
  strings anywhere in doc/ or the READMEs**). The CHANGELOG's head: the
  empty `## [Unreleased]` + `## [0.49.0]` (the Keep-a-Changelog top face,
  its two delivery rows standing); its body green.
- **The example face** — `example/config.example.toml` 17788 B: its kind
  list L12 covering **all nine shipped kinds** (`claude` / `claude-cli` /
  `codex` / `codex-cli` / `strata` / `ollama` / `opencode` / `opencode-go` /
  `llama.cpp` — the kind list L12's own face) with the commented
  `[provider.strata]` block at L74–75 — **the example tree's face is
  complete** (the claimed `strata` gap was withdrawn at the prior pass,
  T-156); `example/agents/` three personas (535/366/452 B, mtimes
  May 11 21:23 — untouched since May 11, their content verified stale-free;
  the FR-52's agent-name face verified). The shipped CLI face (「Prohibit
  mouse operations…」) names doc/usage.md, doc/tools.md, doc/config.md,
  doc/troubleshooting.md, doc/personas.md, doc/providers/*,
  doc/architecture.md — every cited face exists (the shipped CLI face's
  doc citations verified).
- **The toolchain face** — `rust-toolchain.toml` pins `stable` + rustfmt +
  clippy; `cargo clippy -V` reads **1.94.0** — the T-154-era ledger record
  (「clippy 0.1.94」) was a **mis-derived identifier** — corrected at the
  prior pass
  (clippy/cargo 1.94.0; the re-derivation face itself, cargo 1.94.0
  face standing). No stale identifier remains.
- **The tree's doc references** — `src/` names `doc/` at
  `src/ai/llamacpp.rs:22` and `src/config.rs:1082`/`:2091`; the src tree
  names no doc name the doc/ tree lacks (the src→doc refs' existence
  verified; the doc/ internal file refs verified against the tree).

## 3. The verdicts

**R-62 (the FR-59 feature-delivery verdict):** the FR-55 build face (T-147)
delivered as the shipped fourth persona `orchestrator` plus the
coder/reviewer tool-name canonicalization — a feature addition, so C-14
fired in full and its chain executed (the version carry 0.49.0 → 0.50.0,
the CHANGELOG's `[0.50.0]`, the delivery commit tagged `v0.50.0`, the push,
the release standing Latest — its committed naming rides the trail's next
follow-up). The multi-angle verdict on the tool-name finding (AS-112): no
functional harm (the alias map resolves every retired name — verified at the
registry face), the defect presentational (the files/docs naming tools the
registry registers under other names); the fix canonicalizes the names, the
built permission sets unchanged (the suite's registry tests green, no test
change needed). The fmt-generation gate-face (AS-113): the tree-wide fmt
application **withdrawn** — not a product verdict but this session's own
toolchain-generation artifact — the delivery ground kept at its
pre-churn face; the stray fold in `tests/detached_spawn.rs` restored to
HEAD. R-58 untouched by this cycle: the FR-58 audit's zero-finding face
stands (its record below).

**R-58 stands (the tidy verdict): the product documentation stands tidy at
the delivered 0.49.0 ground** — zero stale backend-kind names tree-wide,
the example tree complete (all nine kinds, the `strata` block present at
L74–75), the version strings exactly 0.49.0 in the manifests, zero version
strings in doc/ or the READMEs, the CHANGELOG's top face the delivered
ground, the example tree's face complete — **and the T-154 record's
citation faces corrected by T-156** (the provider-page version-line
citations, the usage.md § citations, the claimed `strata` gap — each
withdrawn; the toolchain identifiers clippy/cargo **1.94.0**). **R-59 (the
citation-correction verdict):** the prior pass's T-154 row carried three
unevidenced citation faces over the zero-finding face — the corrections are
recorded in T-156's row, the requirements' §7/§10, and this design's §2 —
**the audit's zero-finding verdict stands on the corrected face**.
**R-60 (the third pass's citation-correction verdict):** the ledger
record carried two more faces the third pass's live re-derivation could
not evidence — the instruction-mtime's nanosecond-precision tail
(`08:03:12.560246922 +0900`; both live `stat` re-reads return
`2026-11-09 08:03:12.000000000 +0900`, second-precision) and the doc-page
inventory row (the live `doc/` tree carries six top-level pages +
eight provider pages = 14; the "seven/17-page" readings a counting
error) — both withdrawn, the ledgers realigned; the example's line-pins
(L12, L74–75) re-verified standing exactly as cited — the audit's clean
verdict unchanged.

**R-61 (the fourth pass's citation-correction verdict):** the third pass's
work log carried one transcription face its fourth pass's live re-read
could not evidence against the instruction — the Art. 7 quote's delivery
sentence 「機能追加した場合、」 drops the は the instruction carries
(「機能追加した場合**は**、」, hex `e381af` = は; the sha256 `a17757ca…`
intact, the git face clean — the instruction file itself unmodified) —
withdrawn per NFR-11 and the Art. 6/7 append-only rule: the closed log
`2026-11-09_003.md` stands unmodified, the correction stands in the fourth
pass's log and the ledgers; every ledger and prior-log quote face carries
は (the sweep; the one exception the `_003` quote itself). Context recorded:
the instruction's inode-side faces read `ctime`/`birth`
`2026-11-09 13:46:38.354730621 +0900` with the mtime preserved — recorded,
not attributed; the drafted nanosecond tail's source remains unidentified.
The audit's sweeps re-run live green — R-58 unchanged.

## 3.1 The angles (C-12)

- **The FR-59 angles**: the build angle (the generalization decision on the
  live athena-absence face — AS-111; the artifact read and folded into the
  shipped persona), the finding angle (the tool-name analysis — two analyzer
  children, one lost to the timeout, the registry face decisive — AS-112),
  the delivery angle (C-14's chain — version, CHANGELOG, commit, tag, push,
  release), and the gate angle (the fmt-generation face — AS-113).
- **The audit angle** (the FR-58 record): the full re-derivation in §2 — the renames'
  face, the version faces, the example faces, the CHANGELOG's top face, the
  toolchain face.
- **The consistency angle**: the ledgers' citation faces re-derived
  across the cycle's passes — the T-154 row's three unevidenced citations
  withdrawn and the toolchain identifiers corrected, the three typo
  corrections (`2026-2026-11-08`, the trail's `9526983`, the
  instruction-mtime precision face) at the second pass, then the third
  pass's withdrawal of the mtime's nanosecond precision and the
  doc-inventory correction (R-60, T-157), then the fourth pass's withdrawal
of the third pass's own work-log quote face (the は-drop, R-61, T-158) —
and the three documents realigned at every pass (C-08).

## 4. Verification (the gates: this cycle's record)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-82 | FR-58 — the documentation surface surveyed read-only (C-16), every face pinned (NFR-11): the 14 doc pages + 4 example files + the README pair + the CHANGELOG's headings + the version strings; the negative findings verified (the renames' face, the version faces, the example faces, the CHANGELOG's top face); the verdict **R-58 — nothing to tidy this pass** standing. | FR-58, NFR-11, NFR-15, AC-49 | T-154 ✅ | ✅ closed — the audit's zero-finding verdict stands on the corrected face |
| W-83 | The correction pass (T-156) — the instruction re-verified byte-exact at read and exit; the audit re-derived live; the three unevidenced citation faces withdrawn (the provider-page version lines, the usage.md § citations, the claimed `strata` gap); the toolchain identifiers corrected to clippy/cargo 1.94.0; the three typo corrections; the three documents realigned (C-08); the work log `2026-11-09_002.md` written (Art. 6/7). | C-08, NFR-11, NFR-12, C-15, C-16 | T-156 ✅ | ✅ closed (the second pass) |
| W-84 | The third pass's withdrawal (T-157) — the instruction re-verified byte-exact at read and exit (328 B, sha256 `a17757ca…`); the audit re-derived live (R-58 standing: zero stale names rc=1, zero version strings in `doc/`/READMEs rc=1, Cargo/lock 0.49.0, the example kind list L12 + `[provider.strata]` L74, the doc inventory 14 pages); the instruction-mtime precision face and the doc-page count withdrawn (R-60); the three documents realigned (C-08); the work log `2026-11-09_003.md` written (Art. 6/7). | C-08, NFR-11, NFR-12, C-15, C-16, C-17 | T-157 ✅ | ✅ closed this pass |
| W-85 | The fourth pass's withdrawal (T-158) — the instruction re-verified byte-exact at read and exit (328 B, sha256 `a17757ca…`, mtime `2026-11-09 08:03:12 +0900` second-precision); the audit re-derived live (R-58 standing); the third pass's work-log quote face (the は-drop in the Art. 7 citation, AS-110) withdrawn — the closed log `2026-11-09_003.md` stands unmodified per the Art. 6/7 append-only rule, the correction carried in `2026-11-09_004.md` and the ledgers; the three documents realigned (C-08). | C-08, NFR-11, NFR-12, C-15, C-16, C-17 | T-158 ✅ | ✅ closed this pass |
| W-86 | The FR-59 delivery chain (T-159/T-160) — the fourth persona shipped and parse-green (`bundled_example_personas_parse`), the coder/reviewer names canonical, `doc/personas.md` §7.4's quote byte-verified against the shipped file, the CHANGELOG `[0.50.0]` standing under the empty `[Unreleased]`, the version manifests 0.50.0, the suite green at the changed tree twice (668 unit + 6 integration, 0 failed), the delivery commit tagged `v0.50.0`, pushed, the release workflow publishing it standing Latest. | FR-59a, FR-59b, C-14, C-09 | T-159 ✅, T-160 ✅ | ✅ closed (W-86's committed record rides the trail's next follow-up) |

*Standing gate records: W-81 (FR-57's record), W-79/W-80 (FR-56's), W-77
(FR-55's), W-75/W-76 (FR-52/FR-53's), W-73/W-74 (FR-50/FR-51's deliveries)
— the requirements' §12 carries each cycle's record; the earlier cycles'
records stand in the trail.*

## 5. The delivered cycles' records (standing)

**FR-57** (the parent-inaction diagnosis) — closed, 2026-11-09 00:1x: the
incident reconstructed from the two live athena agents' conversation logs
and the registry (reads only, C-17's precedent); the cause stated end-to-end
on pinned file:line evidence: the parent parked on `input_rx.recv()` with
only `AgentInput::Context` report inputs after its last turn (23:15:59 JST),
the Context arm running no turn (`src/agent.rs:376–418`), the only turn
triggers UserPrompt/PeerPrompt (`agent.rs:237`/`:308`), the four
`delivery=ask` probes' answers lost to the 120 s timeout, even a delivered
answer print-only never a turn trigger — the verdict R-57: *an orchestrator
must never wait for a report as if it were a turn trigger*. The work logs
`_012` (the FR-56 pass) and `2026-11-09_000.md`.

## 6. The delivered cycles before FR-57 (their records stand)

**FR-56** (the report-reception investigation) — closed, 2026-11-08: the
delivery semantics explained end to end on pinned sites; the verdict R-56;
the FR-57 incident is its live confirmation. **FR-55** (the athena
formulation) — closed, 2026-11-08: the report `report_athena.md` stands;
its build face **delivered 2026-11-09 as the FR-59 cycle** (T-147 ✅ — see
§1, §3's R-62, §4's W-86). **FR-54** (the loop
investigation) — closed, 2026-11-08: `report_loop.md`; four loop shapes
pinned. **FR-52/FR-53** (the name/model change and the startup-status
investigations) — closed, 2026-11-08: the verdicts R-52/R-53.
**FR-50/FR-51** (the running-turn mouse prohibition; the backend-kind
renames) — delivered and closed 2026-11-08 in the one 0.49.0 carry
(`9562983`, the tag `v0.49.0`, the release standing Latest; gates 668 unit +
6 integration, 0 failed). The earlier cycles' records stand in the
requirements' §12.
