# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "typing prompts during a running turn — accepted into the queue and
run in order once the turn completes" (FR-35, a/b/c)
(`.aiprj/instructions.md`, rewritten 2026-11-04 09:23 (+0900))
Paired with: `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What is decided, and what it costs

- **D-49 — the REPL stops discarding typing during `Pending`; the agent's
  own input channel is the queue; the drain is the loop it always had.**
  The shipped input channel is already a bounded FIFO the REPL, the IPC
  front door and peer sends feed together (`mpsc::channel::<AgentInput>(32)`,
  `src/app.rs:421` interactive / `:734` headless-served); the agent loop
  consumes exactly one input per iteration and, at its bottom, continues to
  the next queued input (BL-M30's file:line record). The feature therefore
  adds no queue structure and no scheduler: it removes the discard.

  Concretely, in the REPL's event loop:
  - the `Pending` branch (`src/app.rs:2740–2764` — cancels only, **all
    other keys ignored**) grows a line-editing acceptor: key events
    **except the cancel keys** build an edit buffer with the shipped
    `InputState` renderer, and the **cancel keys keep their meaning** (a
    cancel governs the current turn only — BL-M30/AS-48; it must not eat
    the queue or the typed line, which the `turn_in_flight` guard of
    `begin_cancel` ensures without change);
  - `Enter` (with non-empty, non-all-space text) sends
    `AgentInput::UserPrompt` by the same `send().await` the other senders
    use (`src/app.rs:2906/3193` for the shipped call sites' shape) and
    clears the edit buffer;
  - the suppressed stdin reads never existed on the agent side at all —
    the queue drains itself, because the loop's bottom moves directly to
    the next `recv()`.

  The one structural consequence the shipped pin `the second prompt sent
  while the input loop is Pending does not arrive` (`src/app.rs:5436–5440`)
  pins the opposite behaviour — the loss this cycle overturns — so it is
  **updated in place** to pin the new contract: during `Pending`, a typed
  prompt arrives on the agent's input channel before the turn ends. The
  update is a specification change, recorded as such (C-13).
- **D-50 — drain semantics: FIFO, no preemption, no prompt return until
  empty.** The queue's order is the channel's FIFO order (arrivals enqueue
  back, the loop takes from the front). A queued prompt is never delivered
  mid-turn: `process_turn` runs to its end before the next `recv()`
  resolves, so there is nothing to preempt. After the last queued prompt's
  turn completes, `input_rx.recv()` awaits (the prompt returns), then
  `input_rx` reaching the quiet state drives FR-32's auto-exit only for the
  headless sessions that carry the flag — the interactive REPL never sets
  it (FR-32's shipped construction forces the interactive flag off).
- **D-51 — the user's visibility of the queue is its count, drawn in
  display-only updates.** The queue is visible as a count of waiting
  prompts, displayed in the same terminal region `AgentEvent::TurnStart`
  drives (`src/app.rs:1380` `needs_refresh(turn_running)`), from the
  channel's own length (`recv`-related introspection the sender side can
  read without receiving) whenever the user types or enqueues while
  Pending. Display-only updates render without taking the bottom rows from
  the running turn's display (the shipped suspend/resume around the
  indicator's rows; the turn's own output is never interrupted). Queued
  prompts are **readable but uneditable** — the edit buffer of an
  already-enqueued prompt no longer exists in the REPL; the count shows
  what the channel holds, the typed line stays independently editable.

  (The FR-35c contract is the drain: after each turn completes, the next
  queued input runs without human input, and the prompt's Ready state
  returns only when the queue is empty — `AgentEvent::Done` releases the
  REPL input loop from `Pending`, and if the channel still holds input the
  next turn's `TurnStart` re-enters Pending immediately, keeping the queue
  draining automatically. No new event type, no new state — the shipped
  state machine with the enqueue and the count.)

## 2. The touchpoints this cycle touches (TPR-20)

| # | Touchpoint | Planned evidence |
|---|---|---|
| TPR-20a | The `Pending` acceptor — the discard removed, the typed line buildable and editable, Enter's enqueue, the count | `src/app.rs` (`Pending` branch `:2740–2764`: the ignore path replaced by the acceptor; the `InputState` renderer; the enqueue shape `:2906/3193`; D-51's count in the display updates), the updated loss pin `:5436–5440` |
| TPR-20b | The agent loop's drain — nothing to change beyond the shipped loop consuming one input per iteration; the quiet check stays after a processed input, the queue's FIFO order untouched | `src/agent.rs:233` (the loop), `:435` (the FR-32 quiet check — headless only), `:207–214` (the queued-input contract) |
| TPR-20c | The cancel keys keep their meaning inside the acceptor (a turn cancel does not eat the typed line or the queue; `turn_in_flight` guards the same way) | the same branch; `src/agent.rs:195–199` (the per-turn cancel clears at the unwind), `src/app.rs:2744–2762` (the cancel handling the acceptor wraps) |

## 3. Analysis verdicts (C-12, AC-32) — the multi-angle record, delivered at the design stage

- **The channel/queue angle** — the FIFO and the one-input-per-iteration
  drain are the channel's own semantics, verified at file:line (BL-M30)
  before this design moved; the queue needs no new structure and no
  scheduler.
- **The input-loop angle** — the discard is located exactly in the
  `Pending` branch (`src/app.rs:2740–2764`), the acceptor can wrap the
  same `InputState` renderer `Ready` uses, and the cancel keys' meaning
  survives unchanged behind the `turn_in_flight` guard (AS-48/AS-50).
- **The cancel angle** — a per-turn cancel clears its flag when the unwind
  lands (`src/agent.rs:195–199`), so a cancelled turn never drops the queue
  behind it; the queue is still guarded by the shipped stop gate (the peer
  branch's `stopping` check, `src/agent.rs:268–302`) — only a genuinely
  stopping session refuses its queue.
- **The exit/quiet angle** — the FR-32 quiet check sits at the loop bottom
  (after a processed input) and sees the **post-drain** channel; with the
  REPL enqueueing during a turn, the channel holds the typed prompts when
  the check runs, so the check cannot fire while the user has queued work
  (the quiet rule `is_empty() && is_closed()` requires both absent
  senders and an empty channel, and the REPL's sender lives for the whole
  session). The interactive REPL
  never sets the flag regardless (FR-32's shipped construction).

## 3′. Superseded state (re-read against the tree, BL-M31)

The previous documents' delivery record for FR-34 — "tag `v0.36.0` on
`ddeaf67`", "the close-out commit pending" — is superseded in the tree: the
release commit `5a2d7f0` ("Release 0.37.0: the compaction repair carries the
version") follows `ddeaf67`, the tag `v0.37.0` stands on it, the CHANGELOG
carries `[0.37.0]` with the repair, and `Cargo.toml` reads `0.37.0`. The
repair is delivered (FR-34 closed as v0.37.0); only this cycle's deliverable
(FR-35) is new.

## 4. Verification of the cycle (executed — the gate table, closed 2026-11-04 at `c02862d`, released v0.38.0)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-53 | FR-35a — the acceptor: the discard removed, the typed line buildable and editable during Pending, Enter enqueues to the channel; the loss pin updated and green | FR-35/FR-35a, NFR-13 | design TPR-20a | ✅ (both loops; the loss pin rewritten and green) |
| W-54 | FR-35b/c — FIFO order, no preemption, automatic drain; the queue-count display (D-51) | FR-35b/FR-35c | TPR-20a/20b | ✅ (`[queued: N]`; the FIFO and drain pins green) |
| W-55 | FR-35's pins — the queued-arrival pin, the FIFO pin, the drain pin, the cancel-keeps-queue pin, the exit/quiet pin (fold-verified at `--locked --all-targets`) | FR-35, NFR-13 | TPR-20c | ✅ (the pin list in tasks §T-75) |
| W-55' | Cancel/stop boundaries — a per-turn cancel doesn't drop the queue; the shipped stop gate still refuses a stopping session's queue | FR-35, NFR-14 | — | ✅ (the cancel-keeps-queue pin green; the stopping gate untouched) |
| W-56 | The gates and the ledger — unit + integration 0 failed, clippy identical to the shipped one; the `.aiprj` documents mutually consistent, the work logs written, the delivery committed per the user's direction | AC-32, C-09, C-08, NFR-12 | — | ✅ (633 unit + 6 integration, 0 failed; clippy = the baseline; delivered `c02862d`, released **v0.38.0** — the tag pushed, the GitHub release published 2026-11-04T03:52:18Z) |

*Executed rows are transcribed (after execution) into `AI_PRJ_TASKS.md` §4
(the task list's execution record).*

## 5. The previous cycles (recorded after their documents were retired)

**"the subagents' auto-exit — permanent (FR-32), configurable and default-on
(FR-33); the 'prompt too long' repair (FR-34)"** — closed, 2026-11-03:
FR-32 (commit `7fe1370`, **v0.35.0**), FR-33 (commit `af48be5`,
2026-11-03 21:36:00 +0900, **v0.36.0**), FR-34 (commit `ddeaf67`, released
as **v0.37.0** — the release commit `5a2d7f0` "Release 0.37.0: the compaction
repair carries the version", the tag `v0.37.0` on `5a2d7f0`, the CHANGELOG
`[0.37.0]`, `Cargo.toml` version 0.37.0 verified in the tree). The tag
stands on the release commit `5a2d7f0`, not on the repair commit — the
previous documents' "tag v0.36.0 on ddeaf67" record is superseded (BL-M31).
The delivered mechanism is recorded in the requirements document's §12 and
the task list's closed sections, mutually consistent (C-08).

**"the multi-angle subagent analysis, added to the configuration and
verified executable" (FR-29/FR-30)** — closed, 2026-11-03 (no release):
`spawn` in the live `[tools] enabled` (the shipped default omits it), the
shipped `[spawn]` bounds; the executability verdict grounded by live round
trips; R-1 (the 5 s registration window's false negative) carried; R-2
(the example-config claim) withdrawn as false at reconciliation.

**"the click rule's exclusion of the elapsed display; the
cancelled-by-user display" (FR-27/FR-28)** — closed, **v0.34.0** (commit
`c022909`, tag pushed, release 2026-10-23T01:03:48Z, binary installed; the
tag's final target the docs-residue follow-up `745dbb2`): the click rule's
strict top excluding the elapsed row, the query-error fallback disarmed;
the three cancel prints `[info] cancelled by user` styled `Role::Info`,
`Role::Cancelled` retired. Suite **629 unit + 6 integration, 0 failed**;
clippy at the baseline.

**"the scroll marker's face — 「Message below」…" (FR-26)** — closed,
v0.33.0 (`6cc6661`; tag retargeted to `745dbb2`): the marker face in
reverse video, the absorbed ▼-era geometry, the repaired narrow-width pin.
An earlier record had claimed a `1bcb6f8` withdrawal as
unverifiable, the correction standing in the task list's history.

**"the agent's running display, the subagent status display, and the
prompt's resumption after a cancel"** — closed, v0.32.0 (`e8ad594` +
`e0ecb1b`): the clock as the screen's bottom row (FR-22,
`subagent_row_plan`), the subagent rows stacked above the bottom block,
oldest-first (FR-23), the Esc-return redraw (FR-24, `esc_return Ready`).
The delivered mechanism was recorded in that cycle's documents;
the work log was written with the instruction's contents (Art. 7);
the delivery committed per the user's direction.

**"the mouse defects — the scrolled clock, the clipboard copy — and the
subagent run-state line; Esc re-shows the prompt"** — closed, v0.31.0
(`052e230`): FR-18/FR-19/FR-20/FR-21.

**"add strata as a connection target; the alias displays as strata"** —
closed, v0.30.0 (`b706889`), v0.30.1 (`70e6b5b`), v0.30.2 (`a766757`).

**"a received peer prompt sometimes never starts processing…"** — closed,
v0.29.0 (`045d569`). **"…left unprocessed"** — v0.28.0 (`d237a7a`).
**"…never progresses"** — v0.27.0 (`afcbe70`). **"mouse input turns into
escape sequences, and typing freezes"** — v0.25.0 (`3884d16`) with the
three 機能追加 addenda closed as v0.26.0 (`251a5bb`).