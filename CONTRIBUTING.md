# Contributing Guide

Contributions to `agent-cli` are welcome. This document summarizes the basic rules for development participation.

## Prerequisites

- Linux (x86_64 / aarch64)
- Rust stable (pinned in `rust-toolchain.toml`)
- `cargo` / `git` / `bash`

## Development Flow

```bash
# 1. Clone
git clone https://github.com/aquaxis/agent-cli.git
cd agent-cli

# 2. Build / test
cargo build
cargo test

# 3. Lint and format
cargo fmt --all
cargo clippy --all-targets -- -D warnings

# 4. Local verification
cargo run --quiet -- providers
cargo run --quiet -- doctor
```

Before creating a PR, ensure the following passes:

- `cargo build` (zero warnings)
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo fmt --check`
- `cargo doc --no-deps` (public APIs must have rustdoc comments)

If you have a live environment (API keys or a local LLM server), also run `./scripts/manual_acceptance.sh`. It aggregates SKIP / PASS / FAIL and detects failures via exit code.

```bash
ANTHROPIC_API_KEY=sk-ant-... \
  OPENAI_API_KEY=sk-...     \
  OLLAMA_URL=http://127.0.0.1:11434 \
  ./scripts/manual_acceptance.sh
```

## Documentation Rules

PRs that add, change, or deprecate features must update the relevant documents **within the same PR**:

- `README.md` (quick start, main commands, configuration, backend table)
- `doc/usage.md` (commands, REPL specification)
- `doc/config.md` (config key additions / changes)
- `doc/providers/<kind>.md` (backend-specific behavior)
- `doc/tools.md` (tool specifications)
- `doc/troubleshooting.md` (known failures and fixes)
- `CHANGELOG.md` (`[Unreleased]` section with Added / Changed / Fixed / Removed)
- `rustdoc` (public APIs require `///`)

## Specification Files

`/.aiprj/AI_PRJ_REQUIREMENTS.md` / `AI_PRJ_DESIGN.md` / `AI_PRJ_TASKS.md` are specification documents that drive AI-assisted development. If the implementation diverges from the spec, **update the spec first** or reconcile both within the same PR.

## Adding a New Backend

1. Create `src/ai/<kind>.rs` and implement `trait Provider`
2. Add a branch in `src/ai/mod.rs::build`
3. Add the name to `src/ai/mod.rs::SUPPORTED`
4. Update dependencies in `Cargo.toml` as needed
5. Add defaults for the `[provider.<kind>]` section in `src/config.rs`
6. Create `doc/providers/<kind>.md` documenting auth, recommended models, and supported features
7. Verify display in `src/commands.rs::providers`
8. Tests: unit tests for the parser (verify mock HTTP flow with `SseAccumulator`)
9. Ensure `agent-cli doctor` runs without errors

## Adding a New Tool

1. Create `src/tools/<name>.rs` and implement `trait Tool`
2. Add to the table in `src/tools/mod.rs::ToolRegistry::build`
3. Consider adding to `tools.enabled` in the default config `src/config.rs::DEFAULT_CONFIG`
4. Document argument schema, return values, and limitations in `doc/tools.md`
5. Tests: `tokio::test` for success and error cases
6. Verify that the tool can be controlled via persona `allowed_tools` / `denied_tools`

## Commit Messages

Use concise imperative form (English or Japanese), keeping PR-level scope in mind. Examples:

- `feat(ollama): add tool_calls parsing`
- `fix(ipc): cleanup stale sockets on PID disappearance`
- `docs(config): explain registry_dir sharing`

## License

Code contributed to this project is published under the MIT License.