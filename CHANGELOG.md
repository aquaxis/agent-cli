# Changelog

This project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format and [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Mouse-wheel scrollback — turning the wheel scrolls the session log while the **prompt line stays where it is**.
  - The pinned prompt keeps the text you had typed and the cursor where it was, and stays editable while you read: typing, `Backspace`, the arrows and history navigation all redraw it in place. Scrolling back to the bottom returns to the live view, and so do `Esc` and submitting the line with `Enter`.
  - It works during a turn as well: output arriving meanwhile is kept but does not move the view, and the spinner with its elapsed time is pinned at the bottom of the screen. The wheel was previously swallowed while a turn ran, since mouse reporting was on only for the clickable reasoning block — clicking it still toggles that block on the live screen.
  - The keyboard is unchanged: `↑` / `↓` stay on the input history, the arrows stay in the line being edited, and `Esc` / `Ctrl+C` still cancel a turn once the view is back at the bottom (the first `Esc` returns from the scrolled view).
  - New `[ui] mouse_scroll` (default `true`) and `[ui] scrollback_lines` (default `2000`). Either `mouse_scroll = false` or `scrollback_lines = 0` disables it completely, and the output is then byte-identical to v0.13.0; the feature also requires stdin and stderr to be interactive terminals, so piped output and `agent-cli serve` are untouched.
  - While it is on the terminal reports mouse events to agent-cli, so selecting text — and the terminal's own scrollback — need the usual `Shift` override.
  - The scrolled view is drawn on the terminal's alternate screen, so returning to the live view restores the screen, and the terminal's own scrollback, exactly as they were.
  - Internally: a new `scroll.rs` module keeps the session transcript (recorded at the four `raw_*` writers, bounded, excluding anything the display erases) and the pure viewport arithmetic, including an SGR-aware wrap so a styled line can be re-wrapped without breaking a colour. No new dependency; Linux-only as before.
  - Docs updated: `README.md`, `README_ja.md`, `doc/usage.md`, `doc/config.md`, `doc/architecture.md`, `doc/troubleshooting.md`.

## [0.13.0]

### Changed

- The `[tool-result …]` lines, the spinner and its elapsed time (with the `… +N more` markers) and the slash-command hint above the prompt are now **dark yellow** instead of dimmed grey — they are secondary, but they still have to be readable. The reasoning rows, the tool arguments, `[cancelled]` and the `/history` listing keep their grey.
- The startup information under the `agent-cli ready` banner — `id`, `name`, `provider`, `features`, `role`, `skills` and the `/help` hint — is no longer dimmed: it is printed in the terminal's default foreground. The banner line itself stays magenta and bold.

## [0.12.0]

### Changed

- `agent-cli update` now builds from **`main`** when `--ref` is not given — a bare `agent-cli update` is exactly `agent-cli update --ref main`. Previously it targeted the latest published GitHub release.
  - Because a branch carries no version to compare against, the update always rebuilds; the "already up to date" shortcut no longer applies to the default path. `agent-cli update --check` still reports the running version against the latest **release** and changes nothing.
  - A published release is no longer needed for an update to work, and a failed GitHub lookup no longer blocks it — the banner just reads `(latest: unknown)`.
  - Pin a release with `--ref v0.11.0`; any other branch or tag still works as before.

### Added

- Terminal colour scheme — the REPL is now colour-coded with the ANSI 16 colours, so input, activity and status are told apart at a glance.
  - Cyan bold for the prompt symbol and `[tool-call] <tool>`; grey for tool arguments; grey dim for `[tool-result]`, reasoning, the spinner, the elapsed time and the `… +N more` markers; magenta bold for the `[answer]` marker and the startup banner; green bold for `✔ <elapsed>`; red bold for `✗ <elapsed>` and `[error]`; blue for `[info]` / `[auto]`; yellow bold for `[tool approval]` and `approve? [y/N]:`; dim for `[cancelled]`, the `/history` listing and the command hint above the prompt.
  - **The answer body is deliberately left uncoloured** — it is the longest thing on screen.
  - New `[ui] color` (default `"auto"`): `"auto"` colours a stream only when it is an interactive terminal, `NO_COLOR` is unset or empty and `TERM` is not `dumb`; `"always"` forces colour even when redirected; `"never"` disables it. Unknown values fall back to `"auto"`.
  - stdout and stderr are decided independently, so `agent-cli run > answer.txt` from a terminal writes a clean file while the on-screen status display stays coloured. `agent-cli serve` is never coloured.
  - Only ANSI 16 foreground colours are used — no background, no `reverse`, no 256-colour — so the terminal's own palette decides the shades and the scheme works on light and dark backgrounds.
  - With colour off, the output is byte-identical to v0.11.0 on every path. Layout is untouched: styling is applied only after each line has been measured, cut and wrapped, and each row carries its own self-closing sequence, so truncation, the prompt's cursor column and the progress indicator's erase are unaffected.
  - Internally: a new `theme.rs` module maps a `Role` (what a piece of text is) to a colour and attributes. No new dependency; Linux-only as before.
  - Docs updated: `README.md`, `README_ja.md`, `doc/usage.md`, `doc/config.md`, `doc/architecture.md`, `doc/troubleshooting.md`.

## [0.11.0]

### Added

- Turn progress indicator — while the agent works, the REPL shows what is being executed on one line and, directly beneath it, a spinner with the elapsed time; the turn ends as a single `✔ <elapsed>` line.
  - The line above the spinner describes the current activity: the question you submitted, then each tool call. While the indicator is on, a `[tool-call]` line is cut to a single terminal row with `…`, so raw arguments no longer push the display around.
  - The spinner row is redrawn in place ten times a second (`0.4s` → `12.4s` → `2m03s`) and is cut to the terminal width, so it never wraps. It is drawn only from the start of a fresh row, leaving streamed text untouched, and is erased before any other output.
  - A finished turn leaves `✔ <elapsed>`; a failed one `✗ <elapsed>`. A turn stopped with `Esc` still prints `[cancelled]` and leaves no mark. The indicator steps aside for the tool-approval prompt and resumes once it is answered.
  - New `[ui] show_progress` (default `true`) turns it off. It is drawn only when stdin and stderr are both terminals, so piped output, redirects and `agent-cli serve` are unchanged, as is every path with the setting disabled.
  - The model's reasoning is shown **live under the spinner**, the last 10 rows at a time with a `… +N more (click to expand)` marker. Clicking the block with the mouse switches to as much of the reasoning as the screen can hold and clicking again collapses it; the choice carries over to later turns. The block belongs to the running turn and is cleared when it ends — the full reasoning is still written to the conversation log.
  - Tool results are cut to five rows on screen while the indicator is on, ending in `… +N more lines`, so a large `bash` output no longer pushes the spinner down the screen. This is a display change only: the model still receives the full result and the conversation log keeps it.
  - Mouse reporting is enabled only while a turn is running, and only when reasoning is shown (`[ui] show_thinking` other than `"hidden"`); while it is on, selecting text needs the terminal's usual Shift override. With `show_progress = false` the previous inline `[thinking]` output is unchanged.
  - Internally: a new `AgentEvent::TurnStart` marks the turn boundary and prints nothing. No new dependency; Linux-only as before.
  - Docs updated: `README.md`, `README_ja.md`, `doc/usage.md`, `doc/config.md`, `doc/architecture.md` (§3.1), `doc/troubleshooting.md`.

## [0.10.0]

### Added

- `Esc` during execution returns to the prompt — pressing `Esc` (or `Ctrl+C`) while the agent is streaming a response, running a tool, or waiting for tool approval stops the turn and hands the prompt straight back.
  - The prompt never waits for the agent: the REPL prints `[cancelled]`, leaves the `Pending` state and redraws immediately, and accepts the next input at once. At the approval prompt, `Esc` also denies the pending tool (`Ctrl+C` keeps its usual clear-line / exit meaning there); at an idle prompt both keys behave exactly as before.
  - A shared cancellation token (`AtomicBool` + `Notify`) is observed by `process_turn` at every await point — the tool-iteration boundary, the provider stream (a `biased` `select!`, so the response body read is dropped mid-stream) and the tool invocation (an already-running tool is abandoned). The turn then ends with exactly one `Done`.
  - The conversation stays usable: every tool call left without a result is recorded as `cancelled by user`, so the assistant message's `tool_calls` stay balanced and the next prompt is still a valid request. The partial answer is kept in history.
  - The cancelled turn's remaining output is discarded instead of printing over the new prompt, and its completion no longer releases a later turn from `Pending`.
  - `/cancel` now raises the same signal — it stops an in-flight turn (e.g. one started by a peer prompt) rather than only requesting it.
  - Not affected: a process a tool already spawned is not killed; it ends on its own `[tools.bash] timeout_ms`. No new dependency; Linux-only as before.
  - Docs updated: `README.md`, `README_ja.md`, `doc/usage.md`, `doc/config.md`, `doc/architecture.md` (§3.1), `doc/troubleshooting.md`.

## [0.9.0]

### Added

- MCP HTTP/SSE transport — MCP servers can now be reached over **Streamable HTTP** (a URL), in addition to stdio.
  - A `[[mcp.servers]]` entry with `transport = "http"` and a `url` connects over HTTP: agent-cli POSTs JSON-RPC and accepts either a single `application/json` reply or a `text/event-stream` (SSE) reply, selecting the message matching the request id. It captures the `Mcp-Session-Id` returned by `initialize` (and the negotiated protocol version) and echoes them on subsequent requests, sends static `headers` and an `api_key_env` Bearer token, and ends the session with a best-effort `DELETE` on shutdown.
  - New per-server config keys: `url`, `headers`, `api_key_env` (http); `command`/`args`/`env`/`cwd` remain stdio-only. HTTP servers register and behave identically to stdio ones (`mcp__<server>__<tool>`, same approval gate, same fail-soft skip-on-error).
  - `McpClient` is generalised over an `McpTransport` seam (`StdioTransport` + new `HttpTransport` in `src/mcp/http.rs`); the handshake / `tools/list` / `tools/call` logic is shared. Reuses `reqwest` and the existing `SseAccumulator` — no new dependency. Only single-endpoint Streamable HTTP is supported (no legacy two-endpoint HTTP+SSE, no OAuth, no server→client listen stream); Linux-only.
  - Docs updated: `doc/config.md`, `doc/usage.md` (Remote (HTTP) servers), `doc/architecture.md` (§8.3), `doc/troubleshooting.md`, `README.md`, `README_ja.md`.

## [0.8.0]

### Added

- MCP client — agent-cli can now access external **Model Context Protocol (MCP) servers** and offer their tools to the agent.
  - Declare servers in a new `[[mcp.servers]]` config section (`name`, `command`, `args`, `env`, `cwd`, `enabled`, `transport`) with an optional `[mcp] init_timeout_ms`. On `run` / `serve`, each enabled server is launched over **stdio**, the MCP handshake runs (`initialize` → `notifications/initialized` → `tools/list`), and every discovered tool is registered under a namespaced name **`mcp__<server>__<tool>`** so it never collides with a built-in or another server.
  - MCP tools flow through the normal agent loop and approval gate; invoking one forwards to the server's `tools/call` and flattens the result (an MCP `isError` becomes a tool error, not a hard failure). A server that fails to launch, handshake, or list within `init_timeout_ms` is logged and **skipped** — startup never aborts on a bad server, and server subprocesses are terminated on shutdown.
  - New `agent-cli mcp list` subcommand connects to the configured servers and lists their tools; `agent-cli doctor` gains an MCP section reporting each server's reachability and tool count.
  - Only the stdio transport and MCP tools are supported (HTTP/SSE and resources/prompts are not); Linux-only. No new dependency (JSON-RPC over `tokio::process` + `serde_json`); the `Tool` trait's `name`/`description` now return `&str` to carry runtime-discovered names (built-ins unchanged).
  - Docs updated: `doc/config.md` (`[mcp]` / `[[mcp.servers]]`), `doc/usage.md` (MCP servers), `doc/architecture.md` (§8.3), `doc/troubleshooting.md` (MCP Issues), `README.md`, `README_ja.md`.

## [0.7.0]

### Added

- Self-update — a new `agent-cli update` subcommand upgrades an installed agent-cli to the latest released version.
  - Since agent-cli ships no prebuilt binaries (installation is a source build via `cargo install`), the update is also a source build: it looks up the latest GitHub release (`releases/latest`, falling back to `tags`), compares it to the running version, and — if newer — runs `cargo install --git <repo> --tag <version> agent-cli --root <prefix>` into the running binary's install prefix (derived from `current_exe()`), then verifies the new `--version`. Needs the Rust toolchain (`cargo`); Linux-only.
  - Flags: `--check` (report current/latest/availability and exit without changing anything — script/CI-safe), `--force` (reinstall even when already up to date, and allow a same/older `--ref`), `--yes` (skip the confirmation prompt; on a non-TTY the command refuses without it), and `--ref <tag|branch>` (build from a specific ref instead of the latest release — e.g. `--ref main` before a release is tagged).
  - Version comparison is a small hand-rolled semver (no new dependency); the network/`cargo` edges wrap a pure, unit-tested core (semver, repo-slug, tag extraction, prefix derivation).
  - Docs updated: `doc/usage.md` (Updating), `doc/architecture.md` (§8.2), `doc/troubleshooting.md` (Update Issues), `README.md`, `README_ja.md`.

## [0.6.0]

### Added

- Group identifier for launched agents — agents that are launched together (a root and the detached peers it spawns) can now carry a shared **group** id, so a whole cohort is recognizable at a glance.
  - New global option `--group <id>` (available on `run` / `serve` / `spawn`) and a new `[runtime] group` config key set the group; the flag overrides the config key, and with neither the agent is ungrouped. A group is a free-form label persisted on the registry entry (`RegistryEntry.group`); registry files written by older versions (no `group` key) still load, and an ungrouped agent's file is byte-unchanged.
  - Detached children **inherit** their launcher's effective group automatically, so the id is named once at the root: `agent-cli spawn` / `/spawn` / the `spawn` tool all propagate it (an explicit `--group`, or the tool's `group` argument, overrides). Because the child resolves `--group` ahead of `[runtime] group`, the inherited value wins even if the child's config names a different default.
  - `agent-cli list` gains a `GROUP` column and a `--group <id>` filter (`agent-cli list --group team` lists only that group's members).
  - New `agent-cli groups` subcommand detects the distinct groups currently running as processes and lists them with member counts (aggregated from the live registry, so a group is reported iff at least one member is alive; ungrouped agents fall under a `-` bucket).
  - Docs updated: `doc/usage.md` (Groups), `doc/architecture.md` (§4, §7.1), `doc/config.md` (`[runtime] group`), `doc/tools.md` (`spawn` tool `group` argument), `README.md`, `README_ja.md`.

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

[Unreleased]: https://github.com/aquaxis/agent-cli/compare/v0.13.0...HEAD
[0.13.0]: https://github.com/aquaxis/agent-cli/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/aquaxis/agent-cli/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/aquaxis/agent-cli/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/aquaxis/agent-cli/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/aquaxis/agent-cli/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/aquaxis/agent-cli/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/aquaxis/agent-cli/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/aquaxis/agent-cli/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/aquaxis/agent-cli/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/aquaxis/agent-cli/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/aquaxis/agent-cli/releases/tag/v0.3.0