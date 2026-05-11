# Tool Reference (`tools.md`)

Describes the argument schemas, return values, limitations, and approval flow for `agent-cli` built-in tools.

## Common Specifications

- Tools are called by the AI with JSON-formatted input.
- Return values are represented as `{"ok": bool, "content": string}` in `ToolOutput`, where `content` is passed back to the AI. Tools that return structured results, such as `shell`, embed a JSON string in `content`.
- Approval flow: When `auto_approve_tools=false` (default), a y/N prompt is obtained via the REPL input loop before execution. See "Tool Execution Approval" below for details. When denied, `user denied tool execution` is returned to the AI.
- Available tools can be controlled via the persona's `allowed_tools`/`denied_tools` (see `doc/config.md`).

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

## `shell`

Executes a shell command.

### Arguments

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `cmd` | string | Yes | -- | Command body to execute (runs via `bash -lc <cmd>`) |
| `cwd` | string | -- | Process cwd | Working directory |
| `timeout_secs` | integer | -- | `[tools.shell] timeout_secs` (default 60) | Per-invocation timeout |

### Return Value (`content` is a JSON string)

```json
{
  "exit_code": 0,
  "stdout": "...",
  "stderr": "..."
}
```

When `stdout`/`stderr` exceeds `[tools.shell] max_output_kb`, `...[truncated]` is appended.

### Limitations

- Runs via `bash -lc`, so `bash` must be installed.
- On timeout, returns `ok=false` with `timed out after <N> seconds: <cmd>`.
- When `auto_approve_tools=false` (default), interactive y/N approval is required (see "Tool Execution Approval" above). Can be disabled for the session with `/auto on`.

### Example

```json
{"name":"shell","arguments":{"cmd":"ls /tmp"}}
```

## `fs_read`

Reads a UTF-8 text file.

### Arguments

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `path` | string | Yes | Target path. Expands `~` and environment variables |
| `offset` | integer | -- | Byte offset to start reading from |
| `limit` | integer | -- | Number of bytes to read |

### Return Value

Returns UTF-8 text in `content`. For binary or non-UTF-8 files, returns `ok=false` with `binary or non-UTF-8 file: <path>`.

### Example

```json
{"name":"fs_read","arguments":{"path":"./Cargo.toml","limit":1024}}
```

## `fs_write`

Writes UTF-8 text to a file.

### Arguments

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `path` | string | Yes | Destination path |
| `content` | string | Yes | Content to write |
| `overwrite` | bool | -- | When `false` (default), returns `ok=false` if the file already exists |

### Return Value

On `ok=true`, returns `wrote <path>` in `content`.

### Notes

- Parent directories are created automatically (`mkdir -p`).
- Overwrite is denied by default, preventing the AI from accidentally clobbering existing files.
- Binary writes are not supported.

## `send_to`

Sends a prompt to an agent in another process (a peer).

### Arguments

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `peer` | string | Yes | Destination agent-id or display name |
| `text` | string | Yes | Prompt to send |

### Return Value

On success, returns `delivered to <agent-id>` in `content`. On failure, returns an error message (e.g., `peer not found by id or name: ...`).

### Example

```json
{"name":"send_to","arguments":{"peer":"alice","text":"Please review this"}}
```

### Notes

- Destination resolution scans `<agent-id>.json` files under `registry_dir`.
- This is asynchronous (it does not wait for a response); success is acknowledged upon receipt of the Ack.
- On the receiving agent side, the prompt is prefixed with `[peer prompt from <agent-id>]` and passed to the AI as user input.

## Tool Disabling and Permission Control

Priority order in config/persona:

```text
[tools] enabled set
  ∩ persona.allowed_tools if specified
  \ persona.denied_tools if specified
= tools available to the agent
```

The current tool set can be checked with the REPL command `/tools`.