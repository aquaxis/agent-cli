# Design Document (AI_PRJ_DESIGN)

Proposed design for **configurable llama.cpp sampling parameters**
(`AI_PRJ_REQUIREMENTS.md`). The llama.cpp backend talks to a llama.cpp server
over OpenAI-compatible `/v1/chat/completions`; the design forwards the user's
`llama-cli`-style sampling knobs as request-body fields.

## 1. Config (`src/config.rs`)

- `ProviderEntry` gains seven optional sampling fields, each
  `#[serde(default)]`:
  - `max_tokens: Option<u32>`  → server `max_tokens` (the `-n` / `n_predict`)
  - `top_k: Option<u32>`       → server `top_k`
  - `top_p: Option<f32>`       → server `top_p`
  - `min_p: Option<f32>`       → server `min_p`
  - `repeat_penalty: Option<f32>` → server `repeat_penalty`
  - `repeat_last_n: Option<i32>`  → server `repeat_last_n`
  - `seed: Option<u64>`        → server `seed`
- These live on the shared `ProviderEntry` (alongside the existing
  per-provider `prompt_cache` / `persistent_session` / `api`); they are
  llama.cpp-specific and simply never read by the other providers.
- `DEFAULT_CONFIG` template: commented opt-in example lines under
  `[provider."llama.cpp"]`, e.g.

  ```
  [provider."llama.cpp"]
  model    = "default"
  base_url = "http://127.0.0.1:8080"
  # Optional sampling knobs (omit ⇒ server default). Names match llama-cli:
  # max_tokens     = 1024   # -n / --n-predict
  # temperature    = 0.2    # --temp
  # top_k          = 80     # --top-k
  # repeat_penalty = 1.05   # --repeat-penalty
  # top_p          = 0.95   # --top-p
  # min_p          = 0.05   # --min-p
  # repeat_last_n  = 64     # --repeat-last-n
  # seed           = 0      # --seed
  ```

- Additive only: every existing config still parses; defaults (`None`)
  preserve current behavior.

## 2. Provider wiring (`src/ai/llamacpp.rs`)

- `LlamaCppProvider` gains the parsed sampling values. Either:
  - carry them as struct fields populated in `from_config` (matching how
    `temperature` is carried today), or
  - keep a small `Sampling` sub-struct on the provider.

  Recommended: individual `Option<…>` fields on `LlamaCppProvider`, mirroring
  the existing `temperature: Option<f32>` field, populated from `entry` in
  `from_config`.

- `complete_stream` body construction (after the existing `temperature`
  block):

  ```rust
  if let Some(v) = self.max_tokens     { body["max_tokens"]     = json!(v); }
  if let Some(v) = self.top_k          { body["top_k"]          = json!(v); }
  if let Some(v) = self.top_p          { body["top_p"]          = json!(v); }
  if let Some(v) = self.min_p          { body["min_p"]          = json!(v); }
  if let Some(v) = self.repeat_penalty { body["repeat_penalty"] = json!(v); }
  if let Some(v) = self.repeat_last_n  { body["repeat_last_n"]  = json!(v); }
  if let Some(v) = self.seed           { body["seed"]           = json!(v); }
  ```

- Each field is written **only when `Some`**, so an unset field is absent from
  the body and the server applies its own default — request body byte-identical
  to today when nothing is configured.

- To make the body testable, factor body construction into a pure helper
  (e.g. `fn build_body(&self, messages, tools) -> Value`) that
  `complete_stream` calls, so a unit test can assert the JSON without a live
  server.

## 3. Wire shape

- The llama.cpp server's `/v1/chat/completions` accepts `max_tokens` (OpenAI
  standard) plus the llama.cpp extensions `top_k`, `top_p`, `min_p`,
  `repeat_penalty`, `repeat_last_n`, `seed`. Field names are forwarded
  verbatim under those keys.
- No SSE / response-parsing change: `handle_llamacpp_frame` is unaffected.

## 4. Source Files Touched

| File | Change |
|------|--------|
| `src/config.rs` | 7 optional sampling fields on `ProviderEntry`; default-config template comments |
| `src/ai/llamacpp.rs` | carry sampling fields; forward present fields into the request body via a testable `build_body`; unit test |
| `doc/providers/llamacpp.md` | document the fields + `llama-cli` mapping (docs task, outside the AI_PRJ scope) |

## 5. Testing

- Unit test on `build_body` (or equivalent): with all sampling fields `None`,
  the body contains only `model`/`stream`/`messages` (+`tools`/`temperature`
  when set) — i.e. byte-compatible with today. With fields set, each appears
  under its server key with the correct value/type.
- Existing `handle_llamacpp_frame` tests are unaffected.

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| A field name not honored by the server | Names taken from the llama.cpp server's documented OpenAI-compatible fields; unknown fields are ignored by the server, not fatal |
| Polluting shared `ProviderEntry` with llama.cpp-only fields | Consistent with the existing per-provider-field pattern; fields are inert for other providers |
| Behavior drift for users with no sampling config | Each field written only when `Some` ⇒ empty config ⇒ body unchanged |
| Invalid value ranges (e.g. negative top_k) | Out of scope client-side; the server validates and returns a normal HTTP error surfaced by the existing error path |

## 7. Handoff

Design only — not yet implemented. Implementation is a small, additive change:
config fields + body forwarding + one unit test, plus a docs update to
`doc/providers/llamacpp.md`. With no sampling fields configured, the request
body and all current behavior are unchanged.
