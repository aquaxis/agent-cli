# Persona Reference (`personas.md`)

`agent-cli` assigns a "persona" to each agent it launches. A persona is described as a Markdown file (YAML frontmatter + body), allowing you to define the agent's **role, skills, description, allowed tools, model, and temperature** all in one place. This document is a reference for configuration, authoring, and runtime behavior.

Related documents:

- [`doc/config.md`](config.md) — Configuration keys such as `agents_dir` / `persona_file`
- [`doc/architecture.md`](architecture.md) — Overall architecture of the persona mechanism (section 6)
- [`doc/usage.md`](usage.md) — REPL commands `/persona` / `/reload-persona` / `/peer`

---

## 1. Overview

Personas affect the following:

| Affected area | Details |
|---------------|---------|
| System prompt | `role` / `skills` / `description` / body are concatenated under prescribed headings and injected as the leading instruction to the AI |
| Tool registry | `allowed_tools` is applied as a whitelist; `denied_tools` as a blacklist |
| Provider settings | If `model` / `temperature` are specified, they override the corresponding provider's request body at startup |
| REPL header | `name` / `role` / `skills` are displayed in the startup banner |
| `agent-cli list` output | `role` / `skills` are added as columns in the listing |
| Registry metadata file | `role` / `skills` / `description` / `source_path` are recorded in the `persona` field of `<registry_dir>/<agent-id>.json`, accessible via `/peer <id>` from other peers |

---

## 2. Configuration (Resolution Order)

The persona file resolution priority is as follows (tried top to bottom; the first hit is used):

```text
1. CLI option        --persona <path>
2. Config file       [runtime] persona_file = "<path>"
3. Filename convention  <agents_dir>/<name>.md  (matches --name)
4. Built-in default   "General-purpose assistant"
```

### 2.1 Explicit CLI option (highest priority)

```bash
agent-cli run --persona ./reviewer.md
```

- Absolute or relative paths are both accepted.
- If the file does not exist, agent-cli **exits with an error** (no fallback to a default path).

### 2.2 Config file specification

```toml
# ~/.config/agent-cli/config.toml
[runtime]
persona_file = "~/.config/agent-cli/agents/alice.md"
```

- `~`, environment variables, and relative paths are expanded (via `shellexpand`).
- If the file does not exist, agent-cli similarly exits with an error.

### 2.3 Name convention (`<agents_dir>/<name>.md`)

This is the most operationally convenient approach. Running `agent-cli run --name <name>` automatically looks for `<agents_dir>/<name>.md`.

```bash
mkdir -p ~/.config/agent-cli/agents
cp example/agents/reviewer.md ~/.config/agent-cli/agents/alice.md

agent-cli run --name alice
# -> ~/.config/agent-cli/agents/alice.md is loaded
```

The default `agents_dir` is `~/.config/agent-cli/agents`. You can change it via `[runtime] agents_dir`:

```toml
[runtime]
agents_dir = "~/projects/agent-cli/personas"
```

With this pattern, **if the file does not exist, agent-cli silently falls back to the built-in default** (unlike the explicit specification paths).

### 2.4 Built-in default

When `--persona`, `persona_file`, and `<agents_dir>/<name>.md` all miss, agent-cli launches with the following persona:

```yaml
name: default
role: General-purpose assistant
skills: [conversation, tool execution]
description: Built-in default persona
```

The body is a short instruction starting with "You are a general-purpose AI assistant running on agent-cli." This is a safe-side default for users who want to get started without any configuration.

---

## 3. Persona File Format

### 3.1 Overall file structure

```markdown
---
<YAML frontmatter>
---

<Markdown body (free-form)>
```

Requirements:

- The file must start with `---` (a leading BOM is ignored).
- A closing `---` on the next line is required.
- The frontmatter is parsed as YAML. `role` is the only required key.
- The body is trimmed and then appended to the system prompt. It may be empty.

### 3.2 Frontmatter key reference

| Key | Type | Required | Default | Description |
|-----|------|----------|---------|-------------|
| `name` | string | — | (`--name` argument, or `(unnamed)` for display) | Display name for the agent. The CLI `--name` takes precedence |
| `role` | string | **Yes** | — | Role (placed in the "# Role" section of the system prompt) |
| `skills` | string[] | — | `[]` | Skill list. Rendered as bullet items under the "# Skills" section |
| `description` | string | — | — | 1-2 line supplemental description. Also shown in `/peer` output |
| `model` | string | — | — | Overrides the provider's model name when this persona is launched |
| `temperature` | number | — | — | Same as above; sampling temperature (float, typically 0.0-2.0) |
| `allowed_tools` | string[] | — | — | Whitelist of available tools. When specified, only these tools are active |
| `denied_tools` | string[] | — | — | Blacklist of denied tools. Subtracted from `tools.enabled` |

#### Validation errors

- `role` missing or empty: `error: \`role\` is required in persona frontmatter`
- Missing opening `---` or closing `---`: `persona file must begin with YAML frontmatter (\`---\`)` or `missing closing \`---\` for YAML frontmatter`
- YAML parse error: `invalid YAML frontmatter: <serde_yaml message>`

### 3.3 Body

The body is written as Markdown, but `agent-cli` does not interpret its content -- it simply appends it to the end of the system prompt. Because you have full freedom in how you write it, it is recommended to keep instructions you want the AI to follow strictly as concise bullet points.

Example:

```markdown
You are an experienced reviewer. Always follow these rules:

- Prioritize ownership and lifetime issues above all else
- Quantify performance impacts
- When proposing fixes, aim for minimal diffs
```

---

## 4. System Prompt Composition

`Persona::to_system_prompt()` assembles a single string in the following order and sends it as the system message to the AI:

```text
<Built-in preamble:
  You are a general-purpose AI assistant running on agent-cli.
  Respond to user requests concisely and accurately, using tools as needed.>

# Role
<frontmatter.role>

# Skills
- <skills[0]>
- <skills[1]>
...

# Description
<frontmatter.description>

# Details
<body>
```

- When `skills` is an empty array, the "# Skills" section is omitted.
- When `description` is empty or unspecified, the "# Description" section is omitted.
- When the body is empty, the "# Details" section is omitted.

The prompt is stored as the leading `Message::System` in the conversation history and can be replaced via `/reload-persona`.

---

## 5. Tool Permission Control

`allowed_tools` / `denied_tools` are applied during `ToolRegistry::build` in the following order:

```text
[tools] enabled set
  -> If allowed_tools is specified, intersect with that list (whitelist)
  -> If denied_tools is specified, remove those elements from the remainder (blacklist)
= Actually enabled tools for this agent
```

### 5.1 Example

Config file:

```toml
[tools]
enabled = ["shell", "fs_read", "fs_write", "send_to"]
```

| Persona specification | Enabled tools |
|----------------------|--------------|
| None specified | `shell, fs_read, fs_write, send_to` |
| `allowed_tools: [shell, fs_read]` | `shell, fs_read` |
| `denied_tools: [fs_write]` | `shell, fs_read, send_to` |
| `allowed_tools: [shell, fs_write]` + `denied_tools: [fs_write]` | `shell` |
| `denied_tools: [send_to]` | This agent cannot send messages to other peers via `send_to` (but can still receive) |

Verify in the REPL:

```text
> /tools
tools: shell, fs_read
```

### 5.2 Security operations tips

- For "read-only" roles (code reviewers, etc.), add `denied_tools: [fs_write]`
- For a dispatcher role that "only delegates to peers", use `allowed_tools: [send_to]` only
- For any persona with `auto_approve_tools=false` (the default), each tool execution requires y/N approval from the REPL input loop (see `doc/tools.md`)

---

## 6. Model / Temperature Override

The persona's `model` / `temperature` are only overridden on the **active provider** (`provider.kind`) configuration at startup.

```yaml
---
name: alice
role: Strict code reviewer
model: claude-opus-4-7
temperature: 0.1
---
```

- Launching with `agent-cli run --provider claude --name alice` using the above persona overrides `provider.claude.model` to `claude-opus-4-7` and `provider.claude.temperature` to `0.1`.
- If launched with `--provider ollama`, the same values are written to `provider.ollama.model` / `provider.ollama.temperature` (note that model names may not be compatible across providers).
- If the CLI `--model` is also used, the CLI override is applied first, then the persona override (the persona wins in the end).

> Temperature is clamped or ignored to an appropriate range by each provider implementation. Anthropic Claude expects `0.0..=1.0`; OpenAI / Ollama expect approximately `0.0..=2.0`.

---

## 7. Complete Examples

The repository ships three examples under `example/agents/`, which are always verified to parse with the latest parser via the `bundled_example_personas_parse` unit test.

### 7.1 `coder.md` (implementer)

```markdown
---
name: coder
role: Rust software engineer
skills:
  - Rust
  - Async programming (tokio)
  - CLI design
description: An engineer focused on writing safe, readable code
allowed_tools:
  - shell
  - fs_read
  - fs_write
  - send_to
---

You are the engineer who writes agent-cli code.
- Start by making a plan; investigate the repository using `shell` and `fs_read` as needed.
- When editing files, aim for minimal diffs and respect existing style.
- Do not guess unclear specs; confirm with other agents via `send_to`.
```

### 7.2 `reviewer.md` (read-only reviewer)

```markdown
---
name: reviewer
role: Code reviewer
skills:
  - Rust
  - Static analysis
  - Security review
description: A reviewer focused on safety and performance
allowed_tools:
  - shell
  - fs_read
denied_tools:
  - fs_write
---

You are an experienced code reviewer. Always keep the following in mind:

- Prioritize ownership and lifetime issues above all else
- Quantify performance impacts
- When proposing fixes, aim for minimal diffs
```

### 7.3 `planner.md` (dispatcher)

```markdown
---
name: planner
role: Planner
skills:
  - Planning
  - Requirements analysis
  - Sub-task decomposition
description: Decomposes large tasks into sub-tasks and manages progress
---

You are the project planner.
- Decompose user requests into bullet-point sub-tasks and prioritize them.
- Delegate tasks to implementer agents via `send_to` as needed.
```

The fastest way to get started is to copy the examples as-is:

```bash
mkdir -p ~/.config/agent-cli/agents
cp example/agents/{coder,reviewer,planner}.md ~/.config/agent-cli/agents/
```

---

## 8. Operational Scenarios

### 8.1 Single agent

```bash
cp example/agents/coder.md ~/.config/agent-cli/agents/me.md
agent-cli run --name me
```

If the REPL header shows `role: Rust software engineer` or similar, the persona has been applied successfully.

### 8.2 Multi-agent coordination (planner + coder + reviewer)

```bash
# Shared registry config (common across all 3 terminals)
cat > /tmp/team.toml <<EOF
[provider]
kind = "claude"
[provider.claude]
api_key_env = "ANTHROPIC_API_KEY"
[runtime]
registry_dir = "/tmp/agent-cli/team"
agents_dir = "$HOME/.config/agent-cli/agents"
EOF

# Terminal A
agent-cli --config /tmp/team.toml run --name planner

# Terminal B
agent-cli --config /tmp/team.toml run --name coder

# Terminal C
agent-cli --config /tmp/team.toml run --name reviewer
```

From terminal A, `/list` will show B and C with their `role` / `skills`, `/peer coder` will display the coder's summary, and `/send coder "<task>"` sends an implementation request.

### 8.3 Switching roles under the same name

You can override with `--persona` each time, independently of the filename convention.

```bash
# Weekday: reviewer
agent-cli --persona ~/personas/strict_reviewer.md

# Weekend: free-form chat
agent-cli --persona ~/personas/casual_chat.md
```

### 8.4 Safe operation with a whitelist

For a CI worker that must never perform destructive operations:

```yaml
---
name: ci-worker
role: CI helper
allowed_tools:
  - fs_read
---
```

Even if `tools.enabled` contains more entries, this agent will only see `fs_read`.

---

## 9. REPL Operations

| Command | Purpose |
|---------|---------|
| `/persona` | Show your own persona details (`name` / `role` / `skills` / `description` / `temperature` / `allowed_tools` / `denied_tools` / `source`) |
| `/reload-persona` | Reload via the same resolution path and replace the system prompt (conversation history is preserved) |
| `/peer <id_or_name>` | Show a peer's persona summary (`role` / `skills` / `description`) |
| `/tools` | List the actually enabled tools for this agent (after applying persona restrictions) |

`/reload-persona` behavior:

1. Re-resolves using the same priority as at startup
2. Replaces the system prompt (`Message::System`) with the new content
3. Preserves `User` / `Assistant` / `ToolResult` messages in the conversation history
4. The tool registry is not rebuilt (to reflect changes to `allowed_tools` / `denied_tools`, restart agent-cli)
5. `model` / `temperature` overrides are also only applied on restart

---

## 10. Registry Reflection

The `persona` field in `<registry_dir>/<agent-id>.json`:

```json
{
  "id": "agent-01HX...",
  "name": "alice",
  "provider": "claude",
  "model": "claude-opus-4-7",
  "socket": "/tmp/agent-cli/agent-01HX....sock",
  "persona": {
    "role": "Code reviewer",
    "skills": ["Rust", "Static analysis", "Security review"],
    "description": "A reviewer focused on safety and performance",
    "source_path": "/home/.../agents/alice.md"
  }
}
```

This allows another process to check a peer's role, skills, and description simply by running `/peer alice`. It is also useful for selecting `/send` destinations.

---

## 11. Troubleshooting

### `persona file not found: <path>`

- The path specified by `--persona` or `[runtime] persona_file` does not exist.
- When using the default path (`<agents_dir>/<name>.md`), agent-cli falls back to the built-in default, so this message will not appear.
- Resolution: Verify the path with `agent-cli config show`, or convert a relative `--persona` path to an absolute path.

### `` `role` is required in persona frontmatter ``

- The frontmatter is missing `role:`, or the value is an empty string.
- Resolution: Always set `role: <string>`. Minimal example:
  ```yaml
  ---
  role: General-purpose assistant
  ---
  ```

### `persona file must begin with YAML frontmatter (\`---\`)`

- The file does not start with `---`. Files containing only a body are not supported.
- Resolution: Add `---\nrole: ...\n---\n` at the top.

### `invalid YAML frontmatter: ...`

- The frontmatter is invalid YAML (broken indentation, unquoted colons, missing `- ` for lists, etc.).
- Resolution: Validate the YAML first using something like `python -c 'import yaml,sys;yaml.safe_load(sys.stdin)'`.

### `/reload-persona` does not change tool permissions

- `allowed_tools` / `denied_tools` replacement does not currently rebuild `ToolRegistry`, so restart agent-cli.
- The system prompt is reflected immediately.

### REPL header shows a different `role`

- The filename convention (`<agents_dir>/<name>.md`) may have been overridden by CLI `--persona`. Check the `source` line in `/persona` to see which file was actually loaded.

### I want to customize the example personas

- Copy `example/agents/*.md` to `~/.config/agent-cli/agents/` and they will be loaded by `--name`. Because the original examples may be overwritten by future updates, always copy them to a different name.

---

## 12. Specification Summary (Cheat Sheet)

```text
Resolution order: --persona > [runtime] persona_file > <agents_dir>/<name>.md > builtin
Required key:     role
Optional keys:    name / skills / description / model / temperature / allowed_tools / denied_tools
Tool selection:   enabled ∩ allowed_tools \ denied_tools
Model override:   Startup only. Not reflected by /reload-persona
Temp. override:   Startup only. Same as above
REPL:            /persona, /reload-persona, /peer <id>, /tools
Registry:        Reflected in the persona field of <registry_dir>/<agent-id>.json
Validation:       cargo test bundled_example_personas_parse continuously verifies example/agents/*.md
```