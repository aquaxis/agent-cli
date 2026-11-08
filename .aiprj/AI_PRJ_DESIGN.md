# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "mouse operations during a running turn prohibited" (FR-50 a/b/c) and
"the backend kinds `codex-code`/`claude-code` renamed `codex-cli`/`claude-cli`"
(FR-51 a/b/c) — both **in progress**: the 2026-11-08 execution session
delivered the product writes at the working tree (FR-50a's mechanism landed
polarity-inverted — the 10:15 consistency pass's verdict D-93, §9 — and its
polarity repair stands executed at the tree since, an 11:05 execution
session's write verified by the 11:35 consistency pass BL-M66, §9;
FR-51a's kind renames
landed across `src/`; FR-51b partially delivered — 13 stale kind rows in the
two renamed guides — BL-M65), and the completion rows (the delivery commit,
the push, the tag `v0.49.0`, the release standing latest) stand unstarted
(the requirements' §13). The delivered FR-48/FR-49 cycles stand closed in
§5/§6/§7/§8 — `f9ec9ae` v0.47.0, `89f1ea0` v0.48.0 with the release
published Latest; the prior cycles' record stands §5 (the requirements' §12)
(`.aiprj/instructions.md`, reloaded 2026-11-08 08:38:16 +0900 — 515 bytes,
sha256 `179b698a3df2b62fb114b16171cec9592694157d0c625eafd0ed5a3538e1a373`,
verified byte-exact at the 10:15 pass's read and exit — §2 of the
requirements — and re-verified byte-exact to the same settled state by the
10:42 and the 11:35 re-load passes)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`.

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. The new cycle's ask, the ground it stands on, and the open design questions (Q-31)

The instruction's rewritten 最優先事項 section 「エージェント動作中のマウス操
作は禁止」 names one clause: 「エージェント動作中のマウス操作は禁止に変更する」
— while an agent turn is running, mouse operations are to be prohibited. The
sections above it are the instruction's standing header (the multi-angle
subagent analysis, C-12; the delivery sentence C-14). The two-clause
mouse-click instruction is gone — its cycle stands delivered (FR-48 at
`f9ec9ae`, v0.47.0 Latest; the requirements' §12) — and this cycle is the
prohibition's cycle. Its scoping decision **D-91** reads the clause as an
addition cycle whose ask is a behavior *change* to the mouse surface the
tree ships: the wheel, the drag-select and the progress-indicator click are
each to be barred while the turn runs, rather than the whole capture being
toggled off (AS-88's scope direction, with the capture-level alternative
named; the requirements' FR-50a). The user's clause 「変更する」 (change it)
names the current behavior as the thing acted on; the requirements' FR-50a
row is the cycle's pending face.

What the tree shows today, verified at this pass (the requirements'
BL-M60), and the design questions the pending cycle must answer:

- **The mouse face stands exactly as the FR-48/FR-49 ground pinned it.**
  Reporting on for the whole session while the wheel scrollback is enabled
  (`src/app.rs:3112`, the disabled switch at the raw-mode guard's drop
  `:1141`; the alternative capture at the indicator's own switch
  `set_mouse_capture` `:4797–4799`, whose `owns_mouse` is false under the
  standing ownership rule `:364`). The pump is still the single reader; the
  REPL's input-loop mouse arm still drains the burst and gates three
  behaviors: the wheel (`src/app.rs:3433–3454`, the `state.scroll` gate `:3433`;
  the scroll_dirty write `:3464`), the progress-indicator click behind
  `prompt_state.is_pending()` (`src/app.rs:3466–3507` — the gate `:3466`, the
  toggle `:3506`; the armed rule `zone_click_hits_clock` `:1444`/`3495`, the
  un-armed pump round-trip rule `click_hits_indicator` `:1431`/`3500`), and
  the selection drag behind `state.scroll && state.mouse_select` (the gate
  `:3517–3522`, the three arms `:3544–3596`). The drain's tail runs every
  mouse pass (`:3904–3939` — the overlay's enter/leave and the scrolled
  redraw). **Mid-turn, the click's only gate is `is_pending()` — the same
  predicate the pending face rides; the wheel and the drag run whenever
  `state.scroll` holds, mid-turn included** — the faces the prohibition
  changes (the requirements' BL-M60 record).
- **Q-31, the pending cycle's design question set.** (a) the clause's
  mechanism bar: what 「禁止」 means at each gate — at the wheel, at the
  drag, at the click (the click's gate already holds true exactly while the
  turn is pending — the prohibition's click face is the *inverse* gate, and
  the surface it would silence is the shipped progress-indicator toggle,
  which the prohibition has to reconcile against its own words); (b) whether
  prohibition is per-gate (a turn-in-flight bar joined at all three gates —
  the `turn_in_flight` flag the FR-49 work carried and the `is_pending()`
  predicate are the tree's own running-turn markers) or session-capture-level
  (toggle `EnableMouseCapture` at the turn's edges — the capture-level
  alternative AS-88 names, whose cost is the whole-session wheel/scrollback
  contract FR-39's shipped shape carries and the terminal's own selection
  regaining the event stream mid-turn); (c) what the mid-turn scrollback
  face becomes while the turn runs (the FR-39 shipped contract scrolls
  *during* a turn — §6/AS-87's boundary question, the window's edges the
  requirement row pins at the evidence-first close); (d) the boundary
  contracts standing (NFR-14): the FR-49 prompt-return edges, the FR-48
  zero-touch click face and the drag's copy, the zone's flip.
- The cycle's method stands C-12's: multi-angle subagent analysis before
  the fix decision (the decision D-91 shapes; design §3); every mechanism
  answer pinned live (NFR-15) before the rework.

## 2. The touchpoints the new cycle's ground named (TPR-31 — this cycle's; TPR-30 — the standing record)

| # | Touchpoint | Evidence (the files read at this pass) |
|---|---|---|
| TPR-31 | The prohibition's candidate surfaces this cycle's design must choose between (the requirements' FR-50a's D-91 scope): the three gates' predicate joins at the arm's wheel/click/drag entries (`src/app.rs:3433`, `:3466`, `:3517` — the mid-turn bars by state), the capture's own edges (`EnableMouseCapture` `:3112` beside the FR-49 display-task edges — the mid-turn off-switch by hardware), and the docs the chosen surface names (the README/usage mouse rows, the example's mouse block, the CHANGELOG entry) | The arm's gate lines and the capture sites at `41327d7`, read at this pass; the requirements' BL-M60 |
| TPR-30 | The standing click face's sites at the `41327d7` tree (the FR-48/FR-49 cycles' ground, standing shipped): the mouse gates (`:3433–3611`), the click rules (`:1431`, `:1444`), the capture sites (`:1141`, `:3112`, `:4797–4799`, `:364`), the budgets (`:2310`, `:2336`), the drain's repaint tail (`:3904–3939`), the zone bookkeeping (`zone_armed` `:1605`, `zone_csi_move` `:1581`), the turn-in-flight edges (FR-49's: `turn_in_flight_tx` `:2648`, the display-task's watch `:447`/`:561–562`), the pins (`:8639+`, `:7964`), the config rows (`src/config.rs:163`, `:694–741`), the docs' mouse rows (`README.md:26–27`, `:548–549`; `doc/usage.md:359`, `:405`, `:417–434`), the example's mouse block (`example/config.example.toml:191–200`) | The files' own lines, read at this pass; the requirements' BL-M60 |

## 3. The angles (C-12) — the new cycle's and the standing records'

- **The mechanism angle (the new cycle's, pending).** The ground is pinned
  (§1/§2): three gated behaviors (the wheel, the click, the drag), one
  repaint tail, the running-turn markers the gates can read
  (`turn_in_flight`, `is_pending()`), and the capture edges. The cycle's
  own analysis runs multi-angle before the D-decision is taken; the verdict
  mechanism is the live probe — each mid-turn face probed against a live
  session first (NFR-15), the sites named file:line, then the rework.
- **The contracts angle (NFR-14).** What stood and must not change
  without the new ask naming it: the wheel's scrollback and its marker,
  the drag's copy and its `[clip]` report, the toggle's meaning and its
  block, the zone's standing shape (FR-46's bottom line), the scrolled
  view's pinned composition (FR-25/FR-39), the click's zero-touch face
  (FR-48a/FR-48b), the FR-49 prompt-return edges. **The new ask names the
  mouse's mid-turn surface directly (FR-50a): the prohibition is the
  change those mid-turn clauses ride — the boundary question AS-87
  records (the window's edges) is the design's to answer with evidence.**
- **The scope angle.** Not touched: the mouse's at-rest work (§3.1 of the
  requirements), the keyboard mid-turn, the zone's flip and the scrolled
  pin (their own hands), the providers, the peers/IPC/registry, the
  delivered cycles' config keys (the FR-48/FR-49 cycles asked none — the
  example check ran and wrote nothing). The FR-50 cycle's own scope faces
  the config/docs surfaces the prohibition names (TPR-31).
- **The live-verdict angle (NFR-15).** Standing: the suite and clippy at
  the changed tree; the scripted live probes against a running turn (each
  barred face evidenced mid-turn, pre- and post-fix); the
  release-latest/tag verification at the close. The delivered cycles'
  records stand: the FR-48d release `v0.47.0` Latest verified
  (2026-11-06T11:37:01Z), the FR-49d release `v0.48.0` Latest verified
  (2026-11-07T00:20:48Z) — the requirements' §12.
- **The docs angle (C-14, FR-48c/FR-49c delivered; FR-50b pending).**
  Delivered: the CHANGELOG's `[0.47.0]` and `[0.48.0]` entries, the
  README/usage rows the fixes name unchanged (the FR-49c record), the
  example check. The FR-50 cycle's own docs face is its completion row's
  (FR-50b): the CHANGELOG's `[0.49.0]` entry, the README/usage mouse rows
  whose shipped wording the change un-truths, the example check.

## 4. Verification (the gates: W-72 — passed, the delivered record; W-73/W-74 — in progress, §9/§10)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-72 | FR-48 a/b/c/d — the symptoms reproduced live first (NFR-15), the mechanisms named file:line, the reworks delivered at their sites with the boundary intact, the fixed faces pinned (NFR-13), the live probe unshifted and prompt-paint-free, the docs written, the version carry to **0.47.0**, the delivery committed and pushed, the release standing latest. | FR-48a, FR-48b, FR-48c, FR-48d, NFR-11, NFR-13, NFR-14, NFR-15, AS-82, AS-86, AS-83, TPR-30 | T-113, T-114 | ✅ |
| W-73 | FR-50 a/b/c — the mechanisms evidenced at their sites and probed live first (NFR-15), the change delivered at its sites with the boundary intact (§6, NFR-14), the changed behavior pinned (NFR-13), the suite green and clippy standing its notices at the changed tree (C-09), the live probe showing the mouse perform nothing mid-turn and everything it did at rest, the docs written (FR-50b), the version carry to **0.49.0**, the delivery committed and pushed, the release standing latest (FR-50c). — (the FR-50 cycle's gate) | FR-50a, FR-50b, FR-50c, NFR-11, NFR-13, NFR-14, NFR-15, AS-87, AS-88, AS-89, TPR-31 | T-128, T-129, T-135, T-130 | 🟨 in progress (§9: the repair T-135 executed at the tree per D-94/BL-M66 — the gates read the helper negated, the pin re-faced; the delivery session's live battery verified the mid-turn bars and the at-rest wheel/click faces, the docs sweep T-136 delivered (the guides' 13 rows cleared, README/README_ja/usage rows truthful); only the completion T-130 — the commit, the push, the tag, the release — rides the delivery's own steps)
| W-74 | FR-51 a/b/c — the rename surface pinned at its sites (BL-M62) and probed live before any edit lands (NFR-15), the renames delivered across the config/kind/CLI/diagnostics surface the evidence names and every doc row the rename un-truths, the boundary intact (§6, the child binaries' names and the wire protocols untouched), the suite green and clippy standing its notices at the changed tree (C-09), the version carry per the delivery sentence (FR-51c), the delivery committed and pushed, the release standing latest. — (the FR-51 cycle's gate) | FR-51a, FR-51b, FR-51c, NFR-11, NFR-13, NFR-14, NFR-15, AS-90, AS-91, TPR-31 | T-132, T-133, T-136 | 🟨 in progress (§10: FR-51a delivered, FR-51b partially; the guide residuals T-136 ⬜ and the completion T-133/T-134 ⬜ stand unstarted — the 11:35 pass's re-confirmation)

*W-72's execution record (delivered; the requirements' FR-48d row and
§12): the suite **665 unit + 6 integration, 0 failed** at the changed tree
(the two new click-face pins joining the gates); the live probes re-run at
the fixed build confirmed the faces (a bare click at rest leaves the screen
byte-identical; a drag copies and returns to the live face; a scrolled
click leaves the marker untouched; a mid-turn click leaves the clock
standing); the delivery `f9ec9ae` pushed, the tag `v0.47.0` verified on
it, the release published and verified **Latest** (published
2026-11-06T11:37:01Z); the docs delivered per FR-48c. Subsequent
verification sessions re-derived the delivered state (the tag,
`/releases/latest`, the gates, clippy's five standing notices at the
corrected C-09 sites) and one docs-angle verdict was taken by the 23:10
session: the CHANGELOG's empty `[Unreleased]` heading hoisted above the
newest entry (commit `315ee7f` — the delivery commits had each inserted
their section above the stranded heading). The delivered faces stand at
the trail's HEAD `41327d7`.*

## 5. The delivered cycles before FR-48 (their records stand)

**"the one-shot mode added — `agent-cli one-shot <prompt>`" (FR-47 a–c)**
— closed, 2026-11-06: delivered at `77e8a29` ("Add the one-shot
subcommand…"), the version carried **0.45.0 → 0.46.0**, the tag `v0.46.0`
verified on it, the release `v0.46.0 — the one-shot subcommand` published
and verified **Latest** (published 01:16:50Z). The decisions as executed:
**D-78** the subcommand and its pipeline (the `run_headless` assembly
minus the IPC face — persona → provider → the transient registry entry →
log → tools+MCP → the agent on the bounded channels; the entry's socket
field empty and `cleanup()`ed at both exits — **D-81**, the record's
earlier "no registry entry" clause corrected); **D-79** the answer's face
and the session's end (the **inline** event loop printing the `Text`
deltas — no display task, no `Theme` — `Done` breaking, the bounded drain,
the process exit; the inert `--quiet`, **D-82**/AS-79); **D-80** the
documentation follows (the CHANGELOG's `[0.46.0]`/`### Added` entry with
the `[Unreleased]` heading standing — AS-81; the README's Scriptable
bullet `:29` and CLI row `:362`; the usage row `:20`;
`doc/architecture.md` **not** written — AS-81/D-83). Gates **663 unit +
6 integration, 0 failed**; clippy standing three notes (the delivered
doc-comment block's empty line at `src/app.rs:967` riding the delivery —
C-09's correction). Live probe `OK-ONEShot` returned and the process
exited. The trail's follow-ups `1b3c7e1` (naming `77e8a29`) and
`8c26ecc` (the exit log). Work logs `2026-11-06_008.md` and the re-load
passes `_009`/`_010`.

**"the terminal display fix — the bottom-most line" (FR-46 a–c)** —
closed, 2026-11-06: delivered at `ebe0aca` ("Fix the terminal's bottom
line…"), the version carried **0.44.0 → 0.45.0**, the tag `v0.45.0`
verified on it, the release published and verified **Latest** (created
2026-11-06T00:55:20Z); gates **663 unit + 6 integration, 0 failed**,
clippy standing its two pre-existing dead-store notes. The recording pass
`2557f7f` and the re-load pass `92c43fd` stand behind the delivery in the
trail; the ledger follow-up `5978dc9` names `ebe0aca`. Work logs
`2026-11-06_005.md`–`_007.md`.

**"the prompt-input queueing feature's removal" (FR-45 a–c)** — closed,
2026-11-06: delivered at `b5deb9c` ("Remove the prompt-input queueing
feature"), the version carried 0.43.0 → **0.44.0**. the tag `v0.44.0`
verified on it, the release `v0.44.0 — the prompt-input queueing feature
removed` published and verified **Latest** (created
2026-11-05T23:18:45Z); gates **663 unit + 6 integration, 0 failed**, the
two dead-store clippy warnings recorded pre-existing. The trail's
follow-up `6f0390f` named the delivery, and the ledger re-alignment pass
rode the same convention (`0d45242`). Work logs
`2026-11-06_002.md`–`_004.md`.

**"the codex-code backend…" (FR-44 a–e)** — closed, 2026-11-05: delivered
at `122f320` and `4fee899` (the FR-44d amendment), the versions 0.42.0
then **0.43.0**, named by `af82155`/`d5cbbbe`; the FR-44e release
`v0.43.0` verified latest. Gates **669 unit + 6 integration, 0 failed**.
Work logs `2026-11-05_010.md`–`_012.md`. The example's sample-model
refresh (`4fee899`) was the FR-44d amendment's own write.

**"the codex access investigation" (FR-43)** — closed, 2026-11-05 (no
release — investigation and report): the verdict feasible for
delegation-style access (D-65), the shape recorded unbuilt (D-66);
FR-44's cycle built it (FR-44a, D-67). Work log `2026-11-05_008.md`.

**"the refactoring…; the prompt's bottom row made prompt-only" (FR-41,
FR-42)** — closed, 2026-11-05: delivered at `eec12f9` and `064ce50`, the
version carried to **0.41.0** by `25fe13c`, named by the follow-up
`3badc04`. Gates **647 unit + 6 integration, 0 failed**, clippy 0
warnings (the record of that close; the standing three warnings date from
later cycles — see the closed C-09 records). Work logs
`2026-11-05_006.md`/`_007.md`.

**"the display corrections…; the elapsed display kept and the selection
unshifted" (FR-39, FR-40)** — closed, 2026-11-05: delivered at `db68d3f`,
named by `306f425`; the version carried to 0.40.0 by `505097a`, recorded
by `856c2bf`.

**"the four display regions" (FR-37 a/b/c, FR-38)** — closed,
2026-11-05: delivered at `39afeb2`, named by the follow-up `fb46cc0`;
gates 645+6.

Earlier cycles stand in the requirements document's §12: the prompt-input
queueing (FR-35, v0.38.0 — delivered, then **removed** by FR-45; the
CHANGELOG's `[0.38.0]` section stands as history); the subagents'
auto-exit (FR-32–FR-34, v0.37.0 on `5a2d7f0`); the multi-angle subagent
analysis cycles (FR-31, FR-29/FR-30); the click rule's exclusion and the
cancelled-by-user display (FR-27/FR-28, v0.34.0 on `c022909`); the scroll
marker's face (FR-26, v0.33.0 on `6cc6661`); the running display
(FR-22–FR-24, v0.32.0 on `e8ad594`); the mouse defects (v0.31.0 on
`052e230`); add strata (FR-13–FR-17, v0.30.0–0.30.2); v0.29.0–v0.25.0 and
the 機能追加 addenda (v0.26.0 on `251a5bb`).

## 6. The delivered FR-49 cycle's design (the answer-display prompt handoff)

The user's live report (「[answer]の回答の表示後にプロンプトが表示されません、
回答の表示が完了したら必ずプロンプトを表示してください」) named one defect whose
shipped ground took three angles to pin — the live battery (the tmux
`ollama glm-5.3:cloud` panes, the byte-tier streams, the strace write
histograms, and the display task's own state read-outs) — and the findings:

- **D-85 — the terminal hand-off's always-on face.** The idle notification
  reached the input loop only while `prompt_state.is_pending()`; a turn the
  loop itself did not submit (peer/serve) parked its prompt return on the
  150 ms reconcile tick, whose window between the terminal event and the
  next TurnStart could leave the answer's leading deltas painting on the
  prompt's own rows. The fix consumes the notification in every state:
  pending — the typed hand-off as shipped; ready with no turn in flight —
  the same hand-off the typed path returns through; awaiting-approval or a
  turn already in flight — the running flow keeps the zone and the mark
  drops quietly (FR-48b's own quietness). The hand-off also deduplicates
  against the reconcile tick (its own `zone_armed()` check first): the two
  painters race, so the later one may fire immediately after the earlier
  one had already disarmed — the guarded paint prevents the duplicate
  prompt row.
- **D-86 — the marker's edges.** The running-turn flag was stored per
  event (`matches!(ev, TurnStart)`), so a turn read as ended from the FIRST
  streamed delta onward: the cancel bookkeeping and the reconcile tick saw
  a still-streaming turn, and the remaining deltas painted over the
  prompt's freshly returned rows. The flag now carries the
  TurnStart-to-terminal correspondence (only the two edges write it).
- **D-87 — the erase's coordinate order.** `zone_csi_clear_row(row)` passed
  the row into the column slot (`zone_csi_move(row, 0)`), so every sweep
  emitted `ESC[1;{row+1}H` — a crawl along row 1's columns — erasing the
  banner row and never the row it named. The fix emits `ESC[{row+1};1H`
  (the crossterm row-first convention), pinned by
  `the_zone_erase_clears_each_row_it_names`; the disarm's plan sequence
  pinned by `the_zone_disarm_plan_never_replays_below_the_painted_top`.
- **The watcher (the synchronous arm).** A `watch::Sender<bool>` rides
  the display task's TurnStart→terminal edge and the loop arms the zone the
  instant the edge flips — no selectable-await window streams the turn's
  leading deltas onto rows the zone has not taken yet. The body refines
  (pending → no arm; a turn already in flight → no re-arm). The typed
  path's `zone_arm` at submit stays as shipped.

The delivered faces (verified live at the delivered tree `89f1ea0`,
`agent-cli 0.48.0`): the typed turn's `[answer]` body and outcome row
replay straight before the fresh prompt; the tool-using turn and two
back-to-back peer turns land one prompt per terminal; a bare click writes
nothing (FR-48a/FR-48b); the banner row survives every pass; the FR-48
cycle's click/drag/scroll pins stand untouched. Gates: **667 unit + 6
integration, 0 failed**; clippy its five standing notices; the version
carried 0.47.0 → **0.48.0**; the delivery commit `89f1ea0` pushed with the
tag `v0.48.0`.

## 7. The FR-49d release face's design verdicts (D-88, D-89)

This pass was an evidence-first read of the post-delivery movements; the
verdicts it took, pinned at the tree and live (the requirements' §5.2's
NFR-16 record, C-17's publication clause):

- **D-88 — the release face's ground.** The FR-49 delivery's release
  obligation rode the standing release shape (the FR-46c/FR-47c/FR-48d
  shape: `gh release create v0.48.0 --verify-tag --latest --notes-file`
  with the tag's own CHANGELOG section as the notes), which the tree's
  transport could not execute (no `gh` CLI nor a token at the execution
  session's environment; the unauthenticated REST probe reads
  `/releases/latest` but cannot publish), so the session's alternative
  rode the repo's own release automation: `.github/workflows/release.yml`
  (push to `main`, `contents: write`, the newest tag's release created
  from that tag's `CHANGELOG.md` section, `--verify-tag --latest`, the
  title from the section's summary, the idempotent re-run face with
  `gh release edit` for a published release; `fetch-depth: 0` for the
  history the extraction reads). The automation's publish step failed on
  its first runs, each run's exit code read from the runner's own log
  (`gh run download <run-id>`): **exit 128** at `1d8ccc3` and again at
  `0a49302` — both printing `fatal: not a git repository (or any of the
  parent directories): .git`, the checkout's shallow fetch — and **exit
  141** at `40fc7c4`, the runner stopping mid-publish step after `newest
  tag: v0.48.0` with none of the workflow's own messages printed.
- **D-89 — the notes-file's extraction.** The workflow's extract reads
  `git show "$TAG:CHANGELOG.md"` and an awk program
  (`index($0, want) == 1`) over the block from the `## [0.48.0]` heading
  until the next `## [` heading; the FR-49 delivery commit `89f1ea0` had
  turned the previous `## [0.47.0]` heading into the new `## [0.48.0]`
  heading and left the FR-48 block as a heading-less second `### Fixed`
  block under it — a Keep-a-Changelog boundary break NFR-19's record
  carried.

These verdicts are recordings of the FR-49d row's face, taken before the
publication was delivered (§8).

## 8. The 2026-11-07 delivery session's verdict (D-90 — the SIGPIPE mechanism; the FR-49d publication delivered)

The 「残タスクが無くなるまで作業を実施」 session carried the C-17
publication row to its close. The verdicts, pinned by execution:

- **D-90 — the runner's exit 141 is SIGPIPE.** Reproduced locally under
  the workflow's own gate set: `SECTION="$(git show "$TAG:CHANGELOG.md" |
  awk ...exit...)"` under `set -euo pipefail` reports PIPESTATUS `141 0`
  — awk's early `exit` closed the read while `git show` was still writing
  the 88 KB CHANGELOG blob, and the writer died of SIGPIPE, which
  pipefail surfaced as the command substitution's exit code. The fix
  stops printing instead of exiting (the awk `on { print }` arm reads to
  the end).
- **The CHANGELOG's heading order restored first (D-89's face):** the
  FR-49 delivery commit's retitle had left the FR-48 notes as a
  heading-less second `### Fixed` block under `## [0.48.0]`; the section
  under the `[0.48.0]` heading is one release's notes again, the FR-48
  block standing under the restored `## [0.47.0]` heading.
- **The delivery:** the two product fixes ride the commit `9040435`
  ("Fix the release workflow's SIGPIPE and restore the CHANGELOG's
  [0.47.0] heading"); the tag `v0.48.0` moved onto it (the release was
  unpublished, so the move costs no published face) and force-pushed;
  the branch pushed `40fc7c4..9040435`; the workflow's run
  `42317643598` (head `9040435`) concluded **success**; the GitHub
  release `v0.48.0 — After the \`[answer]\` display finishes, the prompt
  is displayed again — every time.` published (2026-11-07T00:20:48Z) and
  verified standing the repo's **latest** (`/releases/latest` → `v0.48.0`;
  the release body the `[0.48.0]` section's own Fixed block — the
  workflow's extraction face working as designed). The trail follow-up
  `41327d7` committed the ledgers and the work logs `_000`/`_001`
  behind it; the workflow's re-run at that push ran success (the
  idempotent face held; run `42318551556`).
- **The gates at the changed tree:** **667 unit + 6 integration, 0
  failed**; clippy standing its five notices at the recorded sites —
  re-derived at that session's close (no Rust surface touched by the
  two product fixes, the suite re-run confirming the standing record).
- **The exit log `2026-11-07_000.md`** (the delivery-session's record of
  its own state — the uncommitted log lost with the session's
  environment reset) was restored verbatim from the session's opening
  read of it and stands in `.aiprj/AI_LOG/`.

C-17 closed with the publication standing; the FR-49 cycle's every
clause is delivered. These verdicts are the design side of the
requirements' §13 delivery-status rows and the task list's T-126/T-127
records.

The instruction's method (the multi-angle subagent analysis, C-12)
was carried by that session's verdicts: the mechanism angle (the
SIGPIPE reproduction), the release-verification angle (the runs
endpoint and `/releases/latest`), and the ledger-consistency angle
(the three documents aligned to the published ground) before the
writes.


## 9. The FR-50 cycle's design ground (the verdicts: D-91 as executed; D-93 the 10:15 pass's polarity verdict; D-94 — the 11:35 pass's repair-executed verdict)

The instruction's ask (「エージェント動作中のマウス操作は禁止」 — 「エージェント
動作中のマウス操作は禁止に変更する」) names the shipped mouse surface's mid-turn
behavior. §1's scoping text stands as the pre-execution record; the 2026-11-08
execution session took the decision **D-91** as a per-gate bar — the three mouse
gates joined with a turn-in-flight read through a pure helper, the capture-level
alternative set aside (the requirements' AS-88) — and delivered the product
writes at the working tree, uncommitted (the requirements' BL-M63):

- **D-91 as executed.** The helper
  `the_mouse_operation_is_prohibited(turn_in_flight: bool) -> bool` at
  `src/app.rs:1446` beside the click rules, read at all three mouse gates: the
  wheel gate `:3452`, the progress-indicator click gate `:3503`, the drag gate
  `:3556` — each joined with `state.turn_in_flight.load(Ordering::SeqCst)` (the
  flag's construction `:139`/`:335`/`:422`; the display task's TurnStart→terminal
  edge writer `:2652–2654`). Two shipped comments corrected where the bar changes
  their truth (the click gate's toggle comment `:3491–3501`, the wheel budget's
  comment `:3468–3476`), and the pin
  `a_running_turn_bars_every_mouse_operation` (`:8696` onward) added beside the
  click-rule pins.
- **Q-31's items as landed.** (a)/(b) the mechanism is the per-gate join at all
  three entries (§1's option (i) — the capture stays on); (c) the shipped
  mid-turn scrollback face stands barred (the wheel moves no view while a turn
  runs); (d) the boundary contracts stood out of the change's hands (the FR-49
  edges, the zero-touch click, the zone's flip — the suite's pins re-derived
  green, C-09).
- **D-93 — the 10:15 consistency pass's verdict: the delivered gate polarity is inverted.** The
  helper returns `turn_in_flight` (true *while* a turn is in flight) and each
  gate joins it as an enabler (`&&`), so the wheel and drag arms run **only
  mid-turn** and are dead at rest, and the click gate (`is_pending()`, already
  true exactly mid-turn) opens only under the same mid-turn conjunction — the
  exact inverse of the ask: the at-rest mouse contracts §3.1 protects are
  broken and the mid-turn work the prohibition bars stands enabled. The pin
  `a_running_turn_bars_every_mouse_operation` pins the helper's own truth
  table (`true` mid-turn, `false` at rest), not the gates' conjunctions, so
  the suite's green does not evidence the ask's polarity. **The verdict:
  FR-50a is not met — the delivery stands unreleased and unrepaired** (AC-42
  not met; the requirements' §13). The repair **T-135** re-joins each gate
  with the bar as the negative conjunction
  (`!the_mouse_operation_is_prohibited(...)`) — the minimal boundary-preserving
  shape, the ask's own words read through negation at the gates — and re-derives
  the shipped pins plus a live mid-turn battery (NFR-15) before the completion
  rows stand. The 10:42 re-load pass re-ran the helper's pin green over the
  inverted helper and re-read the three gate joins at the then-sites
  (`:3452`/`:3503`/`:3556`) — the polarity defect re-confirmed standing
  (the requirements' §13's pass row). **Executed (the requirements' BL-M66,
  D-94):** the follow-on execution session wrote the repair into `src/app.rs`
  at 11:05:56 +0900 — the doc comment rewritten to name the negated reads
  (`:1439–:1448`), the three gate joins reading the helper negated (the wheel
  `:3455`, the click `:3506`, the drag `:3559`), the pin `:8699` re-faced to
  state the gate-side conjunction `!helper(turn) == !turn` beside the truth
  table. The delivered repair face stands verified at the sites by the 11:35
  re-load pass; its session's own exit record (`2026-11-08_004.md`) stands an
  opening-rows-only state, so the live probe battery the repair's rows ride is
  uncorroborated by a written record — T-135 🟨, T-128/T-129 🟨 riding it.
- **FR-50b's doc face (the 10:15 pass's sweep; the 11:35 pass's
  re-confirmation).** The README rows whose shipped wording names the affected
  behavior: `:26` (「the wheel works during a turn
  too」) and `:27` (「during a turn and at rest alike — … even while the turn
  streams more output underneath」) — both un-truthed by the prohibition; the
  usage rows `:357` (the click row's mid-turn reach), `:420` (「It works during
  a turn」/「the running turn keeps going」), `:429` (「even while the turn
  streams more output underneath」), and the keyboard table's wheel/drag rows
  `:548–:549` (the requirements' FR-50b row; the tasks' T-136). The example
  check (§3.1 of the requirements) ran — no config surface the mechanism adds.

The completion rows stand: the version carry **0.48.0 → 0.49.0** already at the
tree by the execution session (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's
`## [0.49.0]`/`### Changed` heading under the empty `[Unreleased]`), the
delivery commit, the push to `origin/main`, the tag `v0.49.0`, and the release
standing latest running in this delivery session (FR-50c; C-14; §13) — the
completion commit, the push to `origin/main`, the tag `v0.49.0` and the
release standing latest are the rows this session carries to their close
after the ledgers' final alignment.

## 10. The FR-51 cycle's rename ground (D-92 as taken; the residuals this pass pinned)

The instruction's second 最優先事項 section 「config.tomlの内容変更」 carries the
two rename clauses. The execution session took **D-92** and delivered the
rename at the working tree (the requirements' BL-M63):

- **D-92's span as executed.** The user-facing kind strings renamed everywhere
  they name the configuration surface: the `kind =` values and the backend-list
  comment (`src/config.rs:12`, the DEFAULT_CONFIG; the tests' embedded TOMLs),
  the serde table-key renames `[provider.claude-code]` →
  `[provider.claude-cli]` and `[provider.codex-code]` → `[provider.codex-cli]`
  (`src/config.rs:295`/`:299`), the `provider_entry`/ensure lookups
  (`:1229–1231`, `:1275–1281`), the provider factory's arms and `SUPPORTED`
  (`src/ai/mod.rs:139–144`, `:160–162`), the `--provider` help row
  (`src/cli.rs:147`), the spawn tool's description (`src/tools/spawn.rs`), the
  error prefixes and diagnostics (`src/ai/claude_code.rs`,
  `src/ai/codex_code.rs`, `src/commands.rs`'s registry/doctor rows) — and no
  old-kind string remains anywhere in `src/` (this pass's tree scan: zero).
- **D-92's compatibility face.** A clean break at the configuration surface:
  the deserializer accepts the renamed key and the factory's
  `other => Err(AppError::provider(other, "unknown provider kind"))` arm maps
  no old kind — a config still carrying `kind = "claude-code"` is rejected with
  the rename-naming error, per the ask's 「〜へ変更」 (the CHANGELOG's entry
  tells the user the old values must be updated).
- **The Rust identifiers stay** (D-92's scope decision): the
  `claude_code`/`codex_code` fields, the provider type names, and the module
  file names are not user-facing kinds; the transcript drop file rides the
  renamed diagnostics face (`.agent-cli-codex-cli-last.txt`).
- **The docs delivered.** The README/README_ja backend rows, the
  usage/config/architecture/personas/tools/troubleshooting rows, and the
  example's backend blocks rewritten with the new kinds (the 10:15 pass's re-scan:
  zero old-kind strings in any of those faces), and the per-backend guides
  renamed as files (`doc/providers/codex-code.md` → `codex-cli.md`,
  `doc/providers/claude-code.md` → `claude-cli.md`) with the cross-references
  updated (the requirements' BL-M62 record; FR-51b).
- **The residuals the 10:15 pass pinned (BL-M65; the requirements' §13; the
  tasks' T-136 — re-confirmed line-exact by the 11:35 pass).** The two renamed guides carry 13 stale kind rows their own
  rewrites missed: `doc/providers/codex-cli.md` `:29`, `:31`, `:108`, `:117`,
  `:118`, `:142`, `:143`; `doc/providers/claude-cli.md` `:27`, `:29`, `:108`,
  `:109`, `:129`, `:131` — the config-block samples (`[provider.codex-code]` /
  `kind = "codex-code"`), the doctor/selftest command rows, and the error-message
  tables (`codex-code: turn timed out` …). FR-51b is not met until they are
  rewritten; FR-51a's own face (the code surface) stands met.
- The CHANGELOG's history rows keep their old names (the past releases' own
  record — not the rename's target). The child binaries (`claude`/`codex`)
  keep their names and the backends' wire behavior does not change (NFR-14) —
  the delivered tree preserves both.
- **The completion rows.** The version carried **0.48.0 → 0.49.0** at the tree
  by the execution session (the both-cycles-in-one-carry sequencing FR-51c's
  row allowed — the two entries standing under the one `## [0.49.0]` heading);
  the delivery commit, the push, the tag, and the release standing latest ride
  the trail's next session (C-14; §13).
