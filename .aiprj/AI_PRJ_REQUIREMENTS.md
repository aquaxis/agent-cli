# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)
(`.aiprj/instructions.md`, rewritten 2026-11-04)
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

The v0.35.0–v0.37.0 stretch delivered the subagents' completion-triggered
auto-exit, permanent and configurable and on by default (FR-32/FR-33 — the
in-tree versions 0.35.0/0.36.0, neither ever tagged; BL-M32), and repaired
the compaction that let a live session die with `prompt too long` (FR-34,
closed as **v0.37.0** — the delivery record below corrects the version the
previous ledger carried). The current
instruction (rewritten 2026-11-04; reloaded byte-exact) carries one ask,
and the cycle answers it in three parts:

1. **Input during work** — an agent mid-turn (thinking, a tool call, any
   tool-call work) must accept a prompt typed while it is busy.
2. **The queue** — a prompt that arrives during the turn is held, not
   executed, not dropped and not lost.
3. **The drain** — once the running work finishes, the queued prompts are
   executed in the order they arrived.

Today's shipped behaviour is the exact void the ask fills: during the
`Pending` prompt state the REPL honours only Esc / Ctrl-C and ignores every
other key (`src/app.rs:2740–2764`), so a prompt typed while the agent works
is silently discarded — there is a pin that binds exactly that loss
(`src/app.rs:5436–5440`, "second prompt should not arrive while input loop
is Pending"). The agent's own input channel already is a bounded
FIFO (capacity 32) that peer prompts queue into, and the agent loop
processes one input per iteration (`src/agent.rs:233`), so the queue
mechanism exists on the agent side; what is missing is the REPL's
willingness to read keys and enqueue during a turn.

The standing method applies (C-12): investigation and repair are to be
performed by multi-angle subagent analysis, with every load-bearing
behaviour pinned (NFR-13).

## 2. Source instruction

`.aiprj/instructions.md` as reloaded in this session (rewritten 2026-11-04,
420 bytes; sha256
`26c96f3e630d6a61f1a9a2b12988c8b67e98e10bfb7c4268cde0f4266f052944`;
verified byte-exact):

````text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する

## 最優先事項

エージェントがrhinkingやtool-callなどの作業中でもプロンプトの入力を行えるように機能追加する
エージェントが作業中のプロンプトの入力はキューに貯められ実行中の作業が完了次第、キューに溜まったプロンプトを順次実行する
````

(Translation: **Feature additions to agent-cli. The work is to be performed
by multi-angle subagent analysis. Top priorities:** add the ability to type
a prompt into the agent even while it is working — thinking, tool calls, and
the like; a prompt typed while the agent is working is held in a queue, and
as soon as the running work completes the queued prompts are executed in
order.)

## 3. Baseline — verified in the tree at this update

Verified 2026-11-04 — BL-M30/BL-M31 on the cycle's baseline HEAD `5a2d7f0`
(= tag `v0.37.0`); BL-M32 on HEAD `3af66d4` (worktree clean at both):

| ID | Fact | Evidence |
|---|---|---|
| BL-M30 | A prompt typed during a running turn is **lost, not queued**: in the `PromptState::Pending` branch the input loop honours only the cancel keys (Esc / Ctrl-C) and `continue`s on every other key; stdin line reads are suppressed entirely while Pending, and the shipped pin asserts a second prompt sent during Pending **does not** arrive on the agent's input channel. The agent-side channel, in contrast, already queues: it is a bounded mpsc FIFO (capacity 32) fed by the REPL sender, the IPC front door and peer sends, and the agent loop consumes exactly one input per iteration, so any input already in the channel when a turn ends is delivered next. | `src/app.rs:902–907` (the `Pending` state's contract: "normal stdin reads are suppressed"), `src/app.rs:2740–2764` (during Pending only Esc/Ctrl-C are honoured; "All other keys during Pending are ignored"), `src/app.rs:5436–5440` (the pin that binds today's loss), `src/app.rs:421` / `:734` (the channel: `mpsc::channel::<AgentInput>(32)`), `src/agent.rs:233` (the loop consumes one input per iteration), `src/agent.rs:207–214` (the queued-input contract the FR-32 flag already relies on) |
| BL-M31 | The delivery state the previous ledger recorded for FR-34 is superseded in the tree: the repair's version commit was followed by a release commit, the tag stands on the release commit `5a2d7f0` on `origin/main`, and the CHANGELOG carries `[0.37.0]` with the repair — so FR-34's close-out is re-recorded as **v0.37.0** (the earlier documents' "tag `v0.36.0` on `ddeaf67`" and "close-out commit pending" were written before the release commit landed). | `git log` (`5a2d7f0` "Release 0.37.0: the compaction repair carries the version", 2026-11-03 23:28:17 +0900; parent `ddeaf67`), the tag list (`v0.37.0`), `CHANGELOG.md` `[0.37.0]` (Fixed — the compaction repair), `Cargo.toml` version `0.37.0` |
| BL-M32 | The version trail's release labels for FR-32/FR-33 are superseded: no `v0.35.0` tag exists (the local and remote tag lists jump v0.34.0 → v0.36.0) and the lightweight tag `v0.36.0` dereferences to the FR-34 compaction commit `ddeaf67` (`af48be5` is its ancestor, never a tagged tip). The previous documents' "tag `v0.36.0` on `ddeaf67`" — deleted by BL-M31's rewrite — was the true tag state and stands restored; "`7fe1370` = **v0.35.0**" and "`af48be5` = **v0.36.0**" as *releases* were false records (the commits carry the in-tree versions 0.35.0/0.36.0, never tagged, no GitHub releases). The v0.37.0 publication time already stood corrected (14:17:12Z → 14:33:12Z). | the local and remote tag lists (`git tag`, `git ls-remote --tags origin` — no `v0.35.0`), the dereference (`git rev-parse v0.36.0^{commit}` → `ddeaf67…`), the tree at the commits (`7fe1370`: `Cargo.toml` 0.35.0 + CHANGELOG `[0.35.0]`; `af48be5`: 0.36.0 + `[0.36.0]`; `ddeaf67`: 0.36.0 + `[0.37.0]`), `gh release list`/the release API (none for v0.35.0; v0.36.0 published 2026-11-03T14:24:11Z, v0.37.0 14:33:12Z, v0.38.0 2026-11-04T03:52:18Z, v0.39.0 04:24:19Z) |

## 3.1 What the instruction does NOT ask for

- **Not parallel execution** — queued prompts run **after** the running turn
  completes, one after another; nothing runs concurrently with the
  in-flight turn.
- **Not a new input path** — the queue is the agent's existing input
  channel; the feature is the REPL's acceptance during `Pending`, not a
  second queue or a second channel.
- **Not a change to the cancel keys** — Esc / Ctrl-C keep their
  cancel-during-Pending meaning (`FR-24`'s prompt return stands); enqueue
  must not swallow them.
- **Not an approval-prompt change** — the `AwaitingApproval` state keeps its
  own semantics; the ask covers the working state (`Pending`).
- **Not a headless-side change** — a headless agent's channel already
  queues peer prompts and drains them after each turn (BL-M30); the ask is
  the interactive prompt's behaviour.
- **Not a reorder, not a cancel of queued prompts** beyond what the shipped
  channel and shutdown semantics already do — FIFO arrival order is the
  contract.

## 4. Scope

### 4.1 In scope

- **FR-35a** — the REPL reads and enqueues during `Pending`: typed text is
  accepted while the turn runs.
- **FR-35b** — the queued prompt is held untouched until the turn ends
  (FIFO arrival order; nothing is dropped, nothing is executed early).
- **FR-35c** — after the turn completes, the queue drains in order, each
  prompt getting its own turn (with its own `Done`), the prompt returning
  to the user when the queue is empty.
- The pinned behaviour for all three, and the docs/ledger updates.

### 4.2 Out of scope

- The §3.1 list; any change to the provider contracts, the wire shapes, the
  exit seams, or the shipped teardown.

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-35 | While an interactive agent's turn is running (thinking, a tool call, or any part of `process_turn`), typed prompt input is **accepted, not ignored**: the input loop's `Pending` branch reads line input the way `Ready` does and enqueues it onto the agent's existing input channel (the bounded FIFO all other senders feed). The queued prompt executes **only after** the running work completes; when the turn ends, the loop takes the next queued input in arrival order (FIFO) and runs it as its own turn; the user's prompt returns only when the queue is empty. Queued prompts survive exactly the shipped channel semantics — a cancel of the *current* turn does not drop the queue behind it (the per-turn cancel flag clears when the unwind lands, `src/agent.rs:195–199`), and the queue is still subject to the shipped stop gate (only a genuinely stopping session refuses its queue, `src/agent.rs:200–206`). |
| FR-35a | The `Pending` branch accepts key input and builds the same edit buffer `Ready` uses; Enter enqueues `AgentInput::UserPrompt`, `AgentEvent::TurnStart`-driven display continues over a queued count so the user sees what is waiting. |
| FR-35b | Enqueue order equals arrival order (FIFO); the running turn is never preempted or interleaved by a queued prompt. |
| FR-35c | On each turn's end the drain continues automatically — the next queued prompt starts without human input; the prompt state returns to `Ready` only when the channel holds no further input. |

## 6. Boundary: FR-35 vs the shipped contracts

The shipped contracts stand (NFR-14): the four exit seams and the
teardown/drain, the `stopping` gate's truthful skip replies, the per-turn
cancel semantics (a cancel stops the turn, not the session — the queue
advances after the unwind), the FR-32/FR-33 auto-exit (a queued prompt
keeps a flagged agent alive — `a_queued_prompt_keeps_a_flagged_agent_alive`
already pins the agent side and inherits this feature untouched), the
FR-34 compaction, and every FR-01–FR-34 contract. FR-35 changes one
behaviour: what the REPL does with keys during `Pending` — from
"ignore everything but the cancel keys" to "accept and enqueue". The
interaction surface that shows the running turn (the elapsed display, the
spinner) keeps its shape; the queued state adds display, not reroutes it.

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | Behaviour-binding pins: every load-bearing behaviour is pinned by a test first made to fail against the shipped shape (`src/app.rs:5436–5440` pins today's loss — it is superseded in the same commit that delivers the feature), then kept green. |
| NFR-14 | The changes stay additive to the shipped contracts (the boundary of §6). |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism on the installed binary, not by reading alone. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression: unit + integration counts stay green; clippy output stays identical to the pre-change baseline — re-certified at each of the cycle's commits. |
| C-12 | The instruction's method — multi-angle subagent analysis — is performed for the investigation and the repair. |
| C-13 | Write surface for the `.aiprj` documents: the three documents plus the required work logs; the product tree changes are the cycle's own instructed deliverable (FR-35 is a coding ask), recorded here without attribution. |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-32 | FR-35 implemented (a, b, c) and pinned; the superseded pending-loss pin updated to the queued behaviour; gates green (unit + integration, 0 failed) and clippy identical to the baseline; the ledger consistent, the work logs written, and the delivery committed per the user's direction. |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-46 | The agent-side channel is the queue (BL-M30): bounded FIFO, capacity 32, all senders feed it, the loop drains one per turn-end — so the feature is the REPL's acceptance during `Pending` plus drain semantics, not a new structure. | Grounds FR-35's design; verified at file:line. |
| AS-47 | "rhinking" in the instruction is "thinking" (a typo in the source); the ask covers the whole running turn — thinking, tool calls, any in-flight work. | Recorded; no behaviour hangs on the typo. |
| AS-48 | The shipped per-turn cancel semantics make the queue cancel-safe: a `begin_cancel` on the current turn clears its flag when the unwind lands, so queued prompts after a cancelled turn proceed — the feature adds no new cancel rule. | Grounds FR-35's cancel boundary; verified at `src/agent.rs:195–199`. |
| AS-49 | The channel's bounded capacity (32) is the queue's capacity; a full channel cannot happen from typing alone at human speed, but the enqueue uses the same `send().await` the other senders use, so backpressure behaves identically. | Grounds FR-35a; verified at `src/app.rs:421/734/2906/3193`. |
| AS-50 | The shipped edit machinery is reusable as-is: the `Ready` branch's `InputState` renderer accepts key events and renders the typed line regardless of which prompt state drives it, so the `Pending` acceptor wraps it rather than reimplementing line editing. | Grounds FR-35a's design; verified against the `Pending`/`Ready` branch shapes at `src/app.rs`. |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-35 | design §1 (D-49), §2 | T-74 |
| FR-35a | design §1 (D-49), §3 | T-75 |
| FR-35b | design §1, §3 | T-74, T-75 |
| FR-35c | design §1 (D-50), §3 | T-74 |
| NFR-13 | design §1 (D-49), §3, §4 | T-74, T-75 |
| NFR-14 | design §2, §4 | T-74 |
| AC-32 | design §1 – §5 | T-74, T-75 |

## 12. The previous cycles' record

**"the subagents' auto-exit — permanent (FR-32), configurable and
default-on (FR-33); the 'prompt too long' repair (FR-34)"** — closed,
2026-11-03. FR-32 (commit `7fe1370`, the version 0.35.0 delivered in-tree,
never tagged — no release), FR-33 (commit `af48be5`,
2026-11-03 21:36:00 +0900, version 0.36.0, equally untagged), FR-34
(commit `ddeaf67`, released as **v0.37.0** — the weighted token estimate
(CJK ×3, tool-call arguments, reasoning content, the per-message envelope)
and the kept-tail truncation
(`truncate_tail_tool_results` at half the token budget, marker-annotated);
the release commit `5a2d7f0`, tag pushed, 2026-11-03 23:28:17 +0900; the
earlier ledger's "tag `v0.36.0` on `ddeaf67`" record **was the true tag
state** (BL-M32 narrows BL-M31's supersession at this update; re-verified
from the local and remote tag lists) and stands restored, while the
earlier ledger's
"`7fe1370` = **v0.35.0**"/"`af48be5` = **v0.36.0**" release labels are the
false records and are superseded by BL-M32).

**"the multi-angle subagent analysis (standing) and the subagents' self-exit
on completion" (FR-31)** — closed, 2026-11-03 (no release — investigation
and report): the four exit seams enumerated, the persistence default and
the live self-exit measured, the design space named. Superseded by the
implementation (FR-32/FR-33).

**"the multi-angle subagent analysis, added to the configuration and
verified executable" (FR-29/FR-30)** — closed, 2026-11-03 (no release):
`spawn` in the live `[tools] enabled`, the shipped `[spawn]` bounds; the
executability verdict grounded by live round trips; R-1 (the 5 s
registration window) carried; R-2 withdrawn as false.

**"the click rule's exclusion of the elapsed display; the
cancelled-by-user display" (FR-27/FR-28)** — closed, **v0.34.0**
(`c022909`; the tag's final target `745dbb2`). **"the scroll marker's face
— 「Message below」…"** — closed, v0.33.0 (`6cc6661`). **"the agent's
running display, the subagent status display…"** — closed, v0.32.0
(`e8ad594` + `e0ecb1b`). **"the mouse defects…"** — v0.31.0 (`052e230`).
**"add strata…"** — v0.30.0–0.30.2 (`b706889`, `70e6b5b`, `a766757`).
**v0.29.0** (`045d569`). **v0.28.0** (`d237a7a`). **v0.27.0** (`afcbe70`).
**v0.25.0** (`3884d16`) with the addenda **v0.26.0** (`251a5bb`).

## 13. Delivery status

| Item | State |
|---|---|
| FR-32 (the auto-exit, permanent) | Delivered — commit `7fe1370` (the version 0.35.0, in-tree; **no tag, no release** — BL-M32). |
| FR-33 (configurable, default-on) | Delivered — commit `af48be5` (the version 0.36.0, in-tree; equally untagged — BL-M32). |
| FR-34 (the "prompt too long" repair) | Delivered — commit `ddeaf67`, released **v0.37.0** (the tag on `5a2d7f0`, pushed 2026-11-03 23:28:17 +0900; the release
published 2026-11-03T14:33:12Z — live-verified from the GitHub releases
list and the release API at this update (an earlier draft of this ledger
carried 14:17:12Z, which matches no timestamp in the tree or on the
releases); the binary installed). |
| FR-35 (prompt input during a running turn, queued and drained in order) | Delivered — commit `c02862d` ("Prompts typed while a turn runs are queued and run in order when it ends"), `Cargo.toml` **0.38.0**, the CHANGELOG `[0.38.0]` Added entry; the gates at the close: **633 unit + 6 integration, 0 failed**, clippy identical to the shipped baseline. **Released**: tag `v0.38.0` on `c02862d` (lightweight, per the v0.36.0/v0.37.0 precedent), pushed to `origin/main` (`5a2d7f0..c02862d`) and the tag; the GitHub release published **2026-11-04T03:52:18Z** (the commit authored 2026-11-04 12:41:31 +0900 precedes the publication). |
| State at exit | The cycle closed: T-74/T-75 ✅ (the tasks' execution record), the design's gate rows W-53–W-56 ✅; the documents mutually consistent; the delivery committed, pushed, tagged and released as v0.38.0. **The cycle's post-close ledger events (2026-11-04, this cycle's tail):** the `.aiprj` ledger brought under git tracking — commit `881eda9` "Track the .aiprj project ledger: the three documents, work logs, rules" (123 logs, the three documents, `config.toml`, `README.md`, `rules/`; the `.aiprj` whole-dir gitignore removed, `instructions.md` alone stays untracked — its text is embedded verbatim in every work log); the version released as **v0.39.0** — commit `4f7fad6` "Release 0.39.0: the version carries the ledger tracking", tag `v0.39.0` pushed, the GitHub release published **2026-11-04T04:24:19Z** (https://github.com/aquaxis/agent-cli/releases/tag/v0.39.0). **The records themselves ride the tracked ledger:** the two post-close records above were carried by the ledger's own commit `96a576f` "Record the ledger-tracking release close: the ledger events ride the tracked documents" (`.aiprj`-only — these records and the tasks' follow-ups, work log `2026-11-04_003.md`; 2026-11-04 13:25:12 +0900; from here on every ledger update travels as a normal commit). |