# Cost model migration playbook (Phase 2 of ADR-035A)

**Status:** Phase 2 deferred — not started.
**Phase 1 done (2026-06-18, L5-105):** `pheno-capacity` v0.1.0 published.
**Phase 2 owner:** TBD (per ADR-035A §"Phase 2: Streamlit consumer migration").

## Goal

Migrate HwLedger's Streamlit `Planner` + `WhatIf` pages (historical:
`apps/streamlit/pages/{1_Planner,2_WhatIf}.py`, not in current working tree)
to consume `pheno-capacity` instead of duplicating the math in Python.

## Historical context (git @ 8bf878ca)

The original Python consumer was `apps/streamlit/lib/cost_model.py` (172 LOC),
with helpers in `perf_model.py`, `tokens.py`, `__init__.py`. It exposed:

```python
def vram_estimate_for_model(model_id: str, dtype: str = "fp16", ctx: int = 0) -> int
def fine_tune_overhead(params: int, dtype: str, optimizer: str = "adamw") -> int
def retraining_cost_usd(...) -> float  # spot-price aware
def tokens_to_data_gb(tokens: int, tok_per_gb: int = 350_000_000) -> float
def gb_to_usd(gb: float, provider: str, tier: str = "spot") -> float
```

The Rust crate covers everything except `retraining_cost_usd` (which is
fleet-economic, not pure math — kept in HwLedger).

## Migration options

### Option A — PyO3 / maturin bindings (preferred)

Build a Python wheel from `pheno-capacity` using `maturin develop`. Streamlit
imports:

```python
from pheno_capacity import (
    vram_estimate, model_fits_in, dtype_bytes,
    optimizer_state_vram, chinchilla_tokens,
)
```

- **Pros:** Single source of truth, Rust performance, full API parity.
- **Cons:** Adds a build step (`maturin develop`); Streamlit runtime must
  install the wheel; cross-platform wheels required (macOS arm64 + x86_64,
  Linux x86_64, Windows x86_64).
- **Effort:** 1–2 days (PyO3 bindings + CI matrix for 4 platforms).
- **Owner track:** New L5-XXX task.

### Option B — Interim Python shim (fallback)

Maintain a pure-Python `pheno_capacity` pip package that mirrors the Rust API
1:1. Test in Python; cargo test in Rust. CI runs both.

- **Pros:** No Rust↔Python bridge; Streamlit is happy; same test vectors.
- **Cons:** Two implementations; risk of drift; doubles test surface.
- **Effort:** 1 day (write shim + test vector parity suite).
- **Owner track:** Same L5-XXX.

### Option C — Rust consumer, PyO3 thin shim (most rigorous)

Re-implement Streamlit's Planner/WhatIf as a Rust binary in the HwLedger
workspace. Expose as a CLI; Streamlit pages call it via `subprocess.run` and
parse JSON output.

- **Pros:** Reuses cargo workspace; no Python math at all; same Cargo
  test/coverage story; CI reuses `pheno-ci-templates`.
- **Cons:** Refactor of Streamlit pages; subprocess overhead per page render.
- **Effort:** 3–4 days.
- **Owner track:** Larger ADR; needs user approval.

## Recommendation

**Option A (PyO3/maturin)** is the right answer for production. Open a new
L5-XXX task to:

1. Add a `pyo3-bindings` crate under `pheno-capacity/`.
2. Use `maturin build --release` in CI; publish to TestPyPI.
3. Update `pheno-ci-templates` with the PyO3 build matrix.
4. Re-import in Streamlit pages (when they return to the working tree).
5. Drop the historical `cost_model.py` re-implementation from `hwLedger`'s
   working tree (it's already not present; reference git @ 8bf878ca if needed).

## Test vector parity

Whatever option is chosen, the test suite must enforce Rust ↔ consumer parity
on these canonical models (from `pheno-capacity/src/math.rs` doc tests):

| Model | Params | dtype | Expected `vram_estimate` |
| :-- | --: | :-- | --: |
| LLaMA-7B | 6.74 B | F16 | 13_480_000_000 bytes (≈ 13.48 GB) |
| LLaMA-70B | 68.5 B | F16 | 137_000_000_000 bytes (≈ 137 GB) |
| Mistral 7B | 7.24 B | F16 | 14_480_000_000 bytes (≈ 14.48 GB) |
| Phi-2 | 2.78 B | F16 | 5_560_000_000 bytes (≈ 5.56 GB) |
| Mixtral 8×7B (active 2-of-8) | 12.9 B | F16 | 25_800_000_000 bytes (≈ 25.8 GB) |
| Llama-3-8B | 8.03 B | BF16 | 16_060_000_000 bytes (≈ 16.06 GB) |
| Llama-3-70B | 70.6 B | BF16 | 141_200_000_000 bytes (≈ 141.2 GB) |

## Tracking

- This doc: `docs/integrations/cost-model-migration.md`
- L5-XXX (Phase 2): to be opened after Option A/B/C decision
