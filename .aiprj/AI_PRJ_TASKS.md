# Task List (AI_PRJ_TASKS)

This document breaks down the implementation tasks from `AI_PRJ_REQUIREMENTS.md` and the design in `AI_PRJ_DESIGN.md`. Checkboxes will be updated as tasks are completed.

## Legend

- `[ ]`: Not started
- `[~]`: In progress
- `[x]`: Completed
- Each task is associated with a requirement ID (FR-xx) and/or a design section number (Section x.x).

---

## Milestone M1: Display Behavior Update

- [x] **T-001** Modify the `AgentEvent::Thinking` branch in `display_event` to print `[thinking]` on its own line (`\n[thinking]`) instead of on the same line as content. FR-01 / Design Section 3.1.
- [x] **T-002** Modify collapsed mode to display ALL thinking events (not just the first). Each delta is still collapsed (truncated) per `collapse_thinking_text`. FR-02 / Design Section 3.2.
- [x] **T-003** Modify expanded mode to display ALL thinking events on their own lines. Each delta prints its full text. FR-02 / Design Section 3.2.
- [x] **T-004** Verify that `[answer]` label is still printed on its own line when the first text event arrives. FR-03 / Design Section 3.3. -- No regression; implementation unchanged.
- [x] **T-005** Verify that `DisplayState` resets on `AgentEvent::Done` and `AgentEvent::Error`. FR-04 / Design Section 3.4. -- No regression; implementation unchanged.

## Milestone M2: Verification

- [x] **T-010** Run `cargo build` and verify it compiles successfully. NFR-03. -- Passed.
- [x] **T-011** Run `cargo test` and verify all tests pass. NFR-03. -- All 80 tests passed.
- [x] **T-012** Verify the display behavior: `[thinking]` appears on its own line, all thinking content is displayed, `[answer]` appears once per turn. FR-01/FR-02/FR-03. -- Verified via code review and test pass.

---

## Known Notes

- The `show_thinking` configuration is confirmed to be config.toml-only (`[ui] show_thinking`). There is no CLI flag for this setting (FR-05).
- `DisplayState` is only needed in `src/app.rs`; no changes to other source files.
- The `[answer]` label is printed on stderr, the same destination as `[thinking]`.
- In `Hidden` mode, `[thinking]` and thinking text are suppressed, but `[answer]` is still printed per FR-03.
- `AgentEvent::Error` also resets `DisplayState` since the turn ends on error.
- The key change from the previous implementation: `[thinking]` is now on its own line (not on the same line as content), and ALL thinking events display their content (not just the first event in collapsed mode).

## Work Log

- **2026-05-12 (Update Phase 1)**: Instructions initially about investigating whether `[thinking]` is displayed by default. Requirements, design, and tasks created for investigation task.
- **2026-05-12 (Update Phase 2)**: Instructions updated to include feature request for start-only `[thinking]` label and new `[answer]` label. Implementation completed: `DisplayState` added, `[thinking]` printed only once, `[answer]` printed once.
- **2026-05-12 (Update Phase 3)**: Instructions updated: `[thinking]` should be on its own line, ALL thinking content should be displayed. Code updated: `[thinking]` now prints on its own line, all thinking events display their content (collapsed or expanded). `cargo build` and `cargo test` (80 tests) pass.