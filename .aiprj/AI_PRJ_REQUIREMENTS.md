# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the refactoring — contradictions and waste in the source are removed,
`src/ai`'s provider error chain and the tools' test context each collapse to
one implementation, clippy falls silent, and the documents follow the source;
and the prompt's bottom row is prompt-only — the elapsed row never takes it"
(FR-41, FR-42)
(`.aiprj/instructions.md`, rewritten 2026-11-05 11:21 (+0900))
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

`.aiprj/instructions.md` was rewritten at 2026-11-05 11:21:10 +0900
(364 bytes, sha256 `9929f174…`, §2) and replaces the 2026-11-05 09:31
text this ledger recorded. Its method sentence and its heading stand;
its ask is new: **refactor the source** — remove the contradictions and
the waste the feature cycles left behind, and bring the documents into
agreement with the source.

The instruction's ask has two halves, and the cycle records both:

- the **refactoring** (FR-41): the duplication the feature cycles accrued
  is collapsed to one implementation each, the eleven accumulated
  `#[allow(dead_code)]` blankets are each resolved — the six that hid an
  uncalled item lose the item, the four that hid a called item lose only
  the blanket, and the one test-only item becomes `#[cfg(test)]` — and the
  clippy baseline the ledger certified as "identical to the baseline" is
  brought to silence: the four warning classes are gone, not carried. The
  documents are then corrected to the source (§12, §13).
- the **display defect the refactoring round surfaced** (FR-42): with the
  refactoring's gates green, the user reported that the elapsed row
  occasionally stands on the bottom-most prompt row, and that the bottom
  row is to be the prompt's alone. Its cause is a write-ownership gap, not
  a formatting slip (BL-M44, §3), and its repair is the same class of
  geometry statement the previous cycle recorded.

The previous cycles are not withdrawn: FR-39/FR-40 (the marker above the
prompt, the scrolled screen's four-region pin) and FR-37/FR-38 (the four
display regions and the bare-`Enter` guard) stand as delivered and are the
ground both new asks are measured against (§12, §13). The new asks are
recorded as **FR-41** and **FR-42** (§5) with their baselines verified in
the tree (BL-M43–BL-M45, §3), the design decisions D-62–D-64
(`AI_PRJ_DESIGN.md` §1) and the tasks T-87–T-92 (`AI_PRJ_TASKS.md`).

The standing method applies (C-12): investigation and repair are to be
performed by multi-angle subagent analysis, with every load-bearing
behaviour pinned (NFR-13).

## 2. Source instruction

`.aiprj/instructions.md` as reloaded in this session (rewritten
2026-11-05 11:21:10.000000000 +0900, 364 bytes; sha256
`9929f174e6acfef3479949cdacd6eab34b77301539915a19c723e72fd960b098`;
verified byte-exact at every gate of this cycle and re-verified at exit):

````text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する

## 最優先事項

### ソースコードをリファクタリングする

これまでの機能追加などでソースコードに矛盾点や無駄がないか不リファクタリングを実施する
ドキュメントもソースコードに合わせて修正する
````

(Translation: **Feature additions to agent-cli. The work is to be
performed by multi-angle subagent analysis. Top priority — refactor the
source code:** *look for contradictions and waste in the source code
that the feature additions have left behind, and refactor them out; also
correct the documents so they match the source code.*)

The instruction names two asks: the **refactoring** of the source (its
first sentence) and the **documents' correction** to the source (its
second). The previous cycle's instruction — the display corrections,
595 bytes, sha256 `eca9d4fc…` — was replaced outright; its asks are not
withdrawn, they are delivered and standing (§12), and the refactoring
was measured against them.

## 3. Baseline — verified in the tree at this update

Verified 2026-11-05 at `856c2bf`, the `main` tip at the time of the
recording (`Cargo.toml` 0.40.0, the tag `v0.39.0` on `4f7fad6`; no
`v0.40.0` tag exists). Every citation below is the tree's own line. The
refactoring's baselines BL-M40–BL-M42 are the shapes the previous two
cycles were measured against and keep the line numbers they were read at;
the display fix's baselines BL-M43/BL-M44 were read at `eec12f9` (the
refactoring's commit) and their citations are re-read at the delivered
tip, where every earlier `src/app.rs` citation has shifted by the fix's
own +258 rows (§13).

| ID | Fact | Evidence |
|---|---|---|
| BL-M40 | **The shipped shape, superseded by D-60 and FR-39.** The scroll marker was painted **beneath** the prompt block on the scrolled screen's prompt pin. The marker's text rode the prompt pin's `text` field, was set from `overlay.view_active()` in the prompt-pinning draw, and was painted by the renderer's marker arm after the block's last row: with the caret parked, `k = (end_row.max(cursor_row) − cursor_row) + 1` and `MoveDown(k)` landed the row **one below** the block's last row. The row's budget was already reserved above that arithmetic (`scrolled_marker_rows` `:1378–1380`, `1` off the bottom and `0` at it), so the ask was the row's **place**, not the row's count. The delivered place is D-60's (the row immediately above the block's first row, `:1201–1213`); the tip's arithmetic is TPR-23a's. | `src/app.rs` at `fb46cc0` (the pre-delivery tree; the delivered shape supersedes it) |
| BL-M41 | **The shipped shape, superseded by D-61 and FR-40.** During a running turn the scrolled screen's pin was the **clock alone**, while the live screen's bottom composition was the prompt span with the clock above it — the two coordinate spaces disagreed by the prompt's rows. The pin choice was `draw_scrolled`'s: when `overlay_status_row()` answered a row it drew a clock-only `ScrollBottom::Status` variant, whose budget arm was `1 + peer_lines`, which painted the clock at the bottom row and never the prompt span. The live screen's mid-turn composition is the opposite order (T-76: the span at the bottom rows, the clock block above it, the peers above that — `zone_beat_plan` `:1649–1657`). The mouse arms measured their body from the **`Pending` branch** of `bottom_block_rows`, `view_moved ? 2 : 1 + peer_lines`, which counted the clock's row alone and no prompt span, and both arms gated `view_moved` on the turn's absence — so mid-turn the body was short of any composition carrying the span and the marker's row was never counted at all. Both symptoms the instruction names followed: the elapsed row's place was taken by the pin's bottom row (it "disappears" from where it stood) and the selected position landed one row lower (AS-60). The delivered composition is D-61's; the tip's arithmetic is TPR-23b's. | `src/app.rs` at `fb46cc0` (the pre-delivery tree; the delivered shape supersedes it) |
| BL-M42 | The previous cycle is delivered and stands: FR-37 (a, b, c) and FR-38 landed at `39afeb2` ("During a turn the terminal composes the four display regions and a bare Enter is ignored"), its sha was named by the tracked ledger's follow-up `fb46cc0`, and the ledger was re-certified at that tip (the previous cycle's BL-M36–BL-M39). The FR-37a mechanism — the armed turn zone (`static TURN_ZONE` `:1458`, the struct `:1414`, the arm `:1503`, the beat `:1685–1793` over `zone_beat_plan` `:1649–1657`, the handoff `zone_disarm` `:1557` over `zone_disarm_plan` `:1671–1683`) — is the ground both corrections are measured against, and it is untouched by them. The cycle published no release of its own; the version was afterwards carried to 0.40.0 (§12). | `git log`/`git show`/`git status` at 2026-11-05, re-read at the delivered tip |
| BL-M43 | **The shipped shape, superseded by D-62 and FR-41 (the refactoring).** The provider HTTP-error chain was assembled five times, once per provider: each of `src/ai/{claude,codex,llamacpp,ollama,opencode}.rs` built the same `ProviderError` from the same three facts (status, headers, body) and its own context — `ProviderError::new(..).with_http(status, reason).with_body(text).with_request_id(extract_request_id(&headers, &text)).with_context(&self.context).detect_hint().into_app_error()`, verbatim modulo the provider's name. The tools' test context was built eight times: `fn ctx() -> ToolCtx` stood verbatim in `src/tools/{bash,edit,glob,grep,monitor,read,websearch,write}.rs`, all eight literals filling the same nine fields. The accumulated `#[allow(dead_code)]` blankets (11 sites) were three different things, read one by one. **Six hid an item nothing called**, and the item goes with the blanket: `bytes_to_lines` (`src/ai/stream.rs`), `AppError::Tool`/`Ui`/`Agent` and their constructors `tool()`/`ui()`/`agent()` (`src/error.rs`), the `RegistryHandle.dir` field (`src/ipc/registry.rs`), the `ConversationLog.path` field (`src/log.rs`) and `Agent.name` (`src/agent.rs`). **Four hid an item that is called**, so only the blanket goes: `Agent.config`, `RegistryHandle.socket_path`, `LogEvent` and `ToolRegistry::names`. **One is a test-only item**: `GroupId::generate` (`src/id.rs`) becomes `#[cfg(test)]` rather than being deleted, because its own test still forms a group. clippy carried four warning classes the ledger certified as the baseline. The delivered shape is D-62's; the tip's citations are §13's. | `src/ai/*.rs`, `src/tools/*.rs`, `src/error.rs`, `src/id.rs`, `src/ipc/registry.rs`, `src/log.rs`, `src/agent.rs`, `src/ai/stream.rs` at `856c2bf` (the pre-refactoring tree); `eec12f9` records the collapse |
| BL-M44 | **The shipped shape, superseded by D-63 and FR-42 (the display defect).** Two independent write paths could land on the live prompt block's bottom row. First, `StatusIndicator` paints **cursor-relative** — `repaint()` joins its rows with newlines and `paint(at_line_start)` only checks that the caret is at a line start — and the turn zone was armed at the input loop's own submit sites alone (`zone_arm` at `src/app.rs:3711` and `:3762` at `eec12f9`). A turn the loop did not submit — a peer prompt arriving over IPC (`src/agent.rs:308`, `self.process_turn(&event_tx, log.as_ref()).await?` on the agent's own channel) or the window after `zone_disarm()` + `render_prompt()` — therefore ran with no zone, the caret standing inside the prompt block, and `TurnStart` → `start()` (clearing `suspended`) → `paint(at_line_start = true)` → `repaint()` wrote the spinner-and-elapsed row at the caret: the bottom-most prompt row, ten times a second. Second, `zone_beat`'s two paint loops emitted **all** `block_n`/`peers_n` rows downward without re-clamping — `zone_csi_move(0, block_home + i as u16)` and `zone_csi_move(0, peers_top + i as u16)` at `eec12f9`'s `src/app.rs:1711–1721` — so a clock block or peer band taller than the rows above the span still crossed `span_top` and painted over the prompt span, its bottom-most row among them. Both are the one rule's absence: the live prompt block's rows are prompt-only, and nothing else writes there. The symptom the user reported (the elapsed row on the bottom row, occasionally) is the first path; the second is the same rule broken by the composition. The delivered shape is D-63's. | `src/app.rs` at `eec12f9` (the pre-fix tree: `StatusIndicator` `:4546`, `tick` `:4740`, `paint` `:4752`, `repaint` `:4817`, `clear` `:4867`, `finish` `:4898`; `zone_beat`'s loops `:1711–1721`; the arms `zone_arm` `:3711`/`:3762`) |
| BL-M45 | The refactoring and the display fix are separable, and the fix was measured against the refactoring's tree: `eec12f9` ("Refactor: one HTTP error chain, one test context, no dead code") carried the 26 files of D-62 and no `app.rs` behaviour; the display fix then changed `src/app.rs` alone, ±258 rows, and every `src/app.rs` citation the ledger held was re-read at the delivered tip (the shift map is §13's). The previous cycles stand: FR-37/FR-38 at `39afeb2` (named by `fb46cc0`), FR-39/FR-40 at `db68d3f` (named by `306f425`), the version carried to 0.40.0 by `505097a` and recorded by `856c2bf`. Neither this cycle's ask withdraws one of them. | `git log`, `git show --stat eec12f9`, `git diff eec12f9 -- src/app.rs` at 2026-11-05 |

## 3.1 What the instruction does NOT ask for

- **No new feature, region, face or key.** The refactoring adds nothing:
  each duplicated chain collapses to one implementation with the same
  behaviour, and each deletion removes code nothing called (BL-M43). The
  display fix moves no row the display did not already own: the elapsed
  row keeps the place the four regions give it, and the prompt's bottom
  row stays the prompt's (BL-M44). The four regions, the peer rows'
  content, the `[queued: N]` notice, the outcome rows and every key keep
  their shipped meaning (FR-37, FR-38, FR-39, FR-40 stand).
- **No transcript change.** Neither the marker nor the clock row is
  recorded into the transcript today (§7 of the shipped contract, the
  `elapsed_clock_survives_scrolling_during_a_running_turn` pin
  `src/app.rs:7918–7955`); the corrections do not change what is
  recorded, only where the display rows stand.
- **No queue-contract change** — FR-35's acceptor, enqueue, count display,
  queued-prompt semantics and drain stand untouched.
- **No bare-`Enter` change** — FR-38's guard (`:2657–2669`) stands; the
  correction surface is the display, not the input path.
- **No headless or piped-stdin change** — where `show_progress = false`
  or the streams are piped there is no indicator block and no overlay;
  the ask is the interactive terminal's display.
- **No mouse-capture or clipboard contract change** — the session-wide
  capture, the drag-selection and the `clip::copy` routes stand; FR-40
  corrects the **rows** a selection resolves against, not the selection
  mechanism.

## 4. Scope

### 4.1 In scope

- **FR-41** — the refactoring: `src/ai`'s five provider HTTP-error chains
  collapse to one `http_error`; the eight tools' identical test contexts
  collapse to one `test_ctx`; the four classes of clippy warning the
  baseline carried are gone; and every `#[allow(dead_code)]` blanket is
  resolved rather than carried — the dead items deleted, the live ones
  released from the blanket, the test-only one gated on `test`.
- **FR-42** — the display defect: the live prompt block's rows are
  prompt-only, so no cursor-relative repaint and no beat row may land on
  them, and the elapsed row stands where the four regions put it.
- The pinned behaviour for both, the citation remap the fix's line shift
  forces, and the docs/ledger updates.

### 4.2 Out of scope

- The §3.1 list; any change to the queue contracts, the provider
  contracts' *behaviour*, the wire shapes, the transcript format, the
  exit seams or the shipped teardown. A refactoring that changes what a
  caller observes is out of scope by definition (§6).

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-41 | The duplication and the dead code the feature cycles left behind are removed, and the callers observe the same behaviour: the five `src/ai` providers build a rejected response's error through one `http_error` (the same provider name, status, body, request id, context and hint as each site assembled), and the tools' tests build their `ToolCtx` through one `test_ctx`/`test_ctx_in` (the same nine fields). clippy reports **no** warning: the four classes the baseline carried are fixed at their sites, not suppressed. No `#[allow(dead_code)]` blanket hides a function or field nothing calls — each is either called or deleted. Every behaviour the removed code could have had is either preserved or provably unreachable (NFR-13). |
| FR-42 | On the live screen the prompt block's rows are **prompt-only**: no cursor-relative repaint of the running display and no beat row lands on any row of the prompt block, the bottom-most row above all, and the elapsed row is displayed where the four-region order puts it — above the prompt span, composed by the armed turn zone. The zone is armed for every turn, not only those the input loop submitted: a turn the loop did not submit (a peer prompt over IPC, a queued prompt the agent dequeued itself) is seen and handed the rows within one low-frequency tick, and the handoff back to the prompt happens the same way. The beat never writes below the composition's floor: the clock block stops at the span's top and the peer band at the block's first row, so a truncated block keeps the elapsed row and drops only the reasoning rows beneath it. The transcript's recording rules are unchanged. |
| FR-39 | On the scrolled screen with the prompt pinned and the view off the log's bottom, the `Message below` marker stands on the row **above** the prompt block — the row the four-region order reserves between the run/subagent rows and the bottom-most prompt line — and never below the prompt's last row. The prompt block's rows, the body's slice and the wheel/drag budgets move together with it, so the marker's place, the block's place and the body's end agree in every mixed state; at the log's bottom the marker is absent as shipped. |
| FR-40 | While a turn runs, a mouse gesture on the scrolled screen keeps the running display's elapsed row **displayed and in place**: the scrolled screen composes the running turn's bottom rows as the live screen does (the prompt span at the bottom-most rows, the elapsed row above it, the subagent rows above that), and the row a press, a drag and a release resolve against is the row the screen actually shows — the selected position does not move by a row when the gesture opens the scrolled screen, and the elapsed display is not displaced or withheld. The scroll marker of FR-39 keeps its place within the same composition, and the transcript's recording rules are unchanged (the beat rows stay out of the log). |

## 6. Boundary: FR-41/FR-42 vs the shipped contracts

The shipped contracts stand (NFR-14): the queue semantics (FR-35), the
bare-`Enter` guard (FR-38), the cancel keys' meaning and the `Esc`-to-
live-view path, the approval takeover, the outcome rows' recording, the
four exit seams and the teardown/drain, the mouse-capture and clipboard
contracts, the peer rows' face and stacking (FR-37b), the four display
regions and the two geometry corrections of FR-39/FR-40. FR-41 is a
refactoring in the strict sense: **no caller observes a change** — each
collapsed chain assembles what its own site assembled, and each deletion
removes a function or field whose every reference is a test of the code
deleted with it. FR-42 is one rule stated and enforced: the live prompt
block's rows are the prompt's alone.

Three shipped **statements** are superseded within this cycle, each a
specification change recorded as such (NFR-13):

- C-09's "clippy output stays identical to the pre-change baseline" — the
  baseline carried four warning classes, and the instruction's "無駄"
  reaches them; superseded by FR-41's **zero** warnings (the constraint
  becomes "clippy stays silent", not "clippy stays as noisy as it was");
- the running display's freedom to paint wherever the caret stands
  (BL-M44; the `at_line_start` gate alone) — superseded by FR-42's
  prompt-only rule, under which the live prompt block's rows take no
  cursor-relative write at all;
- the beat's unclamped downward paint (BL-M44; `block_n`/`peers_n` rows
  emitted whatever the room above the span) — superseded by FR-42's
  floor: the composition never writes below its own top.

None changes a key path, a recording rule or a wire shape (NFR-14).

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | Behaviour-binding pins: every load-bearing behaviour is pinned by a test first made to fail against the shipped shape. The previous cycles' geometry pins stand re-derived and green at this tip (`the_scrolled_prompt_pin_carries_the_marker_row`, `src/app.rs:7722–7790`; `the_scrolled_screen_keeps_the_elapsed_row_and_adds_the_marker`, `:7793–7839`; `the_clock_row_is_not_the_subagent_lines_price`, `:7575–7588`; `the_line_survives_the_intersections_and_the_screen_shows_it`, `:7591–7606`; `wheel_range_ends_above_the_clock_row`, `:7958–7981`; `the_prompt_span_holds_the_bottom_rows_with_subagents_pinned`, `:7649–7681`; `elapsed_clock_survives_scrolling_during_a_running_turn`, `:7918–7955`; `finishing_a_turn_while_scrolled_leaves_the_outcome_line`, `:7984–8011`). This cycle's two rules are pinned new (FR-42): `the_indicator_never_paints_over_the_live_prompt_block` (`:8400–8452`, first made to fail against the cursor-relative paint it replaces) and `arming_the_zone_takes_the_rows_from_the_prompt_block` (`:8455–8470`, first made to fail against the arm that left the block's ownership standing). FR-41's refactoring is bound by the shipped suite itself: the same **647 unit + 6 integration** tests that ran before it run after it, unchanged, and the tests the deletions removed are the ones that covered only the deleted code. |
| NFR-14 | The changes stay additive to the shipped contracts (the boundary of §6). |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism on the installed binary, not by reading alone. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression: unit + integration counts stay green, and clippy reports **no** warning. Superseded within this cycle (the boundary of §6): the shipped text certified "clippy output stays identical to the pre-change baseline", and the baseline was four warning classes — `map_or` simplification at `commands.rs:137` and `update.rs:303`, `&PathBuf` at `commands.rs:1296`/`:1317–1318` and `send_to.rs:320`/`:335`/`:412`, and the nested `if let` at the same two sites. FR-41 fixes all seven warnings at their sites rather than suppressing them; the constraint is now **clippy stays silent**, and it is met (zero warnings). Certified at this tip: **647 unit + 6 integration, 0 failed**. |
| C-12 | The instruction's method — multi-angle subagent analysis — is performed for the investigation and the repair. |
| C-13 | Write surface for the `.aiprj` documents: the three documents plus the required work logs. The instruction's second sentence is itself a document ask, so this cycle edits the documents as **work**, not only as record: the citations the fix's line shift forces are re-read, C-09's superseded text is rewritten (§6), and the design and task documents follow the source (FR-41/FR-42, D-62–D-64, T-87–T-92). No document carries any mark of authorship or tooling (NFR-12). |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-34 | FR-39 and FR-40 implemented and pinned (the previous cycle, delivered): with the prompt pinned and the view off the bottom, the `Message below` marker stands above the prompt block and the budgets agree with it; during a run the scrolled screen keeps the elapsed row displayed in the four-region place and a press/drag/release resolves against the rows the screen shows. |
| AC-35 | FR-41 and FR-42 implemented and pinned: the five provider error chains collapse to one `http_error` and the eight test contexts to one `test_ctx`/`test_ctx_in`, each caller passing what its own site passed; every `#[allow(dead_code)]` blanket is resolved by a call or a deletion, none left masking an uncalled item; **clippy reports zero warnings**; on the live screen no write lands on the prompt block's rows — the indicator's `tick`/`paint`/`repaint`/`clear`/`finish` stand down while the prompt owns them, the zone is armed for a turn the loop did not submit within one reconcile tick, and a truncated beat keeps the elapsed row and stops at the span's top — with the elapsed row displayed above the span; both rules pinned failure-first; gates green (**647 unit + 6 integration, 0 failed**, clippy silent); every `src/app.rs` citation in the ledger re-read at the delivered tip; the documents consistent, the work log written, and the delivery committed. |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-59 | 「プロンプトの上」 names the row **immediately above** the prompt block's first row (the bottom-most region boundary), not the top of the screen and not the space above a multi-row wrapped block: the marker is the last row before the prompt, so a wrapped prompt keeps the marker directly above its first row. | Grounds FR-39's place; read from the instruction and verified against BL-M40's arithmetic. |
| AS-60 | The instruction's second ask is one defect seen twice: the elapsed row "disappearing" and the selection dropping one row are the same disagreement between the scrolled screen's clock-only pin and the live screen's four-region composition (BL-M41), not two unrelated faults — the single correction FR-40 asks for repairs both. | Grounds FR-40's scope; verified at BL-M41 (the `Pending` branch `:2207–2228` over `scrolled_bottom_rows` `:2181–2199` vs the live order `zone_beat_plan` `:1649–1657`), and repaired by one plan at the delivered tip (`scrolled_block_plan` `:2152–2168`). |
| AS-61 | "マウススクロール中" is the wheel-actuated scrolled screen with the prompt pinned — the only place the marker text is produced (`render_prompt_with_run`'s `view_active().then(…)`, `:5482–5485` over `SCROLLED_MARK` `:1372`). The shipped clock-only pin's marker (the clock's row swapped for the text) was the FR-25a face; under FR-40's composition the marker is the block's own row above the prompt span, and the elapsed row stands above it rather than in its place. | Grounds FR-39's surface; read from the instruction and verified at `:5482–5485`, `:1201–1213`. |
| AS-62 | The gesture's rows are read in the scrolled screen's own coordinate space, so "the selected position does not move" means: the content row under the pointer at the press is the content row selected at the release, and opening the scrolled screen from the live screen's four regions does not re-map the pointer by the prompt's rows. | Grounds FR-40's acceptance; verified at the drag arm's `body` (`:3376–3385`) and the release's offset read (`:3423`), both on the one `scrolled_bottom_rows` (`:2181–2199`) the drawer also asks (`:2332–2340`). |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-39 | design §1 (D-60), §2 (TPR-23a), §3, §4 (W-60) | T-82, T-83 |
| FR-40 | design §1 (D-61), §2 (TPR-23b), §3, §4 (W-61) | T-84, T-85 |
| NFR-13 | design §1 (D-60/D-61's pin consequences), §4 (W-62) | T-83, T-85 |
| NFR-14 | design §3 (the boundary angle) | T-82, T-84 |
| AC-34 | design §1 – §4 | T-82, T-83, T-84, T-85 |
| FR-41 | design §1 (D-62), §2 (TPR-24a), §3, §4 (W-63) | T-87, T-88 |
| FR-42 | design §1 (D-63), §2 (TPR-24b), §3, §4 (W-64) | T-89, T-90 |
| NFR-13 | design §1 (D-62/D-63's pin consequences), §4 (W-65) | T-88, T-90 |
| C-09 | design §3 (the clippy angle), §4 (W-65) | T-87, T-91 |
| AC-35 | design §1 – §4 | T-87–T-92 |

## 12. The previous cycles' record

**"the terminal's display regions — from the top, the scrollable and
mouse-selection-copyable log, the spinner-and-elapsed run display, the
running subagents' situation (the operating time and the processing
content), the bottom-most line reserved for the prompt — and a bare
`Enter` ignored" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05, delivered at
`39afeb2` ("During a turn the terminal composes the four display regions
and a bare Enter is ignored"), the sha named by the tracked ledger's
follow-up `fb46cc0`; the ledger re-certified at that tip (BL-M36–BL-M39,
BL-M42). The mechanism: the transcript-backed turn zone as the live
screen's sole writer during a run (`static TURN_ZONE` `src/app.rs:1458`,
the beat `:1685–1793` over `zone_beat_plan` `:1649–1657`, the handoff
`zone_disarm` `:1557` over `zone_disarm_plan` `:1671–1683`), the peer rows' processing
content over the measured registry transport (`RegistryEntry.turn`,
`src/ipc/registry.rs:45`), and the bare-`Enter` guard at the editor's arm
(`:2657–2669`). The pins stand (`:7088`ff, `:7309`ff, `:7468`ff); the
gates at the close **645 unit + 6 integration, 0 failed**, clippy at the
baseline.

**FR-36's display ask** — superseded by FR-37a's record and delivered
with it; the FR-36 cycle never published a version or a tag.

**"prompt input during a running turn — accepted and queued; the queue
runs in order once the turn completes" (FR-35, a/b/c)** — closed,
2026-11-04: delivered at commit `c02862d` ("Prompts typed while a turn
runs are queued and run in order when it ends"), `Cargo.toml` **0.38.0**,
the tag `v0.38.0`, the GitHub release published
**2026-11-04T03:52:18Z**; the gates at the close **633 unit + 6
integration, 0 failed**, clippy identical to the baseline. The mechanism
stands in the tree and survives this cycle untouched: the `Pending`
branch's acceptor feeding the shared `InputState` in both REPL loops,
`Enter` enqueueing `AgentInput::UserPrompt` onto the shipped channel, the
helpers `queued_count`/`queue_release_allowed`/`paint_queue_status` (the
one-line `[queued: N]` face, `src/app.rs:1312`/`:1327`/`:1337`), the drain
as shipped, the approval-draft snapshot
(`DraftSnapshot`/`snapshot_draft`/`restore_draft`, `:2629–2642`), the
pins rewritten.

**The post-close ledger events (2026-11-04 – 2026-11-05):** the `.aiprj`
ledger brought under git tracking — commit `881eda9` "Track the .aiprj
project ledger: the three documents, work logs, rules"; the version
released as **v0.39.0** — commit `4f7fad6` "Release 0.39.0: the version
carries the ledger tracking", tag `v0.39.0` pushed, the GitHub release
published **2026-11-04T04:24:19Z**; the records riding the tracked
ledger — `96a576f`, `3af66d4`, `5336a03`; then the FR-36 documents'
rewrite `5257729`, the FR-37/FR-38 cycle's ledger rewrite `c2f78b3`, the
delivery `39afeb2` and the follow-up `fb46cc0` (BL-M42); then the
FR-39/FR-40 delivery `db68d3f`, its ledger follow-up `306f425` naming it,
and the version carried to **0.40.0** by the release commit `505097a`
"Release 0.40.0: the display corrections carry the version" (§13).

**Corrections standing in the ledger:** **BL-M30** (the typed-during-a-
turn prompt was lost, not queued — the shipped contract FR-35
overturned), **BL-M31** (FR-34's close-out re-recorded as v0.37.0 — the
release commit `5a2d7f0`, the tag on it, the CHANGELOG `[0.37.0]`),
**BL-M32** (no `v0.35.0` tag exists; the lightweight tag `v0.36.0`
dereferences to `ddeaf67`; FR-32 `7fe1370` and FR-33 `af48be5` are the
in-tree versions 0.35.0/0.36.0, never tagged, no releases) — each with its
evidence in the cycles' records and work logs.

**"the subagents' auto-exit — permanent (FR-32), configurable and
default-on (FR-33); the 'prompt too long' repair (FR-34)"** — closed,
2026-11-03. FR-32 (commit `7fe1370`), FR-33 (commit `af48be5`), FR-34
(commit `ddeaf67`, released as **v0.37.0** — the release commit
`5a2d7f0`).

**"the multi-angle subagent analysis (standing) and the subagents'
self-exit on completion" (FR-31)** — closed, 2026-11-03 (no release —
investigation and report). Superseded by the implementation (FR-32/
FR-33).

**"the multi-angle subagent analysis, added to the configuration and
verified executable" (FR-29/FR-30)** — closed, 2026-11-03 (no release):
`spawn` in the live `[tools] enabled`, the shipped `[spawn]` bounds.

**"the click rule's exclusion of the elapsed display; the
cancelled-by-user display" (FR-27/FR-28)** — closed, **v0.34.0**
(`c022909`; the tag's final target `745dbb2`). **"the scroll marker's face
— 「Message below」…" (FR-26)** — closed, v0.33.0 (`6cc6661`); its face
and its gate stand, its place on the prompt pin is FR-39's correction.
**"the agent's running display, the subagent status display…"** —
closed, v0.32.0 (`e8ad594` + `e0ecb1b`). **"the mouse defects…"** —
v0.31.0 (`052e230`). **"add strata…"** — v0.30.0–0.30.2 (`b706889`,
`70e6b5b`, `a766757`). **v0.29.0** (`045d569`). **v0.28.0** (`d237a7a`).
**v0.27.0** (`afcbe70`). **v0.25.0** (`3884d16`) with the addenda
**v0.26.0** (`251a5bb`).

## 13. Delivery status

| Item | State |
|---|---|
| FR-39 (the marker above the prompt) | **Delivered.** The marker paints on the row immediately above the prompt block's first row, in one composition with the running turn's rows: the renderer's `above_rows` counts it and folds it into `cursor_row` so the next erase climbs past it (`src/app.rs` `render` `:1156–1260`, the marker arm `:1201–1213`, the fold `:1258`, `clear_block` `:1078–1102`); the text is produced from `overlay.view_active()` at `:5482–5485` and handed over as the pin's `text` field (`:5478–5485`), read back by `marker_row()` (`:2110–2112`). One pure plan answers the place and the count for the drawer and both arms (`scrolled_block_plan` `:2152–2168`, `scrolled_bottom_rows` `:2181–2199`, delegated to by `bottom_block_rows` `:2207–2228`); the wheel arm (`:3300–3306`) and the drag arm (`:3377–3383`) ask that same budget. Pinned by `the_scrolled_prompt_pin_carries_the_marker_row` (`:7722–7790`), re-derived failure-first (NFR-13); T-82/T-83 ✅. |
| FR-40 (the running turn's elapsed row and the selection's rows) | **Delivered.** `ScrollBottom` is a single struct carrying `run_row` beside the prompt's fields (`:2087–2099`, the accessor `:2114–2119`), so the scrolled screen composes the live order in every state: the prompt span at the bottom-most rows, the elapsed row directly above it, the marker above that, the peers above that (`run_row_rows` `:2125–2127`, `scrolled_block_plan` `:2152–2168`). `draw_scrolled` (`:5523–5556`) takes the row from `overlay_status_row()` (`:4897–4904`) and hands it to `render_prompt_with_run` (`:5438–5518`); the renderer paints it above the block (`:1189–1199`), display-only (`finish_line` `:1108–1139` is the only recorder). Both mouse arms pass `view_moved = overlay.view_active()` with no turn gate (`:3299`, `:3376`), so a press, a drag and a release resolve against the rows the screen shows (AS-62). Pinned by `the_scrolled_screen_keeps_the_elapsed_row_and_adds_the_marker` (`:7793–7839`), `the_prompt_span_holds_the_bottom_rows_with_subagents_pinned` (`:7649–7681`), `elapsed_clock_survives_scrolling_during_a_running_turn` (`:7918–7955`) and the re-derived `wheel_range_ends_above_the_clock_row` (`:7958–7981`), `the_clock_row_is_not_the_subagent_lines_price` (`:7575–7588`), `the_line_survives_the_intersections_and_the_screen_shows_it` (`:7591–7606`), `drag_release_resolves_at_the_release_time_offset` (`:7279–7306`); T-84/T-85 ✅. |
| FR-41 (the refactoring) | **Delivered.** One `http_error(provider, status, headers, text, context)` stands at `src/ai/mod.rs:182–206` and assembles the chain once — `extract_request_id` over headers-or-body, `with_http(status.as_u16(), canonical_reason)`, `with_body`, `with_request_id`, `with_context`, `detect_hint`, `into_app_error` — and the five providers call it with what each site assembled: `claude.rs:226`, `codex.rs:190` (its own `kind_label`), `llamacpp.rs:188`, `ollama.rs:209`, `opencode.rs:181`. One `test_ctx()` (`src/tools/mod.rs:208–210`, over `test_ctx_in` `:216–228`) builds the nine-field `ToolCtx`; the eight tool test modules name it as `ctx` (`bash:115`, `edit:83`, `glob:88`, `grep:186`, `monitor:117`, `read:84`, `websearch:169`, `write:54`) and `send_to`'s `build_ctx` (`:320–322`) delegates to `test_ctx_in`. clippy reports **0 warnings**: the seven baseline warnings are fixed at their sites — `.is_none_or(…)` at `commands.rs:140` and `update.rs:303`, `&Path` at `commands.rs:1296`/`:1304` and `send_to.rs:326`/`:386`/`:433`, the merged `if let` at `send_to.rs:402` and `commands.rs:1317`. The eleven `#[allow(dead_code)]` blankets are resolved item by item: six stood over an item nothing calls and the item goes with the blanket — `bytes_to_lines` and the imports it alone used (`src/ai/stream.rs`), `AppError::Tool`/`Ui`/`Agent` and the constructors `tool()`/`ui()`/`agent()` (`src/error.rs`), `RegistryHandle.dir` (`src/ipc/registry.rs`), `ConversationLog.path` (`src/log.rs`) and `Agent.name` (`src/agent.rs`, with its two construction sites in `src/app.rs`); four stood over an item that is called, so only the blanket goes — `Agent.config`, `RegistryHandle.socket_path` (read by the handle's own `Drop`), `LogEvent` and `ToolRegistry::names` (`src/app.rs:325`); and one is a test-only item, `GroupId::generate` (`src/id.rs`), whose blanket becomes `#[cfg(test)]`. 26 files, +139/−314, no behaviour changed; the suite is the binding pin (NFR-13); T-87/T-88 ✅. |
| FR-42 (the prompt's bottom row is prompt-only) | **Delivered.** `StatusIndicator` carries `prompt_owned` (`src/app.rs:4675`, initialised `:4695`) and `set_prompt_owned` (`:4706–4714`), and the flag stands every cursor-relative write down: `tick` (`:4848–4859`, the gate `:4853`), `paint` (`:4865–4874`, the gate `:4870`), `repaint` (`:4935–4990`, the gate `:4951`), `clear` (`:4992–5015`, the gate `:5008`) and `finish`'s write (`:5033–5069`, the gate `:5062` — the transcript record above it still runs, so the outcome line is never lost, only withheld from the screen until the disarm's replay). The live prompt render takes ownership before it draws (`render_prompt_with_run` `:5503`) and the arm releases it (`zone_arm` `:1508` on the too-short degrade, `:1520` before the zone lock), so the state enters with the painted bookkeeping dropped and no cursor move. `zone_arm` no longer re-seeds an armed zone's anchor: it refreshes the session and the two visibility flags and keeps the editor's extent, so the next beat composes over the true span rather than the bare one-row prompt. The reconcile arm (`run_input_loop_raw` `:3936–3968`, its 150 ms ticker `:2994–2999`) hands the rows to the zone the moment a turn is seen in flight that the loop did not submit — a peer prompt over IPC (`src/agent.rs:308` calls `process_turn` on the agent's own channel), a queued prompt the agent dequeued itself — and hands them back on the mirror condition (`!turn_in_flight && zone_armed && is_ready`, `:3957–3967`). The beat's two paint loops stop at their own floor (`zone_beat` `:1739–1756`): the clock block at `sweep_to` (the span's top) and the peer band at `block_home.min(sweep_to)`, so a truncated block keeps its first row — the elapsed row — and drops only the reasoning rows beneath it. The elapsed row is displayed above the span by the beat, and the transcript's recording rules are untouched (`finish_line` `:1106` is still the only recorder). Pinned by `the_indicator_never_paints_over_the_live_prompt_block` (`:8400–8452`) and `arming_the_zone_takes_the_rows_from_the_prompt_block` (`:8455–8470`), both first made to fail against the shape they replace (NFR-13); T-89/T-90 ✅. |
| FR-37 / FR-38 (the previous cycle) | Closed and standing (BL-M42, commit `39afeb2`, named by `fb46cc0`). Neither ask is withdrawn; both corrections were measured against their delivered shape and neither is touched by them. |
| State at exit | The instruction is re-verified byte-exact (364 bytes, sha256 `9929f174…`, mtime 2026-11-05 11:21:10.184391024 +0900); the cycle is recorded and delivered in the three documents (FR-41/FR-42, BL-M43–BL-M45, D-62–D-64, TPR-24a/b, T-87–T-92, W-63–W-65, NFR-13/NFR-14, AC-35, C-09/C-13) and in the product tree. The gates are green at the delivered tip: **647 unit + 6 integration, 0 failed** (the baseline's 645 + 6 plus the two new pins of FR-42, no test lost); clippy **0 warnings** on the whole tree — the four baseline classes are fixed at their sites rather than tolerated, so the "identical to the baseline" reading of C-09 is retired for this cycle and replaced by the stronger one recorded there. The refactoring is one commit of its own (`eec12f9` "Refactor: one HTTP error chain, one test context, no dead code", 26 files, +139/−314) so the display repair and the ledger's record stand apart from it. The FR-42 repair followed the refactoring in the same cycle because the instruction's standing method — no detour beyond the ask, and the ask is "no contradiction, no waste" — made a live display defect found while reading the regions part of the same reading; it carries its own pins and changes no shipped contract (NFR-14). The cycle published no release of its own and carried no version of its own; afterwards, on the user's direction, the version was carried **0.40.0 → 0.41.0** — commit `25fe13c` "Release 0.41.0: the refactoring and the prompt's own rows carry the version", carrying `Cargo.toml`, `Cargo.lock` and `CHANGELOG.md` alone (the changelog's `0.40.0` content, which had stayed under `[Unreleased]` through `505097a`, moved under its own `## [0.40.0]` heading and a `## [0.41.0]` section written for this cycle), the gates green at it; no `v0.40.0` tag exists and no `v0.41.0` tag was made, and the tag `v0.39.0` stands on `4f7fad6`. The work logs `2026-11-05_002.md` (the display corrections' recording), `2026-11-05_003.md` (their execution), `2026-11-05_004.md` (their trail's follow-up), `2026-11-05_005.md` (the 0.40.0 carry), `2026-11-05_006.md` (this cycle) and `2026-11-05_007.md` (the 0.41.0 carry and the push) carry the record; the delivery is committed per the user's direction (AC-35) and named by the ledger's own follow-up, not by itself. |
