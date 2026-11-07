# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the mouse-click display defects repaired — the click leaves every
row where it stood and displays no prompt" (FR-48 a/b/c/d — delivered;
the FR-49 cycle (the answer-display prompt handoff) delivered beside it at
requirements §5.1)
(`.aiprj/instructions.md`, reloaded at this pass — sha256
`aedf46b46f0c3e36638b687b3cbc62e140f19c2c778372e3b5c4b854d762e015`, 520 bytes,
mtime 2026-11-06 17:06:41 +0900, the two-clause ask — **unchanged** at this
pass's read (no new ask): the re-scoping question is whether the tree's
movements — the FR-49 delivery, the release workflow — stand recorded and
consistent; recorded in requirements §2; verified byte-exact at the
2026-11-06 17:03 and 17:18 +0900 re-load passes, and again at the 2026-11-06
20:45 and 22:54 +0900 re-load passes, and at the 2026-11-07 exit log's
embedded record)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`.

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. The cycle's ask, the ground it stood on, and the decision taken (D-84)

The instruction's new 最優先事項 section 「マウスクリックすると1行ズレた表示に
なる」 names two clauses: 「マウスクリックすると1行ズレた表示になる、ズレない
表示に改修する」 — a mouse click shifts the display by one row, the display
is to be reworked so it does not shift — and 「マウスクリックするとプロンプト
を表示しないようにする」 — a mouse click is not to display the prompt. The
two sections above it in the previous instruction stand **delivered**
(FR-46 at `ebe0aca`, v0.45.0; FR-47 at `77e8a29`, v0.46.0 Latest — the
requirements' §12), so this cycle is a defect-fix cycle, not an addition
one. The repair decision **D-84** was taken at the cycle's evidence-first close:
the click path's own tail — the drain's live prompt re-render
(`src/app.rs:3807–3819`) — is the click pass's own prompt display (the
FR-48b clause) and the only write a no-op click performs; a click that
changes nothing must write nothing at all. The fix removes the no-op
gesture's paints (the press no longer flags a repaint; the release repaints
only when the gesture actually took a screen; the empty copy subprocess is
skipped) and leaves every wheel/drag/toggle path byte-identical. The
second clause is scoped as AS-86 records it: the remedy direction of the
same report — the click path's own handling displays the prompt, and the
ask removes that painting; the zone's flip and the scrolled pin (drawn by
their own hands; the requirements' §3.1) are not the clause's target.

What the tree shows today, verified at this pass (the requirements'
BL-M58), and what that ground already narrows:

- The shipped click face has **three** gated behaviors and one shared
  repaint tail. The gates: the wheel scrollback (`src/app.rs:3317–3355`),
  the progress-indicator click (`:3361–3399`, the two rules
  `zone_click_hits_clock` `:1433` and `click_hits_indicator` `:1420`),
  and the selection drag (`:3405–3491`). The tail: every mouse pass ends
  in the drain's repaint (`:3777–3819`) — the alternate screen's
  enter/leave (`ScrollOverlay::enter` `:2149` / `leave` `:2166`, with
  `zone_hide`/`zone_show` inside each), the scrolled redraw
  (`draw_scroll_overlay` `:2432`, `draw_scrolled` `:5409`), or the live
  prompt's re-render — plus the copied report (`:3820–3845`).
- The screen the click happens on is composed by **two row budgets** that
  are deliberately different: the arms count the bottom block with
  `bottom_block_rows` (`:2320`) — the suggestion row excluded, a subset of
  the drawer's count by the shipped note — while the drawer counts with
  `scrolled_bottom_rows` (`:2294`, the suggestion row included) and
  composes the block from `scrolled_block_plan` (`:2265`). The shipped
  record accepted the arms' being a subset (a "notch-shallower clamp");
  whether that accepted subset is itself the displacement's face is one of
  the pending cycle's questions, not a settled verdict.
- The click's own handling is **transient-screen work**: a selection's
  press enters the alternate screen (the overlay takes the terminal), the
  release leaves it and re-renders the live prompt; a running turn's zone
  is hidden under the overlay's screen and re-shown after. Every one of
  those handoffs must put the live screen back exactly as it was — the
  displacement would be a handoff that returns one row short of
  round-trip — and the live re-render arm (`:3807–3819`) is the click
  path's own prompt display, the face the second clause names (FR-48b).
- The shipped pins (`click_hits_indicator_covers_exactly_the_painted_block`
  `src/app.rs:8384`, `wheel_range_ends_above_the_clock_row`
  `src/app.rs:7674`, the scroll/select pins) cover the hit ranges and the
  clamps — the display's own row positions across a click are unpinned,
  which is why the suite is green while the symptoms stand. The repairs
  carry their own pins (NFR-13).

The candidate sites the ground names (candidates, not verdicts — the
cycle's evidence decides): the press/release pair's enter/leave of the
alternate screen returning the live face one row short; the arms'-vs-
drawer budgets' subset meeting a live suggestion row (or the wrapped
prompt's rows) the arms did not count; the click rules' bookkeeping (the
`painted_rows`/cursor-row round-trip, the zone's `clock_top`/`clock_rows`)
steering a repaint that lands a row off; the live re-render arm painting
the prompt where the pre-click face already stood. The instruction's
clauses map one-for-one onto the requirements: the shift complaint →
FR-48a; the no-prompt clause → FR-48b (AS-86's remedy direction); the
standing delivery clause → FR-48d; 「example更新」 → the example check (the
fixes ask no config key; the requirements' §3.1).

## 2. The touchpoints this cycle's ground named (TPR-30)

| # | Touchpoint | Evidence (the files read at this pass) |
|---|---|---|
| TPR-30 | The click path and the faces it repaints: `src/app.rs` — the mouse gates (`:3317–3499`), the click rules (`:1420–1471`), the overlay (`:2062–2181`) with `enter`/`leave`, the budgets (`:2265–2348`), the drain's repaint tail (`:3777–3845`), the zone's beat/bookkeeping (`:1589–1976`, `clock_top`/`clock_rows` set in `zone_beat` `:1784+`), the indicator's block (`:4580–4860`, `toggle_expanded` `:4645`, `painted_rows` `:4656`, `repaint` `:4816`), the prompt renderer (`:1293` `render`, `:5289`/`:5317` the live re-render, `:5409` `draw_scrolled`), the shipped pins (`:8384`, `:7674`); `src/scroll.rs` (`body_rows` `:255`, the wheel's `max_offset`/clamps, the record); `src/select.rs` (the `Selection`/`Point` arithmetic, `selected_text`, `highlight`); the delivery surfaces this cycle's completion writes: `CHANGELOG.md` (`:5`/`:32` the headings), `README.md`, `doc/usage.md` (the mouse rows), `Cargo.toml`/`Cargo.lock` (the version carry) | The files' own lines, read at this pass; the requirements' BL-M58/BL-M59 |

## 3. Analysis verdicts (C-12) — the cycle's angles

- **The mechanism angle.** The ground is pinned (§1/§2): three gated
  behaviors, one repaint tail, two budgets, two transient-screen
  handoffs, one live prompt re-render arm. The cycle's own analysis runs
  multi-angle before the D-decision; the verdict mechanism is the live
  probe — the symptoms reproduced against a live session first (NFR-15),
  the sites named file:line, then the rework.
- **The contracts angle (NFR-14).** What must not change: the wheel's
  scrollback and its marker, the drag's copy and its `[clip]` report, the
  toggle's meaning and its block, the zone's standing shape (FR-46's
  bottom line), the scrolled view's pinned composition (FR-25/FR-39), the
  peers' rows, the one-shot mode. The repairs are the click path's face
  only.
- **The scope angle (§3.1).** Not touched: the clicks' semantics, the
  zone's flip and the scrolled pin (their own hands), the providers, the
  peers/IPC/registry, the config keys (none asked — the example check
  runs and expects no write), the CHANGELOG's history.
- **The live-verdict angle (NFR-15).** Planned at this cycle: the suite
  and clippy at the changed tree; the scripted live click probe with the
  screen's rows compared pre/post click (the symptoms reproduced, then
  the fixed face — the rows unshifted and no prompt painted by the click
  path); the release-latest/tag verification at the close.
- **The docs angle (C-14, FR-48c).** The ledger's three documents and the
  work log; the products the standing sentence names (the CHANGELOG's
  `[0.47.0]` entry, the README/usage rows the fixes name, the version
  files) — the architecture surface follows the tree's own state, touched
  only if a fix's mechanism lands somewhere the doc names (the FR-47
  delivery left it unwritten; D-83 of the closed record).

## 4. Verification of the cycle (the gate W-72 — passed)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-72 | FR-48 a/b/c/d — the symptoms reproduced live first (N-15), the mechanisms named file:line, the reworks delivered at their sites with the boundary intact (the requirements' §6), the fixed faces pinned (NFR-13), the live probe unshifted and prompt-paint-free, the docs written (the CHANGELOG `[0.47.0]` entry, the README/usage rows the fixes name, the example check), the version carry to **0.47.0**, the delivery committed and pushed, the release standing latest — **re-derived green at this pass's read against the published faces**: the published `v0.47.0 — the mouse-click display fixed` body still standing at the release's notes (tag `v0.47.0` on `f9ec9ae`; the requirements' §13 first two rows' record), the FR-48 faces standing (the click leaves every row where it stood and paints no prompt; the requirements' AC-41's live battery record). | FR-48a, FR-48b, FR-48c, FR-48d, NFR-11, NFR-13, NFR-14, NFR-15, AS-82, AS-86, AS-83, TPR-30 | T-113, T-114, T-125 (this pass) | ✅ |

*W-72's execution record: the gate **plan** stands passed at the cycle's
close — the suite re-derived **665 unit + 6 integration, 0 failed** at the
changed tree (the two new pins included: `a_bare_click_costs_the_screen_
nothing`, `a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt`),
clippy standing its three notices; the live probes re-run against the
**fixed** build confirmed the faces (a bare click at rest leaves the screen
byte-identical; a real drag selects, copies with its `[clip]` report and
returns to the live face; a click under the scrolled marker leaves the
screen and the marker untouched; a mid-turn click leaves the clock standing
with the cursor parked). The delivered state's record: the ledger follow-up `e1b9118` committed the ledgers and
the work logs behind the delivery; the 2026-11-06 20:45 +0900 re-load pass
re-verified the instruction byte-exact (520 bytes, sha256 `aedf46b4…`) and
the tree at HEAD `e1b9118` ≡ `origin/main` (0.47.0, the CHANGELOG's
`[0.47.0]`/`### Fixed` entry under the empty `[Unreleased]` heading,
the working tree clean), resolving the ledgers' stale uncommitted-record
clause into the committed trail (C-08). The verification session that followed
(closing 2026-11-06) re-derived the same clauses live; its ledger writes and
the work logs `_014`/`_15` stand committed in the
trail follow-up `7d8e621`, and the 2026-11-06 22:54 +0900 re-load pass
re-verified the instruction byte-exact against that HEAD (`main` ≡
`origin/main`, the tag `v0.47.0` on `f9ec9ae`, the product tree unchanged
since the delivery, the gates standing green 665+6) and resolved the
ledgers' trail clauses into the committed trail (`8c26ecc` → `f9ec9ae` →
`e1b9118` → `7d8e621`; C-08). The
23:10 +0900 execution session that followed re-derived the same clauses
live (the tag `v0.47.0` on `f9ec9ae`, `/releases/latest` standing
`v0.47.0 — the mouse-click display fixed` as the repo's latest, the gates
green **665 unit + 6 integration, 0 failed**, clippy's five notices at the
standing sites) and took the docs angle's first new verdict since the
delivery: the CHANGELOG's empty `[Unreleased]` heading stood stranded
mid-history (each of `ebe0aca`/`77e8a29`/`f9ec9ae` had inserted its
release section above the heading, against the ground records' 「the
empty `[Unreleased]` heading stands above the newest entry」), and the
heading was hoisted to directly precede `[0.47.0]` (`315ee7f`; no release
content changed, no version carry, no new release — no feature added; the
trail `8c26ecc` → `f9ec9ae` → `e1b9118` → `7d8e621` → `315ee7f`; the
follow-ups `13d4356`/`442bc19` committed the 23:10 session's ledgers and
log and the 22:54 pass's untracked log behind it, and the 23:16 re-load
pass re-verified the instruction byte-exact against HEAD `442bc19`
(`main` ≡ `origin/main`, the release standing latest, the CHANGELOG's
heading order intact, the product tree unchanged since the hoist; C-08):
the tag `v0.47.0` on
`f9ec9ae`, the `/releases/latest` endpoint resolving `v0.47.0 — the mouse-click
display fixed` as the repo's latest release (id 436429139), the gates green
**665 unit + 6 integration, 0 failed** and clippy standing **five** distinct
notices — the two further pre-existing ones (`src/app.rs:2382`
`unnecessary_sort_by`, `src/agent.rs:2101` `collapsible_match`) verified
verbatim at the `8c26ecc` ground, the earlier passes' three-notice record
corrected (C-09). The first re-load pass (2026-11-06 17:18 +0900 — after the 17:03 pass had
re-verified it too): instruction byte-exact (520 bytes, sha256
`aedf46b4…`), the product tree standing `8c26ecc` clean with the v0.46.0
release files, the suite re-derived **663 unit + 6 integration, 0
failed**, clippy standing its three notices (the two pre-existing
dead-store notes and the `empty line after doc comment` note at
`src/app.rs:967`, the delivered one-shot's doc-comment block), the
ledger changes of both passes standing uncommitted in the working tree
(C-16; the trail nameable only from a commit to follow). The cycle's
close re-derives the same clauses at the changed tree and verifies the
tag on the delivery commit and the release as Latest. These clauses are
the acceptance criterion AC-41's, which §11 of the requirements traces
here.*

## 5. The previous cycles (recorded after their documents were retired)

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
closed, 202-11-06: delivered at `ebe0aca` ("Fix the terminal's bottom
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
follow-up `6f090f` named the delivery, and the ledger re-alignment pass
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


## 6. The FR-49 cycle's design (the answer-display prompt handoff — delivered 2026-11-07)

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
tag `v0.48.0`. The release standing latest is the cycle's remaining row
(its publication face recorded at §7's D-88 — the tasks list's T-123/T-126;
the requirements' §11 traces FR-49 here and to §7).

## 7. This pass's design verdicts (the FR-49d release face: D-88, D-89)

This pass was an evidence-first read of the post-delivery movements; the
verdicts it took, pinned at the tree and live (the requirements' §5.2's
NFR-16 record, C-17's publication clause; the task list's T-125 row, whose
records stand in the tasks' table cells):

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
  history the extraction reads). The automation's publish step fails,
  each run's exit code read from the runner's own log (`gh run download
  <run-id>`): **exit 128** at `1d8ccc3` and again at `0a49302` — both
  printing `fatal: not a git repository (or any of the parent
  directories): .git`, the checkout's shallow fetch — and **exit 141**
  at `40fc7c4` (the full-history checkout fix's run), the runner
  stopping mid-publish step after `newest tag: v0.48.0` with none of
  the workflow's own messages printed. `v0.48.0` remains unpublished
  (`gh release view v0.48.0` → not found; `/releases/latest` →
  `v0.47.0`); the FR-49d row stands pending the publication. The
  publication face is the `gh release create` line at
  `.github/workflows/release.yml`'s step (the repo's own transport,
  `contents: write`) riding the next push.
- **D-89 — the notes-file's extraction.** The workflow's extract reads
  `git show "$TAG:CHANGELOG.md"` and an awk program
  (`index($0, want) == 1`) over the block from the `## [0.48.0]` heading
  until the next `## [` heading; the section content the extraction
  would publish is the two `### Fixed` blocks the tree's own CHANGELOG
  carries — **a second `### Fixed` heading stands under the
  `## [0.48.0]` heading** (the FR-49 delivery commit `89f1ea0` turned the
  previous `## [0.47.0]` heading into the new `## [0.48.0]` heading and
  added its own `### Fixed` block above the standing one,
  evidenced `git show 89f1ea0 -- CHANGELOG.md`; the heading order now
  `:7` `## [0.48.0]` → `:9` `### Fixed` (the FR-49 block) → `:23`
  `### Fixed` (the FR-48 block) → `:36` `## [0.46.0]`), which stands
  as the delivery recorded the release face ("the `[0.48.0]`/`### Fixed`
  entry written; the `[Unreleased]` heading standing above") — the
  extraction's own section is the merged one the tree carries; the
  D-88/D-89 record is the FR-49d row's face and the next pass's
  publish face is the same `gh release create` line with the section the
  tree carries.

These verdicts are **recordings of the FR-49d row's face** and the next
pass's obligations: the `v0.48.0` release publication (the tag's release
created from the section's own content standing `Latest`, via the repo's
own workflow) is the FR-49d row's remaining clause.

These verdicts are evidence-first recordings of the FR-49d row's face,
not fixes: the workflow file stands as the last session delivered it (the
`40fc7c4` checkout fix); the FR-49d row's remaining clause is the
publication's re-run verdict, taken by the next pass's read (the
publication face is the `gh release create` line's next push's run).

## 8. The 2026-11-07 delivery session's verdict (D-90 — the SIGPIPE mechanism, the publication delivered)

The user's 「残タスクが無くなるまで作業を実施」 carried the C-17
publication row to its close. The verdicts, pinned by execution:

- **D-90 — the runner's exit 141 is SIGPIPE.** Reproduced locally under
  the workflow's own gate set: `SECTION="$(git show "$TAG:CHANGELOG.md" |
  awk ...exit...)"` under `set -euo pipefail` reports PIPESTATUS `141 0`
  — awk's early `exit` closes the read while `git show` is still writing
  the 88 KB CHANGELOG blob, and the writer dies of SIGPIPE, which
  pipefail surfaces as the command substitution's exit code. The fix
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
  unpublished, so the move costs no published face) and force-pushed
  (`89f1ea0` → `592646e` the tag object, `9040435` the peeled target);
  the branch pushed `40fc7c4..9040435`; the workflow's run
  `42317643598` (head `9040435`) concluded **success**; the GitHub
  release `v0.48.0 — After the \`[answer]\` display finishes, the prompt
  is displayed again — every time.` published (2026-11-07T00:20:48Z)
  and verified standing the repo's **latest**
  (`/releases/latest` → tag_name `v0.48.0`; the release body the
  `[0.48.0]` section's own Fixed block — the workflow's extraction face
  working as designed).
- **The gates at the changed tree:** **667 unit + 6 integration, 0
  failed**; clippy standing its five notices at the recorded sites —
  re-derived at this session's close (no Rust surface touched by the
  two product fixes, the suite re-run confirming the standing record).
- **The exit log `2026-11-07_000.md`** (the delivery-session's record of
  its own state — the uncommitted log lost with the session's
  environment reset) was restored verbatim from this session's opening
  read of it and stands in `.aiprj/AI_LOG/` (the work-log rule; the
  trail's follow-up carries it).

C-17 closes with the publication standing; the FR-49 cycle's every
clause is now delivered (the requirements' FR-49d row's remaining
obligation discharged). These verdicts are the design side of the
requirements' §13 delivery-status rows and the task list's T-126/T-127
records.

The instruction's method (the multi-angle subagent analysis, C-12)
stands carried: this session's verdicts took the mechanism angle (the
SIGPIPE reproduction), the release-verification angle (the runs
endpoint and `/releases/latest`), and the ledger-consistency angle
(the three documents aligned to the published ground) before the
writes.
