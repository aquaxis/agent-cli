# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the removal — the prompt input queueing feature (FR-35, v0.38.0) is
deleted: during a running turn the REPL again ignores plain typed input,
the release gate's queue conjuncts and the queue's face go, the CHANGELOG
records the change, the version carries to 0.44.0, and the release stands
latest"
(FR-45 a/b/c)
(`.aiprj/instructions.md`, reloaded 2026-11-06 02:02:15 +0900 — the ask is
the 最優先事項 「プロンプトの入力のキューイング機能を削除する」; the tree
carries v0.43.0, its release v0.43.0 verified latest, per requirements
§2/§13)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`.

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What is decided, and what it costs

The instruction's 最優先事項 is a removal: 「プロンプトの入力のキューイング
機能を削除する」. The feature named is the one FR-35 delivered in v0.38.0
(design §5, below): the `PromptState::Pending` editor left open during a
turn, `Enter` enqueuing `AgentInput::UserPrompt` onto the agent's FIFO
channel mid-turn, the `[queued: N]` notice, the `queue_release_allowed`
release gate, the approval draft pair serving queuing, the `!`/`/` mid-turn
refusal, and the queue's pins. The design decision is **D-72 — queueing is
removed at the mechanism's own sites, contract-backward**, so the turn-time
features the contracts keep — the `Esc`/`Ctrl-C` cancel and its shipped
meaning (in the tree since before FR-35), the approval takeover, the four
display regions, the turn zone, the tracked clock, FR-39/FR-40's geometry,
FR-42's prompt-only rows — stand exactly as shipped. What FR-35 added is
subtracted; nothing else the boundary §6 names is changed by it (NFR-14).
The completion rides the standing delivery sentence: **D-73** fixes the
docs surface, **D-74** the version-carry/commit/push/release shape.

- **D-72 — the removal subtracts FR-35's mechanism down to the
  pre-FR-35 discipline (AS-71).** The removal sites, each bound to the
  shipped-code inventory (BL-M52's file:line):

  1. **Plain input during a turn is ignored again.** The `Pending`
     branches of the raw and piped loops stop accepting agent prompts: the
     submit path's echo, the `AgentInput::UserPrompt` enqueue
     (`src/app.rs:3595` and the piped loop's own) and the mid-turn
     refusal notices for `!`/`/` lines (`src/app.rs:3511`) are deleted —
     the boundary rule `!` and `/` run only at the idle prompt (FR-35a's
     one) becomes the standing rule for every line because no line is
     accepted mid-turn at all. The pre-queue contract is the CHANGELOG's
     own pre-FR-35 record: while the agent is thinking or running a tool,
     every keypress except `Esc` and `Ctrl-C` is ignored.
  2. **The queue mechanics are deleted.** `queued_count`
     (capacity-dip accounting, `src/app.rs:1320`),
     `queue_release_allowed` (`:1335`), `paint_queue_status` (`:1345`),
     `QUEUED_NOTICE_PREFIX`/the `[queued: N]` notice line (`:1364`) —
     and the release rule collapses to the shipped reading: `Pending`
     releases on the turn's terminal event (the idle notification
     consumed, `prompt_state = Ready`, `zone_disarm()` and the repaint
     as shipped), with no third conjunct to consult a queue that no
     longer exists. The stale-notification drain in front of `Pending`
     and the self-healing park arm keep their pre-queue roles for the
     idle path.
  3. **The approval-time draft pair goes with the behaviour it served.**
     `DraftSnapshot`/`snapshot_draft`/`restore_draft`
     (`src/app.rs:2610–2624`) and the arms that call them stood to
     protect the half-typed queued draft across an approval prompt
     (FR-35's approval snapshot, D-49); with queuing gone no mid-turn
     draft exists to protect, so the pair and its arms are deleted and
     an approval prompt takes the editor in the pre-queue shape.
  4. **The queue's pins go with the mechanism.** The agent-side boundary
     pin `a prompt queued behind a turn survives the user cancelling
     that turn` (`src/agent.rs:1419`, the FR-35b comment `:1457`), and
     the four unit pins —
     `a_prompt_typed_during_a_turn_is_queued_and_arrives_while_it_runs`
     (`src/app.rs:6634`), `stale_idle_signals_do_not_withhold_a_prompt_
     typed_during_a_turn` (`:6707`),
     `queue_release_gate_answers_every_combination` (`:8815`),
     `queued_count_tracks_a_live_channel_without_receiving`
     (`:8836`). Every other pin stands; the gate counts re-derive at the
     close (NFR-13: the unit count drops by four).

  The costs are the costs FR-35's CHANGELOG entry records, inverted: a
  prompt typed during a turn is again ignored (not lost on screen — the
  line is simply not accepted), and the pre-queue contract stands again.

- **D-73 — the docs surface is the CHANGELOG's `[0.44.0]` entry, and
  only it.** The shipped READMEs and `doc/usage.md` carry no
  queue-feature sentence (BL-M53's verified text search: their turn-time
  sentences name only `Esc`/`Ctrl-C` stopping, scrollback and the
  `Message below` marker), so no user-facing README/usage sentence needs
  correcting. The CHANGELOG's `[Unreleased]` becomes the `[0.44.0]`
  entry recording the removal in user language (what changed for the
  user; `Esc`/`Ctrl-C` and everything else unchanged). The sentences in
  `doc/architecture.md` §7 (the shutdown drain answering queued
  deliveries) and `doc/troubleshooting.md` (the peer-prompt queue) name
  other mechanisms (§3.1's scope) and stand. The CHANGELOG's `[0.38.0]`
  entry stands as history — no history rewrite (§3.1).

- **D-74 — the completion is the standing delivery sentence's shape
  (AS-72).** The minor version carries **0.43.0 → 0.44.0**
  (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's `[0.44.0]` heading), the
  delivery commit(s) carry the removal, the CHANGELOG entry, the release
  files and the ledgers' record, the push goes to `origin/main`, and the
  release is published and made the **latest** version — the exact
  machinery FR-44e's delivery already exercised and this cycle's
  verification re-proved (`isLatest` programmatic read, BL-M53; D-71
  below). The trail rule governs the sha naming (a commit's sha is
  nameable only from the commit that follows it).

The instruction's clauses map one-for-one onto the decisions: the
最優先事項 削除 → D-72's sites 1–4; ドキュメント更新 → D-73; the
method C-12 → the multi-angle reconciliation of §3;
マイナーバージョンアップ + commit, push + リリース最新 → D-74.
「example更新」 names the example configuration: it carries no queue
surface (BL-M53's verified search — the example's provider blocks stand
as FR-44d shipped them), so no example write exists.

## 2. The touchpoints this cycle touches (TPR-27)

| # | Touchpoint | Evidence (the files read in this cycle) |
|---|---|---|
| TPR-27 | The REPL's queue mechanics and their surface echo: `src/app.rs` — `queued_count` (`:1320`), `queue_release_allowed` (`:1335`), `paint_queue_status` (`:1345`), `QUEUED_NOTICE_PREFIX` (`:1364`), the raw loop's Pending submit arm (`:3505–3609`, the enqueue `:3595`), the raw loop's idle release arm with the queue conjuncts (`:3070–3098`), the approval snapshot arms (`:3131–3134`, `:3715`), the piped loop's mirror arms (the stdin read `:4101`, its Pending submit/release arms); the pair `DraftSnapshot`/`snapshot_draft`/`restore_draft` (`:2610–2624`); the four unit pins (`:6634`, `:6707`, `:8815`, `:8836`); `src/agent.rs`' boundary pin (`:1419–1457`); `doc/CHANGELOG.md`'s `[Unreleased]`/`[0.38.0]` sections; the version files (`Cargo.toml`, `Cargo.lock`); the untouched surfaces verified: the two READMEs' keymap rows, `doc/usage.md`'s keys section, `src/cli.rs:130–151` (`--exit-when-idle`'s FR-32 sentence — FR-32 is out of scope) | The files' own lines, read in this cycle; BL-M52/BL-M53's records |

## 3. Analysis verdicts (C-12) — the multi-angle analysis of the removal

- **The mechanism angle.** The queue is one mechanism in three layers —
  the editor state's mid-turn opening, the channel's enqueue-from-the-REPL,
  and the release gate — and the removal must take all three or leave the
  restored contract broken: a gate-only deletion would return the prompt
  while a typed line still rode the channel. D-72 names the layers so no
  path is left half-removed.
- **The contracts angle (NFR-14).** The boundary §6's items stand except
  FR-35 itself: the bare-`Enter` guard, the cancel keys' meanings, the
  approval takeover, the outcome rows' recording, the four exit seams, the
  teardown/drain, the mouse-capture and clipboard contracts, the peer
  rows' face and stacking, the four display regions, FR-39/FR-40's
  geometry, FR-42's prompt-only rows, FR-41's collapsed shapes, the
  codex-code backend — none is touched; D-72's sites are named so the
  diff cannot reach them.
- **The scope angle (§3.1).** The queue named for removal is the REPL's
  prompt-input queue; **not** touched: the peer prompt deliveries'
  socket-acked queueing, the shutdown drain's truthful answers to queued
  deliveries (`run_headless`'s own mechanism, the sentences
  `doc/architecture.md` §7 carries), `--exit-when-idle`'s
  idle-drain semantics (FR-32), and the agent input channel itself (the
  cancel path rides it).
- **The live-verdict angle (NFR-15).** This cycle's probes: the
  release-latest verification (`isLatest` read from the releases API),
  the tag-target verification; the gate suite re-runs at the removal
  tree's close.
- **The docs angle (C-13, D-73).** The ledger's three documents and the
  work log; the products the standing sentence names (the source
  surfaces, the CHANGELOG, the version files); nothing else.

## 4. Verification of the cycle (the gate table W-69)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-69 | FR-45 a/b/c — the queue mechanics deleted at D-72's named sites, the restored pre-queue contract standing, the shipped contracts other than FR-35 intact, the CHANGELOG `[0.44.0]` entry written, the version carry to **0.44.0**, the delivery committed and pushed, the release standing latest | FR-45a, FR-45b, FR-45c, NFR-13, NFR-14, NFR-15, AS-70, AS-71, AS-72, TPR-27 | T-102, T-103, T-104 | ⬜ |
| W-68 | FR-44d — the post-delivery amendment delivered: the example's sample models refreshed to the newest live-evidenced ids, every shipped surface quoting the example follows, the code defaults untouched (NFR-14), the version carry to **0.43.0** (the CHANGELOG `[0.43.0]`), the commit and the push per the standing clause | FR-44d, NFR-14, AS-69, D-70, TPR-26 | T-100 | ✅ |

*The W-68 row stands ✅ at the recorded v0.43.0 tip (BL-M53's release
state). The W-69 row runs at the delivery close: the gates re-run at the
removal tree (C-09, the counts re-derived per NFR-13), the CHANGELOG
entry and the version carry land with the delivery commit(s), and the
release verification re-asks the `isLatest` probe at 0.44.0 per D-74;
the row is marked ✅ only when every clause stands executed and recorded —
these clauses are the acceptance criterion AC-38's, which §11 of the
requirements traces here.*

## 5. The previous cycles (recorded after their documents were retired)

**"the codex-code backend — the installed codex CLI is driven the way the
claude-code backend drives `claude`, as a ninth `provider.kind`" (FR-44 a–e)**
— closed, 2026-11-05: delivered at `122f320` (the backend
`src/ai/codex_code.rs` delegation-only, the nine-kind wiring, the docs,
the carry to 0.42.0) and `4fee899` (the FR-44d example amendment, the
carry to 0.43.0), named by the follow-ups `af82155` and `d5cbbbe`; the
FR-44e release **v0.43.0 verified latest** (requirements §2/§13, D-71).
Gates **669 unit + 6 integration, 0 failed**, clippy 0 warnings. This
cycle's tree state (v0.43.0) is their delivered ground: the codex-code
backend stands untouched by the removal.

**"the codex access investigation" (FR-43)** — closed, 2026-11-05 (no
release — investigation and report): the verdict feasible for
delegation-style access (D-65), the shape recorded unbuilt (D-66); work
log `2026-11-05_008.md`. The next cycle built D-66 (FR-44a, D-67).

**"the refactoring — contradictions and waste in the source are removed,
`src/ai`'s provider error chain and the tools' test context each collapse
to one implementation, clippy falls silent, and the documents follow the
source; and the prompt's bottom row is prompt-only" (FR-41, FR-42)** —
closed, 2026-11-05: delivered at `eec12f9` and `064ce50`, the version
carried to 0.41.0 by `25fe13c`, named by the follow-up `3badc04`. Gates at
the close **647 unit + 6 integration, 0 failed**, clippy silent. FR-42's
prompt-only rows are the display contract the removal re-verifies at the
close (a `PromptState::Pending` block remains one whose bottom edge is the
row FR-42 pinned).

**"the display corrections — the `Message below` marker above the prompt;
the elapsed display kept and the selection unshifted" (FR-39, FR-40)** —
closed, 2026-11-05: delivered at `db68d3f`, named by `306f425`; the version
afterwards carried to 0.40.0 by `505097a`, recorded by `856c2bf`.

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)** — closed, 2026-11-04,
delivered at `c02862d` ("Prompts typed while a turn runs are queued and run
in order when it ends"), `Cargo.toml` **0.38.0**, the tag `v0.38.0`, the
GitHub release published 2026-11-04T03:52:18Z; the gates at the close **633
unit + 6 integration, 0 failed**. **The mechanism D-72 removes**: the
`Pending`-state open editor and its mid-turn `UserPrompt` enqueues, the
`[queued: N]` notice, the `queue_release_allowed`/`queued_count` gate
pair, the approval draft pair, the boundary pin D-45 added, and the queue
pins T-74/T-75 delivered. This cycle's closure is the removal's own
record, FR-45a — the requirement itself stays in §12's history of shipped
contracts, and the boundary §6 names the one clause it no longer governs.

Later records stand in the requirements document's §12: the subagents'
auto-exit (FR-32–FR-34, v0.37.0 on `5a2d7f0`); the multi-angle subagent
analysis cycles (FR-31, FR-29/FR-30); the click rule's exclusion and the
cancelled-by-user display (FR-27/FR-28, v0.34.0); the scroll marker's face
(FR-26, v0.33.0); the running display (FR-22–FR-24, v0.32.0); the mouse
defects (v0.31.0); **add strata** (FR-13–FR-17, v0.30.0–0.30.2);
v0.29.0–v0.25.0 and the 機能追加 addenda (v0.26.0).

- **D-71 — the published release is latest.** FR-44e's obligation, the
  release state's own verification: the release carrying the version the
  trail's tip recorded is programmatically confirmed **latest** — at this
  update, `v0.43.0` on the GitHub release list (`isLatest: true`,
  created_at 2026-11-05T10:57:38Z; `v0.42.0`/`v0.41.0` not latest, the
  same API read) — with the tag verified on the delivery commit `4fee899`
  (BL-M53's evidence). FR-44e stands closed, no write made beyond the
  ledgers' closures: the state was verified, not adjusted. The removal
  cycle's own carry (FR-45c, D-74) re-asks it at 0.44.0.
