# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the mouse-click display defects repaired — the click leaves every
row where it stood and displays no prompt" (FR-48 a/b/c/d — delivered)
(`.aiprj/instructions.md`, reloaded 2026-11-06 17:03 +0900, verified
byte-exact at the pass's exit — sha256
`aedf46b46f0c3e36638b687b3cbc62e140f19c2c778372e3b5c4b854d762e015`, 520
bytes, mtime 2026-11-06 17:06:41 +0900 — **changed** from the FR-47 cycle's
reload: the delivered 最優先事項 sections are gone, the two-clause ask added)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked

---

## Overview

The reloaded instruction's ask (FR-48), one 最優先事項 section, two clauses:
「マウスクリックすると1行ズレた表示になる — マウスクリックすると1行ズレた
表示になる、ズレない表示に改修する」 — a mouse click shifts the display by
one row, the display is to be reworked so it does not shift — and 「マウス
クリックするとプロンプトを表示しないようにする」 — a mouse click is not to
display the prompt. The delivered FR-46/FR-47 asks were carried by the
instruction until this reload and stand closed (v0.46.0 the latest release,
the requirements' §12). The shipped click paths pin the ground (the
requirements' BL-M58): the wheel gate (`src/app.rs:3317–3355`), the
progress-indicator click gate (`:3361–3399`, the `zone_click_hits_clock` /
`click_hits_indicator` rules and the `toggle_expanded` repaint
`:4645–4654`), the selection gate (`:3405–3491`), and the drain's repaint
tail every mouse pass ends at (`:3777–3845` — the live prompt re-render
`:3807–3819` among its arms). The standing gates were re-derived at the
re-load passes: **663 unit + 6 integration, 0 failed**, clippy its three
standing notices (the requirements' C-09 record at those passes), v0.46.0,
HEAD `8c26ecc` ≡ `origin/main`. The delivered trail now stands `8c26ecc` →
`f9ec9ae` (the delivery, **v0.47.0**) → `e1b9118` (the FR-48 cycle's ledger
follow-up) → `7d8e621` (the verification-session trail follow-up) →
`315ee7f` (the CHANGELOG's `[Unreleased]` heading hoist — the docs angle's
verdict of the 23:10 execution session; **HEAD**, `main` ≡ `origin/main`),
the session's ledger writes standing uncommitted for the trail's next
follow-up.

**The ask is delivered** (2026-11-06): the mechanisms were evidenced
file:line and probed live (the T-112 record), the fixes delivered at their
sites (T-113), and the completion carried the docs, the version and the
release (T-114) — the delivery `f9ec9ae`, v0.47.0 Latest. The final faces,
verified live post-fix: a bare click at rest leaves the screen byte-identical
(`[rest-click] identical=True`, and the only mid-turn variance across the
battery was the clock's own elapsed text, never a row), a real drag selects,
copies (`[clip] copied 2 lines (77 bytes) to the clipboard via osc52`) and
returns to the live view, and a click while the view is scrolled (`Message
below` showing) leaves the screen byte-identical with the marker standing.

## The cycle's tasks

| ID | Task | Requirements | Design | Status |
|---|---|---|---|---|
| T-112 | The symptoms' mechanisms evidenced and the multi-angle analysis recorded (FR-48a, FR-48b; design §1, §3): the shipped click paths read at their sites (the three gates and the drain's repaint tail, the budgets, the transient-screen handoffs — the requirements' BL-M58, the design's TPR-30 candidates), the live reproduction probe **executed** (NFR-15): the click battery against a live ollama session (tmux + raw SGR mouse bytes) found **no one-row shift and no prompt display at any tested target** (body rows at rest and mid-turn with the turn provably alive across the sampling windows, the block/clock rows, a wrapped multi-row prompt block, a suggestion row, wheel followed by click; cursor drift none in every probe; the control run without a click showed the zone's standing clock/caret pose throughout) — the static map named the decisive contract (a bare log click ran the drain-tail's live prompt re-render, the click pass's own painting) and the D-84 decision stands recorded (design §1); the multi-angle record rides the design's §3 and the work log. — **Done**: the analysis, the live battery and the D-84 record delivered. | FR-48a, FR-48b, NFR-11, NFR-13, NFR-15, C-12 | design §1 (D-84), §2 (TPR-30), §3, §4 (W-72) | ✅ |
| T-113 | The fixes (FR-48a, FR-48b; design §1): the one-row displacement removed at its evidenced sites — a click leaves every row where it stood — and the click path's own prompt display removed (AS-86's remedy direction: the prompt face returns only through the hands the shipped flow already owns); the click's own meaningful outcomes unchanged (the toggle, the copy with its `[clip]` report, the wheel's scrollback); the boundary (§3.1, §6, NFR-14) intact. — **Done**: the press records the drag's anchor without painting, the release writes nothing for a bare click (an empty gesture skips the clipboard copy as well) and restores the screen only when the gesture took one, so a click that changes nothing writes nothing at all; the live probes re-derived the faces post-fix (the bare click identical, the real drag copying with its `[clip]` report and returning to the live view, the scrolled-click leaving the marker face untouched, mid-turn A/B unshifted with no prompt display); the fixes' own pins (`a_bare_click_costs_the_screen_nothing`, `a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt`, the empty-gesture silence in the clip pin) ride the gates — **665 unit + 6 integration, 0 failed**, clippy standing its three notices; delivered in the commit `f9ec9ae` | FR-48a, FR-48b, NFR-13, NFR-14, NFR-15 | design §1 (D-84), §3, §4 (W-72) | ✅ |
| T-114 | The completion (FR-48c, FR-48d; design §3 the docs angle, §4 W-72): the CHANGELOG's `[0.47.0]` entry recording both fixes in user language (the empty `[Unreleased]` heading stands above it); the README's and `doc/usage.md`'s rows whose shipped wording names the affected behavior (the mouse rows); the example check (the requirements' §3.1); the version carry **0.46.0 → 0.47.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG heading); the delivery commit pushed to `origin/main`, the tag `v0.47.0`, the release standing the **latest** version | FR-48c, FR-48d, NFR-11, C-14, C-15 | design §3 (the docs angle), §4 (W-72) | ✅ |
| T-115 | The ledger's consistency and the byte-exact exit check (C-08, NFR-12, C-15, C-16): the three documents aligned together at every update pass — the earlier concurrent-draft discrepancies resolved (the T document's stale scheme: the 441-byte instruction record, the single-clause lettering, the dangling B-4/W-1/TPR-31/D-85 references); the 2026-11-06 17:18 +0900 re-load pass re-verified the instruction byte-exact (520 bytes, sha256 `aedf46b4…`), the product tree unchanged and the gates green (**663 unit + 6 integration, 0 failed**; clippy's three notices re-read) with the consistency re-probed (the D/T references resolving into the requirements' rows); the work log `2026-11-06_011.md` written by the ledger pass, `_012.md` by its re-load pass, and `2026-11-06_013.md` by the execution session (Art. 6/7; the 520-byte instruction embedded verbatim each time); no document carrying any mark of authorship or tooling (Art. 8, NFR-12); the write surface the three `.aiprj` documents and the work logs, nothing else (C-15, Art. 3) | — (C-08, NFR-12, C-15, C-16) | design §1 – §4 | ✅ |
| T-116 | The re-load passes' consistency and exit checks (C-08, NFR-12, C-15, C-16): the instruction re-verified byte-exact at each pass's exit (520 bytes, sha256 `aedf46b4…` — at the 17:03/17:18 passes and again at the 2026-11-06 20:45 +0900 pass that resolved the ledgers' stale uncommitted-record clause into the committed trail: HEAD `e1b9118` (the ledger follow-up naming `f9ec9ae`), `main` ≡ `origin/main`, the version files 0.47.0, the CHANGELOG's `[0.47.0]`/`### Fixed` entry under the empty `[Unreleased]` heading, the working tree clean); the three documents aligned together (the traceability's T-116 dangling reference supplied; the design's §1/§4 and this list's overview/trail naming the same commits); no product file written and no commit made (Art. 3), the work log `2026-11-06_014.md` written | — (C-08, NFR-12, C-15, C-16) | design §1, §4 | ✅ |
| T-117 | The delivered state's verification session (the delivery FR-48 a/b/c/d standing verified; design §4's record extended): the instruction re-read byte-exact (520 bytes, sha256 `aedf46b4…`); the tag `v0.47.0` verified pointing at the delivery commit `f9ec9ae`; the Releases API's `/releases/latest` resolving `v0.47.0 — the mouse-click display fixed` as the repo's latest release (id 436429139, published 2026-11-06T11:37:01Z, `isDraft:false`/`isPrerelease:false`); HEAD `e1b9118` ≡ `origin/main`, the version files 0.47.0, the CHANGELOG's `[0.47.0]`/`### Fixed` entry under the empty `[Unreleased]` heading; the gates re-derived at the unchanged tree — **665 unit + 6 integration, 0 failed**, clippy standing five distinct notices (the corrected C-09 record); no product file written, no release made (nothing added beyond the delivered cycle) | FR-48a, FR-48b, FR-48c, FR-48d verified; NFR-13, NFR-15, C-09 re-derived | design §4 (W-72) | ✅ |
| T-118 | The 2026-11-06 22:54 +0900 re-load pass (C-08, NFR-12, C-15, C-16): the instruction re-verified byte-exact (520 bytes, sha256 `aedf46b4…`, mtime 2026-11-06 17:06:41 +0900 — unchanged since the delivered cycle's and the verification session's exits) and re-scoped against the delivered cycle (no new ask); the tree re-derived at the delivered state — HEAD `7d8e621` (the verification-session trail follow-up committing the verification session's ledgers and the work logs `_014`/`_015` behind the delivery `f9ec9ae`), `main` ≡ `origin/main` (verified via `git ls-remote`), the tag `v0.47.0` verified on `f9ec9ae` (remote-verified), the version files 0.47.0, the CHANGELOG's `[0.47.0]`/`### Fixed` entry under the empty `[Unreleased]` heading, the working tree clean; the product tree unchanged since `f9ec9ae` (the commits behind it only `.aiprj` files), so the verification session's gate record stands valid (**665 unit + 6 integration, 0 failed**; clippy five distinct notices); the ledgers' stale trail clauses (HEAD named `e1b9118`) resolved into the committed trail `… → f9ec9ae → e1b9118 → 7d8e621` (HEAD) across all three documents, and the cross-cutting gates aligned to the delivered ground (C-08); no product file written and no commit made (this pass's ledger writes stand uncommitted for the trail's next follow-up, Art. 3), the work log `2026-11-06_016.md` written | — (C-08, NFR-12, C-15, C-16) | design §1, §4 | ✅ |
| T-119 | The 2026-11-06 23:10 +0900 execution session (the delivered state's re-verification, with one docs-angle verdict taken; C-08 resolved): the instruction re-read byte-exact (520 bytes, sha256 `aedf46b4…`); the delivered state verified live — the tag `v0.47.0` on `f9ec9ae` (remote-verified), `/releases/latest` resolving `v0.47.0 — the mouse-click display fixed` as the repo's latest release (published 2026-11-06T11:37:01Z), HEAD `7d8e621` ≡ `origin/main`; the gates re-derived — **665 unit + 6 integration, 0 failed**, clippy standing five distinct notices at the recorded sites; the multi-angle clause taken — the docs/consistency angle and the scope/release angle dispatched to the two registered peers, both reply windows timed out (120s each), the verdict taken on the file/git evidence: the CHANGELOG's empty `[Unreleased]` heading stood stranded mid-history (below `[0.47.0]` — `ebe0aca`/`77e8a29`/`f9ec9ae` each inserted its release section above the heading, against the ledgers' standing record 「the empty `[Unreleased]` heading stands above the newest entry」 and the file's Keep-a-Changelog claim; evidenced `b5deb9c` `:5` → `ebe0aca` `:20` → `77e8a29` `:32` → the tree `:47`); resolved per the Coding Guidelines' Art. 2 by aligning the tree to the recorded intent — the empty heading hoisted to directly precede `[0.47.0]` (2-line heading move; no release content, no version bump, no new release — no feature added; the published `v0.47.0` untouched); committed `315ee7f` and the gates re-derived green at that tree; the work log `2026-11-06_017.md` written | FR-48c (the CHANGELOG's standing shape); C-08, C-09, NFR-15 re-derived; C-14 untriggered (no feature) | design §4 (W-72) | ✅ |

## The closed tasks of the delivered cycles (their records stand)

- **T-108/T-109/T-110** (the FR-47 one-shot mode, the completion, the
  ledger's consistency) — ✅, closed 2026-11-06: the delivery `77e8a29`
  ("Add the one-shot subcommand: one prompt in, the printed answer out,
  then exit"), the version carried **0.45.0 → 0.46.0**, the tag `v0.46.0`,
  the release verified Latest (created 2026-11-06T01:16:40Z, published
  01:16:50Z); gates **663 unit + 6 integration, 0 failed**; the trail's
  follow-up `1b3c7e1`, the session's exit log `8c26ecc`, and the trail's follow-ups behind it —
  the FR-48 delivery's ledger follow-up `e1b9118` and the verification-session
  trail follow-up `7d8e621` (**HEAD**). The
  requirements' §12/§13 and the design's §5 carry the full record.
- **T-105/T-106/T-107** (the FR-46 terminal display fix, the completion,
  the ledger passes) — ✅, closed 2026-11-06: delivered at `ebe0aca`
  ("Fix the terminal's bottom line…"), the version carried to **0.45.0**,
  the tag `v0.45.0`, the release verified Latest; gates **663 unit +
  6 integration, 0 failed**; the recording/re-load passes
  `2557f7f`/`92c43fd` and the ledger follow-up `5978dc9`. Work logs
  `2026-11-06_005.md`–`_007.md`.
- **T-102/T-103/T-104** (the FR-45 removal, the completion, the ledger's
  consistency) — ✅, closed 2026-11-06: delivered at `b5deb9c` (the tag
  `v0.44.0`, the release Latest), the trail's follow-up `6f0390f` naming
  it; gates **663 unit + 6 integration, 0 failed**; the ledger
  re-alignment `0d45242`. Work logs `2026-11-06_002.md`–`_004.md`.
- **T-95/T-96** (the FR-43 investigation and its ledger record) — ✅,
  closed 2026-11-05 (no release; work log `_008` of 2026-11-05).
- **T-97/T-98/T-99** — ✅, closed 2026-11-05: FR-44 delivered at
  `122f320` (the version 0.42.0), gates **669 unit + 6 integration,
  0 failed**.
- **T-100** (the FR-44d example amendment) — ✅, closed 2026-11-05:
  delivered at `4fee899`, the version **0.43.0**.
- **T-101** (FR-44e, the latest release) — ✅, closed on the `v0.43.0`
  release's Latest verification (created 2026-11-05T10:57:38Z).

## Cross-cutting gates (C-09, the boundary) — standing

- The suite at the delivered ground: **665 unit + 6 integration, 0 failed**
  (re-derived at the delivery cycle's close and again at the 2026-11-06
  23:10 execution session's tree — the CHANGELOG heading hoist included; the
  ledger commits behind the delivery touch only `.aiprj` files).
- Clippy at the verification session's re-derivation: **5** standing
  notices — the two pre-existing dead-store notes (`src/app.rs:8324` /
  `:8582`), the `empty line after doc comment` note on `run_oneshot`'s doc
  comment (`:967`; the delivered one-shot's block), plus the two
  pre-existing notices the earlier passes undercounted (`unnecessary_sort_by`
  at `:2382`, `collapsible_match` at `src/agent.rs:2101` — both verbatim at
  the `8c26ecc` ground; the requirements' C-09 record corrected).
- The FR-47 boundary stands: the one-shot process binds **no** socket (the
  transient registry entry, the closed record's D-81); the FR-46 boundary
  stands: the prompt/clock flip (the prompt while input is possible, the
  clock mid-turn).
- The FR-48 cycle's ground (the requirements' §3/BL-M58/BL-M59) verified;
  its gates re-derived at the changed tree with the delivery standing
  (T-113) and re-confirmed at the unchanged tree.
- The `.aiprj` documents mutually consistent (C-08 — this pass resolved
  the task-list draft's stale scheme named in T-115); the work log written
  per Art. 6/7 (never overwritten); the instruction byte-exact at exit
  (520 bytes, sha256 `aedf46b4…`).

## The previous cycles' record

**"the mouse-click display defects repaired" (FR-48 a/b/c/d)** — closed,
2026-11-06: delivered at `f9ec9ae` ("Fix the mouse-click display: a click
leaves every row where it stood and paints no prompt"), the version carried
0.46.0 → **0.47.0**, the tag `v0.47.0` verified on it, the release
`v0.47.0 — the mouse-click display fixed` published and verified **Latest**
(published 2026-11-06T11:37:01Z); gates at the close **665 unit + 6
integration, 0 failed** (the pins `a_bare_click_costs_the_screen_nothing`
and `a_drag_keeps_its_screen_and_a_release_outside_leaves_no_debt` among the
663 standing), clippy standing its three pre-existing notices; the live
click battery (tmux + raw SGR mouse bytes, ~25 scenarios) and the post-fix
verdicts carried the fixed faces — a bare click writes nothing at all, a
real drag keeps its copy-and-return path, a scrolled click leaves the
pinned block face intact. The delivery re-verified live at the verification
session's close: the tag on `f9ec9ae`, the release standing the repo's
latest (`/releases/latest`, id 436429139), the gates **665 + 6, 0 failed**.
Work logs `2026-11-06_013.md` and `_015.md`.

**"the one-shot mode added" (FR-47 a/b/c)** — closed, 2026-11-06:
delivered at `77e8a29` ("Add the one-shot subcommand…"), the version
carried to **0.46.0**, the tag `v0.46.0`, the release `v0.46.0 — the
one-shot subcommand` published and verified Latest (created
2026-11-06T01:16:40Z, published 01:16:50Z); gates **663 unit + 6
integration, 0 failed**; the delivered records stand in the requirements'
§12 and the design's §5 (the transient registry entry removed with the
turn's end; the answer printed inline from the `run_oneshot` event loop;
the `--quiet` flag parsed-and-not-read; `doc/architecture.md` untouched).
Work logs `2026-11-06_008.md` (the cycle's record) and `_009.md`/`_010.md`
(the re-load passes).

**"the terminal's bottom-most line fixed" (FR-46 a/b/c)** — closed,
2026-11-06: delivered at `ebe0aca` ("Fix the terminal's bottom line…"),
the version carried to **0.45.0**, the tag `v0.45.0`, the release verified
Latest (created 2026-11-06T00:55:05Z); gates **663 unit + 6 integration,
0 failed**. Work logs `2026-11-06_005.md`–`_007.md`.

**"the prompt-input queueing feature's removal" (FR-45 a/b/c)** — closed,
2026-11-06: delivered at `b5deb9c` (v0.44.0, the release Latest); gates
663+6; the follow-ups `6f0390f`/`0d45242`. Work logs `_002`–`_004`.

**"the codex-code backend…" (FR-44 a–e)** — closed, 2026-11-05: delivered
at `122f320` and `4fee899` (the FR-44d amendment), versions 0.42.0/0.43.0,
the release **v0.43.0 verified latest**; gates **669 + 6**.

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
delivery; **v0.47.0**) → `e1b9118` (the ledger follow-up naming `f9ec9ae`)
→ `7d8e621` (the verification-session trail follow-up committing the
verification session's ledgers and the logs `_014`/`_015`) → `315ee7f` (the
CHANGELOG's `[Unreleased]` heading hoist — the docs angle's verdict of the
23:10 execution session; **HEAD**) — the trail's
follow-ups name the deliveries behind them (per the trail rule, a delivery
commit's sha is nameable only from the commit that follows it).

Earlier cycles stand in the requirements document's §12 and the design
document's §5.
