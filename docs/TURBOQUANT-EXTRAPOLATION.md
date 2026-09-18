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
- 3B A/B benchmark (`pilot/results/turboquant_3b_ab_20260917-2106.json`): FP16 KV 5.09 t/s vs 4-bit QDQ-hook sim 2.44 t/s, 0/10 corrupted in both. **Superseded for quality:** the corruption check does not detect the +54.5% perplexity regression the same QDQ path produces at 4-bit (see 'Codec fidelity' below).

**NOT DONE:**
- Real packed-KV residency measurement. The 3B A/B used Python QDQ hooks (quantize→dequantize on k_proj/v_proj outputs); resident KV stayed FP16, so the -52% throughput delta measures simulation overhead, not TurboQuant+ cost. A real packed-cache implementation (cache-layout surgery or Rust FFI) is required before any perf claim.
- Quality eval beyond corruption flags (MMLU/GPQA subsets still open).

**3B A/B interpretation (2026-09-17 run):** Quality signal: 4-bit quantization noise did not break generation (0/10 corrupted, hook verification 162,072 calls across 36 layers x 2 projections). Performance signal: not yet valid for claims; Python-hook QDQ is not the shipping path.

**7B A/B (2026-09-18 run, `pilot/results/turboquant_7b_ab_20260918-0720.json`):** A (FP16): 13.41 t/s decode, TTFT 102ms, 14.24 GiB, 0/10 corrupted. B (4-bit QDQ): 3.6 t/s, 5/10 heuristic-corrupted, and manual review shows 10/10 degraded (repetition loops, language mixing, token stutter).

The "quality-scaling cliff" reading of that run (tolerable at 3B, damaging at
7B) does not survive a real quality metric. Perplexity shows the same defect at
3B (+54.5%), just less visibly, and at 7B the repo codec is not degraded but
destroyed (PPL 17.7 -> 15,484). The cause is the codec's K grouping axis, and
per-channel K removes it at both scales; see 'Codec fidelity' below.

### Codec fidelity and a real quality metric (2026-09-18, desktop)

Two findings change how the entries above should be read. Both come from
`perf-core/turbo-quant-cuda/tq_codec_eval.py`; raw output is in
`pilot/results/codec_eval_3b_postrope_b4.json` (scheme matrix),
`pilot/results/codec_eval_3b_postrope_b8.json` (bit-width control),
`pilot/results/codec_eval_3b_postrope_b4_decomp.json` (K/V attribution) and
`pilot/results/codec_eval_3b_prerope_b4.json` (pre-RoPE K variant).
`tq_codec_eval_selftest.py` checks both codecs and the tensor layouts offline;
run it before trusting any new run.

**(a) This codec is not TurboQuant.** `perf-core/turbo-quant/src/encode.rs`
implements plain uniform asymmetric round-to-nearest group quantization
(`scale=(max-min)/qmax`, `zero=min`) with no rotation and no codebook.
Published TurboQuant (arXiv:2504.19874) is a randomized rotation, then a
per-coordinate optimal scalar quantizer, then a 1-bit QJL residual stage for
unbiased inner products. Until the schemes match, that paper's results
("quality neutrality at 3.5 bits per channel") do not transfer to this codec
and must not be quoted as supporting it.

**(b) The corruption heuristic is not a quality gate.** With perplexity on a
fixed 1023-token window (512-token chunks), the "clean at 3B" reading does not
survive. The harness is validated first: an 8-bit control returns to
+0.24% / +0.53% PPL, so the 4-bit deltas below are real for this path.

Qwen2.5-3B-Instruct, RTX 3090 Ti, FP16 PPL = 20.146 over 1023 tokens,
K quantized **post-RoPE** (the tensor that actually enters the cache):

| Scheme | Bits/coord (incl. metadata) | Rel. recon. error | PPL | delta PPL |
|---|---|---|---|---|
| FP16 KV (baseline) | 16.0 | 0 | 20.146 | - |
| uniform RTN g32 (repo codec) | 6.00 | 0.1069 | 31.131 | +54.5% |
| uniform RTN, **per-channel K** (KIVI-style) | 6.00 | 0.0337 | 20.271 | **+0.6%** |
| rotate + repo RTN | 6.00 | 0.0748 | 36.641 | +81.9% |
| rotate + Lloyd-Max 4-bit | 4.125 | 0.0939 | 126.994 | +530% |
| rotate + fixed uniform 4-bit | 4.125 | 0.1155 | 213.607 | +960% |
| 8-bit control (uniform / rotate+Lloyd) | 10.0 / 8.125 | 0.0059 / 0.0101 | 20.194 / 20.253 | +0.24% / +0.53% |

1. **Grouping axis dominates, and it is a K problem.** The 4-bit regression is
almost entirely K grouped per-token. Isolating each tensor at the same bits (K
quantized post-RoPE, V at `v_proj`; `codec_eval_3b_postrope_b4_decomp.json`):

   | Scheme | K axis | V axis | Rel. recon. error | PPL | delta PPL |
   |---|---|---|---|---|---|
   | FP16 KV (baseline) | - | - | 0 | 20.146 | - |
   | uniform RTN g32 (repo codec) | token | token | 0.1069 | 31.131 | +54.5% |
   | K only | token | FP16 | 0.1093 | 30.761 | +52.7% |
   | K only, **per-channel** | channel | FP16 | 0.0218 | 20.125 | **-0.1%** |
   | V only | FP16 | token | 0.0827 | 20.233 | +0.4% |
   | **per-channel K** + token V | channel | token | 0.0337 | 20.271 | **+0.6%** |

   K-per-token alone accounts for +52.7% of the +54.5% regression, while
   V-per-token at the same bits costs +0.4%. Per-channel min/max keeps 5x more of
   K's signal (0.0218 vs 0.1093) because one outlier channel no longer sets the
   scale for its neighbours. This matches KIVI (arXiv:2402.02750), which
   quantizes keys per-channel and values per-token for the same reason. It is the
   cheapest available fix and should be evaluated before any bit-width or
   rotation work.

   The fix does not depend on where K is quantized: with K quantized pre-RoPE
   (the old `tq_ab_bench_v2.py` placement, `codec_eval_3b_prerope_b4.json`) the
   same swap moves +45.1% to +0.23% (both tensors) and +46.5% to +1.0% (K
   alone). Relative reconstruction error is measured on the projection outputs,
   so it is identical in both `k_mode` settings by construction.
2. **Rotation improves distortion and worsens quality.** At equal bits, adding
   the rotation lowers relative reconstruction error (0.0748 vs 0.1069) while
   raising PPL (+81.9% vs +54.5%). Attention reads inner products, not
   distances, so lower MSE is not evidence of a better KV codec.
3. **The 3B "clean" result was a metric artifact**, not evidence of scale
   headroom. Future quality claims need PPL or better, not corruption flags.
4. **TurboQuant parity is untested, not refuted.** A rotation + scalar codec
   without the QJL stage does worse than the current codec here, which is
   consistent with the paper's own stated motivation for QJL. Implementing QJL
   and re-running is a prerequisite for a parity claim in either direction.
5. **Metadata is a real cost.** "4-bit" uniform RTN with fp32 scale + zero per
   group of 32 costs 6 bits/coordinate on the wire. TurboQuant's single fp16
   norm per 128-dim vector costs 0.125 bits/coordinate.

Caveats: this is still fake-quant (QDQ), not a resident packed cache, so it
measures quality only and no throughput claim is made. PPL windows are chunked
at 512 tokens, so no window sees longer context, and the corpus is a local
Phenotype document rather than WikiText/C4, so the absolute PPL is not
comparable to published numbers (only deltas within this harness are). The
numbers in this block are Qwen2.5-3B-Instruct; item (c) below repeats the
headline comparison at 7B. V stays per-token grouped throughout, so the
per-channel result covers K only. Per-channel grouping is degenerate for
single-token decode steps (a group of one reproduces the value exactly), and the
full-sequence PPL windows used here do not exercise that case.

**(c) At 7B the same defect is total, and the same fix removes it.** Same
harness, Qwen2.5-7B-Instruct (28 layers, 4 KV heads), FP16 PPL = 17.679 over the
same 1023-token window (`pilot/results/codec_eval_7b_postrope_b4.json`):

| Scheme | Bits/coord | Rel. recon. error | PPL | delta PPL |
|---|---|---|---|---|
| FP16 KV (baseline) | 16.0 | 0 | 17.679 | - |
| uniform RTN g32 (repo codec) | 6.00 | 0.1141 | 15484.4 | +87,487% |
| K only, token axis | 6.00 | 0.1147 | 15662.4 | +88,494% |
| K only, **per-channel** | 6.00 | 0.0076 | 17.768 | +0.5% |
| V only, token axis | 6.00 | 0.0806 | 17.610 | -0.4% |
| **per-channel K** + token V | 6.00 | 0.0132 | 17.687 | **+0.04%** |
| rotate + repo RTN | 6.00 | 0.0641 | 8090.3 | +45,663% |
| rotate + Lloyd-Max 4-bit | 4.125 | 0.0854 | 12412.8 | +70,113% |

At 4 bits the repo codec does not degrade 7B, it destroys it: PPL 17.7 ->
15,484, which is precisely the repetition and language mixing seen in manual
review of the 7B A/B. Changing only the K grouping axis gives 17.687 (+0.04%) at
the same bits and the same metadata cost, and V per-token costs nothing (-0.4%).
No rotation, no codebook and no QJL stage is needed to reach that. Per-channel K
is therefore the shipping configuration to implement first, at both 3B and 7B.

### Future Work
1. ~~Port turbo_quant codec to CUDA via libtorch~~ DONE 2026-09-17 (`perf-core/turbo-quant-cuda/turbo_quant_cuda.py`, 4/3/2-bit roundtrip tests pass)
2. Real packed-KV residency: replace Python QDQ hooks with a resident packed cache (cache-layout surgery or Rust FFI), then re-run the 3B/7B A/B
3. Re-run 7B/15B benchmarks on desktop with TurboQuant+ packed-KV active
4. Measure actual quality preservation with MMLU/GPQA subsets
5. Concurrency test: 4/8/16 parallel requests, measure VRAM scaling
6. PPL gate: adopt perplexity, with a high-bit control run, as the quality gate before any further quantization claim
7. Per-channel K grouping: implement in the codec and re-run the 3B/7B A/B; it is the only measured configuration that holds near baseline. **Confirmed at 3B (+0.6%) and 7B (+0.04%)** -- see 'Codec fidelity' items (b) and (c). This is a *call-site layout* change, not a codec rewrite: `encode_uniform` already groups along a flat slice, so per-channel K only requires each channel's values to be contiguous (transpose K to `[channels, tokens]`, encode, decode, transpose back). The work belongs in the cache path, which is the same surgery that item 2 needs, so doing item 2 first pays for both.
8. TurboQuant parity: implement the rotation + optimal scalar quantizer + 1-bit QJL pipeline from arXiv:2504.19874, then compare against the current codec before making any paper-vs-implementation claim

## Risk Notes

- Quality impact of 4-bit KV: the lab 0.8B test showed 100% prompt completeness and no corrupted outputs, and the corruption heuristic flagged 0/10 at 3B. Neither is quality evidence. On the same QDQ path a validated perplexity measurement shows +54.5% at nominal 4-bit on 3B and +87,487% on 7B (PPL 17.7 -> 15,484) as the codec stands today, because K is grouped per-token. With K grouped per-channel instead, the same 4 bits cost +0.6% (3B) and +0.04% (7B). The Google TurboQuant paper's "no accuracy loss" result is about *their* rotation + optimal-codebook + QJL scheme at 3.5 bits per channel, not about this uniform codec; do not cite it as support for this codec.
- Re-compaction cost: Compact_after_prefill adds ~100-300ms per request. For long sequences with infrequent compaction, this is negligible.
- Key vs Value bits: Code supports separate K/V compression. For safety, keep K at FP16 (turbo_key_bits=0) for now.

## Files Using TurboQuant+

- `python/omlx_research/backends/mlx_backend.py:295-360` — Main `generate_with_turbo_cache` impl
- `python/omlx_research/backends/mlx_backend.py:101-211` — Rust SIMD codec bindings
- `perf-core/turbo-quant/` — Rust workspace (separate cargo package)
