# Task List (AI_PRJ_TASKS)

This document breaks down the implementation tasks from `AI_PRJ_REQUIREMENTS.md` and the design in `AI_PRJ_DESIGN.md`. Checkboxes will be updated as tasks are completed.

## Legend

- `[ ]`: Not started
- `[~]`: In progress
- `[x]`: Completed
- Each task is associated with a requirement ID (FR-xx) and/or a design section number (Section x.x).

---

## Milestone M1: State Tracking Implementation

- [x] **T-001** Add `DisplayState` struct to `src/app.rs` with `thinking_printed: bool` and `answer_printed: bool` fields, and a `DisplayState::new()` constructor that initializes both to `false`. FR-01/FR-02/FR-04 / Design Section 3.1.
- [x] **T-002** Modify `display_event` function signature to accept `&mut DisplayState` as a third parameter. FR-01/FR-02 / Design Section 4.1.
- [x] **T-003** Modify the call site in `src/app.rs` to create a `DisplayState` instance and pass it to `display_event`. FR-01/FR-02 / Design Section 4.2.

## Milestone M2: Display Behavior Changes

- [x] **T-010** Modify the `AgentEvent::Thinking` branch in `display_event` to print `[thinking]` only when `state.thinking_printed` is `false`, then set `thinking_printed = true`. FR-01 / Design Section 3.2. -- For collapsed mode: suppress subsequent thinking events. For expanded mode: continue printing thinking text without an additional label.
- [x] **T-011** Modify the `AgentEvent::Text` branch in `display_event` to print `[answer]` on stderr when `state.answer_printed` is `false`, then set `answer_printed = true`. FR-02 / Design Section 3.3. -- The `[answer]` label is printed regardless of `show_thinking` mode.
- [x] **T-012** Modify the `AgentEvent::Done` and `AgentEvent::Error` branches to reset `DisplayState` (set `thinking_printed = false` and `answer_printed = false`). FR-04 / Design Section 3.4.

## Milestone M3: Verification

- [x] **T-020** Run `cargo build` and verify it compiles successfully. NFR-03. -- Passed.
- [x] **T-021** Run `cargo test` and verify all tests pass. NFR-03. -- All 80 tests passed.
- [x] **T-022** Verify the display behavior: `[thinking]` appears only once per turn, `[answer]` appears once per turn, and state resets between turns. FR-01/FR-02/FR-04. -- Verified via code review and test pass.

---

## Known Notes

- The `show_thinking` configuration is confirmed to be config.toml-only (`[ui] show_thinking`). There is no CLI flag for this setting (FR-05).
- `DisplayState` is only needed in `src/app.rs`; no changes to other source files.
- The `[answer]` label is printed on stderr, the same destination as `[thinking]`.
- In `Hidden` mode, `[thinking]` and thinking text are suppressed, but `[answer]` is still printed per FR-02.
- `AgentEvent::Error` also resets `DisplayState` since the turn ends on error.

## Work Log

- **2026-05-12 (Update Phase)**: Instructions updated to include feature request for start-only `[thinking]` label and new `[answer]` label. Requirements, design, and tasks fully rewritten. Previous investigation-only task is replaced.
- **2026-05-12 (Implementation Phase)**: All tasks completed. Added `DisplayState` struct with `thinking_printed` and `answer_printed` flags. Modified `display_event` to print `[thinking]` only once per turn and `[answer]` once per turn. State resets on `Done` and `Error` events. `cargo build` and `cargo test` (80 tests) pass successfully.