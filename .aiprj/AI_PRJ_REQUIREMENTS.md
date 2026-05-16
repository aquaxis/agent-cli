# Requirements Specification (AI_PRJ_REQUIREMENTS)

## 1. Project Overview

### 1.1 Project Name

Context-efficiency features for agent-cli (persistent session / prompt cache /
history-window management)

### 1.2 Background

`.aiprj/instructions.md` (2026-05-16) requests three opt-in context-efficiency
features. agent-cli currently replays the full conversation history to every
provider on every send (stateless), which grows cost/latency unbounded and has
no context-window management. The previous project (the `opencode` provider)
is complete; this builds on it.

### 1.3 Scope Decision (confirmed with user, 2026-05-16)

- **#3 history management strategy = Hybrid:** summarize the oldest turns via
  the LLM into one summary message; if still over budget, drop the oldest
  remaining turns.
- **Rollout = all opt-in:** every feature is gated by a config flag, **default
  OFF**. With all flags off, behavior is byte-for-byte unchanged.

### 1.4 Objectives

Implement all three:

1. **opencode local persistent session** — reuse one OpenCode `session_id`
   across turns and send only new user/tool turns; reset on history clear or
   system-prompt change.
2. **Claude prompt caching** — Anthropic `cache_control` breakpoints on the
   system prompt, tools, and the conversation tail.
3. **Hybrid history-window management** — summarize-then-drop when estimated
   context exceeds a configurable budget, always keeping the system/persona
   prefix and the most recent N turns verbatim.

### 1.5 Non-Objectives

- No change to the `Provider` trait signature or `ProviderEvent` variants.
- No behavior change when all flags are off.
- No tokenizer dependency (token estimate is a cheap heuristic).
- No persistence to disk / conversation resume (out of scope here).
- opencode cloud mode and other providers' session handling unchanged.

## 2. Terminology

| Term | Definition |
|------|-----------|
| Persistent session | Reusing a server-side OpenCode `session_id` across turns |
| Prompt cache | Anthropic `cache_control: {type:"ephemeral"}` breakpoints |
| Compaction | Summarize-then-drop of the old history span |
| Old span | History after the system prefix and before the last N turns |
| Estimated tokens | `sum(content chars) / 4` heuristic |

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
- Best-effort: a summarization-call failure degrades to drop-only and must
  never fail the user's turn. An `Info` event reports what was compacted.
- Disabled: full history replayed verbatim, unchanged.

## 4. Non-Functional Requirements

- **NFR-01** Docs in English; Requirements/Design/Tasks mutually consistent.
- **NFR-02** `cargo build` / `cargo test` / `cargo clippy` clean; new logic
  unit-tested.
- **NFR-03** Additive, non-breaking config (`#[serde(default)]`); all existing
  tests still pass; default behavior unchanged.
- **NFR-04** Summarization must not recurse into compaction.

## 5. Constraints

- Source changes limited to: `src/config.rs`, `src/ai/claude.rs`,
  `src/ai/opencode.rs`, new `src/history.rs`, `src/agent.rs`, `src/main.rs`
  (module decl), plus the embedded default config template.

## 6. Acceptance Criteria

- Three config flags exist, default OFF; all-off ⇒ unchanged behavior.
- FR-01..FR-03 implemented with unit tests for the pure logic.
- `cargo build`/`test`/`clippy` green; existing tests unaffected.
- Requirements/Design/Tasks consistent.
</content>
