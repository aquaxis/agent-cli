# Design Document (AI_PRJ_DESIGN)

This document describes the design for modifying the "[thinking]" display to show the label only at the start of thinking and adding an "[answer]" label, as required by `AI_PRJ_REQUIREMENTS.md`.

## 1. Design Scope

### 1.1 In Scope

- Modifying `display_event` to track thinking/answer phase state and print labels only once per turn.
- Adding `[answer]` label output when the first `AgentEvent::Text` event is received.
- Ensuring `[thinking]` label is printed only once at the start of the thinking phase.
- Resetting state between turns.

### 1.2 Out of Scope

- Modifying `--help` output or CLI argument handling.
- Changing `show_thinking` configuration values or semantics.
- Modifying provider implementations or `AgentEvent` enum variants.
- Changing logging behavior.

## 2. Current Behavior

### 2.1 Current `[thinking]` Display (Before Change)

In the current implementation (`src/app.rs`), every `AgentEvent::Thinking` event triggers a `eprintln!("\n[thinking] {text}")` (collapsed or expanded). This means `[thinking]` is printed on every thinking event, not just at the start.

```rust
AgentEvent::Thinking { text } => match show_thinking {
    ShowThinkingMode::Hidden => {}
    ShowThinkingMode::Collapsed => {
        let collapsed = collapse_thinking_text(&text);
        eprintln!("\n[thinking] {collapsed}");
    }
    ShowThinkingMode::Expanded => {
        eprintln!("\n[thinking] {text}");
    }
},
```

### 2.2 Current Text Display (Before Change)

`AgentEvent::Text` events are printed to stdout with no label:

```rust
AgentEvent::Text { delta } => {
    print!("{delta}");
    let _ = std::io::stdout().flush();
}
```

There is no `[answer]` label anywhere in the codebase.

### 2.3 Current State Management

Currently, `display_event` is a pure function that takes `(AgentEvent, ShowThinkingMode)` with no state. There is no tracking of whether `[thinking]` or `[answer]` has been printed in the current turn.

## 3. New Design

### 3.1 Phase State Tracking (Requirement FR-01, FR-02, FR-04)

Introduce a `DisplayState` struct to track whether `[thinking]` and `[answer]` have been printed in the current turn:

```rust
struct DisplayState {
    thinking_printed: bool,
    answer_printed: bool,
}
```

This state will be initialized at the start of each turn (reset on `AgentEvent::Done`).

### 3.2 Modified `[thinking]` Display (Requirement FR-01, FR-03)

Change the thinking event handling to print `[thinking]` only once at the start:

**When `thinking_printed` is `false`** (first thinking event in a turn):
- Print `\n[thinking] {text}` on stderr (collapsed or expanded depending on mode).
- Set `thinking_printed = true`.

**When `thinking_printed` is `true`** (subsequent thinking events):
- For `Collapsed` mode: Do not print any additional output (the collapsed summary was already shown).
- For `Expanded` mode: Continue appending thinking text to stderr, but without another `[thinking]` label.

### 3.3 New `[answer]` Label (Requirement FR-02)

Change the text event handling to print `[answer]` once at the start:

**When `answer_printed` is `false`** (first text event in a turn):
- Print `\n[answer]` on stderr.
- Set `answer_printed = true`.
- Then print the text delta to stdout as usual.

**When `answer_printed` is `true`** (subsequent text events):
- Print the text delta to stdout as usual (no additional `[answer]` label).

### 3.4 State Reset (Requirement FR-04)

On `AgentEvent::Done`, reset `DisplayState`:

```rust
AgentEvent::Done => {
    display_state.thinking_printed = false;
    display_state.answer_printed = false;
    println!();
}
```

### 3.5 Configuration Method (Requirement FR-05)

No code changes needed. The existing `[ui] show_thinking` in `config.toml` is the only configuration method. This is confirmed by investigation:
- `src/cli.rs` has no `show_thinking` flag.
- `src/config.rs` defines `show_thinking` under `UiConfig` with default `"collapsed"`.
- Unknown values fall back to `Collapsed`.

## 4. Implementation Details

### 4.1 Modified Function Signature

Change `display_event` from a pure function to a method that takes mutable state:

```rust
fn display_event(ev: AgentEvent, show_thinking: ShowThinkingMode, state: &mut DisplayState)
```

Or alternatively, keep `display_event` as-is and manage state at the call site. The call site in `src/app.rs` already has a loop processing events, so state can be managed there.

### 4.2 Call Site Changes

The call site in `src/app.rs` currently looks like:

```rust
let show_thinking = config.ui.show_thinking_mode();
// ...
display_event(ev, show_thinking);
```

After the change, it will need to create and manage `DisplayState`:

```rust
let show_thinking = config.ui.show_thinking_mode();
let mut display_state = DisplayState::new();
// ...
display_event(ev, show_thinking, &mut display_state);
```

### 4.3 Output Examples

**Before (current behavior) — Collapsed mode:**

```
[thinking] I need to analyze this problem...
[thinking] Let me consider the options...
[answer text starts flowing to stdout]
```

**After (new behavior) — Collapsed mode:**

```
[thinking] I need to analyze this problem...
[answer]
[answer text flows to stdout]
```

**Before (current behavior) — Expanded mode:**

```
[thinking] I need to analyze this problem. Let me consider the options...
[thinking] Continuing my analysis...
[answer text starts flowing to stdout]
```

**After (new behavior) — Expanded mode:**

```
[thinking] I need to analyze this problem. Let me consider the options...
Continuing my analysis...
[answer]
[answer text flows to stdout]
```

**Hidden mode (unchanged):**

```
[answer text flows to stdout]
```

Note: In hidden mode, `[answer]` is still printed on stderr when the first text event arrives.

## 5. Key Source Files

| File | Change |
|------|--------|
| `src/app.rs` | Add `DisplayState`, modify `display_event`, modify call site |

No other source files need modification.

## 6. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| State not reset between turns | `[thinking]` and `[answer]` not printed in subsequent turns | Reset `DisplayState` on `AgentEvent::Done` |
| `[answer]` printed even in hidden mode | Could be unexpected | This is by design per FR-02; `[answer]` is always shown to demarcate the answer phase |
| Expanded mode behavior with subsequent thinking events | Need to decide whether to append text or suppress | Per FR-03, in expanded mode subsequent thinking text continues to be appended without an additional label |
| `cargo test` failures | Possible if tests check stderr output format | Update tests that check for `[thinking]` output patterns |

## 7. Handoff to Implementation Phase

The implementation phase should:

1. Add `DisplayState` struct to `src/app.rs`.
2. Modify `display_event` to accept `&mut DisplayState`.
3. Change `[thinking]` handling to print label only on first thinking event.
4. Add `[answer]` label on first text event.
5. Reset state on `AgentEvent::Done`.
6. Verify `cargo build` and `cargo test` pass.