# aiprj - AI Project Management Tool

A project management tool for Claude Code. It deploys AI operation guidelines and document structure (requirements, design, tasks) to a target directory with a single command.

## Overview

aiprj provides the following features:

- Definition of AI operation guidelines and rules
- Document structure for requirements, design specifications, and task lists
- Slash commands for Claude Code (`/setup_ai` `/ai` `/update_ai` `/next_ai` `/close_ai`)
- Automatic work log saving (`.aiprj/AI_LOG/yyyy-MM-dd_NNN.md`)

## Setup

### Setup in current directory

```bash
curl -fsSL https://raw.githubusercontent.com/aquaxis/aiprj/main/install.sh | sh
```

### Setup in a specified directory

```bash
curl -fsSL https://raw.githubusercontent.com/aquaxis/aiprj/main/install.sh | sh -s -- <directory_name>
```

### Manual setup

```bash
git clone https://github.com/aquaxis/aiprj.git
cd aiprj
./install.sh <target_directory>
```

The following files are created by setup:

- `.aiprj/` - AI rules, `instructions.md`, `README.md`
- `.claude/` - Claude Code settings and slash commands
- `.mcp.json` - MCP server configuration
- `.gitignore` - Git ignore configuration (prepended to existing file if present)

### Claude Code Slash Commands

| Command | Description |
|---------|-------------|
| `/setup_ai` | Create project documents (requirements, design, tasks) |
| `/ai` | Execute tasks based on `instructions.md` |
| `/update_ai` | Update project documents |
| `/next_ai` | Proceed to next task |
| `/close_ai` | Save work log and exit |

## Project Structure

After setup, the AI manages the following documents:

| File | Content |
|------|---------|
| `.aiprj/AI_PRJ_REQUIREMENTS.md` | Requirements document |
| `.aiprj/AI_PRJ_DESIGN.md` | Design specification document |
| `.aiprj/AI_PRJ_TASKS.md` | Implementation tasks and work instruction list |
| `.aiprj/AI_LOG/` | Work logs (`yyyy-MM-dd_NNN.md` format, sequential, no overwriting) |

## AI Operation Guidelines

The AI operates according to the following guidelines:

1. Must formulate a work plan before starting any task
2. Distorting or reinterpreting the AI Operation Guidelines is prohibited
3. Taking detours or modifying the approach beyond user instructions is prohibited
4. Optimizing, rewriting, or reinterpreting user instructions is prohibited
5. Must not stop until the user's instructions are fully completed
6. Work logs must be saved to `.aiprj/AI_LOG/` in `yyyy-MM-dd_NNN.md` format (sequential, no overwriting)
7. Work logs must include the contents of `.aiprj/instructions.md`

## File Structure

```
aiprj/
├── install.sh               # Setup script
├── .mcp.json                # MCP configuration
├── .gitignore.aiprj         # gitignore template
├── .aiprj/
│   ├── instructions.md.org  # Instructions template
│   └── rules/
│       ├── setup_project.md  # Setup rules
│       ├── exec_job.md       # Task execution rules
│       ├── update_project.md # Update rules
│       └── close_ai.md       # Exit rules
└── .claude/
    ├── settings.json        # Claude Code settings
    └── commands/            # Slash command definitions
        ├── setup_ai.md
        ├── ai.md
        ├── update_ai.md
        ├── next_ai.md
        └── close_ai.md
```

## Requirements

- `curl` (for setup)
- `tar` (for one-liner fallback), or `git`
- Claude Code CLI
- Node.js / `npx` (for MCP integration)

## License

[MIT License](./LICENSE.md)
