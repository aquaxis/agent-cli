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
| `agent-cli spawn [...]` | Create a **detached** agent that does not depend on this process: it runs headless in its own session, self-registers as a peer, and outlives the launcher. Accepts the same options as `run`. See [Detached agents](#detached-agents) |
| `agent-cli stop <peer>` | Stop a running peer (id or name) by requesting a graceful shutdown; falls back to `SIGTERM` if the socket is unreachable |
| `agent-cli serve [...]` | Run headless (register + serve peers over IPC, no REPL). This is the target `spawn` launches; running it directly gives a foreground headless agent |
| `agent-cli list` | List running peers |
| `agent-cli send <peer> <text>` | Send a prompt to a peer and exit (does not wait for a response) |
| `agent-cli ask <peer> <text> [--timeout <secs>]` | Send a prompt to a peer, wait for its AI response, print it, and exit (default timeout 120 seconds) |
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
| `--provider <kind>` | Override backend: `claude` / `claude-code` / `codex` / `ollama` / `opencode` / `opencode-go` / `llama.cpp` |
| `--model <model>` | Override model |
| `--persona <path>` | Explicit persona file path |
| `--auto-approve-tools` | Skip y/N approval for tool invocations |

`spawn` and `serve` accept the same options as `run` (`--name` / `--provider` / `--model` / `--persona` / `--auto-approve-tools`).

## Detached agents

Every agent-cli process is already an independent peer, discovered through the
shared registry directory ([`architecture.md`](architecture.md) §1). `spawn`
lets a running agent-cli (or a one-shot command) **create** such a peer without
opening a second terminal:

```text
agent-cli spawn --name worker            # start a detached headless peer
agent-cli list                           # `worker` appears in the registry
agent-cli ask worker "summarise X"       # talk to it like any peer
agent-cli stop worker                    # ask it to shut down cleanly
```

- The child inherits the launcher's config file (hence the same
  `[runtime] registry_dir`), so it is immediately visible to `list` / `send` /
  `ask` / the `send_to` tool. Per-child overrides are limited to the `run`
  options above.
- The child runs **headless** (`serve` mode): it has no REPL and no controlling
  terminal, it self-registers, answers peer prompts over IPC, and — having no
  console to answer a y/N prompt — auto-approves tool execution.
- It is **independent**: launched in a new session (`setsid`) with detached
  stdio, it survives the launcher's exit, the launcher's `Ctrl+C`, and terminal
  hang-up. Stop it with `agent-cli stop <peer>` / `/stop <peer>` (a graceful
  IPC shutdown, falling back to `SIGTERM`), or with an OS signal. A crashed
  detached agent is reaped lazily from the registry like any other peer.

From inside a REPL the same is available as `/spawn [name] [provider]` and
`/stop <peer>`.

## REPL Commands

In the REPL, lines starting with `/` are commands; everything else is a normal prompt to the active agent.

| Command | Purpose |
|---------|---------|
| `/list` | List peers (id, name, provider, model, role) |
| `/send <peer> <text>` | Send a prompt to a peer |
| `/spawn [name] [provider]` | Create a detached agent peer (see [Detached agents](#detached-agents)) that outlives this session |
| `/stop <peer>` | Stop a running peer (id or name), including detached agents |
| `/tools` | List tools enabled for this agent (agent-cli's own registry; not offered to the model under `claude-code` delegation) |
| `/persona` | Show this agent's persona (role / skills / description / tool restrictions / source path) |
| `/reload-persona` | Re-resolve and reload the persona file, updating the system prompt (history preserved) |
| `/peer <id_or_name>` | Show a peer's persona summary |
| `/history [n]` | Show last n (default 20) user inputs |
| `/clear`, `/reset` | Clear conversation history (system prompt = persona is kept; User / Assistant / ToolResult are all removed) |
| `/cancel` | Request cancellation of in-flight processing (request only; no guarantee of immediate stream stop) |
| `/auto [on\|off\|status]` | Toggle tool-approval skip at runtime. No argument or `status` shows the current value |
| `/commands` | List custom slash commands with their first line and file path |
| `/reload-commands` | Re-scan the custom commands directory without restarting |
| `/help` | Show command list |
| `/quit` / `/exit` | Terminate the application |

### REPL Input Editing

When stdin is a terminal, the prompt runs in raw mode and supports in-place line editing and history browsing.

| Key | Action |
|-----|--------|
| `Enter` | Submit the line |
| `↑` / `↓` | Browse input history (older / newer) |
| `←` / `→` | Move the cursor one character |
| `Ctrl+A` / `Home` | Move to the start of the line |
| `Ctrl+E` / `End` | Move to the end of the line |
| `Backspace` / `Delete` | Delete the character before / at the cursor |
| `Esc` | Leave history browsing; on a normal line, clear it |
| `Ctrl+C` | Clear the line; on an empty line, exit |
| `Ctrl+D` | Exit on an empty line; ignored otherwise |
| `Tab` | Complete the slash command being typed (see below) |

Notes:

- **Draft preservation**: the line you were typing is saved when you first press `↑`, and restored when you press `↓` past the newest history entry.
- **Command candidates**: while the line starts with `/` and contains no space, the matching command names are listed on the line **above** the prompt, so the line you are typing stays put instead of being pushed around. Keep typing to narrow the list, or press `Enter` — prefix resolution is described under "Custom Slash Commands".
- **Tab completion**: `Tab` completes the command name from that same candidate list. One match completes it and adds a space, ready for an argument (`/sen` → `/send `). Several matches extend the line as far as the candidates agree (`/rel` → `/reload-`), leaving the list on screen to choose from. When there is nothing to add — no match, an already-settled name, or an argument already started — `Tab` does nothing. Built-in and custom commands complete alike.
- **Display width**: full-width characters (CJK) are counted as two columns, so cursor positioning stays correct in mixed-width lines.
- **TTY requirement**: raw mode is only enabled when stdin is a terminal. With piped or redirected input the REPL falls back to line-buffered reading, where the editing keys, the candidate list, and `Tab` completion are unavailable — everything else (tools, custom commands, peer messaging) works unchanged. See "Non-interactive / Scripted Use".

### Custom Slash Commands

Any `*.md` file in the commands directory becomes a slash command named after the file stem: `.agent-cli/commands/review.md` defines `/review`. Running it expands the file content and submits the result to the agent as a user prompt — the file is a prompt template, not a script.

The directory is `[runtime] commands_dir` (default `.agent-cli/commands`, resolved against the working directory). A missing directory is not an error; the REPL simply runs with built-in commands only. See [`doc/config.md`](config.md) `[runtime]`.

#### Template expansion

| Placeholder | Expands to |
|-------------|-----------|
| `$ARGUMENTS` | The entire argument string typed after the command name |
| `$1`, `$2`, … | The Nth whitespace-separated argument (1-based). Absent arguments expand to an empty string |
| `@<path>` | The contents of the referenced file. An unreadable path expands to `[error: cannot read @<path>]` |

Expansion runs once, in that order (`$` placeholders first, then `@` references), so a `$N` value that happens to contain `@path` is expanded, but `$N` inside an included file is not.

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

#### Resolution rules

1. **Built-in commands win.** A custom `help.md` never shadows `/help`.
2. **Exact match** on a custom command name runs it.
3. **Prefix match**: if exactly one custom command starts with what you typed, it runs automatically and prints `[auto] /<typed> → /<resolved>`.
4. **Ambiguous prefix**: if several match, the candidates are listed and nothing is executed.
5. **No match**: `unknown command: <name>`.

#### Managing commands

```text
> /commands
  /review  Review the following file and list the three most severe issues.  [.agent-cli/commands/review.md]

> /reload-commands
[reload-commands] 1 command(s) loaded
```

`/reload-commands` re-scans the directory, so new or edited files take effect without restarting the REPL. `/help` also lists the loaded custom commands in a separate section after the built-ins.

### Skipping Tool Approval

Tool invocations (bash, read, write, send_to, monitor, edit, glob, grep, websearch, webfetch) request y/N approval by default. There are three ways to skip approval (any combination works):

| Method | Example | When it takes effect |
|--------|---------|---------------------|
| Config file | `[runtime] auto_approve_tools = true` | At `agent-cli` startup |
| CLI flag | `agent-cli run --auto-approve-tools` | Startup only (temporary override) |
| REPL command | `/auto on` | Immediately. `/auto off` returns to approval mode |

`/auto status` (or `/auto` with no argument) shows the current value. In approval mode, each tool request displays `[tool approval] <tool> <args>` and `approve? [y/N]:`. Only `y` / `yes` is accepted; anything else (blank input, other words) counts as denial.

**Scope.** Approval governs the tools `agent-cli` itself runs. With
`kind = "claude-code"` in the default `mode = "delegation"`, the tools are run
inside Claude Code and reported afterwards, so `auto_approve_tools`,
`--auto-approve-tools`, `/auto`, and persona allow / deny lists do not gate
them. Restrict that backend with its own `tools` / `allowed_tools` /
`disallowed_tools` / `permission_mode` keys instead — see
[`doc/providers/claude-code.md`](providers/claude-code.md).

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

### 3. Local Claude Code CLI (claude-code)

Uses the `claude` CLI installed on the machine as the backend, with no API key —
Claude Code brings its own authentication.

```toml
[provider]
kind = "claude-code"

[provider.claude-code]
# bin   = "claude"   # executable name (resolved on PATH) or an absolute path
# model = "opus"     # omit to use Claude Code's own default
```

```bash
agent-cli run --provider claude-code
```

In the default `mode = "delegation"` Claude Code runs its own tools inside its
own process, so agent-cli's tools and its approval prompt are not part of the
turn; `mode = "gateway"` is chat-only. See
[`doc/providers/claude-code.md`](providers/claude-code.md).

### 4. Two-process coordination (claude x ollama)

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

### 5. Role assignment (persona operation)

```bash
cp example/agents/reviewer.md ~/.config/agent-cli/agents/alice.md
cp example/agents/coder.md    ~/.config/agent-cli/agents/bob.md

# In separate terminals
agent-cli run --name alice    # reviewer persona auto-applied
agent-cli run --name bob      # coder persona auto-applied
```

Type `/persona` in the REPL to see the currently applied role and skills. For the full list of frontmatter keys (`role` / `skills` / `allowed_tools` / `denied_tools` / `model` / `temperature` etc.) and operational patterns, see [`doc/personas.md`](personas.md).

### 6. One-shot send from CLI

To send a short message to another agent without starting a REPL:

```bash
agent-cli send alice "stand-by"
```

This runs as an IPC client only and exits immediately. The receiving agent continues to respond.

### 7. One-shot ask from CLI (waits for the answer)

When you want the peer's answer back on stdout instead of just delivering a prompt:

```bash
agent-cli ask alice "Summarize the current design risks in three bullets"
agent-cli ask alice "Run the tests and report the failure count" --timeout 300
```

`ask` binds a temporary reply socket, sends the prompt with that address attached, and prints the response text — and nothing else — when it arrives. This makes it the form to use inside shell scripts:

```bash
risks=$(agent-cli ask alice "List the top risk in one line")
```

Differences from `send`:

| | `send` | `ask` |
|---|--------|-------|
| Waits for the answer | No | Yes |
| Output | `delivered to <agent-id>` | The peer's response text |
| Failure mode | Peer not found | Peer not found, or timeout (default 120 s, `--timeout`) |

The peer must already be running. If it needs tools to answer, start it with `--auto-approve-tools`, otherwise it will stop at an approval prompt that no one can answer.

### 8. Configuration switching

```bash
agent-cli --config ./project-a.toml run --name proj-a
agent-cli --config ./project-b.toml run --name proj-b
```

If `registry_dir` is different, they run in completely isolated environments.

## Non-interactive / Scripted Use

`agent-cli run` does not require a terminal. When stdin is a pipe or a file, the
REPL reads it line by line, so a complete question-and-answer cycle can be driven
from the command line alone.

```bash
echo "Explain Rust ownership in three lines" | agent-cli run
```

Rules:

- **One input line is one prompt.** Use a heredoc to ask several questions in
  sequence; they share one conversation, so later questions can refer to earlier
  answers.
- **The answer is never truncated.** After a prompt is submitted the input loop
  stops reading stdin until the turn completes, so end-of-input is only noticed
  once the agent is idle again. The process then shuts down on its own.
- Lines beginning with `/` are still commands, so `printf '/tools\n' | agent-cli run` works too.

```bash
agent-cli run --provider claude <<'EOF'
Summarize the architecture of this repository
Which module owns tool approval?
EOF
```

### Letting tools run

Tools work exactly as they do interactively — only line editing and the inline
suggestion require a terminal. The one thing to plan for is approval:

```bash
echo "Count the .rs files under src with bash and answer with the number only" \
  | agent-cli run --auto-approve-tools
```

`--auto-approve-tools` (or `[runtime] auto_approve_tools = true`) is the
supported way to run tools unattended. Without it, approval answers are read
from the *same* stdin, which is workable but fragile:

- While an approval is pending, the next input line is consumed as the answer.
  Only `y` / `yes` approves; anything else denies.
- If you supply **more** `y` lines than there were tool calls, the surplus lines
  are read afterwards as ordinary prompts and sent to the model.
- If input ends while an approval is still pending, the call is denied as part
  of shutdown.

Since the number of tool calls is not predictable, prefer `--auto-approve-tools`
for scripted runs, and restrict what the agent may do with a persona
(`denied_tools`) rather than by withholding approval.

### Getting clean output

A piped `run` still prints the startup header, the `> ` prompt markers, and
`tracing` log lines alongside the answer. For scripts that need the response
text only, run a persistent agent and query it with `ask`:

```bash
agent-cli run --name worker --auto-approve-tools &
answer=$(agent-cli ask worker "Run the tests and report the failure count" --timeout 300)
```

## Input History

User prompts **and executed slash commands** are persisted to `<runtime.log_dir>/history.txt`, one entry per line. They are reloaded on next startup and can be viewed with `/history [n]`.

- Built-in commands (`/help`, `/tools`, …) and custom commands are both recorded, with their arguments. `/quit` and `/exit` are the only exceptions — they terminate before the entry is written.
- A command executed through prefix matching is stored under its resolved name: typing `/rev` and having it resolve to `/review` records `/review`.
- Consecutive duplicates are collapsed into a single entry.
- With the default `runtime.log_dir = "~/.local/share/agent-cli/logs"`, history lives at `~/.local/share/agent-cli/logs/history.txt`.
- The in-memory limit is the last 200 entries. The file is append-only.
- If you enter sensitive information, delete it from the history file manually.

## Resetting Conversation History (`/clear`)

To reset the conversation context (System / User / Assistant / ToolResult) sent to the LLM each turn, run `/clear` (or its alias `/reset`).

- The system prompt (derived from the persona) is kept. Only User / Assistant / ToolResult messages are removed.
- The subsequent Info output shows the removal count: `[info] conversation history cleared (N message(s) removed; persona retained)`.
- To change the persona itself, use `/reload-persona` in combination.
- `/clear` operates on the in-memory history within the current process. It does not delete conversation log files at `<log_dir>/<agent-id>/<timestamp>.jsonl` (you can review them later).