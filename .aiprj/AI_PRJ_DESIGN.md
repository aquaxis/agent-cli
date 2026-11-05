# AI_PRJ_DESIGN.md — Design Specification

Project: `agent-cli`
Cycle: "the refactoring — contradictions and waste in the source are removed,
`src/ai`'s provider error chain and the tools' test context each collapse to
one implementation, clippy falls silent, and the documents follow the source;
and the prompt's bottom row is prompt-only — the elapsed row never takes it"
(FR-41, FR-42)
(`.aiprj/instructions.md`, rewritten 2026-11-05 11:21 (+0900))
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_TASKS.md`

Requirement IDs (FR-/NFR-/AS-/AC-) refer to `AI_PRJ_REQUIREMENTS.md`.

---

## 1. What is decided, and what it costs

The instruction was rewritten at 2026-11-05 11:21 (+0900) and names one
ask with two halves: **refactor the source** — remove the contradictions
and the waste the feature cycles left behind — and **correct the
documents to the source**. Every earlier decision stands untouched:
D-27 and D-28 (the clock's row and the marker's place, both since
corrected), D-50 (the queue's one-face display), D-52–D-54 (the running
turn's composition), D-56 (the four-region shape), D-58 (the peer rows'
content over the measured registry transport), D-59 (the bare-`Enter`
guard), D-60 (the marker above the prompt block) and D-61 (the scrolled
screen's mid-turn composition). What this cycle decides is **how the
duplication and the dead code go**, and **which writer owns the prompt
block's rows**:

- **D-62 — one chain for a rejected response, one context for the tools'
  tests, and no `#[allow(dead_code)]` blanket standing over an uncalled
  item (delivered).** Five providers assembled the same diagnostic error
  from the same three facts. The chain — recover the request id from
  headers or body, attach the status and the reason phrase, attach the
  body, fold in the provider's config and key context, derive the hint,
  convert — is one function now, `http_error(provider, status, headers,
  text, context)` (`src/ai/mod.rs:182–205`), and each site passes what it
  assembled: `claude.rs:226`, `codex.rs:190` (its own `kind_label`),
  `llamacpp.rs:188`, `ollama.rs:209`, `opencode.rs:181`. The eight tool
  test modules each carried a verbatim `fn ctx() -> ToolCtx`; one
  `test_ctx()` (`src/tools/mod.rs:208–210`) over `test_ctx_in`
  (`:216–228`) fills the nine fields once, and each module names it as
  `ctx` (`bash:115`, `edit:83`, `glob:88`, `grep:186`, `monitor:117`,
  `read:84`, `websearch:169`, `write:54`); `send_to`'s `build_ctx`
  (`:320–322`) keeps its own name and delegates. The eleven
  `#[allow(dead_code)]` blankets are resolved one way or the other, and
  the two answers are kept apart because the instruction's "無駄" reaches
  both: **six stand over an item nothing calls**, and the item goes —
  `bytes_to_lines` and the imports it alone used (`src/ai/stream.rs`),
  `AppError::Tool`/`Ui`/`Agent` and the constructors `tool()`/`ui()`/
  `agent()` (`src/error.rs`; the enum and the impl keep their annotation
  off because their remaining variants and constructors are called),
  `RegistryHandle.dir` (`src/ipc/registry.rs`) and `ConversationLog.path`
  (`src/log.rs`), and `Agent.name` (`src/agent.rs`, with the two
  construction sites in `src/app.rs` and the one test literal that filled
  it); **four stand over an item that is called**, so only the blanket
  goes — `Agent.config` (`src/agent.rs:178`), `RegistryHandle.socket_path`
  (`src/ipc/registry.rs:110`), `LogEvent` (`src/log.rs:14`) and
  `ToolRegistry::names` (`src/tools/mod.rs:197`, called at
  `src/app.rs:325`); and **one is a test-only item**, `GroupId::generate`
  (`src/id.rs`), whose blanket becomes `#[cfg(test)]` — no caller forms a
  group, and its own test still forms one, so the function stays and the
  production build stops compiling it. clippy's four baseline
  classes are fixed rather than tolerated (`.is_none_or(…)` at
  `commands.rs:140` and `update.rs:303`; `&Path` at `commands.rs:1296`/
  `:1304` and `send_to.rs:326`/`:386`/`:433`; the merged `if let` at
  `send_to.rs:402` and `commands.rs:1317`), so the tree reports **no**
  warning. No call, no wire shape, no transcript and no key changes — the
  shipped suite is the whole proof, and it is unchanged (NFR-13).

- **D-63 — the live prompt block's rows are prompt-only, and the running
  display reaches them through no path (delivered).** The rule is one
  sentence, and the tree had no single place holding it. `StatusIndicator`
  writes **relative to the caret** — `repaint()` joins its rows with
  newlines, `paint(at_line_start)` only asks that the caret stand at a
  line start — and the turn zone that silences it was armed at the input
  loop's own submit sites alone (`zone_arm` at `src/app.rs:3711` and
  `:3762` at `eec12f9`). A turn the loop did not submit — a peer prompt
  arriving over IPC (`src/agent.rs:308` calls `process_turn` on the
  agent's own channel), a queued prompt the agent dequeued itself, or the
  window after `zone_disarm()` + `render_prompt()` — ran with no zone
  while the caret stood inside the prompt block, so `TurnStart` →
  `start()` (clearing `suspended`) → `paint(at_line_start = true)` →
  `repaint()` wrote the spinner-and-elapsed row **at the caret**: the
  bottom-most prompt row, ten times a second. Two decisions close it.
  First, **ownership**: `StatusIndicator` carries `prompt_owned`
  (`:4675`, initialised `:4695`) set by `set_prompt_owned` (`:4706–4714`),
  and every cursor-relative writer stands down while it is set — `tick`
  (`:4848–4859`, the gate `:4853`), `paint` (`:4865–4874`, `:4870`),
  `repaint` (`:4935–4990`, `:4951`), `clear` (`:4992–5015`, `:5008`) and
  `finish`'s screen write (`:5033–5069`, `:5062`; the transcript record
  above it still runs, so the outcome line is never lost — only withheld
  from the screen until the disarm's replay). The live prompt render
  takes ownership before it draws (`render_prompt_with_run` `:5503`) and
  the arm releases it (`zone_arm` `:1508` on the too-short degrade,
  `:1520` before the zone lock), so the state is entered with the painted
  bookkeeping dropped and no cursor move. Second, **the handoff is not
  the submit site's to make**: a 150 ms reconcile arm in the input loop
  (`ZONE_RECONCILE_MS` `:2994–2999`, the arm `:3936–3968`) arms the zone
  the moment a turn is seen in flight that the loop did not submit, and
  disarms it on the mirror condition (`!turn_in_flight && zone_armed &&
  is_ready`) — the same replay the release path performs, so the prompt
  and the outcome row come back the same way they do after a submitted
  turn. `zone_arm` (`:1501`) no longer re-seeds an armed zone's anchor: it
  refreshes the session and the two visibility flags and keeps the
  editor's extent, so the next beat composes over the true span rather
  than the bare one-row prompt. The beat's own two paint loops were the
  second half of the same rule: they emitted **all** `block_n`/`peers_n`
  rows downward, so a clock block or peer band taller than the room above
  the span crossed `span_top` and painted over the prompt span. They now
  stop at their own floor (`zone_beat` `:1739–1756`): the block at
  `sweep_to` (the span's top) and the peer band at
  `block_home.min(sweep_to)`, so a truncated block keeps its first row —
  the elapsed row — and drops only the reasoning rows beneath it. The
  elapsed row therefore stands above the span, composed by the zone,
  which is where the four regions put it; the transcript's recording
  rules are untouched (`finish_line` `:1106` is still the only recorder).

- **D-64 — the documents follow the source, and a superseded statement is
  retired, not annotated (delivered).** The instruction's second sentence
  is a work item, not a formality: `C-09` certified "clippy output stays
  identical to the pre-change baseline", and the baseline was four
  warning classes — a constraint that asked the tree to stay as noisy as
  it was, which is exactly the waste the first sentence names. D-62 fixes
  the warnings; the constraint is rewritten to **clippy stays silent**,
  and the requirements' §6 records the retirement as a specification
  change (NFR-13). The same applies to the geometry the previous cycle
  delivered: the citations the display fix's +258 rows shifted were
  re-read at the delivered tip and written with their new numbers rather
  than left to drift, and the two statements D-63 supersedes (the running
  display's freedom to paint wherever the caret stands, and the beat's
  unclamped downward paint) are recorded in §6 beside the one already
  retired there. A document that cites a line it has not re-read is the
  contradiction the instruction asks to remove.

The three decisions are separable in the tree and are delivered as two
commits: D-62 alone (`eec12f9`, 26 files, +139/−314, no `src/app.rs`
behaviour), then D-63 and D-64 together in `src/app.rs` and the documents.
D-62 and D-63 touch disjoint files, and the suite bound both.

## 2. The touchpoints this cycle touches (TPR-24)

| # | Touchpoint | Evidence (the tree at `856c2bf` / `eec12f9`) |
|---|---|---|
| TPR-24a | The duplicated chains and the dead code — the five provider error assemblies, the eight tool test contexts, the eleven `#[allow(dead_code)]` sites and the items behind them, and the seven clippy warnings | `src/ai/{claude:226,codex:190,llamacpp:188,ollama:209,opencode:181}.rs` → `src/ai/mod.rs:182–205`; `src/tools/{bash:115,edit:83,glob:88,grep:186,monitor:117,read:84,websearch:169,write:54}.rs` → `src/tools/mod.rs:208–228`; `send_to.rs:320–322`; the deletions in `src/ai/stream.rs`, `src/error.rs`, `src/id.rs`, `src/ipc/registry.rs`, `src/log.rs`, `src/agent.rs`; the clippy sites `commands.rs:140`, `update.rs:303`, `commands.rs:1296`/`:1304`, `send_to.rs:326`/`:386`/`:433`/`:402`, `commands.rs:1317` |
| TPR-24b | The live prompt block's ownership — the indicator's five cursor-relative writers, the arm and disarm, the input loop's reconcile arm, and the beat's two paint loops | `src/app.rs` (`StatusIndicator` `:4675`, `set_prompt_owned` `:4706–4714`, `tick` `:4848–4859`, `paint` `:4865–4874`, `repaint` `:4935–4990`, `clear` `:4992–5015`, `finish` `:5033–5069`; `zone_arm` `:1501–1520`; `zone_beat` `:1683–1756`; `ZONE_RECONCILE_MS` `:2994–2999`; the reconcile arm `:3936–3968`; `render_prompt_with_run` `:5436–5518`, the ownership take `:5503`; `finish_line` `:1106`) |

## 3. Analysis verdicts (C-12) — the investigation stage

- **The duplication angle.** The five provider sites were compared text
  after text: each built `ProviderError::new(<name>)`, attached
  `with_http(status, reason)`, `with_body(text)`, `with_request_id(
  extract_request_id(&headers, &text))`, `with_context(&self.context)`,
  `detect_hint()` and `into_app_error()`, differing only in the name and
  in what each had already parsed. The parameterisation is therefore the
  site's own inputs — name, status, headers, body, context — and nothing
  else; a caller that had already taken `headers` and `text` by value
  passes them, one that needs them after the error passes them by
  reference. The eight test contexts were compared field by field: the
  same nine-field literal in eight files, so the collapse is a function,
  not an abstraction. What the collapses cost is one indirection per
  call and the loss of the per-file copy that made a local change easy;
  what they buy is that a field added to `ToolCtx` or a step added to the
  chain is written once. The instruction's "無駄" is exactly this trade
  taken in this direction.
- **The dead-code angle.** A blanket `#[allow(dead_code)]` (11 sites at
  `856c2bf`) is a promise the compiler is not allowed to check, and the
  eleven turned out to be three different things — which is why they were
  read one by one rather than removed as a class. **Six hid an item
  nothing called**, and the item goes with the blanket: `bytes_to_lines`
  and the imports it alone used (`src/ai/stream.rs`), `AppError::Tool`/
  `Ui`/`Agent` and the constructors `tool()`/`ui()`/`agent()`
  (`src/error.rs`), `RegistryHandle.dir` (`src/ipc/registry.rs`),
  `ConversationLog.path` (`src/log.rs`) and `Agent.name`
  (`src/agent.rs`, constructed at two `src/app.rs` sites and read at
  none). **Four hid an item that is called** — `Agent.config`,
  `RegistryHandle.socket_path` (read by the handle's own `Drop`),
  `LogEvent` and `ToolRegistry::names` (`src/app.rs:325`) — so only the
  blanket goes; deleting the item would have been a behaviour change
  disguised as a cleanup. **One is a test-only item**: `GroupId::generate`
  has no production caller and one test of its own, so its blanket becomes
  `#[cfg(test)]` and the production build stops compiling it. The rule
  applied is not "delete what nothing calls" but "read what the attribute
  hid, then delete only what nothing calls"; nothing reachable was
  removed, which is what the unchanged suite measures.
- **The ownership angle.** A cursor-relative painter is safe only if
  nobody else owns the rows at the caret. The tree had two owners on
  overlapping states and no rule between them: the zone owns the rows
  above the span while it is armed, the prompt block owns the rows it
  drew, and the indicator's `at_line_start` gate proves nothing about
  **which** rows those are — a caret parked in the prompt after a
  `render_prompt` satisfies it as readily as a caret at an idle line.
  The shipped gate was therefore a proxy for the real condition, and the
  proxy held only on the path the input loop itself drove. Stating the
  real condition as a flag (`prompt_owned`) and gating all five writers on
  it is the smallest shape that cannot be re-broken by a new call site:
  the flag is set where the rows are drawn and cleared where they are
  given up, and every writer asks it. `finish`'s split — the transcript
  record kept, the screen write withheld — is the same rule applied to the
  one writer that also records: losing the outcome line would be a
  behaviour change, and it is not one.
- **The arm-and-handoff angle.** Arming at the submit sites alone makes
  the zone a property of the *keypress*, not of the *turn*. The turn's
  identity is `turn_in_flight` with its terminal events, and the loop
  already watches both (`agent_idle_rx` releases the prompt state). A
  low-frequency ticker that reconciles the zone to the turn — arm when a
  turn is in flight and the zone is not, disarm when neither holds and
  the prompt is ready — closes every path at once, including the ones the
  loop cannot see (a peer prompt over IPC arrives on the agent's channel,
  never through `input_tx`). The period is 150 ms because the loop only
  has to *notice*: the wrong-row window is bounded by one tick, no race
  with the beat exists (both run on the loop's own thread for the arm,
  and the beat takes the zone lock), and the ticker is skipped while the
  overlay is active, because that screen's pin carries the elapsed row
  itself (`draw_scrolled`, D-61).
- **The floor angle.** `zone_beat_plan` clamped `block_home` and
  `peers_top` with `saturating_sub`, which answers where the composition
  *starts* but says nothing about how far it may paint. The draw loops
  then emitted every row downward, so the clamp was invisible: a tall
  clock block crossed `span_top` and the prompt span's rows were
  overwritten, its bottom-most row among them. The floor belongs to the
  draw, not the plan: the block stops at `sweep_to`, the band at
  `block_home.min(sweep_to)`. The order of the rows makes the truncation
  tolerable — the block's first row is the elapsed row, so what a short
  terminal loses is reasoning text, never the elapsed display the
  instruction names.
- **The documentation angle.** The instruction's second sentence makes
  the documents part of the work. Three things followed from reading
  them against the source: `C-09`'s "identical to the baseline" is
  superseded by D-62 and is rewritten as "clippy stays silent" (§6 of the
  requirements); the `src/app.rs` citations the fix shifted by +258 rows
  were re-read content-by-content, not renumbered arithmetically, and
  written with the numbers the tree answers; and this design and the
  task list were brought to the source's shape (D-62–D-64, TPR-24a/b,
  T-87–T-92, W-63–W-65). No claim was carried whose evidence could not be
  re-read at the tip (NFR-11).
- **The pins angle.** D-62 is bound by the shipped suite: the same tests
  run before and after, and the only tests removed with the code are the
  ones that covered the deleted items alone — of which there were none:
  `bytes_to_lines` carried no test of its own, and
  `GroupId::generate`'s test still runs, because the function it exercises
  still exists (`#[cfg(test)]` rather than deleted). D-63 gets
  new pins, both first made to fail against the shape they replace: the
  indicator's five writers each stand down under `prompt_owned`
  (`the_indicator_never_paints_over_the_live_prompt_block`,
  `src/app.rs:8400–8452`) and arming the zone takes the rows from the
  prompt block (`arming_the_zone_takes_the_rows_from_the_prompt_block`,
  `:8455–8470`). The previous cycles' geometry pins stand green and
  unmoved in meaning (`:7573`, `:7589`, `:7647`, `:7720`, `:7791`,
  `:7916`, `:7956`, `:7982`).

## 4. Verification of the cycle (the gate table W-63–W-65)

| Step | Check | Covers | Task | Status |
|---|---|---|---|---|
| W-63 | FR-41 — the refactoring: one `http_error` carrying every provider's chain, one `test_ctx`/`test_ctx_in` filling the tools' context, no `#[allow(dead_code)]` blanket over an uncalled item, clippy silent, and no caller observing a change (the suite unchanged) | FR-41, NFR-13, NFR-14, C-09 | TPR-24a | ✅ |
| W-64 | FR-42 — the prompt's rows prompt-only: the indicator's five cursor-relative writers stand down under ownership, the ownership is taken where the block is drawn and released where it is given up, the zone is armed for a turn the loop did not submit within one reconcile tick and handed back on the mirror condition, the beat stops at the span's top and keeps the elapsed row, and the transcript's recording rules hold | FR-42, NFR-13, NFR-14 | TPR-24b | ✅ |
| W-65 | The pins, the gates and the documents: the two new pins failure-first and green, the shipped suite unchanged, clippy silent, every `src/app.rs` citation in the ledger re-read at the tip, the three documents mutually consistent with the source, the work log written, the delivery committed | FR-41, FR-42, NFR-13, C-08, C-09, NFR-12, AC-35 | TPR-24a/b | ✅ |

*The rows stand ✅ at the delivered tip. The gates were executed live at
this update: **647 unit + 6 integration, 0 failed** — the baseline's
645 + 6 plus this cycle's two new pins, no test lost — and clippy reports
**no warning at all**, the seven baseline warnings having been fixed at
their sites rather than suppressed (D-62, C-09). The refactoring's commit
`eec12f9` was gated on its own before the display fix began, and the fix
was gated again after it landed.*

## 5. The previous cycles (recorded after their documents were retired)

**"the display corrections — during mouse scrolling the `Message below`
marker stands above the prompt, not below it; and during a running turn a
mouse selection no longer makes the elapsed display vanish nor drop the
selection one row" (FR-39, FR-40)** — closed, 2026-11-05: delivered at
`db68d3f`, its sha named by the ledger's follow-up `306f425`, with the
follow-up's own work log `85f9ce9`. Its two decisions stand as the ground
this cycle's D-63 is measured against: D-60 (the marker on the row above
the prompt block's first row, `scrolled_block_plan` `:2150–2166`,
`scrolled_marker_rows` `:1376–1380`, the renderer's marker arm
`:1201–1213` and the fold `:1258`) and D-61 (the scrolled screen's
mid-turn composition — the prompt span at the bottom rows, the elapsed
row above it, the marker above that, the peers above that — carried by
one `ScrollBottom` struct `:2085–2102` and one budget
`scrolled_bottom_rows` `:2179–2197` that the drawer `:2330–2338`, the
wheel arm `:3299–3307` and the drag arm `:3376–3385` all ask, with
`view_moved = overlay.view_active()` ungated by the turn). Its pins stand
green (`:7573`, `:7589`, `:7647`, `:7720`, `:7791`, `:7916`, `:7956`,
`:7982`). The cycle published no release of its own; the version was
afterwards carried to **0.40.0** by the release commit `505097a`
"Release 0.40.0: the display corrections carry the version", recorded by
`856c2bf` (the requirements' §12/§13).

**"the terminal's display regions — from the top, the scrollable and
mouse-selection-copyable log, the spinner-and-elapsed run display, the
running subagents' situation (the operating time and the processing
content), the bottom-most line reserved for the prompt — and a bare
`Enter` ignored" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05: delivered at
commit `39afeb2` ("During a turn the terminal composes the four display
regions and a bare Enter is ignored"), its sha named by the tracked
ledger's follow-up `fb46cc0`, the ledger re-certified at that tip
(BL-M36–BL-M42). The mechanism stands and D-63 is measured against it:
the transcript-backed turn zone as the live screen's sole writer during a
run (`static TURN_ZONE` `src/app.rs:1456`, the struct `:1412`, the arm
`:1501`, the beat `:1683–1756` over the pure `zone_beat_plan`
`:1647–1657`, the span paint `zone_paint_span` `:1845`, the re-anchor
`:1797`, the park `:1611`, the hide/show `:1812`/`:1822`, the handoff
`zone_disarm` `:1555` over `zone_disarm_plan` `:1669–1681`); the peer
rows' processing content over the measured registry transport
(`RegistryEntry.turn`, `src/ipc/registry.rs:45`, the writes `:215`/`:231`,
the reader `live_children_states` `:2256–2278`, the face
`peer_status_rows` `:2236–2250`); and the bare-`Enter` guard at the
editor's arm (`:2657–2669`, the empty check `:2664–2666`). Its decisions
D-56/D-58/D-59 are the ground D-60/D-61 and D-63 stand on. The cycle
published no release of its own; the version was afterwards carried to
0.40.0 by `505097a`.

**FR-36's display ask** — superseded by FR-37a's record and delivered with
it; the FR-36 cycle never published a version or a tag. Its worktree block
(`src/app.rs:1328–1747` at `5257729`) was carried to delivery as T-76's
transcript-backed turn zone (AS-58, the previous cycle's BL-M39).

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35, a/b/c)** — closed,
2026-11-04: delivered at commit `c02862d` ("Prompts typed while a turn
runs are queued and run in order when it ends"), `Cargo.toml` **0.38.0**,
the tag `v0.38.0`, the GitHub release published **2026-11-04T03:52:18Z**;
the gates at the close: **633 unit + 6 integration, 0 failed**, clippy
identical to the baseline (a reading D-62 supersedes for this and every
later tree — C-09 in the requirements' §6). The mechanism stands in the
tree and survives this cycle untouched (the boundary §6 of the
requirements): the `Pending` branch's acceptor feeding the shared
`InputState` in both REPL loops, `Enter` enqueueing `AgentInput::UserPrompt`
onto the shipped channel, the helpers
`queued_count`/`queue_release_allowed`/`paint_queue_status` (the one-line
`[queued: N]` face, `src/app.rs:1345`; the comment's `:1327`/`:1337`
range in the tasks' history is read at the pre-fix tree), the drain as
shipped, the approval-draft snapshot
(`DraftSnapshot`/`snapshot_draft`/`restore_draft`, `src/app.rs:2629–2642`),
the pins rewritten. The full cycle history (the post-close ledger events
`881eda9` → v0.39.0 `4f7fad6` → `96a576f` → `3af66d4` → `5336a03` →
`5257729` → `c2f78b3` → `39afeb2` → `fb46cc0` → `db68d3f` → `306f425` →
`85f9ce9` → v0.40.0 `505097a`; the BL-M30/BL-M31/BL-M32 corrections)
stands in the requirements document's §12 and the task list's history
section — one shared record, not duplicated here (C-08).

**"the subagents' auto-exit — permanent (FR-32), configurable and
default-on (FR-33); the 'prompt too long' repair (FR-34)"** — closed,
2026-11-03: FR-32 (commit `7fe1370`, the version 0.35.0 never tagged),
FR-33 (commit `af48be5`, version 0.36.0, equally never tagged), FR-34
(commit `ddeaf67`, released as **v0.37.0** — the release commit
`5a2d7f0`; the tag `v0.36.0` on `ddeaf67` stands restored — BL-M32, with
BL-M31 narrowed). Gates at the close: **629 unit + 6 integration,
0 failed**; clippy at the baseline.

**"the multi-angle subagent analysis, added to the configuration and
verified executable" (FR-29/FR-30)** — closed, 2026-11-03 (no release):
`spawn` in the live `[tools] enabled` (the shipped default omits it),
the shipped `[spawn]` bounds; live round trips grounding the verdict;
R-1 carried, R-2 withdrawn as false.

**"the click rule's exclusion of the elapsed display; the
cancelled-by-user display" (FR-27/FR-28)** — closed, **v0.34.0** (commit
`c022909`, the tag's final target `745dbb2`): the click rule's strict
top excluding the elapsed row, the query-error fallback disarmed; the
three cancel prints `[info] cancelled by user` styled `Role::Info`.
While the zone is armed the hit is judged against the zone's own clock
extent (`zone_click_hits_clock`, `src/app.rs:1292`, over the pure
`click_hits_clock_block` `:1309`), the shipped pump-query path standing
when it is not. D-61's composition keeps the block where the click rule
expects it — the beat's own `clock_top`/`clock_rows` bookkeeping
(`:1438–1439`, set at `:1784–1791`) is unchanged, and D-63's floor leaves
it where the click rule finds it.

**"the scroll marker's face — 「Message below」…" (FR-26)** — closed,
v0.33.0 (`6cc6661`). Its face and its `view_active` gate stand; its
**place** on the prompt pin is FR-39's correction (D-60). **"the agent's
running display, the subagent status display, and the prompt's resumption
after a cancel" (FR-22–FR-24)** — closed, v0.32.0 (`e8ad594` + `e0ecb1b`;
the clock as the screen's bottom row then — the geometry FR-37a
superseded, and the scrolled screen's clock-only pin D-61 now brings into
line with the live composition). **"the mouse defects…"** — v0.31.0
(`052e230`), its drag/release-offset rule
(`drag_release_resolves_at_the_release_time_offset`,
`src/app.rs:7277–7304`) standing and re-bound by D-61. **"add strata as a connection target…"** — v0.30.0–0.30.2
(`b706889`, `70e6b5b`, `a766757`). **"a received peer prompt sometimes
never starts processing…"** — closed, v0.29.0 (`045d569`). **"…left
unprocessed"** — v0.28.0 (`d237a7a`). **"…never progresses"** —
v0.27.0 (`afcbe70`). **"mouse input turns into escape sequences, and
typing freezes"** — v0.25.0 (`3884d16`) with the three 機能追加 addenda
closed as v0.26.0 (`251a5bb`).
