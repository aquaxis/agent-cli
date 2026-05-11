# Codex (OpenAI) Backend

This backend uses the OpenAI Chat Completions API (streaming, function calling). The name "codex" is the internal `kind` in agent-cli and does not refer to OpenAI's legacy Codex model.

## Prerequisites

- Issue an API key from an OpenAI account
- Set it as an environment variable (default: `OPENAI_API_KEY`)

## Configuration

```toml
[provider]
kind = "codex"

[provider.codex]
api_key_env = "OPENAI_API_KEY"
model       = "gpt-4.1"
base_url    = "https://api.openai.com/v1"
```

## Recommended Models

Choose from OpenAI's official model list; approximate guidance:

| Use case | Example |
|----------|---------|
| Reasoning / code | `gpt-4.1` series |
| General chat | `gpt-4o` series |
| Lightweight | `gpt-4o-mini` series |

Changing `base_url` allows operation with OpenAI-compatible gateways, enterprise proxies, or Azure OpenAI endpoints.

## Supported Features

| Feature | Support | Notes |
|---------|---------|-------|
| Streaming | ✓ | SSE |
| Tool use | ✓ | Normalizes function calling to `ProviderEvent::ToolUse` |
| Thinking | ✗ | Not supported (`Capabilities::thinking=false`) |

## Verification

```bash
export OPENAI_API_KEY="sk-..."
# doctor uses config's provider.kind
agent-cli --config ./codex.toml doctor
# selftest can override with --provider
agent-cli selftest --provider codex
```

## Known Limitations

- Models that do not support function calling may not produce tool invocations. Use `gpt-4.1` / `gpt-4o` series for best results.
- The implementation flushes remaining tool_calls before the `[DONE]` sentinel. Mid-stream disconnections may affect output.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `env var OPENAI_API_KEY not set` | Not set | `export OPENAI_API_KEY=...` |
| `HTTP 401` | Key expired or organization restriction | Try a different API key |
| Incomplete response | Model does not support function calling | Change model |