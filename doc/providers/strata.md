# Strata Backend

Strata is a free, open-source **local inference engine** that runs
[Qwen3.8-Flash-Next](https://huggingface.co/Qwen/Qwen3.8-Flash-Next) (a 125-billion-parameter mixture-of-experts model) on a single gaming PC, and serves it behind an **OpenAI-compatible API** — no account, no cloud, no telemetry. In agent-cli it connects through the same Chat Completions wire as the `codex` backend, with **no API key**.

- Repository: <https://github.com/Niko1221/Strata> (MIT license)
- Requirements (on Strata's side): an NVIDIA RTX 20/30/40/50-series or AMD RX 6800-or-newer GPU with 12 GB+ VRAM, 32 GB+ RAM (~80 GB disk for the model)

## Prerequisites

- Install and start Strata on the machine agent-cli runs on (Strata's `START-HERE.bat` / `./setup.sh`; its web UI opens at `http://127.0.0.1:8080`).
- **The engine must be running before the agent-cli session starts.** A server that is down surfaces as a connection error on the first request — there is no pre-flight ping at startup.

## Configuration (keyless, the default shape)

```toml
[provider]
kind = "strata"
```

That is the whole configuration. `kind = "strata"` is a convenience alias of the codex provider with Strata's defaults filled in:

| Key | Default | Notes |
|-----|---------|-------|
| `base_url` | `http://127.0.0.1:8080/v1` | Strata's documented OpenAI-compatible base URL |
| `model` | `strata` | The engine serves **any model name** — the id is name-agnostic; the quant/family (`--model iq2_xs`, `--family coder`) is chosen at engine launch, not per request |
| `api_key_env` | *(none)* | **No `Authorization` header is sent at all.** Strata's loopback endpoint accepts any key, so none is configured |

The alias keeps the codex wire and diagnostics, and **agent-cli keeps calling the kind `strata` everywhere it is displayed** — the startup banner's `provider` row, the registry and `/list`, selftest and the `providers` listing, and the `provider error (strata)` prefixes. Internally the codex provider serves the `[provider.strata]` slot (defaults and overrides as above), and a `[provider.codex]` table in the file is never consulted.

## Explicit table / overrides

A `[provider.strata]` table overrides field by field; the keyless defaults fill only the fields it leaves unset (the same shape `opencode-go` uses against `[provider.opencode]`):

```toml
[provider]
kind = "strata"

[provider.strata]
model    = "strata"          # any name works; `strata` follows the docs
base_url = "http://127.0.0.1:8080/v1"
# request_timeout_secs = 1800 # a slow long-prompt prefill may want a bigger bound
```

## Keyed variant (public-host exposure)

Strata binds to loopback with no key by default. When you expose it beyond the local machine (`START-HERE.bat --setup --host 0.0.0.0 --api-key ...`, or `--access lan|cloudflare|tailscale`, where a key is recommended), set the env var yourself and the keyed path serves you:

```toml
[provider]
kind = "strata"

[provider.strata]
api_key_env = "STRATA_API_KEY"
```

The configured-unset variable keeps the usual codex-provider construction error (`env var STRATA_API_KEY not set`) — a typo must not become a silent unauthenticated request.

## Keyless codex in general (FR-15)

The keyless form is a property of the codex provider now, not of the alias: `[provider.codex]` **without `api_key_env`** connects keyless the same way — useful for any OpenAI-compatible server that requires no authentication. A codex config that names an `api_key_env` behaves exactly as before.

## Supported Features

| Feature | Support | Notes |
|---------|---------|-------|
| Streaming | ✓ | SSE with `[DONE]` |
| Tool use | ✓ | OpenAI function calling served by the engine |
| Thinking | ✗ | `Capabilities::thinking = false` (the codex wire does not carry it) |
| Images / vision | ✗ | Not part of the Chat Completions wire agent-cli sends |

## Verification

```bash
agent-cli doctor                      # provider kind / server reachability
agent-cli selftest --provider strata  # 5-stage smoke test against the engine
```

`selftest` stage 1 exercises the provider round-trip; stages 2–5 need the engine running for stage 1 only.

## Known Limitations

- One request at a time per Strata engine default; agent-cli's serial turn model matches that. (Strata's `"parallel": 2` exists; agent-cli does not need it.)
- Long prompts are read slowly by the engine itself (~1 minute per 30 000 tokens for the first message of a chat); `request_timeout_secs` (default 900 s) bounds the whole exchange.
- The OpenAI **Responses API** (`/v1/responses`) and the Anthropic-compatible endpoint (`/v1/messages`) that Strata also serves are not used by agent-cli; the Chat Completions wire covers the connection.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `error sending request ... connection refused` at the first turn | The Strata engine is not running | Start it (its web UI must answer at `http://127.0.0.1:8080` first) |
| `env var <NAME> not set` at startup | `[provider.strata]` names an `api_key_env` whose variable is unset | Export the variable, or remove `api_key_env` for the keyless default |
| `HTTP 404` on `/v1/chat/completions` | `base_url` missing the `/v1` suffix, or an old engine build | Use `http://127.0.0.1:8080/v1`; update the engine |
| Tool calls never fire | The loaded model builds without tool calls | Reload with a full-size or Coder variant (tool use served on all sizes, but a too-small quant may weaken it) |