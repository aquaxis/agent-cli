# Codex Code Backend

This backend drives the locally installed **Codex CLI** (`codex`) as an
`agent-cli` provider. `agent-cli` spawns `codex exec` once per turn, feeds it
the turn's prompt, and maps its JSONL output onto the usual `ProviderEvent`
stream — so the REPL, personas, peer IPC, conversation logging, and custom
slash commands wrap around Codex.

It has no API key of its own: it inherits whatever authentication Codex
already has on the machine (e.g. a ChatGPT login).

> Not to be confused with the [`codex` backend](./codex.md), which is a direct
> HTTPS client for the OpenAI Chat Completions API and never executes the
> Codex CLI.

## Prerequisites

- Codex CLI installed and on `PATH` (or `bin` pointed at it)
- Codex already authenticated (`codex login status` shows "Logged in")

Verified against Codex CLI `0.166.0`. The sandbox default assumes the CLI's
own default is read-only, which is why this backend defaults to
`workspace-write`.

## Configuration

```toml
[provider]
kind = "codex-code"

[provider.codex-code]
bin       = "codex"        # executable name (resolved via PATH) or full path
model     = "gpt-5.6-terra"  # -m; omit for Codex's own default
transport = "items"        # "items" (--json item stream) | "oneshot"
session   = "persistent"   # "persistent" | "ephemeral"
sandbox   = "workspace-write"  # -s: read-only | workspace-write | danger-full-access
turn_timeout_secs = 900
# add_dir                        = []  # --add-dir, repeated per entry
# approval_policy                = "never"  # -c approval_policy=...
# bypass_sandbox_and_approvals   = false    # --dangerously-bypass-approvals-and-sandbox
# skip_git_repo_check            = true     # --skip-git-repo-check
# extra_args                     = []       # passed through verbatim
```

Every key is optional. The whole section may be omitted, in which case the
defaults above apply.

| Key | Default | Effect |
|-----|---------|--------|
| `bin` | `"codex"` | Executable. A value containing a path separator is used as-is; otherwise it is looked up on `PATH`. A missing binary fails at startup. |
| `model` | unset | `-m`. Also settable with `agent-cli run --model`. Omit to use Codex's own default (its `~/.codex/config.toml`). |
| `transport` | `"items"` | `"items"` streams `--json` items as they happen (turn-grain, not token-grain); `"oneshot"` buffers the whole turn and yields the final reply once, read back from the `-o/--output-last-message` file, which is deleted after the turn. |
| `session` | `"persistent"` | `"persistent"` opens a thread on the first turn and resumes it (`codex exec resume <thread_id>`) afterwards; `"ephemeral"` adds `--ephemeral` every turn so Codex persists no session files and each conversation stands alone. |
| `sandbox` | `"workspace-write"` | `-s`. Codex's own default is `read-only`, which would block the tools delegation needs, so the backend overrides it. |
| `add_dir` | `[]` | `--add-dir`, one flag per entry: additional directories Codex may write. |
| `approval_policy` | unset | `-c approval_policy=...`. |
| `bypass_sandbox_and_approvals` | `false` | `--dangerously-bypass-approvals-and-sandbox`. |
| `skip_git_repo_check` | `true` | `--skip-git-repo-check`: without it Codex refuses to run outside a trusted git directory. Set `false` to honour Codex's own trust gate. |
| `turn_timeout_secs` | `900` | Per-turn wall clock. On expiry the child is killed, the recorded thread is discarded, and the turn ends with an error; the next turn starts a fresh thread. |
| `extra_args` | `[]` | Extra flags appended verbatim. |

## Delegation only

Codex runs its own tools inside its own sandbox. Its `command_execution` items
arrive **already executed**, so they are never converted into agent-cli tool
calls — the backend reports `tool_use = false` — and agent-cli's approval
prompt is not involved. Control what Codex may do with `sandbox`, `add_dir`,
`approval_policy`, `bypass_sandbox_and_approvals`, and `skip_git_repo_check`.

Consequently there is **no gateway mode**: `claude -p`'s `--tools ""` has no
Codex counterpart — `codex exec` runs its own agent and accepts no external
tool list, so agent-cli's tools cannot be offered to it.

## Process model

Every turn is one `codex exec` spawn; there is **no resident child**. The
prompt rides stdin (the `-` marker) in the `items` transport and the argv tail
in `oneshot`; either way the pipe is closed once written, because a held-open
stdin keeps the run waiting for a prompt that never completes.

| transport / session | Behaviour |
|---|---|
| `items` + `persistent` | First turn: `codex exec --json ... -`, whose `thread.started` line pins the thread id. Later turns: `codex exec --json ... resume <thread_id> -`, sending only the user turns Codex has not seen — the transcript is never re-sent, Codex holds the history. |
| `items` + `ephemeral` | `codex exec --json ... --ephemeral -` every turn; the conversation is flattened and re-sent each time; nothing persists. |
| `oneshot` (either session) | One child per turn, the whole turn buffered, the reply read back from the `-o` file; sessionless by design — no thread id is ever seen. |

The child is killed when `agent-cli` exits or a turn times out, so no orphan
process is left behind.

### Recovering a lost thread

A resume leg whose thread no longer exists (Codex: `no rollout found for
thread id ... (code -32600)`) clears the recorded thread id: the surfaced
error still says what happened, and the **next turn** re-enters fresh. A
codex-side failure that does not match that signature (a bad model, the trust
gate) never discards a perfectly good thread.

## Supported Features

| Feature | Support | Notes |
|---------|---------|-------|
| Streaming | ✓ (`transport = "items"`) | Item-granular: text arrives **whole per completed item**, not as token deltas — `oneshot` delivers the reply at end of turn |
| Tool use | ✗ as agent-cli tools | Tools are Codex's own, executed inside it; agent-cli's registry cannot be handed to the child (no `--tools ""` counterpart) |
| Thinking | ✗ | Not surfaced by the probed CLI |
| Approval prompt | ✗ | Codex owns permission decisions; control with `sandbox`/`approval_policy`/`bypass_sandbox_and_approvals` |
| agent-cli tool registry | ✗ | Cannot be offered to the child |

The one knob that differs in meaning from claude-code's `system_prompt_mode`:
**there is no system-prompt flag** on the probed CLI, so the persona rides the
turn payload (the system text is prepended to the prompt of a fresh leg, and
a persona change re-enters through a fresh conversation).

## Verification

```bash
agent-cli providers          # shows the resolved codex binary
agent-cli doctor             # with kind = "codex-code": checks binary + --version
agent-cli selftest --provider codex-code
```

## Known Limitations

- The JSONL event shapes (`thread.started` / `turn.started` / `item.*` /
  `turn.failed` / `turn.completed`) are an implementation detail of the Codex
  CLI, not a stable interface. Parsing ignores unknown line types and
  unparsable lines, but a future release could still change what is emitted.
  Pinned observations are from `0.166.0`.
- Turn granularity is the item, not the token: long replies appear at the
  moment their item completes, so the display reads calmer than
  token-streamed backends.
- In delegation mode agent-cli's history window management and logs describe
  only what passed through agent-cli; Codex's own thread holds the rest (and
  replays it against the model on every resume, which is Codex-side cost).
- Usage figures (`turn.completed`'s `usage`) are parsed for tokens-observed
  accounting, not surfaced to the REPL.

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `Codex CLI executable not found` | `codex` not on `PATH` | Install the Codex CLI, or set `bin` to its full path |
| `codex-code: turn timed out` | Turn exceeded `turn_timeout_secs` | Raise the limit, or shorten the task |
| `codex-code: ended without a result` | The CLI refused the invocation, or the turn failed mid-stream | Read the quoted stderr tail; check the model name and `extra_args` |
| `no rollout found for thread id ...` | The recorded thread was lost (deleted sessions, `--ephemeral`) | Nothing to fix: the next turn re-enters fresh automatically |
| `writing is blocked by read-only sandbox` | `sandbox = "read-only"` (Codex's own default) | Set `sandbox = "workspace-write"` |
| `Not inside a trusted directory` | `skip_git_repo_check = false` and the working directory is not trusted | Trust the directory (`codex`), or set `skip_git_repo_check = true` |
