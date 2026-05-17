# Requirements Specification (AI_PRJ_REQUIREMENTS)

## 1. Project Overview

### 1.1 Project Name

Context-efficiency features for agent-cli (persistent session / prompt cache /
history-window management), plus the opencode wire-correctness follow-ups
required to make the cloud path usable with DeepSeek.

### 1.2 Background

`.aiprj/instructions.md` (2026-05-16) requests three opt-in context-efficiency
features. agent-cli currently replays the full conversation history to every
provider on every send (stateless), which grows cost/latency unbounded and has
no context-window management. The previous project (the `opencode` provider)
is complete; this builds on it.

Live use of the opencode cloud path against DeepSeek then surfaced two
protocol bugs that made the feature set unusable in practice (every
tool-calling turn HTTP 400'd). They are in-scope follow-ups: the
context-efficiency work is moot if the provider cannot complete a turn.

### 1.3 Scope Decision (confirmed with user, 2026-05-16)

- **#3 history management strategy = Hybrid:** summarize the oldest turns via
  the LLM into one summary message; if still over budget, drop the oldest
  remaining turns.
- **Rollout = all opt-in:** every context-efficiency feature is gated by a
  config flag, **default OFF**. With all flags off, behavior is byte-for-byte
  unchanged.
- **Wire-correctness follow-ups (FR-04/FR-05) are NOT opt-in:** they are
  unconditional protocol fixes, made additive (serde `default` +
  `skip_serializing_if`) so the wire shape is unchanged when there is no
  tool call / reasoning content.

### 1.4 Objectives

Implement the three context-efficiency features:

1. **opencode local persistent session** — reuse one OpenCode `session_id`
   across turns and send only new user/tool turns; reset on history clear or
   system-prompt change.
2. **Claude prompt caching** — Anthropic `cache_control` breakpoints on the
   system prompt, tools, and the conversation tail.
3. **Hybrid history-window management** — summarize-then-drop when estimated
   context exceeds a configurable budget, always keeping the system/persona
   prefix and the most recent N turns verbatim.

Plus the opencode cloud (OpenAI-compatible) wire-correctness follow-ups:

4. **Assistant `tool_calls` echo** — record and re-emit the assistant turn's
   tool calls so a following `role:"tool"` message has a qualifying
   predecessor.
5. **Assistant `reasoning_content` echo** — capture DeepSeek thinking-mode
   chain-of-thought and echo it back on the prior assistant turn.

### 1.5 Non-Objectives

- No change to the `Provider` trait signature or `ProviderEvent` variants
  (FR-05 reuses the existing `Thinking` event).
- No behavior change when all context-efficiency flags are off; the FR-04/05
  fixes are wire-shape-preserving when there are no tool calls / reasoning.
- No tokenizer dependency (token estimate is a cheap heuristic).
- No persistence to disk / conversation resume (out of scope here).
- opencode cloud Anthropic-API path and other providers' session handling
  unchanged; FR-04/05 are implemented on the shipped opencode OpenAI path
  (claude/codex/llamacpp/ollama get the new `Message` field only as a
  compile-only match arm — documented latent residual).

## 2. Terminology

| Term | Definition |
|------|-----------|
| Persistent session | Reusing a server-side OpenCode `session_id` across turns |
| Prompt cache | Anthropic `cache_control: {type:"ephemeral"}` breakpoints |
| Compaction | Summarize-then-drop of the old history span |
| Old span | History after the system prefix and before the last N turns |
| Estimated tokens | `sum(content chars) / 4` heuristic |
| Tool-only turn | An assistant turn with tool calls but no prose (`content` empty) |

## 3. Functional Requirements

### 3.1 opencode Persistent Session (FR-01)

- Config `[provider.opencode] persistent_session` (bool, default false).
  Effective only in local mode (no API key); ignored in cloud mode.
- When enabled: create the session once, cache `session_id`, and on each turn
  send only the new `User`/`ToolResult` turns (skip `Assistant`; `System` via
  the session). `sent_count` tracks delivered history length.
- Reset (recreate session, resend from scratch) when: no session yet, history
  shrank (e.g. `/clear`), or the system prompt changed.
- A stale-session HTTP response (404/400/410) triggers exactly one transparent
  session recreate + resend.
- Disabled: existing ephemeral-session-per-turn behavior, unchanged.

### 3.2 Claude Prompt Caching (FR-02)

- Config `[provider.claude] prompt_cache` (bool, default false).
- When enabled, add `cache_control` to: the system prompt (as a text block),
  the last tool definition, and the last message's last content block
  (≤ 3 of Anthropic's 4 allowed breakpoints).
- String content is converted to block form only when caching; tool_result
  array content gets the marker on its last block.
- Disabled: request body identical to today (plain strings).

### 3.3 Hybrid History-Window Management (FR-03)

- Config `[history]`: `enabled` (bool, default false),
  `max_context_tokens` (default 24000), `keep_recent_turns` (default 6).
- Before each provider call, if enabled and estimated tokens exceed
  `max_context_tokens`: summarize the old span via the LLM into one
  `System` summary message replacing that span; if still over budget, drop the
  oldest old-span messages one at a time.
- The leading system/persona prefix and the last `keep_recent_turns` messages
  are never summarized or dropped.
- **old_span boundary guard:** the span boundary must not split an
  `Assistant(tool_calls)` → `ToolResult` pair; if the recent-turns cut would
  orphan a `ToolResult`, the boundary moves back to keep the pair intact (so
  compaction can never produce an unanswerable `role:"tool"` on the wire).
- Best-effort: a summarization-call failure degrades to drop-only and must
  never fail the user's turn. An `Info` event reports what was compacted.
- Disabled: full history replayed verbatim, unchanged.

### 3.4 Assistant `tool_calls` Echo (FR-04, unconditional)

- `Message::Assistant` carries `tool_calls: Vec<ToolCall>`
  (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`).
- The agent records the assistant turn **with its tool calls** before pushing
  the `ToolResult`(s), including the previously-dropped tool-only (no-prose)
  case.
- The opencode OpenAI serializer emits `tool_calls[]` (`id`/`type`/`function`,
  `arguments` as a JSON string) and `content: null` for a tool-only turn, so a
  following `role:"tool"` message always has a qualifying `tool_calls`
  predecessor. Eliminates the DeepSeek HTTP 400 "Messages with role 'tool'
  must be a response to a preceding message with 'tool_calls'".
- Wire-shape-preserving: no `tool_calls` ⇒ key omitted, byte-identical to
  before.

### 3.5 Assistant `reasoning_content` Echo (FR-05, unconditional)

- `Message::Assistant` carries `reasoning_content: Option<String>`
  (`#[serde(default, skip_serializing_if = "Option::is_none")]`).
- opencode SSE handling surfaces `delta.reasoning_content` as the existing
  `ProviderEvent::Thinking`; the agent accumulates it and stores it on the
  assistant message, and the opencode OpenAI serializer echoes it back on that
  turn. Eliminates the DeepSeek HTTP 400 "The `reasoning_content` in the
  thinking mode must be passed back to the API."
- Wire-shape-preserving: `None` ⇒ key omitted, byte-identical to before.

## 4. Non-Functional Requirements

- **NFR-01** Docs in English; Requirements/Design/Tasks mutually consistent.
- **NFR-02** `cargo build` / `cargo test` / `cargo clippy` clean; new logic
  unit-tested.
- **NFR-03** Additive, non-breaking config and message shape
  (`#[serde(default)]` / `skip_serializing_if`); all existing tests still
  pass; default behavior unchanged.
- **NFR-04** Summarization must not recurse into compaction.
- **NFR-05** FR-04/FR-05 are protocol fixes verified live against DeepSeek
  (via opencode), not only by unit tests.

## 5. Constraints

- Context-efficiency source changes: `src/config.rs`, `src/ai/claude.rs`,
  `src/ai/opencode.rs`, new `src/history.rs`, `src/agent.rs`, `src/main.rs`
  (module decl), plus the embedded default config template.
- FR-04/FR-05 additionally touch `src/ai/mod.rs` (the `ToolCall` struct and
  the two new `Message::Assistant` fields) and add compile-only match arms in
  `src/ai/claude.rs`, `src/ai/codex.rs`, `src/ai/llamacpp.rs`,
  `src/ai/ollama.rs` (those providers' wire output unchanged).

## 6. Acceptance Criteria

- Three config flags exist, default OFF; all-off ⇒ context-efficiency
  behavior unchanged.
- FR-01..FR-05 implemented with unit tests for the pure logic; FR-04/05
  additionally verified live against DeepSeek.
- `cargo build`/`test`/`clippy` green; existing tests unaffected;
  `cargo test` **102/102**.
- Requirements/Design/Tasks consistent.
