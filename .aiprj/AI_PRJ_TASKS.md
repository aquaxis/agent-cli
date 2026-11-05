# AI_PRJ_TASKS.md — Task List

Project: `agent-cli`
Cycle: "the refactoring — contradictions and waste in the source are removed,
`src/ai`'s provider error chain and the tools' test context each collapse to
one implementation, clippy falls silent, and the documents follow the source;
and the prompt's bottom row is prompt-only — the elapsed row never takes it"
(FR-41, FR-42) (`.aiprj/instructions.md`, rewritten 2026-11-05 11:21 (+0900))
Paired with `AI_PRJ_REQUIREMENTS.md`, `AI_PRJ_DESIGN.md`.

Status: ⬜ not started / 🟨 in progress / ✅ done / ⛔ blocked
Article 3 of the AI Coding Guidelines: record progress updates in this
file whenever a task changes state.

---

## Overview

The instruction (rewritten 2026-11-05 11:21:10 +0900, re-verified
byte-exact at this update; 364 bytes, sha256
`9929f174e6acfef3479949cdacd6eab34b77301539915a19c723e72fd960b098`)
replaces the 2026-11-05 09:31 text the previous cycle's ledger recorded.
Its method sentence and its heading stand; its ask is new: **refactor the
source** — remove the contradictions and the waste the feature additions
left behind — and **correct the documents to the source**. The previous
cycles stand: FR-37/FR-38 (the four display regions, the bare-`Enter`
guard) at `39afeb2`, named by the follow-up `fb46cc0`; FR-39/FR-40 (the
marker above the prompt, the scrolled screen's four-region pin) at
`db68d3f`, named by `306f425`. The cycle's asks are recorded as **FR-41**
(the refactoring) and **FR-42** (the display defect the refactoring round
surfaced) with their baselines verified in the tree (BL-M43–BL-M45), the
design decisions **D-62–D-64** and the tasks **T-87–T-92** below.

**Both are delivered.** T-87–T-91 stand ✅ in the task list and their
execution record stands below it. The refactoring is one commit of its
own (`eec12f9` "Refactor: one HTTP error chain, one test context, no dead
code", 26 files, +139/−314) so the display repair and the ledger's record
stand apart from it. The tree the cycle started from was `856c2bf` (the
`main` tip, the version carry's record); the version stands at **0.40.0**,
no `v0.40.0` tag exists, the tag `v0.39.0` stands on `4f7fad6`. The gates
are green at the delivered tip: **647 unit + 6 integration, 0 failed**
(the baseline's 645 + 6 plus the two new pins of FR-42, no test lost), and
clippy reports **0 warnings** — the seven baseline warnings fixed at their
sites rather than tolerated (C-09, D-62).

The two asks' baselines, as the tree stood before the work:

- **The duplication and the dead code** (BL-M43, now superseded by D-62):
  the provider HTTP-error chain was assembled five times — once per
  provider, verbatim modulo the name — and the tools' test context was
  built eight times, the same nine-field literal in eight files. The
  eleven `#[allow(dead_code)]` blankets turned out to be three different
  things — six hid an item nothing called, four hid an item that is
  called, one hid a test-only item — and clippy carried four warning
  classes the ledger had certified as the baseline. The delivered shape:
  one `http_error` (`src/ai/mod.rs:182–205`), one
  `test_ctx`/`test_ctx_in` (`src/tools/mod.rs:208–228`), the six uncalled
  items deleted, the four called items kept with only their blanket
  removed, `GroupId::generate` made `#[cfg(test)]`, clippy silent.
- **The live prompt block's ownership** (BL-M44, now superseded by D-63):
  `StatusIndicator` paints cursor-relative and only asks that the caret
  stand at a line start; the turn zone was armed at the input loop's own
  submit sites alone. A turn the loop did not submit — a peer prompt over
  IPC (`src/agent.rs:308`), a queued prompt the agent dequeued itself, the
  window after `zone_disarm()` + `render_prompt()` — ran with no zone while
  the caret stood inside the prompt block, and the repaint wrote the
  spinner-and-elapsed row at the caret: the bottom-most prompt row, ten
  times a second. Separately, `zone_beat`'s two paint loops emitted every
  `block_n`/`peers_n` row downward without re-clamping, so a clock block or
  peer band taller than the room above the span painted over the prompt
  span. The delivered shape: `prompt_owned` gates all five cursor-relative
  writers, a 150 ms reconcile arm arms and disarms the zone to the turn
  rather than to the keypress, and the two paint loops stop at their own
  floor.

| ID | Task | Requirements | Design | Depends on | Status |
|---|---|---|---|---|---|
| T-87 | FR-41's implementation: the five providers' HTTP-error chains collapse to one `http_error` in `src/ai/mod.rs`, each call site passing its own name, status, headers, body and context; the eight tool test contexts collapse to one `test_ctx`/`test_ctx_in`, `send_to`'s `build_ctx` delegating to it; the seven clippy warnings fixed at their sites (`is_none_or` ×2, `&PathBuf` → `&Path` ×4, the merged `if let` ×2); and every `#[allow(dead_code)]` blanket read and resolved — the six over an uncalled item deleted (`bytes_to_lines`, `AppError::Tool`/`Ui`/`Agent` and their constructors, `RegistryHandle.dir`, `ConversationLog.path`, `Agent.name`), the four over a called item kept with only the blanket removed (`Agent.config`, `RegistryHandle.socket_path`, `LogEvent`, `ToolRegistry::names`), and `GroupId::generate` made `#[cfg(test)]`. No caller observes a change | FR-41, NFR-14, C-09 | design §1 (D-62), §2 (TPR-24a), §3 | — | ✅ |
| T-88 | FR-41's proof: the shipped suite unchanged — the same tests run before and after, and none was removed, because `bytes_to_lines` carried no test of its own and `GroupId::generate`'s test still runs against the `#[cfg(test)]` function — and clippy silent on the whole tree, so the refactoring's "no contradiction, no waste" is measured rather than asserted | FR-41, NFR-13, C-09 | design §1 (D-62), §4 (W-63) | T-87 | ✅ |
| T-89 | FR-42's implementation: the live prompt block's rows become prompt-only — `StatusIndicator` carries `prompt_owned` and `set_prompt_owned`, and `tick`/`paint`/`repaint`/`clear`/`finish`'s screen write all stand down under it while `finish`'s transcript record still runs; the live prompt render takes ownership before it draws and the arm releases it; `zone_arm` stops re-seeding an armed zone's anchor so the next beat composes over the editor's true extent; a 150 ms reconcile arm in the input loop arms the zone for a turn the loop did not submit and disarms it on the mirror condition; and `zone_beat`'s two paint loops stop at `sweep_to` and at `block_home.min(sweep_to)`, so a truncated block keeps the elapsed row. The elapsed row is composed above the span, its four-region place; the transcript's recording rules untouched | FR-42, NFR-14 | design §1 (D-63), §2 (TPR-24b), §3 | — | ✅ |
| T-90 | FR-42's pins: failure-first against the shapes they replace, then green — `the_indicator_never_paints_over_the_live_prompt_block` (each of the five writers stands down under `prompt_owned`, and `finish` still records) and `arming_the_zone_takes_the_rows_from_the_prompt_block` (the arm releases the ownership); the previous cycles' geometry pins re-run green and unmoved in meaning | FR-42, NFR-13 | design §1 (D-63), §4 (W-64) | T-89 | ✅ |
| T-91 | The documents corrected to the source (the instruction's second sentence as a work item): `C-09`'s "clippy output stays identical to the pre-change baseline" retired for **clippy stays silent** and recorded as a specification change (the requirements' §6); the `src/app.rs` citations the fix's +258 rows shifted re-read content-by-content at the delivered tip and written with the numbers the tree answers; the three documents brought to the source's shape (FR-41/FR-42, BL-M43–BL-M45, D-62–D-64, TPR-24a/b, T-87–T-92, W-63–W-65, AC-35) | — (NFR-11, NFR-12, C-08, C-13) | design §1 (D-64), §3 (the documentation angle), §4 (W-65) | T-87–T-90 | ✅ |
| T-92 | The cycle's record and delivery: the work log written (`2026-11-05_006.md`, embedding the instruction verbatim), the three documents mutually consistent, the instruction re-verified byte-exact at exit, and the delivery committed per the user's direction | — (C-08, C-13, AC-35) | design §4 (W-65) | T-87–T-91 | ✅ |

Sections below the table are the execution record (Article 3) — written as
each task's status changes.

## The cycle's record

- **The instruction was rewritten, and the previous cycles' asks stand** —
  the 2026-11-05 09:31 text (595 bytes, sha256 `eca9d4fc…`) is replaced by
  the 2026-11-05 11:21 text (364 bytes, sha256 `9929f174…`). FR-37/FR-38
  were delivered at `39afeb2` (named by `fb46cc0`) and FR-39/FR-40 at
  `db68d3f` (named by `306f425`); neither is withdrawn. This cycle's first
  ask is the refactoring they left owed, not a correction of them.
- **The refactoring is a refactoring in the strict sense** — no caller
  observes a change (BL-M43, the requirements' §6). Each collapsed chain
  assembles what its own site assembled; each deletion removes a function
  or field whose every reference was the deleted code's own test. The
  unchanged suite is the proof, and clippy's silence is the second half of
  the ask measured (`無駄` reaching the baseline's warnings).
- **The display defect surfaced while reading the regions** — the
  refactoring's third angle (the live prompt block's writers) found the two
  paths by which the running display could take the prompt's bottom row
  (BL-M44). It is recorded as FR-42 because the instruction's method — one
  reading, no detour beyond the ask, and the ask is "no contradiction, no
  waste" — makes a live defect found in that reading part of the same
  cycle. It carries its own pins and changes no shipped contract
  (NFR-14).
- **The documents were edited as work, not only as record** (C-13, D-64) —
  the instruction's second sentence is itself a document ask. `C-09`'s
  superseded text is rewritten, the shifted `src/app.rs` citations are
  re-read rather than renumbered, and the design and task documents follow
  the source.

## T-87 — FR-41's implementation (✅)

Delivered. One function assembles what five providers each assembled:
`http_error(provider, status, headers, text, context)`
(`src/ai/mod.rs:182–205`) recovers the request id from headers or body,
attaches the status and its canonical reason, attaches the body, folds in
the provider's config and key context, derives the pattern-specific hint
and converts to `AppError`. Each site passes what it already held —
`claude.rs:226`, `codex.rs:190` (its own `kind_label`), `llamacpp.rs:188`,
`ollama.rs:209`, `opencode.rs:181` — and each keeps its own module free of
the chain. `extract_request_id` and `ProviderError` are imported where they
are still used and dropped where they are not.

One function fills the tools' test context: `test_ctx()`
(`src/tools/mod.rs:208–210`) over `test_ctx_in(registry_dir)`
(`:216–228`), the nine fields written once with a comment naming what each
default means. The eight tool test modules name it as `ctx` — `bash:115`,
`edit:83`, `glob:88`, `grep:186`, `monitor:117`, `read:84`, `websearch:169`,
`write:54` — and `send_to`'s `build_ctx` (`:320–322`) keeps its own name and
delegates to `test_ctx_in`, which is the only one of the nine that needs a
directory.

clippy's seven warnings are fixed where they were reported, not
suppressed: `.is_none_or(…)` at `commands.rs:140` and `update.rs:303`,
`&Path` in place of `&PathBuf` at `commands.rs:1296`/`:1304` and
`send_to.rs:326`/`:386`/`:433`, and the merged `if let` at `send_to.rs:402`
and `commands.rs:1317`.

The eleven `#[allow(dead_code)]` blankets were read one by one, and they
turned out to be three different things — which is why none was removed as
a class. **Six stood over an item nothing calls**, and the item goes with
the blanket: `bytes_to_lines` and the imports it alone used
(`src/ai/stream.rs`), `AppError::Tool`/`Ui`/`Agent` and the constructors
`tool()`/`ui()`/`agent()` (`src/error.rs`), `RegistryHandle.dir`
(`src/ipc/registry.rs`), `ConversationLog.path` (`src/log.rs`) and
`Agent.name` (`src/agent.rs`, with the two construction sites in
`src/app.rs` and the one test literal that filled it). **Four stood over an
item that is called**, so only the blanket goes — `Agent.config`,
`RegistryHandle.socket_path` (read by the handle's own `Drop`), `LogEvent`
and `ToolRegistry::names` (`src/app.rs:325`) — since deleting the item
would have been a behaviour change disguised as a cleanup. **One is a
test-only item**: `GroupId::generate` (`src/id.rs`) has no production
caller and one test of its own, so its blanket becomes `#[cfg(test)]` and
the production build stops compiling it. 26 files, +139/−314.

Depends on: nothing (the first task of the cycle).

Design trace: design §1 (D-62), §2 (TPR-24a), §3 (the duplication and
dead-code angles), §4 (W-63).

## T-88 — FR-41's proof (✅, depends on T-87)

Delivered, green. The suite is the proof and it is unchanged: the same
tests run before and after, and **none was removed** — `bytes_to_lines`
carried no test of its own, and `GroupId::generate`'s test still runs
against the function the `#[cfg(test)]` attribute keeps. Nothing reachable
was removed, which is what an unchanged passing suite measures; no test was
adjusted to accommodate the collapse, and none was rewritten to hide one.

Clippy reports **no warning at all** on the whole tree — the four baseline
classes are gone, not tolerated (C-09 as rewritten, the requirements' §6).
The refactoring's commit `eec12f9` was gated on its own before the display
fix began (`cargo test` 645 + 6, 0 failed; `cargo clippy` silent), so the
two halves of the cycle are measured separately.

Depends on: T-87.

Design trace: design §1 (D-62), §3 (the duplication, dead-code and pins
angles), §4 (W-63).

## T-89 — FR-42's implementation (✅)

Delivered. The rule — the live prompt block's rows are prompt-only — is
held by one flag and one handoff.

**The flag.** `StatusIndicator` carries `prompt_owned`
(`src/app.rs:4675`, initialised `:4695`) and `set_prompt_owned(on)`
(`:4706–4714`), which drops the painted bookkeeping when it is set on (no
cursor move: the screen is about to be redrawn by its new owner). Every
cursor-relative writer asks it: `tick` (`:4848–4859`, the gate `:4853`),
`paint` (`:4865–4874`, `:4870`), `repaint` (`:4935–4990`, `:4951`),
`clear` (`:4992–5015`, `:5008`) and `finish`'s screen write
(`:5033–5069`, `:5062`). `finish` is split deliberately: the transcript
record above the gate still runs, so the outcome line is never lost — only
withheld from the screen until the disarm's replay. The live prompt render
takes ownership before it draws (`render_prompt_with_run` `:5503`), and the
arm releases it (`zone_arm` `:1508` on the too-short degrade, `:1520`
before the zone lock).

**The handoff.** Arming at the submit sites alone made the zone a property
of the keypress rather than of the turn. A 150 ms reconcile arm in the
input loop (`ZONE_RECONCILE_MS` `:2994–2999`, the arm `:3936–3968`) arms
the zone the moment `turn_in_flight` is seen with no zone armed, and
disarms it on the mirror condition (`!turn_in_flight && zone_armed &&
is_ready`) with the same replay the release path performs. That covers the
paths the loop cannot see — a peer prompt over IPC (`src/agent.rs:308`
calls `process_turn` on the agent's own channel, never through `input_tx`),
a queued prompt the agent dequeued itself, and the window after
`zone_disarm()` + `render_prompt()`. The ticker is skipped while the
overlay is active, because that screen's pin already carries the elapsed
row (`draw_scrolled`, D-61). `zone_arm` (`:1501`) no longer re-seeds an
armed zone's anchor: it refreshes the session and the two visibility flags
and keeps the editor's extent, so the next beat composes over the true span
rather than the bare one-row prompt.

**The floor.** `zone_beat`'s two paint loops stop at their own floor
(`:1739–1756`): the clock block at `sweep_to` (the span's top) and the peer
band at `block_home.min(sweep_to)`. The order of the rows makes the
truncation tolerable — the block's first row is the elapsed row, so what a
short terminal loses is reasoning text, never the elapsed display.

The elapsed row therefore stands above the span, composed by the armed
zone: the four regions' own place. The transcript's recording rules are
untouched — `renderer.render` writes rows, `finish_line` (`:1106`) is the
only recorder.

Depends on: nothing (it touches `src/app.rs` alone, disjoint from T-87).

Design trace: design §1 (D-63), §2 (TPR-24b), §3 (the ownership,
arm-and-handoff and floor angles), §4 (W-64).

## T-90 — FR-42's pins (✅, depends on T-89)

Delivered, green, both first made to fail against the shape they replace.

`the_indicator_never_paints_over_the_live_prompt_block` (`:8400–8452`):
with ownership set, `tick` writes nothing, `paint` writes nothing, `repaint`
writes nothing, `clear` writes nothing, and `finish` still records its
outcome while writing nothing to the screen; with ownership released, each
writes again. It was first written against the ungated writers, where the
paint reached the prompt's row.

`arming_the_zone_takes_the_rows_from_the_prompt_block` (`:8455–8470`):
with the prompt block owning its rows, arming the zone clears the
ownership, so the block's bookkeeping is dropped and the beat — not the
indicator — becomes the writer of the rows above the span. It was first
written against the arm that left the ownership standing, where a peer
prompt's beat and the indicator's repaint both wrote the same rows.

The previous cycles' geometry pins stand green and unmoved in meaning:
`the_clock_row_is_not_the_subagent_lines_price` (`:7573`),
`the_line_survives_the_intersections_and_the_screen_shows_it` (`:7589`),
`the_prompt_span_holds_the_bottom_rows_with_subagents_pinned` (`:7647`),
`the_scrolled_prompt_pin_carries_the_marker_row` (`:7720`),
`the_scrolled_screen_keeps_the_elapsed_row_and_adds_the_marker` (`:7791`),
`elapsed_clock_survives_scrolling_during_a_running_turn` (`:7916`),
`wheel_range_ends_above_the_clock_row` (`:7956`),
`finishing_a_turn_while_scrolled_leaves_the_outcome_line` (`:7982`). D-63
moves no row those pins bind; it decides who writes them.

Depends on: T-89.

Design trace: design §1 (D-63), §3 (the pins angle), §4 (W-64).

## T-91 — the documents corrected to the source (✅)

Delivered. The instruction's second sentence is a work item, and three
corrections followed from reading the documents against the source:

- **`C-09` retired and rewritten.** The shipped constraint certified
  "clippy output stays identical to the pre-change baseline", and the
  baseline was four warning classes — a constraint that asked the tree to
  stay as noisy as it was, which is the waste the instruction's first
  sentence names. FR-41 fixes the seven warnings at their sites; the
  constraint is now **clippy stays silent**, and the retirement is recorded
  as a specification change in the requirements' §6 (NFR-13).
- **The citations re-read.** The display fix's +258 rows shifted every
  `src/app.rs` citation the ledger held. Each was re-read at the delivered
  tip content-by-content rather than renumbered arithmetically, and written
  with the number the tree answers (NFR-11).
- **The documents brought to the source's shape.** FR-41/FR-42, BL-M43–
  BL-M45, D-62–D-64, TPR-24a/b, T-87–T-92, W-63–W-65, AC-35, C-09 and C-13
  are written from the tree, not from the previous cycles' text.

Depends on: T-87–T-90.

Design trace: design §1 (D-64), §3 (the documentation angle), §4 (W-65).

## T-92 — the cycle's record and delivery (✅)

Delivered. The work log `2026-11-05_006.md` embeds the instruction verbatim
(364 bytes, sha256 `9929f174…`) and carries the cycle; the three documents
are mutually consistent (C-08); the instruction was re-verified byte-exact
at exit; and the delivery is committed per the user's direction (AC-35),
with the refactoring's commit already standing on its own (`eec12f9`) so
the display repair and the ledger's record stand apart from it. The cycle
published no release of its own and carried no version: the version stands
at 0.40.0 (commit `505097a`, recorded by `856c2bf`), and no `v0.40.0` tag
exists.

Depends on: T-87–T-91.

Design trace: design §4 (W-65), requirements §13 (State at exit).

## Cross-cutting gates (✅)

- Unit + integration: green at the delivered tip — **647 unit + 6
  integration, 0 failed**. The baseline's 645 + 6 held through the
  refactoring, and the two new pins of FR-42 account for the two added; no
  test was lost and none was adjusted (C-09, NFR-13).
- Clippy: **0 warnings** on the whole tree. The seven baseline warnings are
  fixed at their sites (`commands.rs:140`, `update.rs:303`,
  `commands.rs:1296`/`:1304`, `send_to.rs:326`/`:386`/`:433`/`:402`,
  `commands.rs:1317`) rather than tolerated, so C-09's "identical to the
  baseline" is superseded by "clippy stays silent" (the requirements' §6).
- The `.aiprj` documents mutually consistent (C-08), the work log written
  (`2026-11-05_006.md`; Art. 6/7), the instruction byte-exact at exit, and
  the delivery committed per the user's direction (AC-35).
- The boundary §6's items (the queue contracts, the bare-`Enter` guard, the
  cancel keys, the approval takeover, the headless/piped paths, the
  mouse-capture and clipboard contracts, the peer rows' face and stacking,
  the four display regions and FR-39/FR-40's geometry) unchanged — verified
  by the pins standing green and the shipped flows untouched (NFR-14).

## The previous cycles' record

**"the display corrections — during mouse scrolling the `Message below`
marker stands above the prompt, not below it; and during a running turn a
mouse selection no longer makes the elapsed display vanish nor drop the
selection one row" (FR-39, FR-40)** — closed, 2026-11-05, delivered at
commit `db68d3f`, its sha named by the ledger's follow-up `306f425`, with
the follow-up's own work log `85f9ce9`. Its two decisions stand as the
ground D-63 is measured against: D-60 (the marker on the row immediately
above the prompt block's first row — `scrolled_block_plan` `:2150–2166`,
`scrolled_marker_rows` `:1376–1380`, the renderer's marker arm
`:1201–1213`, the fold `:1258`) and D-61 (the scrolled screen's mid-turn
composition — the prompt span at the bottom rows, the elapsed row above
it, the marker above that, the peers above that — carried by one
`ScrollBottom` struct `:2085–2102` and one budget `scrolled_bottom_rows`
`:2179–2197` that the drawer `:2330–2338`, the wheel arm `:3299–3307` and
the drag arm `:3376–3385` all ask, with `view_moved =
overlay.view_active()` ungated by the turn). Its pins stand green
(`:7573`, `:7589`, `:7647`, `:7720`, `:7791`, `:7916`, `:7956`, `:7982`).
The cycle published no release of its own; the version was afterwards
carried to 0.40.0 (`505097a`, recorded by `856c2bf`).

**"the terminal's display regions — from the top, the scrollable and
mouse-selection-copyable log, the spinner-and-elapsed run display, the
running subagents' situation (the operating time and the processing
content), the bottom-most line reserved for the prompt — and a bare
`Enter` ignored" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05, delivered at
commit `39afeb2` ("During a turn the terminal composes the four display
regions and a bare Enter is ignored"), its sha named by the tracked
ledger's follow-up `fb46cc0` ("Name the delivery commit in the ledger: the
trail's follow-up"), the ledger re-certified at that tip (BL-M36–BL-M42).
The mechanism stands and D-63 is measured against it: the transcript-backed
turn zone as the live screen's sole writer during a run (`static TURN_ZONE`
`src/app.rs:1456`, the struct `:1412`, the arm `:1501`, the beat
`:1683–1756` over the pure `zone_beat_plan` `:1647–1657`, the span paint
`zone_paint_span` `:1845`, the re-anchor `:1797`, the park `:1611`, the
hide/show `:1812`/`:1822`, the handoff `zone_disarm` `:1555` over
`zone_disarm_plan` `:1669–1681`); the peer rows' processing content over
the measured registry transport (`RegistryEntry.turn`,
`src/ipc/registry.rs:45`, the writes `:215`/`:231`, the reader
`live_children_states` `:2256–2278`, the face `peer_status_rows`
`:2236–2250`); and the bare-`Enter` guard at the editor's arm
(`:2657–2669`, the empty check `:2664–2666`). Its decisions D-56/D-58/D-59
are the ground D-60/D-61 and D-63 stand on. Its pins stand (`:7088`ff,
`:7309`ff, `:7468`ff); the gates at the close **645 unit + 6 integration,
0 failed**, clippy at the baseline. The cycle published no release of its
own; the version was afterwards carried to 0.40.0 (`505097a`).

**FR-36's display ask** — superseded by FR-37a's record and delivered with
it; the FR-36 cycle never published a version or a tag. Its worktree block
(`src/app.rs:1328–1747` at `5257729`) was carried to delivery as T-76's
transcript-backed turn zone.

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35)** — closed, 2026-11-04,
delivered at commit `c02862d` ("Prompts typed while a turn runs are queued
and run in order when it ends"), `Cargo.toml` **0.38.0**, the tag
`v0.38.0`, the GitHub release published **2026-11-04T03:52:18Z**; the
gates at the close **633 unit + 6 integration, 0 failed**, clippy
identical to the baseline (a reading D-62 supersedes for this and every
later tree — C-09 in the requirements' §6). The mechanism stands in the
tree and survives this cycle untouched (the requirements' §6): the
`Pending` branch's acceptor feeding the shared `InputState` in both REPL
loops, `Enter` enqueueing `AgentInput::UserPrompt`, the helpers
`queued_count` (`src/app.rs:1320`)/`queue_release_allowed` (`:1335`)/
`paint_queue_status` (`:1345`, the `[queued: N]` face), the drain as
shipped, the approval-draft snapshot
`DraftSnapshot`/`snapshot_draft`/`restore_draft` (`src/app.rs:2610–2642`),
the pins rewritten
(`a_prompt_typed_during_a_turn_is_queued_and_arrives_while_it_runs`
superseding the shipped pending-loss pin, and its five companions). T-74/
T-75 stand ✅ in that cycle's closed ledger.

Version trail: FR-32 `7fe1370` (the version 0.35.0, never tagged) → FR-33
`af48be5` (the version 0.36.0, never tagged) → `ddeaf67` (FR-34's code; the
tag `v0.36.0` on it) → v0.37.0 `5a2d7f0` (FR-34) → v0.38.0 `c02862d` (FR-35)
→ `881eda9` (the ledger tracked) → v0.39.0 `4f7fad6` (the version
release) → `96a576f` (the records ride the tracked ledger) → `3af66d4`
(the ledger trail extended; BL-M32 recorded) → `5336a03` (the BL-M32
records rode its commit) → `5257729` (the FR-36 documents' rewrite) →
`c2f78b3` (the FR-37/FR-38 cycle's ledger rewrite) → `39afeb2` (the
delivery commit — the cycle's implementation, records and work log) →
`fb46cc0` (the tracked ledger's follow-up, naming `39afeb2` — the rule that
a delivery commit's sha is nameable only from the commit that follows it) →
`db68d3f` (the FR-39/FR-40 delivery commit) → `306f425` (the trail's
follow-up, naming `db68d3f`) → `85f9ce9` (that follow-up's own work log) →
v0.40.0 `505097a` (the version carry the user directed after the delivery)
→ `856c2bf` (the version carry's own record in the tracked ledger) →
`eec12f9` (this cycle's refactoring — one HTTP error chain, one test
context, no dead code; 26 files, +139/−314) → the display fix and the
ledger's record (this cycle's delivery).

Earlier cycles stand in the requirements document's §12 and the design
document's §5: **"the subagents' auto-exit — permanent (FR-32),
configurable and default-on (FR-33); the 'prompt too long' repair
(FR-34)"** — closed, 2026-11-03 (FR-34 released v0.37.0, the tag on
`5a2d7f0`; the tag `v0.36.0` on `ddeaf67` stands restored — BL-M32);
**"the multi-angle subagent analysis and the subagents' self-exit"
(FR-31)** — investigation and report; **FR-29/FR-30** — `spawn` in the
live `[tools] enabled`; **"the click rule's exclusion of the elapsed
display; the cancelled-by-user display" (FR-27/FR-28)** — v0.34.0
(`c022909`; the tag's final target `745dbb2`); **"the scroll marker's face
— 「Message below」…" (FR-26)** — v0.33.0 (`6cc6661`), its face and its
`view_active` gate standing, its **place** on the prompt pin FR-39's
correction; **the agent's running display cycle (FR-22–FR-24)** — v0.32.0
(`e8ad594` + `e0ecb1b`; the clock as the screen's bottom row then — the
geometry FR-37a superseded and FR-40 brought the scrolled screen into line
with); **the mouse defects cycle (FR-18–FR-21)** — v0.31.0 (`052e230`), its
drag/release-offset rule standing and re-bound by FR-40; **add strata
(FR-13–FR-17)** — v0.30.0–0.30.2 (`b706889`, `70e6b5b`, `a766757`); **the
peer-prompt cycles** — v0.29.0 (`045d569`), v0.28.0 (`d237a7a`), v0.27.0
(`afcbe70`); **the mouse input cycle** — v0.25.0 (`3884d16`) with the three
機能追加 addenda closed as v0.26.0 (`251a5bb`).
