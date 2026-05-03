# agent-cli

`agent-cli` is a standalone Rust CLI that bundles a Claude Code-equivalent AI agent (tools / thinking / streaming REPL) into a single binary. It does not depend on tmux: each process owns exactly one agent and talks to other agents over local Unix-domain-socket IPC.

> 日本語版: [`README.md`](README.md)

## Highlights

- Standalone — no tmux required. Just run `agent-cli` (the no-arg form is equivalent to `agent-cli run`).
- Claude Code-equivalent REPL with built-in tools and thinking, implemented from scratch (does not call out to the `claude` CLI).
- Four backends: `claude` / `codex` / `ollama` / `llama.cpp`.
- Multi-agent coordination — separate processes exchange prompts via `/send <peer> <text>`.
- Persona files (YAML frontmatter + Markdown body) define role, skills, tool allow / deny lists, model, and temperature.
- Built-in tools: `shell` / `fs_read` / `fs_write` / `send_to`. Approval mode can be flipped at runtime with `/auto on`.
- Streaming responses are synchronized with the REPL prompt so a fresh `> ` is always redrawn after the response completes.
- Reliable shutdown — any of `/quit`, `/exit`, `Ctrl+D`, `Ctrl+C`, or `SIGTERM` exits within ~1 s and cleans up the IPC socket and registry metadata automatically.
- Self-diagnostics with `agent-cli doctor` and a 5-stage smoke test with `agent-cli selftest` (Provider OK / shell tool / IPC / subprocess registration / subprocess AI response).
- Configurable tool-use loop cap via `[runtime] max_tool_iterations` (default 24, max `u32::MAX`) — see "[info] max tool-use iterations reached" below.
- Ollama `message.thinking` field is decoded as `[thinking]` for thinking-capable models such as `glm-5.1:cloud`.

## Supported backends

| kind | API | Default model |
|------|-----|--------------|
| claude | Anthropic Claude (Messages, SSE) | `claude-opus-4-7` |
| codex | OpenAI Chat Completions (SSE) | `gpt-4.1` |
| ollama | Ollama `/api/chat` (NDJSON) | `glm-5.1:cloud` |
| llama.cpp | OpenAI-compatible `/v1/chat/completions` (SSE) | `default` |

The mandatory verification targets are `claude` and `ollama` (with model `glm-5.1:cloud`).

| Capability | claude | codex | ollama | llama.cpp |
|------------|--------|-------|--------|-----------|
| Streaming  | ✓ | ✓ | ✓ | ✓ |
| Tool use   | ✓ | ✓ (function calling) | ✓ (model-dependent) | ✓ (server-build dependent) |
| Thinking   | ✓ (`thinking_delta`) | ✗ | ✓ (model-dependent, `message.thinking`) | ✗ |

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

## Configuration

Config files are TOML. Resolution order:

1. `--config <path>` (explicit)
2. `AGENT_CLI_CONFIG` environment variable
3. Default `~/.config/agent-cli/config.toml`

Explicit paths must exist (no auto-creation). The default path auto-generates a sensible template on first run.

Minimum edits to get going:

```toml
[provider]
kind = "claude"  # or "codex" | "ollama" | "llama.cpp"

[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"  # name of the env var that holds the secret
model       = "claude-opus-4-7"

[provider.ollama]
base_url = "http://127.0.0.1:11434"
model    = "glm-5.1:cloud"
```

To run multiple profiles in parallel, point each instance at its own `--config` file. Share `[runtime] registry_dir` if you want them to discover each other as peers.

`agent-cli config path` prints the resolved config file currently in effect. Provider HTTP error messages also include the resolved `config` line, so when in doubt you can disambiguate `~/.local/config/...` versus `~/.config/...` mistakes immediately.

See [`doc/config.md`](doc/config.md) for the full reference and [`doc/troubleshooting.md`](doc/troubleshooting.md) for common failure modes.

## Subcommands

| Command | Purpose |
|---------|---------|
| `agent-cli run` | Start the REPL (one agent per process) |
| `agent-cli list` | List running peers |
| `agent-cli send <peer> <text>` | Send a one-shot prompt to a peer |
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
| `/help` | Show help |
| `/quit`, `/exit` | Terminate (full aliases) |

User prompts are persisted to `<runtime.log_dir>/history.txt` (last 200 entries) and reloaded on next startup. See [`doc/usage.md`](doc/usage.md) for full details.

### Skipping tool approval

Tool invocations (shell, fs_*, send_to) request a y/N approval by default. There are three ways to skip approval:

| Method | Example |
|--------|---------|
| Config file | `[runtime] auto_approve_tools = true` |
| CLI flag | `agent-cli run --auto-approve-tools` |
| REPL command | `/auto on` (`/auto off` returns to approval mode, `/auto status` shows the current value) |

In approval mode, each tool request shows `[tool approval] <tool> <args>` and `approve? [y/N]:`. Only `y` / `yes` is accepted; anything else (blank input, other words) counts as denial.

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

# Smoke test (5 stages: provider OK / shell / IPC / subprocess / subprocess AI response)
agent-cli selftest --provider claude
agent-cli selftest --provider ollama

# Semi-automated acceptance scenarios (PASS / SKIP / FAIL aggregated by env-var presence)
scripts/manual_acceptance.sh
```

Stage 1 of `selftest` requires a live backend. Stages 2–4 (shell tool, IPC roundtrip, subprocess IPC) run without external dependencies; Stage 5 needs a working provider plus child-process startup.

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
allowed_tools: [shell, fs_read]
denied_tools:  [fs_write]
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
- [`doc/providers/claude.md`](doc/providers/claude.md) / [`codex.md`](doc/providers/codex.md) / [`ollama.md`](doc/providers/ollama.md) / [`llamacpp.md`](doc/providers/llamacpp.md) — per-backend guides
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — development guide
- [`CHANGELOG.md`](CHANGELOG.md) — release notes

## License

MIT License. See [`LICENSE`](LICENSE).
