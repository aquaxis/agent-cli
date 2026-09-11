# Tool Reference (`tools.md`)

Describes the argument schemas, return values, limitations, and approval flow for `agent-cli` built-in tools. The tool set is aligned with Claude Code's tools.

## Common Specifications

- Tools are called by the AI with JSON-formatted input.
- Return values are represented as `{"ok": bool, "content": string}` in `ToolOutput`, where `content` is passed back to the AI on the next provider call. Tools that return structured results, such as `bash`, embed a JSON string in `content`.
- Approval flow: When `auto_approve_tools=false` (default), a y/N prompt is obtained via the REPL input loop before execution. See "Tool Execution Approval" below for details. When denied, `user denied tool execution` is returned to the AI.
- Available tools can be controlled via the persona's `allowed_tools`/`denied_tools` (see `doc/config.md`).

### Which backends see these tools

The twelve tools below are `agent-cli`'s own registry. They are offered to the
model by the HTTP backends (`claude`, `codex`, `ollama`, `opencode`,
`opencode-go`, `llama.cpp`), which return `tool_use` requests that `agent-cli`
executes through the approval flow described below.

The `claude-code` backend is different, because the model it drives is Claude
Code, which carries tools of its own:

| `[provider.claude-code] mode` | Tools the model can use | This document applies |
|---|---|---|
| `"delegation"` (default) | Claude Code's own, executed inside Claude Code and reported afterwards | No — configure that set with `tools` / `allowed_tools` / `disallowed_tools` / `permission_mode` |
| `"gateway"` | none (`--tools ""`, chat only) | No |

`agent-cli`'s registry is never handed to that backend, so neither the approval
flow nor persona `allowed_tools` / `denied_tools` restrict what Claude Code
runs. See [`providers/claude-code.md`](providers/claude-code.md).

## Tool Execution Approval

The approval y/N input/output is **integrated into the REPL's main input loop** (it does not read directly via `std::io::stdin().read_line()`). This prevents approval input from being confused with the user's normal prompt.

Mechanism:

1. The agent task sends `ApprovalRequest { tool_name, args, response: oneshot::Sender<bool> }` to the input loop.
2. The input loop transitions state to `AwaitingApproval` and renders a `[tool approval] <tool> <args>` banner with `approve? [y/N]: `.
3. The input loop reads the next stdin line; only `y`/`yes` is treated as approval. Anything else (empty input or a different word) is treated as denial and sent via `oneshot`.
4. The agent task executes the tool according to the response, or returns `user denied tool execution`.

Approval skip (auto-approve) paths:

| Path | Example | When Applied |
|------|---------|-------------|
| Config file | `[runtime] auto_approve_tools = true` | At startup |
| CLI flag | `agent-cli run --auto-approve-tools` | Overrides at startup only |
| REPL command | `/auto on` | Immediate. `/auto off` returns to approval mode; `/auto status` shows current value |

In the implementation, `auto_approve` is shared between the agent and REPL as `Arc<AtomicBool>`, so it can be toggled at any time during the session via `/auto on`/`/auto off`.

## `spawn` (opt-in — not enabled by default)

Creates a detached agent-cli peer that runs headless in its own session and does
not depend on the current process. It is registered in the tool set but, unlike
the other tools, is **not** in the default `[tools] enabled`, because
autonomous process creation is more impactful than peer messaging; add `spawn`
to `[tools] enabled` (or a persona's `allowed_tools`) to offer it to the model.
It is the tool-level equivalent of the `agent-cli spawn` subcommand /
`/spawn` REPL command (see [`usage.md`](usage.md) "Detached agents").

### Arguments

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `name` | string | No | Display name for the new agent |
| `provider` | string | No | Backend override (`claude` / `claude-code` / `codex` / `ollama` / `opencode` / `opencode-go` / `llama.cpp`) |
| `model` | string | No | Model override |
| `persona` | string | No | Persona file path |
| `group` | string | No | Group id for the new agent; omit to inherit this agent's group (see [`usage.md`](usage.md) "Groups") |
| `prompt` | string | No | Initial prompt delivered to the new agent (fire-and-forget); use `send_to` for a reply |

### Return

`ok` with the new agent's `id` / `name` / `provider` / `model` / `socket`. The
new agent shares this agent's config file (hence the same `registry_dir`), so it
is immediately reachable with `send_to`, visible to `list_agents` and stoppable
with `stop_agent`. It runs headless and therefore auto-approves its own tool
execution. When `group` is omitted the new agent inherits this agent's group, so
a model spawning a fleet keeps the whole cohort under one recognizable group id.

### Limits

Autonomous creation is bounded by `[spawn]` (see [`config.md`](config.md)):
`max_children` live direct children per agent (default 4) and `max_depth`
generations of tool-spawned agents (default 2). When either is reached the tool
returns an **error** naming the limit, its configured value and the current
count — no process is created and the turn continues:

```
child limit reached: 4 live child agent(s) and [spawn] max_children is 4. Stop one
with stop_agent before creating another, or reuse an existing child with send_to.
```

Stopping a child frees its slot immediately. Either limit set to `0` disables
autonomous creation entirely. The limits bind **this tool only**: the
`agent-cli spawn` subcommand and the REPL's `/spawn` are a person deciding and
are not bounded.

## `list_agents`

Lists the agents this agent created — the peers it spawned, and the peers those
spawned. Read-only: one registry scan, with agents that have exited already
pruned. It is enabled by default.

Only this agent's **own tree** is shown. Peers created by someone else — a
sibling, the agent that created this one, an unrelated session — are not this
agent's to manage and do not appear. Use `agent-cli list` for the whole machine.

### Arguments

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `scope` | string | No | `"children"` for direct children only; `"subtree"` (default) for every agent below this one |

### Return

`ok` with one line per agent — id, name, provider/model, group, depth, and
whether it is a direct `child` or a deeper `descendant (via <name>)`:

```
agent-01M27QQ4BX…  name=worker  provider=ollama model=glm-5.1:cloud  group=-  depth=1  child
```

An empty tree returns `ok` with `no agents below this one` — nothing to manage
is an answer, not a failure. A descendant whose intermediate agent has exited is
still listed, marked `no longer running`, because it is still this agent's to
stop.

## `stop_agent`

Stops an agent this agent created, shutting it down gracefully (the same path
the `agent-cli stop` subcommand uses). Enabled by default.

It reaches **only the caller's own subtree**. A sibling, the agent that created
the caller, the caller itself and any unrelated peer are refused with a reason,
so an agent can undo what it did and nothing more:

```
"reviewer" is not in your tree: you did not create it. You can only stop agents
you spawned; list_agents shows them.
```

This is a guard against the model's mistakes, not a security boundary: anyone
who can write to the registry directory can already start processes. A person
keeps the unrestricted `agent-cli stop` and the REPL's `/stop`.

### Arguments

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `agent` | string | Yes | Agent id or display name, as shown by `list_agents` |

### Return

`ok` with the stopped agent's id and name, or an error naming why it was
refused (not in this agent's tree, no such agent, or the caller itself).
Stopping an agent that has children of its own leaves those running; they stay
in the caller's subtree and can be stopped next.

## `bash`

Executes a bash command (equivalent to Claude Code `Bash`).

### Arguments

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `command` | string | Yes | -- | Command body to execute (runs via `bash -lc <command>`) |
| `description` | string | -- | -- | Clear description of what the command does |
| `timeout_ms` | integer | -- | `[tools.bash] timeout_ms` (default 120000) | Per-invocation timeout in milliseconds |
| `run_in_background` | bool | -- | `false` | Not supported by `bash`; use the `monitor` tool |

### Return Value (`content` is a JSON string)

```json
{
  "exit_code": 0,
  "stdout": "...",
  "stderr": "..."
}
```

When `stdout`/`stderr` exceeds `[tools.bash] max_output_kb`, `...[truncated]` is appended.

### Limitations

- Runs via `bash -lc`, so `bash` must be installed.
- On timeout, returns `ok=false` with `timed out after <N> ms: <command>`.
- When `auto_approve_tools=false` (default), interactive y/N approval is required (see "Tool Execution Approval" above). Can be disabled for the session with `/auto on`.

### Example

```json
{"name":"bash","arguments":{"command":"ls /tmp"}}
```

## `read`

Reads a UTF-8 text file (equivalent to Claude Code `Read`). Output is line-numbered (`cat -n` style).

### Arguments

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `file_path` | string | Yes | Target path. Expands `~` and environment variables |
| `offset` | integer | -- | 1-based line number to start reading from |
| `limit` | integer | -- | Number of lines to read |

### Return Value

Returns line-numbered UTF-8 text in `content`. For binary or non-UTF-8 files, returns `ok=false` with `binary or non-UTF-8 file: <path>`.

### Notes

- The read content is returned to the AI via the tool-result feedback loop in `process_turn()` (the `ToolOutput.content` is pushed to history as a `Message::ToolResult` and re-sent on the next provider call).
- Image/PDF support is out of scope; text files only.

### Example

```json
{"name":"read","arguments":{"file_path":"./Cargo.toml","limit":40}}
```

## `write`

Writes UTF-8 text to a file (equivalent to Claude Code `Write`). Overwrites any existing content.

### Arguments

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `file_path` | string | Yes | Destination path |
| `content` | string | Yes | Content to write |

### Return Value

On `ok=true`, returns `wrote <path>` in `content`.

### Notes

- Parent directories are created automatically (`mkdir -p`).
- Overwrites existing files by default (matches Claude Code `Write`).

## `edit`

Performs an exact string replacement in a file (equivalent to Claude Code `Edit`).

### Arguments

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `file_path` | string | Yes | -- | Target path |
| `old_string` | string | Yes | -- | Exact text to replace (must be unique unless `replace_all`) |
| `new_string` | string | Yes | -- | Text to replace it with |
| `replace_all` | bool | -- | `false` | Replace every occurrence |

### Return Value

On `ok=true`, returns `replaced <N> occurrence(s) in <path>`. On `ok=false`, returns an error (`old_string not found`, `old_string is not unique (<n> occurrences)`, or `old_string and new_string must differ`).

## `send_to`

Sends a message to an agent in another process (a peer). The sender chooses how
it is delivered — and what it costs the peer.

### Arguments

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `peer` | string | Yes | -- | Destination agent-id or display name |
| `text` | string | Yes | -- | Message body |
| `delivery` | string | -- | `"prompt"` | `"prompt"` / `"report"` / `"ask"` — see below |
| `wait_reply` | bool | -- | `false` | Deprecated alias for `delivery="ask"`; still honoured |

| `delivery` | What the peer does | What comes back here |
|---|---|---|
| `"prompt"` (default) | Runs a turn and answers in its own session | `delivered to <id> as a prompt` |
| `"report"` | **Adds it to its conversation and runs no turn** | `reported to <id> (…it runs no turn and sends no answer)` |
| `"ask"` | Runs a turn and sends the answer back | the peer's answer text |

Pick by what the message *is*: `"ask"` when you need an answer, `"report"` when
you are handing over a finished result, `"prompt"` when the peer should act on it
in its own session. A result sent as a prompt makes the peer spend a turn
composing an answer nobody reads.

### Return Value

The delivery used, or the peer's answer for `"ask"`. On failure, an error message
(e.g. `peer not found by id or name: ...`). An unknown `delivery` value is an
error naming the three valid ones rather than a guess.

### Notes

- Destination resolution scans `<agent-id>.json` files under `registry_dir`.
- `"prompt"` and `"report"` are asynchronous: success means the peer acknowledged receipt, not that it has finished with it.
- On the receiving side, a prompt is prefixed with `[peer prompt from <agent-id>]` and a report with `[peer report from <agent-id>]`, so an agent that receives several can tell them apart.
- `"report"` against an agent-cli older than this feature is delivered **as a prompt instead**, and the tool result says so: the text still arrives, but that peer will answer it rather than only record it.

### Distributing work to child agents

The three deliveries make the fan-out pattern cheap. It needs `spawn`, which is
opt-in (see above):

1. `spawn` a child per task, passing its task as the tool's `prompt` argument — the children then work **in parallel**, and each one already knows the id of the agent that created it (it is in the header of that first message).
2. Tell each child, in that prompt, to report its result back with
   `send_to` using `delivery="report"`.
3. Each result arrives in the parent as one line — `[info] peer report from … : N characters added to the conversation` — and is added to its conversation **without costing it a turn**.
4. Ask the parent once to combine them. It answers from a conversation that already holds every report.

Without step 2's `"report"`, every result would arrive as a prompt and the parent
would run a turn per result, answering messages that have no recipient. Note that
tool calls within one turn run sequentially, so collecting with
`delivery="ask"` waits for one child at a time; reporting avoids the wait
entirely.

## `monitor`

Runs a long-running shell command and collects its stdout lines, returning them when the command exits or the timeout elapses (equivalent to Claude Code `Monitor`).

### Arguments

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `command` | string | Yes | -- | Shell command whose stdout lines are streamed |
| `description` | string | -- | -- | What is being monitored |
| `timeout_ms` | integer | -- | 60000 | Kill the command after this many milliseconds |
| `run_in_background` | bool | -- | `false` | Run detached; return immediately |

### Return Value

On `ok=true`, returns the collected stdout lines joined by newlines. On timeout, returns `ok=false` with the partial output plus `...[timed out after <N> ms]`.

## `glob`

Finds files matching a glob pattern (equivalent to Claude Code `Glob`). Supports `*`, `**`, `?`, and `[..]`.

### Arguments

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `pattern` | string | Yes | -- | Glob pattern (`*` does not cross `/`; `**` does) |
| `path` | string | -- | current dir | Directory to search in |
| `output_mode` | string | -- | `content` | `content` / `files_with_matches` / `count` |

### Return Value

`content`/`files_with_matches`: sorted matching paths, one per line. `count`: the number of matches.

## `grep`

Searches file contents for a regex pattern (equivalent to Claude Code `Grep`).

### Arguments

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `pattern` | string | Yes | -- | Regular expression to search for |
| `path` | string | -- | current dir | File or directory to search in |
| `glob` | string | -- | -- | File-name glob filter |
| `output_mode` | string | -- | `content` | `content` / `files_with_matches` / `count` |
| `-i` | bool | -- | `false` | Case-insensitive |
| `-n` | bool | -- | `true` | Show line numbers (content mode) |
| `-A` | integer | -- | -- | Lines of context after a match |
| `-B` | integer | -- | -- | Lines of context before a match |
| `-C` | integer | -- | -- | Lines of context around a match |
| `head_limit` | integer | -- | -- | Limit number of result entries |

### Return Value

`content`: matching lines as `file:line:content`. `files_with_matches`: list of files with matches. `count`: total match count. Binary/non-UTF-8 files are skipped.

## `websearch`

Runs a web search (equivalent to Claude Code `WebSearch`). Requires `[tools.websearch]` configuration.

### Arguments

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `query` | string | Yes | Search query |
| `allowed_domains` | string[] | -- | Only include results from these domains |
| `blocked_domains` | string[] | -- | Exclude results from these domains |

### Return Value

Result entries (`- <title>: <url>\n  <snippet>`) in `content`. When unconfigured, returns `ok=false` with a configuration hint.

### Configuration

```toml
[tools.websearch]
api_key_env = "TAVILY_API_KEY"
endpoint    = "https://api.tavily.com/search"
provider    = "tavily"
```

## `webfetch`

Fetches a URL, converts HTML to readable text, and returns it so the model can answer a prompt against the content (equivalent to Claude Code `WebFetch`).

### Arguments

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `url` | string | Yes | URL to fetch (`http://` is upgraded to `https://`; `file://` is supported) |
| `prompt` | string | Yes | Question to answer against the fetched content |

### Return Value

Returns `prompt: <prompt>\n\n<converted page text>` in `content`. The main conversation loop feeds this to the LLM, which answers `prompt`. On HTTP failure or non-text content, returns `ok=false`.

## Tool Disabling and Permission Control

Priority order in config/persona:

```text
[tools] enabled set
  ∩ persona.allowed_tools if specified
  \ persona.denied_tools if specified
= tools available to the agent
```

The current tool set can be checked with the REPL command `/tools`.

The pre-rename names `shell`, `fs_read`, and `fs_write` are still accepted in
`[tools] enabled` and in persona `allowed_tools` / `denied_tools`; they are
canonicalised to `bash`, `read`, and `write`. Configuration files written before
the rename therefore keep working, but new files should use the canonical names.

## Default enabled set

The default `[tools] enabled` list registers twelve tools: `bash`, `read`, `write`, `send_to`, `list_agents`, `stop_agent`, `monitor`, `edit`, `glob`, `grep`, `websearch`, `webfetch`. `spawn` is the one built-in left out, because creating processes is more impactful than the rest; `list_agents` and `stop_agent` are in, because listing is read-only and stopping cannot reach outside the caller's own tree — together they are what makes enabling `spawn` reasonable. `websearch` degrades to a clear configuration error when `[tools.websearch]` is not set.