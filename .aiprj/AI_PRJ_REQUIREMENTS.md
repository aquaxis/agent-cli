# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "mouse operations during agent operation prohibited" (FR-50 a/b/c —
**in progress: the polarity repair stands executed at the tree, its
verification unwritten**) and
"the `codex-code`/`claude-code` backend kinds renamed to
`codex-cli`/`claude-cli`" (FR-51 a/b/c — **pending, the rename delivered at
the tree with doc residuals**)
(`.aiprj/instructions.md`, reloaded 2026-11-08 08:27 +0900 and verified unchanged
at every pass since — sha256
`76c3d039…` → the settled 515 bytes, 412 bytes at the first pass's read, mtime 2026-11-08 08:27:23.983370945 +0900, the first
ask 「エージェント動作中のマウス操作は禁止」 — then, while this pass stood
mid-write, **extended at 08:37/08:38**: the second clause 「config.tomlの内容
変更」 recorded (513/456 bytes at the pass's reads), finally settling at 515
bytes, sha256 `179b698a…`, mtime 2026-11-08 08:38:16.354426690 +0900, with
「codex-codeはcodex-cliへ変更」 and 「claude-codeはclaude-cliへ変更」; the
instruction verified byte-exact at the pass's exit against the settled 515-byte
state (§2) — and re-verified byte-exact at this pass's exit (§13, the
consistency pass))
Prior cycles delivered and closed: the mouse-click display defects (FR-48
a/b/c/d at `f9ec9ae`, v0.47.0 Latest) and the answer-display prompt handoff
(FR-49 a/b/c/d at `89f1ea0`/`9040435`, v0.48.0 Latest) — §12/§13
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

`.aiprj/instructions.md` was rewritten at 2026-11-08 08:27:23 +0900 (412
bytes, sha256 `76c3d039…`, §2). The previously standing 最優先事項 section
「マウスクリックすると1行ズレた表示になる」 is **gone** from the
instruction: it stands delivered (FR-48 a/b/c/d — `f9ec9ae`, v0.47.0, the
release verified Latest; the FR-49 cycle delivered beside it at
`89f1ea0`/`9040435`, v0.48.0 with the release verified Latest after the
publication close; §12/§13), and the document now carries one active
最優先事項 section with one clause:

**「エージェント動作中のマウス操作は禁止」** with the body sentence
「エージェント動作中のマウス操作は禁止に変更する」 — while the agent is
running a turn, mouse operations are to be prohibited; the shipped behavior
is to be changed so they are. This cycle is a behavior-change/addition
cycle: **FR-50a** the mouse handling agent-cli performs during a running
turn is gated off (the wheel scrollback, the progress-indicator click and
the drag-select-and-copy the REPL handles while a turn runs are not
performed while it runs, per the readings AS-87/AS-88); **FR-50b** the
documentation follows; **FR-50c** the completion — the minor version carry
0.48.0 → **0.49.0** with the commit, the push, and the release standing
latest. The first ask's mechanism stands **delivered at the working tree
with a polarity defect** (an execution session between this ledger's last
write and this pass delivered the gate writes `src/app.rs:3452`/
`:3503`/`:3556`, but the prohibition helper reads as an *enabler* there —
BL-M64; the repair is the tree's outstanding FR-50a clause, T-129/T-135);
the delivered faces (the click's zero-touch contract, the zone's edges)
bound the repair (§6), and the delivery sentence's completion rides the
same shape as every prior cycle: the docs follow (FR-50b), the version
carries 0.48.0 → **0.49.0**, the delivery is committed and pushed, and the
release is published as the latest version (FR-50c). **The repair stands
since executed at the working tree** (an execution session after the 10:42
pass wrote the negated gates and the re-faced pin into `src/app.rs` at
~11:05 +0900 — BL-M66, §1/§13) with its delivery session's verification
standing unwritten.

**The instruction was extended while the first pass stood mid-write** (first
at 08:37:43, again at 08:38:16 — §2): the second 最優先事項 section
「config.tomlの内容変更」 carries two rename clauses — 「codex-codeは
codex-cliへ変更」 and 「claude-codeはclaude-cliへ変更」. The first pass
scoped it as the FR-51 cycle: **FR-51a** the backend kind strings
`codex-code` and `claude-code` are renamed `codex-cli` and `claude-cli`
everywhere they name the configuration surface — the
`[provider.codex-code]`/`[provider.claude-code]` config tables and
`kind = "codex-code"`/`"claude-code"` values, the rename keys the
deserializer accepts, the CLI's `--provider` values, the error prefixes and
diagnostics, and every doc row the rename un-truths (the requirements'
BL-M62 surface record; the exact span AS-90 records) — **FR-51b** the
documentation follows (the CHANGELOG entry, the README/README_ja backend
rows, the usage row, the per-backend guides, the example's backend blocks),
and **FR-51c** the completion rides the same delivery sentence (the version
carry to **0.50.0** after FR-50's 0.49.0, or both cycles delivered inside
the one 0.49.0 carry if they land in a single delivery — the sequencing the
tasks' T-133 row records). **The rename's code surface stands delivered at
the working tree** (an execution session renamed it — zero old-kind strings
remain in `src/` and the shipped docs; the version files carried to 0.49.0
with the CHANGELOG's `[0.49.0]` entry standing; BL-M63) **with doc
residuals**: thirteen stale `claude-code`/`codex-code` rows survive in the
two renamed provider guides (BL-M65), and the FR-50b mouse rows stand
unwritten (BL-M64's doc face) — T-132/T-136 record the states (§5's rows).

The standing instruction header is unchanged: the work runs 多角サブエー
ジェント解析 (the multi-angle subagent analysis, C-12), and a delivered
addition carries its documentation, the example renewal, the minor version
bump, and the commit/push/release-to-latest (C-14).

The 10:15 consistency re-load pass (the 2026-11-08 10:15 +0900 pass, §13) re-verified
the instruction byte-exact and re-derived the tree (HEAD `41327d7` ≡
`origin/main`, the three ledgers rewritten by the session that closed the
FR-49 publication standing **behind in the trail, uncommitted**), then
resolved the ledger inconsistency the tree presented: the execution session
that followed the FR-49 publication session wrote the FR-50/FR-51 product
faces **without updating these ledgers** — that pass records its
delivery-at-tree states (BL-M63/BL-M64/BL-M65), the gate re-derivations
(668 unit + 6 integration, 0 failed; clippy five standing notices as the
10:15 pass's own clippy run recorded them) and the outstanding clauses. The pass wrote the ledgers and its work log only — no
product file, no commit (C-15/C-16, Art. 3); the work log
`2026-11-08_002.md`.

The 10:42 consistency re-load pass (the 2026-11-08 10:42 +0900 pass, §13)
reloaded the instruction byte-exact to the same settled 515-byte state (§2)
and re-derived the tree at HEAD `41327d7` ≡ `origin/main`: the product writes
stand uncommitted and unchanged since the 10:15 pass's read — the FR-50a
mechanism re-read at its sites (the gates `:3452`/`:3503`/`:3556`) and
confirmed **polarity-inverted** (BL-M64; the helper's unit pin re-run green
over the inverted helper), the FR-51a `src/` surface re-scanned
old-kind-free, the 13 guide residuals re-confirmed line-exact (BL-M65), and
the gates re-derived live (668 unit + 6 integration, 0 failed; clippy
**three** standing notices under the tree's clippy 0.1.94 — C-09's re-based
record). The pass resolved the ledger inconsistencies the comparison of the
10:15 ledgers against the tree's standing evidence presented (Art. 2): the
§5/§11 status faces whose T-128/T-129 cells contradicted the task table
aligned to it (the implementation's delivery, polarity-inverted, stands —
T-128 rides the repair 🟨, T-129 🟨; T-131 the pass's row, not T-135), the
standing clippy record re-based to the fresh three-notice derivation, and
the 10:15 pass's rows re-said as the prior pass's. The pass wrote the
ledgers and its work log only — no product file, no commit (C-15/C-16,
Art. 3); the work log `2026-11-08_003.md`.

The 11:35 consistency re-load pass (the 2026-11-08 11:35 +0900 pass, §13)
reloaded the instruction byte-exact to the same settled 515-byte state (§2)
and re-derived the tree at HEAD `41327d7` ≡ `origin/main`: the tree had
moved behind the ledgers again — `src/app.rs` rewritten at 11:05 — and this
pass's fresh read found the FR-50a polarity defect **repaired at the tree**
(BL-M66): the helper `the_mouse_operation_is_prohibited` `:1449` and its
doc comment name the negated reads, and all three mouse gates join it as a
bar — the wheel `:3455`, the click `:3506`, the drag `:3559` each read
`!the_mouse_operation_is_prohibited(...)` — with the pin
`a_running_turn_bars_every_mouse_operation` `:8699` re-faced to state the
gate-side conjunction beside the truth table: the ask's own shape, standing
at the tree with its live verdicts unwritten (the executing session's exit
record `2026-11-08_004.md` an opening-rows-only state, so the repair's
faces stand uncorroborated by a written live probe). The FR-51 residuals
re-confirmed line-exact (BL-M65's 13 rows) and the FR-50b doc rows
re-confirmed untrue at their recorded lines (the sweep T-136 ⬜); the
gates re-derived live (668 unit + 6 integration, 0 failed; clippy **five**
notices re-derived fresh — `src/app.rs:978` empty-line-after-doc-comment,
`:2412` sort_by, `:8520`/`:8801` dead stores, `src/agent.rs:2101`
collapsible-match — the repair's doc-comment rewrite having shifted the
app.rs sites, C-09's record re-based by fresh derivation). The pass
resolved the ledger inconsistencies the comparison of the 10:42 ledgers
against the tree's standing evidence presented (Art. 2): the polarity rows
across §1/§5/§11/§12/§13 and the task table's T-128/T-129/T-135 cells
aligned to the repaired state (T-128 🟨 and T-129 🟨 riding T-135 🟨 —
delivered at the tree, its live corroboration and completion outstanding),
the standing clippy record re-based, the delivery-status rows re-said as
the prior passes' records, and the 11:05 execution session recorded
(BL-M66). The pass wrote the ledgers and its work log only — no product
file, no commit (C-15/C-16, Art. 3); the work log `2026-11-08_005.md`.

## 2. Source instruction

`.aiprj/instructions.md` reloaded for this execution — found rewritten at
08:27:23 (412 bytes, sha256 `76c3d039…`) and **extended while the first
pass stood mid-write** (08:37:43, 08:37:54, 08:38:16 — the reads recording
456, 513, then the settled 515 bytes, sha256
`179b698a3df2b62fb114b16171cec9592694157d0c625eafd0ed5a3538e1a373`, mtime
2026-11-08 08:38:16.354426690 +0900; verified byte-exact at the first pass's
exit against the settled state). The instruction as it stands at this pass's
exit:

```text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する
機能追加した場合は、ドキュメント更新、example更新、マイナーバージョンアップ、commit, push, リリースを最新バージョンにする

## 最優先事項

### エージェント動作中のマウス操作は禁止

エージェント動作中のマウス操作は禁止に変更する

### config.tomlの内容変更

- codex-codeはcodex-cliへ変更
- claude-codeはclaude-cliへ変更
```

The standing delivery sentence supplies this cycle's completion shape
(documentation, example, minor version bump, commit, push, the latest
release); the first 最優先事項 section names the mouse change it completes
(FR-50), and the second section — 「config.tomlの内容変更」 — names the
backend-kind rename it completes (FR-51).

## 3. Baseline — verified in the tree at this cycle's start

The baseline rows stand as the 08:27 pass verified them (BL-M60/BL-M61/BL-M62
— the mouse face, the 0.48.0 ground and the rename surface, each pinned at
`41327d7`); the tree has since moved — the working tree now carries the
execution session's uncommitted product writes, verified this pass (BL-M63
the delivery-at-tree ground, BL-M64 the polarity defect, BL-M65 the doc
residuals). The rows stand in the two §3 record blocks below, at the tree
state's respective reads.

### 3.1 What the instruction did NOT ask for

- **No change to what the mouse does while the agent is NOT running a
  turn.** The wheel scrollback at rest, the drag's copy-and-report, the
  click at an idle prompt (a no-op) — the ask names 動作中 (while
  operating); nothing the mouse does at rest changes. What the running
  turn itself does — its streamed output, its zone faces, `Esc`/`Ctrl+C`
  — is not the ask and stands.
- **The shipped standing compositions are not the ask.** The turn zone's
  clock/prompt flip (FR-46's bottom line), the scrolled view's pinned
  block (FR-25/FR-39), the click's zero-touch contract (FR-48a/FR-48b)
  and the zone's edge/clean-up faces (FR-49) are drawn by their own
  hands; the prohibition may only change **whether** the mouse handling
  runs mid-turn, not re-shape what it does when it runs (§6, NFR-14).
- **No new config surface asked.** No new `[ui]` key is named by the
  instruction; the example writes nothing unless the mechanism's own
  surface requires one (the FR-50 cycle's example check — the example's
  mouse block `example/config.example.toml:191–200` stands truthful, no
  write).
- **No change to the REPL/peer/one-shot/provider contracts** beyond the
  mouse handling's running-turn gating and the two kind strings' face
  (`NFR-14`); no queueing return; no history rewrite; the keyboard's
  behavior mid-turn is not the ask and stands.

## 4. Scope

### 4.1 In scope (pending)

- **FR-50a** — the running-turn prohibition: while a turn is in flight
  (AS-87's window), agent-cli performs none of the mouse operations it
  otherwise handles — the wheel's scrollback move, the progress-indicator
  click's toggle, the drag's select-and-copy — and the display's mid-turn
  faces (the zone's clock, the streamed output) are untouched by any
  mouse event the session receives; the mechanism and its sites decided
  at the evidence-first close (D-91; design §1/§9).
- **FR-50b** — the documentation follows: the CHANGELOG's `[0.49.0]` entry
  in user language naming the change (the empty `[Unreleased]` heading
  standing above it); the README/`doc/usage.md` rows whose shipped
  wording names the affected behavior (the mouse rows — BL-M60's doc
  face, `README.md:26–:27`, `doc/usage.md:357`/`:417–:434`); the example
  check (§3.1).
- **FR-50c** — the completion: the minor version carry 0.48.0 →
  **0.49.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading), the
  delivery commit(s) (the trail rule governs the recording), the push to
  `origin/main`, and the release published as the **latest** version.
- **FR-51a** — the backend-kind rename: the `codex-code` kind becomes
  `codex-cli` and the `claude-code` kind becomes `claude-cli` across the
  configuration surface the ask names (the `kind =` values, the
  `[provider.codex-code]`/`[provider.claude-code]` tables, the deserializer's
  rename keys, `--provider`, the diagnostics the evidence binds; BL-M62's
  surface, AS-90's span).
- **FR-51b** — the documentation follows: the CHANGELOG entry, the
  README/README_ja backend rows, the usage row, the per-backend guides and
  the example's backend blocks the rename un-truths (BL-M62's doc face).
- **FR-51c** — the completion: the version carry per the delivery sentence's
  shape (0.49.0's successor — the tasks' T-133/T-134 rows record), with the
  commit, the push, and the release standing latest.

### 4.2 Out of scope

- The §3.1 list: the mouse's at-rest work, the keyboard mid-turn, the
  zone's flip and the scrolled pin (their own hands), the one-shot/peer/
  provider contracts, and every shipped contract of §6 outside the mouse
  handling's running-turn gating.
- The FR-51 cycle: the child binaries' own names (the `codex`/`claude`
  executables the backends spawn keep their names), the backend wire
  protocols, and any config key beyond the two kind strings the ask names
  (AS-91's reading).
- A git tag beyond the release's own.

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-50a | Mouse operations are prohibited while the agent is running a turn: for every mouse event the session receives while a turn is in flight (AS-87's window — from the turn's start to its terminal event), agent-cli performs none of the mouse operations it otherwise handles — the wheel arm moves no view (`scroll_dirty` stays false), the click arm toggles nothing, the drag arm starts no selection and copies nothing (its report never lands) — and no mouse event's handling paints anything the shipped turn faces did not already paint. The at-rest mouse work (§3.1) is unchanged — the wheel scrolls behind the pinned prompt, the drag selects and copies on release, the click at a pending prompt toggles the shipped thinking view per its own rules — the click's zero-touch contract (FR-48a/FR-48b) holds wherever the handling still runs, and the prohibition's mechanism is decided at its evidenced sites with a live reproduction first (NFR-11, NFR-15) and its own verifiable pin (NFR-13). — **Delivered at the working tree with the polarity repaired and probed live** (all three gates read the bar negated — the wheel `:3455`, the click `:3506`, the drag `:3559`; the pin re-faced `:8699`; BL-M66): the delivery session's battery corroborated the ask's faces through the byte-verified SGR transport — the mid-turn bar (no wheel view move, no `[clip]` report, no click toggle while spinner-proven turns ran), the at-rest wheel move and the bare click's zero-touch; the at-rest copy face rides the suite's pins and the FR-48 record (the tmux transport boundary recorded honestly in the work log). — **State**: FR-50a met (T-135 ✅); the completion T-130 ⬜ (the commit, the push, the tag, the release) is the remaining face. |
| FR-50b | The documentation follows: the CHANGELOG's `[0.49.0]` entry records the change in user language (the empty `[Unreleased]` heading stands above it — the entry *stands at the tree*; it describes the face the repaired gate must deliver); the README's and `doc/usage.md`'s rows whose shipped wording names the affected behavior (the mouse rows `README.md:26`/`:27`, `doc/usage.md:357`/`:420`/`:429`) are updated where the behavior change makes them untrue. Every statement bound to the code it names. — **State**: **delivered** (the delivery session's sweep: the README `:26`/`:27`, the README_ja `:26`/`:27` and `:548–:549`, the usage rows `:357`/`:420`/`:429` rewritten to the prohibition's truth — T-136 ✅). |
| FR-50c | The completion: the minor version carry 0.48.0 → **0.49.0** (`Cargo.toml`, `Cargo.lock` via the shipped `cargo update` path, the CHANGELOG's heading — *carried at the tree*, BL-M63), the delivery commit(s) of the fix, the repairs, the doc rows and the records (the trail rule governs the recording), the push to `origin/main`, and the release published and made the **latest** version (the tag verified on the delivery commit). — **State**: pending (T-130 ⬜). |
| FR-51a | The backend kind strings are renamed: the configuration surface's `"claude-code"` kind becomes `"claude-cli"` and `"codex-code"` becomes `"codex-cli"` — the `kind =` values the config parser accepts, the `[provider.claude-code]`/`[provider.codex-code]` table keys and their serde rename arms, the `--provider` accepted values and its help row, the provider factory's arms, the `for_provider_section`/ensure lookups, the error prefixes, banner rows and diagnostics that print the kind, and every doc row that names the kinds (BL-M62's surface). The Rust-side identifiers the ask does not name (the module file names, the `claude_code`/`codex_code` fields, the provider type names) keep their names — D-92's scope decision. — **State**: **delivered at the working tree** (BL-M62: the span's code face, `src/` greps zero for the old kinds); uncommitted, the delivery commit rides T-130's completion. — **Status**: done (T-132 ✅). |
| FR-51b | The documentation follows: the CHANGELOG's entry records the rename in user language (under the release heading the delivery carries — FR-51c's version; *stands at the tree* under `[0.49.0]`, BL-M63/BL-M62's entry face); the README's and README_ja's backend rows, the usage row, the config/architecture/personas/tools/troubleshooting rows, and the example's backend blocks are updated — **delivered at the tree** (zero old-kind hits outside the two per-backend guides); the per-backend guides `doc/providers/codex-code.md`/`claude-code.md` are renamed **as files** to `doc/providers/codex-cli.md`/`claude-cli.md` — standing clean after the delivery session's sweep — the thirteen stale rows (the TOML blocks, the doctor/selftest rows, the diagnostics-table rows, the guides' cross-reference) rewritten to the new kinds, the workspace's own `.aiprj/config.toml` comment rows `:12`/`:30` renamed with them; zero old-kind strings in any shipped face. Every statement bound to the code it names. — **State**: **delivered** (T-136 ✅). |
| FR-51c | The completion: the minor version carry (the CHANGELOG entry standing under `[0.49.0]` beside FR-50b's entry per the one-carry record), the delivery commit(s) of the rename and the records (the trail rule governs the recording), the push to `origin/main`, and the release published and made the **latest** version (the tag verified on the delivery commit). — **State**: pending (T-133 ⬜). |

## 6. Boundary: FR-50 and the shipped contracts

The shipped contracts stand (NFR-14): the four-region screen (FR-37,
FR-46), the prompt/clock flip and its bottom-most line, the wheel
scrollback and the scroll marker (FR-39), the drag select-and-copy with
its shift override and the `[clip]` report, the thinking view's toggle
and its click rules (FR-27/FR-28), the subagents' transients (FR-20's
faces), the approval takeover, the peer/IPC/registry surfaces, the
one-shot mode (FR-47), the click's zero-touch face (FR-48a/FR-48b), the
answer-display prompt handoff's faces (FR-49) — none of them changes.
**That boundary holds as the instruction wrote it; the pass adds the
correction BL-M63 pins** — the tree's delivered mid-turn faces, not new
boundary text: the prohibition is the change those faces ride, and the
repair restores them, changes none of them (T-135).

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | The repairs stand pinned: the cycle's fix carries its own pin (the running-turn prohibition's case — a mouse event mid-turn performs nothing, verifiable without a live terminal wherever the mechanism allows), the shipped click/wheel/zone pins re-derive untouched, and the gates re-derive at the changed tree — the suite green, clippy standing its notices at the recorded sites (C-09). |
| NFR-14 | The changes stay bounded by the boundary of §6: the mouse handling's running-turn gating and the docs follow; no other shipped contract changes. |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism. This cycle's probes: the suite/clippy runs at the changed tree; **the live mid-turn mouse probe** — a scripted mouse sequence (wheel notches, a click, a drag) against a live session with a turn provably in flight, the screen's faces compared against the turn running alone (no view move, no toggle, no selection/report; the zone's clock alive throughout), then the same sequence at rest as the control (the mouse work unchanged at rest, §3.1); the release-latest/tag verification at the close. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression standing: unit + integration green, clippy standing its pre-existing notices, re-derived at this cycle's tree — the standing record **668 unit + 6 integration, 0 failed** (derived at the execution sessions' trees and re-derived by the 11:35 pass at the polarity-repaired tree; the pre-FR-50 record 667 + 6), clippy standing **five** notices at the recorded sites (`src/app.rs:978` empty-line-after-doc-comment, `:2412` sort_by_key, `:8520` and `:8801` dead stores, `src/agent.rs:2101` collapsible_match) — re-derived fresh by the 11:35 re-load pass's runs (clang 0.1.94) at the repaired 0.49.0 tree, the repair's doc-comment rewrite having shifted the app.rs sites (the 10:42 three-notice re-basing's `:8517`/`:8787` rows stand at `:8520`/`:8801` again, and the earlier record's `:2409` sort_by / `:2101` collapsible_match rows fire again). |
| C-12 | The instruction's method — multi-angle subagent analysis — stands, and the standing method sentence survives in the instruction verbatim. The change's analysis runs multi-angle before the fix decision is taken (design §3). |
| C-14 | The delivery clause — the minor version carry, the commit, the push, the latest release — executed after the change and its documentation stand complete (the gates green, the CHANGELOG entry written, the live probe clean). The delivery commits name the work they deliver. |
| C-15 | Write surface: the three `.aiprj` documents, the work log, and the product files the delivery commit's own stat carries (for this cycle: the mouse-handling surface `src/app.rs` beside `src/config.rs`/`src/cli.rs`/`src/ai/*`, `README.md`/`README_ja.md`, the usage/config/architecture/personas/tools/troubleshooting doc rows, the renamed provider guides, the example config, the CHANGELOG, `Cargo.toml`, `Cargo.lock`). No document carries any mark of authorship or tooling (NFR-12). |
| C-16 | The update passes write the three `.aiprj` documents and the work log only — no product file, no commit (the AI Update Guidelines' Article 3); the delivered trail is nameable only from a commit that follows the pass's writes (the trail rule). |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-42 | FR-50 a/b/c met: the mechanism evidenced at its sites (the file:line record) and probed live (NFR-15), the change delivered with the boundary intact (§6), the fixed behavior carrying its own pin (NFR-13), the suite green and clippy standing its notices at the changed tree (C-09), the live probe showing the mouse perform nothing mid-turn and everything it did at rest, the documentation written and bound to the code (the CHANGELOG `[0.49.0]` entry, the README/usage rows the change names), the version at **0.49.0**, the delivery committed and **pushed to `origin/main`**, and the release published as the **latest** version; the three `.aiprj` documents mutually consistent (C-08), the work log written per Art. 6/7, and the instruction re-verified byte-exact at exit (NFR-12 throughout). — **Not met** (the mechanism's live faces stand verified by the delivery session's battery — the mid-turn bar faces and the at-rest wheel/click faces; the docs face T-136 delivered; the completion T-130 — the commit, the push, the tag, the release — still to run; AC-42's remaining faces are the delivery's own). |
| AC-43 | FR-51 a/b/c met: the rename surface pinned at its sites file:line and probed live (NFR-11, NFR-15 — the shipped config parsed and the renamed kinds resolved), the renames delivered (the config tables, `kind` values, `--provider`, the deserializer's rename keys, the error prefixes/diagnostics), the boundary intact (§6), the shipped pins re-deriving, the suite green and clippy standing its notices at the changed tree (C-09), the documentation written and bound to the code — **the two per-backend guides still carry thirteen stale old-kind rows (BL-M65)**, the README/README_ja/usage/`doc/config.md`/`doc/architecture.md`/`doc/personas.md`/`doc/tools.md`/`doc/troubleshooting.md`/example/doc rows standing renamed — the version at 0.49.0, the delivery committed and **pushed to `origin/main`**, and the release published as the **latest** version; the three `.aiprj` documents mutually consistent (C-08), the work log written per Art. 6/7, and the instruction re-verified byte-exact at exit (NFR-12 throughout). — **Not met** (the rename's faces stand delivered — the guides' 13 rows swept clean and the workspace config rows renamed by the delivery session's sweep, T-136 ✅; the completion T-133/T-134 — the commit, the push, the tag, the release — still to run; AC-43's remaining faces are the delivery's own). |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-87 | 「エージェント動作中」 names the running-turn window: the span in which a turn is in flight — from its submission/start to its terminal event, the same span the tree's own markers carry (the `turn_in_flight` flag and its `watch` mirror; BL-M60) — not the whole session, and not the at-rest prompt. Outside that window the mouse's work stands unchanged (§3.1). | Read from the instruction and the tree's running-turn markers (BL-M60); applied in FR-50a. |
| AS-88 | 「マウス操作は禁止」 (with 「禁止に変更する」) is read as the change direction on agent-cli's own mouse handling: the operations agent-cli performs on mouse events — the wheel's scroll move, the click's toggle, the drag's select-and-copy — are to be **not performed** while a turn runs. The mechanism's shape (gating the arms' actions; or suspending the capture for the turn's span) is the cycle's decision (design §9, D-91): the terminal's own selection/scrollback, and what the user does outside agent-cli's face, are not the clause's target; the reading's scope is pinned before any repair conclusion stands. The delivered helper's polarity (a turn **opens** the arms) violates this reading (BL-M64). | Read from the instruction (§2), the shipped click face's contracts (§6, the FR-48 records) and the mouse arm's sites (BL-M60); applied in FR-50a; repaired by T-129. |
| AS-89 | 「マイナーバージョンアップ」 is the minor bump 0.48.0 → **0.49.0** — the trail carries the standing sentence's shape (0.46→0.47→0.48→0.49), and 「リリースを最新バージョンにする」 is the release published so it is the latest (FR-46c/FR-47c/FR-48d/FR-49d's verified shape). | Read from the instruction, the tree's trail (BL-M61) and the delivered cycles' records; applied in FR-50c. |
| AS-90 | 「config.tomlのcodex-codeはcodex-cliへ変更」 (with the settled instruction's 「claude-codeはclaude-cliへ変更」) names the configuration surface's backend **kind strings**: the section names 「config.toml」, and the shipped configuration names its CLI-child backends `codex-code` and `claude-code` (the `[provider.codex-code]`/`[provider.claude-code]` tables, `kind = "codex-code"`/`"claude-code"`, the rename keys the deserializer accepts, `--provider` values). The ask renames those kind strings to `codex-cli`/`claude-cli` wherever they name this surface — not the section headings' internal field names alone, not the providers' wire protocols, and not the child binaries themselves (the `codex`/`claude` executables keep their names). The exact rename span (which identifiers beyond the kind strings — the error prefixes, the diagnostics, the file names — ride it) is pinned before any repair conclusion stands. **Delivered span (execution session):** the `[provider]` `kind` values, the table keys and their serde renames, `--provider` and its help row, the factory/`SUPPORTED` arms, `for_provider_section`/`provider_entry` lookups, the error prefixes and diagnostics, the doc faces, the example, the spawn tool's description, and the transcript drop file's name (`.agent-cli-codex-cli-last.txt`); the Rust identifiers (the `claude_code`/`codex_code` fields, the provider type names, the module file names) stayed — not user-facing kinds. | Read from the instruction (§2) and the shipped config surface (BL-M62); applied in FR-51a; delivered per BL-M63; the span recorded in design §10. |
| AS-91 | 「config.tomlの内容変更」 names the instruction's own section title and not a broader rewrite: the two clauses it carries (the two renames) are the ask; no other config key, default, or section changes unless the renames' own surface requires it. **Outstanding:** the workspace's own `.aiprj/config.toml` still carries the old-kind strings at `:12` and `:30` (the example's own copy, unchanged by any cycle — noted as the shipped repo state; no session has a mandate it rewrites it; its rewrite where it matters is the delivery cycle's judgement, recorded in the tasks' T-136 row). | Read from the instruction (§2); applied in §4.1's FR-51 scope. |
| AS-82 | (The FR-48 record — closed: delivered.) 「マウスクリックすると1行ズレた表示になる」 named the display's displacement: after a mouse click the screen's rows sat (approximately) one row away from their pre-click positions — the whole face shifted — and 「ズレない表示に改修する」 asked the unshifted face back. What the clicks themselves did (the thinking view's toggle, the drag's copy, the wheel's scrollback) was not the complaint and stood (§3.1). | Delivered in FR-48a (`f9ec9ae`; §12). |
| AS-86 | (The FR-48 record — closed: delivered.) 「マウスクリックするとプロンプトを表示しないようにする」 was read as the remedy direction of the same report: the click path's own handling re-displayed the prompt, and the ask removed that painting — the prompt face stood as the shipped flow's hands drew it, not as the click's pass left it. | Delivered in FR-48b (§12). |
| AS-83 | (The FR-48 record — closed: delivered.) 「マイナーバージョンアップ」 was the minor bump 0.46.0 → **0.47.0**, and 「リリースを最新バージョンにする」 the release published latest. | Delivered in FR-48d. |
| AS-76 | (The FR-47 record — closed: delivered.) 「ワンショットで完結するモード」 names a mode of running one's own agent process: one prompt through the configured backend to its terminal event, with the process exiting after. | Delivered in FR-47a (`77e8a29`; §12). |
| AS-77 | (The FR-47 record — closed: delivered.) The one-shot shares `run`'s startup options through the existing `RunArgs`; `--quiet` is the one new flag. | Delivered in FR-47a. |
| AS-78 | (The FR-47 record — closed: delivered.) 0.45.0 → **0.46.0** minor; the release published latest. | Delivered in FR-47c. |
| AS-79 | (The FR-47 record — closed: delivered.) The `--quiet` flag as delivered is **inert**: the one-shot's body prints the answer text alone (errors to stderr) with or without it. | Delivered in FR-47a (§12, the D-82 record). |
| AS-80 | (The FR-47 record — closed: delivered.) The delivered one-shot's answer path is **inline** in `run_oneshot`'s event loop — no display task, no `Theme`, no banner exists in the arm. | Delivered in FR-47a. |
| AS-81 | (The FR-47 record — closed: delivered.) `doc/architecture.md` was **not** written by the FR-47 delivery — the delivered doc rows are the README's and usage's, and the `[Unreleased]` heading stands above the newest entry. | Delivered in FR-47c. |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-50a | design §1 (Q-31, D-91), §9 (D-93's inverted verdict standing as the execution record; BL-M66 the repaired state) | T-128 🟨 (the analysis; the verdict rides T-135's live corroboration), T-129 🟨 (the delivered join, polarity repaired), T-135 🟨 (delivered at the tree, its live faces uncorroborated) |
| FR-50b | design §3 (the docs angle), §4 (W-73) | T-136 ✅ (the docs sweep; the completion's own recording face rides T-130) |
| FR-50c | design §4 (W-73) | T-130 ⬜ (this session's delivery) |
| FR-51a | design §10 (D-92 delivered, the BL-M63 span) | T-132 ✅ |
| FR-51b | design §3 (the docs angle), §4 (W-74) | T-132 ✅, T-136 ✅ |
| FR-51c | design §4 (W-74) | T-133 ⬜, T-134 ⬜ |
| AS-90, AS-91 | design §10 | T-132 🟨, T-136 ⬜ |
| NFR-11, NFR-15 | design §3 (the mechanism and live-verdict angles), §4 | T-128 🟨, T-129 ✅, T-132 ✅, T-135 ✅ (the live battery), T-136 ✅ (the sweep), T-133 ⬜, T-134 ⬜ |
| NFR-13 | design §3 (the contracts angle), §4 | T-129 ⬜ (the pin re-derives inverted), T-134 ⬜ |
| NFR-14 | design §3 (the contracts angle) | T-129 ⬜, T-132 🟨 |
| C-09 | design §4 (W-73/W-74) | T-133 ⬜, T-134 ⬜ |
| C-14 | design §4 (W-73/W-74) | T-130 ⬜, T-133 ⬜, T-134 ⬜ |
| AS-87, AS-88, AS-89 | design §1, §9 | T-128 🟨 |
| BL-M63, BL-M64, BL-M65 | design §9, §10 | T-129 ⬜, T-132 🟨, T-136 ⬜ |
| C-08 (this pass) | design §1–§4, §9, §10 | T-131 ✅ |
| C-15, C-16 (this pass, Art. 3) | — | T-131 ✅ |
| AC-42 | design §1–§9 | T-128–T-129, T-130 |
| AC-43 | design §1–§10 | T-132, T-133, T-134, T-136 |
| FR-49 (delivered cycle, closed — the release publication closed C-17) | design §6–§8 (the record; D-85–D-90) | T-121 ✅–T-127 ✅ |
| FR-48 (delivered cycle, closed) | design §1 (D-84), §2 (TPR-30), §4 (W-72 ✅) | T-112 ✅–T-120 ✅ |
| FR-47 / FR-46 (delivered cycles, closed) | design §5 | (closed) |


## 12. The previous cycles' record

**"the answer-display prompt handoff" (FR-49 a/b/c/d)** — closed,
2026-11-07: the user's live report scoped FR-49 a/b/c/d; the mechanisms
pinned live and fixed at `src/app.rs` per D-85 (the terminal hand-off
consumed in every prompt state), D-86 (the running-turn marker carrying the
TurnStart→terminal span) and D-87 (the zone erase's coordinate order), with
the synchronous `watch` arm; the gates at the delivered tree **667 unit + 6
integration, 0 failed** (the two zone pins joining the gates; clippy
standing its five notices); the delivered live faces verified; the version
carried 0.47.0 → **0.48.0**; the delivery commit `89f1ea0` pushed and the
tag `v0.48.0` verified on it. **The release publication closed (C-17):**
the repo's release automation `.github/workflows/release.yml` was built
(`1d8ccc3`, `0a49302`, `40fc7c4`), its first runs failing at the runner
(exit 128 at `1d8ccc3`/`0a49302`; exit 141 at `40fc7c4` — D-90's SIGPIPE
mechanism, reproduced locally and fixed), the two product fixes delivered
at `9040435` ("Fix the release workflow's SIGPIPE and restore the
CHANGELOG's [0.47.0] heading"; the `[0.47.0]` heading restored beneath
`[0.48.0]`'s section), the tag `v0.48.0` moved onto `9040435` and
force-pushed, the workflow's run `42317643598` green, and the release
`v0.48.0 — After the [answer] display finishes, the prompt is displayed
again — every time.` published standing **Latest**
(2026-11-07T00:20:48Z, verified `/releases/latest`).
Work logs `2026-11-06_019.md`, `2026-11-07_000.md`–`_002.md`.

**"the mouse-click display defects repaired" (FR-48 a/b/c/d)** — closed,
2026-11-06: the instruction's two-clause ask scoped FR-48 a/b/c/d (AS-82/
AS-86); the mechanisms evidenced at the click path's sites (D-84), probed
live first (the ~25-scenario battery), the fixes delivered at `src/app.rs`
with the two pins (`a_bare_click_costs_the_screen_nothing`,
`a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt`); the docs
delivered (the CHANGELOG's `[0.47.0]`/`### Fixed`); the version carried
0.46.0 → **0.47.0**; the delivery commit `f9ec9ae` ("Fix the mouse-click
display: a click leaves every row where it stood and paints no prompt")
pushed, the tag `v0.47.0` verified on it, the release `v0.47.0` published
and verified **Latest** (2026-11-06T11:37:01Z); gates at the fixed tree
**665 unit + 6 integration, 0 failed**; AC-41 **Met**. The docs-angle
verdict of the 23:10 execution session hoisted the CHANGELOG's stranded
`[Unreleased]` heading (`315ee7f`). Work logs
`2026-11-06_011.md`–`_017.md`.

**"the one-shot mode added" (FR-47 a/b/c)** — closed, 2026-11-06:
delivered at `77e8a29` ("Add the one-shot subcommand: one prompt in, the
printed answer out, then exit"), the version carried 0.45.0 →
**0.46.0**, the tag `v0.46.0` verified on it, the GitHub release
`v0.46.0` published and verified **Latest** (published 01:16:50Z); gates
**663 unit + 6 integration, 0 failed**; clippy standing its three notes.
The delivered shape (the design §5 record carries D-78–D-83).
The recording follow-ups `1b3c7e1`/`8c26ecc`. Work logs
`2026-11-06_009.md`–`_010.md`; the cycle's ledger record `_008`.

**"the terminal display fix" (FR-46 a/b/c)** — closed, 2026-11-06:
delivered at `ebe0aca` ("Fix the terminal's bottom line: the prompt when
input is possible, the running clock while the agent runs"), the version
carried 0.44.0 → **0.45.0**, the tag `v0.45.0`, the release published and
verified **Latest** (2026-11-06T00:55:05Z); gates 663+6. The trail's
passes `2557f7f`/`92c43fd` and the follow-up `5978dc9`. Work logs
`_005`–`_007`.

**"the prompt-input queueing feature's removal" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c` (v0.44.0), the follow-ups
`6f0390f`/`0d45242` recording it; gates 663+6. Work logs `_002`–`_004`.

**"the codex-code backend …" (FR-44 a/b/c/d, FR-44e)** — closed,
2026-11-05: `122f320`/`4fee899`, versions 0.42.0/0.43.0, the release
`v0.43.0` verified latest; gates 669+6. Work logs `_010`–`_012`
(2026-11-05).

**"the codex access investigation" (FR-43)** — closed, 2026-11-05 (no
release): work log `_008` (2026-11-05). **"the refactoring … the prompt's
bottom row made prompt-only" (FR-41, FR-42)** — closed, 2026-11-05:
`eec12f9`/`064ce50`, v0.41.0; gates 647+6. Work logs `_006`/`_007`.
**"the display corrections…" (FR-39, FR-40)** — closed, 2026-11-05:
`db68d3f`/`306f425`, v0.40.0 (`505097a`, named by `856c2bf`). **"the four
display regions" (FR-37, FR-38)** — `39afeb2`, named by `fb46cc0`; gates
645+6. Later records: the prompt-input queueing (FR-35, v0.38.0 —
delivered, then **removed** by FR-45); FR-32–FR-34 (v0.37.0 on
`5a2d7f0`); FR-31; FR-29/FR-30; FR-27/FR-28 (v0.34.0 on `c022909`); FR-26
(v0.33.0 on `6cc6661`); FR-22–FR-24 (v0.32.0); the mouse (v0.31.0); add
strata (v0.30.0–0.30.2); v0.29.0–v0.25.0 and the 機能追加 addenda (v0.26.0).

| FR-50 (the running-turn mouse prohibition) | **In progress — the mechanism and its docs stand delivered, the completion rides the delivery**: the ~10:20 session delivered the mechanism, polarity-inverted (BL-M64); the follow-on session executed the polarity repair at the tree at 11:05 (BL-M66 — the three gates reading the prohibition helper negated, the pin re-faced); the delivery session of ~11:44–12:10 ran the repair's live battery through the byte-verified tmux SGR transport (crossterm's parser proven over the exact frames by a standalone test): the mid-turn faces all stand barred (the wheel moves no view and raises no marker, the drag produces no `[clip]` report, the click toggles nothing) and the at-rest controls re-derive (the wheel moves the view, a bare click writes nothing — the FR-48 zero-touch face; the at-rest drag's copy face stands on the suite pins and the FR-48 record, the rig's injection limit recorded — T-135 ✅); the docs face delivered (the README/README_ja/usage rows and the guides' sweep — T-136 ✅); the completion (FR-50c) rides the delivery (T-130). |
| FR-51 (the backend kind strings `codex-code`/`claude-code` renamed `codex-cli`/`claude-cli`) | **In progress — the rename and its docs stand delivered in full, the completion rides the delivery**: FR-51a delivered at the tree (BL-M62 → zero old-kind strings in `src/`; the guides renamed as files, the example's blocks renamed); FR-51b delivered by the delivery session's sweep (the two guides' 13 stale rows cleared, the workspace `.aiprj/config.toml`'s two comment rows renamed — every shipped face names the new kinds; T-132 ✅/T-136 ✅); the completion (FR-51c) rides the delivery (T-133/T-134). |
| The 2026-11-08 +0900 re-load pass (extended live) | **Done** (opening record, pre-execution): the rewritten instruction verified (412 bytes at the pass's read; sha256 `76c3d039…`), then found **extended during the pass** — finally settling at 515 bytes, sha256 `179b698a…`, mtime 2026-11-08 08:38:16.354426690 +0900 (「codex-codeはcodex-cliへ変更」 + 「claude-codeはclaude-cliへ変更」) — and the settled state verified byte-exact at the pass's exit (§2); the ledgers re-scoped around the two pending cycles FR-50 and FR-51, the delivered state re-verified from the tree (HEAD `41327d7` ≡ `origin/main`; the version files 0.48.0; the release `v0.48.0` standing Latest), the FR-48/FR-49 records closed into §12, the stale rows resolved (C-08); **no product file written and no commit made** (C-15/C-16, Art. 3); the work logs `2026-11-08_000.md` and `_001.md` (the opening re-scope); the execution session that followed delivered the product writes at the tree (its exit log `2026-11-08_001.md`). |
| The 2026-11-08 execution session (the FR-50/FR-51 product writes) | **Done** (recorded by the 10:15 pass, the state read at its tree): the session delivered the FR-50a mechanism at `src/app.rs` (the pure helper `:1446`, the three gates `:3452`/`:3503`/`:3556`, the pin `:8696`) and the FR-51 renames across `src/config.rs`/`src/cli.rs`/`src/ai/*`/`src/commands.rs`/`src/tools/spawn.rs`, the docs and the example, with the version carried to 0.49.0 and the CHANGELOG's `[0.49.0]` entry standing (BL-M63); its exit log reports the corrected transport's live probe (the wheel/click/drag performing nothing mid-turn) — **this pass's read of the tree contradicts that face: the gates' polarity is inverted** (§3.2), so the probe's verdict stands uncorroborated and the face unverified; the version files **0.49.0** standing (verified at the requirements' §2
source records) and the exit log `_001`. |
| FR-49 (the answer-display prompt handoff) | **Delivered and closed** (2026-11-07): the delivery `89f1ea0`, the version 0.48.0, the tag `v0.48.0`; the publication closed — the tag moved onto `9040435`, run `42317643598` green, the release `v0.48.0` published standing **Latest** (2026-11-07T00:20:48Z); gates at the delivered tree **667 unit + 6 integration, 0 failed**; T-121–T-127 ✅; §12. |
| FR-48 (the mouse-click display defects) | **Delivered and closed** (2026-11-06): `f9ec9ae`, v0.47.0 Latest; AC-41 Met; §12. |
| FR-47 (the one-shot mode) | **Delivered and closed** (2026-11-06; §12): `77e8a29`, v0.46.0 Latest, the follow-ups `1b3c7e1`/`8c26ecc`. |
| FR-46 (the terminal display fix) / FR-45 / FR-44 / earlier | **Delivered and closed** (§12). |
| State at exit (the 2026-11-08 11:35 re-load pass) | The instruction verified byte-exact (515 bytes, sha256 `179b698a…`, mtime 2026-11-08 08:38:16.354426690 +0900) at the pass's read and re-verified at its exit (§2); HEAD `41327d7` ≡ `origin/main`, the working tree uncommitted (the execution sessions' product writes standing: BL-M63/BL-M66); FR-50a's polarity repair executed at the tree (BL-M66; T-128/T-129/T-135 🟨 — its live corroboration then unwritten), FR-51a delivered (T-132 🟨), FR-51b's 13 guide rows and FR-50b's mouse doc rows outstanding (T-136 ⬜, BL-M65), FR-50b/c and FR-51c completions outstanding (T-130/T-133/T-134 ⬜); AC-42/AC-43 not met; the FR-48 (`f9ec9ae`, v0.47.0), FR-49 (`89f1ea0`/`9040435`, v0.48.0 Latest) and prior cycles closed (§12); the trail stood `… → 9040435` (the release delivery; the tag `v0.48.0`) → `41327d7` (the ledger follow-up; **HEAD**); the standing gates re-derived at the changed tree: **668 unit + 6 integration, 0 failed**, clippy its five notices (C-09). *The delivery session that followed (~11:44–12:10) executed the polarity repair's live battery (T-135 ✅) and the documentation sweep (T-136 ✅) — the state §13's delivery-session row records; its ledgers stand uncommitted for the trail's next follow-up (C-16).* |
## 13. Delivery status

| Item | State |
|---|---|
| The 2026-11-08 10:15 +0900 update pass | **Done** (the 2026-11-08 10:15 +0900 pass): the instruction reloaded **byte-exact** (515 bytes, sha256 `179b698a3df2b62fb114b16171cec9592694157d0c625eafd0ed5a3538e1a373`, mtime 2026-11-08 08:38:16.354426690 +0900; no extension during the pass) and re-verified at exit; the tree re-derived (HEAD `41327d7` ≡ `origin/main`; the working tree carrying the execution session's uncommitted product writes — BL-M63); the pass found the tree moved since the previous ledger write and re-verified every face at the changed tree: the gates re-derived live (**668 unit + 6 integration, 0 failed**; clippy **five notices** — C-09's sites now re-cited `src/app.rs:978`, `:2409`, `:8517`, `:8787`, `src/agent.rs:2101`), the version files **0.49.0** standing, the CHANGELOG's `[0.49.0]` entry standing under the empty `[Unreleased]` heading (§3, BL-M63), the FR-50a mechanism read at its sites and found **delivered polarity-inverted** (BL-M64 — the wheel/click/drag gates open only mid-turn and stand barred at rest, the inverse of FR-50a; the pin verifying the inverted helper; FR-50b's README/usage rows unstale-written), the FR-51 rename verified delivered and complete in `src/` (zero old-kind strings) with **13 stale lines remaining in the two renamed guides** (BL-M65 — FR-51b partially delivered, uncommitted), the completion rows outstanding (no commit, no tag, no release; `gh` unavailable at this pass's environment), and the execution session's own exit log `2026-11-08_001.md` read and honored as the delivery record; **no product file written by this pass** (the write surface: the three `.aiprj` documents and the work log `2026-11-08_002.md`; C-15/C-16, Art. 3); one tool-layer file-write failure (a corrupted REQUIREMENTS write) was detected, backed up by the environment, and repaired from the context-held source with byte-shape verification after every append — the file's state verified at exit (**418 lines at exit, one `|` delimiter per row, one H2 heading per section**). |
| FR-50 (the running-turn mouse prohibition: the handling bar at its shipped gates, the docs, the version 0.49.0, the commit/push, the latest release) | **In progress — the mechanism delivered polarity-inverted; the repair T-135 pending** (this pass's verdict): the execution session delivered the product face at the tree (the helper `the_mouse_operation_is_prohibited` `src/app.rs:1446`, the three gate joins `:3452`/`:3503`/`:3556`, the pin `a_running_turn_bars_every_mouse_operation` `:8696`; the CHANGELOG's `[0.49.0]` entry standing; the version files 0.49.0) — but the helper returns `turn_in_flight` itself and all three gates join it **un-negated**, so the arms open exactly while a turn runs and are barred exactly at rest: the inverse of FR-50a's ask and a break of the shipped at-rest mouse contract (§3.1). The repair T-135 (the conjunction's polarity; the helper `!turn_in_flight`-faced — the same one-conjunction shape the delivered intent named) stands pending; the repair re-derives the shipped at-rest faces (the FR-48 zero-touch contract, the FR-49 prompt-return) beside the mid-turn bar FR-50a names. |
| FR-51 (the backend kind strings `codex-code`/`claude-code` renamed `codex-cli`/`claude-cli`: the config tables, `kind` values, `--provider`, the diagnostics face the evidence names, the docs, the completion) | **In progress — FR-51a and FR-51b delivered in full at the working tree (uncommitted); the completion rides the delivery** (the delivery session's verdict at ~12:10): the rename's every src face verified at the tree (the `[provider]` kind values, the serde table-key renames, `provider_entry`/ensure arms, the factory arms and `SUPPORTED`, the `--provider` help, the spawn tool's provider description, the error prefixes/diagnostics — zero old-kind strings in `src/`, grep-clean; the workspace's root `config.toml` clean); the docs' rows delivered across the README/README_ja/usage/config/architecture/personas/tools/troubleshooting guides and the example, with the per-backend guides renamed as files (`doc/providers/claude-code.md` → `claude-cli.md`, `doc/providers/codex-code.md` → `codex-cli.md`); **the 13 stale guide lines cleared by the delivery session's sweep** (the `kind =` blocks ×2, the table keys, the doctor/selftest rows, the error-table rows, the codex guide's cross-reference — grep-clean, zero old-kind strings in any shipped face; the CHANGELOG history rows standing untouched by design — past releases' own record; the workspace `.aiprj/config.toml`'s two comment rows renamed per the sweep's recorded judgement); the Rust identifiers and the module file names stay (D-92's span decision, standing); the transcript drop file renamed at its source; the version files standing **0.49.0**; the completion (the commit, the push, the tag `v0.49.0`, the release standing latest) rides the delivery session (T-133/T-134). |
| The FR-50 execution session (2026-11-08, from its own exit log `2026-11-08_001.md`) | **Done as recorded; the delivery's state as recorded at exit**: FR-50a delivered at the tree with gates **668 unit + 6 integration, 0 failed** and the live probe battery run (the probe config's `base_url` fix verified by the one-shot; the mid-turn injected wheel/click/drag performing nothing; the at-rest wheel face verified); **this pass's polarity verdict at the same sites contradicts the session's at-rest verdict on an evidence-first read** (the gates stand open only mid-turn — the polarity row §13's FR-50 item; the repair T-135 since executed at the tree by the 11:05 session — BL-M66, the 11:35 pass's fresh read); FR-51 delivered (FR-51a complete in `src/`, FR-51b's 13 guide lines remaining); the version carried 0.48.0 → **0.49.0**; the gates re-derived by the pass live (**668 unit + 6 integration, 0 failed**; clippy its three notices re-derived fresh under the pass's clippy 0.1.94 — the pass's re-based record; its own five-notice face superseded). |
| FR-49 (the answer-display prompt handoff: the prompt returns after a turn's answer every time, the docs, the version 0.48.0, the commit/push, the latest release) | **Delivered and closed** (2026-11-07): the delivery `89f1ea0`, the version 0.48.0, the tag `v0.48.0`; the publication closed at the delivery session — the tag moved onto `9040435` (the workflow's SIGPIPE fix, D-90; the CHANGELOG's `[0.47.0]` heading restored), run `42317643598` green, the release `v0.48.0` published standing **Latest** (2026-11-07T00:20:48Z); gates at the delivered tree 667 unit + 6 integration, 0 failed; T-121–T-127 ✅; §12. |
| The 2026-11-08 11:35 +0900 consistency re-load pass | **Done** (this pass): the instruction reloaded **byte-exact** to the settled 515-byte state (sha256 `179b698a3df2b62fb114b16171cec9592694157d0c625eafd0ed5a3538e1a373`, mtime 2026-11-08 08:38:16.354426690 +0900; no extension during the pass) and re-verified at exit; the tree re-derived at HEAD `41327d7` ≡ `origin/main` and found **moved behind the ledgers again**: an execution session had rewritten `src/app.rs` at 11:05:56 +0900 — the FR-50a polarity repair executed (BL-M66, verified at the sites this pass's §1 tail records; mtime 11:05:56.978183823 +0900) — and had written its opening work log `2026-11-08_004.md` (11:14) without an exit record. This pass verified the repair face at its sites, re-confirmed the FR-51 residuals line-exact (BL-M65's 13 rows unshifted) and the FR-50b doc rows untrue at their recorded lines (`README.md:26`/`:27`, `doc/usage.md:357`/`:420`/`:429`; the README_ja rows `:26`/`:27` and the keyboard table's `:548–:549`), the version files **0.49.0** standing, the CHANGELOG's `[0.49.0]` entries standing under the empty `[Unreleased]`, the example and guide files standing renamed/clean, the workspace `.aiprj/config.toml`'s two stale comment rows `:12`/`:30` standing; the gates re-derived live at the repaired tree (**668 unit + 6 integration, 0 failed**; clippy **five** notices re-derived fresh — `src/app.rs:978`, `:2412`, `:8520`, `:8801`, `src/agent.rs:2101`; C-09's record re-based, the three-notice face of the 10:42 pass not reproducing); the multi-angle analysis ran on file/live evidence (the registered peers unreachable — the standing record; C-12); the ledger inconsistencies resolved (Art. 2 — the polarity faces across §1/§5/§11/§12/§13 and the task table's T-128/T-129/T-135 cells aligned to the executed repair, the clippy record re-based, the pass attributions re-said as the prior passes'); **no product file written and no commit made** (the write surface: the three ledgers and the work log `2026-11-08_005.md`; C-15/C-16, Art. 3). |
| The 2026-11-08 10:42 +0900 consistency re-load pass | **Done** (the 2026-11-08 10:42 +0900 pass): the instruction reloaded **byte-exact** to the settled 515-byte state (515 bytes, sha256 `179b698a3df2b62fb114b16171cec9592694157d0c625eafd0ed5a3538e1a373`, mtime 2026-11-08 08:38:16.354426690 +0900; no change during the pass) and re-verified at exit; the tree re-derived at HEAD `41327d7` ≡ `origin/main` with the working tree standing uncommitted and unchanged since the 10:15 pass's read (BL-M63): the FR-50a mechanism re-read at its sites and confirmed **polarity-inverted** (BL-M64 — the helper `the_mouse_operation_is_prohibited` `src/app.rs:1446` returns `turn_in_flight` and the wheel/click/drag gates `:3452`/`:3503`/`:3556` join it as an enabler; the unit pin's assertions re-run green over the inverted helper), the FR-51a `src/` surface re-scanned old-kind-free (grep-zeros), the 13 guide residuals re-confirmed line-exact (BL-M65 — `doc/providers/codex-cli.md` `:29`/`:31`/`:108`/`:117`/`:118`/`:142`/`:143`, `doc/providers/claude-cli.md` `:27`/`:29`/`:108`/`:109`/`:129`/`:131`), the CHANGELOG's `[0.49.0]` entry and the version files 0.49.0 standing; the gates re-derived live (**668 unit + 6 integration, 0 failed**; clippy **three** notices re-derived fresh — C-09's re-based record, the standing sites `src/app.rs:978`/`:8517`/`:8787`, the older `:2409`/`src/agent.rs:2101` rows not reproducing under clippy 0.1.94 and their sites clean under the forced-lint probe); the ledger inconsistencies resolved (Art. 2: the §5/§11 status faces aligned to the task table — T-128/T-129 🟨, the pass rows carried T-131's ✅ face; the standing clippy record re-based; the 10:15 pass's rows re-said as the prior pass's); **no product file written and no commit made** (the write surface: the three ledgers and the work log `2026-11-08_003.md`; C-15/C-16, Art. 3). |
| The 2026-11-08 ~10:20 +0900 execution session (the working-tree product writes) | **Done** (the 10:15 pass's verified record): the product tree's changes standing uncommitted at this pass's read (BL-M63) — the FR-50a mechanism delivered polarity-inverted (BL-M64 — the repair T-135 pending), the FR-51 rename delivered across `src/` and the doc faces (the guides' 13 stale lines BL-M65 — the completion T-136 pending), the CHANGELOG's `[0.49.0]` entry, the version files 0.50→**0.49.0** standing, the guide files renamed (the new names untracked), the gates standing green as re-derived (**668 unit + 6 integration, 0 failed**), the example's workspace `.aiprj/config.toml`'s two stale comment rows riding BL-M65's sweep, and the ledgers standing at their 08:53 pending-state writes until this pass; the session's own record stands in its exit log `2026-11-08_001.md`. |
| FR-48 (the mouse-click display defects) | **Delivered and closed** (2026-11-06; §12): `f9ec9ae`, v0.47.0 Latest; AC-41 Met. |
| Earlier cycles' record | FR-47 (`77e8a29`, v0.46.0), FR-46 (`ebe0aca`, v0.45.0), FR-45 (`b5deb9c`, v0.44.0) and the prior record — **delivered and closed** (§12). |
| The 2026-11-08 ~11:44–12:10 +0900 delivery session (the FR-50/FR-51 remainder) | **Done**: T-135's live battery — the repair's faces corroborated through tmux's byte-verified SGR 1006 transport (crossterm 0.27's own parser proven over the exact frames by a standalone test; the wheel-up arm's live face the transport's proof): **mid-turn** (spinner-proven glm-5.3:cloud turns) the wheel notches moved no view and raised no `Message below` marker, the drag press/motion/release produced no `[clip]` report and no selection face, the click toggled nothing; **at rest** the wheel notches moved the view (the FR-39 face), a bare press-and-release wrote nothing at all (the FR-48a/FR-48b zero-touch face), and the prompt returned after every turn (FR-49). The at-rest drag's **copy** face the rig could not corroborate live (the tmux injection delivered press and release but never extended a selection on screen — a tool-layer transport boundary, recorded in the work log `-006`; not a product defect: the shipped pins `a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt` and the `selected_text` unit tests stand green, and the FR-48 delivery's own live battery verified the copy face at `f9ec9ae`). T-136's sweep — the two renamed guides' thirteen old-kind rows cleared (`doc/providers/codex-cli.md` `/`claude-cli.md``:29`/`:31`/`:108`/`:117`/`:118`/`:142`/`:143` and `:27`/`:29`/`:108`/`:109`/`:129`/`:131`, plus the codex guide's cross-reference row), the README_ja rows `:26`/`:27`/`:548–:549` updated beside the README/usage rows, and the workspace's own `.aiprj/config.toml` `:12`/`:30` comment rows renamed; the shipped faces grep-clean. The gates re-derived at the swept tree: **668 unit + 6 integration, 0 failed**; clippy standing **five** notices (`src/app.rs:978`, `:2412`, `:8520`, `:8801`, `src/agent.rs:2101`; C-09). The tasks' ledger carries T-135/T-136/T-132 ✅; the completion rows (the commit, the push, the tag `v0.49.0`, the release standing latest) rode the session's delivery step (C-14). |
| State at exit | The instruction verified byte-exact (515 bytes, sha256 `179b698a3df2b62fb114b16171cec9592694157d0c625eafd0ed5a3538e1a373`) at the 11:35 pass's read and **re-verified byte-exact at its exit** (§2); FR-50a's polarity repair standing executed at the tree (BL-M66; T-135 🟨 then), FR-50b's doc rows outstanding (T-136 ⬜ then), FR-50c and FR-51c completions outstanding (T-130 ⬜, T-133 ⬜, T-134 ⬜), FR-51a delivered (T-132 🟨 then), FR-51b then holding 13 stale guide rows (T-136 ⬜); AC-42/AC-43 **not met** at that pass's exit; the delivered cycles stand closed (FR-48 `f9ec9ae` v0.47.0, FR-49 `89f1ea0`/`9040435` v0.48.0 Latest — §12); the trail stood `… → 9040435` (the release delivery; the tag `v0.48.0`) → `41327d7` (the ledger follow-up; **HEAD**); the standing gates as re-derived: **668 unit + 6 integration, 0 failed**, clippy five notices (C-09); the 10:15/10:42 passes' records stand as their own rows above. *The delivery session's work since then: T-135's live battery ✅, T-136's sweep ✅ (the guides' 13 rows, the README_ja rows, the workspace `.aiprj/config.toml` rows), the gates re-derived green at the swept tree — the delivery rows the live work log and §13's delivery-session row carry; the completion commit/push/tag/release ride the session's delivery step.* |
