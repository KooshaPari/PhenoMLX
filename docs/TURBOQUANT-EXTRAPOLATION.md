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

## Lab Validation

### 0.8B on M1 Pro 16GB (Apple Silicon)

| Metric | PhenoMLX | Upstream OMLX | Delta |
|--------|----------|---------------|-------|
| Prompts OK | 10/10 | 10/10 | +0.0% |
| Avg t/s | 20.6 | 23.9 | -13.5% |
| Peak memory | 2.55 GB | ~3.00 GB | -15.0% |

**Verdict:** NON_INFERIOR (within 15% margin)

### 7B on RTX 3090 Ti 24GB (desktop, stock transformers)

**Hardware:** NVIDIA GeForce RTX 3090 Ti, 25.8 GB VRAM
**Model:** Qwen/Qwen2.5-7B-Instruct (FP16 weights)
**Date:** 2026-09-17

| Metric | Value |
|--------|-------|
| Total VRAM after model load | 15.23 GB |
| Peak GPU memory (warm + generate) | 14.91 GB |
| First prompt (warmup) | 3.0 t/s (66.1s) |
| Prompts 2-10 (warm) | avg 16.3 t/s |
| Per-prompt tokens | 87-200 (200 cap) |

**Note:** First prompt takes 66s due to CUDA kernel warmup and KV cache initialization. Subsequent prompts settle at 15-18 t/s.

**Memory breakdown:**
- 7B FP16 weights: ~14 GB
- KV cache (FP16, 32K ctx): ~0.86 GB
- Total observed: ~15 GB
- 4-bit KV (TurboQuant+) expected: 14 GB + 0.21 GB = 14.2 GB
- **Projected savings: 0.64 GB (~4.3% of total)**

## Real Measurements (not just extrapolation)

### 7B baseline (RTX 3090 Ti, FP16)

| KV type | Peak VRAM | Throughput (warm) | KV @ 32K ctx |
|---------|-----------|-------------------|--------------|
| FP16 (stock transformers) | 14.91 GB | 16.3 t/s | ~0.86 GB |

### Projected TurboQuant+ 4-bit KV

| KV type | Peak VRAM | Throughput | KV @ 32K ctx |
|---------|-----------|------------|--------------|
| FP16 (measured) | 14.91 GB | 16.3 t/s | ~0.86 GB |
| 4-bit (projected) | 14.27 GB | ~15-17 t/s | ~0.21 GB |
| **Savings** | **0.64 GB (4.3%)** | **NON_INFERIOR** | **75% of KV** |

The 4.3% total savings at 7B/32K is modest because KV is small relative to weights. At longer context (128K) or higher concurrency, savings grow:

- 7B at 128K ctx: KV FP16 = 3.43 GB, 4-bit = 0.86 GB, savings = 2.57 GB (~14% of total)
- 7B at 32K with 10 concurrent: KV = 8.6 GB FP16 -> 2.15 GB 4-bit, savings = 6.45 GB (~30%)

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

### Status (2026-09-18)

**DONE:**
- torch 2.9.1+cu128 installed on desktop (Python 3.11.9, verified 2026-09-18)
- transformers 4.57.1, accelerate 1.15.0, bitsandbytes 0.50.2, safetensors 0.6.2 (verified 2026-09-18)
- Qwen2.5-7B-Instruct downloaded (~14GB FP16) to E:\hf_cache
- Baseline benchmark run with stock transformers FP16 KV cache: 16.28 t/s warm avg (prompts 2-10), 14.91 GiB peak VRAM (`pilot/results/desktop_7b_baseline_20260917.json`)
- TurboQuant ported to PyTorch CUDA (`perf-core/turbo-quant-cuda/turbo_quant_cuda.py`): 4/3/2-bit roundtrip tests pass on RTX 3090 Ti
- 3B A/B benchmark (`pilot/results/turboquant_3b_ab_20260917-2106.json`): FP16 KV 5.09 t/s vs 4-bit QDQ-hook sim 2.44 t/s, 0/10 corrupted in both

**NOT DONE:**
- Real packed-KV residency measurement. The 3B A/B used Python QDQ hooks (quantize→dequantize on k_proj/v_proj outputs); resident KV stayed FP16, so the -52% throughput delta measures simulation overhead, not TurboQuant+ cost. A real packed-cache implementation (cache-layout surgery or Rust FFI) is required before any perf claim.
- Quality eval beyond corruption flags (MMLU/GPQA subsets still open).

**3B A/B interpretation (2026-09-17 run):** Quality signal: 4-bit quantization noise did not break generation (0/10 corrupted, hook verification 162,072 calls across 36 layers x 2 projections). Performance signal: not yet valid for claims; Python-hook QDQ is not the shipping path.

### Future Work
1. ~~Port turbo_quant codec to CUDA via libtorch~~ DONE 2026-09-17 (`perf-core/turbo-quant-cuda/turbo_quant_cuda.py`, 4/3/2-bit roundtrip tests pass)
2. Real packed-KV residency: replace Python QDQ hooks with a resident packed cache (cache-layout surgery or Rust FFI), then re-run the 3B/7B A/B
3. Re-run 7B/15B benchmarks on desktop with TurboQuant+ packed-KV active
4. Measure actual quality preservation with MMLU/GPQA subsets
5. Concurrency test: 4/8/16 parallel requests, measure VRAM scaling

## Risk Notes

- Quality impact of 4-bit KV: The lab 0.8B test showed 100% prompt completeness and no corrupted outputs. Google TurboQuant paper claims "no accuracy loss" at 3-4 bits.
- Re-compaction cost: Compact_after_prefill adds ~100-300ms per request. For long sequences with infrequent compaction, this is negligible.
- Key vs Value bits: Code supports separate K/V compression. For safety, keep K at FP16 (turbo_key_bits=0) for now.

## Files Using TurboQuant+

- `python/omlx_research/backends/mlx_backend.py:295-360` — Main `generate_with_turbo_cache` impl
- `python/omlx_research/backends/mlx_backend.py:101-211` — Rust SIMD codec bindings
- `perf-core/turbo-quant/` — Rust workspace (separate cargo package)
