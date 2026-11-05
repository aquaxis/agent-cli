# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the codex-code backend — the installed codex CLI is driven the way the
claude-code backend drives `claude`, as a ninth `provider.kind` with its own
`[provider.codex-code]` section, and it is documented, version-carried and
pushed"
(FR-44 a/b/c/d) (`.aiprj/instructions.md`, rewritten 2026-11-05 15:55 (+0900),
re-rewritten 2026-11-05 16:46 (+0900), then 2026-11-05 17:37 (+0900) — the
same ask clause each time, the last rewrite adding the standing delivery
sentence 「機能追加した場合は、ドキュメント更新、example更新、マイナー
バージョンアップ、commit, push」)
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked
Article 3 of the AI Coding Guidelines: record progress updates in this
file whenever a task changes state.

---

## Overview

The instruction (rewritten 2026-11-05 15:55:07 +0900; re-rewritten
2026-11-05 16:46:10 +0900 — the same ask clause with the delivery machinery
made explicit) names one new ask: **implement** codex access the way the
claude-code access is implemented, and then update the docs and the config
template and **carry a minor version, commit and push**. The ask is
recorded as **FR-44** a/b/c/d (the backend, the wiring, the docs, the
release, and the post-delivery example amendment) with the baselines
verified in the tree (BL-M49–BL-M51), the design decisions
**D-67/D-68/D-69/D-70** and the tasks **T-97/T-98/T-99/T-100** below.

**The ask is delivered.** The backend is `src/ai/codex_code.rs` — one
`codex exec` spawn per turn, `--json` items or one-shot, the session
carried by codex's own thread id (the resume leg `codex exec resume
<thread_id>`), the tools delegated (arriving already executed, never
re-run), the argv geometry flags-first (the probes falsified the
subcommand-first order live, T-98) and the trust/sandbox mapping
deliberate (`workspace-write` + `--skip-git-repo-check` defaults, since
delegation must run commands headlessly). The docs stand: README ×2, the
new `doc/providers/codex-code.md`, the config reference §2/§3, the
CHANGELOG `0.42.0` entry, and the shipped config template
(`DEFAULT_CONFIG` in `src/config.rs`). The version carried to **0.42.0**
and is released and pushed in T-99. Gates at the close: **669 unit + 6
integration, 0 failed**, clippy **0 warnings** (T-98).

The ask's baselines, as the tree stood when each was verified:

- **The two shipped backends and the named precedent** (BL-M46): the
  `claude-code` backend (`src/ai/claude_code.rs`, 1,583 lines) drives the
  installed `claude` CLI headlessly — `Settings` over
  `[provider.claude-code]`, the pure `build_args` fed by
  `SessionAction::None/New/Resume`, the resident child only for
  delegation+stream+persistent, one child per turn otherwise — and the
  codex wire backend (`src/ai/codex.rs`, 525 lines) speaks OpenAI Chat
  Completions HTTP and never touches the codex CLI. The CLI-driven codex
  access the instruction asks about does not exist in the tree.
- **The codex CLI's live surface** (BL-M47, probed on `codex-cli 0.166.0`,
  ChatGPT auth): `codex exec` emits turn-level JSONL (`thread.started`
  carrying `thread_id`, `turn.started`, `item.completed` —
  `agent_message` whole, `command_execution` typed with
  `command`/`aggregated_output`/`exit_code`/`status` — `turn.completed`
  with token usage); `codex exec resume <thread_id> --json` continues the
  thread codex-side (one id carried five probes, history accumulating
  13,094 → 52,496 input tokens with the transcript never re-sent);
  assistant text arrives whole per item — **no deltas**; `--ephemeral`
  runs without session files; exec refuses outside a trusted git
  directory without `--skip-git-repo-check`; the sandbox/approval knobs
  differ from claude-code's `--permission-mode`.

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-95 | The investigation and its report (FR-43a/FR-43b/FR-43c): the six mechanisms the claude-code pattern depends on — dispatch/construction, session continuity, tool ownership, streaming granularity, persona/system-prompt surface, trust and approvals — each answered against the codex CLI's live surface (probes on the installed binary plus the tree's citations, every claim bound to a transcript or a `file:line`, unprobed surfaces recorded as unprobed); the multi-angle subagent analysis performed and reconciled, the probe deciding disagreements; the verdict, the mapping, the costs and the future shape written into the requirements (§1, §3) and the design (D-65, D-66) | FR-43a, FR-43b, FR-43c, NFR-11, NFR-15, C-12 | design §1 (D-65), §2 (TPR-25), §3 | — | ✅ |
| T-96 | The cycle's record and consistency (Art. 2, C-08): the three documents written from the probed evidence with the shared identifiers (FR-43 a/b/c, BL-M46–BL-M48, TPR-25, D-65/D-66, W-66, T-95/T-96, AC-36, AS-63–AS-65), mutually consistent; the boundary §6's shipped contracts named untouched; the instruction re-verified byte-exact at exit; the work log written per Art. 6/7; no product source, config, test, pin, release or version change | — (NFR-12, NFR-13, NFR-14, C-08, C-09, C-13, AC-36) | design §3 (the docs angle), §4 (W-66) | T-95 | ✅ |
| T-97 | The backend (FR-44a; design §1 D-67, §2 TPR-26): `src/ai/codex_code.rs` — the codex sibling of `claude_code.rs`, built on the investigation's probes (BL-M47): delegation only, thread-id session, the transports `items`/`oneshot`, session `persistent`/`ephemeral`, the sandbox/trust knobs codex-side, the resume-not-found and timeout recoveries, no gateway mode (D-65's recorded gap), the persona riding the turn payload | FR-44a, NFR-13, NFR-15, C-09 | design §1 (D-67), §2 (TPR-26), §3 | T-96 | ✅ |
| T-98 | The wiring, the pins and the docs (FR-44a/FR-44b; design §1 D-68, §2 TPR-26): the dispatch `build()` arm and `SUPPORTED`, the `[provider.codex-code]` slot (`codex_code` field, `provider_entry`/`_mut` arms, the sample block in `DEFAULT_CONFIG`), `providers()`/`doctor()`'s arms, the `--provider`/`spawn` lists, and the docs set — the two READMEs (「Eight backends」→「Nine」, the backend row, the capability column, the claude-code-style section, the link), the guide `doc/providers/codex-code.md`, `doc/config.md` §2/§3, the CHANGELOG `[0.42.0]`, `example/config.example.toml` and the usage/tools kind lists — every statement bound to the code or a live probe. The post-delivery amendment (the user's chat ask, the standing post-addition clause of the instruction as rewritten 2026-11-05 17:37) updated `example/config.example.toml` further: the `codex-code` block joined (with the sample model refreshed to `gpt-5.6-terra`, the Codex CLI's own live default on the probe machine, `codex exec` verified) and the sample model lines of the other blocks were refreshed to the newest ids, each evidenced live (claude `claude-opus-5-5`; claude-code `opus`, the alias the running `~/.claude/settings.json` itself carries; codex `gpt-5.6-terra`, the Chat Completions page naming function calling Supported; ollama `glm-5.3-flash:cloud`, the server's own newest-GLM pointer; opencode local `claude-sonnet-5-5`; opencode-go `minimax-m3.1-flash`, the newest open-weights id (`open_weights: true`, tools) on the `/zen/go/v1` catalog; the two usage/tools kind lists joined) | FR-44a, FR-44b, NFR-12, NFR-14, C-08, C-13 | design §1 (D-68), §2 (TPR-26), §3 (the docs angle), §4 (W-67) | T-97 | ✅ |
| T-99 | The completion (FR-44c; design §1 D-69, §4 W-67): the version carry 0.41.0 → 0.42.0 (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's `[0.42.0]` heading), the delivery commit(s) pushed to `origin/main` (AS-68), the ledger's rewrite riding the delivery commit, its sha named in the follow-up per the trail rule (a commit's sha is nameable only from the commit that follows it), the instruction re-verified byte-exact at exit | — (NFR-12, NFR-13, NFR-14, C-12, C-14, W-67, AC-37) | design §1 (D-69), §4 (W-67) | T-97, T-98 | ✅ |

Sections below the table are the execution record (Article 3) — written as
each task's status changes.

## The cycle's record

- **The ask is delivered.** FR-44a (the backend), FR-44b (the docs) and
  FR-44c (the release) stand as T-97/T-98/T-99 below record them, and
  FR-44d (the post-delivery example amendment: the newest sample models,
  the missing config-template block, the usage/tools kind lists, the
  shipped-README/doc sample refresh, the version carry to 0.43.0) as
  T-100 records it.

- **The ask was an investigation, and its deliverable was the ledger.**
  「調査して報告して下さい」 names two verbs: investigate and report. No
  implementation is asked for and none is made (the requirements' §3.1;
  D-66); the verdict stands in the documents where the next cycle can
  build from it.
- **The verdict is feasible for delegation-style access** (D-65), decided
  by execution (NFR-15) rather than by `--help` reading: the five-probe
  resume chain, the delegation run with its `command_execution` items,
  and the stdin/`--ephemeral`/`-o`/`--skip-git-repo-check` runs are the
  evidence; the delta absence is a live observation, not an inference.
- **The method ran as the instruction requires** (C-12): the subagent
  verdict (its six angles) was collected independently and reconciled
  against the session's own probes — the probes decided the delta and
  resume questions, and the subagent's mapping (the `SessionAction`
  port, the event mapping, the costs) was confirmed against the tree.
- **No shipped contract is touched** (NFR-14; the boundary §6): the
  investigation writes the three `.aiprj` documents and the work log and
  nothing else; the gates stand by construction (C-09 per BL-M48).

## T-95 — the investigation and its report (✅)

Delivered. The six mechanisms were each answered live on the installed
codex CLI (`codex-cli 0.166.0`, ChatGPT auth, `~/.codex/config.toml`
carrying `model = "gpt-5.6-terra"`), with the tree's citations beside
them (BL-M46 for the shipped pattern):

1. **Dispatch and construction — yes.** A new kind reaches a backend
   through `build()`'s arm (`src/ai/mod.rs:132–150`) and a
   `[provider.*]` entry (`src/config.rs:271–290`, the lookup
   `:1174–1194`, the CLI's `--provider` list `src/cli.rs:134`); the
   claude-code backend tolerates an absent entry at construction
   (`claude_code.rs:630–641`), and CLI-driven codex needs no credential
   of its own where ChatGPT auth already stands (the wire kinds differ
   here, and BL-M47 is the evidence both ways).
2. **Session continuity — yes, by codex's own id.** The first turn's
   `thread.started` hands over `thread_id`; `codex exec resume
   <thread_id> --json` continues the thread codex-side — five probes on one
   thread with the history accumulating (`input_tokens` 13,094 →
   26,208 → 39,342 → 52,496, mostly cached) and the transcript never
   re-sent. This is claude-code's `session = "persistent"` behaviour
   reached with one spawn per turn; the resume subcommand **is** the
   `SessionAction`, and no resident child exists or is needed. The
   prompt rides argv or stdin (stdin works as the sole carrier;
   the help's `<stdin>` append behaviour noted).
3. **Tool ownership — yes, delegation.** A turn asked to run a command
   emitted `item.started`/`item.completed` pairs of `type:
   "command_execution"` (`command`, `aggregated_output`, `exit_code`,
   `status` in_progress → completed) between the agent messages —
   codex's tools executed inside codex, the claude-code delegation
   rule's own semantics: never map them to `ProviderEvent::ToolUse`,
   never re-run them. Gateway mode has **no** codex counterpart (the
   child runs its own agent and accepts no external tool list), so it
   is recorded as not implementable over the CLI.
4. **Streaming granularity — no deltas on the probed surface.** A
   three-line answer arrived as **one** `item.completed`, not as
   deltas; the exec JSONL has no `--include-partial-messages` analog.
   Stream granularity is therefore turn-level (the claude-code
   `Transport::OneShot` class), and the display would show one row per
   turn. The `app-server` protocol may expose finer granularity but was
   not probed at protocol depth (recorded as unprobed, AS-65).
5. **Persona / system prompt — no dedicated slot probed.** The probed
   `codex exec` help exposes no `--append-system-prompt` /
   `--system-prompt` pair; a persona must ride the turn payload (and a
   resumed thread replays it, growing what is visible in the session
   history). Recorded as a stated cost, not a blocker.
6. **Trust and approvals — the deliberate mapping.** Outside a trusted
   git directory exec refuses until `--skip-git-repo-check` is passed
   (live-verdict: the refusal was executed and then satisfied); the
   sandbox is `-s read-only|workspace-write|danger-full-access` and
   bypass flags exist where claude-code has `--permission-mode`; the
   per-turn spawn keeps the policy per invocation (the `TurnPlan`-like
   shaping `build_args` gives claude-code). `--ephemeral` runs without
   persisting session files; `-o/--output-last-message` writes the turn's
   final message — useful, not load-bearing.

**The mapping** (the claude-code pattern → codex's own): `Settings`
→ a `[provider.<kind>]` entry (the knobs codex's own: sandbox, approval,
model, trust); `TurnPlan`/`build_args` → the per-invocation argv of
`codex exec`/`exec resume`; `SessionAction::New/Resume` → the first
turn's `thread_id` capture and `codex exec resume <thread_id>`;
`stream-json` line parser → the JSONL event parser (unknown lines
ignored, the claude-code module's own rule); `Transport::Stream` →
not offered on the probed surface (no deltas); `Mode::Delegation` →
the native behaviour; `Mode::Gateway` → no counterpart. The shape a
future cycle would build — a sibling `CodexCodeProvider`, a `codex-cli`
(or as then decided) kind behind the same `Provider` trait, the event
map `agent_message` → `Text`, turn end → `Done`, item pairs → a display
event or swallowed — stands in D-66 as a report, unbuilt.

**The method** (FR-43c): the subagent analysis ran (its six angles), and
the reconciliation record is the requirements §1's "reconciled" clause
and the design §3's citations: where the subagent's verdict and a probe
differed (the delta question, the resume shape), the probe decided.

Depends on: nothing (the first task of the cycle).

Design trace: design §1 (D-65), §2 (TPR-25), §3 (all six angles), §4 (W-66).

## T-96 — the cycle's record and consistency (✅, depends on T-95)

Delivered. The three documents carry the shared identifiers and agree
(C-08): FR-43 a/b/c, BL-M46–BL-M48, TPR-25, D-65/D-66, T-95/T-96, W-66,
AC-36, AS-63–AS-65. The instruction was re-verified byte-exact at exit
(241 bytes, sha256 `1d813156…`). The work log
`2026-11-05_008.md` embeds the instruction verbatim (Art. 7) and carries
the execution record. No product source, config, test, pin, release or
version changed (`git status` carries the three documents and the work
log alone — BL-M48), so the delivered tip's gates stand by construction
(C-09 as standing): **647 unit + 6 integration, 0 failed**, clippy
**0 warnings**. The delivered contracts stand named in the boundary §6
of the requirements, and the trail names no new commit — the rewrite
stood uncommitted at that cycle's close, per the trail's rule that a
commit's sha is nameable only from behind it; the next cycle (FR-44,
whose instruction directed the 完成後 carry) made the delivery commits
and named them in its own follow-up.

Depends on: T-95.

Design trace: design §3 (the docs angle), §4 (W-66); requirements
§13 (State at exit).

## Cross-cutting gates (✅)

- Unit + integration: **669 unit + 6 integration, 0 failed** at this
  cycle's close (BL-M50; C-09 re-executed per its re-execution clause —
  the tree changed).
- Clippy: **0 warnings** at this cycle's close (BL-M50).
- The `.aiprj` documents mutually consistent (C-08), the work log written
  (`2026-11-05_009.md`; Art. 6/7), the instruction byte-exact at exit.
- The boundary §6's items (the queue contracts, the bare-`Enter` guard,
  the cancel keys, the approval takeover, the headless/piped paths, the
  mouse-capture and clipboard contracts, the peer rows' face and
  stacking, the four display regions, FR-39/FR-40's geometry and
  FR-42's prompt-owned rows, FR-41's collapsed shapes) unchanged —
  verified by no write reaching the product tree (NFR-14).

## The previous cycles' record

**"the refactoring — contradictions and waste in the source are removed,
`src/ai`'s provider error chain and the tools' test context each collapse
to one implementation, clippy falls silent, and the documents follow the
source; and the prompt's bottom row is prompt-only" (FR-41, FR-42)** —
closed, 2026-11-05: delivered at `eec12f9` ("Refactor: one HTTP error
chain, one test context, no dead code", 26 files, +139/−314) and
`064ce50` ("Keep the prompt's rows prompt-only: the elapsed row stands
above the span"), each decision D-62–D-64 standing; the version carried
to **0.41.0** by `25fe13c` ("Release 0.41.0: the refactoring and the
prompt's own rows carry the version") and the trail's follow-up
`3badc04` naming it; work logs `2026-11-05_006.md` (the cycle) and
`2026-11-05_007.md` (the carry and the push). Its delivered tree is this
cycle's untouched ground (BL-M48).

**"the display corrections — during mouse scrolling the `Message below`
marker stands above the prompt, not below it; and during a running turn a
mouse selection no longer makes the elapsed display vanish nor drop the
selection one row" (FR-39, FR-40)** — closed, 2026-11-05, delivered at
commit `db68d3f`, its sha named by the ledger's follow-up `306f425`, with
the follow-up's own work log `85f9ce9`. Its two decisions stand as the
ground D-63 is measured against: D-60 (the marker on the row immediately
above the prompt block's first row — `scrolled_block_plan`
`:2150–2166`, `scrolled_marker_rows` `:1376–1380`, the renderer's marker
arm `:1201–1213`, the fold `:1258`) and D-61 (the scrolled screen's
mid-turn composition — the prompt span at the bottom rows, the elapsed
row above it, the marker above that, the peers above that — carried by
one `ScrollBottom` struct `:2085–2102` and one budget
`scrolled_bottom_rows` `:2179–2197` that the drawer `:2330–2338`, the
wheel arm `:3299–3307` and the drag arm `:3376–3385` all ask, with
`view_moved = overlay.view_active()` ungated by the turn). Its pins
stand green (`:7573`, `:7589`, `:7647`, `:7720`, `:7791`, `:7916`,
`:7956`, `:7982`). The cycle published no release of its own; the
version was afterwards carried to 0.40.0 (`505097a`, recorded by
`856c2bf`).

**"the terminal's display regions — from the top, the scrollable and
mouse-selection-copyable log, the spinner-and-elapsed run display, the
running subagents' situation (the operating time and the processing
content), the bottom-most line reserved for the prompt — and a bare
`Enter` ignored" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05, delivered
at commit `39afeb2` ("During a turn the terminal composes the four
display regions and a bare Enter is ignored"), its sha named by the
tracked ledger's follow-up `fb46cc0` ("Name the delivery commit in the
ledger: the trail's follow-up"), the ledger re-certified at that tip
(BL-M36–BL-M42). The mechanism stands and D-63 is measured against it:
the transcript-backed turn zone as the live screen's sole writer during
a run (`static TURN_ZONE` `src/app.rs:1456`, the struct `:1412`, the arm
`:1501`, the beat `:1683–1756` over the pure `zone_beat_plan`
`:1647–1657`, the span paint `zone_paint_span` `:1845`, the re-anchor
`:1797`, the park `:1611`, the hide/show `:1812`/`:1822`, the handoff
`zone_disarm` `:1555` over `zone_disarm_plan` `:1669–1681`); the peer
rows' processing content over the measured registry transport
(`RegistryEntry.turn`, `src/ipc/registry.rs:45`, the writes `:215`/`:231`,
the reader `live_children_states` `:2256–2278`, the face
`peer_status_rows` `:2236–2250`); and the bare-`Enter` guard at the
editor's arm (`:2657–2669`, the empty check `:2664–2666`). Its decisions
D-56/D-58/D-59 are the ground D-60/D-61 and D-63 stand on. Its pins
stand (`:7088`ff, `:7309`ff, `:7468`ff); the gates at the close **645
unit + 6 integration, 0 failed**, clippy at the baseline. The cycle
published no release of its own; the version was afterwards carried to
0.40.0 (`505097a`).

**FR-36's display ask** — superseded by FR-37a's record and delivered
with it; the FR-36 cycle never published a version or a tag. Its
worktree block (`src/app.rs:1328–1747` at `5257729`) was carried to
delivery as T-76's transcript-backed turn zone.

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)** — closed, 2026-11-04,
delivered at commit `c02862d` ("Prompts typed while a turn runs are
queued and run in order when it ends"), `Cargo.toml` **0.38.0**, the tag
`v0.38.0`, the GitHub release published **2026-11-04T03:52:18Z**; the
gates at the close **633 unit + 6 integration, 0 failed**, clippy
identical to the baseline (a reading D-62 supersedes for this and every
later tree — C-09 in the requirements' §6). The mechanism stands in the
tree and survives this cycle untouched (the requirements' §6): the
`Pending` branch's acceptor feeding the shared `InputState` in both
REPL loops, `Enter` enqueueing `AgentInput::UserPrompt`, the helpers
`queued_count` (`src/app.rs:1320`)/`queue_release_allowed` (`:1335`)/
`paint_queue_status` (`:1345`, the `[queued: N]` face), the drain as
shipped, the approval-draft snapshot
`DraftSnapshot`/`snapshot_draft`/`restore_draft` (`src/app.rs:2610–2642`),
the pins rewritten
(`a_prompt_typed_during_a_turn_is_queued_and_arrives_while_it_runs`
superseding the shipped pending-loss pin, and its five companions).
T-74/T-75 stand ✅ in that cycle's closed ledger.

Version trail: FR-32 `7fe1370` (the version 0.35.0, never tagged) →
FR-33 `af48be5` (the version 0.36.0, never tagged) → `ddeaf67` (FR-34's
code; the tag `v0.36.0` on it) → v0.37.0 `5a2d7f0` (FR-34) → v0.38.0
`c02862d` (FR-35) → `881eda9` (the ledger tracked) → v0.39.0 `4f7fad6`
(the version release) → `96a576f` (the records ride the tracked ledger)
→ `3af66d4` (the ledger trail extended; BL-M32 recorded) → `5336a03`
(the BL-M32 records rode its commit) → `5257729` (the FR-36 documents'
rewrite) → `c2f78b3` (the FR-37/FR-38 cycle's ledger rewrite) →
`39afeb2` (the delivery commit — the cycle's implementation, records and
work log) → `fb46cc0` (the tracked ledger's follow-up, naming `39afeb2`
— the rule that a delivery commit's sha is nameable only from the commit
that follows it) → `db68d3f` (the FR-39/FR-40 delivery commit) →
`306f425` (the trail's follow-up, naming `db68d3f`) → `85f9ce9` (that
follow-up's own work log) → v0.40.0 `505097a` (the version carry the
user directed after the delivery) → `856c2bf` (the version carry's own
record in the tracked ledger) → `eec12f9` (the refactoring — one HTTP
error chain, one test context, no dead code; 26 files, +139/−314) →
`064ce50` (the display fix and the ledger's record) → v0.41.0 `25fe13c`
(the version carry the user directed after the delivery) → `3badc04`
(the trail's follow-up naming `25fe13c`). The FR-44 cycle's delivery lands
at `122f320` ("Add the codex-code backend driving the installed Codex CLI
like claude-code" — the backend, the wiring, the docs, the version carry
to 0.42.0, the ledger's FR-44 record and the work logs `_008`/`_009`),
named by the follow-up `af82155` (its work log `2026-11-05_010.md`). The
FR-44d amendment's delivery lands at `4fee899` ("Refresh the example's
sample models to the newest evidenced ids: the FR-44d amendment" — the
example's refreshed ids, the quoting surfaces, the version carry to
0.43.0, the ledger's FR-44d record and the work log `_011`), named by
the follow-up that lands after it — the trail rule holds that a
delivery commit's sha is nameable only from the commit that follows it.
This follow-up's own work log is `2026-11-05_012.md`.

Earlier cycles stand in the requirements document's §12 and the design
document's §5: **"the subagents' auto-exit — permanent (FR-32),
configurable and default-on (FR-33); the 'prompt too long' repair
(FR-34)"** — closed, 2026-11-03 (FR-34 released v0.37.0, the tag on
`5a2d7f0`; the tag `v0.36.0` on `ddeaf67` stands restored — BL-M32);
**"the multi-angle subagent analysis and the subagents' self-exit"
(FR-31)** — investigation and report (this cycle's closest precedent in
kind); **FR-29/FR-30** — `spawn` in the live `[tools] enabled`;
**"the click rule's exclusion of the elapsed display; the
cancelled-by-user display" (FR-27/FR-28)** — v0.34.0 (`c022909`; the
tag's final target `745dbb2`); **"the scroll marker's face — 「Message
below」…" (FR-26)** — v0.33.0 (`6cc6661`), its face and its
`view_active` gate standing, its **place** on the prompt pin FR-39's
correction; **the agent's running display cycle (FR-22–FR-24)** —
v0.32.0 (`e8ad594` + `e0ecb1b`; the clock as the screen's bottom row
then — the geometry FR-37a superseded and FR-40 brought the scrolled
screen into line with); **the mouse defects cycle (FR-18–FR-21)** —
v0.31.0 (`052e230`), its drag/release-offset rule standing and re-bound
by FR-40; **add strata (FR-13–FR-17)** — v0.30.0–0.30.2 (`b706889`,
`70e6b5b`, `a766757`); **the peer-prompt cycles** — v0.29.0 (`045d569`),
v0.28.0 (`d237a7a`), v0.27.0 (`afcbe70`); **the mouse input cycle** —
v0.25.0 (`3884d16`) with the three 機能追加 addenda closed as v0.26.0
(`251a5bb`).

## T-100 — the post-delivery amendment (✅, depends on T-99)

Delivered. The user's chat ask (「example/config.example.tomlのサンプルモ
デルを最新のモデルにして」) rode the standing post-addition clause of the
instruction as rewritten 2026-11-05 17:37 as **FR-44d**:

1. `example/config.example.toml`: the `codex-code` block joined ahead of
   `--- OpenAI (Codex) ---` and every sample `model` refreshed to the
   newest ids, each evidenced live — claude `claude-opus-5-5` (the
   Anthropic lineup the docs name), claude-code `opus` (the alias the
   running `~/.claude/settings.json` itself carries), codex and codex-code
   `gpt-5.6-terra` (the Codex CLI's own live default, `codex exec -m`
   verified; the `gpt-6*` ids ChatGPT auth rejects), ollama
   `glm-5.3-flash:cloud` (the server's own newest-GLM pointer after it
   rejected `glm-5.4`), opencode local `claude-sonnet-5-5`, opencode-go
   `minimax-m3.1-flash` (the newest open-weights id with tools on the
   `/zen/go/v1` catalogue; `hy4` not open-weights).
2. The shipped sample surfaces the READMEs and `doc/config.md` carry were
   refreshed to the same ids — the READMEs' backend tables now
   `claude-opus-5-5` / `gpt-5.6-terra` / `glm-5.3-flash:cloud` /
   `claude-sonnet-5-5` (and `opencode-go`'s row to `minimax-m3.1-flash`
   with its API text), the sample blocks the same, `doc/config.md` §4.3's
   full-featured block too — because the user's ask (「ドキュメント更新」
   as the standing clause reads) names the documentation; `doc/config.md`
   §4.2's verification block and the two READMEs' ollama verification
   sentences moved to `glm-5.3-flash:cloud` — the refreshed id was itself
   re-verified live (`glm-5.3-flash:cloud` answered `AIPROBE-93`,
   thinking-streamed, 321B), so the verification target is a
   live-tested engine on the newest id, not a catalogue pointer — and
   `src/config.rs`'s
   `DEFAULT_CONFIG` and the `unwrap_or_else` fallbacks stand untouched
   (NFR-14: shipped contracts).
3. The kind lists in `doc/usage.md` and `doc/tools.md` name `codex-code`
   (the join the delivery commit left out of the two lists).
4. The version carried **0.42.0 → 0.43.0** (`Cargo.toml`, `Cargo.lock`),
   the CHANGELOG's `[0.43.0]` entry recording the refresh.

Depends on: T-99.

Design trace: design §2 (TPR-26), §4 (W-68).

