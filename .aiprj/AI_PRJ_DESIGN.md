# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the terminal display fix — the screen's bottom-most line reads the
prompt while input is possible and nothing but the spinner-and-elapsed clock
while the agent runs" (FR-46 a/b/c)
(`.aiprj/instructions.md`, reloaded 2026-11-06 09:16:49 +0900 — sha256
`5cb53102…`, 699 bytes, recorded in requirements §2; the tree carries
v0.44.0, its release verified Latest and re-read at this cycle's ground)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`.

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What is decided, and what it costs

The instruction's 最優先事項 is a display rewrite, refined by two clauses
that bind the bottom row per state: 「ターミナルの最下位行を入力が可能なときは
プロンプト表示、エージェントが実行中のときはクルクルと経過時間に修正する」,
「最下位行は必ず、「プロンプト表示」か実行中を示す「クルクルと経過時間」を
表示しなければいけない」, 「プロンプト表示は入力下の状態のときのみ表示する」.
The third clause is the binding constraint: the **prompt display appears
only** in the input-possible states (`Ready` at the input prompt,
`AwaitingApproval` at the y/N prompt) — mid-turn the bottom line is the
clock alone, and **the prompt span does not paint mid-turn at all**
(AS-74's overturn of the earlier span-above reading; the span's editing
affordance rides FR-45's own removal — while a turn runs every keypress
but the cancel keys is ignored, so a visible half-typed line mid-turn was
shipped affordance without a way to use it. FR-46's flip retires the
vestigial span). The shipped shape the ask replaces: while a turn runs the
prompt span stays pinned on the screen's bottom rows and the clock block
composes **above** it (FR-37a/FR-40; the CHANGELOG's `[0.42.0]` record:
「The bottom-most line is the prompt's alone, and the elapsed row stays
displayed above it」; BL-M54's file:line inventory). The design decision is
**D-75 — the mid-turn standing shape inverts to clock-at-bottom, spanless**.

- **D-75 — the zone's composition recomposes with the clock as the
  bottom-most tenant.** The live screen mid-turn paints the same three
  transient regions in the same relative stacking (the peers' band above
  the clock block, the log tail above the band — FR-37's order, bottom-up
  unchanged), but the **clock block's last row lands on the screen's bottom
  row** and **no prompt span paints** while the zone is armed. Concretely,
  in `src/app.rs`:
  1. **The span paint retires from the armed screens.** The `render_prompt`
     call sites while a turn runs — the submit arm (`src/app.rs:3567–3579`),
     the command arm (`:3515–3529`), the reconcile arm's armed side
     (`:3687–3698`) — stop painting the span: `zone_paint_span` (`:1797`)
     is no longer called while the zone is armed, and the armed screen's
     bottom rows belong to the clock block alone. The bare-prompt side of
     the reconcile arm (`:3699–3708`) keeps its shipped shape (the prompt
     is the idle bottom line).
  2. **The zone anchors on the clock's floor.** `zone_arm` (`:1453`)
     anchors the zone with `span_top` retired — the zone bookkeeping keeps
     the geometry the beat needs (`height`, `painted_top = height − 1`) and
     drops the span anchor/caret fields the spanless shape has no tenant
     for; `zone_beat_plan` (`:1599`) re-derives: `block_home = height −
     block_n` (the clock block's last row on the screen's bottom row), the
     peers' band above the block, the tail above the band, the sweep
     erasing the whole region `0..height` the beat owns (the span's
     former-floor `sweep_to` retires with the span).
  3. **The disarm restores the shipped handoff.** `zone_disarm_plan`
     (`:1621`) keeps the `height−2` replay floor (D-54): the turn's end
     erases the zone's rows and replays the tail above the fresh bottom
     row, where the shipped render draws the prompt — the release arms
     (`:2973–2986` idle; the cancel paths `:3404`/`:3434`; the approval
     deny at `:3424–3446`) already paint the prompt after the disarm, so
     the 「入力が可能なときはプロンプト表示」 clause is the shipped shape
     restored, and the third clause's 「入力下の状態のときのみ」 holds
     there too (the approval prompt is an input-possible state).
  4. **The scrolled screen stands unchanged** (AS-73): the scrolled pin
     keeps FR-39/FR-40's composition (`scrolled_block_plan` `:2102` — the
     elapsed row above the pinned span, the marker above that, the peers
     above) and `draw_scrolled` (`:5219`)/`render_prompt_with_run`
     (`:5134`) keep their handoffs; the instruction names the live
     terminal's bottom line, and the scrolled screen is the log's own
     viewport. (The delivery's live-verdict probe re-asks against FR-40's
     record before writing — the design keeps the scrolled side whole
     unless the implementation read names it otherwise.)
  5. **The click rule re-derives with the geometry.** The zone click rule
     (`zone_click_hits_clock` `:1292`, `click_hits_clock_block` `:1309`)
     re-derives against the block's moved extent (the block's rows ending
     at the screen's bottom row now); the shipped thinking-toggle rule
     (FR-27) rides it unchanged in shape.

  The cost: the pins binding the shipped composition re-derive together
  (NFR-13's list) — the beat-plan order pin, the disarm fresh-row pin, the
  additive budget pins (FR-46a's statement in the requirements' §5), the
  scrolled plan's pins standing per AS-73 — and any site left painting the
  shipped shape would land the clock over the span or the span over the
  clock; the re-derivation is one coherent move, not a per-site patch.

- **D-76 — the docs follow the swap, at the sentences that name the
  shipped shape.** The CHANGELOG's `[Unreleased]` becomes the `[0.45.0]`
  entry recording the swap in user language (what changed at the screen's
  bottom: while the agent runs the spinner-and-elapsed clock is the
  bottom-most line alone — the typed line's echo stands in the log from
  the submit — and when input is possible the prompt is the bottom line
  again; `Esc`/`Ctrl-C` and every other behaviour unchanged). The shipped
  sentences naming the old owner follow (BL-M55's surface list):
  `README.md:13`'s 「the spinner's clock stays the screen's bottom row」,
  `README.md:26`'s wheel bullet, `doc/usage.md:77–79`'s Subagents
  paragraph, `doc/usage.md:418`'s 「the pinned block keeps the bottom
  row」, the `/help` subagent sentence (`src/app.rs:5307–5308`, riding the
  code's commit), and `doc/architecture.md`'s region paragraphs as the
  delivery reads them. The CHANGELOG's `[0.40.0]`/`[0.42.0]` entries stand
  as history — no rewrite.

- **D-77 — the completion repeats the standing delivery machinery
  (AS-75).** The minor version carries **0.44.0 → 0.45.0** (`Cargo.toml`,
  `Cargo.lock`, the CHANGELOG's `[0.45.0]` heading), the delivery
  commit(s) of the fix and the documents are pushed to `origin/main` (the
  trail rule governs the sha naming — a commit is nameable only from the
  commit that follows it), and the GitHub release is **published as the
  latest version** (FR-44e/FR-45c's verified shape: the `isLatest` probe
  re-asked at 0.45.0, the tag verified on the delivery commit via
  `git rev-parse`).

The instruction's clauses map one-for-one onto the decisions:
「入力が可能なときはプロンプト表示」 → D-75's at-rest shape (the shipped
idle and approval prompts, restored at every handoff); 「実行中のときは
クルクルと経過時間」 + 「必ず…どちらかを表示」 → D-75's inverted mid-turn
shape (the clock's own face — `status_row` `:4577`, `SPINNER_FRAMES`
`:4157`, `format_elapsed` `:4179` — unchanged, its tenant changed) and the
at-rest shape's guarantee; 「プロンプト表示は入力下の状態のみ」 → D-75's
spanless mid-turn composition (AS-74's overturn). ドキュメント更新 → D-76;
the method C-12 → §3's angles; マイナーバージョンアップ + commit, push +
リリース最新 → D-77. 「example更新」 names the example configuration: its
`[ui]` block names the spinner's existence only (`show_progress`'s comment
— BL-M55's verified absence of a bottom-line sentence), so no example write
exists unless the delivery's read contradicts it.

## 2. The touchpoints this cycle touches (TPR-28)

| # | Touchpoint | Evidence (the files read in this cycle) |
|---|---|---|
| TPR-28 | The REPL's mid-turn standing shape and the surfaces that name it: `src/app.rs` — `zone_arm` (`:1453`), `zone_disarm` (`:1507`) and `zone_disarm_plan` (`:1621`), `zone_park` (`:1563`), `zone_beat_plan` (`:1599`) and `zone_beat` (`:1635`), `zone_paint_span` (`:1797`, retiring from the armed screens), `zone_reanchor` (`:1749`), the clock's face `zone_block_rows` (`:4605`)/`status_row` (`:4577`) with `SPINNER_FRAMES`/`spinner_frame` (`:4157`–`:4173`), `format_elapsed` (`:4179`), `TICK_INTERVAL` (`:4160`); the arm/disarm sites — the submit arm (`:3567–3579`), the command arm (`:3515–3529`), the reconcile arm (`:3687–3710`), the release arm (`:2973–2986`), the cancel paths (`:3404`, `:3434`), the approval deny (`:3424–3446`), the shell state's arm (`:3502–3507`), the piped loop's mirrored arms (`:2949–2986`); the scrolled side kept whole — `draw_scrolled` (`:5219`), `render_prompt_with_run` (`:5134`), `scrolled_block_plan` (`:2102`), `scrolled_bottom_rows` (`:2131`), `bottom_block_rows` (`:2157`), `run_row_rows` (`:2075`); the pins re-deriving: `the_beat_plan_holds_the_four_regions_in_the_instruction_order` (`:6983`), `the_disarm_leaves_the_bottom_row_fresh…` (`:7051`), `the_clock_row_is_not_the_subagent_lines_price` (`:7090`), `the_line_survives_the_intersections…` (`:7106`), the scrolled plan pins (`:7193`, `:7308`); the docs sentences — `/help` (`:5307–5308`); `README.md:13`, `:26`, the keymap row `:547`; `doc/usage.md:77–79`, `:415–418`; `doc/architecture.md:123` + the turn-zone paragraphs; `CHANGELOG.md` `[Unreleased]`/`[0.40.0]`/`[0.42.0]`/`[0.44.0]`; `example/config.example.toml` `[ui]`; `Cargo.toml`, `Cargo.lock` | The files' own lines, read in this cycle; BL-M54/BL-M55's records |

## 3. Analysis verdicts (C-12) — the multi-angle analysis of the swap

- **The mechanism angle.** The mid-turn shape is one composed drawing —
  the zone beat's — and the swap is one re-derivation: the span paint
  retires while armed, the zone's bookkeeping keeps what the beat needs
  and drops the span fields, the beat's pure plan re-binds its floors with
  the clock at the screen's bottom row, and the disarm's handoff already
  restores the prompt on a fresh bottom row. No new writer, no new event
  type, no new state machine: the beat already owns the armed rows and the
  release arms already draw the prompt after the disarm; the shell (`!`)
  state keeps its shipped all-zone shape (no span at all — AS-73's
  reading), which D-75's spanless mid-turn shape now matches.
- **The contracts angle (NFR-14).** The shipped contracts stand except the
  bottom-most row's tenant mid-turn: the four regions keep their relative
  stacking (the peers' band above the clock, the log above the band —
  FR-37's order with the floor moved), the bare-`Enter` guard (FR-38), the
  cancel keys' meanings, the approval takeover, the outcome rows'
  recording, the four exit seams and the teardown/drain, the mouse-capture
  and clipboard contracts, FR-39's marker place on the scrolled screen,
  the queueing feature's removal (FR-45's pre-queue contract), the
  codex-code backend — none touched. The span's retirement mid-turn rides
  the boundary's own clause (AS-74's overturn is the ask): the editing
  affordance was vestigial under FR-45's ignore-everything contract.
- **The scope angle (§3.1).** The ask names the bottom-most line's tenant
  by state — **not** touched: the spinner's frames and format, the
  thinking rows' presence and click rule (FR-27), the peers' band's face
  (FR-37b), the scrolled screen's composition (FR-39/FR-40 — AS-73), the
  disarm's replay handoff (D-54), `--exit-when-idle`'s drain semantics
  (FR-32), the queue channel, the provider backends, and every shipped
  key meaning.
- **The live-verdict angle (NFR-15).** This cycle's probes, executed at
  the delivery: the suite + clippy at the changed tree, the interactive
  bottom-line composition (a running turn's screen — the clock alone on
  the bottom-most line, no span), the release-latest re-read at the close
  (`isLatest` for `v0.45.0`), the tag-target verification
  (`git rev-parse v0.45.0^{commit}`). The probes executed at this pass:
  the release `v0.44.0` Latest re-read (the releases API,
  `createdAt` 2026-11-05T23:18:45Z), the tree clean at `0d45242`, the
  instruction byte-exact (699 bytes, sha256 `5cb53102…`, re-verified at
  the pass's exit).
- **The docs angle (C-15, D-76).** The ledger's three documents and the
  work log; the products the standing sentence names (the source surfaces,
  the README/usage/help/architecture sentences naming the shipped shape,
  the CHANGELOG, the version files); nothing else.

## 4. Verification of the cycle (the gate table W-70)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-70 | FR-46 a/b/c — the mid-turn standing shape inverted at D-75's named sites (the clock block the screen's bottom-most row alone while the agent runs — no prompt span mid-turn; the prompt restored at every input-possible handoff), the scrolled screen kept whole (AS-73), the shipped contracts other than the swapped tenant intact, the docs rewritten at the sentences naming the old shape (D-76), the version carry to **0.45.0**, the delivery committed and pushed, the release standing latest | FR-46a, FR-46b, FR-46c, NFR-13, NFR-14, NFR-15, AS-73, AS-74, AS-75, TPR-28 | T-105, T-106, T-107 | ⬜ |

*W-70's execution record stands at the delivery close (T-105/T-106's
record); this pass's own record — the ledger write — verified the ground
live: the release `v0.44.0` Latest (re-read 2026-11-06,
`createdAt` 2026-11-05T23:18:45Z), the tree clean at `0d45242` with
`main` ≡ `origin/main`, the version files at 0.44.0, the instruction
byte-exact (699 bytes, sha256 `5cb53102…`). The gate counts re-derive at
the delivery tree (C-09; the recorded ground **663 unit + 6 integration,
0 failed** at the FR-45 close, the product tree unchanged since).*

## 5. The previous cycles' records (already retired; one shared record with the requirements' §12)

**"the prompt-input queueing feature's removal" (FR-45 a–c)** — closed,
2026-11-06: delivered at `b5deb9c` ("Remove the prompt-input queueing
feature"), the version carried 0.43.0 → **0.44.0**, the tag `v0.44.0`
verified on it, the release `v0.44.0 — the prompt-input queueing feature
removed` published and verified **Latest** (created
2026-11-05T23:18:45Z); gates **663 unit + 6 integration, 0 failed**, the
two dead-store clippy warnings recorded pre-existing. The trail's
follow-up `6f0390f` named the delivery (it names `b5deb9c` in the three
ledgers and the work log `2026-11-06_003.md`), and the ledger
re-alignment pass rode the same convention (`0d45242`: the design's three
`d5cbbde` sha slips corrected to `d5cbbbe`, the follow-up naming
verified, the delivered state re-read live). Work logs
`2026-11-06_002.md`–`_004.md`.

**"the codex-code backend — the installed codex CLI is driven the way the
claude-code backend drives `claude`, as a ninth `provider.kind`" (FR-44
a–e)** — closed, 2026-11-05: delivered at `122f320` (the backend
`src/ai/codex_code.rs` delegation-only, the nine-kind wiring, the docs,
the carry to 0.42.0) and `4fee899` (the FR-44d example amendment, the
carry to 0.43.0), named by the follow-ups `af82155` and `d5cbbbe`; the
FR-44e release `v0.43.0` verified **latest** (`isLatest: true`). Gates
**669 unit + 6 integration, 0 failed**, clippy 0 warnings. Work logs
`2026-11-05_010.md`–`_012.md`.

**"the codex access investigation" (FR-43)** — closed, 2026-11-05 (no
release — investigation and report): the verdict feasible for
delegation-style access (D-65), the shape recorded unbuilt (D-66); FR-44's
cycle built it (FR-44a, D-67). Work log `2026-11-05_008.md`.

**"the refactoring — contradictions and waste removed… and the prompt's
bottom row made prompt-only" (FR-41, FR-42)** — closed, 2026-11-05:
delivered at `eec12f9` and `064ce50`, the version carried to **0.41.0** by
`25fe13c`, named by the follow-up `3badc04`. Gates **647 unit + 6
integration, 0 failed**, clippy 0 warnings. FR-42's prompt-only rows are
the at-rest contract D-75 keeps; work logs `2026-11-05_006.md`/`_007.md`.

**"the display corrections — the `Message below` marker above the prompt;
the elapsed display kept and the selection unshifted" (FR-39, FR-40)** —
closed, 2026-11-05: delivered at `db68d3f`, named by `306f425`; the
version carried to 0.40.0 by `505097a`, recorded by `856c2bf`. FR-40's
scrolled composition is the scrolled-side contract AS-73 keeps whole.

**"the four display regions" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05:
delivered at `39afeb2`, named by the follow-up `fb46cc0`; gates 645+6.
**The bottom pair of the four-region shape is the contract this cycle's
FR-46 supersedes mid-turn** (the boundary's clause); the region order's
relative stacking survives the flip.

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
