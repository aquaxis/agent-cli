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
├── app.rs               ... `run` REPL body / run_input_loop (raw + line mode) / PromptState / slash-command dispatch / prompt + candidate rendering / Tab completion / wait_for_termination_signal
├── editor.rs            ... input buffer and history cursor (InputState) / display-width math
├── custom_commands.rs   ... `.md` custom slash command discovery / `@file` + `$ARGUMENTS` expansion
├── agent.rs             ... single agent conversation loop / ApprovalRequest / request_approval
├── commands.rs          ... list/send/ask/providers/doctor/selftest/config
├── config.rs            ... config file loading / resolution order
├── id.rs                ... AgentId
├── history.rs           ... opt-in history-window mgmt (estimate_tokens/old_span/render_transcript)
├── persona.rs           ... persona (YAML + body)
├── log.rs               ... conversation log
├── error.rs             ... AppError
├── ai/
│   ├── mod.rs           ... Provider trait, build()
│   ├── claude.rs        ... Anthropic Messages (SSE, thinking, tool_use)
│   ├── claude_code.rs   ... Claude Code CLI as a child process (stream-json / --print JSON)
│   ├── codex.rs         ... OpenAI Chat Completions (SSE, function calling)
│   ├── ollama.rs        ... Ollama /api/chat (NDJSON, tool_calls)
│   ├── opencode.rs      ... OpenCode local session API / Zen cloud (OpenAI- or Anthropic-compatible via `api`)
│   ├── llamacpp.rs      ... llama.cpp /v1/chat/completions (OpenAI-compatible)
│   ├── tool_bridge.rs   ... tool definition format conversion
│   └── stream.rs        ... SSE frame assembly
├── tools/
│   ├── mod.rs           ... Tool trait, ToolRegistry
│   ├── bash.rs          ... bash command execution
│   ├── read.rs          ... line-numbered file read
│   ├── write.rs         ... file write
│   ├── edit.rs          ... exact string replacement
│   ├── glob.rs          ... glob pattern file search
│   ├── grep.rs          ... regex content search
│   ├── monitor.rs       ... long-running command monitor
│   ├── websearch.rs     ... web search (config-driven)
│   ├── webfetch.rs      ... URL fetch + HTML-to-text
│   └── send_to.rs       ... peer prompt delivery
└── ipc/
    ├── mod.rs           ... IpcMessage (Prompt / PromptReply / Ack / Error / Ping / Pong / Shutdown)
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
- It has two front ends. With a TTY it runs `run_input_loop_raw`: crossterm raw mode, key events handled by `handle_key`, edit buffer and history cursor in `editor.rs`, prompt redrawn with the command candidates on the row above it, and `Tab` completion resolved against the same candidate list (`command_completion`). Otherwise (pipe, redirect, tests) it runs `run_input_loop_line` over `BufReader::lines()`. Both feed the same `AgentInput` channel, so tool use, approval, and custom commands behave identically in either mode.
- Immediately after sending user input, it transitions to `Pending` and suppresses stdin reads until `Done` is received (via `mpsc::<()>` from `display_task`). This prevents interleaving of streaming output and input echo, and it is also why piped input reaches EOF only after the in-flight turn has finished.
- Input starting with `/` is dispatched by `handle_repl_command`: built-in commands first, then an exact custom-command match, then a unique prefix match (auto-executed and reported as `[auto] /<typed> → /<resolved>`); several matches list the candidates. A resolved custom command is expanded by `custom_commands::expand_template` and entered into the same `AgentInput::UserPrompt` path as a typed prompt.
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

**Reply path.** `IpcMessage::Prompt` carries an optional `reply_to` socket path.
When it is set, agent B sends `IpcMessage::PromptReply { from, text }` to that
socket once its response is complete, instead of only acknowledging receipt:

```text
proc A                                          proc B
------                                          ------
agent-cli ask bob "..."  /  send_to { wait_reply: true }
   │ binds a temporary reply socket
   ▼
{"kind":"prompt", ..., "reply_to":"<tmp>/reply.sock"}   ──►  Ack
                                                             │
                                              Agent loop (B) ┘
                                                             │
   temporary UnixListener  ◄── {"kind":"prompt_reply","text":"..."} ─┘
   │
   ▼ prints the response text (ask) or returns it as tool output (send_to)
```

Two callers use it: the `ask` subcommand (`commands::ask_and_receive`, default
timeout 120 s, `--timeout` to change) and the `send_to` tool with
`wait_reply = true`. With `reply_to` absent the flow is fire-and-forget, which
is what `/send` and `agent-cli send` use.

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

## 7.1 Detached agents (`spawn` / `serve` / `stop`)

`app::run` is the interactive front end; `app::run_headless` is its
non-interactive sibling. `run_headless` builds the same provider / IPC server /
registry / agent / display stack but omits the stdin input loop — a detached
process has `stdin` at `/dev/null`, and `run` treats stdin EOF as shutdown, which
would kill it. Having no console to answer tool approval, `serve` forces
`auto_approve = true` and builds the agent with `approval_tx: None`.

```text
 parent (interactive `run`, or one-shot `spawn`)
   │ commands::spawn_detached()
   │   current_exe()  --config <same>  serve  [--name…]
   │   pre_exec(setsid) + stdin/out/err = /dev/null + drop child (no wait, no kill_on_drop)
   ▼
 child (`serve` → app::run_headless): self-registers in the SHARED registry_dir,
   serves peers over IPC, shuts down on SIGINT | SIGTERM | IpcMessage::Shutdown
```

Independence follows from three facts: `setsid` isolates the child from the
parent's terminal signals; stdio is detached; and the parent never holds the
child with `kill_on_drop` nor `wait()`s on it, so dropping the handle leaves it
running (a spawned agent-cli already has no lifetime tie to its launcher). `stop`
resolves the peer and sends `IpcMessage::Shutdown` (the receiver Acks it and
converges on the §7 shutdown), falling back to `SIGTERM` by the registered pid.

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

## 8.1 Child-process backend (`claude-code`)

Every other backend is an HTTP client. `claude-code` instead spawns the local
Claude Code CLI and talks to it over pipes, which puts two things outside
agent-cli's control:

- **Tools.** In the default `delegation` mode Claude Code executes its own
  tools and reports them afterwards, so `ai/claude_code.rs` never converts a
  `tool_use` block into `ProviderEvent::ToolUse` — doing so would make
  `agent.rs` run the same command a second time. The approval flow of §3.3 is
  therefore not exercised by this backend. In `gateway` mode (`--tools ""`) no
  tools run at all: `claude -p` accepts no external tool definitions, so
  agent-cli's registry cannot be offered either.
- **Process lifetime.** `delegation` + `stream` + `session = "persistent"`
  keeps one resident child for the conversation, fed one JSON line per turn and
  guarded by a per-turn timeout; every other combination spawns one child per
  turn. Children are spawned with `kill_on_drop`, so agent-cli's exit paths in
  §7 leave no orphan.

Text and thinking deltas arrive as Anthropic-shaped SSE wrapped in
`stream_event`, so `ai/claude.rs::handle_frame` is reused after one unwrap.
See [`doc/providers/claude-code.md`](providers/claude-code.md).

## 9. Target OS

Linux only. The implementation assumes Unix domain sockets, `XDG_RUNTIME_DIR`, `/proc/<pid>`, and `tokio::signal::unix`.