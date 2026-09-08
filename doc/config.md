# Configuration Reference (`config.md`)

This document provides a comprehensive guide to configuring `agent-cli`. For a quick reference, see `README.md`; for detailed startup options, see `doc/usage.md`.

## Table of Contents

1. [Configuration File Location and Resolution Order](#1-configuration-file-location-and-resolution-order)
2. [Overall Structure and Section Roles](#2-overall-structure-and-section-roles)
3. [Full Item Reference](#3-full-item-reference)
4. [Complete Examples](#4-complete-examples)
5. [API Key and Secret Management](#5-api-key-and-secret-management)
6. [Multiple Profile Usage](#6-multiple-profile-usage)
7. [Bash Tool Tuning](#7-bash-tool-tuning)
8. [UI Display Mode](#8-ui-display-mode)
9. [Common Configuration Mistakes and Diagnostics](#9-common-configuration-mistakes-and-diagnostics)
10. [Applying Configuration Changes and Restarting](#10-applying-configuration-changes-and-restarting)
11. [Context-efficiency Features (opt-in)](#11-context-efficiency-features-opt-in)

## 1. Configuration File Location and Resolution Order

`agent-cli` resolves the configuration file path in the following priority order:

```text
1. --config <path>                 <- Highest priority (explicit specification)
2. Environment variable AGENT_CLI_CONFIG   <- Next
3. ./.agent-cli/config.toml        <- Project-local (only if it already exists)
4. ~/.config/agent-cli/config.toml <- Default
```

Behavior:

- If the file specified by option 1 or 2 **does not exist**, the process exits with an error. No auto-generation is performed.
- Option 3 is the `.agent-cli/config.toml` file under the **current working directory**. It is used **only when it already exists**; it is never auto-generated, and its absence silently falls through to option 4. The path is resolved to an absolute path so a detached agent spawned from here reads the same file. Only the current directory is checked — parent directories are not walked.
- When option 4 is used and the file does not exist, it is **auto-generated** with default values.
- The resolved path can be confirmed with `agent-cli config path`.

```bash
agent-cli config path
# Example: /home/alice/.config/agent-cli/config.toml

agent-cli --config ./project-a.toml config path
# Example: /home/alice/work/project-a.toml
```

A fully commented starting point covering every section below ships with the
repository as [`example/config.example.toml`](../example/config.example.toml).
Copy it to the resolved path and edit, or point `--config` at your own copy.

## 2. Overall Structure and Section Roles

```toml
[provider]                  # Which backend to use
[provider.claude]           # claude backend-specific settings
[provider.claude-code]      # claude-code backend (drives the local `claude` CLI)
[provider.codex]            # codex (OpenAI) backend-specific settings
[provider.ollama]           # ollama backend-specific settings
[provider.opencode]         # opencode backend (local serve / OpenCode Zen)
[provider."llama.cpp"]      # llama.cpp server-specific settings (key must be quoted)

[runtime]                   # Runtime behavior and paths
[tools]                     # Tool-wide settings
[tools.bash]                # Bash tool tuning
[tools.websearch]           # websearch tool endpoint / key (opt-in)

[ui]                        # Display mode
[history]                   # Opt-in history-window management
[mcp]                       # MCP client: global options
[[mcp.servers]]             # MCP client: one external server per entry
```

## 3. Full Item Reference

### `[provider]`

| Key | Type | Default | Required | Description |
|------|----|------|------|------|
| `kind` | string | `"claude"` | Yes | Backend to use: `"claude"` / `"claude-code"` / `"codex"` / `"ollama"` / `"opencode"` / `"opencode-go"` / `"llama.cpp"` |

### `[provider.claude]` / `[provider.codex]` / `[provider.ollama]` / `[provider.opencode]` / `[provider.opencode-go]` / `[provider."llama.cpp"]`

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
| `temperature` | float | Backend default | No | Sampling temperature. When omitted, the field is left out of the request entirely and the backend's own default applies. A persona's `temperature` overrides this for the agent that loads it |
| `max_retries` | int | `3` (**ollama only**) | No | Retry count for transient failures (retryable HTTP status, timeout, connection error). Other backends do not retry |

#### `llama.cpp` sampling keys

These are accepted on any `[provider.*]` table but only consumed by the
`llama.cpp` backend, where they mirror the `llama-cli` flags of the same name.
All are optional — omit a key and the server's own default applies.

| Key | Type | `llama-cli` equivalent | Description |
|------|----|------|------|
| `max_tokens` | int | `-n` / `--n-predict` | Maximum number of tokens to generate |
| `top_k` | int | `--top-k` | Top-K sampling cutoff |
| `top_p` | float | `--top-p` | Nucleus sampling cutoff |
| `min_p` | float | `--min-p` | Minimum-probability cutoff |
| `repeat_penalty` | float | `--repeat-penalty` | Repetition penalty |
| `repeat_last_n` | int | `--repeat-last-n` | Window size the repetition penalty applies to |
| `seed` | int | `--seed` | Sampling seed for reproducible output |

Per-backend defaults:

| kind | model default | base_url default | api_key_env default |
|------|-----------|---------------|-------------------|
| claude | `claude-opus-4-7` | `https://api.anthropic.com` | `ANTHROPIC_API_KEY` |
| codex | `gpt-4.1` | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| ollama | `glm-5.1:cloud` | `http://127.0.0.1:11434` | (not needed) |
| opencode | `claude-sonnet-4-5` | `http://127.0.0.1:4096` (local) / `https://opencode.ai/zen/v1` (when key set) | (none = local; set = cloud, e.g. `OPENCODE_API_KEY`) |
| opencode-go | `claude-sonnet-4-5` | `https://opencode.ai/zen/go/v1` | `OPENCODE_API_KEY` |
| llama.cpp | `default` | `http://127.0.0.1:8080` | (optional) |

### `[provider.claude-code]`

Drives the locally installed **Claude Code CLI** (`claude`) as a backend. It
takes no `api_key_env`, `base_url`, or `request_timeout_secs`: authentication
and endpoints belong to Claude Code itself. Every key below is optional, and the
whole section may be omitted. See
[`doc/providers/claude-code.md`](providers/claude-code.md).

| Key | Type | Default | Description |
|------|----|------|------|
| `bin` | string | `"claude"` | Executable. A value containing a path separator is used as-is; otherwise it is resolved on `PATH`. A missing binary is a startup error |
| `model` | string | (Claude Code's own) | `--model` |
| `mode` | string | `"delegation"` | `"delegation"` — Claude Code runs its own tools and keeps its session; `"gateway"` — `--tools ""`, chat only |
| `transport` | string | `"stream"` | `"stream"` — `--output-format stream-json` with token deltas; `"oneshot"` — `--output-format json`, whole reply at end of turn |
| `session` | string | `"persistent"` | Delegation only. `"persistent"` reuses one Claude Code session and sends only new messages; `"ephemeral"` adds `--no-session-persistence` and re-sends the transcript |
| `tools` | array | unset | `--tools`. Ignored in gateway mode, which always sends `--tools ""` |
| `allowed_tools` | array | unset | `--allowed-tools` |
| `disallowed_tools` | array | unset | `--disallowed-tools` |
| `permission_mode` | string | unset | `--permission-mode`, passed through unvalidated |
| `turn_timeout_secs` | int | `900` | Per-turn wall clock. On expiry the child is killed and the turn ends with an error; the provider stays usable |
| `max_budget_usd` | float | unset | `--max-budget-usd`. Recommended for unattended use |
| `system_prompt_mode` | string | `"append"` | `"append"` → `--append-system-prompt`; `"replace"` → `--system-prompt` |
| `extra_args` | array | `[]` | Extra CLI flags appended verbatim before the prompt argument |

Unknown values for `mode`, `transport`, `session`, or `system_prompt_mode` are
rejected at startup with the accepted set listed.

Two consequences worth knowing before choosing a mode:

- **Gateway mode is chat-only.** `claude -p` accepts no external tool
  definitions, so agent-cli's own tools cannot be handed to it. The model will
  describe the tool it would use and stop.
- **Delegation mode bypasses agent-cli's approval prompt.** Claude Code has
  already run the tool by the time it reports it, so those events are never
  turned into agent-cli tool calls (that would execute everything twice).
  Control permissions with `permission_mode` / `tools` / `allowed_tools` /
  `disallowed_tools`.

`opencode` runs in two modes selected by **API-key presence**: no resolved key → **local** mode against a running `opencode serve` (native session API); key resolved → **cloud** mode against OpenCode Zen (OpenAI-compatible). `opencode-go` is a convenience alias that sets `base_url` to the Go endpoint, `api` to `"anthropic"`, and `api_key_env` to `"OPENCODE_API_KEY"`. It still uses `[provider.opencode]` for overrides. See [`doc/providers/opencode.md`](providers/opencode.md).

### `[runtime]`

| Key | Type | Default | Description |
|------|----|------|------|
| `auto_approve_tools` | bool | `false` | When `true`, skips the y/N approval prompt for tool execution. At runtime, the same toggle can be switched via REPL commands `/auto on` / `/auto off` / `/auto status` |
| `log_dir` | string | `~/.local/share/agent-cli/logs` | Directory where conversation logs are saved |
| `registry_dir` | string | empty | Location of the agent registry. When empty, uses `$XDG_RUNTIME_DIR/agent-cli` or `/tmp/agent-cli` |
| `agents_dir` | string | `~/.config/agent-cli/agents` | Directory to search for persona files (`<agents_dir>/<name>.md`). See [`doc/personas.md`](personas.md) for details |
| `persona_file` | string | empty | Explicit persona file path. When empty, falls back to `<agents_dir>/<name>.md` or the built-in default. See [`doc/personas.md`](personas.md) for details |
| `max_tool_iterations` | u32 | `24` | Upper limit for tool_use iterations within a single turn. Minimum is 1 (`0` or negative values are clamped to `1` internally), maximum is `u32::MAX = 4,294,967,295`. This is a safeguard to prevent infinite loops. See "Tuning `max_tool_iterations`" below for details |
| `commands_dir` | string | `.agent-cli/commands` | Directory scanned for user-defined custom slash commands (`*.md`). Relative paths resolve against the working directory; `~` and env-style paths are expanded. An empty string falls back to the default, and a directory that does not exist is not an error — the REPL simply runs with built-in commands only. See [`doc/usage.md`](usage.md) "Custom Slash Commands" |
| `group` | string | unset | Default group id for agents this config launches. The `--group` command-line flag overrides it; with neither, agents are ungrouped. Detached children inherit their launcher's effective group. See [`doc/usage.md`](usage.md) "Groups" |

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
| Default (design-then-debug, etc.) | `24` (default) | Fits a typical workflow of design artifact generation -> verification -> lint fix -> write |
| Multi-step orchestrator | `32-48` | When calling multiple tools sequentially |
| Long autonomous execution (experimental) | `64-256` | When decomposing large tasks step by step |
| Beyond that | Not recommended | You should suspect the AI is stuck in a loop. Operate with the assumption that you can intervene: `Esc` (or `Ctrl+C`, or `/cancel`) stops the running turn and returns you to the prompt |

Configuration example:

```toml
[runtime]
max_tool_iterations = 48   # Multi-step orchestrator use case
```

### `[tools]`

| Key | Type | Default | Description |
|------|----|------|------|
| `enabled` | string[] | `["bash","read","write","send_to","monitor","edit","glob","grep","websearch","webfetch"]` | Tools to enable |

If the persona has `allowed_tools` / `denied_tools`, the **intersection / difference** with this list determines the final tool set.

**Legacy tool names.** The pre-rename names `shell`, `fs_read`, and `fs_write`
are still accepted in `enabled` (and in persona `allowed_tools` / `denied_tools`)
and are mapped to `bash`, `read`, and `write` respectively, so configuration
files written before the rename keep working. New configurations should use the
canonical names.

### `[tools.bash]`

| Key | Type | Default | Description |
|------|----|------|------|
| `timeout_ms` | int | `120000` | Timeout per command, **in milliseconds** (the default is 120 seconds) |
| `max_output_kb` | int | `256` | Maximum retained size for stdout/stderr (KB) |

A legacy `[tools.shell]` table is still loaded as `[tools.bash]`, so an old
configuration file does not break. Its `timeout_secs` key has no equivalent and
is ignored — the default `timeout_ms` applies unless you add it explicitly.

### `[tools.websearch]`

Configuration for the `websearch` tool. Network access is opt-in: while
`api_key_env` and `endpoint` are unset, `websearch` returns a configuration hint
instead of results, and every other tool is unaffected.

| Key | Type | Default | Description |
|------|----|------|------|
| `api_key_env` | string | unset | Name of the environment variable holding the search API key (not the key itself) |
| `endpoint` | string | unset | Search endpoint URL (provider-specific) |
| `provider` | string | `"tavily"` | Search provider identifier, e.g. `"tavily"` / `"brave"` |

```toml
[tools.websearch]
api_key_env = "TAVILY_API_KEY"
endpoint    = "https://api.tavily.com/search"
provider    = "tavily"
```

### `[ui]`

| Key | Type | Default | Description |
|------|----|------|------|
| `show_thinking` | string | `"collapsed"` | Thinking display mode: `"collapsed"` (truncated to the first 80 characters + first line) / `"expanded"` (full text) / `"hidden"` (not displayed). See "UI Display Mode" below for details |
| `show_progress` | bool | `true` | Draw the progress indicator (activity line + spinner and elapsed time) while a turn runs. Only ever drawn when stdin and stderr are both terminals; see "UI Display Mode" below |

### `[history]`

Opt-in hybrid history-window management. When `enabled = false` (default), the
full conversation is replayed verbatim every turn (unchanged behavior).

| Key | Type | Default | Description |
|------|----|------|------|
| `enabled` | bool | `false` | Master switch. When false, no summarization or trimming occurs |
| `max_context_tokens` | int | `24000` | Approx. budget (estimated tokens ≈ chars/4). Compaction runs when exceeded |
| `keep_recent_turns` | int | `6` | Most-recent messages always kept verbatim (system/persona prefix is always kept too) |

See §11 for the compaction algorithm.

### `[mcp]` / `[[mcp.servers]]`

Model Context Protocol (MCP) **client** configuration. On `run` / `serve`,
agent-cli connects each enabled server — over **stdio** (a launched subprocess,
the default) or **http** (Streamable HTTP to a URL) — discovers its tools via the
MCP handshake, and registers each one under the namespaced name
`mcp__<server>__<tool>`. Omitting the section (no servers) leaves behaviour
unchanged. Only MCP **tools** (not resources/prompts) are consumed; Linux-only.

`[mcp]` (optional, global):

| Key | Type | Default | Description |
|------|----|------|------|
| `init_timeout_ms` | int | `15000` | Per-server handshake + `tools/list` timeout, in milliseconds. A server exceeding it is skipped |

`[[mcp.servers]]` (one table per server):

| Key | Type | Default | Description |
|------|----|------|------|
| `name` | string | — (required) | Logical name; used in the tool namespace `mcp__<name>__<tool>` |
| `transport` | string | `"stdio"` | Transport kind: `"stdio"` or `"http"` |
| `enabled` | bool | `true` | Whether the server is connected |
| `command` | string | — (required for stdio) | Executable to launch (resolved on `PATH` or an absolute path). **stdio only** |
| `args` | string[] | `[]` | Arguments passed to `command`. **stdio only** |
| `env` | table | `{}` | Extra environment variables merged onto the inherited environment. **stdio only** |
| `cwd` | string | unset | Working directory for the child (`~` / env expansion applied). **stdio only** |
| `url` | string | — (required for http) | HTTP endpoint (Streamable HTTP). **http only** |
| `headers` | table | `{}` | Static request headers sent on every HTTP call. **http only** |
| `api_key_env` | string | unset | Env var whose value is sent as `Authorization: Bearer <value>`. **http only** |

The `http` transport POSTs JSON-RPC to `url` and accepts either a single
`application/json` reply or a `text/event-stream` (SSE) reply; it echoes the
`Mcp-Session-Id` returned by `initialize` on subsequent requests. Only the
single-endpoint Streamable HTTP transport is supported (no legacy two-endpoint
HTTP+SSE, no OAuth — use `api_key_env`/`headers` for auth).

MCP tools are **not** gated by `[tools] enabled` (that list governs only the
built-ins); a declared, enabled server's tools are available by default, subject
to the persona `allowed_tools` / `denied_tools` filter (which may name an
individual `mcp__…` tool). MCP tool calls pass through the normal approval gate.
A server that fails to launch, handshake, or list is logged and skipped, so a bad
server never aborts startup.

```toml
[mcp]
init_timeout_ms = 15000

[[mcp.servers]]                 # stdio
name    = "filesystem"
command = "npx"
args    = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user"]
# env     = { EXAMPLE = "1" }
# cwd     = "/home/user"
# enabled = true

[[mcp.servers]]                 # http (Streamable HTTP)
name        = "remote"
transport   = "http"
url         = "https://example.com/mcp"
# headers     = { X-Example = "1" }
# api_key_env = "REMOTE_MCP_TOKEN"       # -> Authorization: Bearer <value>
```

Inspect configured servers with `agent-cli mcp list` (and `agent-cli doctor`).
See [`doc/usage.md`](usage.md#mcp-servers).

## 4. Complete Examples

### 4.1 Minimal Configuration (claude)

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
```

### 4.1b Minimal Configuration (OpenCode Go)

```toml
[provider]
kind = "opencode-go"

[provider.opencode]
api_key_env = "OPENCODE_API_KEY"
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
enabled = ["bash", "read", "write", "send_to", "monitor", "edit", "glob", "grep", "websearch", "webfetch"]

[tools.bash]
timeout_ms    = 120000
max_output_kb = 512

[ui]
show_thinking = "collapsed"
show_progress = true
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
commands_dir        = ".agent-cli/commands"         # Custom slash commands (*.md)

[tools]
enabled = ["bash", "read", "write", "send_to", "monitor", "edit", "glob", "grep", "websearch", "webfetch"]

[tools.bash]
timeout_ms    = 120000
max_output_kb = 256

[tools.websearch]
api_key_env = "TAVILY_API_KEY"
endpoint    = "https://api.tavily.com/search"
provider    = "tavily"

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

## 7. Bash Tool Tuning

To allow long-running jobs or commands that produce large output, adjust `[tools.bash]`.

```toml
[tools.bash]
timeout_ms    = 600000   # 10 minutes (value is in milliseconds; default 120000 = 2 minutes)
max_output_kb = 4096     # 4 MB
```

Notes:

- `timeout_ms` is milliseconds, not seconds. `1200000` is 20 minutes, not 20 seconds.
- Processes exceeding `timeout_ms` are force-killed, and the tool result is treated as a failure.
- stdout/stderr exceeding `max_output_kb` is truncated with `...[truncated]` appended to the end.
- To prevent the AI from accidentally invoking huge commands, it is recommended to also use interactive approval (`auto_approve_tools=false`).

## 8. UI Display Mode

`ui.show_thinking` controls the display volume of thinking blocks (Claude's `thinking_delta` / Ollama's `message.thinking`). It is interpreted at `agent-cli` startup; unknown values (e.g., `"verbose"`) fall back to the default `"collapsed"`.

| Value | Behavior |
|----|------|
| `"collapsed"` (default) | Truncates each thinking delta to "first 80 characters + `...`"; if there is a newline, only the first line is shown. Displayed as a single line in the format `[thinking] <truncated>...` |
| `"expanded"` | Displays the full received thinking text in real time (`[thinking] <text>`) |
| `"hidden"` | Does not display thinking lines at all (discards `AgentEvent::Thinking` on the REPL side) |

`ui.show_progress` controls the progress indicator drawn while a turn runs: the line being executed (a tool call, cut to one terminal row with `…`) and, beneath it, a spinner with the elapsed time, replaced by `✔ <elapsed>` (or `✗ <elapsed>` after an error) when the turn ends.

| Value | Behavior |
|----|------|
| `true` (default) | The indicator is drawn, the `[tool-call]` line is shortened to a single row and a `[tool-result]` to five rows (`… +N more lines`) so the spinner stays close to them, and reasoning is shown live under the spinner instead of being streamed into the scrollback |
| `false` | Nothing is drawn; `[tool-call]` lines keep their full arguments and `[tool-result]` its full output |

`show_thinking` decides what the live reasoning block contains: with `"hidden"` there is none (and no mouse reporting is enabled), otherwise the last 10 rows are shown and a mouse click expands the block to as much as the screen can hold. Because the block replaces the inline `[thinking]` output, reasoning is no longer kept in the scrollback while the indicator is on — it is still written to the conversation log.

Shortening affects the screen only: the model receives every tool result in full, and the conversation log keeps the complete text.

The indicator additionally requires stdin **and** stderr to be interactive terminals: with piped or redirected output, and in `agent-cli serve`, it is never drawn regardless of this setting.

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

### Symptom: Bash tool reports "timed out"

- Cause: `timeout_ms` was exceeded.
- Resolution: Increase `[tools.bash] timeout_ms`, or instruct the AI to use shorter commands.

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