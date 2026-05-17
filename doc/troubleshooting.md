# Troubleshooting (`troubleshooting.md`)

The first commands to try when something goes wrong are `agent-cli doctor` and `agent-cli selftest`.

## API Key Issues

### Exiting immediately with `env var ANTHROPIC_API_KEY not set`

- The environment variable specified by `api_key_env` in your config is not set.
- Run `export ANTHROPIC_API_KEY=...` and try again.
- If using `direnv`, check that `.envrc` has been `direnv allow`ed.

### Getting `HTTP 401: ...` responses

- The API key may be expired, invalid, or from a different account.
- Reproduce with `agent-cli doctor` in the `provider conn` step.
- Verify the key's validity in the provider's console.

### Getting `HTTP 400 Bad Request` + `Your credit balance is too low ...` (Claude)

When using the Anthropic Claude backend, you may see a multi-line message like this:

```
[error] provider error (claude): HTTP 400 Bad Request
  request_id : req_011Caekm33...
  config     : /home/hidemi/.config/agent-cli/config.toml
  api_key_env: ANTHROPIC_API_KEY (sk-a...nQAA)
  detail     : {"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits."},...}
  hint       : Your Anthropic account credit balance is insufficient. Visit https://console.anthropic.com/settings/billing to check/purchase credits, or set a different account's API key in the environment variable referenced by `api_key_env`.
```

- **The direct cause is insufficient account credit** (HTTP 400 + `invalid_request_error` response pattern). API key authentication itself succeeded (an invalid key would return HTTP 401).
- Workarounds:
  1. Visit https://console.anthropic.com/settings/billing to purchase credits or enable auto-recharge.
  2. Or set a different account's API key in the environment variable referenced by `api_key_env`.
  3. As a temporary workaround, switch to `provider.kind = "ollama"` or another backend (edit the config file and restart `agent-cli`).
- The `config` line shown is the **resolved path of the actually loaded config file**. If you're unsure whether `~/.local/config/...` or `~/.config/...` is being used, check with `agent-cli config path`.
- The `(sk-a...nQAA)` shown on the `api_key_env` line is a masked version showing the first 4 and last 4 characters of the environment variable value. If it doesn't match the key you expect, a different key is being passed.

### Getting `HTTP 429: ...` responses

- Rate limiting. Check whether you're sending a large number of requests in a short time.
- Increasing `[tools.shell] timeout_secs` can reduce excessive retries during long-running operations.

### Not sure which config file is being used

- `agent-cli config path` prints the resolved config file path.
- Resolution order: `--config <path>` → `AGENT_CLI_CONFIG` env var → default path (`$XDG_CONFIG_HOME/agent-cli/config.toml`, or `~/.config/agent-cli/config.toml` if unset).
- Provider HTTP error messages also include the resolved `config` line, so you can cross-check with `agent-cli config path` output to catch unexpected file usage.

## Ollama / llama.cpp Issues

### `provider conn : FAIL (...)` in `doctor` output

- The local server may be down or running on a different port.
- Verify:
  ```bash
  curl -s http://127.0.0.1:11434/api/tags    # ollama
  curl -s http://127.0.0.1:8080/v1/models    # llama.cpp
  ```
- Update `base_url` in your config to match the actual server.

### `glm-5.1:cloud` not found

- Cloud models in Ollama may not be available in your local environment.
- Run `ollama list` to see available model names, and start with `--model <existing>`.

## OpenCode Issues

### Unexpectedly hitting the cloud (Zen) instead of the local server

- `opencode` mode is selected by **API-key presence**: if the env var named by
  `[provider.opencode] api_key_env` resolves to a value, cloud mode is used.
- For local mode, omit `api_key_env` (or unset the variable) and point
  `base_url` at your `opencode serve` (default `http://127.0.0.1:4096`).

### Connection refused / `provider conn : FAIL` (local opencode)

- `opencode serve` is not running or is on a different port.
- Start it, then `curl -s http://127.0.0.1:4096/session` to confirm, and verify
  `base_url`.

### Local mode: the model can't call shell/fs tools

- Known v1 limitation: agent-cli tool specs are **not** forwarded to a local
  `opencode serve` (session-API tool schema unconfirmed). Use cloud mode if you
  need tool use. See [`doc/providers/opencode.md`](providers/opencode.md).

### Cloud: `ModelError` / 404 / wrong wire format

- `{"error":{"type":"ModelError","message":"Model X not supported"}}` means
  the `model` id is not served by that endpoint (auth was fine — the request
  reached the gateway). Pick a valid id from `{base_url}/models`. Note the
  `hint:` line may misleadingly say "API key invalid" on a 401; the `detail:`
  body is authoritative.
- Choose the wire format with `[provider.opencode] api` and pair it with the
  matching `base_url`: `api = "openai"` → `{base_url}/chat/completions`;
  `api = "anthropic"` → `{base_url}/messages`. For the "go" endpoints use
  `base_url = "https://opencode.ai/zen/go/v1"`. A format/endpoint mismatch
  typically yields 404 or a parse error.

### `persistent_session` seems to forget context / starts a new session

- The session is intentionally recreated when conversation history is cleared
  (`/clear`) or the system prompt changes (e.g. `/reload-persona`), and once
  transparently on a stale-session server error. This is expected; prior
  context is rebuilt from agent-cli's replayed history.
- `persistent_session` is ignored in cloud mode (no local session concept).

## Registry / IPC Issues

### Other processes don't appear in `/list`

- Most likely, the two processes have different `[runtime] registry_dir` settings.
- Check both with `agent-cli config show` and share the same `registry_dir`, then restart.

### Stale sockets / JSON files remaining

- Normally these are cleaned up automatically on process exit. Any of `/quit` / `/exit` / `Ctrl+D` / `Ctrl+C` (SIGINT) / `SIGTERM` triggers `IpcServer` and `RegistryHandle` `Drop` implementations that delete the socket and meta JSON, so leftovers are rare.
- Exception: `SIGKILL` (`kill -9`) or an OS force-kill during a panic may leave remnants.
- Running `agent-cli list` automatically cleans up stale entries (missing PID or missing socket).
- Manual cleanup:
  ```bash
  rm /tmp/agent-cli/*.sock /tmp/agent-cli/*.json
  ```

### `bind ... failed: Permission denied`

- Insufficient permissions on `registry_dir`. Create it with `mkdir -p` and verify `chmod 0700`.
- If a socket was created by root, ownership issues can occur. Clean up and recreate.

## REPL Issues

### `/quit` or `/exit` doesn't terminate

- This was a bug in older versions. The current version (fixed in T-504) reliably exits within 1 second via either `/quit` or `/exit`.
- If you're running an older binary that won't exit, use `Ctrl+C` (SIGINT) or `Ctrl+\` (SIGQUIT) to force-quit, then update to the latest version.

### `Ctrl+D` doesn't exit

- Same issue as above (T-504). In the current version, EOF detection triggers the shutdown channel, which aborts all tasks, deletes files, and calls `std::process::exit(0)`.

### Next prompt doesn't appear after a response / response mixes with previous prompt

- T-505 implemented prompt synchronization (`PromptState::Pending → Ready`), which ensures the next prompt is not drawn until the response completes (`AgentEvent::Done`).
- If symptoms persist, verify that the mechanism that always emits `Done` even on `provider.complete_stream` failure (in `agent.rs`) is working. On errors, the sequence is: error message → `Done` → new prompt.

### Approval `y` being consumed as the next user input (old bug)

- T-506 replaced direct `std::io::stdin` reads with the approval channel (`mpsc::Sender<ApprovalRequest>` + `oneshot::Sender<bool>`). This bug no longer occurs in the current version.
- The approval screen shows `[tool approval] <tool> <args>` and `approve? [y/N]:`. Only `y` / `yes` is accepted as approval; anything else (blank input, other words) counts as denial.

### Tired of approving every time

- During a session, switch to skip mode with `/auto on` (`/auto off` to return, `/auto status` to check).
- To persist, add `[runtime] auto_approve_tools = true` to the config file, or use `--auto-approve-tools` at startup.

### Screen filled with `[thinking]` output (especially with `glm-5.1:cloud`)

Long-reasoning models like `glm-5.1:cloud` emit large amounts of thinking tokens before the response body. agent-cli renders these as `[thinking] <text>` lines, which can fill the terminal and make the actual response hard to see.

Workaround: change `[ui] show_thinking` in `~/.config/agent-cli/config.toml` and restart `agent-cli`.

| Value | Behavior | Recommended when |
|-------|----------|------------------|
| `"hidden"` | Never print `[thinking]` lines | You don't need thinking, just the response |
| `"collapsed"` (default) | Truncate each delta to "first 80 chars + `...`" on one line | You want to see that thinking happened but don't need details |
| `"expanded"` | Full text (previous behavior) | Debugging or you want the full reasoning trace |

Changes require `agent-cli` restart (runtime toggling is not supported). See [`doc/config.md`](config.md) "UI display modes" for implementation details, and `src/app.rs::display_event` / `src/config.rs::ShowThinkingMode` for the code.

### When `[info] max tool-use iterations reached` appears

| Item | Description |
|------|-------------|
| Type | `AgentEvent::Info` (informational; not an error) |
| Trigger | The AI cycles through `tool_use → tool result → tool_use → ...` for a single user input and reaches the `[runtime] max_tool_iterations` cap (default 24) |
| What happens next | The turn ends with `Done`; the next user prompt `> ` is redrawn (FR-03-2) |
| Impact | Not written to error logs or monitoring alerts (`[info]` channel, not `[error]`). Conversation history is preserved |

Meaning: The AI kept calling tools iteratively without reaching a conclusion. This is a guard mechanism (`agent.rs::process_turn` applies `self.config.runtime.max_tool_iterations.max(1)`) to prevent infinite loops.

User-side workarounds (in recommended order):

1. **Split the prompt**: Narrow the goal per request and give instructions step by step.
2. **Be more specific**: State the desired result (file, command, output example) explicitly. This reduces the AI's tendency to keep "exploring" with tools.
3. **Exclude unnecessary tools with `denied_tools`**: Add `denied_tools: [fs_read, fs_write]` in the persona file to prevent the AI from calling irrelevant tools.
4. **Reset the conversation**: Run `/clear` to wipe history and retry with a fresh instruction.
5. **Raise `[runtime] max_tool_iterations`**: Edit the config file to increase the cap (default 24, min 1, max `u32::MAX = 4,294,967,295`). For multi-step orchestrators, try 32/48; for long autonomous runs, 64-256. Changes take effect on `agent-cli` restart. See [`doc/config.md`](config.md) section `[runtime]` for details.

## Shell Tool Issues

### `timed out after 60 seconds: ...`

- The default timeout was exceeded. Increase `[tools.shell] timeout_secs` or instruct the AI to use shorter commands.

### Output ending with `...[truncated]`

- Output exceeded `max_output_kb` (default 256 KB) and was truncated. Increase the threshold.

### `tool_result ERR: spawn error: ...`

- `bash` was not found or is not executable. `agent-cli doctor` also detects this in its bash check.
- This application only supports Linux.

## Persona Issues

For detailed troubleshooting, see [`doc/personas.md`](personas.md) section 11 "Troubleshooting".

### `persona file not found: ...` at startup

- The path specified via `--persona` or `[runtime] persona_file` does not exist.
- The default path (`<agents_dir>/<name>.md`) silently falls back to the built-in default if the file is missing.

### `role` is required error

- The YAML frontmatter at the top of the persona file is missing `role: ...` or has an empty value.
- See the example in `example/agents/coder.md`.

### `/reload-persona` doesn't seem to take effect

- Did you edit the persona file? Check the path with `/persona`'s `source` line.
- The system prompt changes in the next response after reload.
- Note that `allowed_tools` / `denied_tools` / `model` / `temperature` changes currently require a restart; only the system prompt is updated immediately.

## Config File Issues

### `error: config file not found: ...`

- An explicit path specified via `--config` or `AGENT_CLI_CONFIG` must exist; it will not be auto-generated.
- The default path (`~/.config/agent-cli/config.toml`) is auto-generated on first run.

### `provider error (claude): [provider.claude] missing`

- The config file may be missing or have a broken `[provider.claude]` section.
- Check the TOML structure with `agent-cli config show`.

## Build / Install Issues

### `cargo install` fails with `--locked`

- This occurs when `Cargo.lock` is not in the repository. `install.sh` automatically retries without `--locked`, so this is usually not a problem.
- To install manually:
  ```bash
  cargo install --path . --root "$HOME/.local"
  ```

### `install.sh` says `cargo is required but not found.`

- The Rust toolchain is not installed.
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  ```

## Context-efficiency Features (opt-in)

These are all default-OFF; see [`doc/config.md`](config.md) §11.

### `[info] history compacted: ...` appears mid-conversation

- Not an error. With `[history] enabled = true`, when the estimated context
  exceeds `max_context_tokens` the oldest turns are summarized (and/or
  dropped). The system/persona prefix and the most recent `keep_recent_turns`
  are always kept. Raise `max_context_tokens` / `keep_recent_turns`, or set
  `enabled = false`, to change this.

### Older details seem lost after a long session

- Expected when `[history] enabled = true`: old turns are condensed into a
  summary. Increase `keep_recent_turns` or `max_context_tokens` to retain more
  verbatim, or disable history management.

### Claude `prompt_cache = true` but no apparent speed-up

- The cache has a ≈5-minute TTL and only helps on a stable repeated prefix
  (system/tools/conversation tail). The first call is always a cache write;
  benefits show on subsequent calls within the TTL. The full history is still
  sent either way. No effect on non-Claude backends.

## Still not resolved?

Please open an issue on the repository with:

1. Full output of `agent-cli doctor`
2. `cargo --version` and `rustc --version`
3. Your config file (with API keys redacted)
4. The command you ran and the error message you saw