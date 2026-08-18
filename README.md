# agent-cli

`agent-cli` is a standalone Rust CLI that bundles a Claude Code-equivalent AI agent (tools / thinking / streaming REPL) into a single binary. It does not depend on tmux: each process owns exactly one agent and talks to other agents over local Unix-domain-socket IPC.

> 日本語版は [`README_ja.md`](README_ja.md) を参照してください. (The main README is in English; `README_ja.md` is the maintained Japanese translation.)

## Highlights

- Standalone — no tmux required. Just run `agent-cli` (the no-arg form is equivalent to `agent-cli run`).
- Claude Code-equivalent REPL with built-in tools and thinking, implemented from scratch — the REPL and tools never call out to the `claude` CLI. (Driving that CLI is available separately, as the opt-in `claude-code` backend below.)
- Seven backends: `claude` / `claude-code` / `codex` / `ollama` / `opencode` / `opencode-go` / `llama.cpp`.
- Multi-agent coordination — separate processes exchange prompts via `/send <peer> <text>`.
- Persona files (YAML frontmatter + Markdown body) define role, skills, tool allow / deny lists, model, and temperature.
- Built-in tools: `bash` / `read` / `write` / `send_to` / `edit` / `glob` / `grep` / `monitor` / `websearch` / `webfetch`. Approval mode can be flipped at runtime with `/auto on`.
- Custom slash commands — drop a Markdown file into `.agent-cli/commands/` and it becomes `/<name>`, with `$ARGUMENTS` / `$1`…`$N` / `@file` expansion and prefix auto-execution.
- Line editing at the prompt — `↑` / `↓` history browsing, `Ctrl+A` / `Ctrl+E`, `Esc` to clear, live command candidates shown above the prompt, and `Tab` completion for `/` commands.
- Scriptable — pipe a question straight into `agent-cli run`, or query a running agent with `agent-cli ask <peer> <text>` and get just the answer on stdout.
- Streaming responses are synchronized with the REPL prompt so a fresh `> ` is always redrawn after the response completes.
- Reliable shutdown — any of `/quit`, `/exit`, `Ctrl+D`, `Ctrl+C`, or `SIGTERM` exits within ~1 s and cleans up the IPC socket and registry metadata automatically.
- Self-diagnostics with `agent-cli doctor` and a 5-stage smoke test with `agent-cli selftest` (Provider OK / bash tool / IPC / subprocess registration / subprocess AI response).
- Configurable tool-use loop cap via `[runtime] max_tool_iterations` (default 24, max `u32::MAX`) — see "[info] max tool-use iterations reached" below.
- Ollama `message.thinking` field is decoded as `[thinking]` for thinking-capable models such as `glm-5.1:cloud`.
- Opt-in context-efficiency features (all default OFF): Claude prompt caching, opencode local persistent session, and hybrid history-window management (summarize-then-drop). See [`doc/config.md`](doc/config.md) §11.

## Supported backends

| kind | API | Default model |
|------|-----|--------------|
| claude | Anthropic Claude (Messages, SSE) | `claude-opus-4-7` |
| claude-code | Local Claude Code CLI as a child process (no API key) | Claude Code's own |
| codex | OpenAI Chat Completions (SSE) | `gpt-4.1` |
| ollama | Ollama `/api/chat` (NDJSON) | `glm-5.1:cloud` |
| opencode | OpenCode — dual mode (see below) | `claude-sonnet-4-5` |
| opencode-go | OpenCode Go cloud (auto-configured shortcut) | `claude-sonnet-4-5` |
| llama.cpp | OpenAI-compatible `/v1/chat/completions` (SSE) | `default` |

`opencode` selects its mode by API-key presence:

- **No key → local mode.** Talks to a running `opencode serve` over its native
  session API: `POST /session` → `POST /session/:id/message` (synchronous JSON).
  Default `base_url` `http://127.0.0.1:4096`.
- **Key set → cloud mode (OpenCode Zen).** Default cloud `base_url`
  `https://opencode.ai/zen/v1`, key env `OPENCODE_API_KEY`,
  `Authorization: Bearer`. The wire format is selectable via
  `[provider.opencode] api`: `"openai"` (default) →
  `POST {base_url}/chat/completions` (SSE, `[DONE]`); `"anthropic"` →
  `POST {base_url}/messages` (Anthropic SSE). Pair with the matching
  `base_url`, e.g. the "go" endpoints `https://opencode.ai/zen/go/v1`. See
  [`doc/providers/opencode.md`](doc/providers/opencode.md).

**`opencode-go`** is a convenience alias for OpenCode with Go-specific defaults
already filled in. Set `kind = "opencode-go"` and `api_key_env` only — the
`base_url` (`https://opencode.ai/zen/go/v1`), `api` (`"anthropic"`), and
`model` (`claude-sonnet-4-5`) are auto-populated. You can still override any
field in `[provider.opencode]`. See the configuration example below.

**`claude-code`** runs the Claude Code CLI installed on the machine and adapts
it to the provider interface, so `agent-cli` wraps it with personas, peer IPC,
logging, and custom slash commands. It needs no API key — Claude Code brings its
own authentication:

```toml
[provider]
kind = "claude-code"

[provider.claude-code]
mode      = "delegation"   # Claude Code runs its own tools (default)
transport = "stream"       # token-level streaming (default)
```

Two consequences are worth knowing up front: in the default `delegation` mode
Claude Code executes its own tools, so agent-cli's tool registry and approval
prompt are not involved; and in `gateway` mode (`--tools ""`) nothing executes
at all, because `claude -p` accepts no external tool definitions. See
[`doc/providers/claude-code.md`](doc/providers/claude-code.md).

The mandatory verification targets are `claude` and `ollama` (with model `glm-5.1:cloud`).

| Capability | claude | claude-code | codex | ollama | opencode | llama.cpp |
|------------|--------|-------------|-------|--------|----------|-----------|
| Streaming  | ✓ | ✓ (`transport = "stream"`) | ✓ | ✓ | ✓ (cloud SSE; local buffered) | ✓ |
| Tool use   | ✓ | ✓ delegation mode — run **inside** Claude Code | ✓ (function calling) | ✓ (model-dependent) | ✓ cloud / ✗ local (v1) | ✓ (server-build dependent) |
| Thinking   | ✓ (`thinking_delta`) | ✓ (`transport = "stream"`) | ✗ | ✓ (model-dependent, `message.thinking`) | ✗ | ✗ |

`opencode-go` has the same capabilities as `opencode` (cloud mode); it is a config shortcut, not a separate backend.

## Install

### One-liner

```bash
curl -fsSL https://raw.githubusercontent.com/aquaxis/agent-cli/main/install.sh | sh
```

### What `install.sh` does

- Targets Linux (x86_64 / aarch64). Other platforms exit early with an error.
- Default install prefix: `$HOME/.local/bin/agent-cli`.
- If invoked from inside an `agent-cli` repository it builds local sources, otherwise it `git clone`s `AGENT_CLI_REPO` and builds.
- Existing binaries are overwritten. Your `~/.config/agent-cli/config.toml` is left alone.

| Variable | Default | Purpose |
|----------|---------|---------|
| `AGENT_CLI_REPO` | GitHub source repo | Clone source |
| `AGENT_CLI_REF` | `main` | Branch / tag / commit |
| `AGENT_CLI_PREFIX` | `$HOME/.local` | Install prefix |
| `AGENT_CLI_INSTALL_FORCE` | (unset) | Set to `1` to silence the overwrite notice |

### Build from source

```bash
git clone https://github.com/aquaxis/agent-cli.git
cd agent-cli
cargo install --path . --root "$HOME/.local"
```

## Quick start

```bash
# 1. Default config is created on first run.
agent-cli config path
# => ~/.config/agent-cli/config.toml

# 2. Set the API key for your backend (Claude example).
export ANTHROPIC_API_KEY=sk-ant-...

# 3. Start the REPL (the no-arg form is equivalent to `agent-cli run`).
agent-cli                       # uses provider.kind from config
# or
agent-cli run --provider claude # override at the command line

# 4. In another terminal, start a second agent on Ollama.
agent-cli run --provider ollama --model glm-5.1:cloud --name bob

# 5. From the first session, send a prompt across.
> /list
> /send bob "hello from claude side"

# 6. Exit the REPL.
> /quit       # or /exit, Ctrl+D, Ctrl+C — all of them work
```

No terminal session required — a question can be answered straight from the command line:

```bash
# Pipe the question in; the answer is printed and the process exits on EOF.
echo "Explain Rust ownership in three lines" | agent-cli run

# Let the agent use tools while unattended.
echo "Count the .rs files under src with bash" | agent-cli run --auto-approve-tools

# Or ask an already-running agent and get only the response text back.
agent-cli ask bob "Summarize the current design risks"
```

See [`doc/usage.md`](doc/usage.md) "Non-interactive / Scripted Use" for the rules that apply (one line per prompt, approval handling, output composition).

## Configuration

Config files are TOML. Resolution order:

1. `--config <path>` (explicit)
2. `AGENT_CLI_CONFIG` environment variable
3. Default `~/.config/agent-cli/config.toml`

Explicit paths must exist (no auto-creation). The default path auto-generates a sensible template on first run, and [`example/config.example.toml`](example/config.example.toml) is a fully commented starting point covering every section.

`[provider] kind` selects the active backend; only that backend's `[provider.*]` table needs to be filled in, but you can keep several tables in one file and switch with `kind` (or `--provider`).

### Per-backend configuration examples

**claude** — Anthropic Claude (Messages, SSE):

```toml
[provider]
kind = "claude"

[provider.claude]
api_key_env  = "ANTHROPIC_API_KEY"  # name of the env var that holds the secret
model        = "claude-opus-4-7"
base_url     = "https://api.anthropic.com"   # usually leave as-is
thinking     = true                          # enable thinking blocks
# prompt_cache = true                         # opt-in Anthropic prompt caching
```

**claude-code** — the locally installed Claude Code CLI, driven as a child process. No API key; it uses Claude Code's own authentication. Every key below is optional:

```toml
[provider]
kind = "claude-code"

[provider.claude-code]
bin       = "claude"       # executable name (resolved via PATH) or full path
model     = "sonnet"       # --model; omit for Claude Code's own default
mode      = "delegation"   # "delegation" (its own tools) | "gateway" (chat only)
transport = "stream"       # "stream" (token streaming) | "oneshot"
session   = "persistent"   # "persistent" | "ephemeral" (delegation only)
turn_timeout_secs = 900
# tools           = ["Bash", "Read"]
# permission_mode = "auto"
# max_budget_usd  = 1.0
```

**codex** — OpenAI Chat Completions (SSE, function calling). `kind = "codex"` is the internal name; it is not OpenAI's legacy Codex model. `base_url` also works with OpenAI-compatible gateways / Azure OpenAI:

```toml
[provider]
kind = "codex"

[provider.codex]
api_key_env = "OPENAI_API_KEY"
model       = "gpt-4.1"
base_url    = "https://api.openai.com/v1"
```

**ollama** — local or cloud Ollama `/api/chat` (NDJSON). No API key required:

```toml
[provider]
kind = "ollama"

[provider.ollama]
model    = "glm-5.1:cloud"
base_url = "http://127.0.0.1:11434"
```

**opencode** — local mode talks to a running `opencode serve` (no key); a resolved API key automatically switches to cloud mode (OpenCode Zen):

```toml
[provider]
kind = "opencode"

# Local mode (default): a running `opencode serve`, no key needed.
[provider.opencode]
base_url = "http://127.0.0.1:4096"
model    = "claude-sonnet-4-5"
# persistent_session = true   # opt-in (local only): reuse one server session

# Cloud mode (OpenCode Zen): set api_key_env — its presence switches to cloud.
# base_url    = "https://opencode.ai/zen/v1"
# api_key_env = "OPENCODE_API_KEY"
# api         = "anthropic"   # cloud wire format: "openai" (default) | "anthropic";
#                             # pair with the matching base_url, e.g. .../zen/go/v1
```

**opencode-go** — convenience kind for OpenCode Go cloud. Auto-fills `base_url`, `api`, and `model`:

```toml
[provider]
kind = "opencode-go"

[provider.opencode]
api_key_env = "OPENCODE_API_KEY"   # only required field
# base_url, api, and model are auto-populated; override in [provider.opencode] if needed
```

**llama.cpp** — OpenAI-compatible `/v1/chat/completions` of a `llama-server`. Quote `"llama.cpp"` because the TOML key contains a dot. Sampling knobs mirror the `llama-cli` flags and are all optional (omit any → the server's own default):

```toml
[provider]
kind = "llama.cpp"

[provider."llama.cpp"]
model    = "default"
base_url = "http://127.0.0.1:8080"
# api_key_env = "LLAMACPP_API_KEY"   # optional; only for Bearer-auth builds
# max_tokens     = 1024   # -n / --n-predict : max tokens to generate
# temperature    = 0.2    # --temp
# top_k          = 80     # --top-k
# top_p          = 0.95   # --top-p
# min_p          = 0.05   # --min-p
# repeat_penalty = 1.05   # --repeat-penalty
# repeat_last_n  = 64     # --repeat-last-n
# seed           = 0      # --seed
```

### Opt-in context-efficiency features

All default OFF; with every flag off, request bodies and history handling are byte-for-byte unchanged. See [`doc/config.md`](doc/config.md) §11.

```toml
[provider.claude]
prompt_cache = true              # Anthropic prompt caching (system + tools + tail)

[provider.opencode]
persistent_session = true        # reuse one local OpenCode session across turns

[history]
enabled            = true        # summarize-then-drop old turns when over budget
max_context_tokens = 24000
keep_recent_turns  = 6
```

To run multiple profiles in parallel, point each instance at its own `--config` file. Share `[runtime] registry_dir` if you want them to discover each other as peers.

`agent-cli config path` prints the resolved config file currently in effect. Provider HTTP error messages also include the resolved `config` line, so when in doubt you can disambiguate `~/.local/config/...` versus `~/.config/...` mistakes immediately.

See [`doc/config.md`](doc/config.md) for the full reference and [`doc/troubleshooting.md`](doc/troubleshooting.md) for common failure modes.

## Subcommands

| Command | Purpose |
|---------|---------|
| `agent-cli run` | Start the REPL (one agent per process) |
| `agent-cli list` | List running peers |
| `agent-cli send <peer> <text>` | Send a one-shot prompt to a peer (no response) |
| `agent-cli ask <peer> <text> [--timeout <secs>]` | Send a prompt to a peer, wait for the answer, print it (default 120 s) |
| `agent-cli providers` | Show backend status |
| `agent-cli doctor` | Sanity-check config / API keys / connectivity / registry / `bash` |
| `agent-cli selftest [--provider <kind>]` | Smoke test in 5 stages |
| `agent-cli config show` | Print current config |
| `agent-cli config edit` | Open config in `$EDITOR` |
| `agent-cli config path` | Print resolved config path |

REPL commands inside `agent-cli run`:

| Command | Purpose |
|---------|---------|
| `/list` | List running peers |
| `/send <peer> <text>` | Send a prompt to a peer |
| `/tools` | List tools enabled for this agent |
| `/persona` | Show this agent's persona (role / skills / source path) |
| `/reload-persona` | Re-resolve and reload the persona file (history is preserved) |
| `/peer <id_or_name>` | Show a peer's persona summary |
| `/history [n]` | Show last n (default 20) user inputs |
| `/clear`, `/reset` | Clear conversation history (persona / system prompt are kept) |
| `/cancel` | Request cancel of the in-flight AI response or tool call |
| `/auto [on\|off\|status]` | Toggle tool-approval skip at runtime |
| `/commands` | List custom slash commands (name, first line, file path) |
| `/reload-commands` | Re-scan the custom commands directory |
| `/help` | Show help |
| `/quit`, `/exit` | Terminate (full aliases) |

User prompts and executed slash commands are persisted to `<runtime.log_dir>/history.txt` (last 200 entries) and reloaded on next startup; `/quit` and `/exit` are excluded. See [`doc/usage.md`](doc/usage.md) for full details.

### Skipping tool approval

Tool invocations (bash, read, write, send_to, monitor, edit, glob, grep, websearch, webfetch) request a y/N approval by default. There are three ways to skip approval:

| Method | Example |
|--------|---------|
| Config file | `[runtime] auto_approve_tools = true` |
| CLI flag | `agent-cli run --auto-approve-tools` |
| REPL command | `/auto on` (`/auto off` returns to approval mode, `/auto status` shows the current value) |

In approval mode, each tool request shows `[tool approval] <tool> <args>` and `approve? [y/N]:`. Only `y` / `yes` is accepted; anything else (blank input, other words) counts as denial.

This governs the tools `agent-cli` runs itself. With `kind = "claude-code"` in
`delegation` mode the tools run inside Claude Code, so none of the three methods
apply — use that backend's `permission_mode` / `tools` / `allowed_tools` /
`disallowed_tools` keys instead.

### Custom slash commands

Every `*.md` file in `.agent-cli/commands/` becomes a slash command named after the file stem, so `.agent-cli/commands/review.md` defines `/review`. Running it expands the file and submits the result to the agent as a user prompt.

```markdown
<!-- .agent-cli/commands/review.md -->
Review the following file and list the three most severe issues.

Target: $1
Focus: $ARGUMENTS

@doc/tools.md
```

```text
> /review src/agent.rs security
```

| Placeholder | Expands to |
|-------------|-----------|
| `$ARGUMENTS` | The whole argument string after the command name |
| `$1`, `$2`, … | The Nth whitespace-separated argument; absent ones become empty |
| `@<path>` | The file's contents (`[error: cannot read @<path>]` if unreadable) |

- Built-in commands take precedence — a custom `help.md` never shadows `/help`.
- A prefix matching exactly one custom command runs it and prints `[auto] /<typed> → /<resolved>`; several matches list the candidates instead.
- `/commands` lists what is loaded, `/reload-commands` re-scans without restarting.
- The directory is `[runtime] commands_dir` (default `.agent-cli/commands`); a missing directory is not an error.

See [`doc/usage.md`](doc/usage.md) "Custom Slash Commands" for the full reference.

### REPL input editing

With a terminal attached, the prompt supports in-place editing and history browsing:

| Key | Action |
|-----|--------|
| `↑` / `↓` | Browse history (the in-progress draft is restored when you come back past the newest entry) |
| `Ctrl+A` / `Home`, `Ctrl+E` / `End` | Jump to start / end of the line |
| `Esc` | Leave history browsing, or clear the line |
| `Ctrl+C` | Clear the line; exit when the line is empty |
| `Ctrl+D` | Exit on an empty line |

While the line starts with `/` and has no space, the best-matching command is suggested inline. Raw mode needs a TTY; with piped input the REPL falls back to plain line reading — tools, custom commands, and peer messaging all keep working.

### Suppressing `[thinking]` output

Long-reasoning models such as `glm-5.1:cloud` emit large amounts of thinking text, which can fill the REPL with `[thinking] ...` lines. Use `[ui] show_thinking` to control the display volume:

```toml
[ui]
show_thinking = "hidden"     # suppress entirely
# show_thinking = "collapsed"  # default: first 80 chars + "..." on one line
# show_thinking = "expanded"   # full text
```

| Value | Behavior |
|-------|----------|
| `"hidden"` | `[thinking]` is never printed |
| `"collapsed"` (default) | Each thinking delta is truncated to "first 80 chars + `...`"; if multi-line, only the first line is shown |
| `"expanded"` | Full text printed verbatim |

Changes take effect on next `agent-cli` start. See [`doc/config.md`](doc/config.md) "UI display modes" for details.

### `[info] max tool-use iterations reached`

This message appears in the REPL when the AI keeps emitting `tool_use` requests round after round and reaches the per-turn iteration cap without producing a final text answer. It is a guard against runaway loops.

- **Not an error** — `[info]` prefix, not `[error]`. It is not written to error logs and does not trigger monitoring alerts.
- The next `> ` prompt is redrawn immediately and conversation history is preserved.
- **Can it be changed via config?** Yes. Edit `[runtime] max_tool_iterations` in `~/.config/agent-cli/config.toml` and restart `agent-cli` (default `24`).
- **Can it be set to "unlimited"?** Strictly no (a true uncapped mode is intentionally not provided to prevent runaway billing / GPU / stdout). The type is `u32`, so the practical maximum is `u32::MAX = 4,294,967,295` — effectively unlimited for any real workflow.
- Recommended ranges: simple chat 4–8, design-then-debug orchestrators 24–48, long-running autonomous experiments 64–256.
- Workarounds: split the prompt, give a more concrete goal, use `denied_tools` in the persona to remove unrelated tools, run `/clear` and retry, or raise `max_tool_iterations`. See [`doc/troubleshooting.md`](doc/troubleshooting.md) and [`doc/config.md`](doc/config.md).

### Termination

Any of the following terminates the process within ~1 s and removes the IPC socket (`<registry_dir>/<agent-id>.sock`) and registry metadata (`<registry_dir>/<agent-id>.json`). It works even mid-stream or while a tool is running.

| Method | Action |
|--------|--------|
| REPL command | `/quit` or `/exit` |
| EOF | `Ctrl+D` (stdin close) |
| Signal | `Ctrl+C` (SIGINT) or `kill <pid>` (SIGTERM) |

## Verification

```bash
# Automated test suite
cargo test

# Format / lint
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings

# Self-diagnostics
agent-cli doctor

# Smoke test (5 stages: provider OK / bash / IPC / subprocess / subprocess AI response)
agent-cli selftest --provider claude
agent-cli selftest --provider ollama

# Semi-automated acceptance scenarios (PASS / SKIP / FAIL aggregated by env-var presence)
scripts/manual_acceptance.sh
```

Stage 1 of `selftest` requires a live backend. Stages 2–4 (bash tool, IPC roundtrip, subprocess IPC) run without external dependencies; Stage 5 needs a working provider plus child-process startup.

## Personas

A persona file (Markdown with YAML frontmatter) defines the agent's role, skills, description, allowed / denied tools, model, and temperature. Examples ship under [`example/agents/`](example/agents/).

```bash
mkdir -p ~/.config/agent-cli/agents
cp example/agents/reviewer.md ~/.config/agent-cli/agents/alice.md
agent-cli run --name alice
# → <agents_dir>/alice.md is auto-loaded
```

Resolution order: **`--persona <path>` → `[runtime] persona_file` → `<agents_dir>/<name>.md` → built-in default.**

Minimal example:

```markdown
---
name: alice
role: code reviewer
skills: [Rust, security]
allowed_tools: [bash, read]
denied_tools:  [write]
---

You are a senior reviewer. Always propose minimal-diff fixes.
```

Full frontmatter reference, validation rules, and operational scenarios are in [`doc/personas.md`](doc/personas.md).

## Documentation

- [`doc/usage.md`](doc/usage.md) — CLI and REPL command reference
- [`doc/config.md`](doc/config.md) — full configuration reference (most detailed)
- [`doc/personas.md`](doc/personas.md) — persona reference (all frontmatter keys, operational scenarios)
- [`doc/tools.md`](doc/tools.md) — built-in tool specifications
- [`doc/architecture.md`](doc/architecture.md) — architecture overview
- [`doc/troubleshooting.md`](doc/troubleshooting.md) — known failures and fixes
- [`doc/providers/claude.md`](doc/providers/claude.md) / [`claude-code.md`](doc/providers/claude-code.md) / [`codex.md`](doc/providers/codex.md) / [`ollama.md`](doc/providers/ollama.md) / [`opencode.md`](doc/providers/opencode.md) / [`llamacpp.md`](doc/providers/llamacpp.md) — per-backend guides
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — development guide
- [`CHANGELOG.md`](CHANGELOG.md) — release notes

## License

MIT License. See [`LICENSE.md`](LICENSE.md).
