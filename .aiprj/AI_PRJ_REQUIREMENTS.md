# Requirements Specification (AI_PRJ_REQUIREMENTS)

## 1. Project Overview

### 1.1 Project Name

Display Label Refactoring: "[thinking]" Start-Only and "[answer]" Label for agent-cli

### 1.2 Background

The following instructions were received in `.aiprj/instructions.md` (as of 2026-05-12):

> - When running agent-cli, "[thinking]" is displayed. The configuration method is not shown in --help. Should it be configured in config.toml?
> - When in display mode (not per-token), show "[thinking]" only at the start of thinking, and show "[answer]" when answering.

Currently, when `show_thinking` is set to `"collapsed"` or `"expanded"`, every thinking event emits a `[thinking]` prefix on each delta. The user requests that `[thinking]` be shown only once at the start of the thinking phase, and that a new `[answer]` label be shown when the answer (text) begins.

Additionally, the user noted that `--help` does not display the `show_thinking` configuration option, and asks whether it should be configured via `config.toml` (answer: yes, it is config-file-only).

### 1.3 Objectives

- Modify the `display_event` function so that `[thinking]` is printed only once at the start of the thinking phase (not on every thinking delta).
- Add a `[answer]` label that is printed once when the first text (answer) event is received, to clearly demarcate the transition from thinking to answering.
- Preserve the existing `show_thinking` configuration mechanism (`[ui] show_thinking` in config.toml) and its three modes (`hidden`, `collapsed`, `expanded`).
- Document that `show_thinking` is configured via `config.toml` only and is not exposed in `--help`.

### 1.4 Non-Objectives

- No changes to the `--help` output or CLI argument handling.
- No changes to the `show_thinking` configuration values or their semantics.
- No changes to provider-level thinking support (Claude, Ollama, etc.).
- No changes to logging behavior (`LogEvent::Thinking` is still always logged).

## 2. Terminology

| Term | Definition |
|------|-----------|
| `[thinking]` label | The label printed on stderr at the start of the thinking phase |
| `[answer]` label | The new label printed on stderr when the first text (answer) event is received |
| Thinking phase | The period during which `AgentEvent::Thinking` events are received |
| Answer phase | The period during which `AgentEvent::Text` events are received |
| `show_thinking` | The configuration key under `[ui]` that controls how thinking output is displayed |
| `ShowThinkingMode` | The Rust enum (`Hidden`, `Collapsed`, `Expanded`) that determines display behavior |

## 3. Functional Requirements

### 3.1 Start-Only "[thinking]" Label (FR-01)

- When `show_thinking` is set to `"collapsed"` or `"expanded"`, print `[thinking]` on stderr only once at the start of the thinking phase (i.e., when the first `AgentEvent::Thinking` event is received).
- Subsequent `AgentEvent::Thinking` events in the same turn must NOT print additional `[thinking]` labels.
- When `show_thinking` is set to `"hidden"`, no `[thinking]` label or thinking text is displayed (existing behavior preserved).

### 3.2 "[answer]" Label (FR-02)

- Print `[answer]` on stderr exactly once when the first `AgentEvent::Text` event is received in a turn.
- Subsequent `AgentEvent::Text` events must NOT print additional `[answer]` labels.
- The `[answer]` label must be printed regardless of the `show_thinking` mode (i.e., even when `show_thinking` is `"hidden"`).

### 3.3 Thinking Text Display Behavior (FR-03)

- When `show_thinking` is `"collapsed"`, the thinking text content (truncated to the first line, 80 chars max) must be printed on stderr immediately after the `[thinking]` label, on the same line.
- When `show_thinking` is `"expanded"`, the full thinking text must be printed on stderr immediately after the `[thinking]` label, on the same line.
- Thinking text from subsequent events in the same thinking phase must NOT be displayed in `"collapsed"` mode. In `"expanded"` mode, subsequent thinking text should continue to be appended.

### 3.4 State Reset Between Turns (FR-04)

- The thinking/answer phase state (whether `[thinking]` or `[answer]` has been printed) must be reset at the start of each new turn (i.e., when `AgentEvent::Done` is received or a new turn begins).

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

- Source code modifications are limited to `src/app.rs` (display logic) and `src/config.rs` only if necessary. No other source files should be modified.
- No changes to the CLI argument parser (`src/cli.rs`).
- No changes to provider implementations.
- No changes to the agent event model (`AgentEvent` enum variants).

## 6. Intended Users

- Users of agent-cli who want a clearer distinction between the thinking and answering phases in the output.
- Developers maintaining the agent-cli display layer.

## 7. Acceptance Criteria

- `[thinking]` is printed only once per turn at the start of the thinking phase, not on every thinking event.
- `[answer]` is printed exactly once per turn when the first text event is received.
- The `show_thinking` configuration continues to work as before (`"hidden"`, `"collapsed"`, `"expanded"`).
- `cargo build` and `cargo test` pass after the changes.
- No other display behavior is unintentionally altered.