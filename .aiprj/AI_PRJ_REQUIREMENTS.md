# Requirements Specification (AI_PRJ_REQUIREMENTS)

## 1. Project Overview

### 1.1 Project Name

Thinking Display: Sentence-Based Line Breaking for agent-cli

### 1.2 Background

The following instruction was received in `.aiprj/instructions.md` (as of 2026-05-12):

> When displaying "[thinking]", instead of breaking lines per token, please break lines per sentence.

The previous implementation printed each thinking delta (token) on its own line using `eprintln!`. This created fragmented output where each small streaming chunk appeared on a separate line. The user wants thinking text to flow together naturally, with line breaks occurring at sentence boundaries rather than at every token.

### 1.3 Objectives

- Change the thinking display so that thinking text flows together instead of each token getting its own line.
- Use `eprint!` instead of `eprintln!` for thinking text so that deltas concatenate naturally.
- In collapsed mode, add a space separator between deltas to prevent words from running together.
- In expanded mode, let provider-sent deltas flow together naturally (providers include appropriate spacing).
- Preserve the `[thinking]` label on its own line and the `[answer]` label on its own line.
- Preserve the `show_thinking` configuration mechanism and its three modes.

### 1.4 Non-Objectives

- No changes to the `--help` output or CLI argument handling.
- No changes to the `show_thinking` configuration values or their semantics.
- No changes to provider-level thinking support (Claude, Ollama, etc.).
- No changes to logging behavior (`LogEvent::Thinking` is still always logged).
- No changes to sentence boundary detection logic (provider-sent newlines serve as natural breaks).

## 2. Terminology

| Term | Definition |
|------|-----------|
| Token-based line breaking | Breaking lines at each thinking delta/token (previous behavior) |
| Sentence-based line breaking | Letting text flow together, with line breaks at natural boundaries (new behavior) |
| `[thinking]` label | The label printed on stderr on its own line at the start of the thinking phase |
| `[answer]` label | The label printed on stderr on its own line when the first text event is received |
| `show_thinking` | The configuration key under `[ui]` that controls how thinking output is displayed |
| `ShowThinkingMode` | The Rust enum (`Hidden`, `Collapsed`, `Expanded`) that determines display behavior |

## 3. Functional Requirements

### 3.1 Sentence-Based Line Breaking for Thinking (FR-01)

- In expanded mode, thinking text must flow together using `eprint!` instead of `eprintln!`. Each thinking delta is printed without a trailing newline, allowing tokens to concatenate naturally.
- In collapsed mode, each thinking delta is still truncated (first line, 80 chars max), but deltas are concatenated with spaces between them using `eprint!` instead of each delta getting its own line.
- Provider-sent newlines in thinking text create natural sentence/paragraph boundaries.
- The `[thinking]` label remains on its own line (unchanged from previous implementation).
- The `[answer]` label remains on its own line (unchanged from previous implementation).

### 3.2 Collapsed Mode Space Separator (FR-02)

- In collapsed mode, a space must be inserted between consecutive thinking deltas to prevent truncated words from running together.
- The space is inserted before each subsequent delta (not the first one), using the `thinking_printed` flag to distinguish the first delta from subsequent ones.

### 3.3 Expanded Mode Natural Flow (FR-03)

- In expanded mode, thinking deltas must flow together naturally without artificial spaces or newlines between them.
- The provider sends tokens that include appropriate whitespace (e.g., " World" with a leading space), so no additional separator is needed.

### 3.4 "[answer]" Label (FR-04)

- No change from previous implementation. `[answer]` is printed on stderr on its own line when the first `AgentEvent::Text` event is received.
- The `\n` prefix in `eprintln!("\n[answer]")` ensures a newline after any flowing thinking text, creating separation between thinking and answer.

### 3.5 State Reset Between Turns (FR-05)

- No change from previous implementation. `DisplayState` resets on `AgentEvent::Done` and `AgentEvent::Error`.

## 4. Non-Functional Requirements

### 4.1 Document Language (NFR-01)

- All project management documents (Requirements, Design, Tasks) will be written in English.

### 4.2 Consistency (NFR-02)

- The design and tasks will be consistent with the requirements.

### 4.3 Backward Compatibility (NFR-03)

- The existing `show_thinking` configuration values (`"hidden"`, `"collapsed"`, `"expanded"`) and their semantics must remain unchanged.
- The `--help` output must not be modified.
- Logging behavior (`LogEvent::Thinking`) must remain unchanged.

## 5. Constraints

- Source code modifications are limited to `src/app.rs` (display logic). No other source files should be modified.
- No changes to the CLI argument parser (`src/cli.rs`).
- No changes to provider implementations.
- No changes to the agent event model (`AgentEvent` enum variants).

## 6. Intended Users

- Users of agent-cli who want thinking text to flow naturally instead of being fragmented per token.
- Developers maintaining the agent-cli display layer.

## 7. Acceptance Criteria

- Thinking text flows together using `eprint!` instead of `eprintln!`.
- In collapsed mode, deltas are separated by spaces, not newlines.
- In expanded mode, deltas concatenate naturally without separators.
- `[thinking]` remains on its own line.
- `[answer]` remains on its own line.
- `cargo build` and `cargo test` pass after the changes.