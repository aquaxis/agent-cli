# Changelog

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format and [Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/aquaxis/agent-cli/compare/HEAD...HEAD