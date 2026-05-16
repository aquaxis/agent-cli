# OpenCode Backend

This backend talks to [OpenCode](https://opencode.ai) in one of two modes,
selected automatically by **API-key presence**:

- **Local mode** (no resolved API key) — a running `opencode serve` reached
  over its native session API: `POST /session` → `POST /session/:id/message`
  (synchronous JSON). Default `base_url` `http://127.0.0.1:4096`.
- **Cloud mode** (API key resolved) — OpenCode Zen, an OpenAI-compatible
  endpoint `POST {base_url}/chat/completions` (SSE, `[DONE]`,
  `Authorization: Bearer`). Default cloud `base_url` `https://opencode.ai/zen/v1`.

## Prerequisites

- **Local:** install OpenCode and run `opencode serve` (listens on
  `http://127.0.0.1:4096` by default). No API key needed.
- **Cloud:** an OpenCode Zen account; copy your API key and set it as an
  environment variable (conventionally `OPENCODE_API_KEY`).

## Configuration

Local mode:

```toml
[provider]
kind = "opencode"

[provider.opencode]
base_url = "http://127.0.0.1:4096"
model    = "claude-sonnet-4-5"        # a model your opencode instance serves
# persistent_session = true           # opt-in; see below
```

Cloud mode (OpenCode Zen) — set `api_key_env`; presence switches to cloud:

```toml
[provider.opencode]
base_url    = "https://opencode.ai/zen/v1"
api_key_env = "OPENCODE_API_KEY"
model       = "claude-sonnet-4-5"
```

## Cloud Wire Format — `api` (OpenAI vs Anthropic compatible)

OpenCode Zen exposes both an OpenAI-compatible and an Anthropic-compatible
surface (including the "go" endpoints). Select which one agent-cli uses with
`api` (cloud mode only; default `"openai"`):

| `api` | Endpoint | Parser |
|-------|----------|--------|
| `"openai"` (default) | `{base_url}/chat/completions` | OpenAI SSE (`[DONE]`) |
| `"anthropic"` | `{base_url}/messages` | Anthropic SSE (reuses the Claude parser) |

Pair it with the matching `base_url`. Example — Anthropic-compatible "go"
endpoint:

```toml
[provider.opencode]
base_url    = "https://opencode.ai/zen/go/v1"   # → POST .../zen/go/v1/messages
api         = "anthropic"
api_key_env = "OPENCODE_API_KEY"
model       = "claude-sonnet-4-5"               # a model the endpoint serves
```

OpenAI-compatible "go" endpoint: same but `api = "openai"` (or omit) →
`POST https://opencode.ai/zen/go/v1/chat/completions`. The API key is sent as
`Authorization: Bearer` (and, for the Anthropic path, also `x-api-key` +
`anthropic-version`) for gateway compatibility. Always use a model id the
endpoint actually serves (check `{base_url}/models`); an unknown model returns
a `ModelError`.

## Supported Features

| Feature | Support | Notes |
|---------|---------|-------|
| Streaming | ✓ | Cloud: SSE. Local: synchronous reply surfaced as an event stream |
| Tool use | ✓ cloud / ✗ local | Cloud forwards tool specs (OpenAI format). Local v1 does not forward tool specs (OpenCode session-API tool schema unconfirmed) |
| Thinking | ✗ | Neither mode streams a separate reasoning channel |
| Persistent session | ✓ (opt-in, local only) | `persistent_session = true`; see below |

## Persistent Session (opt-in, local mode only)

By default local mode creates an ephemeral OpenCode session **per turn** and
re-flattens the whole conversation into it. With:

```toml
[provider.opencode]
persistent_session = true
```

agent-cli instead creates one session, caches its `session_id`, and on each
turn sends only the **new** user/tool turns (the server retains prior context).
The session is recreated when the conversation history is cleared (`/clear`) or
the system prompt changes; a stale-session server error (404/400/410) triggers
one transparent recreate + resend. Ignored in cloud mode. See
[`doc/config.md`](../config.md) §11.2.

## Verification

```bash
# Local: start the server first
opencode serve &
agent-cli --config ./opencode.toml doctor
agent-cli selftest --provider opencode
```

`doctor` gives a pass/fail summary; `selftest` runs 5 stages. Exit code 0
means the backend is healthy.

## Known Limitations

- **Local tool use:** agent-cli tool specs are not forwarded to a local
  `opencode serve` in v1 (the session-API tool schema is unconfirmed). Use
  cloud mode if you need the model to call agent-cli's tools.
- **Persistent session dual-state:** with `persistent_session = true`, both
  agent-cli (history) and the OpenCode server (session) hold conversation
  state. Resets on `/clear` / system-prompt change keep them aligned; the
  one-shot stale-session retry covers server restarts.
- Conversation continuity in non-persistent local / cloud mode relies on
  agent-cli replaying full history each turn (the APIs are stateless).

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| Connection refused (local) | `opencode serve` not running | Start the server; verify `base_url` / port |
| `HTTP 401` (cloud) | Key missing/expired | Set/refresh the `api_key_env` variable |
| Unexpectedly using cloud | `api_key_env` resolves to a value | Unset it (or omit) for local mode |
| Model not found | `model` not served by your endpoint | Set `model` to one the local server / Zen serves |
