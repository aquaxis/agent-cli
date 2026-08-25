# Changelog

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format and [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.5.0]

### Added

- Project-local configuration — when neither `--config` nor `AGENT_CLI_CONFIG` is set, `agent-cli` now uses a `./.agent-cli/config.toml` under the current working directory if one exists, taking precedence over the default `~/.config/agent-cli/config.toml`. The file is used only when it already exists (never auto-generated), its path is resolved to an absolute path so a detached agent spawned from there reads the same file, and only the current directory is checked (parent directories are not walked). Docs updated: `doc/config.md` §1, `doc/usage.md`, `doc/troubleshooting.md`, `README.md`, `README_ja.md`.

## [0.4.0]

### Added

- Detached agent creation — a running (or one-shot) `agent-cli` can now **create** an `agent-cli` process that does not depend on the parent: it runs headless in its own session, self-registers as a peer, and outlives the launcher.
  - New subcommands: `agent-cli spawn [...]` launches a detached peer (same options as `run`) and returns once it has registered; `agent-cli serve [...]` runs headless (register + serve peers over IPC, no interactive REPL — the target `spawn` launches; usable directly for a foreground headless agent); `agent-cli stop <peer>` requests a graceful shutdown.
  - The detached child is launched with a **double fork + `setsid`**: the intermediate process exits (reaped by the launcher) and the `serve` process is reparented to init, so it survives the launcher's exit, `Ctrl+C`, and terminal hang-up, and never lingers as a zombie. It shares the launcher's config file (hence the same `[runtime] registry_dir`), so it is immediately reachable via `list` / `send` / `ask` / the `send_to` tool.
  - New REPL commands `/spawn [name] [provider]` and `/stop <peer>`.
  - New **opt-in** LLM tool `spawn` (registered as a candidate but not in the default `[tools] enabled`, since autonomous process creation is higher-impact than peer messaging) that lets the model create a detached peer, with an optional fire-and-forget initial prompt.
  - New IPC message `IpcMessage::Shutdown` (acknowledged by the server), used by `stop` / `/stop`, with a `SIGTERM`-by-pid fallback. A headless agent has no console to answer a y/N prompt, so it auto-approves its own tool execution.
  - Docs updated: `doc/usage.md` (Detached agents), `doc/tools.md` (opt-in `spawn` tool), `doc/architecture.md` (§7.1), `README.md`, `README_ja.md`.
- `Tab` completes the slash command being typed in the REPL (`app.rs::command_completion`). A single match completes it and appends a space so an argument can follow (`/sen` → `/send `); several matches extend the line as far as the candidates agree (`/rel` → `/reload-`). Nothing to add, no match, or an argument already started leaves the line untouched. Built-in and custom commands complete alike.

### Changed

- The live command-candidate list is now drawn on the row **above** the prompt instead of below it, so the line being typed stays where the eye already is. The hint row is part of the tracked render block and is cleared with it.

### Fixed

- The `--provider` help text (`agent-cli --help` / `run --help`) now lists all seven backend kinds, including the previously-omitted `claude-code`.

## [0.3.0]

### Added

- New backend `kind = "claude-code"` (`src/ai/claude_code.rs`): drives the locally installed Claude Code CLI (`claude`) as a child process, so `agent-cli` wraps it with its REPL, personas, peer IPC, conversation logging, and custom slash commands. It needs no API key — Claude Code's own authentication is used. Not to be confused with the existing `claude` backend, which is a direct client of the Anthropic Messages API.
  - Two modes. `mode = "delegation"` (default) lets Claude Code run its own tools, apply its own permission mode, and keep its own session; its `tool_use` blocks are deliberately **not** mapped to `ProviderEvent::ToolUse`, because the tool has already run by the time it is reported and converting it would execute every command twice. `mode = "gateway"` passes `--tools ""` and is chat-only: `claude -p` accepts no external tool definitions, so agent-cli's own tools cannot be offered to it either.
  - Two transports. `transport = "stream"` (default) uses `--output-format stream-json --verbose --include-partial-messages` for token-level streaming; `transport = "oneshot"` uses `--output-format json`. Only `delegation` + `stream` + `session = "persistent"` keeps a resident child process; every other combination spawns one child per turn.
  - New `[provider.claude-code]` keys, all optional: `bin`, `model`, `mode`, `transport`, `session`, `tools`, `allowed_tools`, `disallowed_tools`, `permission_mode`, `turn_timeout_secs`, `max_budget_usd`, `system_prompt_mode`, `extra_args`. Unknown values for the enumerated keys are rejected at startup with the accepted set listed.
  - Per-turn timeout kills a stuck child and leaves the provider usable; children are spawned with `kill_on_drop`, so no orphan survives an `agent-cli` exit.
  - `agent-cli providers` and `agent-cli doctor` report the resolved `claude` binary (and its `--version`) instead of an API-key status for this backend.
  - New guide `doc/providers/claude-code.md`; `doc/config.md`, `doc/architecture.md` (§8.1), `README.md`, `README_ja.md`, and `example/config.example.toml` updated.

### Changed

- `README.md` / `README_ja.md`: the highlight stating that agent-cli "does not call out to the `claude` CLI" now says that this holds for the REPL and built-in tools, since driving that CLI is available as the opt-in `claude-code` backend.

### Added

- The `[ui] show_thinking` setting now actually controls thinking display in the REPL (FR-03-1-2 follow-up, T-512). Previously the setting was defined but not consumed by `display_event`. Three values are implemented: `"hidden"` (suppress entirely) / `"collapsed"` (default: truncate each delta to "first 80 chars + first line") / `"expanded"` (full text, previous behavior). Unknown values fall back to `"collapsed"`. Recommended `"hidden"` for long-reasoning models like `glm-5.1:cloud` that fill the screen with thinking output.
- The Ollama parser now emits `message.thinking` fields as `ProviderEvent::Thinking` (FR-03-1-2, T-511). Thinking-capable models like `glm-5.1:cloud` display `[thinking] ...` in the REPL. Emission order is `Thinking` → `Text` → `ToolUse` (consistent with Anthropic convention). `Capabilities::thinking` is now set to `true`.
- Added `[runtime] max_tool_iterations` config key (FR-04-3, T-510/T-510-2). Configurable per-turn tool_use iteration cap. Minimum 1 (`0` and negative values are clamped to `1` internally), maximum `u32::MAX = 4,294,967,295`. See `doc/config.md` section `[runtime]` for configuration method, recommended ranges, and boundary behavior.

### Changed

- Tool-use loop cap changed from 8 (hardcoded) to `[runtime] max_tool_iterations` (default 24) (FR-04-3). The default was raised so that design-then-debug orchestrators (AI generates design artifacts → verification tool → lint fix → final fs_write) fit within a single turn.

### Added

- Initial release skeleton implementation:
  - Standalone Rust CLI (`agent-cli`)
  - REPL + tools + thinking display (Claude Code-equivalent)
  - Four backends: `claude` / `codex` / `ollama` / `llama.cpp`
    - Each backend's stream parser extracted as a pure function, unit-tested with mock input
    - Persona `model` / `temperature` reflected in request body
  - Built-in tools: `shell` / `fs_read` / `fs_write` / `send_to`
  - Inter-agent messaging (Unix domain sockets, JSON Lines)
  - Registry (`<registry_dir>/<agent-id>.{sock,json}`, PID liveness check, stale cleanup)
  - Agent persona files (YAML frontmatter + Markdown body)
    - Resolution order: `--persona` → `[runtime] persona_file` → `<agents_dir>/<name>.md` → built-in default
    - REPL commands: `/persona` / `/reload-persona` / `/peer <id>` / `/tools`
  - Config file `~/.config/agent-cli/config.toml`, individual override via `--config` / `AGENT_CLI_CONFIG`
  - Self-diagnostics `agent-cli doctor`
  - Smoke test `agent-cli selftest` (5 stages)
    - Stage 1: Provider "OK" round-trip
    - Stage 2: Shell tool direct execution
    - Stage 3: IPC round-trip
    - Stage 4: Subprocess startup with registry registration + Ping/Pong + Prompt/Ack
    - Stage 5: Peer prompt to subprocess → AI response → conversation log write confirmation
  - One-liner installer `install.sh`
  - Sample personas: `example/agents/{coder,reviewer,planner}.md`
  - Documentation: `README.md` / `README.en.md` / `doc/` directory / `CONTRIBUTING.md` / `CHANGELOG.md` / `LICENSE`
  - GitHub Actions CI (`.github/workflows/ci.yml`): fmt / clippy / build / test / doc / selftest
  - Input history persistence (`<log_dir>/history.txt`, last 200 entries) and REPL `/history [n]` command
  - `agent-cli list` column-aligned output
  - Semi-automated acceptance test script `scripts/manual_acceptance.sh`
    - Supports mandatory A (claude) / B (ollama) and optional D1 (codex) / D2 (llama.cpp)
    - Auto-determines SKIP based on API key / local server availability

### Verification

- `cargo build` with zero warnings
- `cargo clippy --all-targets -- -D warnings` passes
- `cargo fmt --all -- --check` passes
- `cargo test` all 74 tests pass (Provider parsers, Agent loop E2E, IPC, personas, doc consistency, CLI consistency, Ollama thinking, `max_tool_iterations` boundary values)
- `cargo doc --no-deps` with zero warnings

[Unreleased]: https://github.com/aquaxis/agent-cli/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/aquaxis/agent-cli/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/aquaxis/agent-cli/releases/tag/v0.3.0