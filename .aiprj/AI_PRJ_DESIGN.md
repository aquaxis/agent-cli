# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the terminal display fix — the screen's bottom-most line reads the
prompt while input is possible, and the spinner-and-elapsed clock while the
agent runs" (FR-46 a/b/c)
(`.aiprj/instructions.md`, reloaded 2026-11-06 09:16:49 +0900 — sha256
`5cb53102…`, 699 bytes, recorded in requirements §2; the tree carried
v0.44.0, its release `v0.44.0` verified Latest at the cycle's ground)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`.

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What was decided, and what it cost

The instruction's 最優先事項 is a display rewrite: 「ターミナルの最下位行を
入力が可能なときはプロンプト表示、エージェントが実行中のときはクルクルと
経過時間に修正する」, with the two refinement sentences (the bottom-most
line must always show one of the two faces, and the prompt display belongs
to the input-possible states only). The shipped shape the ask replaced:
while a turn ran the prompt span stayed pinned on the screen's bottom rows
and the clock block (the spinner-and-elapsed row first, the thinking rows
under it) composed **above** it (FR-37a/FR-40; the CHANGELOG's `[0.42.0]`
record: 「The bottom-most line is the prompt's alone, and the elapsed row
stays displayed above it」; BL-M54's file:line inventory). The design
decision was **D-75 — the mid-turn standing shape inverted, spanless**, and
it is **delivered** (requirements §13).

- **D-75 — the zone's composition recomposed with the clock as the
  bottom-most tenant.** The live screen mid-turn paints the same three
  transient regions in the same relative stacking (the peers' band above
  the clock block, the log tail above the band — FR-37's order, bottom-up
  unchanged), but the **clock block's last row lands on the screen's bottom
  row** and **no prompt span paints** while the zone is armed. The
  concrete sites, as written at `src/app.rs` (the delivery commit
  `ebe0aca`):
  1. **The spanless arm (`zone_arm`, `:1453`).** The zone now anchors with
     `span_top = height` — the beat's region is every row the beat owns,
     the clock block's last row on the screen's bottom row. The span's
     anchor bookkeeping stays in the struct (the sweep floor's role), and
     the caret fields keep the park target the spanless shape has.
  2. **The face order (`zone_block_rows`, `:4608`).** The status row
     (spinner + elapsed) is pushed **after** `thinking_rows`, so the
     block's last row is the spinner-and-elapsed row itself — the row that
     lands on the screen's bottom-most line, the instruction's クルクルと
     経過時間 clause; the reasoning rows stand above it. All rows keep the
     progress colour; the role-map's old spinner-first distinction (the
     indented reasoning rows taking `Role::Thinking`) is collapsed to the
     single progress colour the beat paints with — the shipped rule the
     shipped indicator used for the row it owned, with the reasoning rows
     reading as the block's detail either way.
  3. **The state-driven routing (`render_prompt_with_run`, `:5134`).**
     While armed, the armed branch paints the span **only for the approval
     role** (`Role::Confirm` — an input-possible state whose y/N prompt
     keeps its shipped takeover with its own span); the input prompt
     (`Role::PromptSymbol`) paints nothing while a turn runs — the prompt
     display belongs to the input-possible states (the refinement's
     sentence 3), so the armed screen's bottom rows belong to the clock
     block alone.
  4. **The approval answer's retirement (`zone_span_retire`, `:1776`,
     called at the approval submit arm `:3514`).** The one span that painted
     while armed was the approval's; the moment its y/N is answered (the
     turn continues running — the approval arm answers inside `Pending`),
     the span retires to the spanless anchor (`span_top = height`, the
     caret to the bottom row) and the next beat takes the screen's bottom
     row back for the clock. The reconcile arm (`:3717`) arms spanless and
     re-paints the span only for the awaiting-approval state it races.
  5. **The click rule re-bound (`click_hits_clock_block`, `:1309`).** With
     the status row the block's **last** row, the exclusion pivots: the
     toggle rows are the block's rows **above its last row** (the reasoning
     rows and the elision marker), the status row itself never toggles, and
     `clock_rows <= 1` still means no toggle surface. The pin
     (`the_zone_click_rule_hits_only_the_blocks_reasoning_rows`, `:7077`)
     re-derived with it, the fold-check inverted.
  6. **The disarm handoff unchanged where it was already the restored-shape
     arm.** `zone_disarm_plan` (`:1621`) keeps the `height−2` replay floor
     (D-54): the zone's rows erase, the record's tail lands above the fresh
     bottom row, and the shipped render draws the prompt there — the
     「入力が可能なときはプロンプト表示」 clause restored at every handoff
     (the idle release `:2973–2986`, the cancel paths, the approval deny).
     With `span_top = height` the plan's arithmetic needs no change: the
     zone painted every row it could paint, `painted_top` records it, and
     the replay paints rows above the bottom row's fresh face.
  7. **The scrolled screen kept whole (AS-73).** The scrolled pin keeps
     FR-39/FR-40's composition (`scrolled_block_plan` `:2102`), the
     budgets stand, and the instruction names the live terminal's bottom
     line.

  The cost: the pins binding the shipped composition re-derived together —
  the click rule and its pin (the one suite failure the re-derivation
  surfaced, fixed at the same commit's tree), the beat-plan order pin's
  reading (the four regions in the instruction's order, read with the
  floor at the clock), the additive budget pins standing (the beat plan's
  arithmetic is parameter-neutral). The suite at the delivery tree: **663
  unit + 6 integration, 0 failed**; clippy carrying the two pre-existing
  dead-store notes in test code (verified pre-existing at the FR-45 close).

- **D-76 — the docs followed the swap at the sentences naming the shipped
  shape.** The CHANGELOG's `[Unreleased]` became the `[0.45.0]` entry
  recording the swap in user language (while the agent runs the
  spinner-and-elapsed clock is the screen's bottom-most line, no prompt
  display mid-turn — the typed line's echo stands in the log from the
  submit — and the prompt takes the bottom back when input is possible;
  `Esc`/`Ctrl-C` and every other behaviour unchanged). The shipped
  sentences naming the old owner followed (BL-M55's surface list):
  `README.md:13`'s 「the spinner's clock stays the screen's bottom row」
  corrected to the peer rows reading above the clock block;
  `README.md:26`'s wheel bullet corrected to 「the spinner-and-elapsed
  clock on the terminal's bottom line while a turn runs」;
  `doc/usage.md:77–79`'s Subagents paragraph corrected to 「the spinner's
  clock holds the screen's bottom row while the agent runs, and the prompt
  takes it back when input is possible again」; `doc/usage.md:418`'s wheel
  paragraph corrected to 「On both screens the pinned block sits above the
  running clock mid-turn, and keeps the bottom row at every
  input-possible state」; `doc/architecture.md`'s indicator/turn-zone
  paragraphs corrected to the swapped composition. The `/help` subagent
  sentence (`src/app.rs`) follows the code the same commit carries. The
  CHANGELOG's `[0.40.0]`/`[0.42.0]` entries stand as history — no rewrite.

- **D-77 — the completion repeated the standing delivery machinery
  (AS-75).** The minor version carried **0.44.0 → 0.45.0** (`Cargo.toml`,
  `Cargo.lock` via `cargo update -p agent-cli --precise 0.45.0`, the
  CHANGELOG's `[0.45.0]` heading), the delivery commit was
  `ebe0aca` ("Fix the terminal's bottom line: the prompt when input is
  possible, the running clock while the agent runs" — pushed
  `92c43fd..ebe0aca`), the tag `v0.45.0` verified on it, and the GitHub
  release `v0.45.0 — the terminal's bottom line shows the prompt when
  input is possible and the running clock while the agent runs` published
  from the tag and verified **Latest** (`gh release list`'s Latest label;
  the release API read, `createdAt` 2026-11-06T00:55:05Z). The trail rule
  governs the sha naming (the follow-up commit names `ebe0aca` from
  behind it).

The instruction's clauses map one-for-one onto the decisions:
「入力が可能なときはプロンプト表示」 → D-75's at-rest shape (the shipped
idle prompt and the approval takeover, restored at every handoff);
「実行中のときはクルクルと経過時間」 → D-75's inverted mid-turn shape;
the refinement sentence 2 (必ず one of the two faces) → the spanless arm's
invariant (the armed face is the clock alone, the input-possible face the
prompt — no third face at the bottom line); sentence 3 (プロンプト表示は
入力下の状態のときのみ) → D-75 site 3's state-driven routing;
ドキュメント更新 → D-76; the method C-12 → §3's angles;
マイナーバージョンアップ + commit, push + リリース最新 → D-77.
「example更新」 names the example configuration: its `[ui]` block names
the spinner's existence only (`show_progress`'s comment — BL-M55's
verified absence of a bottom-line sentence), so no example write existed.

## 2. The touchpoints this cycle touched (TPR-28)

| # | Touchpoint | Evidence (the files written this cycle) |
|---|---|---|
| TPR-28 | `src/app.rs` — `zone_arm` (`:1453`, the spanless anchor), `zone_span_retire` (`:1776`, new), `click_hits_clock_block` (`:1309`, the pivoted exclusion), `zone_block_rows` (`:4608`, the status-last face), `render_prompt_with_run` (`:5134`, the state-driven routing), the approval answer arm (`:3506–3514`), the reconcile arm (`:3717–3733`), the click pin (`:7077`); `CHANGELOG.md` (`[0.45.0]` entry); `README.md` (`:13`, `:26`); `doc/usage.md` (`:77–79`, `:418`); `doc/architecture.md` (the indicator/turn-zone paragraphs); `/help`'s subagent sentence (`src/app.rs`); `Cargo.toml`/`Cargo.lock` (0.45.0) | The delivery commit `ebe0aca`'s diff; the suite run at the delivery tree |

## 3. Analysis verdicts (C-12) — the multi-angle analysis of the swap

- **The mechanism angle.** The mid-turn shape is one composed drawing — the
  zone beat's — and the swap was one re-derivation: the arm anchors
  spanless, the face order flips, the routing gates the span by state, the
  click exclusion pivots to the last row. No new writer, no new event
  type, no new state machine; the release arms already drew the prompt
  after the disarm and needed no change.
- **The contracts angle (NFR-14).** The shipped contracts stand except the
  bottom-most row's tenant mid-turn: the four regions keep their relative
  stacking, FR-27's toggle keeps its rows (the reasoning set, pivoted),
  the approval takeover keeps its span and retires it on the answer, the
  idle prompt and the cancel paths keep their shapes, the four exit seams
  and the teardown/drain, the mouse-capture and clipboard contracts, the
  queueing feature's removal (FR-45's contract) and the codex-code
  backend — none touched by the delivery.
- **The scope angle (§3.1).** Not touched: the spinner's frames and
  format, the thinking rows' presence, the peers' band's face, the
  scrolled screen's composition (AS-73 — the delivery kept it whole), the
  disarm's replay handoff, `--exit-when-idle`'s drain (FR-32), the queue
  channel, the provider backends, every shipped key meaning.
- **The live-verdict angle (NFR-15).** Executed probes: the suite at the
  delivery tree (**663 unit + 6 integration, 0 failed** — the first run
  surfaced the one re-derivable pin, the click rule's, fixed at the same
  tree), clippy at the delivery tree (the two pre-existing dead-store
  notes), the release-latest verification (`gh release list`'s Latest
  label for `v0.45.0`; the API read's `createdAt` 2026-11-06T00:55:05Z),
  the tag target (`ebe0aca`).
- **The docs angle (C-15, D-76).** The ledger's three documents and the
  work log; the products (the source surfaces, the README/usage/
  architecture sentences, `/help`, the CHANGELOG, the version files);
  nothing else. A session artifact (`src/app.rs.patch`, then the release
  notes file) was removed from the commit and the tree — the delivery
  commit carries only the seven product files the write surface names
  (verified: the amended commit's file list shows 7 files, no `.patch`).

## 4. Verification of the cycle (the gate table W-70)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-70 | FR-46 a/b/c — the mid-turn standing shape inverted at D-75's named sites (the clock block the screen's bottom-most row alone while the agent runs — no prompt span mid-turn; the approval span keeping its takeover and retiring on the answer; the prompt restored at every input-possible handoff), the scrolled screen kept whole (AS-73), the shipped contracts other than the swapped tenant intact, the docs rewritten at the sentences naming the old shape (D-76), the version carry to **0.45.0**, the delivery committed and pushed (`ebe0aca`), the release standing latest | FR-46a, FR-46b, FR-46c, NFR-13, NFR-14, NFR-15, AS-73, AS-74, AS-75, TPR-28 | T-105, T-106, T-107 | ✅ |

*W-70's execution record: the suite stands green at the delivery tree
(**663 unit + 6 integration, 0 failed** — the click rule's pin re-derived
at the same tree), clippy stands its two pre-existing dead-store notes,
the tag `v0.45.0` verified on `ebe0aca`, the release `v0.45.0` verified
**Latest** (created 2026-11-06T00:55:05Z) — these clauses are the
acceptance criterion AC-39's, which §11 of the requirements traces here.
The delivery commit `ebe0aca` carries the seven product files only (the
amend verified the `.patch` session artifact out) — the trail rule leaves
its own naming to the commit that follows.*

## 5. The previous cycles' records (already retired; one shared record with the requirements' §12)

**"the prompt-input queueing feature's removal" (FR-45 a–c)** — closed,
2026-11-06: delivered at `b5deb9c` ("Remove the prompt-input queueing
feature"), the version carried 0.43.0 → **0.44.0**, the tag `v0.44.0`
verified on it, the release `v0.44.0 — the prompt-input queueing feature
removed` published and verified **Latest** (created
2026-11-05T23:18:45Z); gates **663 unit + 6 integration, 0 failed**, the
two dead-store clippy warnings recorded pre-existing. The trail's
follow-up `6f0390f` named the delivery, and the ledger passes `0d45242`
and `2557f7f` re-aligned and recorded the re-load. Work logs
`2026-11-06_002.md`–`_006.md`.

**"the codex-code backend — the installed codex CLI is driven the way the
claude-code backend drives `claude`, as a ninth `provider.kind`" (FR-44
a–e)** — closed, 2026-11-05: delivered at `122f320` (the backend
`src/ai/codex_code.rs` delegation-only, the nine-kind wiring, the docs,
the carry to 0.42.0) and `4fee899` (the FR-44d example amendment, the
carry to 0.43.0), named by the follow-ups `af82155` and `d5cbbbe`; the
FR-44e release `v0.43.0` verified **latest**. Gates **669 unit + 6
integration, 0 failed**, clippy 0 warnings. Work logs
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
**The bottom pair of the four-region shape was the contract this cycle's
FR-46 superseded mid-turn** (the boundary's clause); the region order's
relative stacking survived the flip.

Earlier cycles stand in the requirements document's §12.
