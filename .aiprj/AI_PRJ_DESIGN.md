# Design Document (AI_PRJ_DESIGN)

This document is the design document for the conversion work required to satisfy the conversion requirements in `AI_PRJ_REQUIREMENTS.md`. It does not describe software design; rather, it defines "how to convert" and "how to ensure quality."

## 1. Conversion Scope

### 1.1 In Scope

- Detect Japanese text (hiragana, katakana, kanji, full-width symbols) in all files in the repository and convert it to equivalent English text.
- Detect full-width alphanumeric characters and full-width symbols in all files in the repository and convert them to their corresponding half-width characters.
- Conversion target files are classified into the following categories (Requirements Section 3.4).

| Category | Files | Conversion Policy |
|----------|-------|-------------------|
| Documentation | `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `doc/*.md`, `example/agents/*.md`, `report_cli.md` | Convert entire body text to English + convert full-width to half-width |
| Source code | `src/**/*.rs` | Convert only comments, doc comments, and user-facing messages to English |
| Project management | `.aiprj/AI_PRJ_REQUIREMENTS.md`, `.aiprj/AI_PRJ_DESIGN.md`, `.aiprj/AI_PRJ_TASKS.md` | Convert to English in the implementation phase |
| Project management (other) | `.aiprj/README.md`, `.aiprj/rules/*.md` | Convert to English + convert full-width to half-width |

### 1.2 Out of Scope

- Logic changes to source code (Rust). Function names, variable names, and type names will not be changed.
- Files in external directories such as `.git/`, `target/`, and `node_modules/`.
- Binary files and image files.
- `.aiprj/instructions.md` (already in English, so no conversion needed).

## 2. Conversion Policy

### 2.1 English Conversion of Japanese Text (Requirement FR-01)

#### 2.1.1 Translation Principles

- **Context preservation**: Accurately reflect the Japanese context and intent in English. Produce natural English suitable for technical documentation rather than literal translations.
- **Terminological consistency**: Use the same English translation for the same Japanese term. The primary terminology mapping is defined below.

| Japanese | English |
|---------|---------|
| Requirements | Requirements |
| Design | Design |
| Task | Task |
| Agent | Agent |
| Provider | Provider |
| Tool | Tool |
| Session | Session |
| Streaming | Streaming |
| Configuration | Configuration |
| Command | Command |
| Output | Output |
| Input | Input |
| Launch | Launch / Start |
| Terminate | Terminate / Exit |
| Approval | Approval |
| Permission | Permission |

- **Structure preservation**: Maintain heading levels, list structures, table structures, and code block boundaries.
- **Information preservation**: Verify that no information is lost through translation.

#### 2.1.2 Category-Specific Policies

**Documentation files**:
- Translate the entire body text to English.
- Translate comments within code blocks to English as well.
- Do not translate execution commands or code bodies within code blocks.

**Source code files (Rust)**:
- Translate `///` doc comments and `//` comments to English.
- Translate user-facing messages (error messages, help text, etc.) among string literals to English.
- Do not modify internal identifiers (function names, variable names, type names, module names).
- Translate assertion messages in test code to English as well.

### 2.2 Half-Width Conversion of Full-Width Characters (Requirement FR-02)

- Convert full-width alphanumeric characters (A-Z, a-z, 0-9) to their corresponding half-width alphanumeric characters (A-Z, a-z, 0-9).
- Convert full-width symbols (_, -, (, ), ., ,, :, ;, etc.) to their corresponding half-width symbols.
- Punctuation marks within Japanese text (. -> ., , -> ,) will be converted as part of the FR-01 translation.
- Parts intentionally expressed in full-width (decorative separator lines, etc.) will be evaluated based on context and replaced with half-width characters as necessary.

### 2.3 Quality Assurance (Requirement FR-03)

#### 2.3.1 Structural Integrity Check

- Verify that heading levels (`#`, `##`, etc.) match before and after translation for each file.
- Verify that the number of columns and rows in tables match.
- Verify that code block boundaries (sections enclosed by ` ``` `) are maintained.
- Verify that the URL portions of links (in `[text](url)` format) have not been changed.

#### 2.3.2 Terminological Consistency Check

- Verify that the same Japanese term is translated to the same English term across multiple files.
- Verify consistency in the use of abbreviations (e.g., CLI, API, IPC, etc.).

#### 2.3.3 Behavioral Impact Check

- For Rust source code translations, verify that changes to string literals do not affect compilation or tests.
- Verify that `cargo build` and `cargo test` complete successfully after changes.

## 3. Conversion Target File List

The following are files in the repository containing Japanese text (as of 2026-05-11).

### 3.1 Documentation Files

| File | Conversion Content |
|------|--------------------|
| `README.md` | Convert entire body text to English + convert full-width to half-width |
| `README.en.md` | Content consistency check only (already in English) |
| `CHANGELOG.md` | Convert entire body text to English + convert full-width to half-width |
| `CONTRIBUTING.md` | Convert entire body text to English + convert full-width to half-width |
| `doc/architecture.md` | Convert entire body text to English + convert full-width to half-width |
| `doc/config.md` | Convert entire body text to English + convert full-width to half-width |
| `doc/personas.md` | Convert entire body text to English + convert full-width to half-width |
| `doc/providers/claude.md` | Convert entire body text to English + convert full-width to half-width |
| `doc/providers/codex.md` | Convert entire body text to English + convert full-width to half-width |
| `doc/providers/llamacpp.md` | Convert entire body text to English + convert full-width to half-width |
| `doc/providers/ollama.md` | Convert entire body text to English + convert full-width to half-width |
| `doc/tools.md` | Convert entire body text to English + convert full-width to half-width |
| `doc/troubleshooting.md` | Convert entire body text to English + convert full-width to half-width |
| `doc/usage.md` | Convert entire body text to English + convert full-width to half-width |
| `example/agents/coder.md` | Convert entire body text to English + convert full-width to half-width |
| `example/agents/planner.md` | Convert entire body text to English + convert full-width to half-width |
| `example/agents/reviewer.md` | Convert entire body text to English + convert full-width to half-width |
| `report_cli.md` | Convert entire body text to English + convert full-width to half-width |

### 3.2 Source Code Files

| File | Conversion Content |
|------|--------------------|
| `src/agent.rs` | Comments and doc comments only |
| `src/ai/claude.rs` | Comments and doc comments only |
| `src/ai/codex.rs` | Comments and doc comments only |
| `src/ai/llamacpp.rs` | Comments and doc comments only |
| `src/ai/mod.rs` | Comments and doc comments only |
| `src/ai/ollama.rs` | Comments and doc comments only |
| `src/ai/stream.rs` | Comments and doc comments only |
| `src/ai/tool_bridge.rs` | Comments and doc comments only |
| `src/app.rs` | Comments and doc comments only |
| `src/cli.rs` | Comments and doc comments only |
| `src/commands.rs` | Comments and doc comments only |
| `src/config.rs` | Comments and doc comments only |
| `src/id.rs` | Comments and doc comments only |
| `src/ipc/mod.rs` | Comments and doc comments only |
| `src/ipc/registry.rs` | Comments and doc comments only |
| `src/ipc/server.rs` | Comments and doc comments only |
| `src/main.rs` | Comments and doc comments only |
| `src/persona.rs` | Comments and doc comments only |
| `src/tools/mod.rs` | Comments and doc comments only |

### 3.3 Project Management Files

| File | Conversion Content |
|------|--------------------|
| `.aiprj/README.md` | Convert entire body text to English + convert full-width to half-width |
| `.aiprj/rules/exec_job.md` | Convert entire body text to English + convert full-width to half-width |
| `.aiprj/rules/update_project.md` | Convert entire body text to English + convert full-width to half-width |
| `.aiprj/AI_PRJ_REQUIREMENTS.md` | Convert to English + convert full-width to half-width in the implementation phase |
| `.aiprj/AI_PRJ_DESIGN.md` | Convert to English + convert full-width to half-width in the implementation phase |
| `.aiprj/AI_PRJ_TASKS.md` | Convert to English + convert full-width to half-width in the implementation phase |

## 4. Tools Used for Conversion Work

| Purpose | Tool |
|---------|------|
| Read files and verify content | Read |
| Convert files (write) | Edit / Write |
| Detect Japanese text | Bash (grep) |
| Detect full-width characters | Bash (grep) |
| Build and test Rust code | Bash (cargo build / cargo test) |
| Progress management | TaskCreate (during implementation phase) |

## 5. Risks and Countermeasures

| Risk | Impact | Countermeasure |
|------|--------|----------------|
| Inconsistent translation quality | English translations vary by context | Ensure consistency using the terminology mapping (Section 2.1.1) |
| Build errors due to string changes in Rust source code | Compilation failure | Run `cargo build` and `cargo test` after conversion to verify |
| Markdown structural corruption | Reduced document readability | Verify heading levels, table structures, and code block boundaries before and after translation |
| Missed full-width to half-width conversions | Remaining full-width characters | Re-run `grep` to detect full-width characters and verify |
| Content divergence between `README.md` and `README.en.md` | Information inconsistency | Verify consistency with `README.en.md` after converting `README.md` to English |

## 6. Handoff to Implementation Phase

Actions to take in the implementation phase (separate session):

1. Execute the tasks in `AI_PRJ_TASKS.md` in order, converting Japanese text to English and full-width characters to half-width in each file.
2. Follow the file list in Section 3 and apply the category-specific conversion policies.
3. Perform the quality checks in Section 2.3 after conversion.
4. Append findings during progress to `.aiprj/AI_LOG/`.
5. Finalize updates using `/close_ai` or similar upon completion.