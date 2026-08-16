# Claude Code Backend

This backend drives the locally installed **Claude Code CLI** (`claude`) as an
`agent-cli` provider. `agent-cli` spawns `claude` as a child process, feeds it
turns, and maps its output onto the usual `ProviderEvent` stream — so the REPL,
personas, peer IPC, conversation logging, and custom slash commands wrap around
Claude Code.

It has no API key of its own: it inherits whatever authentication Claude Code
already has on the machine.

> Not to be confused with the [`claude` backend](./claude.md), which is a direct
> HTTPS client for the Anthropic Messages API and has nothing to do with the
> Claude Code CLI.

## Prerequisites

- Claude Code installed and on `PATH` (or `bin` pointed at it)
- Claude Code already authenticated (`claude` runs without prompting)

Verified against Claude Code `2.1.233`.

## Configuration

```toml
[provider]
kind = "claude-code"

[provider.claude-code]
bin       = "claude"       # executable name (resolved via PATH) or full path
model     = "sonnet"       # --model; omit for Claude Code's own default
mode      = "delegation"   # "delegation" | "gateway"
transport = "stream"       # "stream" | "oneshot"
session   = "persistent"   # "persistent" | "ephemeral" (delegation only)
turn_timeout_secs = 900
# tools              = ["Bash", "Read"]  # --tools
# allowed_tools      = ["Bash(git *)"]   # --allowed-tools
# disallowed_tools   = ["WebFetch"]      # --disallowed-tools
# permission_mode    = "auto"            # --permission-mode
# max_budget_usd     = 1.0               # --max-budget-usd
# system_prompt_mode = "append"          # "append" | "replace"
# extra_args         = []                # passed through verbatim
```

Every key is optional. The whole section may be omitted, in which case the
defaults above apply.

| Key | Default | Effect |
|-----|---------|--------|
| `bin` | `"claude"` | Executable. A value containing a path separator is used as-is; otherwise it is looked up on `PATH`. A missing binary fails at startup. |
| `model` | unset | `--model`. Also settable with `agent-cli run --model`. |
| `mode` | `"delegation"` | Who owns the agent loop — see below. |
| `transport` | `"stream"` | `"stream"` streams token deltas; `"oneshot"` returns the whole reply at end of turn. |
| `session` | `"persistent"` | Delegation only. `"persistent"` reuses one Claude Code session; `"ephemeral"` adds `--no-session-persistence` and re-sends the transcript each turn. |
| `tools` | unset | `--tools`. Ignored in gateway mode. |
| `allowed_tools` / `disallowed_tools` | unset | `--allowed-tools` / `--disallowed-tools`. |
| `permission_mode` | unset | `--permission-mode`, passed through unvalidated. |
| `turn_timeout_secs` | `900` | Per-turn wall clock. On expiry the child is killed and the turn ends with an error; the next turn starts a fresh child. |
| `max_budget_usd` | unset | `--max-budget-usd`. Recommended for unattended use. |
| `system_prompt_mode` | `"append"` | `--append-system-prompt` vs `--system-prompt` for the persona. |
| `extra_args` | `[]` | Extra flags appended before the prompt argument. |

## Delegation vs gateway

**`mode = "delegation"` (default).** Claude Code runs its own tools, applies its
own permission mode, and keeps its own session. `agent-cli` supplies the front
end and everything around it. Tool activity is reported by Claude Code *after*
it has already executed, so those blocks are never converted into agent-cli tool
calls — otherwise every command would run twice. Consequently agent-cli's
approval prompt is not involved; control permissions with `permission_mode`,
`tools`, `allowed_tools`, and `disallowed_tools`.

**`mode = "gateway"`.** `--tools ""` disables Claude Code's tools and the
backend behaves as a plain chat endpoint.

> **Gateway mode is chat-only.** `claude -p` accepts no external tool
> definitions (short of MCP), so agent-cli's own tools cannot be offered to it
> either. In gateway mode the model will describe the tool it *would* use and
> stop. If you want tool execution, use delegation mode.

## Process model

| mode / transport | Behaviour |
|---|---|
| delegation + stream + persistent | One **resident** child for the conversation: `--input-format stream-json --output-format stream-json --verbose --include-partial-messages`. Each turn writes one JSON line to its stdin; only new messages are sent. |
| delegation + oneshot | One child per turn with `--session-id` on the first turn and `--resume` afterwards. |
| delegation + ephemeral | One child per turn, `--no-session-persistence`, full transcript re-sent, tools still enabled. |
| gateway (either transport) | One child per turn, `--tools ""`, `--no-session-persistence`, full transcript as the prompt. |

The child is killed when `agent-cli` exits or a turn times out, so no orphan
process is left behind.

## Supported Features

| Feature | Support | Notes |
|---------|---------|-------|
| Streaming | ✓ (`transport = "stream"`) | Token deltas; `oneshot` delivers the reply at end of turn |
| Tool use | ✓ in delegation mode | Executed **inside Claude Code**, not by agent-cli |
| Thinking | ✓ (`transport = "stream"`) | Forwarded when Claude Code emits thinking blocks |
| Cost control | ✓ | `max_budget_usd`; per-turn cost is logged at debug level |
| agent-cli tool registry | ✗ | Cannot be offered to `claude -p` in either mode |
| Approval prompt | ✗ | Claude Code owns permission decisions |

## Verification

```bash
agent-cli providers          # shows the resolved claude binary
agent-cli doctor             # with kind = "claude-code": checks binary + --version
agent-cli selftest --provider claude-code
```

## Known Limitations

- The `stream-json` message shapes are an implementation detail of the Claude
  Code CLI, not a stable interface. Parsing here ignores unknown message types,
  but a future release could still change what is emitted. Pinned observations
  are from `2.1.233`.
- Gateway mode cannot use tools at all (see above).
- In delegation mode `agent-cli`'s history window management and logs describe
  only what passed through agent-cli; Claude Code's own session holds the rest.
- One user turn can cost several Claude Code turns (tool loops), so cost per
  turn is less predictable than with a raw API backend. Set `max_budget_usd`.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `Claude Code executable not found` | `claude` not on `PATH` | Install Claude Code, or set `bin` to its full path |
| `claude-code: turn timed out` | Turn exceeded `turn_timeout_secs` | Raise the limit, or shorten the task |
| `error_max_budget_usd` | `--max-budget-usd` reached | Raise `max_budget_usd` |
| `claude-code: exited with …` | The CLI refused the invocation | Read the quoted stderr; check `extra_args` and `permission_mode` |
| Model describes a tool but never runs it | Gateway mode | Switch to `mode = "delegation"` |
| Tool ran but agent-cli never asked for approval | Expected in delegation mode | Control with `permission_mode` / `allowed_tools` |
