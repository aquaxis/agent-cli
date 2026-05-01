# agent-cli

`agent-cli` is a standalone Rust CLI that bundles a Claude Code-equivalent AI agent (tools / thinking / streaming REPL) into a single binary. It does not depend on tmux: each process owns exactly one agent and talks to other agents over local Unix-domain-socket IPC.

> 日本語版は [`README.md`](README.md) を参照してください。

## Highlights

- Standalone — no tmux required, just run `agent-cli`
- Claude Code-equivalent REPL with built-in tools and thinking, implemented from scratch (does not call out to the `claude` CLI)
- Four backends: `claude` / `codex` / `ollama` / `llama.cpp`
- Multi-agent coordination — separate processes exchange prompts via `/send <peer> <text>`
- Persona files (YAML frontmatter + Markdown body) define role, skills, and tool allow / deny lists
- Built-in tools: `shell` / `fs_read` / `fs_write` / `send_to`
- Self-diagnostics with `agent-cli doctor` and a 4-stage smoke test with `agent-cli selftest`

## Supported backends

| kind | API | Default model |
|------|-----|--------------|
| claude | Anthropic Claude | `claude-opus-4-7` |
| codex | OpenAI Chat Completions | `gpt-4.1` |
| ollama | Ollama `/api/chat` | `glm-5.1:cloud` |
| llama.cpp | OpenAI-compatible `/v1/chat/completions` | `default` |

The mandatory verification targets are `claude` and `ollama` (with model `glm-5.1:cloud`).

## Install

### One-liner

```bash
curl -fsSL https://raw.githubusercontent.com/example/agent-cli/main/install.sh | sh
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
git clone https://github.com/example/agent-cli.git
cd agent-cli
cargo install --path . --root "$HOME/.local"
```

## Quick start

```bash
# 1. Default config is created on first run.
agent-cli config path
# => ~/.config/agent-cli/config.toml

# 2. Configure your provider. Example: Claude.
export ANTHROPIC_API_KEY=sk-ant-...

# 3. Start the REPL.
agent-cli run --provider claude

# 4. In another terminal, start a second agent on Ollama.
agent-cli run --provider ollama --model glm-5.1:cloud --name bob

# 5. From the first session, send a prompt across.
> /list
> /send bob "hello from claude side"
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

See [`doc/config.md`](doc/config.md) for the full reference.

## Subcommands

| Command | Purpose |
|---------|---------|
| `agent-cli run` | Start the REPL (one agent per process) |
| `agent-cli list` | List running peers |
| `agent-cli send <peer> <text>` | Send a one-shot prompt to a peer |
| `agent-cli providers` | Show backend status |
| `agent-cli doctor` | Sanity-check config / API keys / connectivity / registry / `bash` |
| `agent-cli selftest [--provider <kind>]` | Smoke test in 4 stages |
| `agent-cli config show` | Print current config |
| `agent-cli config edit` | Open config in `$EDITOR` |
| `agent-cli config path` | Print resolved config path |

REPL commands include `/list`, `/send <peer> <text>`, `/tools`, `/persona`, `/reload-persona`, `/peer <id>`, `/history [n]`, `/cancel`, `/help`, `/quit`.

User prompts are persisted to `<runtime.log_dir>/history.txt` (last 200 entries) and reloaded on next startup.

## Verification

```bash
cargo test
agent-cli doctor
agent-cli selftest --provider claude
agent-cli selftest --provider ollama
```

Stage 1 of `selftest` requires a live backend; stages 2–4 (shell tool, IPC roundtrip, subprocess IPC) run with no external dependencies.

## Personas

Drop a Markdown file under `~/.config/agent-cli/agents/<name>.md` to define a persona. Examples are in [`example/agents/`](example/agents/).

```bash
cp example/agents/reviewer.md ~/.config/agent-cli/agents/alice.md
agent-cli run --name alice
```

## Documentation

- [`doc/usage.md`](doc/usage.md) — CLI / REPL details
- [`doc/config.md`](doc/config.md) — full configuration reference
- [`doc/tools.md`](doc/tools.md) — built-in tools
- [`doc/architecture.md`](doc/architecture.md) — architecture overview
- [`doc/troubleshooting.md`](doc/troubleshooting.md) — known issues & fixes
- [`doc/providers/`](doc/providers/) — per-backend guides
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — development guide
- [`CHANGELOG.md`](CHANGELOG.md) — release notes

## License

MIT License. See [`LICENSE`](LICENSE).
