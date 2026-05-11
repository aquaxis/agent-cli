# Task List (AI_PRJ_TASKS)

This document breaks down the conversion requirements from `AI_PRJ_REQUIREMENTS.md` and the conversion policies from `AI_PRJ_DESIGN.md` into executable tasks. Checkboxes will be updated sequentially in the implementation phase (separate session).

## Legend

- `[ ]`: Not started
- `[~]`: In progress
- `[x]`: Completed
- Each task is associated with a requirement ID (FR-xx) and/or a design section number (Section x.x).

---

## Milestone M1: Conversion Preparation

- [ ] **T-001** Final confirmation and listing of conversion target files. FR-04 / Design Section 3. -- Re-scan files in the repository containing Japanese text and full-width characters, and verify there are no differences from the file list in Design Section 3.
- [ ] **T-002** Create the base set of the terminology mapping (Design Section 2.1.1). FR-01 / Design Section 2.1.1. -- Establish English translations for project-specific terms (agent, provider, tool, etc.) as the standard for consistency.
- [ ] **T-003** Run `cargo build` and `cargo test` before conversion and record the baseline. FR-03 / Design Section 2.3.3.

## Milestone M2: Documentation File Conversion

- [ ] **T-101** Convert Japanese text in `README.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1. -- Verify consistency with `README.en.md` after translation.
- [ ] **T-102** Convert Japanese text in `CHANGELOG.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-103** Convert Japanese text in `CONTRIBUTING.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-104** Convert Japanese text in `doc/architecture.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-105** Convert Japanese text in `doc/config.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-106** Convert Japanese text in `doc/personas.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-107** Convert Japanese text in `doc/providers/claude.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-108** Convert Japanese text in `doc/providers/codex.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-109** Convert Japanese text in `doc/providers/llamacpp.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-110** Convert Japanese text in `doc/providers/ollama.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-111** Convert Japanese text in `doc/tools.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-112** Convert Japanese text in `doc/troubleshooting.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-113** Convert Japanese text in `doc/usage.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-114** Convert Japanese text in `example/agents/coder.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-115** Convert Japanese text in `example/agents/planner.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-116** Convert Japanese text in `example/agents/reviewer.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.
- [ ] **T-117** Convert Japanese text in `report_cli.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.1.

## Milestone M3: Source Code File Conversion

- [ ] **T-201** Convert comments and doc comments in `src/agent.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-202** Convert comments and doc comments in `src/ai/claude.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-203** Convert comments and doc comments in `src/ai/codex.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-204** Convert comments and doc comments in `src/ai/llamacpp.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-205** Convert comments and doc comments in `src/ai/mod.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-206** Convert comments and doc comments in `src/ai/ollama.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-207** Convert comments and doc comments in `src/ai/stream.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-208** Convert comments and doc comments in `src/ai/tool_bridge.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-209** Convert comments and doc comments in `src/app.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-210** Convert comments and doc comments in `src/cli.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-211** Convert comments and doc comments in `src/commands.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-212** Convert comments and doc comments in `src/config.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-213** Convert comments and doc comments in `src/id.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-214** Convert comments and doc comments in `src/ipc/mod.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-215** Convert comments and doc comments in `src/ipc/registry.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-216** Convert comments and doc comments in `src/ipc/server.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-217** Convert comments and doc comments in `src/main.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-218** Convert comments and doc comments in `src/persona.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.
- [ ] **T-219** Convert comments and doc comments in `src/tools/mod.rs` to English. FR-01 / Design Section 3.2 / Section 2.1.2.

## Milestone M4: Project Management File Conversion

- [ ] **T-301** Convert Japanese text in `.aiprj/README.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.3.
- [ ] **T-302** Convert Japanese text in `.aiprj/rules/exec_job.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.3.
- [ ] **T-303** Convert Japanese text in `.aiprj/rules/update_project.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.3.
- [ ] **T-304** Convert Japanese text in `.aiprj/AI_PRJ_REQUIREMENTS.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.3.
- [ ] **T-305** Convert Japanese text in `.aiprj/AI_PRJ_DESIGN.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.3.
- [ ] **T-306** Convert Japanese text in `.aiprj/AI_PRJ_TASKS.md` to English and convert full-width characters to half-width. FR-01/FR-02 / Design Section 3.3.

## Milestone M5: Quality Verification

- [ ] **T-401** Verify Markdown structural integrity of documentation files (M2). FR-03 / Design Section 2.3.1. -- Verify that heading levels, table structures, and code block boundaries match before and after translation.
- [ ] **T-402** Verify that `cargo build` completes successfully after translating source code files (M3). FR-03 / Design Section 2.3.3.
- [ ] **T-403** Verify that `cargo test` completes successfully after translating source code files (M3). FR-03 / Design Section 2.3.3.
- [ ] **T-404** Verify terminological consistency across all files. FR-03 / Design Section 2.3.2. -- Verify that the same Japanese term is translated to the same English term, following the terminology mapping (T-002).
- [ ] **T-405** Verify content consistency between `README.md` and `README.en.md`. FR-03 / Design Section 5.
- [ ] **T-406** Use `grep` to verify that no Japanese text or full-width characters remain in the repository. FR-01/FR-02. -- However, exclude `.aiprj/instructions.md` and parts intentionally kept in Japanese.

## Milestone M6: Acceptance Verification

- [ ] **T-501** Convert the acceptance criteria (Requirements Section 7) into a checklist and verify that all items are satisfied.
- [ ] **T-502** Leave a work log in `.aiprj/AI_LOG/` (date of execution, major changes, remaining issues).
- [ ] **T-503** Finalize updates using `/close_ai` or similar.

---

## Known Notes

- `.aiprj/AI_PRJ_REQUIREMENTS.md`, `.aiprj/AI_PRJ_DESIGN.md`, and `.aiprj/AI_PRJ_TASKS.md` are written in Japanese during this update phase (per the instruction "Output language: Japanese"). English conversion and half-width conversion will be performed in the implementation phase (M4).
- `report_cli.md` is an investigation report output at the repository root and is a conversion target. No review of the content itself will be performed; only translation is in scope.
- Requirements, design, and tasks based on the former `.aiprj/instructions.md` (other agent CLI investigation tasks) have been completely replaced by this update. The previous tasks (M1-M6: investigation preparation through acceptance verification) are completed, and the new tasks correspond to the conversion work.
- In the Rust source code, `src/ai/claude.rs` and `src/ai/codex.rs` have `README.en.md` detected as a file containing Japanese text, but this is the English README and its content needs to be verified.

## Work Log

- **2026-05-11 (Update Phase)**: The content of `.aiprj/instructions.md` was changed to "Convert all Japanese text in files to English / Convert full-width characters to half-width characters," so the requirements specification, design document, and task list were fully updated. The previous tasks (agent CLI investigation) are completed, and the new tasks correspond to English conversion of Japanese text and half-width conversion of full-width characters. Consistency among the three documents was verified within this update (Requirements Section 3 <-> Design Section 2 / Section 3 <-> Tasks M1-M6).