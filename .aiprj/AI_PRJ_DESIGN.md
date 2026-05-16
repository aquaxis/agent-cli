# Design Document (AI_PRJ_DESIGN)

As-built design for the three opt-in context-efficiency features in
`AI_PRJ_REQUIREMENTS.md`.

## 1. Config (`src/config.rs`)

- `ProviderEntry` += `prompt_cache: Option<bool>`, `persistent_session:
  Option<bool>` (both `#[serde(default)]`, `unwrap_or(false)` at use).
- New `HistoryConfig` (`enabled` default false, `max_context_tokens` default
  24000, `keep_recent_turns` default 6) added to `Config` as
  `#[serde(default)] history`.
- `DEFAULT_CONFIG` template: commented opt-in lines under `[provider.claude]`
  and `[provider.opencode]`, and a `[history]` block (enabled=false).
- Additive only — every existing config still parses; defaults preserve current
  behavior.

## 2. Feature #1 — opencode Persistent Session (`src/ai/opencode.rs`)

- `OpenCodeProvider` += `persistent_session: bool` and
  `session: tokio::sync::Mutex<PersistState>` where
  `PersistState { id: Option<String>, sent_count: usize, sys: String }`.
  `tokio` mutex because the guard is held across `.await`; turns are
  sequential so serialization is acceptable.
- `complete_stream` dispatch: cloud (key) → unchanged; local + flag →
  `complete_stream_local_persistent`; local + no flag → existing
  `complete_stream_local` (ephemeral, unchanged).
- `complete_stream_local_persistent`:
  - compute `system` via `flatten_history`.
  - reset if `id.is_none()` || `messages.len() < sent_count` ||
    `sys != system` → `create_session` (`POST /session`), `sent_count = 0`.
  - `new_turn_text(messages, sent_count)` builds the payload from new
    `User`/`ToolResult` turns only (`Assistant` skipped — server keeps its
    own; `System` via the message `system` field on the first message).
    Falls back to the latest `User` if nothing new qualifies.
  - `POST /session/:id/message`; on 404/400/410 → one recreate + resend; on
    other non-2xx → standard `ProviderError`.
  - on success `sent_count = messages.len()`; parse via `parse_local_parts`.
- The server retains prior context, so full history is NOT re-flattened.

## 3. Feature #2 — Claude Prompt Cache (`src/ai/claude.rs`)

- `ClaudeProvider` += `prompt_cache: bool` (`entry.prompt_cache`).
- After building the request body, `if self.prompt_cache {
  apply_prompt_cache(&mut body) }`.
- `apply_prompt_cache` (pure, `&mut serde_json::Value`):
  - `system` string → `[{type:text,text,cache_control:ephemeral}]`.
  - `tools` → `cache_control` on the last tool object.
  - `messages` last message: string content → text block with
    `cache_control`; array content (tool_result) → `cache_control` on its
    last block.
- Disabled path untouched (plain strings, byte-identical to before).

## 4. Feature #3 — Hybrid History Management (`src/history.rs` + `src/agent.rs`)

- New pure module `src/history.rs` (declared in `main.rs`):
  - `estimate_tokens(&[Message]) -> usize` = Σ content chars / 4.
  - `old_span(&[Message], keep) -> Option<Range>` = after leading `System`
    prefix, before last `keep` messages.
  - `render_transcript(&[Message]) -> String` for the summarization prompt.
- `Agent::process_turn` calls `maybe_compact_history(event_tx)` before the
  tool-iteration loop, only when `config.history.enabled`.
- `maybe_compact_history` (`&mut self`):
  1. return early if `estimate_tokens <= max_context_tokens` or no old span.
  2. clone old span → transcript; call `self.provider.complete_stream(&[
     System(summarizer), User(transcript) ], &[])` with **no tools**
     (cannot recurse — direct provider call, not `process_turn`); collect
     `Text` until `Done`/`Error`.
  3. if summary non-empty → `history.splice(span, [System(summary)])` and
     emit an `Info` event.
  4. while still over budget → `history.remove(old_span.start)` (drop oldest
     eligible) and emit an `Info` event.
  - Borrow safety: the provider call only borrows `&self`; the stream is
    dropped before any `&mut self.history` mutation (sequential, NLL-clean).
  - Best-effort: `complete_stream` error ⇒ skip summary, fall through to
    drop-only; the turn never fails.

## 5. Source Files Touched

| File | Change |
|------|--------|
| `src/config.rs` | `prompt_cache`/`persistent_session` fields; `HistoryConfig`; template |
| `src/ai/claude.rs` | `prompt_cache` field; `apply_prompt_cache` + 3 tests |
| `src/ai/opencode.rs` | `persistent_session`, `PersistState`, persistent path, `new_turn_text` |
| `src/history.rs` | **new** pure module + 4 tests |
| `src/agent.rs` | `maybe_compact_history` + call site in `process_turn` |
| `src/main.rs` | `mod history;` |

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| Persistent-session dual-state drift (agent-cli history vs server session) | Send only new user/tool turns; skip assistant; reset on shrink/system change; one stale-session retry |
| Summarization recursion / failure | No tools + direct provider call; failure degrades to drop-only; never fails turn |
| Token estimate inaccuracy | Heuristic only gates compaction; conservative default budget; recent turns always kept |
| Prompt-cache breakpoint overuse | ≤ 3 of Anthropic's 4 allowed; only when opted in |

## 7. Handoff

Implementation complete and verified (`cargo build`/`test`/`clippy` green;
+7 unit tests). Live checks (real OpenCode server with `persistent_session`;
real Anthropic cache-hit metrics; long-conversation compaction) require live
backends and are user-side.
</content>
