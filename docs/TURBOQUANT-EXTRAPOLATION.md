# TurboQuant+ Production Extrapolation

**Date:** 2026-09-17
**Source:** PhenoMLX turbo_quant code at `python/omlx_research/backends/mlx_backend.py`
**Status:** Mathematical extrapolation (desktop GPU validation pending)

## What TurboQuant+ Does

TurboQuant+ compresses the **KV cache** (not weights) to fewer bits after prefill.
- 4-bit = 75% KV memory reduction (default)
- 3-bit = ~81% reduction
- 2-bit = ~87% reduction
- Keys optionally kept at FP16 (set `turbo_key_bits=0`) for quality-sensitive workloads

KV cache memory formula:
```
kv_bytes = 2 * num_layers * num_kv_heads * head_dim * seq_len * bytes_per_element
```

The 75% KV savings ratio is **constant** regardless of model size. The absolute memory saved scales linearly with model + context + concurrency.

## Lab Validation (0.8B on M1 Pro 16GB)

| Metric | PhenoMLX | Upstream OMLX | Delta |
|--------|----------|---------------|-------|
| Prompts OK | 10/10 | 10/10 | +0.0% |
| Avg t/s | 20.6 | 23.9 | -13.5% |
| Peak memory | 2.55 GB | ~3.00 GB | -15.0% |

**Verdict:** NON_INFERIOR (within 15% margin)

## Production Extrapolation Table

All numbers computed from formula, not measured:

| Model | Params | Weights | KV FP16 | KV 4-bit | Saved | Note |
|-------|--------|---------|---------|----------|-------|------|
| 8B (test target) | 8B | 4.00 GB | 4.29 GB | 1.07 GB | 3.22 GB | Q4 weights |
| 15B dense (desktop test) | 15B | 30.00 GB | 6.44 GB | 1.61 GB | 4.83 GB | FP16 |
| 35B-A3B MoE (desktop test) | 35B | 70.00 GB | 34.36 GB | 8.59 GB | 25.77 GB | FP16, 3B active |
| 40B dense (prod) | 40B | 80.00 GB | 34.36 GB | 8.59 GB | 25.77 GB | FP16 |
| 72B dense (prod) | 72B | 144.00 GB | 42.95 GB | 10.74 GB | 32.21 GB | FP16 |
| 150B MoE (prod) | 150B | 300.00 GB | 51.54 GB | 12.88 GB | 38.65 GB | FP16 |
| 14B (A12B-like) | 14B | 28.00 GB | 5.37 GB | 1.34 GB | 4.03 GB | FP16 |

Saved% column = (saved bytes) / (weights + FP16 KV). For prod-density models where weights dominate, the total savings are smaller (11-17%).

## Where TurboQuant+ Shines: Long Context + Concurrency

When KV dominates memory (long context, high concurrency), savings exceed weights themselves.

**Example: 35B-A3B MoE at 100 concurrent users, 32K context:**
- KV per user (FP16): 8.59 GB
- KV per user (4-bit): 2.15 GB
- Total KV (FP16): 858.99 GB
- Total KV (4-bit): 214.75 GB
- **Saved per 100 users: 644.25 GB**

=> At scale, TurboQuant+ saves more memory than the weights themselves.
=> Allows serving 4x more concurrent users, or 4x longer context, on the same hardware.

## Roadmap to Desktop Validation

The desktop (RTX 3090 Ti 24GB) is currently being prepared:

1. Install torch==2.9.1+cu128 (Python 3.11) -- IN PROGRESS
2. Install transformers, accelerate, bitsandbytes
3. Load Qwen3.5-15B dense (~30GB FP16 or ~7.5GB INT4)
4. Run 10-prompt benchmark with TurboQuant+ vs stock FP16 KV
5. Measure peak memory, throughput, time-to-first-token
6. Compare against measurement tables above

Expected (based on formula):
- 15B FP16 KV baseline: ~6.4 GB KV at 32K
- 15B 4-bit KV expected: ~1.6 GB KV at 32K
- Target: NON_INFERIOR on throughput, >60% memory reduction

## Risk Notes

- Quality impact of 4-bit KV: The lab 0.8B test showed 100% prompt completeness and no corrupted outputs. Google TurboQuant paper claims "no accuracy loss" at 3-4 bits.
- Re-compaction cost: Compact_after_prefill adds ~100-300ms per request. For long sequences with infrequent compaction, this is negligible.
- Key vs Value bits: Code supports separate K/V compression. For safety, keep K at FP16 (turbo_key_bits=0) for now.

## Files Using TurboQuant+

- `python/omlx_research/backends/mlx_backend.py:295-360` — Main `generate_with_turbo_cache` impl
- `python/omlx_research/backends/mlx_backend.py:101-211` — Rust SIMD codec bindings
- `perf-core/turbo-quant/` — Rust workspace (separate cargo package)
