# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)
(`.aiprj/instructions.md`, rewritten 2026-11-04 09:23)
Paired with: `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked
Article 3 of the AI Coding Guidelines: record progress updates in this
file whenever a task changes state.

---

## Overview

The instruction (rewritten 2026-11-04 09:23:37 +0900, verified byte-exact on
2026-11-04; 420 bytes, sha256 `26c96f3e630d6a61…`) carries one ask in three
parts: an agent mid-work must accept a typed prompt (**FR-35a**), the typed
prompt must be queued — not executed, not lost (**FR-35b**) — and once the
running work completes the queued prompts run in arrival order
(**FR-35c**). The baseline verification (BL-M30) located today's loss on
the REPL side: during `Pending` the input loop honours only Esc / Ctrl-C
(`src/app.rs:2740–2764`), stdin reads are suppressed, and a shipped pin
binds the loss (`src/app.rs:5436–5440`). The queue itself already exists on
the agent side — the input channel is a bounded FIFO (capacity 32) the
loop drains one input per iteration (`src/app.rs:421/:734`,
`src/agent.rs:233`) — so the cycle's deliverable is the REPL's acceptance
during `Pending` plus the drain/count display, not a new structure.

One correction to the ledger was carried in at the cycle's start (BL-M31):
the previous cycle's FR-34 close-out is re-recorded as **v0.37.0** — the
release commit `5a2d7f0` ("Release 0.37.0: the compaction repair carries the
version") landed after `ddeaf67`, the tag `v0.37.0` stands on the release
commit, and the CHANGELOG carries `[0.37.0]`. The previous documents'
"tag `v0.36.0` on `ddeaf67`"/"close-out commit pending" record was written
before the release commit landed and is superseded. One more correction
(BL-M32) was carried at this cycle's second update, and it narrows
BL-M31's supersession: BL-M31's rewrite had dropped the true "tag
`v0.36.0` on `ddeaf67`" fact — the tag **does** stand on `ddeaf67` (the
record BL-M31 deleted was the true tag state) and is restored; what
BL-M31 truly superseded is only the "close-out pending" part. The version
trail's "v0.35.0/v0.36.0" *release* labels were the false records and
stand recast as the untagged in-tree versions they are (recorded with
BL-M32 in the corrections section).

Tasks: T-74 the FR-35 implementation (✅), T-75 the pins (✅). The cycle is
delivered at commit `c02862d` ("Prompts typed while a turn runs are queued
and run in order when it ends") and versioned **0.38.0**; the gates ran
green at the close — **633 unit + 6 integration, 0 failed**, clippy
identical to the shipped baseline (3 bin + 8 test-target warning lines,
the shipped four kinds).

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-74 | FR-35's implementation: the `Pending` branch's acceptor — the all-keys-ignored path replaced by line input read the way `Ready` reads it, `Enter` enqueueing `AgentInput::UserPrompt` onto the existing channel by the shipped `send().await`; the queue-count added to the display-only updates so the user sees what is waiting; the cancel keys keeping their meaning (the `turn_in_flight` guard unchanged, the typed line and the queue untouched by a cancel); the drain standing as shipped — the loop takes the next queued input after each turn, the prompt returning only when the queue is empty | FR-35, FR-35a, FR-35b, FR-35c, NFR-14 | design §1 (D-49, D-50, D-51), §2 (TPR-20a/20b) | — | ✅ |
| T-75 | The pins and the pending-loss pin's supersession: the shipped pin `the second prompt sent while the input loop is Pending does not arrive` (`src/app.rs:5436–5440`) **updated in place** to the queued contract — the specification change recorded (§T-75); the new pins — the queued prompt arrives while the turn runs, the FIFO order, the automatic drain, the cancel-keeps-queue boundary, the queued prompt keeping a flagged agent alive (the FR-32 pin inheriting the feature untouched); gates green (unit + integration, 0 failed; clippy identical to the baseline); the delivery committed per the user's direction | FR-35, FR-35a, FR-35b, FR-35c, NFR-13, NFR-14, AC-32 | design §1 (D-49), §3, §4 (W-53–W-56) | T-74 | ✅ |

Sections below the table are the execution record (Article 3) — written as
each task's status changes.

## The cycle's ledger corrections (carried at the cycle's start)

- **BL-M31** — FR-34's close-out re-recorded as v0.37.0: the release commit
  `5a2d7f0` ("Release 0.37.0: the compaction repair carries the version";
  parent `ddeaf67`; authored 2026-11-03 23:28:17 +0900) carries the tag
  `v0.37.0`, the CHANGELOG `[0.37.0]` (Fixed — the compaction repair), and
  `Cargo.toml` reads `0.37.0`. The three `.aiprj` documents were corrected
  to this delivery state before T-74 started, keeping the ledger honest
  (C-08).
- The v0.37.0 GitHub release's publication time stands corrected from the
  previous ledger's `2026-11-03T14:17:12Z` to the live-verified
  `2026-11-03T14:33:12Z` (`gh` release view; the commit authored
  14:28:17Z precedes the publication — the earlier "14:17:12" figure
  matched v0.36.0's neighbourhood and no live timestamp).
- **BL-M32** (this update) — the version trail's release labels for
  FR-32/FR-33 are superseded: no `v0.35.0` tag exists (the tags jump
  v0.34.0 → v0.36.0, local and remote), the lightweight tag `v0.36.0`
  dereferences to `ddeaf67` (the FR-34 code commit; `af48be5` is its
  ancestor, never a tagged tip), and the GitHub releases carry no v0.35.0.
  So FR-32 `7fe1370` and FR-33 `af48be5` are the in-tree versions
  0.35.0/0.36.0, never tagged, never released, and the trail's
  "v0.35.0 → v0.36.0" release pair is recast commit-only — while
  BL-M31's deletion of the older documents' "tag `v0.36.0` on `ddeaf67`"
  fact is itself corrected: that record stood and is restored. Evidence:
  `git tag`, `git ls-remote --tags`, `git rev-parse v0.36.0^{commit}`,
  each sha's `Cargo.toml`/`CHANGELOG.md`, `gh release list`.

## T-74 — FR-35's implementation (closed ✅, delivered at `c02862d`)

Grounded in the baseline (BL-M30) and the design's decisions D-49–D-51.
The shape, verified against the tree before the design moved:

- **The acceptor** (`src/app.rs`, the `Pending` branch `:2740–2764`): the
  ignore path (`continue` on every non-cancel key) is replaced by the line
  input the `Ready` branch runs — the same `InputState` edit buffer and
  renderer (AS-50), so the typed line builds and edits during the turn.
  The cancel keys (Esc / Ctrl-C) keep their exact meaning and precedence —
  a `begin_cancel` governs the running turn only (AS-48; the guard is the
  shipped `turn_in_flight`).
- **The enqueue**: `Enter` with non-empty, non-all-space text sends
  `AgentInput::UserPrompt` by the same `send().await` the REPL's other
  call sites use (`src/app.rs:2906/3193` shapes; AS-49 for the bound).
  The enqueued line clears the edit buffer; nothing else on the running
  turn's display is taken.
- **The count** (D-51): the display-only updates carry the queue's length
  so the user sees what the channel holds while the turn runs; the queued
  prompts are readable only as their count (an enqueued line's text is no
  longer editable — it is the channel's property, not the REPL's).
- **The drain**: no agent-side change — the loop consumes one input per
  iteration (`src/agent.rs:233`); after a turn's end the next queued input
  is taken automatically, each prompt getting its own turn and its own
  `Done`; the prompt state returns to `Ready` only when the channel holds
  no further input (D-50). The FR-32 quiet check stays after each
  processed input and sees the post-drain channel — the interactive REPL
  does not carry the flag (the shipped construction).

Depends on: nothing (the first task of the cycle).

**Delivered at `c02862d` (2026-11-04):** the acceptor stands in both REPL
loops — the raw interactive loop's `Pending` branch feeds every non-cancel
key to the shared `InputState` (Tab's slash completion included), and the
piped/line-mode loop reads stdin lines during a turn on the same terms
(the shipped all-keys-ignored path is gone). `Enter` enqueues only
`classify_line → Line::Prompt` text as `AgentInput::UserPrompt` via the
shipped `send().await`; `!`/`/`/empty lines are refused with an inline
`[info]` notice and stay editable. Helpers added: `queued_count`
(`max_capacity() − capacity()` — tokio 1.54.1 has no `Sender::len`),
`queue_release_allowed(pending, queue_len, turn_in_flight)` (the release
gate; the `turn_in_flight` conjunct closes the dequeue race), and
`paint_queue_status` (the one-line `[queued: N]` face, D-51). The idle arm
stays Pending when the gate fails and stamps `pending_since`; a
low-priority 120 ms `park_timer` tick arm (last arm of the `biased`
select, both loops) is the self-healing release for stale idle
notifications. The half-typed draft survives a tool-approval pop via
`DraftSnapshot`/`snapshot_draft`/`restore_draft` (restored after y/N and
after the Esc refusal). The display task was left untouched — giving it a
sender clone would change the exit seam (channel-close on
`drop(input_tx)`); TPR-20b's no-change record stands.

Design trace: design §1 (D-49 the acceptor and enqueue, D-50 the drain,
D-51 the count), §2 (TPR-20a the acceptor/touchpoint, TPR-20b the drain's
no-change record, TPR-20c the cancel keys), §3 (the analysis verdicts the
shape stands on).

## T-75 — The pins and gates (closed ✅, depends on T-74)

- **The superseded pending-loss pin** (`src/app.rs:5436–5440`, shipped):
  updated in place as
  `a_prompt_typed_during_a_turn_is_queued_and_arrives_while_it_runs` — the
  typed prompt arrives on the agent's input channel **while the turn runs,
  in order** (the FIFO extended to a third prompt). A specification change:
  the shipped pin asserted the loss this cycle overturns; the rewrite
  travelled in the same commit as the feature (NFR-13) and was made to fail
  against the shipped shape first (`queued prompt timeout: Elapsed`).
- **The new pins**, all green at the close: the stale-idle pin
  `stale_idle_signals_do_not_withhold_a_prompt_typed_during_a_turn`
  (replaces the shipped `stale_idle_signal_is_drained_before_pending`, the
  inverted pre-FR-35 contract); the approval-draft snapshot pin
  `approval_snapshot_roundtrip_preserves_the_half_typed_draft`; the
  agent-side cancel-keeps-queue pin
  `a_queued_prompt_survives_a_cancelled_turn_in_front_of_it`
  (`src/agent.rs:1412` neighbourhood, the FR-32 pin's neighbour — the
  cancelled turn's tool call is cancelled, the queue's next prompt runs a
  full turn afterwards; `src/agent.rs:195–199` the unwind clears the
  flag); the helper unit pins `queue_release_gate_answers_every_combination`
  (the full truth table) and `queued_count_tracks_a_live_channel_without_
  receiving`; the FR-32 pin `a_queued_prompt_keeps_a_flagged_agent_alive`
  inheriting the feature untouched.
- **The stop boundary** (W-55'): the shipped stop gate still refuses a
  genuinely stopping session's queue; nothing in the acceptor bypasses it
  (`src/agent.rs:268–302`, the peer branch's `stopping` check).
- **The gates** (W-56, all green at the close): **633 unit + 6 integration,
  0 failed** (`--locked --all-targets`); clippy identical to the shipped
  baseline (3 bin + 8 test-target warning lines, the four shipped kinds —
  no new warning); `CHANGELOG.md` `[0.38.0]` the cycle's entry; `Cargo.toml`
  0.38.0; the `.aiprj` documents mutually consistent (C-08); the delivery
  committed as `c02862d` ("Prompts typed while a turn runs are queued and
  run in order when it ends") per the user's direction (AC-32); pushed to
  `origin/main` (`5a2d7f0..c02862d`) with the lightweight tag `v0.38.0`
  (the v0.36.0/v0.37.0 precedent), the GitHub release published
  2026-11-04T03:52:18Z.

Depends on: T-74.

Design trace: design §1 (D-49's pin consequence), §3 (the cancel and
exit/quiet angles), §4 (W-53–W-56 the gate rows T-75 executes).

## The post-cycle follow-ups (closed ✅, 2026-11-04)

- **The ledger tracked by git** (the user's directive "ドキュメントも更新",
  after the cycle's close): the `.aiprj` exclusion was lifted — the three
  documents, the work logs (`AI_LOG/`, 2025-06-17 → 2026-11-04), the rules
  and the shipped example `config.toml`/`README.md` joined the tree at
  commit `881eda9` ("Track the .aiprj project ledger: the three documents,
  work logs, rules"; 133 files, +12,246/−6, pushed
  `c02862d..881eda9 main → origin/main`). `.aiprj/instructions.md` alone
  stays untracked (`.gitignore`, with the comment recording the reason:
  every work log embeds its full text per Art. 7). Earlier work-log
  statements that `.aiprj` is untracked are the records of their own days
  and stand unedited. Secret scan clean before the commit (`config.toml`
  names env variables only, no key material); the ledger lives in the
  repository from here on, so its updates ride the normal commits.
- **The version carried forward, v0.39.0** (the user's "必要" on
  "タグ/リリースが必要なら"): `Cargo.toml` 0.39.0 (`Cargo.lock` following);
  commit `4f7fad6` "Release 0.39.0: the version carries the ledger
  tracking" (`5a2d7f0`-style: the version-only release commit, no
  CHANGELOG section — the release carries no user-facing change);
  the build verified (`agent-cli 0.39.0`); pushed
  `881eda9..4f7fad6 main → origin/main`; lightweight tag `v0.39.0` pushed
  (remote sha confirmed `4f7fad6…`); the GitHub release published
  **2026-11-04T04:24:19Z**,
  https://github.com/aquaxis/agent-cli/releases/tag/v0.39.0. Work log
  `AI_LOG/2026-11-04_003.md` records the close; the records themselves —
  these follow-ups and the requirements document's *State at exit* row —
  travelled in the ledger's own commit `96a576f` ("Record the
  ledger-tracking release close: the ledger events ride the tracked
  documents", `.aiprj`-only, 2026-11-04 13:25:12 +0900), the first commit
  to carry the tracked ledger's updates.

Version trail: FR-32 `7fe1370` (the version 0.35.0, never tagged) → FR-33 `af48be5` (the version 0.36.0) →
`ddeaf67` (FR-34's code; the tag `v0.36.0` on it) → v0.37.0 `5a2d7f0` (FR-34) → v0.38.0 `c02862d` (FR-35) →
`881eda9` (the ledger tracked) → v0.39.0 `4f7fad6` (the version release) →
`96a576f` (the records ride the tracked ledger) → `3af66d4` (the ledger
trail extended; BL-M32 recorded).

## The previous cycle's record (closed before this cycle)

**"the subagents' auto-exit — permanent (FR-32), configurable and
default-on (FR-33); the 'prompt too long' repair (FR-34)"** — closed,
2026-11-03. FR-32 (commit `7fe1370`, the version 0.35.0 delivered in-tree,
never tagged — no release — BL-M32): the agent's own quiet
rule (`input_quiet()` = the channel empty and closed, checked after each
processed input), the serve owner's wait selecting on the agent task's
completion, `spawn_detached` forwarding `--exit-when-idle`, the spawn
tool's and `/spawn`'s children defaulted on, the interactive REPL forced
off; the first planned shape (`AgentInput::ExitWhenIdle` + an in-flight
counter) was removed before commit when the queued-prompt pin found its
race. FR-33 (commit `af48be5`, the version 0.36.0, equally never tagged —
BL-M32): `[spawn] exit_when_idle`
(serde-default `true`), carried via `SpawnLimits`/`ToolCtx`, overridable
per launch by the spawn tool's optional argument. FR-34: the weighted token
estimate (CJK ×3, tool-call argument JSON, reasoning content, the
per-message envelope) and the kept-tail truncation
(`truncate_tail_tool_results`, the `max * 4 / 2` budget) — delivered at
commit `ddeaf67` and released as **v0.37.0** (the release commit `5a2d7f0`,
the tag on the release commit, pushed 2026-11-03; BL-M31 recorded above;
the tag `v0.36.0` on `ddeaf67` stands restored — BL-M32).
Gates at the close: **629 unit + 6 integration, 0 failed**; clippy
identical to the baseline. The tasks T-66 → T-73 stand ✅ in the cycle's
closed ledger; the FR-34 close-out's "pending" item is resolved by BL-M31's
v0.37.0 re-record.

**"the multi-angle subagent analysis, added to the configuration and
verified executable" (FR-29/FR-30)** — closed, 2026-11-03 (no release):
`spawn` in the live `[tools] enabled` (the shipped default omits it), the
shipped `[spawn]` bounds; the executability verdict grounded by live round
trips; R-1 (the 5 s registration window's false negative) carried; R-2
(the example-config claim) withdrawn as false at reconciliation.

Earlier cycles stand in the requirements document's §12: **"the click rule's
exclusion of the elapsed display; the cancelled-by-user display" (FR-27/
FR-28)** — v0.34.0 (`c022909`; the tag's final target `745dbb2`);
**"the scroll marker's face — 「Message below」…" (FR-26)** — v0.33.0
(`6cc6661`); **the agent's running display cycle (FR-22–FR-24)** — v0.32.0
(`e8ad594` + `e0ecb1b`); **the mouse defects cycle (FR-18–FR-21)** —
v0.31.0 (`052e230`); **add strata (FR-13–FR-17)** — v0.30.0–0.30.2
(`b706889`, `70e6b5b`, `a766757`); **the peer-prompt cycles** — v0.29.0
(`045d569`), v0.28.0 (`d237a7a`), v0.27.0 (`afcbe70`); **the mouse input
cycle** — v0.25.0 (`3884d16`) with the three 機能追加 addenda closed as
v0.26.0 (`251a5bb`).