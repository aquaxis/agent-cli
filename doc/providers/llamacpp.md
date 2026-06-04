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

## Sampling Parameters

The generation knobs you would pass on the `llama-cli` / `llama-server` command
line can be set in config; agent-cli forwards each one into the
`/v1/chat/completions` request body. Every field is **optional** — omit it and
the llama.cpp server applies its own default, so a config without any of these
behaves exactly as before.

```toml
[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"
max_tokens     = 1024   # -n / --n-predict : max tokens to generate
temperature    = 0.2    # --temp
top_k          = 80     # --top-k
top_p          = 0.95   # --top-p
min_p          = 0.05   # --min-p
repeat_penalty = 1.05   # --repeat-penalty
repeat_last_n  = 64     # --repeat-last-n
seed           = 0      # --seed (reproducibility)
```

| Config key | `llama-cli` flag | Server field | Type |
|------------|------------------|--------------|------|
| `max_tokens` | `-n` / `--n-predict` | `max_tokens` | integer |
| `temperature` | `--temp` | `temperature` | float |
| `top_k` | `--top-k` | `top_k` | integer |
| `top_p` | `--top-p` | `top_p` | float |
| `min_p` | `--min-p` | `min_p` | float |
| `repeat_penalty` | `--repeat-penalty` | `repeat_penalty` | float |
| `repeat_last_n` | `--repeat-last-n` | `repeat_last_n` | integer |
| `seed` | `--seed` | `seed` | integer |

`top_k`, `min_p`, `repeat_penalty`, `repeat_last_n`, and `seed` are llama.cpp
extensions to the OpenAI schema; the llama.cpp server honors them on
`/v1/chat/completions`. (These options apply to the `llama.cpp` backend only.)

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