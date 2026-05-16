# Task List (AI_PRJ_TASKS)

Implementation breakdown for the three opt-in context-efficiency features
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

## M5: Verification

- [x] **T-050** Unit tests: history (4), claude prompt_cache (3). NFR-02.
- [x] **T-051** `cargo build` clean; `cargo test` **93/93** (was 86; +7);
  `cargo clippy --all-targets` clean; existing tests unaffected. NFR-02/03.
- [~] **T-052** Live checks (real `opencode serve` persistent session,
  Anthropic cache-hit metrics, long-conversation compaction) — deferred to
  user; no live backends in this environment. FR-01/02/03.

---

## Notes

- All three features are **opt-in, default OFF**. With every flag off the
  request bodies and history handling are byte-for-byte unchanged (verified:
  the 86 pre-existing tests still pass untouched).
- Persistent session deliberately sends only new user/tool turns and resets on
  history shrink / system-prompt change; documented dual-state limitation
  mitigated by a one-shot stale-session retry.
- Summarization uses a direct provider call with no tools, so it cannot
  recurse into compaction; any failure degrades to drop-only.
- Token budgeting is a `chars/4` heuristic by design (no tokenizer dep).

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
  unaffected: still 93/93). Note: `doc/config.md` was also previously missing
  the `opencode` provider — brought current in the same pass.
</content>
