# Requirements Specification (AI_PRJ_REQUIREMENTS)

## 1. Project Overview

### 1.1 Project Name

Configurable llama.cpp sampling parameters — let the user set generation knobs
such as `-n` (max tokens), `--repeat-penalty`, and `--top_k` for the llama.cpp
backend instead of relying on hard-coded defaults.

### 1.2 Background

`.aiprj/instructions.md` shows the user driving a model via the `llama-cli`
binary with explicit sampling flags:

```
./build/bin/llama-cli -m models/LFM2.5-8B-A1B-BF16.gguf -p "…" \
  -n 1024 --temp 0.2 --top_k 80 --repeat-penalty 1.05
```

The user wants the same parameters (`-n 1024`, `--repeat-penalty 1.05`, etc.)
to be configurable when running through agent-cli, and asks for a proposal.

agent-cli's llama.cpp backend (`LlamaCppProvider`) does **not** spawn
`llama-cli`; it talks to a running **llama.cpp server** over the
OpenAI-compatible `/v1/chat/completions` endpoint (`doc/providers/llamacpp.md`).
Today the request body forwards only `model`, `messages`, `stream`, optional
`tools`, and optional `temperature` (`src/ai/llamacpp.rs`). Every other
sampling knob is left at the server's default, so the `llama-cli` flags above
have no agent-cli equivalent. The llama.cpp server accepts these same knobs as
request-body fields, so they can be surfaced through config and forwarded.

### 1.3 Flag → server-field mapping

`llama-cli` CLI flags map onto llama.cpp server request-body fields:

| `llama-cli` flag | Server request field | Meaning |
|------------------|----------------------|---------|
| `-n, --n-predict` | `max_tokens` (alias `n_predict`) | Max tokens to generate |
| `--temp` | `temperature` | Sampling temperature (already supported) |
| `--top-k` | `top_k` | Top-K sampling |
| `--top-p` | `top_p` | Top-P (nucleus) sampling |
| `--min-p` | `min_p` | Min-P sampling |
| `--repeat-penalty` | `repeat_penalty` | Repetition penalty |
| `--repeat-last-n` | `repeat_last_n` | Window for the repeat penalty |
| `--seed` | `seed` | RNG seed (reproducibility) |

`top_k`, `min_p`, `repeat_penalty`, `repeat_last_n`, and `seed` are llama.cpp
extensions to the OpenAI schema; the llama.cpp server honors them on
`/v1/chat/completions`.

### 1.4 Scope Decision (proposal)

- **Surface as explicit typed config fields** on the llama.cpp provider entry,
  mirroring the existing per-provider field pattern (`prompt_cache`,
  `persistent_session`, `api`). Each is `Option<…>`, `#[serde(default)]`.
- **Opt-in / additive:** every field absent ⇒ omitted from the request body ⇒
  the llama.cpp server applies its own default ⇒ **behavior byte-for-byte
  unchanged** from today.
- **Scope to the llama.cpp backend only.** Other providers (claude/codex/
  ollama/opencode) are untouched. `temperature` is already shared and keeps
  working.

### 1.5 Objectives

1. Add llama.cpp sampling config fields: `max_tokens`, `top_k`, `top_p`,
   `min_p`, `repeat_penalty`, `repeat_last_n`, `seed`.
2. Forward each present field into the `/v1/chat/completions` request body in
   `LlamaCppProvider::complete_stream`.
3. Document the fields (default config template + `doc/providers/llamacpp.md`).

### 1.6 Non-Objectives

- No spawning of / dependency on the `llama-cli` binary; the server transport
  is unchanged.
- No new sampling parameters beyond the table in §1.3 (e.g. `mirostat`,
  `tfs_z`, grammar) — out of scope unless requested.
- No per-turn / interactive override (CLI flag or slash command) of these
  values; configuration is via the config file.
- No change to other providers or to the shared `temperature` handling.
- No client-side validation of value ranges beyond TOML type parsing; invalid
  values are the server's to reject.

## 2. Terminology

| Term | Definition |
|------|-----------|
| llama.cpp server | A running `llama-server` exposing `/v1/chat/completions` |
| Sampling field | A generation knob forwarded in the request body |
| `n_predict` | llama.cpp's native name for the `-n` / `max_tokens` limit |
| Present field | A config value that is `Some(..)` and therefore serialized |

## 3. Functional Requirements

### 3.1 Sampling config fields (FR-01)

- `[provider."llama.cpp"]` accepts optional keys: `max_tokens` (int),
  `top_k` (int), `top_p` (float), `min_p` (float), `repeat_penalty` (float),
  `repeat_last_n` (int), `seed` (int).
- All `#[serde(default)]` ⇒ omitting any/all of them still parses every
  existing config unchanged.

### 3.2 Request-body forwarding (FR-02)

- `LlamaCppProvider::complete_stream` adds each **present** sampling field to
  the JSON body using its server field name (`max_tokens`, `top_k`, `top_p`,
  `min_p`, `repeat_penalty`, `repeat_last_n`, `seed`).
- An **absent** field is not written to the body, so the server default applies
  (current behavior). `temperature` continues to be forwarded as today.

### 3.3 Documentation (FR-03)

- `DEFAULT_CONFIG` template gains commented example lines for the new fields
  under `[provider."llama.cpp"]`.
- `doc/providers/llamacpp.md` documents each field with its `llama-cli`
  equivalent (the §1.3 mapping) and notes that omitted fields fall back to the
  server default.

## 4. Non-Functional Requirements

- **NFR-01** Docs in English; Requirements/Design/Tasks mutually consistent.
- **NFR-02** `cargo build` / `cargo test` / `cargo clippy` clean; the new
  body-building logic unit-tested.
- **NFR-03** Additive, non-breaking config (`Option` + `#[serde(default)]`):
  all existing configs still parse; with no sampling fields set, the request
  body is byte-identical to today.
- **NFR-04** Field names on the wire match what the llama.cpp server expects
  (verified against the server's documented OpenAI-compatible fields).

## 5. Constraints

- Source changes confined to `src/config.rs` (new optional fields on
  `ProviderEntry` + default template) and `src/ai/llamacpp.rs` (body
  building + tests). Docs: `doc/providers/llamacpp.md`.
- `ProviderEntry` is the shared provider struct; the new fields are llama.cpp
  -specific but live there to match the existing per-provider-field pattern
  (`prompt_cache`/`persistent_session`/`api`), keeping them inert for other
  providers.

## 6. Acceptance Criteria

- The seven sampling fields parse from `[provider."llama.cpp"]`; omitting them
  leaves the request body unchanged from today.
- Present fields appear in the `/v1/chat/completions` body under their server
  names; absent fields do not.
- A unit test asserts body construction (present ⇒ included with correct name,
  absent ⇒ omitted).
- `cargo build`/`test`/`clippy` green; existing tests unaffected.
- Requirements/Design/Tasks consistent; llama.cpp provider doc updated.
