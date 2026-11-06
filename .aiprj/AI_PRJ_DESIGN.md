# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the mouse-click display defects repaired — the click leaves every
row where it stands and displays no prompt" (FR-48 a/b/c/d — delivered)
(`.aiprj/instructions.md`, reloaded at this pass — sha256
`aedf46b46f0c3e36638b687b3cbc62e140f19c2c778372e3b5c4b854d762e015`, 520 bytes,
mtime 2026-11-06 17:06:41 +0900, the two-clause ask; recorded in
requirements §2; verified byte-exact at the 2026-11-06 17:03 +0900 re-load
pass and again at the 2026-11-06 17:18 +0900 re-load pass's exit; the tree
carries v0.46.0, its release `v0.46.0` verified Latest)
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
| W-72 | FR-48 a/b/c/d — the symptoms reproduced live first (NFR-15), the mechanisms named file:line, the reworks delivered at their sites with the boundary intact (the requirements' §6), the fixed faces pinned (NFR-13), the live probe unshifted and prompt-paint-free, the docs written (the CHANGELOG `[0.47.0]` entry, the README/usage rows the fixes name, the example check), the version carry to **0.47.0**, the delivery committed and pushed, the release standing latest | FR-48a, FR-48b, FR-48c, FR-48d, NFR-11, NFR-13, NFR-14, NFR-15, AS-82, AS-86, AS-83, TPR-30 | T-113, T-114 | ✅ |

*W-72's execution record: the gate **plan** stands passed at the cycle's
close — the suite re-derived **665 unit + 6 integration, 0 failed** at the
changed tree (the two new pins included: `a_bare_click_costs_the_screen_
nothing`, `a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt`),
clippy standing its three notices; the live probes re-run against the
**fixed** build confirmed the faces (a bare click at rest leaves the screen
byte-identical; a real drag selects, copies with its `[clip]` report and
returns to the live face; a click under the scrolled marker leaves the
screen and the marker untouched; a mid-turn click leaves the clock standing
with the cursor parked). The first re-load pass (2026-11-06 17:18 +0900 — after the 17:03 pass had
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
feature"), the version carried 0.43.0 → **0.44.0**, the tag `v0.44.0`
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
