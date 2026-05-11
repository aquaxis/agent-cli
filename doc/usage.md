# Usage Reference (`usage.md`)

## Subcommands

```text
agent-cli [--config <path>] <subcommand>
```

### Global Options

| Option | Description |
|--------|-------------|
| `--config <path>` | Config file to use. The `AGENT_CLI_CONFIG` environment variable is also accepted |

### Subcommands

| Form | Purpose |
|------|---------|
| `agent-cli run [...]` | Start the REPL (default) |
| `agent-cli list` | List running peers |
| `agent-cli send <peer> <text>` | Send a prompt to a peer and exit |
| `agent-cli providers` | Show available backend status |
| `agent-cli doctor` | Sanity-check config / API keys / connectivity / registry / bash |
| `agent-cli selftest [--provider <name>]` | Run smoke test |
| `agent-cli config show` | Print current configuration |
| `agent-cli config edit` | Open config in `$EDITOR` |
| `agent-cli config path` | Print resolved config path |

### `run` subcommand options

| Option | Description |
|--------|-------------|
| `--name <name>` | Agent display name |
| `--provider <kind>` | Override backend |
| `--model <model>` | Override model |
| `--persona <path>` | Explicit persona file path |
| `--auto-approve-tools` | Skip y/N approval for tool invocations |

## REPL Commands

In the REPL, lines starting with `/` are commands; everything else is a normal prompt to the active agent.

| Command | Purpose |
|---------|---------|
| `/list` | List peers (id, name, provider, model, role) |
| `/send <peer> <text>` | Send a prompt to a peer |
| `/tools` | List tools enabled for this agent |
| `/persona` | Show this agent's persona (role / skills / description / tool restrictions / source path) |
| `/reload-persona` | Re-resolve and reload the persona file, updating the system prompt (history preserved) |
| `/peer <id_or_name>` | Show a peer's persona summary |
| `/history [n]` | Show last n (default 20) user inputs |
| `/clear`, `/reset` | Clear conversation history (system prompt = persona is kept; User / Assistant / ToolResult are all removed) |
| `/cancel` | Request cancellation of in-flight processing (request only; no guarantee of immediate stream stop) |
| `/auto [on\|off\|status]` | Toggle tool-approval skip at runtime. No argument or `status` shows the current value |
| `/help` | Show command list |
| `/quit` / `/exit` | Terminate the application |

### Skipping Tool Approval

Tool invocations (shell, fs_*, send_to) request y/N approval by default. There are three ways to skip approval (any combination works):

| Method | Example | When it takes effect |
|--------|---------|---------------------|
| Config file | `[runtime] auto_approve_tools = true` | At `agent-cli` startup |
| CLI flag | `agent-cli run --auto-approve-tools` | Startup only (temporary override) |
| REPL command | `/auto on` | Immediately. `/auto off` returns to approval mode |

`/auto status` (or `/auto` with no argument) shows the current value. In approval mode, each tool request displays `[tool approval] <tool> <args>` and `approve? [y/N]:`. Only `y` / `yes` is accepted; anything else (blank input, other words) counts as denial.

### Suppressing `[thinking]` Output

Claude's `thinking_delta` and Ollama's `message.thinking` (e.g. `glm-5.1:cloud`) are passed to the REPL as `AgentEvent::Thinking` and rendered as `[thinking] <text>` lines. Long-reasoning models emit large amounts of thinking text, so `[ui] show_thinking` provides three levels of control:

| Value | Behavior |
|-------|----------|
| `"hidden"` | Never print `[thinking]` lines |
| `"collapsed"` (default) | Truncate each delta to a single line: "first 80 chars + `...`" |
| `"expanded"` | Print the full thinking text verbatim |

Unknown values (e.g. `"verbose"`) fall back to `"collapsed"`. Changes take effect on next `agent-cli` restart; runtime toggling is not supported. See [`doc/config.md`](config.md) "UI display modes" for details.

### `[info]` Messages in the REPL

The REPL renders `Info` variants of `AgentEvent` with an `[info]` prefix. `Info` is supplementary / status information, not an error (errors use the `[error]` prefix). Common messages:

| Message | Trigger | What happens next |
|---------|---------|-------------------|
| `[info] cancel requested` | `/cancel` entered | Sends a cancellation request to in-flight processing (no guarantee of immediate stop) |
| `[info] history persisted (N entries)` | History save trigger (e.g. `/history`) | Flush to input history file complete |
| `[info] system prompt updated` | `/reload-persona` replaced the system prompt at the head of history | Subsequent responses use the new system prompt |
| `[info] history cleared (N message(s) removed)` | `/clear` / `/reset` cleared conversation history | System prompt (persona) kept; User / Assistant / ToolResult all removed |
| `[info] max tool-use iterations reached` | tool_use iteration count reached `[runtime] max_tool_iterations` (default 24) (FR-04-3 / design doc 4.3B) | The turn ends with `Done`; the next user input prompt is redrawn |

`[info] max tool-use iterations reached` is a guard mechanism that activates when the AI keeps cycling through `tool_use → tool result → tool_use → ...` without reaching a conclusion (`agent.rs::process_turn` applies `self.config.runtime.max_tool_iterations.max(1)`). The cap is configurable (default 24, min 1, max `u32::MAX`). For meaning and workarounds, see `doc/troubleshooting.md` "When `[info] max tool-use iterations reached` appears"; for tuning the value, see `doc/config.md` section `[runtime]`.

## Use Cases

### 1. Standalone chat (claude)

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
agent-cli run --provider claude
```

### 2. Local LLM (ollama)

```bash
ollama serve &
agent-cli run --provider ollama --model glm-5.1:cloud
```

### 3. Two-process coordination (claude x ollama)

```toml
# Share registry_dir in both configs
[runtime]
registry_dir = "/tmp/agent-cli/team"
```

```bash
# Terminal A
agent-cli run --provider claude --name alice

# Terminal B
agent-cli run --provider ollama --model glm-5.1:cloud --name bob
```

From terminal A:

```text
> /list
agent-01HX...    alice    claude    claude-opus-4-7    general assistant
agent-01HY...    bob      ollama    glm-5.1:cloud      general assistant

> /send bob "Give me a one-line review from B's perspective"
delivered to agent-01HY...
```

Terminal B shows `[peer prompt from agent-01HX...]` and the AI responds.

### 4. Role assignment (persona operation)

```bash
cp example/agents/reviewer.md ~/.config/agent-cli/agents/alice.md
cp example/agents/coder.md    ~/.config/agent-cli/agents/bob.md

# In separate terminals
agent-cli run --name alice    # reviewer persona auto-applied
agent-cli run --name bob      # coder persona auto-applied
```

Type `/persona` in the REPL to see the currently applied role and skills. For the full list of frontmatter keys (`role` / `skills` / `allowed_tools` / `denied_tools` / `model` / `temperature` etc.) and operational patterns, see [`doc/personas.md`](personas.md).

### 5. One-shot send from CLI

To send a short message to another agent without starting a REPL:

```bash
agent-cli send alice "stand-by"
```

This runs as an IPC client only and exits immediately. The receiving agent continues to respond.

### 6. Configuration switching

```bash
agent-cli --config ./project-a.toml run --name proj-a
agent-cli --config ./project-b.toml run --name proj-b
```

If `registry_dir` is different, they run in completely isolated environments.

## Input History

User inputs (normal prompts not starting with `/`) are persisted to `<runtime.log_dir>/history.txt`, one entry per line. They are reloaded on next startup and can be viewed with `/history [n]`.

- With the default `runtime.log_dir = "~/.local/share/agent-cli/logs"`, history lives at `~/.local/share/agent-cli/logs/history.txt`.
- The in-memory limit is the last 200 entries. The file is append-only.
- If you enter sensitive information, delete it from the history file manually.

## Resetting Conversation History (`/clear`)

To reset the conversation context (System / User / Assistant / ToolResult) sent to the LLM each turn, run `/clear` (or its alias `/reset`).

- The system prompt (derived from the persona) is kept. Only User / Assistant / ToolResult messages are removed.
- The subsequent Info output shows the removal count: `[info] conversation history cleared (N message(s) removed; persona retained)`.
- To change the persona itself, use `/reload-persona` in combination.
- `/clear` operates on the in-memory history within the current process. It does not delete conversation log files at `<log_dir>/<agent-id>/<timestamp>.jsonl` (you can review them later).