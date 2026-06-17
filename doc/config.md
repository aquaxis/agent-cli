# Configuration Reference (`config.md`)

This document provides a comprehensive guide to configuring `agent-cli`. For a quick reference, see `README.md`; for detailed startup options, see `doc/usage.md`.

## Table of Contents

1. [Configuration File Location and Resolution Order](#1-configuration-file-location-and-resolution-order)
2. [Overall Structure and Section Roles](#2-overall-structure-and-section-roles)
3. [Full Item Reference](#3-full-item-reference)
4. [Complete Examples](#4-complete-examples)
5. [API Key and Secret Management](#5-api-key-and-secret-management)
6. [Multiple Profile Usage](#6-multiple-profile-usage)
7. [Shell Tool Tuning](#7-shell-tool-tuning)
8. [UI Display Mode](#8-ui-display-mode)
9. [Common Configuration Mistakes and Diagnostics](#9-common-configuration-mistakes-and-diagnostics)
10. [Applying Configuration Changes and Restarting](#10-applying-configuration-changes-and-restarting)
11. [Context-efficiency Features (opt-in)](#11-context-efficiency-features-opt-in)

## 1. Configuration File Location and Resolution Order

`agent-cli` resolves the configuration file path in the following priority order:

```text
1. --config <path>             <- Highest priority (explicit specification)
2. Environment variable AGENT_CLI_CONFIG   <- Next
3. ~/.config/agent-cli/config.toml <- Default
```

Behavior:

- If the file specified by option 1 or 2 **does not exist**, the process exits with an error. No auto-generation is performed.
- When option 3 is used and the file does not exist, it is **auto-generated** with default values.
- The resolved path can be confirmed with `agent-cli config path`.

```bash
agent-cli config path
# Example: /home/alice/.config/agent-cli/config.toml

agent-cli --config ./project-a.toml config path
# Example: /home/alice/work/project-a.toml
```

## 2. Overall Structure and Section Roles

```toml
[provider]                  # Which backend to use
[provider.claude]           # claude backend-specific settings
[provider.codex]            # codex (OpenAI) backend-specific settings
[provider.ollama]           # ollama backend-specific settings
[provider.opencode]         # opencode backend (local serve / OpenCode Zen)
[provider."llama.cpp"]      # llama.cpp server-specific settings (key must be quoted)

[runtime]                   # Runtime behavior and paths
[tools]                     # Tool-wide settings
[tools.shell]               # Shell tool tuning

[ui]                        # Display mode
[history]                   # Opt-in history-window management
```

## 3. Full Item Reference

### `[provider]`

| Key | Type | Default | Required | Description |
|------|----|------|------|------|
| `kind` | string | `"claude"` | Yes | Backend to use: `"claude"` / `"codex"` / `"ollama"` / `"opencode"` / `"llama.cpp"` |

### `[provider.claude]` / `[provider.codex]` / `[provider.ollama]` / `[provider.opencode]` / `[provider."llama.cpp"]`

| Key | Type | Default | Required | Description |
|------|----|------|------|------|
| `model` | string | Per-backend default | Yes | Model name to use |
| `api_key_env` | string | Per-backend default | Cond. | Environment variable name holding the API key (not the value itself). For `opencode`, presence selects cloud (Zen) vs local mode |
| `base_url` | string | Per-backend default | Cond. | Endpoint URL. Override when using a proxy or compatible server |
| `thinking` | bool | `true` (only meaningful for claude) | Cond. | Enable thinking blocks (`claude` only) |
| `prompt_cache` | bool | `false` (**claude only**) | No | Opt-in Anthropic prompt caching (`cache_control` on system / tools / conversation tail). See §11 |
| `persistent_session` | bool | `false` (**opencode local only**) | No | Opt-in: reuse one OpenCode server session across turns. See §11 |
| `api` | string | `"openai"` (**opencode cloud only**) | No | Cloud wire format: `"openai"` → `{base}/chat/completions`; `"anthropic"` → `{base}/messages`. Pair with the matching `base_url` (e.g. `https://opencode.ai/zen/go/v1`) |
| `request_timeout_secs` | int | `900` | No | Total HTTP timeout incl. streaming |

Per-backend defaults:

| kind | model default | base_url default | api_key_env default |
|------|-----------|---------------|-------------------|
| claude | `claude-opus-4-7` | `https://api.anthropic.com` | `ANTHROPIC_API_KEY` |
| codex | `gpt-4.1` | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| ollama | `glm-5.1:cloud` | `http://127.0.0.1:11434` | (not needed) |
| opencode | `claude-sonnet-4-5` | `http://127.0.0.1:4096` (local) / `https://opencode.ai/zen/v1` (when key set) | (none = local; set = cloud, e.g. `OPENCODE_API_KEY`) |
| llama.cpp | `default` | `http://127.0.0.1:8080` | (optional) |

`opencode` runs in two modes selected by **API-key presence**: no resolved key → **local** mode against a running `opencode serve` (native session API); key resolved → **cloud** mode against OpenCode Zen (OpenAI-compatible). See [`doc/providers/opencode.md`](providers/opencode.md).

### `[runtime]`

| Key | Type | Default | Description |
|------|----|------|------|
| `auto_approve_tools` | bool | `false` | When `true`, skips the y/N approval prompt for tool execution. At runtime, the same toggle can be switched via REPL commands `/auto on` / `/auto off` / `/auto status` |
| `log_dir` | string | `~/.local/share/agent-cli/logs` | Directory where conversation logs are saved |
| `registry_dir` | string | empty | Location of the agent registry. When empty, uses `$XDG_RUNTIME_DIR/agent-cli` or `/tmp/agent-cli` |
| `agents_dir` | string | `~/.config/agent-cli/agents` | Directory to search for persona files (`<agents_dir>/<name>.md`). See [`doc/personas.md`](personas.md) for details |
| `persona_file` | string | empty | Explicit persona file path. When empty, falls back to `<agents_dir>/<name>.md` or the built-in default. See [`doc/personas.md`](personas.md) for details |
| `max_tool_iterations` | u32 | `24` | Upper limit for tool_use iterations within a single turn. Minimum is 1 (`0` or negative values are clamped to `1` internally), maximum is `u32::MAX = 4,294,967,295`. This is a safeguard to prevent infinite loops. See "Tuning `max_tool_iterations`" below for details |

#### Tuning `max_tool_iterations`

This is the upper limit for the loop where the AI repeats `tool_use -> tool result -> tool_use -> ...` for a single user input. When the limit is reached, the REPL displays `[info] max tool-use iterations reached` and ends that turn as `Done` (this is an informational notification, not an error).

**Q&A:**

| Question | Answer |
|------|------|
| Can I change this in the config file? | Yes. Edit `[runtime] max_tool_iterations` and restart `agent-cli`. It cannot be changed dynamically in a running REPL. |
| Is an unlimited setting possible? | Not strictly. The type is `u32`, so the maximum is `u32::MAX = 4,294,967,295` iterations (practically unlimited). A "truly unlimited loop" is intentionally not provided to prevent runaway API costs, GPU occupation, and stdout blocking. If you need effectively unlimited, set `max_tool_iterations = 4294967295`. |

**Boundary value behavior:**

- `0` or negative values: Treated as `1` iteration via `.max(1)` in the implementation.
- `1` to `u32::MAX`: Used as-is.
- Values exceeding `u32::MAX`: Cause an overflow error during TOML parsing, and startup fails.

**Recommended ranges (by use case):**

| Use case | Recommended value | Rationale |
|------|--------|------|
| Simple conversation / education | `4-8` | Truncates runaway loops earlier |
| Default (design-then-debug, etc.) | `24` (default) | Fits a typical workflow of design artifact generation -> verification -> lint fix -> fs_write |
| Multi-step orchestrator | `32-48` | When calling multiple tools sequentially |
| Long autonomous execution (experimental) | `64-256` | When decomposing large tasks step by step |
| Beyond that | Not recommended | You should suspect the AI is stuck in a loop. Operate with the assumption that you can intervene via `/cancel` or `Ctrl+C` |

Configuration example:

```toml
[runtime]
max_tool_iterations = 48   # Multi-step orchestrator use case
```

### `[tools]`

| Key | Type | Default | Description |
|------|----|------|------|
| `enabled` | string[] | `["shell","fs_read","fs_write","send_to"]` | Tools to enable |

If the persona has `allowed_tools` / `denied_tools`, the **intersection / difference** with this list determines the final tool set.

### `[tools.shell]`

| Key | Type | Default | Description |
|------|----|------|------|
| `timeout_secs` | int | `60` | Timeout per command (seconds) |
| `max_output_kb` | int | `256` | Maximum retained size for stdout/stderr (KB) |

### `[ui]`

| Key | Type | Default | Description |
|------|----|------|------|
| `show_thinking` | string | `"collapsed"` | Thinking display mode: `"collapsed"` (truncated to the first 80 characters + first line) / `"expanded"` (full text) / `"hidden"` (not displayed). See "UI Display Mode" below for details |

### `[history]`

Opt-in hybrid history-window management. When `enabled = false` (default), the
full conversation is replayed verbatim every turn (unchanged behavior).

| Key | Type | Default | Description |
|------|----|------|------|
| `enabled` | bool | `false` | Master switch. When false, no summarization or trimming occurs |
| `max_context_tokens` | int | `24000` | Approx. budget (estimated tokens ≈ chars/4). Compaction runs when exceeded |
| `keep_recent_turns` | int | `6` | Most-recent messages always kept verbatim (system/persona prefix is always kept too) |

See §11 for the compaction algorithm.

## 4. Complete Examples

### 4.1 Minimal Configuration (claude)

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
```

### 4.2 Recommended Configuration (claude as primary, ollama reserved for verification)

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
model       = "claude-opus-4-7"
thinking    = true

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"

[runtime]
auto_approve_tools = false
log_dir            = "~/.local/share/agent-cli/logs"

[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]

[tools.shell]
timeout_secs  = 120
max_output_kb = 512

[ui]
show_thinking = "collapsed"
```

### 4.3 Full-featured Configuration

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
model       = "claude-opus-4-7"
base_url    = "https://api.anthropic.com"
thinking    = true

[provider.codex]
api_key_env = "OPENAI_API_KEY"
model       = "gpt-4.1"
base_url    = "https://api.openai.com/v1"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"

[runtime]
auto_approve_tools  = false
log_dir             = "~/.local/share/agent-cli/logs"
registry_dir        = "/tmp/agent-cli"
agents_dir          = "~/.config/agent-cli/agents"
persona_file        = ""
max_tool_iterations = 48                            # Multi-step orchestrator assumed

[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]

[tools.shell]
timeout_secs  = 60
max_output_kb = 256

[ui]
show_thinking = "expanded"
```

## 5. API Key and Secret Management

`agent-cli` **never writes API key values in the configuration file**. `api_key_env` specifies the **environment variable name**, and the actual value is retrieved from that environment variable.

### 5.1 Setting in Shell

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
agent-cli run
```

### 5.2 `direnv` `.envrc`

Example of switching values specific to a project directory:

```bash
# .envrc
export ANTHROPIC_API_KEY="$(pass anthropic/api_key)"
export AGENT_CLI_CONFIG="$PWD/agent-cli.toml"
```

### 5.3 `systemd EnvironmentFile`

```ini
# ~/.config/systemd/user/agent-cli@.service
[Service]
Type=simple
EnvironmentFile=%h/.config/agent-cli/secrets.env
ExecStart=%h/.local/bin/agent-cli run --name %i
```

Store API keys in `secrets.env` with `chmod 600`.

### 5.4 Notes

- **Never commit** plaintext secrets to a repository. Add `.env`, `.envrc`, and `secrets.*` to `.gitignore`.
- `agent-cli config show` outputs the environment variable name (`api_key_env`), so the API key itself is not leaked.

## 6. Multiple Profile Usage

You can switch configurations per project or per use case using `--config`.

```bash
# claude profile
agent-cli --config ~/profiles/claude.toml run --name alice

# ollama profile
agent-cli --config ~/profiles/ollama.toml run --name bob
```

### 6.1 Running as Independent Agents

By setting different `registry_dir` values, each profile operates in an independent namespace invisible to the other via `/list`.

```toml
# claude.toml
[runtime]
registry_dir = "/tmp/agent-cli/claude"

# ollama.toml
[runtime]
registry_dir = "/tmp/agent-cli/ollama"
```

### 6.2 Peer-to-Peer Communication

By sharing `registry_dir`, agents with different profiles can call each other via `/send`.

```toml
# Add the following to both configurations
[runtime]
registry_dir = "/tmp/agent-cli/team"
```

## 7. Shell Tool Tuning

To allow long-running jobs or commands that produce large output, adjust `[tools.shell]`.

```toml
[tools.shell]
timeout_secs  = 600   # 10 minutes
max_output_kb = 4096  # 4 MB
```

Notes:

- Processes exceeding `timeout_secs` are force-killed, and the tool result is treated as a failure.
- stdout/stderr exceeding `max_output_kb` is truncated with `...[truncated]` appended to the end.
- To prevent the AI from accidentally invoking huge commands, it is recommended to also use interactive approval (`auto_approve_tools=false`).

## 8. UI Display Mode

`ui.show_thinking` controls the display volume of thinking blocks (Claude's `thinking_delta` / Ollama's `message.thinking`). It is interpreted at `agent-cli` startup; unknown values (e.g., `"verbose"`) fall back to the default `"collapsed"`.

| Value | Behavior |
|----|------|
| `"collapsed"` (default) | Truncates each thinking delta to "first 80 characters + `...`"; if there is a newline, only the first line is shown. Displayed as a single line in the format `[thinking] <truncated>...` |
| `"expanded"` | Displays the full received thinking text in real time (`[thinking] <text>`) |
| `"hidden"` | Does not display thinking lines at all (discards `AgentEvent::Thinking` on the REPL side) |

Configuration changes take effect after restarting `agent-cli`. Dynamic switching at runtime is not supported.

## 9. Common Configuration Mistakes and Diagnostics

### Symptom: Process exits immediately after startup

- Cause: The environment variable specified by `api_key_env` is not set.
- Diagnosis: Run `agent-cli doctor`. It will display `api key env : ANTHROPIC_API_KEY ... NOT set`.
- Resolution: `export` the environment variable, or switch to a different `provider.kind`.

### Symptom: Other processes do not appear in `agent-cli list`

- Cause: `registry_dir` differs between processes, or the socket is stale.
- Diagnosis: Compare the `registry_dir` in `agent-cli config show` from both sides. Check `.sock` / `.json` files with `ls /tmp/agent-cli/`.
- Resolution: Restart with a shared `registry_dir` configuration.

### Symptom: `provider conn : FAIL` appears in `doctor`

- Cause: API key is incorrect / local server is stopped / `base_url` is wrong.
- Diagnosis: Try `curl -s $base_url/health` manually.
- Resolution: Verify the URL, key, and server status.

### Symptom: Shell tool reports "timed out"

- Cause: `timeout_secs` was exceeded.
- Resolution: Increase `[tools.shell] timeout_secs`, or instruct the AI to use shorter commands.

### Symptom: Exits with `config file not found`

- Cause: A non-existent path was specified via `--config` or `AGENT_CLI_CONFIG` (explicit paths are not auto-generated).
- Resolution: Verify the path, or use the default path (which is auto-generated).

## 10. Applying Configuration Changes and Restarting

- Most settings are **loaded at process startup**, so restart `agent-cli` after making changes.
- As exceptions, the following can be changed dynamically from a running REPL:
  - **Persona file**: Reload with `/reload-persona` in the REPL (updates the system prompt only; conversation history is preserved).
  - **Tool approval skip**: `/auto on` / `/auto off` / `/auto status` in the REPL (overrides `auto_approve_tools` on the spot).
- `--provider` / `--model` / `--persona` / `--auto-approve-tools` can be overridden via CLI options (per process).

## 11. Context-efficiency Features (opt-in)

agent-cli replays the full conversation history to the provider on every send.
For long sessions this grows cost/latency. Three **opt-in** features mitigate
this; all default OFF, and with every flag off behavior is byte-for-byte
unchanged.

### 11.1 Claude prompt caching — `[provider.claude] prompt_cache`

```toml
[provider.claude]
prompt_cache = true
```

Adds Anthropic `cache_control: {type:"ephemeral"}` breakpoints to the system
prompt, the last tool definition, and the last message's last content block
(≤ 3 of Anthropic's 4 allowed). The repeated prefix is then served from
Anthropic's cache (≈ 5-minute TTL) instead of being reprocessed each turn —
the full history is still sent, but cheaper/faster. No effect on other
backends.

### 11.2 opencode persistent session — `[provider.opencode] persistent_session`

```toml
[provider.opencode]
base_url           = "http://127.0.0.1:4096"   # local mode (no api_key_env)
persistent_session = true
```

Local mode only (ignored in cloud/Zen mode). Creates one OpenCode server
session and reuses its `session_id` across turns, sending only the new
user/tool turns instead of re-flattening the whole history (the server retains
prior context). The session is recreated when history is cleared (`/clear`) or
the system prompt changes; a stale-session server error triggers one
transparent recreate + resend.

### 11.3 Hybrid history-window management — `[history]`

```toml
[history]
enabled            = true
max_context_tokens = 24000
keep_recent_turns  = 6
```

Before each turn, if the estimated context (≈ chars/4) exceeds
`max_context_tokens`:

1. **Summarize:** the "old span" (everything after the system/persona prefix
   and before the last `keep_recent_turns` messages) is summarized by the LLM
   into a single summary message that replaces that span.
2. **Drop:** if still over budget, the oldest old-span messages are dropped
   one at a time until under budget.

The system/persona prefix and the most recent `keep_recent_turns` messages are
never summarized or dropped. Summarization is best-effort: a failed
summarization call degrades to drop-only and never fails the turn. The REPL
prints an `[info]` line reporting what was compacted. Works with any provider.

> Provider-side note: at the network layer every send is still an independent
> request (claude/ollama/codex/opencode-cloud APIs are stateless); only the
> TCP/TLS socket is pooled. These features reduce *what* is reprocessed/sent,
> not the per-send request model. See [`doc/architecture.md`](architecture.md).