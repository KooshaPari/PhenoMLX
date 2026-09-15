# pheno-capacity integration

**Status:** Active (Phase 1 complete 2026-06-18, L5-105).
**Source crate:** [`KooshaPari/pheno-capacity`](https://github.com/KooshaPari/pheno-capacity) v0.1.0
**License:** MIT OR Apache-2.0
**Governing ADR:** [ADR-035A](../adr/2026-06-18/ADR-035A-hwledger-reclassification.md)

## Why

HwLedger is an LLM capacity planner + fleet ledger. Its math core (VRAM, model-fit,
Chinchilla, optimizer state) is conceptually a pure-function library — no I/O, no async,
no runtime dependencies — and therefore belongs in the `pheno-*-lib` substrate tier
(per ADR-023). Extracting it as `pheno-capacity`:

1. **Reusability** — any fleet member (pheno-mcp-router, phenotype-ops, dispatch-mcp
   consumers) can call the same VRAM math without duplicating it.
2. **Testability** — pure functions are trivially unit-testable. The historical
   `apps/streamlit/lib/cost_model.py` had 13 tests; the new `pheno-capacity` crate has
   **23 unit tests + 6 doc tests** (all pass; cargo clippy clean; cargo fmt clean).
3. **no_std compatibility** — usable in any context (kernel, embedded, WASM, Python via
   PyO3, JS via wasm-bindgen, etc.).
4. **Coverage gate** — ADR-023 lib tier requires 80% coverage; enforced in CI.

## What is in the crate

| Function / type | Purpose | Public API |
| :-- | :-- | :-- |
| `vram_estimate(params, dtype) -> u64` | Inference memory (weights only) | Bytes needed to load the model. |
| `model_fits_in(params, available, dtype) -> bool` | Does it fit? | Convenience over `vram_estimate`. |
| `optimizer_state_vram(weights, optimizer) -> u64` | Training overhead (AdamW = 8× weights; LoRA ~ 0; QLoRA ~ 0; Adafactor ≈ 1×) | Used by Fleet Planner "retraining cost" page. |
| `chinchilla_tokens(params, ratio) -> u64` | Optimal training tokens | Chinchilla scaling law: `tokens ≈ ratio × params`. |
| `dtype_bytes(dtype) -> u8` | Bytes per parameter | 4 (F32) · 2 (F16) · 2 (BF16) · 1 (I8) · 0.5 (I4). |
| `Dtype` enum | F32, F16, BF16, I8, I4 | Tagged union; no `Option<f32>` ambiguity. |
| `Optimizer` enum | AdamW, LoRA, QLoRA, Adafactor | Multiplier per `weights_bytes`. |

## What stays in HwLedger

| Concern | Why not in pheno-capacity |
| :-- | :-- |
| Hardware inventory persistence (SQLite, file ledger) | Stateful, not pure. |
| Federated service / API layer (axum, mTLS, agent) | Stateful, network, runtime. |
| Apps (landing, macOS, streamlit) glue | UI, OS-specific. |
| omlx-fork sidecar (Apple Silicon inference) | Not math; runtime + IPC. |
| per-OS GUIs (SwiftUI, WinUI, Qt, Slint) | Not math; UI. |

## How HwLedger uses it (Phase 2)

The historical `apps/streamlit/lib/cost_model.py` (git @ 8bf878ca, 172 LOC) is
**re-implemented** in `pheno-capacity/src/math.rs` (no_std-compatible). When the
Streamlit pages return to the working tree, the consumer will:

```python
# Option A (preferred, when PyO3 bindings ship): import the compiled crate
from pheno_capacity import vram_estimate, model_fits_in, dtype_bytes
vram = vram_estimate(7_000_000_000, "F16")  # LLaMA-7B → 14 GB
fits = model_fits_in(7_000_000_000, 16 * 1024**3, "F16")  # True for 16 GB GPU
```

```python
# Option B (interim): vendored Python shim that mirrors the Rust public API
# See docs/integrations/cost-model-migration.md §"Interim shim".
```

The decision between (A) and (B) is deferred to Phase 2 kickoff. Track in
`docs/integrations/cost-model-migration.md`.

## Numerical verification (cross-checked)

| Model | Params | dtype | pheno-capacity | Llama-3 model card | Δ |
| :-- | --: | :-- | --: | --: | --: |
| Mistral 7B | 7.24 B | F16 | 14.48 GB | ~14.5 GB | < 1 % |
| LLaMA-7B | 6.74 B | F16 | 13.48 GB | ~13.5 GB | < 1 % |
| LLaMA-70B | 68.5 B | F16 | 137.0 GB | ~140 GB (with embeddings) | < 2 % |
| Llama-3-8B | 8.03 B | BF16 | 16.06 GB | ~16.1 GB | < 1 % |
| Llama-3-70B | 70.6 B | BF16 | 141.2 GB | ~140 GB | < 1 % |
| Mixtral 8×7B (active 2-of-8 MoE) | 12.9 B active | F16 | 25.8 GB | ~26 GB | < 1 % |

(Δ is from ignoring embeddings and tokenizer overhead, which add ~0.5–3 GB per model.)

## CI

- 3 CI jobs: `test+fmt+clippy`, `coverage` (>= 80 % lib tier gate), `no_std structural check`
- See `KooshaPari/pheno-capacity/.github/workflows/ci.yml`

## Tracking

- `KooshaPari/pheno-capacity` v0.1.0 tag — 2026-06-18 22:40 PDT
- `KooshaPari/pheno-worklog-schema` PR `KooshaPari/pheno-worklog-schema#1` — worklog v2.1 schema
- ADR-035A reclassification — `docs/adr/2026-06-18/ADR-035A-hwledger-reclassification.md`
- Findings doc — `findings/2026-06-18-L5-105-hwledger-reclassify.md`
