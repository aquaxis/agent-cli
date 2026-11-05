# AI_PRJ_REQUIREMENTS.md — Requirements Specification

Project: `agent-cli`
Cycle: "the prompt-input queueing feature is deleted — prompts typed during a
running turn are again ignored (only the cancel keys act), the deletion is
documented, the version carries to 0.44.0, and the release stands the latest
version"
(FR-45 a/b/c; the standing delivery sentence of the instruction, whose
最優先事項 names the removal)
(`.aiprj/instructions.md`, reloaded 2026-11-06 — current source text and
sha256 recorded in §2)
Paired with: `AI_PRJ_DESIGN.md`, `AI_PRJ_TASKS.md`

---

## 1. Purpose

`.aiprj/instructions.md` was rewritten at 2026-11-06 02:02:15 +0900
(349 bytes, sha256 `06620319…`, §2). The delivery sentence
「機能追加した場合は、ドキュメント更新、example更新、マイナーバージョン
アップ、commit, push, リリースを最新バージョンにする」 stands (now with
the release explicitly named), and the method sentence 「作業は多角
サブエージェント解析で実施する」 stands; the ask is new: **「プロンプトの
入力のキューイング機能を削除する」** — delete the prompt-input queueing
feature. This named the mechanism the FR-35 cycle (v0.38.0, delivered
`c02862d`, 2026-11-04) shipped: prompts typed while a turn runs are
accepted, edited and enqueued onto the agent's input channel
(`AgentInput::UserPrompt`), run in order once the turn completes, shown as
the `[queued: N]` one-line notice, and refused mid-turn only for `!` and `/`
lines. **The removal is delivered** (§13): the REPL stands at the shipped
pre-FR-35 contract the queue's own pins recorded as overturned (the
CHANGELOG's `[0.38.0]` record) — while a turn runs the input loop ignores
every keypress except `Esc` and `Ctrl-C`, and the prompt returns to the
user as soon as the turn's terminal event arrives.

The previous cycle's ask (FR-44, the codex-code backend and its FR-44d
amendment) stands delivered and closed (§12). FR-44e — the latest release
corresponding to the completed minor version — was verified delivered at
this cycle's ground (the release `v0.43.0`, `isLatest: true`, the tag on
`4fee899`), and this cycle repeated the same machinery at **0.44.0**
(FR-45c).

## 2. Source instruction

`.aiprj/instructions.md` reloaded for this update (sha256
`066203190b3c5fc30e3a43e1cd09819998bd462886f9eb5a87f4757c591a4e0b`,
349 bytes, mtime 2026-11-06 02:02:15 +0900):

```text
# agent-cliの機能追加

作業は多角サブエージェント解析で実施する
機能追加した場合は、ドキュメント更新、example更新、マイナーバージョンアップ、commit, push, リリースを最新バージョンにする

## 最優先事項

プロンプトの入力のキューイング機能を削除する
```

The standing delivery sentence supplied this cycle's completion shape
(documentation, example, minor version bump, commit, push, the latest
release), and the 最優先事項 named the change it completes.

## 3. Baseline — verified in the tree at this cycle's start

Verified 2026-11-06 in the working tree at `d5cbbbe` (v0.43.0 delivered
and pushed — the FR-44d amendment's delivery commit `4fee899` and its
trail's follow-up `d5cbbbe`, per the trail rule), before the removal
writes stood:

| ID | Fact | Evidence |
|---|---|---|
| BL-M52 | **The queue feature stood shipped at v0.43.0.** The input loop kept its editor open during a turn (`PromptState::Pending`, `run_input_loop`): a plain prompt line typed mid-turn was echoed and enqueued as `AgentInput::UserPrompt` (`src/app.rs:3595`, the `input_tx.send(...)` in the Pending submit arm), and the agent ran it after the turn; the queue's face was the `[queued: N]` notice (`paint_queue_status` `src/app.rs:1345`, `QUEUED_NOTICE_PREFIX` `src/app.rs:1364`); the depth probe was `queued_count` (`src/app.rs:1320`, the channel's `max_capacity() − capacity()`); the release gate was `queue_release_allowed` (`src/app.rs:1335`, `pending && queue_len == 0 && !turn_in_flight`; the raw loop's idle arm applied it at `src/app.rs:3077`, the piped loop's at its own release arm); `!`/`/` lines were refused mid-turn (`src/app.rs:3511`, 「queue accepts prompts only; run ! and / lines at the idle prompt」); the approval-time draft pair (`DraftSnapshot`/`snapshot_draft`/`restore_draft`, `src/app.rs:2610–2624`) protected the half-typed queued draft (the raw loop's snapshot arms `:3131–3134`/`:3715`); the agent-side boundary pin recorded the queued prompt surviving a cancelled turn (`src/agent.rs:1419`, the FR-35b comment `:1457`); the queue's unit pins stood (`a_prompt_typed_during_a_turn_is_queued_and_arrives_while_it_runs` `src/app.rs:6634`, `stale_idle_signals_do_not_withhold_a_prompt_typed_during_a_turn` `:6707`, `queue_release_gate_answers_every_combination` `:8815`, `queued_count_tracks_a_live_channel_without_receiving` `:8836`). | `src/app.rs`, `src/agent.rs` at `d5cbbbe`; the line citations above |
| BL-M53 | **The v0.43.0 ground: the version, the release and the docs surface.** `Cargo.toml` was 0.43.0; the tag `v0.43.0` was verified pointing at the delivery commit `4fee899` (`git rev-parse v0.43.0^{commit}`); the GitHub release `v0.43.0 — the example's sample models refreshed to the newest evidenced ids` stood created 2026-11-05T10:57:38Z with `isLatest: true` programmatically read from the releases API (`isLatest: false` returned for `v0.42.0` and `v0.41.0`); FR-44e was thereby delivered and closed. The CHANGELOG carried `[Unreleased]` empty and `[0.43.0]` latest; the FR-35 record stood in the CHANGELOG's `[0.38.0]` section (the queue's shipped description, 2026-11-04) as history. The shipped READMEs and `doc/usage.md` carried **no** queue-feature sentence (verified by text search: their turn-time sentences name only `Esc`/`Ctrl-C` stopping, scrollback and markers), so the shipped document the change wrote was the CHANGELOG's new entry — `doc/architecture.md` §7's queued-deliveries sentences and `doc/troubleshooting.md`'s queue pages name the shutdown drain and the peer-prompt queue, other mechanisms (§3.1). The suite at the delivered tip: **669 unit + 6 integration, 0 failed** — the state whose counts the FR-45 gates re-derived (the six deleted pins lower the count by six; the delivered close stands at **663 unit + 6 integration, 0 failed**, T-102's record). | `Cargo.toml`, `CHANGELOG.md`, `gh api repos/aquaxis/agent-cli/releases` (`isLatest` fields), `git rev-parse v0.43.0^{commit}`, the README keymap rows read this cycle, this cycle's own suite run |

## 3.1 What the instruction did NOT ask for

- **No change to the shipped codex-code, codex or claude-code backends.**
  The provider surface stands untouched by the removal (verified: zero
  writes outside `src/app.rs`/`src/agent.rs`/the CHANGELOG/the version
  files).
- **No change to shipped REPL behaviour beyond the queue's own shapes**
  (§4.1, FR-45a's boundary): the turn-zone composition, the approval
  takeover, the cancel keys' shipped meaning, the peer rows, the shutdown
  drain's truthful answers to queued deliveries, and every other shipped
  contract of §6 stand untouched.
- **No history rewrite.** The CHANGELOG's `[0.38.0]` entry stands as the
  record of what FR-35 delivered; the ledgers' §12 records stand closed.

## 4. Scope

### 4.1 In scope (delivered)

- **FR-45a** — the queue feature's removal: the mid-turn submit path (the
  Pending arm's echo, the `UserPrompt` enqueue and the mid-turn refusal
  notices for `!`/`/` lines), the `[queued: N]` notice, the queue depth
  probe and the release gate's queue conjuncts, the approval-time draft
  pair the queue motivated, the queue's four unit pins and the agent-side
  boundary pin — during a running turn the loop ignores plain typed input
  (only `Esc`/`Ctrl-C` act), and the prompt returns to `Ready` on the
  turn's terminal event.
- **FR-45b** — the documentation followed: the CHANGELOG's `[Unreleased]`
  became the `[0.44.0]` entry recording the removal in user language.
  The shipped READMEs and `doc/usage.md` carried no queue sentence to
  correct (BL-M53's verified absence); the CHANGELOG's `[0.38.0]` entry
  and §12's cycle records stand as history.
- **FR-45c** — the completion: the minor version carry 0.43.0 → 0.44.0
  (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's `[0.44.0]` heading), the
  delivery commit, the push, and the release published as the latest
  version.

### 4.2 Out of scope

- The §3.1 list: the provider backends, every shipped contract of §6
  outside the queue's own shapes, and the shutdown-drain/peer-prompt
  queue mechanisms the docs also call queues.
- A git tag beyond the release's own (the release machinery owns it).

## 5. Functional requirements

| ID | Requirement |
|---|---|
| FR-45a | The prompt-input queueing feature is deleted: during a running turn the input loop no longer keeps its editing line open for agent prompts — plain typed lines are ignored (the shipped pre-FR-35 contract the CHANGELOG `[0.38.0]` entry records), only `Esc`/`Ctrl-C` (the cancel keys' shipped meaning) act, the prompt returns to `Ready` on the turn's terminal event (the idle-notification release, no queue-depth conjunct), the `[queued: N]` notice, `queued_count`, `queue_release_allowed`'s queue conjuncts, the mid-turn refusal of `!`/`/` lines, the approval-time draft pair, the queue's four unit pins and the agent-side queued-prompt boundary pin are all removed with the paths they served; the `Pending` state's remaining duties (the terminal event's wait, the approval arm) stand in their pre-queue shape. Every deletion bound to its file:line evidence before the change (BL-M52). — **Delivered** (T-102; design §1 D-72). |
| FR-45b | The documentation follows the removal: the CHANGELOG's `[0.44.0]` entry records the queue's deletion in user language (what changed for the user: prompts typed mid-turn are no longer queued or accepted — the REPL ignores them until the turn ends, exactly as before v0.38.0; `Esc`/`Ctrl-C` and every other shipped behaviour unchanged). No other shipped document carries a queue-feature sentence (BL-M53's verified absence); the `[0.38.0]` entry and the closed ledgers' records stand as history. — **Delivered** (T-103; design §1 D-73). |
| FR-45c | The completion: the minor version carry 0.43.0 → **0.44.0** (`Cargo.toml`, `Cargo.lock`, the CHANGELOG's `[0.44.0]` heading), the delivery commit(s) of the removal and the documents (the trail rule — a commit's sha is nameable only from the commit that follows it — governs the naming), the push to `origin/main`, and the release published and made the **latest** version (the standing sentence's リリース clause, FR-44e's own verified shape). — **Delivered** (T-103; design §1 D-74). |
| FR-44e | **Delivered and closed** (verified at this cycle's ground, BL-M53): the release corresponding to the completed minor version (then 0.43.0) was published and made the latest release — `v0.43.0` with `isLatest: true` on the GitHub releases list, the tag `v0.43.0` verified on the delivery commit `4fee899`. The FR-45c machinery repeated it at 0.44.0. |

## 6. Boundary: FR-45 and the shipped contracts

The shipped contracts stand (NFR-14): the bare-`Enter` guard (FR-38), the
cancel keys and the `Esc`-to-live-view path, the approval takeover, the
outcome rows' recording, the four exit seams and the teardown/drain, the
mouse-capture and clipboard contracts, the peer rows' face and stacking
(FR-37b), the four display regions and the geometry corrections
FR-39/FR-40, the prompt-only rows FR-42, the refactoring's collapsed
shapes FR-41, and the FR-44 backend additions (the codex-code surface
beside them). **The prompt-input queueing contract (delivered as FR-35,
v0.38.0) is superseded by this cycle's FR-45**: the instruction's
最優先事項 deleted the feature; the boundary records the removal
explicitly rather than carrying the queue as a shipped contract. FR-45
touched only the queue's own surfaces; no other shipped statement — a key
path, a recording rule or a wire shape — changed (NFR-14; verified §13's
boundary record).

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-11 | Evidence-first (standing): every named mechanism carries file:line or a live measurement before any conclusion; claims that cannot be evidenced are withdrawn, not carried. |
| NFR-12 | No document or source file carries any mark of authorship or tooling attribution. |
| NFR-13 | The removal stands pinned: the queue's six pins (four unit + the approval-snapshot pin + the agent-side boundary pin) are deleted with the feature; the surviving suite re-derives the restored pre-queue contract's shapes at green; the gate counts change by the deleted pins (669 − 6 at the removal tree) and stand re-derived at the close (**663 unit + 6 integration, 0 failed**). |
| NFR-14 | The changes stay bounded by the boundary of §6: the queue's own surfaces are deleted; no other shipped contract changes. |
| NFR-15 | Live-verdict (standing): an executability question is answered by executing the mechanism. This cycle's probes: the release-latest verifications (`isLatest` programmatically read for `v0.43.0` and again for `v0.44.0`), the tag-target verifications (`git rev-parse` on both tags), the suite + clippy runs at the removal tree. |

## 8. Constraints

| ID | Constraint |
|---|---|
| C-08 | The three `.aiprj` documents are kept mutually consistent at every update. |
| C-09 | Suite regression standing: unit + integration green, clippy silent, re-run after this cycle's tree changes — re-executed at the removal tree: **663 unit + 6 integration, 0 failed** (669 − the six deleted pins), clippy with the two pre-existing dead-store warnings verified to stand at `d5cbbbe`'s tip as well (a `git stash` re-run of the same test build at HEAD produced the identical warnings — not this cycle's new debt). |
| C-12 | The instruction's method — multi-angle subagent analysis — stands, and the standing method sentence survives in the instruction verbatim. The removal's analysis ran multi-angle (design §3: the mechanism, contracts, scope, live-verdict and docs angles) before D-72 drew the sites. |
| C-13 | Write surface: the three `.aiprj` documents, the work log, and the product files the ask names through the standing delivery sentence (`src/app.rs`, `src/agent.rs`, `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`). Nothing else was written; no document carries any mark of authorship or tooling (NFR-12). |
| C-14 | The delivery clause — the minor version carry, the commit, the push, the latest release — executed after the removal and its documentation stood complete (the gates green, the CHANGELOG entry written). The delivery commits name the work they deliver. |

## 9. Acceptance criteria

| ID | Criterion |
|---|---|
| AC-38 | FR-45 a/b/c met: the queue feature deleted from `src/app.rs`/`src/agent.rs` with every named surface's file:line evidence recorded before the change (BL-M52), the input loop at the pre-queue contract, the suite green and clippy silent at the removal tree (C-09), the documentation written (the CHANGELOG `[0.44.0]` entry) with every statement bound to the code, the version at **0.44.0** with the release files carried, the delivery committed and **pushed to `origin/main`**, and the release published as the **latest** version; the three `.aiprj` documents mutually consistent (C-08), the work log written per Art. 6/7, and the instruction re-verified byte-exact at exit (NFR-12 throughout). — **Met** (§13). |

## 10. Assumptions

| ID | Item | Handling |
|---|---|---|
| AS-70 | 「プロンプトの入力のキューイング機能を削除する」 named the FR-35-delivered queue feature (the v0.38.0 record, the `[queued: N]` notice, the mid-turn editing line) — not the agent's headless input channel itself (`AgentInput`'s channel, which `spawn`/peers/shutdown share), not the peer-prompt queue, and not the shutdown drain that answers queued deliveries. | Read from the tree (BL-M52's surfaces) and the CHANGELOG's `[0.38.0]` record; applied in FR-45a's surface list. |
| AS-71 | The restored contract is the shipped pre-queue one: while a turn runs the REPL ignores every keypress except `Esc` and `Ctrl-C` (the CHANGELOG `[0.38.0]` entry's own record of what FR-35 changed), and the prompt returns to the user when the turn's terminal event arrives — the state the queue's pins recorded as overturned. | Read from the CHANGELOG record and the deleted pins' own recorded contracts; applied in FR-45a. |
| AS-72 | 「マイナーバージョンアップ」 is the minor bump 0.43.0 → **0.44.0** — the change the standing delivery sentence completes is carried by the project's trail the same way (0.40→0.41→0.42→0.43→0.44), and 「リリースを最新バージョンにする」 is the release published so it is the latest (FR-44e's own verified shape, repeated here). | Read from the instruction, the tree's trail (BL-M53) and FR-44e's record; applied in FR-45c (delivered). |

## 11. Traceability

| Requirement | Design | Task |
|---|---|---|
| FR-45a | design §1 (D-72), §2 (TPR-27), §3 | T-102 ✅ |
| FR-45b | design §1 (D-73), §2 (TPR-27), §3, §4 (W-69) | T-103 ✅ |
| FR-45c | design §1 (D-74), §4 (W-69) | T-103 ✅ |
| NFR-13 | design §3 (the pins angle), §4 | T-102 ✅ |
| NFR-15 | design §3 (the live-verdict angle) | T-103 ✅ |
| C-09 | design §4 (W-69) | T-102 ✅ |
| C-14 | design §1 (D-74), §4 (W-69) | T-103 ✅ |
| AC-38 | design §1 – §4 | T-102, T-103, T-104 ✅ |
| FR-44 (delivered cycles) — closed | design §5 | (closed) |
| FR-44e — closed | design §1 (D-74's verification note) | T-101 ✅ |

## 12. The previous cycles' record

**"the codex-code backend — the installed codex CLI driven the way the
claude-code backend drives `claude`, as a ninth `provider.kind` … and the
example's sample models refreshed to the newest evidenced ids" (FR-44
a/b/c/d, FR-44e)** — closed, 2026-11-05: the backend delivered at
`122f320` (named by the follow-up `af82155`), the FR-44d amendment at
`4fee899` (named by `d5cbbbe`), the version carried to **0.42.0** then
**0.43.0**, the release `v0.43.0` published and verified **latest**
(`isLatest: true`, BL-M53 — FR-44e closed), the trail's work logs
`_010`/`_011`/`_012`. Gates at the close **669 unit + 6 integration,
0 failed**, clippy 0 warnings.

**"the codex access investigation" (FR-43 a/b/c)** — closed,
2026-11-05 (no release — the investigation): the verdict D-65 feasible
for delegation-style access, the shape D-66 recorded unbuilt; FR-44's
cycle implemented it (D-67). Work log `2026-11-05_008.md`.

**"the refactoring — contradictions and waste removed… and the prompt's
bottom row made prompt-only" (FR-41, FR-42)** — closed, 2026-11-05:
delivered at `eec12f9` and `064ce50`, the version carried to **0.41.0**
by `25fe13c` and named by the follow-up `3badc04`. Gates **647 unit + 6
integration, 0 failed**, clippy 0 warnings; work logs
`2026-11-05_006.md` and `_007.md`.

**"the display regions…" (FR-37 a/b/c, FR-38)** — closed, 2026-11-05:
delivered at `39afeb2`, named by the follow-up `fb46cc0`; gates 645+6.
FR-39/FR-40 closed at `db68d3f`/`306f425`; the version carried to 0.40.0
(`505097a`, recorded by `856c2bf`).

**"prompt input during a running turn…" (FR-35)** — closed, 2026-11-04
(`c02862d`, v0.38.0, the tag and the release; gates 633+6). **Its
delivered feature is the ask this cycle deleted** (FR-45a, §6's
supersession clause): the removal reinstates the pre-queue contract the
CHANGELOG record and the pins describe, and the feature's history record
stands (the CHANGELOG's `[0.38.0]` section). **FR-32–FR-34** — closed,
2026-11-03 (FR-34 released **v0.37.0**, the release commit `5a2d7f0`;
FR-32 `7fe1370`, FR-33 `af48be5` — the versions 0.35.0/0.36.0 never
tagged; the tag `v0.36.0` on `ddeaf67` stands restored — BL-M32).
**FR-31** — closed, 2026-11-03 (no release). **FR-29/FR-30** — closed,
2026-11-03 (no release). **FR-27/FR-28** — closed, **v0.34.0**
(`c022909`; the tag's final target `745dbb2`). **FR-26** — v0.33.0
(`6cc6661`). **FR-22–FR-24** — v0.32.0 (`e8ad594` + `e0ecb1b`). **the
mouse** — v0.31.0 (`052e230`). **add strata** — v0.30.0–0.30.2
(`b706889`, `70e6b5b`, `a766757`). **v0.29.0** (`045d569`). **v0.28.0**
(`d237a7a`). **v0.27.0** (`afcbe70`). **v0.25.0** (`3884d16`) with the
addenda **v0.26.0** (`251a5bb`).

## 13. Delivery status

| Item | State |
|---|---|
| FR-45 (the prompt-input queueing feature's removal: the deletion, the CHANGELOG entry, the version 0.44.0, the commit/push, the latest release) | **Delivered** (2026-11-06): the queue's helpers, Pending submit arm, `[queued: N]` face, idle-arm queue conjuncts, both park arms, the approval-time draft pair and the six pins deleted per FR-45a (T-102); the CHANGELOG `[0.44.0]` entry written (FR-45b, T-103); the version carried 0.43.0 → **0.44.0** (`Cargo.toml`, `Cargo.lock`) and the delivery commit `b5deb9c` ("Remove the prompt-input queueing feature") pushed to `origin/main` (`d5cbbbe..b5deb9c`); the tag `v0.44.0` verified on `b5deb9c` (`git rev-parse v0.44.0^{commit}`) and the GitHub release `v0.44.0 — the prompt-input queueing feature removed` published as **Latest** (`gh release list`'s Latest label + the release API read, created 2026-11-05T23:18:45Z). Gates at the close: **663 unit + 6 integration, 0 failed** (C-09). |
| FR-44e (the latest release, the previous cycle's) | **Delivered and closed** (verified this cycle): `v0.43.0` stood the latest release at the cycle's ground (`isLatest: true`, created 2026-11-05T10:57:38Z; the tag `v0.43.0` verified on `4fee899`). |
| FR-44 (the codex-code backend: implementation, docs, version, commit, push, the FR-44d amendment) | **Delivered and closed** (2026-11-05; §12): the backend, the docs, the version carries to 0.42.0 and 0.43.0, the commits `122f320`/`4fee899` named by the follow-ups `af82155`/`d5cbbbe`. |
| State at exit | The instruction as rewritten 2026-11-06 02:02:15 +0900 is re-verified byte-exact (349 bytes, sha256 `06620319…`) at the cycle's exit; the cycle is recorded in the three documents (FR-45 a/b/c, BL-M52–BL-M53, TPR-27, D-72–D-74, T-102/T-103/T-104, W-69, AC-38, AS-70–AS-72, C-08/C-09/C-12–C-14) and in the work log `2026-11-06_003.md` (embedding the instruction verbatim). The trail rule stands fulfilled — verified at the 2026-11-06 ledger-consistency pass (no product-tree write): the follow-up `6f0390f` (「Record the queue removal's delivery in the ledger: the trail's follow-up」) names `b5deb9c` in the three ledgers and the work log `2026-11-06_003.md`, and the tip stood pushed to `origin/main` at the pass's start (both at `6f0390f`); the same pass re-verified the instruction byte-exact (349 bytes, sha256 `06620319…`), the release `v0.44.0` standing **Latest** (the releases API re-read, `createdAt` 2026-11-05T23:18:45Z), the tag `v0.44.0` on `b5deb9c`, the version files at 0.44.0, and zero queue-mechanism identifiers left in `src/` (the `stop queued` shutdown-drain line stands the sole remaining queue phrase, the separate mechanism §3.1's scope keeps) — the recorded gates (**663 unit + 6 integration, 0 failed**) stand without a re-run at the pass; the three documents re-aligned there (the design's three `d5cbbde` sha slips corrected to `d5cbbbe`), and the pass's work log `2026-11-06_004.md` written. |
