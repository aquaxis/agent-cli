# Design Document (AI_PRJ_DESIGN)

This document describes the design for changing thinking display from token-based to sentence-based line breaking, as required by `AI_PRJ_REQUIREMENTS.md`.

## 1. Design Scope

### 1.1 In Scope

- Changing `eprintln!` to `eprint!` for thinking text in both collapsed and expanded modes.
- Adding space separators between collapsed deltas.
- Keeping `[thinking]` and `[answer]` labels on their own lines.

### 1.2 Out of Scope

- Modifying `--help` output or CLI argument handling.
- Changing `show_thinking` configuration values or semantics.
- Modifying provider implementations or `AgentEvent` enum variants.
- Changing logging behavior.
- Implementing sentence boundary detection (relying on provider-sent newlines instead).

## 2. Previous Implementation (Before This Update)

### 2.1 Previous Thinking Display

The previous implementation printed each thinking delta on its own line:

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

Issues:
1. Each thinking delta got its own line (`eprintln!`), creating fragmented output.
2. In expanded mode, streaming tokens appeared on separate lines: "I", "need", "to", "analyze" etc.
3. In collapsed mode, each truncated delta appeared on its own line.

### 2.2 Previous Output Examples

**Collapsed mode (before):**
```
[thinking]
I need to analyze this...
Let me consider the options...
Continuing my analysis...
```
Each delta on its own line.

**Expanded mode (before):**
```
[thinking]
I need to analyze this problem.
Let me consider the options.
Continuing my analysis.
```
Each delta on its own line.

## 3. New Design

### 3.1 Sentence-Based Line Breaking (Requirement FR-01)

Change `eprintln!` to `eprint!` for thinking text. This allows deltas to flow together instead of each getting its own line.

### 3.2 Collapsed Mode Space Separator (Requirement FR-02)

In collapsed mode, add a space before each subsequent delta (using the `thinking_printed` flag to identify subsequent deltas). This prevents truncated words from running together.

```rust
ShowThinkingMode::Collapsed => {
    if !state.thinking_printed {
        eprintln!("\n[thinking]");
        state.thinking_printed = true;
    } else {
        eprint!(" ");
    }
    let collapsed = collapse_thinking_text(&text);
    eprint!("{collapsed}");
}
```

The `else { eprint!(" "); }` adds a space before each subsequent delta (but not the first one). The `thinking_printed` flag distinguishes the first delta from subsequent ones.

### 3.3 Expanded Mode Natural Flow (Requirement FR-03)

In expanded mode, use `eprint!` for thinking text without any separator. Provider-sent tokens already include appropriate whitespace (e.g., " World" with a leading space), so they concatenate naturally.

```rust
ShowThinkingMode::Expanded => {
    if !state.thinking_printed {
        eprintln!("\n[thinking]");
        state.thinking_printed = true;
    }
    eprint!("{text}");
}
```

### 3.4 Answer Label Separation (Requirement FR-04)

The `\n` prefix in `eprintln!("\n[answer]")` ensures a newline after flowing thinking text. This creates clear separation:

```
[thinking]
thinking text flows here...

[answer]
answer text flows here
```

The `\n` in `\n[answer]` ends the current line (where thinking text is flowing), and `[answer]` starts on the next line.

### 3.5 Output Examples

**Collapsed mode (after):**
```
[thinking]
I need to analyze this... Let me consider the options... Continuing my analysis...

[answer]
The answer is 42.
```
Deltas flow together with spaces between truncated fragments.

**Expanded mode (after):**
```
[thinking]
I need to analyze this problem. Let me consider the options. Continuing my analysis...

[answer]
The answer is 42.
```
Deltas flow together naturally. Provider-sent newlines create sentence boundaries.

## 4. Key Source Files

| File | Change |
|------|--------|
| `src/app.rs` | Change `eprintln!` to `eprint!` for thinking text; add space separator for collapsed mode |

No other source files need modification.

## 5. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Very long lines in collapsed mode | All deltas on one line could exceed terminal width | Terminal will wrap; this is better than per-token line breaks |
| Missing spaces between expanded deltas | Provider tokens that don't include spaces could concatenate | Most providers include appropriate whitespace in streaming tokens |
| Trailing space in collapsed mode | Last delta has a space before `[answer]` | The `\n` in `\n[answer]` creates separation; trailing space is minor |
| `eprint!` buffering | Output may not flush immediately on some systems | Stderr is typically unbuffered; if needed, add explicit flush |

## 6. Handoff to Implementation Phase

The implementation phase should:

1. Change `eprintln!("{collapsed}")` to `eprint!("{collapsed}")` in collapsed mode, with space separator for subsequent deltas.
2. Change `eprintln!("{text}")` to `eprint!("{text}")` in expanded mode.
3. Verify `cargo build` and `cargo test` pass.