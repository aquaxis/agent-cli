# Requirements Specification (AI_PRJ_REQUIREMENTS)

## 1. Project Overview

### 1.1 Project Name

Display Label Refactoring: "[thinking]" on Separate Line, Full Content Display, and "[answer]" Label for agent-cli

### 1.2 Background

The following instruction was received in `.aiprj/instructions.md` (as of 2026-05-12):

> When displaying "[thinking]", put "[thinking]" on a new line, display all thinking content, and then display "[answer]" when answering.

The previous implementation placed `[thinking]` on the same line as the thinking content and only showed the first thinking event in collapsed mode. The user now requests:
1. `[thinking]` label on its own line (not on the same line as content).
2. All thinking content to be displayed (not just the first event in collapsed mode).
3. `[answer]` label displayed when the answer starts.

### 1.3 Objectives

- Modify the `display_event` function so that `[thinking]` is printed on its own line, followed by all thinking content on subsequent lines.
- Display ALL thinking content, not just the first thinking event. In collapsed mode, each thinking delta is still truncated (first line, 80 chars max) but all deltas are shown. In expanded mode, all thinking text is shown in full.
- Keep the `[answer]` label printed once when the first text event is received.
- Preserve the existing `show_thinking` configuration mechanism (`[ui] show_thinking` in config.toml) and its three modes (`hidden`, `collapsed`, `expanded`).

### 1.4 Non-Objectives

- No changes to the `--help` output or CLI argument handling.
- No changes to the `show_thinking` configuration values or their semantics.
- No changes to provider-level thinking support (Claude, Ollama, etc.).
- No changes to logging behavior (`LogEvent::Thinking` is still always logged).

## 2. Terminology

| Term | Definition |
|------|-----------|
| `[thinking]` label | The label printed on stderr on its own line at the start of the thinking phase |
| `[answer]` label | The label printed on stderr on its own line when the first text (answer) event is received |
| Thinking phase | The period during which `AgentEvent::Thinking` events are received |
| Answer phase | The period during which `AgentEvent::Text` events are received |
| `show_thinking` | The configuration key under `[ui]` that controls how thinking output is displayed |
| `ShowThinkingMode` | The Rust enum (`Hidden`, `Collapsed`, `Expanded`) that determines display behavior |

## 3. Functional Requirements

### 3.1 "[thinking]" Label on Own Line (FR-01)

- When `show_thinking` is set to `"collapsed"` or `"expanded"`, print `[thinking]` on stderr on its own line at the start of the thinking phase (i.e., when the first `AgentEvent::Thinking` event is received).
- The `[thinking]` label must be on a separate line from the thinking content. The format is: `\n[thinking]\n{content}`, not `\n[thinking] {content}`.
- When `show_thinking` is set to `"hidden"`, no `[thinking]` label or thinking text is displayed (existing behavior preserved).

### 3.2 Display All Thinking Content (FR-02)

- ALL thinking events must display their content, not just the first one.
- In `"collapsed"` mode, each thinking delta is displayed on its own line with the collapsed text (first line truncated to 80 chars). All deltas are shown, not just the first.
- In `"expanded"` mode, each thinking delta is displayed in full on its own line.
- The `[thinking]` label is printed only once at the start; subsequent thinking events print their content without another label.

### 3.3 "[answer]" Label (FR-03)

- Print `[answer]` on stderr on its own line exactly once when the first `AgentEvent::Text` event is received in a turn.
- Subsequent `AgentEvent::Text` events must NOT print additional `[answer]` labels.
- The `[answer]` label must be printed regardless of the `show_thinking` mode (i.e., even when `show_thinking` is `"hidden"`).

### 3.4 State Reset Between Turns (FR-04)

- The thinking/answer phase state (whether `[thinking]` or `[answer]` has been printed) must be reset at the start of each new turn (i.e., when `AgentEvent::Done` or `AgentEvent::Error` is received).

### 3.5 Configuration Method Documentation (FR-05)

- Confirm and document that `show_thinking` is configured exclusively via the `[ui]` section of `config.toml`. There is no `--help` flag or CLI argument for this setting.

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

- Users of agent-cli who want a clearer distinction between the thinking and answering phases in the output.
- Developers maintaining the agent-cli display layer.

## 7. Acceptance Criteria

- `[thinking]` is printed on its own line (not on the same line as thinking content) only once per turn at the start of the thinking phase.
- ALL thinking content is displayed (not just the first event).
- `[answer]` is printed on its own line exactly once per turn when the first text event is received.
- The `show_thinking` configuration continues to work as before (`"hidden"`, `"collapsed"`, `"expanded"`).
- `cargo build` and `cargo test` pass after the changes.
- No other display behavior is unintentionally altered.