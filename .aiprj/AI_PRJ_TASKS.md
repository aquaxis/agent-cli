# Task List (AI_PRJ_TASKS)

Implementation breakdown for **configurable llama.cpp sampling parameters**
(`AI_PRJ_REQUIREMENTS.md` / `AI_PRJ_DESIGN.md`).

## Legend

`[ ]` not started · `[~]` in progress · `[x]` done.

---

## M0: Scope

- [x] **T-000** Map `instructions.md`'s `llama-cli` flags (`-n`,
  `--repeat-penalty`, `--top_k`, …) onto llama.cpp server request-body fields;
  decide surface = explicit opt-in typed config fields, default OFF (omitted ⇒
  server default). Rewrite the three AI_PRJ docs.

## M1: Config

- [x] **T-010** `src/config.rs`: add `max_tokens`, `top_k`, `top_p`, `min_p`,
  `repeat_penalty`, `repeat_last_n`, `seed` (all `Option`, `#[serde(default)]`)
  to `ProviderEntry`; add commented example lines under `[provider."llama.cpp"]`
  in `DEFAULT_CONFIG`. FR-01, NFR-03 / Design 1.

## M2: Provider wiring

- [x] **T-020** `src/ai/llamacpp.rs`: carry the sampling fields on
  `LlamaCppProvider` (populated in `from_config`, mirroring `temperature`).
  FR-01 / Design 2.
- [x] **T-021** Factor request-body construction into a testable helper
  (`build_body`) and forward each **present** sampling field under its
  server key (`max_tokens`/`top_k`/`top_p`/`min_p`/`repeat_penalty`/
  `repeat_last_n`/`seed`); absent ⇒ omitted. FR-02 / Design 2.

## M3: Verification

- [x] **T-030** Unit tests on `build_body`: all-`None` ⇒ body byte-compatible
  with today (no `temperature`/sampling keys); fields set ⇒ each appears under
  the correct server key/type. NFR-02 / Design 5.
- [x] **T-031** `cargo build`/`clippy` clean; `cargo test` **104/104** (was
  102; +2); existing tests unaffected. NFR-02/03.
- [~] **T-032** Live check against a real `llama-server` (e.g. confirm
  `max_tokens`/`repeat_penalty` take effect) — deferred to user; no live
  backend in this environment. FR-02.

## M4: Documentation

- [x] **T-040** `doc/providers/llamacpp.md`: new "Sampling Parameters" section
  documenting each field with its `llama-cli` equivalent + server field name,
  and the "omit ⇒ server default" rule. FR-03.

---

## Notes

- The llama.cpp backend is a **server** client (OpenAI-compatible
  `/v1/chat/completions`), not a `llama-cli` subprocess. The `llama-cli` flags
  in `instructions.md` map onto request-body fields (see Requirements §1.3).
- All sampling fields are **opt-in**: each is written to the body only when
  `Some`, so an empty config yields a request body byte-identical to today.
- `temperature` is already supported (shared `ProviderEntry` field) and keeps
  working; this project adds the remaining knobs.
- `top_k` / `min_p` / `repeat_penalty` / `repeat_last_n` / `seed` are llama.cpp
  extensions to the OpenAI schema, honored by the llama.cpp server.

## Work Log

- **2026-05-16/17 (prior projects — opencode provider + context-efficiency +
  DeepSeek wire fixes)**: opencode provider, three opt-in context-efficiency
  features (persistent session / Claude prompt cache / hybrid history
  management), the opencode dual cloud `api` selector, and the DeepSeek
  `tool_calls` (FR-04) and thinking-mode `reasoning_content` (FR-05) wire
  fixes. Completed and committed on `feat/opencode-context-efficiency`
  (`cargo test` 102/102). See git history (commits `fdab7e4`…`d5950ea`) and
  `AI_LOG/2026-05-16_*` / `2026-05-17_*` for the full record.
- **2026-06-04 (llama.cpp sampling parameters — docs)**: `instructions.md`
  replaced with a request to make `llama-cli` sampling flags (`-n 1024`,
  `--repeat-penalty 1.05`, `--top_k`, …) configurable for the llama.cpp
  backend. Mapped the flags onto llama.cpp server request-body fields and
  rewrote the three AI_PRJ docs to specify an opt-in, additive set of typed
  config fields forwarded into `/v1/chat/completions`.
- **2026-06-04 (llama.cpp sampling parameters — implementation)**: Implemented
  T-010…T-040. `src/config.rs`: 7 optional sampling fields (`max_tokens`,
  `top_k`, `top_p`, `min_p`, `repeat_penalty`, `repeat_last_n`, `seed`) on
  `ProviderEntry` + commented `DEFAULT_CONFIG` examples. `src/ai/llamacpp.rs`:
  carry the fields on `LlamaCppProvider` (populated in `from_config`), factor
  request-body construction into a pure `build_body`, forward each present
  field under its server key; 2 unit tests (all-`None` omits everything; set
  fields forwarded with correct key/type). `doc/providers/llamacpp.md`: new
  "Sampling Parameters" section. `cargo build`/`clippy` clean, `cargo test`
  **104/104** (+2). With no sampling fields configured the request body is
  byte-identical to before. T-032 live check deferred (no `llama-server` in
  this environment). Log: `AI_LOG/2026-06-04_000.md`.
