# Architecture Overview (`architecture.md`)

This is a summary of `AI_PRJ_DESIGN.md`. Read it as a map for implementation.

## 1. Big Picture

```text
+--------------------+        +--------------------+
| agent-cli (proc A) |        | agent-cli (proc B) |
|  - 1 AI agent      |        |  - 1 AI agent      |
|  - REPL front-end  |        |  - REPL front-end  |
|  - Tools registry  |        |  - Tools registry  |
|  - IPC server      |<------>|  - IPC server      |
|  - IPC client      | local  |  - IPC client      |
+----------+---------+  IPC   +----------+---------+
           |                              |
           v                              v
       AI Provider API              AI Provider API

Registry directory:
  $XDG_RUNTIME_DIR/agent-cli/   or /tmp/agent-cli/
    └─ <agent-id>.sock   ... IPC socket for each process
    └─ <agent-id>.json   ... metadata (name/provider/model/persona/...)
```

- 1 process = 1 agent.
- Inter-process communication uses **local Unix domain sockets** (`0600`). No publicly open ports.
- HTTP communication to backends is independent per process.

## 2. Module Structure

```text
src/
├── main.rs              ... CLI entry point / subcommand dispatch / definitive exit via std::process::exit
├── cli.rs               ... clap argument definitions
├── app.rs               ... `run` REPL body / run_input_loop / PromptState / handle_auto_command / wait_for_termination_signal
├── agent.rs             ... single agent conversation loop / ApprovalRequest / request_approval
├── commands.rs          ... list/send/providers/doctor/selftest/config
├── config.rs            ... config file loading / resolution order
├── id.rs                ... AgentId
├── history.rs           ... opt-in history-window mgmt (estimate_tokens/old_span/render_transcript)
├── persona.rs           ... persona (YAML + body)
├── log.rs               ... conversation log
├── error.rs             ... AppError
├── ai/
│   ├── mod.rs           ... Provider trait, build()
│   ├── claude.rs        ... Anthropic Messages (SSE, thinking, tool_use)
│   ├── codex.rs         ... OpenAI Chat Completions (SSE, function calling)
│   ├── ollama.rs        ... Ollama /api/chat (NDJSON, tool_calls)
│   ├── opencode.rs      ... OpenCode local session API / Zen cloud (OpenAI-compatible)
│   ├── llamacpp.rs      ... llama.cpp /v1/chat/completions (OpenAI-compatible)
│   ├── tool_bridge.rs   ... tool definition format conversion
│   └── stream.rs        ... SSE frame assembly
├── tools/
│   ├── mod.rs           ... Tool trait, ToolRegistry
│   ├── shell.rs
│   ├── fs_read.rs
│   ├── fs_write.rs
│   └── send_to.rs
└── ipc/
    ├── mod.rs           ... IpcMessage
    ├── server.rs        ... UnixListener (0600) / Drop performs accept abort + socket deletion
    ├── client.rs        ... UnixStream
    └── registry.rs      ... <agent-id>.{sock,json} scan / Drop performs automatic cleanup
```

Key types:

- `Agent.auto_approve: Arc<AtomicBool>` -- toggled at runtime with `/auto on|off`
- `Agent.approval_tx: Option<mpsc::Sender<ApprovalRequest>>` -- approval request path to the input loop
- `enum PromptState { Ready, Pending, AwaitingApproval(oneshot::Sender<bool>) }` -- REPL input loop state

## 3. Core Data Flows

### 3.1 User Prompt Processing

```text
stdin -> run_input_loop -> mpsc -> Agent loop -> Provider -> ProviderEvent stream
            ^                          |
            |                          +-- text_delta -> mpsc -> display task -> stdout
            |                          +-- thinking   -> mpsc -> display task -> stdout
            |                          +-- tool_use   -> approval (3.3) -> ToolRegistry -> ToolOutput
            |                          +-- Done       -> mpsc -> display task -> agent_idle notification -> input loop
            |
            +-- On receiving agent_idle, transitions Pending -> Ready and redraws the next prompt `> `
```

- `run_input_loop` holds `enum PromptState { Ready, Pending, AwaitingApproval(oneshot::Sender<bool>) }` and multiplexes 4 channels (shutdown / idle / approval / stdin) via `tokio::select!`.
- Immediately after sending user input, it transitions to `Pending` and suppresses stdin reads until `Done` is received (via `mpsc::<()>` from `display_task`). This prevents interleaving of streaming output and input echo.
- When `[history] enabled = true`, `process_turn` calls `maybe_compact_history` **before** the provider call: if estimated tokens (≈ chars/4) exceed `max_context_tokens`, the old span is summarized by a no-tool provider call into one system message, then oldest messages are dropped if still over budget. Best-effort (failure → drop-only, never fails the turn); disabled by default → full history replayed verbatim. See §8 and `doc/config.md` §11.3.
- Tool execution iterates up to `[runtime] max_tool_iterations` (default 24, minimum 1, maximum `u32::MAX`). See `self.config.runtime.max_tool_iterations.max(1)` in `agent.rs::process_turn`. This is a guard mechanism to prevent infinite loops. When `auto_approve_tools=false` (default), y/N confirmation is obtained via the approval channel described in 3.3.
- On reaching the limit: If the AI continues returning `tool_use` after exhausting the configured number of iterations, the loop exits and issues `AgentEvent::Info { message: "max tool-use iterations reached" }` followed by `AgentEvent::Done` in this order. Notification goes through the Info channel rather than the Error channel (since it means "not converged" rather than "abnormal"). The REPL treats it the same as a normal `Done` and redraws the next input prompt. For meaning, mitigation, and recommended ranges, see `doc/troubleshooting.md` / `doc/config.md`.
- `Done` is always issued not only on normal response completion but also when `provider.complete_stream` fails, ensuring the input loop never gets stuck in Pending state.
- At startup, `display_task` resolves `ShowThinkingMode { Hidden, Collapsed, Expanded }` from `config.ui.show_thinking_mode()` and branches `AgentEvent::Thinking` rendering across 3 modes (FR-03-1-2 / Design doc 4.3C). `Hidden` skips rendering, `Collapsed` truncates to "first 80 chars + line 1" via `collapse_thinking_text()`, and `Expanded` shows full text. Setting changes take effect on restart; there is no runtime toggle.

### 3.2 Inter-Peer Messaging

```text
proc A                                          proc B
------                                          ------
/send bob "hi" or send_to tool
   │
   ▼
ipc::client::send (UnixStream)
   │ JSONL: {"kind":"prompt","from":"<A id>","text":"hi"}
   ▼
                                          UnixListener
                                              │
                                              ▼
                                        IpcMessage::Prompt
                                              │
                                              ▼ mpsc
                                       AgentInput::PeerPrompt
                                              │
                                              ▼
                                       Agent loop (B)
                                              │
                                              ▼
                                      Provider response -> screen display
```

### 3.3 Tool Execution Approval I/O Integration

Approval is handled via a two-channel path between the agent task and the input loop (direct reads from `std::io::stdin` are prohibited).

```text
Agent::process_turn (auto_approve=false)
   │
   ├── ApprovalRequest { tool_name, args, response: oneshot::Sender<bool> }
   │       │
   │       ▼ mpsc::Sender<ApprovalRequest>
   │   run_input_loop  (transitions to PromptState::AwaitingApproval(resp_tx))
   │       │
   │       │ Draws "[tool approval] ... approve? [y/N]:"
   │       │
   │       ▼ reads the next line from stdin
   │   y/yes -> resp_tx.send(true), otherwise -> false
   │       │
   │       ▼ oneshot::Receiver<bool>
   └── based on approval result, executes tool or "user denied tool execution"
```

- `auto_approve` is shared between the agent and REPL via `Arc<AtomicBool>` and can be toggled at runtime with the REPL command `/auto on|off|status`.
- While awaiting approval (`AwaitingApproval`), if a shutdown signal arrives, `resp_tx.send(false)` provides a fail-safe default, and the agent's `oneshot::Receiver::await` resolves immediately, preventing any dangling waits.

## 4. Registry Specification

`<registry_dir>/<agent-id>.json`:

```json
{
  "id":"agent-01HX...",
  "name":"alice",
  "pid":12345,
  "started_at":"2026-05-01T10:00:00Z",
  "provider":"claude",
  "model":"claude-opus-4-7",
  "socket":"/tmp/agent-cli/agent-01HX....sock",
  "persona": {"role":"...","skills":[...],"description":"...","source_path":"..."}
}
```

During scanning:

- Reads `*.json` and verifies the corresponding `*.sock` exists
- Confirms PID liveness via `/proc/<pid>` existence
- If either is missing, treats as stale and cleans up `<agent-id>.{sock,json}`

## 5. Provider Abstraction

```rust
#[async_trait]
trait Provider {
    fn name(&self) -> &'static str;       // "claude" | "codex" | "ollama" | "opencode" | "llama.cpp"
    fn capabilities(&self) -> Capabilities;
    fn model(&self) -> &str;
    async fn complete_stream(&self, messages: &[Message], tools: &[ToolSpec])
        -> Result<EventStream<'_>>;
}

enum ProviderEvent {
    Thinking { text: String },
    Text     { delta: String },
    ToolUse  { id: String, name: String, args: Value },
    Done,
    Error    { message: String },
}
```

Each backend normalizes its internal representation into the same `ProviderEvent` sequence before passing it upstream.

## 6. Persona Mechanism

Priority order:

```text
1. --persona <path>
2. [runtime] persona_file
3. <agents_dir>/<name>.md
4. Built-in default (general-purpose assistant)
```

The persona's `role` / `skills` / body text are synthesized into the system prompt. `allowed_tools` / `denied_tools` are reflected in `ToolRegistry::build`, and the result can be confirmed with `/tools`. `model` / `temperature` override the corresponding provider's request body at startup (not reflected on reload). Reload is done via the REPL command `/reload-persona` (preserves conversation history).

For configuration methods, frontmatter keys, writing examples, and operational scenarios, see [`doc/personas.md`](personas.md).

## 7. Shutdown Coordination

Regardless of the trigger -- `/quit` / `/exit` / `Ctrl+D` (EOF) / `Ctrl+C` (SIGINT) / `SIGTERM` -- all paths converge to the same shutdown sequence.

```text
[/quit /exit handler]   [stdin EOF detected]   [SIGINT/SIGTERM handler]
              \              |              /
               \             v             /
                +-- shutdown_tx.send(true) (tokio::sync::watch) --+
                                    │
                                    ▼
        ┌─────────────────────────────────────┐
        │ stdin_task.abort()                  │
        │ ipc_task.abort()                    │
        │ signal_task.abort()                 │
        │ drop(input_tx)                      │
        │ agent_handle (500ms timeout)        │
        │ display_task.await                  │
        │ drop(ipc_server)  -> IpcServer::Drop│
        │   - accept loop abort               │
        │   - <id>.sock deletion               │
        │ registry_handle (RegistryHandle::Drop)│
        │   - <id>.sock / <id>.json deletion  │
        └─────────────────────────────────────┘
                                    │
                                    ▼
                          std::process::exit(0)
```

- `IpcServer` and `RegistryHandle` perform abort + file deletion in their `Drop` implementations, ensuring no remnants remain even on panic.
- `main` explicitly calls `std::process::exit(0/1)` to avoid the tokio runtime drop waiting for the `tokio::io::stdin()` blocking thread.
- On development machines, all 5 paths confirmed normal termination within 1 second with no registry remnants (`/quit` 110ms / `/exit` 110ms / `Ctrl+D` 110ms / `SIGINT` 19ms / `SIGTERM` 3ms).
- When awaiting approval (`AwaitingApproval`), on input loop break, `oneshot::Sender::send(false)` provides a fail-safe default (see 3.3).

## 8. Context-efficiency Features (opt-in)

agent-cli replays full history to the provider on every send (the provider
APIs are stateless; only the pooled TCP/TLS socket persists). Three opt-in,
default-OFF features reduce cost/latency without changing the per-send model.
Full reference: [`doc/config.md`](config.md) §11.

| Feature | Config | Where | Effect |
|---------|--------|-------|--------|
| Claude prompt caching | `[provider.claude] prompt_cache` | `ai/claude.rs::apply_prompt_cache` | Adds `cache_control` to system / last tool / last message block; repeated prefix served from Anthropic's cache |
| opencode persistent session | `[provider.opencode] persistent_session` | `ai/opencode.rs` (`PersistState`, `complete_stream_local_persistent`) | Local mode only: reuse one server `session_id`, send only new user/tool turns; reset on `/clear` or system-prompt change; one stale-session retry |
| Hybrid history-window mgmt | `[history]` | `history.rs` + `agent.rs::maybe_compact_history` | Summarize old span → drop oldest if still over budget; keeps system prefix + recent N turns |

All three are independent and additive; with every flag off the request
bodies and history handling are byte-for-byte unchanged.

## 9. Target OS

Linux only. The implementation assumes Unix domain sockets, `XDG_RUNTIME_DIR`, `/proc/<pid>`, and `tokio::signal::unix`.