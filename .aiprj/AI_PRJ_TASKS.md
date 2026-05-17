# Task List (AI_PRJ_TASKS)

Implementation breakdown for the three opt-in context-efficiency features and
the two opencode wire-correctness follow-ups
(`AI_PRJ_REQUIREMENTS.md` / `AI_PRJ_DESIGN.md`).

## Legend

`[ ]` not started · `[~]` in progress · `[x]` done.

---

## M0: Scope

- [x] **T-000** Confirm strategy with user: #3 = Hybrid, rollout = all opt-in
  (default OFF). Update `instructions.md`; rewrite the three docs.

## M1: Config

- [x] **T-010** `src/config.rs`: `ProviderEntry.prompt_cache` +
  `.persistent_session`; `HistoryConfig` + `Config.history`; default template
  (commented opt-ins + `[history]` block). FR-01/02/03, NFR-03 / Design 1.

## M2: Feature #1 — opencode persistent session

- [x] **T-020** `OpenCodeProvider`: `persistent_session` field,
  `PersistState`, `tokio::sync::Mutex`, `from_config` wiring. FR-01 / Design 2.
- [x] **T-021** `complete_stream_local_persistent` + `create_session` +
  `new_turn_text`; dispatch in `complete_stream`; stale-session retry; reset
  rule. FR-01 / Design 2.

## M3: Feature #2 — Claude prompt cache

- [x] **T-030** `ClaudeProvider.prompt_cache` field + `from_config`. FR-02.
- [x] **T-031** `apply_prompt_cache` (system/tools/last-message) + opt-in call
  in `complete_stream`. FR-02 / Design 3.

## M4: Feature #3 — hybrid history management

- [x] **T-040** New `src/history.rs` (`estimate_tokens`, `old_span`,
  `render_transcript`) + `mod history;` in `main.rs`. FR-03 / Design 4.
- [x] **T-041** `Agent::maybe_compact_history` (summarize→splice→drop,
  best-effort, no recursion) + call site in `process_turn`. FR-03 / Design 4.
- [x] **T-042** `old_span` boundary guard so compaction can't orphan an
  `Assistant(tool_calls)`→`ToolResult` pair + unit tests. FR-03 / Design 4.

## M5: Verification (context-efficiency)

- [x] **T-050** Unit tests: history, claude prompt_cache. NFR-02.
- [x] **T-051** `cargo build`/`clippy` clean; `cargo test` **102/102**
  (was 86 pre-project; +16); existing tests unaffected. NFR-02/03.
- [~] **T-052** Live checks (real `opencode serve` persistent session,
  Anthropic cache-hit metrics, long-conversation compaction) — deferred to
  user; no live backends in this environment. FR-01/02/03.

## M6: opencode wire-correctness follow-ups (DeepSeek)

- [x] **T-060** `src/ai/mod.rs`: `ToolCall` struct; `tool_calls` (default,
  skip-if-empty) on `Message::Assistant`; compile-only `{ content, .. }` arms
  for claude/codex/llamacpp/ollama. FR-04 / Design 1a.
- [x] **T-061** `agent.rs`: record the assistant turn with its tool calls
  (incl. the tool-only no-prose case) before `ToolResult`. `opencode.rs`:
  serialize OpenAI `tool_calls` (`arguments` as JSON string), `content:null`
  for tool-only turns + regression tests. FR-04 / Design 4a. Verified live
  vs DeepSeek (HTTP 400 "role 'tool' must follow 'tool_calls'" eradicated).
- [x] **T-062** `src/ai/mod.rs`: `reasoning_content: Option<String>` (default,
  skip-if-none) on `Message::Assistant`. `opencode.rs`:
  `handle_opencode_frame` surfaces `delta.reasoning_content` as a `Thinking`
  event; `to_openai_messages` echoes it back + tests. `agent.rs` accumulates
  reasoning and stores it on the assistant message. FR-05 / Design 4b.
  Verified live vs DeepSeek (thinking-mode `reasoning_content` 400 eradicated).

---

## Notes

- The three context-efficiency features are **opt-in, default OFF**. With
  every flag off the request bodies and history handling are byte-for-byte
  unchanged. FR-04/FR-05 are unconditional protocol fixes but
  wire-shape-preserving (serde `default` + `skip_serializing_if`): no tool
  calls / no reasoning ⇒ key omitted, byte-identical to before.
- Persistent session deliberately sends only new user/tool turns and resets on
  history shrink / system-prompt change; documented dual-state limitation
  mitigated by a one-shot stale-session retry.
- Summarization uses a direct provider call with no tools, so it cannot
  recurse into compaction; any failure degrades to drop-only.
- Token budgeting is a `chars/4` heuristic by design (no tokenizer dep).
- FR-04/05 ship only on the opencode OpenAI path; other providers carry the
  new `Message::Assistant` fields without serializing them (documented latent
  residual, out of scope).

## Work Log

- **2026-05-16 (prior)**: opencode provider implemented + finalized
  (`AI_LOG/2026-05-16_001/002`); README + docs.
- **2026-05-16 (context-efficiency)**: User requested all of #1/#2/#3;
  confirmed Hybrid + all-opt-in. Updated `instructions.md`; implemented config
  (`HistoryConfig`, `prompt_cache`, `persistent_session`), claude prompt cache
  (`apply_prompt_cache`), opencode persistent session
  (`complete_stream_local_persistent`), and hybrid history management
  (`src/history.rs` + `Agent::maybe_compact_history`). `cargo build`/`clippy`
  clean, `cargo test` 93/93 (+7). T-052 live checks deferred (no backends).
  Log: `AI_LOG/2026-05-16_003.md`.
- **2026-05-16 (Docs + README)**: User-requested doc/README update. `README.md`
  (Highlights opt-in bullet; opencode guide link; config example flags),
  `doc/config.md` (opencode provider rows + per-backend defaults; `[history]`
  subsection; new §11 "Context-efficiency features (opt-in)"; structure/TOC),
  `doc/providers/claude.md` (prompt_cache), and new
  `doc/providers/opencode.md`. Docs-only; no source changed (build/test
  unaffected: still 93/93).
- **2026-05-16 (opencode dual cloud API)**: User-requested. Added
  `[provider.opencode] api` selector (`"openai"` default → `/chat/completions`;
  `"anthropic"` → `/messages`) so the OpenCode Zen "go" endpoints work in both
  wire formats. `CloudApi` enum + `cloud_url`; new
  `complete_stream_cloud_anthropic` reusing Claude's `to_anthropic_messages` /
  `handle_frame` / `ClaudeParseState`. Config field + default-template comment
  + docs. Backward-compatible (default openai). `cargo build`/`clippy` clean,
  `cargo test` 95/95 (+2). Committed on `feat/opencode-context-efficiency`.
- **2026-05-16 (Docs pass 3)**: Doc/README update for the opencode `api`
  selector: `README.md`, `doc/architecture.md`, `doc/troubleshooting.md`
  ("Cloud: ModelError / wrong wire format"). Docs-only; build/test unaffected
  (95/95).
- **2026-05-16 (Docs pass 2)**: Brought the two remaining stale docs current:
  `doc/architecture.md` (module map, Provider list, §3.1 compaction step, new
  §8 "Context-efficiency Features") and `doc/troubleshooting.md` (new
  "OpenCode Issues" / "Context-efficiency Features" sections). Docs-only;
  build/test still 93/93.
- **2026-05-17 (DeepSeek tool_calls fix — FR-04, commit `4ef7c7e`)**:
  `Message::Assistant` had no `tool_calls`, so the OpenAI serializer never
  emitted it; a following `role:"tool"` had no qualifying predecessor and
  DeepSeek (via opencode) returned HTTP 400, halting every conductor on its
  first tool call. Added `ToolCall` + `tool_calls` field (back-compat serde);
  `agent.rs` records the assistant turn (incl. tool-only no-prose) before
  `ToolResult`; `opencode.rs` serializes OpenAI `tool_calls`
  (`arguments` as JSON string), `content:null` for tool-only turns; `history.rs`
  old_span boundary guard so compaction can't re-orphan the pair;
  claude/codex/llamacpp/ollama compile-only arms (latent residual documented).
  +regression/unit tests. `cargo build --release` ok, `cargo test` 100/100.
  Verified live vs DeepSeek: original 400 eradicated.
- **2026-05-17 (DeepSeek thinking-mode fix — FR-05)**: DeepSeek thinking mode
  streams `delta.reasoning_content` and **requires it echoed back** on the
  prior assistant turn, else HTTP 400 "The `reasoning_content` in the thinking
  mode must be passed back to the API." Added
  `reasoning_content: Option<String>` on `Message::Assistant` (back-compat
  serde); `opencode.rs` `handle_opencode_frame` surfaces it as a `Thinking`
  event and `to_openai_messages` echoes it back; `agent.rs` accumulates
  reasoning deltas and stores them on the assistant message (the push
  condition now also fires for reasoning-only turns). +unit tests.
  `cargo test` **102/102**. Verified live vs DeepSeek. (Working-tree change on
  `feat/opencode-context-efficiency`; README/architecture/troubleshooting
  doc updates for the `api` selector also pending in the same tree.)
