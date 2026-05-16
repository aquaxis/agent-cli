# Claude Backend

This backend uses the Anthropic Claude API (Messages, SSE). It is the reference implementation for `agent-cli`, supporting thinking, tool_use, and streaming.

## Prerequisites

- Issue an API key from the Anthropic console
- Set the API key as an environment variable (default: `ANTHROPIC_API_KEY`)

## Configuration

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env  = "ANTHROPIC_API_KEY"
model        = "claude-opus-4-7"
base_url     = "https://api.anthropic.com"   # usually leave as-is
thinking     = true                           # enable thinking blocks
prompt_cache = false                          # opt-in; see "Prompt Caching"
```

## Prompt Caching (opt-in)

Set `prompt_cache = true` to add Anthropic `cache_control: {type:"ephemeral"}`
breakpoints to the system prompt, the last tool definition, and the last
message's last content block (≤ 3 of Anthropic's 4 allowed). The repeated
prefix is then served from Anthropic's prompt cache (≈ 5-minute TTL) instead of
being reprocessed each turn — the full history is still sent, just cheaper and
faster on cache hits. Default is `false` (request body identical to before).
See [`doc/config.md`](../config.md) §11.1.

## Recommended Models

| Use case | Model |
|----------|-------|
| Reasoning-focused / code generation | `claude-opus-4-7` |
| Balanced | `claude-sonnet-4-6` |
| Lightweight / fast | `claude-haiku-4-5-20251001` |

Check the Anthropic console for currently available models.

## Supported Features

| Feature | Support | Notes |
|---------|---------|-------|
| Streaming | ✓ | SSE |
| Tool use | ✓ | Parses native Anthropic tool_use blocks |
| Thinking | ✓ | Displayed with `[thinking]` header. Controlled via `ui.show_thinking` |
| Prompt caching | ✓ (opt-in) | `prompt_cache = true`; `cache_control` on system / tools / tail |

## Verification

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
# doctor uses config's provider.kind; to check claude,
# set kind = "claude" in the config file or pass a dedicated config
agent-cli doctor                 # when config file has claude
agent-cli --config ./claude.toml doctor

# selftest can override with --provider
agent-cli selftest --provider claude
```

`doctor` provides a pass/fail summary; `selftest` runs 5 stages (Provider / shell tool / IPC / subprocess startup / subprocess AI response). Both exit code 0 means the backend is healthy.

## Proxy / Compatible Server

To use a corporate proxy or Anthropic-compatible gateway, override `base_url`:

```toml
[provider.claude]
base_url = "https://proxy.example.com/anthropic"
```

## Known Limitations

- Long responses with many tool_use calls may exceed `reqwest`'s timeout (120 seconds). For long-running operations, manage timeouts on the tool side.
- `thinking_delta` output is rendered line-by-line, which may affect readability depending on terminal width. The default `[ui] show_thinking = "collapsed"` (first 80 chars + first line) is generally the best choice. Use `"hidden"` to suppress entirely, or `"expanded"` for debugging. See [`doc/config.md`](../config.md) "UI display modes" for details.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `env var ANTHROPIC_API_KEY not set` | Environment variable not set | `export ANTHROPIC_API_KEY=...` |
| `HTTP 401` | Key expired or incorrect | Re-issue from the official console |
| `HTTP 429` | Rate limiting | Reduce request pace |
| Empty response | Model set to thinking-only and producing only thinking output | Retry with `thinking=false` |