# Ollama Backend

This backend uses a local or cloud [Ollama](https://ollama.com) server. It is one of agent-cli's **mandatory verification targets** (alongside `claude`).

## Prerequisites

- Install Ollama (see the official README)
- For local use, start the server with `ollama serve`
- For cloud models, ensure the corresponding backend is running

## Configuration

```toml
[provider]
kind = "ollama"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"
```

No API key is required (setting `api_key_env` has no effect).

## Recommended Models

The mandatory verification target is **`glm-5.1:cloud`**. Any model visible in `ollama list` can be specified using the same format.

```bash
ollama pull glm-5.1:cloud      # cloud version
ollama pull llama3.1:8b        # local model example
```

## Supported Features

| Feature | Support | Notes |
|---------|---------|-------|
| Streaming | ✓ | NDJSON |
| Tool use | ✓ (model-dependent) | Only works with `tools`-capable models |
| Thinking | ✓ (model-dependent) | Renders NDJSON `message.thinking` as `[thinking]`. Works with `glm-5.1:cloud` etc. |

`Capabilities` statically returns `tool_use=true` / `thinking=true`, but if the server or model does not support these features, tool calls or thinking output may not be produced (or may be silently ignored).

### Controlling Thinking Display

When a thinking-capable model like `glm-5.1:cloud` returns the `message.thinking` field, agent-cli renders it as `ProviderEvent::Thinking` with a `[thinking]` prefix in the REPL (emission order: `Thinking` → `Text` → `ToolUse`, consistent with Anthropic convention). Display mode is controlled via `[ui] show_thinking`:

| Setting | Behavior |
|---------|----------|
| `"collapsed"` (default) | Truncate each thinking delta to "first 80 chars + `...`" on one line |
| `"expanded"` | Print full thinking text as it arrives |
| `"hidden"` | Suppress thinking output entirely |

If thinking output is noisy, set `[ui] show_thinking = "hidden"`. See [`doc/config.md`](../config.md) "UI display modes" for details.

## Verification

```bash
ollama serve &
# doctor uses config's provider.kind
agent-cli --config ./ollama.toml doctor
# selftest can override with --provider
agent-cli selftest --provider ollama
```

If `doctor`'s `provider conn` step shows `OK (stream initiated)`, connectivity is healthy. Cloud-routed models (`*:cloud` tags) may have cold-start delays; the connectivity timeout is set to 60 seconds.

## Proxy / Remote Host

If Ollama runs on a different host:

```toml
[provider.ollama]
base_url = "http://gpu-server.local:11434"
```

## Known Limitations

- `tool_calls` JSON formats may vary between models. If errors occur, retry without tools.
- Large models may exceed the 180-second timeout. For long-generation scenarios, also review `[tools.shell] timeout_secs`.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `connection refused` | Server not running | `ollama serve` |
| `model 'X' not found` | Model not pulled | `ollama pull X` |
| Tools not called | Model does not support tools | Switch to a tools-capable model |
| Slow response | Model is large / no GPU | Use a lighter model or switch to a GPU environment |