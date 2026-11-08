# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "mouse operations during agent operation are to be prohibited" (FR-50 a/b/c — the repair executed and live-verified, the docs swept; the delivery rides this session) and "the backend kinds `codex-code`/`claude-code` renamed `codex-cli`/`claude-cli`" (FR-51 a/b/c — the rename surface and the docs delivered; the delivery rides this session)
(`.aiprj/instructions.md`, reloaded 2026-11-08 08:27 +0900 (412 bytes, sha256 `76c3d039…`), then **extended live while this pass stood mid-write** — first at 08:37:43 (515→456 bytes; a second 最優先事項 section 「config.tomlのcodex-codeはcodex-cliへ」, whose title then settled as 「config.tomlの内容変更」), finally settling at 08:38:16 at 515 bytes, sha256 `179b698a3df2b62fb114b16171cec9592694157d0c625eafd0ed5a3538e1a373`, mtime 2026-11-08 08:38:16.354426690 +0900, with the two rename clauses 「codex-codeはcodex-cliへ変更」/「claude-codeはclaude-cliへ変更」; the settled state recorded in the requirements' §2; the tree verified byte-exact against it — and re-verified byte-exact at every subsequent re-load pass, the 11:35 pass included)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked

---

## Overview

The reloaded instruction now carries two active 最優先事項 sections. The first, 「エージェント動作中のマウス操作は禁止」 (FR-50a): while a turn is in flight, the mouse operations agent-cli performs — the wheel's scrollback move, the progress-indicator click's toggle, the drag's select-and-copy — are not to be performed; the shipped gates ride `state.scroll` / `prompt_state.is_pending()` / `state.mouse_select` (`src/app.rs:3433` the wheel, `:3477` the click, `:3544` the drag) and the running-turn marker the gates read is the `turn_in_flight` flag the FR-49 cycle carried (`Arc<AtomicBool>`, `src/app.rs:139`/`:335`/`:422`, set at the turn's own edges only; the input loop reads `state.turn_in_flight` at `:3688`/`:3721`/`:3984`). The mechanism (per-gate turn-in-flight bars versus the session capture toggled off at the turn's edges) is recorded at the requirements' AS-88 and the design's D-91. The second section, 「config.tomlの内容変更」, carries the two rename clauses — 「codex-codeはcodex-cliへ変更」 and 「claude-codeはclaude-cliへ変更」 (FR-51a): the backend kind strings `codex-code`/`claude-code` are renamed `codex-cli`/`claude-cli` across the configuration surface — the `[provider.codex-code]`/`[provider.claude-code]` tables, the `kind =` values, the deserializer's rename arms, `--provider`, the diagnostics face, and the docs/example rows the rename un-truths (the requirements' BL-M62 surface record, AS-90's span, the design's D-92).

**The first ask stands repaired and live-verified (D-93 → D-94, §13; BL-M66)**: the multi-angle analysis, the implementation at the three gates, the pin and the CHANGELOG entry stood at the tree from the first execution session (T-128 🟨, T-129 🟨), with the gates first read as an **enabler** (BL-M64, D-93) — and the follow-on execution session delivered the polarity repair (T-135 ✅): the helper `the_mouse_operation_is_prohibited` (`src/app.rs:1449`) is read **negated** at all three gates (the wheel `:3455`, the click `:3506`, the drag `:3559`; the pin `:8699` stating the gate-side conjunction), so the arms run at rest and stand mute mid-turn. The delivery session's live battery (NFR-15) proved the ask's faces: the mid-turn wheel/click/drag perform nothing (no view move, no marker, no `[clip]` report, no toggle) and the at-rest contracts stand (the wheel scrolls, a bare click writes nothing, the prompt returns); the at-rest copy face stands on the suite pins and the FR-48 record (the tool-layer injection boundary recorded in the session's log).

**The second ask stands delivered in full (the kinds, the docs, the guides, the example; T-132 ✅)**: FR-51a's rename surface ran at the tree (the kind values, the tables, the serde arms, `--provider`, the factory's arms and `SUPPORTED`, the diagnostics — zero old-kind strings in `src/`); FR-51b's doc face ran beside it and the delivery session's sweep cleared the guides' 13 stale rows and the workspace `.aiprj/config.toml`'s two comment rows, so every shipped face names the new kinds. The Rust identifiers and the module file names stand (D-92's span), and the transcript drop file rides the renamed surface (`.agent-cli-codex-cli-last.txt`). The completion (the commit, the push, the tag, the release) rides T-130/T-133/T-134.

The FR-48 click/zero-touch fixes and the FR-49 handoff fixes stand as they were delivered: a no-op gesture writes nothing at all (the FR-48 delivered face, standing), and after a turn's answer the prompt returns by the shipped flow's own hands every time (FR-49's delivered face, standing — T-122's fixes stand shipped).

## The cycle's tasks

| ID | Task | Requirements | Design | Status |
|---|---|---|---|---|
| T-128 | The multi-angle analysis and the live mechanism evidence for the pending FR-50 cycle (FR-50a; C-12, NFR-11/NFR-15): the running-turn edge the gates read — `prompt_state.is_pending()` at the click gate `:3471`, the wheel gate's own condition `state.scroll` only `:3427`, the drag gate `:3540–3596`, the turn-in-flight flag the FR-49 work carried (`Arc<AtomicBool>` `:139`/`:335`/`:422`, its `watch` mirror `:447`/`:561–562`, read in the input loop at `:3688`/`:3721`/`:3984`); the pinned record rides the design's §9 and the work log. — **In progress**: the mechanism decided (D-91 per-gate turn-in-flight bars — the analysis verdict recorded; the dispatch face: the registered peers stood unreachable, the verdict on file/live evidence per the standing record); the live probe battery (NFR-15) rode the implementation's session (its verdicts §13: the mid-turn faces barred, the at-rest wheel face verified, the drag's copy face resting on the code identity — superseded by D-93's verdict). | FR-50a, NFR-11, NFR-15, C-12 | design §9 (D-91) | 🟨 In progress (the delivered implementation's polarity repair executed at the tree — T-135 executed per D-94/BL-M66, its live corroboration unwritten) |
| T-129 | The prohibition's implementation at its evidenced sites (FR-50a; D-91): the three mouse gates joined with the turn-in-flight bar — the wheel gate, the click gate and the drag gate each gaining `!state.turn_in_flight.load(Ordering::SeqCst)` (a mouse event mid-turn performs nothing: no view move, no toggle, no selection/copy/report); the shipped zero-touch contract (FR-48's delivered face: a no-op gesture writes nothing at all) and the FR-49 prompt-return contract stand intact; the boundary (§3.1, §6, NFR-14) holds; the fix's own pin (NFR-13) joins the gates and the live probe (wheel/click/drag mid-turn performing nothing, the same faces at rest standing) re-derives the delivered faces. | FR-50a, NFR-13, NFR-14, NFR-15 | design §9 (D-91, D-93) | 🟨 In progress (the implementation delivered; its polarity repair executed at the tree by the follow-on session — D-94/BL-M66; the repaired faces' live corroboration rides T-135) |
| T-130 | The completion (FR-50b, FR-50c; C-14): the CHANGELOG's `[0.49.0]` entry recording the prohibition in user language (the `[Unreleased]` heading standing above it); the README/`doc/usage.md` rows whose shipped wording names the mouse's mid-turn behavior updated where the change makes them untrue (the prohibition is user-visible — the docs face FR-50b names: the README mouse rows `:26–:27`/`:548–:549`, the usage rows `:359`/`:405`/`:417–:434`); the example check (the requirements' §3.1 — no config surface the mechanism adds); the version carry **0.48.0 → 0.49.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading), the delivery commit pushed to `origin/main`, the tag `v0.49.0`, the release standing the **latest** version (the repo's own workflow face, per C-17's delivered shape). | FR-50b, FR-50c, NFR-11, C-14, C-15 | design §3 (the docs angle), §4 (W-73) | ⬜ Not started |
| T-131 | The ledgers' consistency and the work log at every update pass (C-08, NFR-12, C-15/C-16): the three documents aligned together (this pass's rewrite recorded in this row, the requirements' §13's pass row, and the design's §1/§9), the work logs written per Art. 6/7 (never overwritten) with the instruction embedded, the instruction byte-exact re-verified at each exit, no document carrying any mark of authorship or tooling (Art. 8, NFR-12). | — (C-08, NFR-12, C-15, C-16) | design §1–§4, §9 | ✅ (this pass) |
| T-132 | The FR-51 rename's implementation and documentation (FR-51a, FR-51b; D-92, BL-M62's surface): the kind strings `codex-code`→`codex-cli` and `claude-code`→`claude-cli` renamed across the configuration surface — the `[provider]` `kind` values the parser accepts, the table keys `[provider.codex-code]`/`[provider.claude-code]` and their serde rename arms, the `--provider` accepted values and its help row, the provider factory's arms, the `for_provider_section`/ensure lookups, the error prefixes, banner rows and diagnostics the evidence binds, and the doc/example face the rename un-truths (the README's and README_ja's backend rows, the usage row, the per-backend guides; the workspace's own `.aiprj/config.toml` comment row and its `[provider.claude-code]` comment header ride FR-51b's doc sweep (T-136)); the shipped pins re-derive and the suite/clippy gates re-derive at the changed tree; AS-90's span decided and recorded (D-92's execution span: the user-facing kind strings carried; the Rust identifiers and the module file names standing). — **In progress**: the implementation and the doc rename executed at the tree by the execution session (the `src/` tree zeroing the old kind strings — BL-M63/BL-M65). | FR-51a, FR-51b, NFR-11, NFR-13 | design §10 (D-92), §13 (the pass's alignment row) | ✅ Done (FR-51a delivered — zero old-kind strings in `src/`; FR-51b's 13 guide rows cleared and the doc/example faces zeroed by the delivery session's sweep, T-136)
| T-133 | The FR-51 completion (FR-51c; C-14): the version carry per the delivery sentence's shape — the successor minor of whatever FR-50's delivery carried (0.50.0 following 0.49.0; both cycles sequenced per the tasks' rows) — with the CHANGELOG heading, the delivery commit pushed to `origin/main`, the tag, and the release standing the **latest** version. — the carry landed at the tree inside the FR-50's own 0.49.0 carry (the both-cycles-in-one-carry shape FR-51c's row allowed; the CHANGELOG's one `## [0.49.0]` section holding both entries). | FR-51c, C-14 | design §4 (W-74) | ⬜ Not started (the version carried 0.48.0 → 0.49.0 at the tree; the commit, the push and the release ride the delivery session) |
| T-134 | The FR-51 gates and delivery (C-09, C-14; design W-74): the suite and clippy re-derived at the changed tree, the renamed kinds verified live (the example config parsed, `--provider codex-cli`/`claude-cli` resolving, the banner/diagnostics naming the new kinds), the release published standing latest per FR-51c's carry. | FR-51c, C-09, C-14 | design §4 (W-74) | ⬜ Not started |
| T-135 | The polarity repair (FR-50a; the requirements §13, D-93 → executed per D-94/BL-M66): the three mouse gates — the wheel, the click and the drag — re-read the prohibition as a bar (the gate predicates negated, or the helper re-decided), so the arms run at rest and stand mute mid-turn; the at-rest shipped contracts re-derived (the FR-39 wheel face, the FR-48 zero-touch face, the drag’s copy), the mid-turn faces re-probed live (NFR-15), and the suite/clippy re-derived green at the changed tree (C-09). — **Executed and live-verified** (the repair written at the tree 11:05:56 +0900; the live battery run by the delivery session of ~11:44–12:10 against spinner-proven turns through the byte-verified tmux SGR transport — crossterm's own parser proven over the exact frames by a standalone test): the mid-turn faces all stand barred (the wheel notches move no view and raise no `Message below` marker; the drag press/motion/release produces no `[clip]` report and no selection face; the click toggles nothing mid-turn) — the repair's own ask, delivered; the at-rest controls re-derived (the wheel notches move the view; a bare press-and-release writes nothing at all — the FR-48 zero-touch face; the prompt returns after every turn). The one face the live rig could not corroborate is the at-rest drag's **copy** (the tmux injection transport delivered press and release but never produced a selection extension on screen — the tool-layer boundary recorded in the session's work log, not a product defect: the shipped pins `a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt` and the `selected_text` unit tests stand green, and the FR-48 delivery's own live battery verified the copy face at `f9ec9ae`). | FR-50a, NFR-13, NFR-14, NFR-15 | design §9 (D-93, D-94) | ✅ Done (the repair executed and its live battery run; the at-rest copy face standing on the suite pins and the FR-48 record) |
| T-136 | The FR-50b/FR-51b documentation sweep (FR-50b, FR-51b; BL-M65): the README’s and `doc/usage.md`’s mouse rows whose shipped wording names the mouse’s mid-turn behavior updated where the prohibition makes them untrue (the README wheel/drag rows `:26`/`:27`, the usage rows `:357`/`:420`/`:429`); the two renamed guides’ thirteen stale old-kind rows cleared (`doc/providers/codex-cli.md` `:29`, `:31`, `:108`, `:117`, `:118`, `:142`, `:143`; `doc/providers/claude-cli.md` `:27`, `:29`, `:108`, `:109`, `:129`, `:131`) with the workspace’s own `.aiprj/config.toml` comment rows (`:12`, `:30`) treated by their owner’s hand; every statement re-bound to the code it names. — **Delivered by the delivery session** (~11:44–12:10): the two guides' thirteen rows (the `kind =` blocks ×2, the `[provider.…]` table keys, the doctor/selftest rows, the error-table rows, and the codex guide's cross-reference row) rewritten to the new kinds — `grep`-clean, zero old-kind strings in any shipped face; the README_**ja**'s four mouse rows (`:26`/`:27`, the keyboard table's wheel/drag rows `:548–:549`) updated alongside the README/usage rows the prohibition un-truths; the workspace's own `.aiprj/config.toml` comment rows `:12`/`:30` renamed (the sweep's recorded judgement taken). | FR-50b, FR-51b, NFR-11 | design §3 (the docs angle) | ✅ Done (the docs face delivered; the guides' residual rows cleared) |

## The closed tasks of the delivered cycles (their records stand)

- **T-108/T-109/T-110** (the FR-47 one-shot mode, the completion, the
  ledger's consistency) — ✅, closed 2026-11-06: the delivery `77e8a29`
  ("Add the one-shot subcommand: one prompt in, the printed answer out,
  then exit"), the version carried **0.45.0 → 0.46.0**, the tag `v0.46.0`,
  the release verified Latest (created 2026-11-06T01:16:40Z, published
  01:16:50Z); gates **663 unit + 6 integration, 0 failed**; the trail's
  follow-up `1b3c7e1`, the session's exit log `8c26ecc`. The
  requirements' §12 and the design's §5 carry the full record.
- **T-105/T-106/T-107** (the FR-46 terminal display fix, the completion,
  the ledger passes) — ✅, closed 2026-11-06: delivered at `ebe0aca`
  ("Fix the terminal's bottom line…"), the version carried to **0.45.0**,
  the tag `v0.45.0`, the release verified Latest; gates **663 unit +
  6 integration, 0 failed**; the recording/re-load passes
  `2557f7f`/`92c43fd` and the ledger follow-up `5978dc9`. Work logs
  `2026-11-06_005.md`–`_007.md`.
- **T-102/T-103/T-104** — ✅, closed 2026-11-06: FR-45 delivered at
  `b5deb9c`, v0.44.0 Latest; the follow-ups `6f0390f`/`0d45242`. Work
  logs `_002`–`_004`.
- **T-95/T-96** (the FR-43 investigation and its ledger record) — ✅,
  closed 2026-11-05 (no release; work log `_008` of 2026-11-05).
- **T-97/T-98/T-99/T-100/T-101** — ✅, closed 2026-11-05: FR-44
  delivered at `122f320`/`4fee899`, versions 0.42.0/0.43.0, the release
  **v0.43.0 verified latest**; gates 669+6.

## Cross-cutting gates (C-09) — standing

- The suite at the working tree (the FR-50/FR-51 changes standing, the polarity repair executed): **668 unit + 6 integration, 0 failed**, re-derived live at the repaired tree by the 11:35 re-load pass; clippy standing **five** notices re-derived fresh at the repaired 0.49.0 tree (`src/app.rs:978` empty-line-after-doc-comment, `:2412` sort_by_key, `:8520` and `:8801` dead stores, `src/agent.rs:2101` collapsible_match) under the tree's clippy 0.1.94 — the repair's doc-comment rewrite shifted the app.rs sites, and the 10:42 pass's three-notice re-basing (`:8517`/`:8787`, the sort/collapsible rows withdrawn) does not reproduce at this tree (C-09's record re-based by this pass's fresh derivation).
- The FR-47 boundary stands: the one-shot process binds **no** socket
  (the closed record's D-81); the FR-46 boundary stands: the prompt/clock
  flip; the FR-48 click contract stands: a no-op gesture writes nothing
  at all; the FR-49 contract stands: the prompt returns after every
  turn's answer by the shipped flow's own hands.
- The `.aiprj` documents mutually consistent (C-08 — the 11:35 pass
  aligned the three documents to the polarity-repaired tree and
  resolved the status/attribution faces the comparison presented, as
  the 10:42 pass had aligned them to its own read before it); the work
  log written per Art. 6/7 (never overwritten); the instruction
  verified byte-exact at exit (515 bytes, sha256 `179b698a…`).

## The previous cycles' record

**"the answer-display prompt handoff" (FR-49 a/b/c/d)** — **closed,
2026-11-07**: delivered at `89f1ea0` ("Fix the answer-display prompt
handoff: the prompt returns after a turn's answer without a click or a
keystroke"), the version carried **0.47.0 → 0.48.0**, the tag `v0.48.0`
verified on the publication's fix commit `9040435` (peeled; the tag
object `592646e`), the release
`v0.48.0 — After the [answer] display finishes, the prompt is displayed
again — every time.` published standing the repo's **latest** release
(published 2026-11-07T00:20:48Z, verified at `/releases/latest`; the
body the `[0.48.0]` section's own Fixed block — the workflow's extraction
face verified end-to-end); the gates at the delivered tree **667 unit +
6 integration, 0 failed** (D-85/D-86/D-87's fixes + two new zone pins at
`src/app.rs`); the delivered faces verified live (the typed turn, the
tool-using turn, two back-to-back peer turns, the bare click's zero-touch
face intact, the banner row surviving). The publication mechanism
(D-88/D-89/D-90): the repo's own release workflow built at
`1d8ccc3`/`0a49302`/`40fc7c4` (its publish runs failing at exit 128 and
then exit 141 — the runner-log record), the exit-141/SIGPIPE mechanism
reproduced locally and fixed at `9040435`
("Fix the release workflow's SIGPIPE and restore the CHANGELOG's
`[0.47.0]` heading" — the extract reads to the end, the `[0.47.0]`
heading restored beneath `[0.48.0]`'s section), the tag `v0.48.0` moved
onto `9040435` (the release was unpublished, so the move was allowed) and
force-pushed, the workflow's run `42317643598` **success** — the release
published by the runner standing latest. The ledger follow-up `41327d7`
("Record the v0.48.0 release publication in the ledger: the trail's
follow-up") committed the ledgers and the work logs `_000`/`_001` and
stands **HEAD**. Work logs `2026-11-06_019.md` (the cycle's record),
`2026-11-07_000.md` (the delivery-session exit log),
`2026-11-07_001.md` (the 01:38 re-load pass's log),
`2026-11-07_002.md` (the publication-delivery exit log, 09:38).

**"the mouse-click display defects repaired" (FR-48 a/b/c/d)** — closed,
2026-11-06: delivered at `f9ec9ae` ("Fix the mouse-click display: a click
leaves every row where it stood and paints no prompt"), the version
carried 0.46.0 → **0.47.0**, the tag `v0.47.0` verified on it, the
release `v0.47.0 — the mouse-click display fixed` published and verified
**Latest** (published 2026-11-06T11:37:01Z); gates at the close **665
unit + 6 integration, 0 failed** (the pins
`a_bare_click_costs_the_screen_nothing` and
`a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt` among the
663 standing), clippy standing its three pre-existing notices; the live
click battery and the post-fix verdicts carried the fixed faces — a bare
click writes nothing at all, a real drag keeps its copy-and-return path,
a scrolled click leaves the pinned block face intact. Work logs
`2026-11-06_013.md` and `_015.md`.

**"the one-shot mode added" (FR-47 a/b/c)** — closed, 2026-11-06:
delivered at `77e8a29` ("Add the one-shot subcommand…"), the version
carried to **0.46.0**, the tag `v0.46.0`, the release `v0.46.0 — the
one-shot subcommand` published and verified Latest (created
2026-11-06T01:16:40Z, published 01:16:50Z); gates **663 unit + 6
integration, 0 failed**; the delivered records stand in the
requirements' §12 and the design's §5. Work logs `2026-11-06_008.md`
(the cycle's record) and `_009.md`/`_010.md` (the re-load passes).

**"the terminal's bottom-most line fixed" (FR-46 a/b/c)** — closed,
2026-11-06: delivered at `ebe0aca` ("Fix the terminal's bottom line…"),
the version carried to **0.45.0**, the tag `v0.45.0`, the release
verified Latest (created 2026-11-06T00:55:05Z); gates **663 unit + 6
integration, 0 failed**. Work logs `2026-11-06_005.md`–`_007.md`.

**"the prompt-input queueing feature's removal" (FR-45 a/b/c)** —
closed, 2026-11-06: delivered at `b5deb9c` (v0.44.0, the release
Latest); gates 663+6; the follow-ups `6f0390f`/`0d45242`. Work logs
`_002`–`_004`.

**"the codex-code backend…" (FR-44 a–e)** — closed, 2026-11-05: delivered
at `122f320` and `4fee899` (the FR-44d amendment), versions
0.42.0/0.43.0, the release **v0.43.0 verified latest**; gates **669 + 6**.

**"the codex access investigation" (FR-43)** — closed, 2026-11-05 (no
release — the investigation and report).

**"the refactoring…; the prompt's bottom row made prompt-only" (FR-41,
FR-42)** — closed, 2026-11-05: `eec12f9`/`064ce50`, **0.41.0**, the
follow-up `3badc04`; gates **647 + 6**.

**"the display regions…" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05:
delivered at `39afeb2`, named by the follow-up `fb46cc0`; gates 645+6.
FR-39/FR-40 closed at `db68d3f`/`306f425`, the version 0.40.0.

**"prompt input during a running turn…" (FR-35)** — closed, 2026-11-04
(`c02862d`, v0.38.0); the delivered feature was deleted by FR-45 (the
CHANGELOG's `[0.38.0]` section stands as history).

Version trail: … → `e8ad594` (v0.32.0) → … → `122f320` → `af82155` (named)
→ `4fee899` → `d5cbbbe` (named) → `b5deb9c` (v0.44.0) → `6f0390f` →
`0d45242` → `2557f7f` → `92c43fd` → `5978dc9` → `ebe0aca` (v0.45.0) →
`77e8a29` (the one-shot; **v0.46.0**) → `1b3c7e1` (the ledger follow-up
naming `77e8a29`) → `8c26ecc` (the exit log) → `f9ec9ae` (the FR-48
delivery; **v0.47.0**) → `e1b9118` (the ledger follow-up naming
`f9ec9ae`) → `7d8e621` (the verification-session trail follow-up) →
`315ee7f` (the CHANGELOG's `[Unreleased]` heading hoist) → `13d4356` →
`442bc19` → `0be0754` → `89f1ea0` (the FR-49 delivery, **v0.48.0**) →
`0efdc45` (the FR-49 ledger follow-up) → `1d8ccc3` → `0a49302` →
`40fc7c4` (the release workflow's commits) → `9040435` (the SIGPIPE fix +
the CHANGELOG heading restore; the tag `v0.48.0` moved onto it; the
release published standing **latest**) → `41327d7` (the ledger follow-up
naming the publication; **HEAD**, `main` ≡ `origin/main`, this pass's
ledger writes standing uncommitted for the trail's next follow-up). The
trail's follow-ups name the deliveries behind them (per the trail rule,
a delivery commit's sha is nameable only from the commit that follows it).
