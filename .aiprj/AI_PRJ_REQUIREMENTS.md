# Requirements Specification (AI_PRJ_REQUIREMENTS)

## 1. Project Overview

### 1.1 Project Name

English Conversion and Full-Width to Half-Width Character Normalization for the agent-cli Repository

### 1.2 Background

The following instructions were received in `.aiprj/instructions.md` (as of 2026-05-11):

> - Convert all Japanese text in files to English
> - Convert full-width characters to half-width characters

Numerous files in the agent-cli repository (documentation, configuration, source code comments, etc.) contain Japanese text and full-width alphanumeric characters and symbols. Converting these to English text and half-width characters will improve readability and maintainability for international developers.

### 1.3 Objectives

- Eliminate Japanese text (hiragana, katakana, kanji, full-width symbols) from all files in the repository and replace it with equivalent English text.
- Convert all full-width characters (alphanumeric characters and symbols) in all files in the repository to their corresponding half-width characters.
- Maintain the structure, intent, and information content of the original documents after conversion.

### 1.4 Non-Objectives

- No new features will be added and no existing feature specifications will be changed.
- No logic changes will be made to source code (Rust, etc.). Only comments will be translated.
- No changes will be made to external service or API calls.
- The report `./report_cli.md` will not be reviewed or supplemented with additional research (translation only).
- If `README.en.md` already exists as the English version, consistency with `README.md` will be verified after converting the Japanese text in `README.md` to English, but `README.en.md` itself will not be substantially modified.

## 2. Terminology

| Term | Definition |
|------|-----------|
| Japanese text | Text containing hiragana, katakana, kanji, or full-width symbols |
| Full-width characters | Alphanumeric characters and symbols belonging to full-width character sets such as JIS X 0208 (e.g., A->A, 1->1) |
| Half-width characters | Alphanumeric characters and symbols belonging to the ASCII character set |
| Conversion target files | Files in the repository that contain Japanese text or full-width characters |
| English conversion | Translating Japanese text into equivalent English text |
| Half-width conversion | Converting full-width alphanumeric characters and symbols to their corresponding half-width characters |

## 3. Functional Requirements

### 3.1 English Conversion of Japanese Text (FR-01)

- Scan all files in the repository and identify files containing Japanese text.
- Translate the Japanese text in each file into equivalent English while preserving context.
- The translation should be natural English suitable for technical documentation, not machine translation.
- Verify that the translated text conveys the same amount of information as the original text.

### 3.2 Half-Width Conversion of Full-Width Characters (FR-02)

- Scan all files in the repository and identify files containing full-width alphanumeric characters and symbols.
- Convert full-width alphanumeric characters (A-Z, a-z, 0-9) to their corresponding half-width alphanumeric characters (A-Z, a-z, 0-9).
- Convert full-width symbols (_, -, (, ), etc.) to their corresponding half-width symbols (_, -, (, ), etc.).
- Punctuation marks within Japanese text (.,) will be converted to English punctuation (., ) as part of the FR-01 translation.

### 3.3 Quality Assurance of Conversion (FR-03)

- Verify that translated files retain the same structure as the original files (headings, tables, lists, code blocks, etc.).
- Verify that Markdown structural integrity (links, code blocks, tables, front matter, etc.) is maintained.
- Ensure that translating string literals and comments in source code (Rust) does not affect program behavior.
- When the same term is used across multiple files, use the same English translation consistently (terminological consistency).

### 3.4 Classification of Conversion Targets (FR-04)

Classify conversion target files into the following categories and apply conversion policies appropriate to each category.

| Category | Target Files | Conversion Policy |
|----------|-------------|-------------------|
| Documentation | `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `doc/*.md`, `example/agents/*.md`, `report_cli.md` | Convert entire body text to English. Convert full-width characters to half-width. |
| Source code | `src/**/*.rs` | Convert only comments and doc comments to English. Convert user-facing messages among string literals to English. Do not modify internal identifiers or function names. |
| Project management | `.aiprj/AI_PRJ_REQUIREMENTS.md`, `.aiprj/AI_PRJ_DESIGN.md`, `.aiprj/AI_PRJ_TASKS.md` | Written in Japanese during this update phase (per the instruction "Output language: Japanese"). Subject to English conversion in the implementation phase. |
| Project management (other) | `.aiprj/README.md`, `.aiprj/rules/*.md` | Subject to English conversion and half-width conversion. However, `.aiprj/instructions.md` is the source of the conversion instructions and is already in English, so no conversion is needed. |

### 3.5 Exclusions (FR-05)

- Files in the `.git/` directory are excluded from conversion.
- Generated files in the `target/` directory are excluded from conversion.
- Files in external dependency directories such as `node_modules/` are excluded from conversion.
- Binary files and image files are excluded from conversion.
- `.aiprj/instructions.md` is already in English and requires no conversion.

## 4. Non-Functional Requirements

### 4.1 Document Language (NFR-01)

- All converted files will be written in English.
- Full-width alphanumeric characters and symbols will be converted to half-width.
- However, during this update phase, `.aiprj/AI_PRJ_REQUIREMENTS.md`, `.aiprj/AI_PRJ_DESIGN.md`, and `.aiprj/AI_PRJ_TASKS.md` will be written in Japanese (per the instruction "Output language: Japanese").

### 4.2 Consistency (NFR-02)

- When the same term is used across multiple files, use the same English translation consistently (terminological consistency).
- Section numbers and requirement IDs will maintain the original structure.
- Maintain content consistency between `README.md` and `README.en.md`.

### 4.3 Safety (NFR-03)

- When translating source code, if changes to string literals affect test expected values or output verification, update the test code accordingly.
- Sensitive information such as API keys must not be leaked during the translation and conversion process.
- Execution commands within code blocks will not be translated and will be preserved as-is.

## 5. Constraints

- Write access in this repository is limited to `.aiprj/AI_PRJ_REQUIREMENTS.md`, `.aiprj/AI_PRJ_DESIGN.md`, `.aiprj/AI_PRJ_TASKS.md`, and log files under `.aiprj/AI_LOG/` (Article 3 of `.aiprj/rules/update_project.md`).
- Actual file conversion will be performed in the implementation phase (work started in a separate session via `/ai`, etc.). This update phase only covers updating the requirements specification, design document, and task list.
- Requirements, design, and tasks associated with the former `.aiprj/instructions.md` (other agent CLI investigation tasks) will be replaced by this update.

## 6. Intended Users

- Developers of the agent-cli project: International developers who use the English-converted documentation and code.
- Contributors: The barrier to entry is lowered by converting documents such as `CONTRIBUTING.md` to English.

## 7. Acceptance Criteria

- This document, `AI_PRJ_DESIGN.md`, and `AI_PRJ_TASKS.md` have been updated with content consistent with the conversion task in the current `.aiprj/instructions.md` (Article 1).
- There are no contradictions among the three documents (Article 2). Specifically:
  - The functional requirements in Section 3 of this document correspond to the conversion policies in Section 2 of `AI_PRJ_DESIGN.md`.
  - For each requirement in Section 3 of this document, a corresponding task is assigned in `AI_PRJ_TASKS.md`.
- All files in the repository containing Japanese text have been identified and assigned to conversion tasks.
- As the completion criterion for the implementation phase, all Japanese text in conversion target files has been replaced with English, and all full-width characters have been converted to half-width.