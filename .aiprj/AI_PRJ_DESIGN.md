# Design Document (AI_PRJ_DESIGN)

This document describes the design for modifying the "[thinking]" display to show the label on its own line, display all thinking content, and add an "[answer]" label, as required by `AI_PRJ_REQUIREMENTS.md`.

## 1. Design Scope

### 1.1 In Scope

- Modifying `display_event` to print `[thinking]` on its own line (not on the same line as content).
- Displaying ALL thinking content (not just the first thinking event).
- Keeping `[answer]` label on its own line when the first text event arrives.
- Resetting state between turns.

### 1.2 Out of Scope

- Modifying `--help` output or CLI argument handling.
- Changing `show_thinking` configuration values or semantics.
- Modifying provider implementations or `AgentEvent` enum variants.
- Changing logging behavior.

## 2. Previous Implementation (Before This Update)

### 2.1 Previous `[thinking]` Display

The previous implementation placed `[thinking]` on the same line as the thinking content:

```rust
AgentEvent::Thinking { text } => match show_thinking {
    ShowThinkingMode::Hidden => {}
    ShowThinkingMode::Collapsed => {
        if !state.thinking_printed {
            let collapsed = collapse_thinking_text(&text);
            eprintln!("\n[thinking] {collapsed}");
            state.thinking_printed = true;
        }
    }
    ShowThinkingMode::Expanded => {
        if !state.thinking_printed {
            eprintln!("\n[thinking] {text}");
            state.thinking_printed = true;
        } else {
            eprintln!("{text}");
        }
    }
},
```

Issues with the previous implementation:
1. `[thinking]` was on the same line as the content: `\n[thinking] {text}`.
2. In collapsed mode, only the first thinking event was shown; subsequent events were suppressed.
3. In expanded mode, subsequent thinking events printed without a label, which was correct.

### 2.2 Previous `[answer]` Display

The previous implementation was correct:

```rust
AgentEvent::Text { delta } => {
    if !state.answer_printed {
        eprintln!("\n[answer]");
        state.answer_printed = true;
    }
    print!("{delta}");
    let _ = std::io::stdout().flush();
}
```

## 3. New Design

### 3.1 `[thinking]` on Own Line (Requirement FR-01)

Change the `[thinking]` label to be printed on its own line, separate from the thinking content:

**First thinking event in a turn** (`thinking_printed` is `false`):
1. Print `\n[thinking]` on stderr (label on its own line).
2. Set `thinking_printed = true`.
3. Then print the thinking content on the next line.

**Subsequent thinking events** (`thinking_printed` is `true`):
- Print the thinking content without another `[thinking]` label.

### 3.2 Display All Thinking Content (Requirement FR-02)

**Collapsed mode** (`ShowThinkingMode::Collapsed`):
- First thinking event: Print `[thinking]` label, then print the collapsed thinking text on the next line.
- Subsequent thinking events: Print the collapsed thinking text on its own line (no label).

**Expanded mode** (`ShowThinkingMode::Expanded`):
- First thinking event: Print `[thinking]` label, then print the full thinking text on the next line.
- Subsequent thinking events: Print the full thinking text on its own line (no label).

**Hidden mode** (`ShowThinkingMode::Hidden`):
- No thinking output at all (unchanged).

### 3.3 `[answer]` Label (Requirement FR-03)

No change from previous implementation. `[answer]` is printed on stderr on its own line when the first `AgentEvent::Text` event is received.

### 3.4 State Reset (Requirement FR-04)

No change from previous implementation. `DisplayState` is reset on `AgentEvent::Done` and `AgentEvent::Error`.

### 3.5 Implementation Details

The `display_event` function for `AgentEvent::Thinking` changes from:

```rust
AgentEvent::Thinking { text } => match show_thinking {
    ShowThinkingMode::Hidden => {}
    ShowThinkingMode::Collapsed => {
        if !state.thinking_printed {
            let collapsed = collapse_thinking_text(&text);
            eprintln!("\n[thinking] {collapsed}");
            state.thinking_printed = true;
        }
    }
    ShowThinkingMode::Expanded => {
        if !state.thinking_printed {
            eprintln!("\n[thinking] {text}");
            state.thinking_printed = true;
        } else {
            eprintln!("{text}");
        }
    }
},
```

To:

```rust
AgentEvent::Thinking { text } => match show_thinking {
    ShowThinkingMode::Hidden => {}
    ShowThinkingMode::Collapsed => {
        if !state.thinking_printed {
            eprintln!("\n[thinking]");
            state.thinking_printed = true;
        }
        let collapsed = collapse_thinking_text(&text);
        eprintln!("{collapsed}");
    }
    ShowThinkingMode::Expanded => {
        if !state.thinking_printed {
            eprintln!("\n[thinking]");
            state.thinking_printed = true;
        }
        eprintln!("{text}");
    }
},
```

Key changes:
1. `[thinking]` label is printed on its own line (`\n[thinking]`), not on the same line as content.
2. In collapsed mode, ALL thinking events display their content (not just the first).
3. In expanded mode, ALL thinking events display their content (unchanged behavior).

### 3.6 Output Examples

**Collapsed mode — before this update:**
```
[thinking] I need to analyze this problem...
```
(Only first thinking event shown, label and content on same line)

**Collapsed mode — after this update:**
```
[thinking]
I need to analyze this problem...
Let me consider the options...
```
(All thinking events shown, each truncated to 80 chars, label on own line)

**Expanded mode — before this update:**
```
[thinking] I need to analyze this problem. Let me consider the options...
Continuing my analysis...
```
(First event with label, subsequent events without)

**Expanded mode — after this update:**
```
[thinking]
I need to analyze this problem. Let me consider the options...
Continuing my analysis...
```
(All thinking events shown, label on own line)

**Answer phase (all modes):**
```
[answer]
{answer text flows to stdout}
```

**Hidden mode:**
```
[answer]
{answer text flows to stdout}
```
(No `[thinking]` label or thinking content, but `[answer]` is still shown)

## 4. Key Source Files

| File | Change |
|------|--------|
| `src/app.rs` | Modify `display_event` for `AgentEvent::Thinking` to put `[thinking]` on its own line and display all thinking content |

No other source files need modification.

## 5. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| More verbose output in collapsed mode | All thinking deltas are now shown, not just the first | Each delta is still collapsed (truncated) per the existing `collapse_thinking_text` function |
| State not reset between turns | `[thinking]` and `[answer]` not printed in subsequent turns | Reset `DisplayState` on `AgentEvent::Done` and `AgentEvent::Error` |
| `cargo test` failures | Possible if tests check stderr output format | Update tests that check for `[thinking]` output patterns |

## 6. Handoff to Implementation Phase

The implementation phase should:

1. Modify the `AgentEvent::Thinking` branch in `display_event` to print `[thinking]` on its own line.
2. Change collapsed mode to display ALL thinking events (not just the first).
3. Change expanded mode to display all thinking events on their own lines.
4. Verify `cargo build` and `cargo test` pass.