# aiprj - AI Project Management Tool

A project management tool for Claude Code. It deploys behavioral guidelines and document structures (requirements, design, tasks) for AI work into a target directory with a single command.

## Overview

aiprj provides the following features:

- Definition of AI behavioral guidelines and rules
- Document structure for requirements, design specifications, and task lists
- Claude Code slash commands (`/setup_ai` `/ai` `/update_ai` `/next_ai` `/close_ai`)
- Automatic work log storage (`.aiprj/AI_LOG/yyyy-MM-dd_NNN.md`)

## Setup

### Setup in the current directory

```bash
curl -fsSL https://raw.githubusercontent.com/aquaxis/aiprj/main/install.sh | sh
```

### Setup in a specified directory

```bash
curl -fsSL https://raw.githubusercontent.com/aquaxis/aiprj/main/install.sh | sh -s -- <directory-name>
```

### Manual setup

```bash
git clone https://github.com/aquaxis/aiprj.git
cd aiprj
./install.sh <setup-target-directory>
```

Setup creates the following files:

- `.aiprj/` — AI rules, `instructions.md`, `README.md`
- `.claude/` — Claude Code settings and slash commands
- `.mcp.json` — MCP server configuration
- `.gitignore` — Git exclusion settings (prepends template if file exists)

### Claude Code Slash Commands

| Command | Description |
|---------|-------------|
| `/setup_ai` | Create project documents (requirements, design, tasks) |
| `/ai` | Execute tasks based on `instructions.md` |
| `/update_ai` | Update project documents |
| `/next_ai` | Move to the next task |
| `/close_ai` | Save work log and finish |

## Project Structure

After setup, the AI manages the following documents:

| File | Content |
|------|---------|
| `.aiprj/AI_PRJ_REQUIREMENTS.md` | Requirements definition |
| `.aiprj/AI_PRJ_DESIGN.md` | Design specification |
| `.aiprj/AI_PRJ_TASKS.md` | Implementation tasks and work instructions |
| `.aiprj/AI_LOG/` | Work logs (`yyyy-MM-dd_NNN.md` format, sequential, no overwrites) |

## AI Behavioral Guidelines

The AI operates according to the following guidelines:

1. Before commencing any task, formulate a comprehensive work plan
2. Do not distort, alter, or reinterpret the AI Operation Guidelines
3. Do not take detours or modify the approach beyond what the user has explicitly instructed
4. Do not optimize, rewrite, or reinterpret user instructions
5. Do not stop execution until the user's instructions are fully completed
6. Store work logs in `.aiprj/AI_LOG/` using `yyyy-MM-dd_NNN.md` format (sequential, no overwrites)
7. Include the full content of `.aiprj/instructions.md` in each work log

## File Structure

```
aiprj/
├── install.sh               # Setup script
├── .mcp.json                # MCP configuration
├── .gitignore.aiprj         # gitignore template
├── .aiprj/
│   ├── instructions.md.org  # Instruction template
│   └── rules/
│       ├── setup_project.md  # Setup rules
│       ├── exec_job.md       # Task execution rules
│       ├── update_project.md # Update rules
│       └── close_ai.md       # Closing rules
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
- `tar` (fallback for one-liner) or `git`
- Claude Code CLI
- Node.js / `npx` (for MCP integration)

## License

MIT License