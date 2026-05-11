# llama.cpp Backend

This backend uses the OpenAI-compatible API (`/v1/chat/completions`) provided by a [llama.cpp](https://github.com/ggerganov/llama.cpp) server.

## Prerequisites

- Build and start a llama.cpp server:
  ```bash
  ./llama-server --port 8080 -m /path/to/model.gguf --jinja
  ```
- OpenAI-compatible options (`--jinja` etc., build flags for tool support) must be enabled

## Configuration

In the config file, quote `"llama.cpp"` because the TOML key contains a dot (`.`).

```toml
[provider]
kind = "llama.cpp"

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"
api_key_env = "LLAMACPP_API_KEY"   # optional; only for Bearer-auth builds
```

## Supported Features

| Feature | Support | Notes |
|---------|---------|-------|
| Streaming | ✓ | SSE (OpenAI-compatible) |
| Tool use | △ | Depends on server build / model. Disabled if not working |
| Thinking | ✗ | Not supported |

## Verification

```bash
./llama-server --port 8080 -m model.gguf &
# doctor uses config's provider.kind
agent-cli --config ./llamacpp.toml doctor
# selftest can override with --provider
agent-cli selftest --provider llama.cpp
```

## Recommended Models

Any OpenAI-compatible chat model that runs on llama.cpp (e.g. `llama3`, `qwen2.5`, `gpt-oss`). For tool calling, choose a model with an included Jinja template.

## Known Limitations

- Even with the same OpenAI-compatible API, different servers may have slight differences in `tool_calls` format or role representation. If tools don't work, try setting `[tools] enabled` to empty and verify text-only responses first.
- Tool calling requires `--jinja` at server startup to work correctly.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `connection refused` | Server not running | `./llama-server ...` |
| Response is empty string only | Template not supported | Build / start with `--jinja` enabled |
| Tools not called | Model does not support function calling | Use a different model, or operate without tools |