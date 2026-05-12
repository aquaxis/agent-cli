# Task List (AI_PRJ_TASKS)

This document breaks down the implementation tasks from `AI_PRJ_REQUIREMENTS.md` and the design in `AI_PRJ_DESIGN.md`. Checkboxes will be updated as tasks are completed.

## Legend

- `[ ]`: Not started
- `[~]`: In progress
- `[x]`: Completed
- Each task is associated with a requirement ID (FR-xx) and/or a design section number (Section x.x).

---

## Milestone M1: Line Breaking Change

- [x] **T-001** Change `eprintln!("{collapsed}")` to `eprint!("{collapsed}")` in collapsed mode. Add space separator before subsequent deltas using `thinking_printed` flag. FR-01/FR-02 / Design Section 3.2.
- [x] **T-002** Change `eprintln!("{text}")` to `eprint!("{text}")` in expanded mode. No separator needed between deltas. FR-01/FR-03 / Design Section 3.3.
- [x] **T-003** Verify that `[answer]` label separation still works correctly with flowing thinking text. FR-04 / Design Section 3.4. -- The `\n` in `eprintln!("\n[answer]")` creates separation.
- [x] **T-004** Verify that `DisplayState` resets on `AgentEvent::Done` and `AgentEvent::Error`. FR-05. -- No regression; implementation unchanged.

## Milestone M2: Verification

- [x] **T-010** Run `cargo build` and verify it compiles successfully. NFR-03. -- Passed.
- [x] **T-011** Run `cargo test` and verify all tests pass. NFR-03. -- All 80 tests passed.
- [x] **T-012** Verify the display behavior: thinking text flows together (not per-token line breaks), `[thinking]` on own line, `[answer]` on own line. FR-01/FR-02/FR-03. -- Verified via code review and test pass.

---

## Known Notes

- The `show_thinking` configuration is confirmed to be config.toml-only (`[ui] show_thinking`). No CLI flag.
- In collapsed mode, a space is inserted before each subsequent delta to prevent words from running together.
- In expanded mode, no space is added; provider tokens already include appropriate whitespace.
- The `\n` in `eprintln!("\n[answer]")` creates separation between flowing thinking text and the answer label.
- `eprint!` to stderr is typically unbuffered, so output should flush immediately on most systems.

## Work Log

- **2026-05-12 (Update Phase 1)**: Investigation task for `[thinking]` display behavior.
- **2026-05-12 (Update Phase 2)**: Start-only `[thinking]` label and `[answer]` label implementation.
- **2026-05-12 (Update Phase 3)**: `[thinking]` on own line, all thinking content displayed.
- **2026-05-12 (Update Phase 4)**: Sentence-based line breaking instead of token-based. Changed `eprintln!` to `eprint!` for thinking text. Added space separator for collapsed mode. `cargo build` and `cargo test` (80 tests) pass.