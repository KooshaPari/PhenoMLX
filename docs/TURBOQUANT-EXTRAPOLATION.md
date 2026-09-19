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
measures quality only and no throughput claim is made. PPL windows are chunked,
so no window sees context beyond the chunk: blocks (b) and (c) use 512-token
windows over a local Phenotype document (5.7 KB) and block (d) uses 2048-token
windows over the repo's own docs (77 KB, `tq_codec_eval_corpus.py`). Neither is
WikiText/C4, so absolute PPL is not comparable to published numbers, nor is it
comparable across blocks -- only deltas within a single run are. The numbers in
blocks (b) and (c) are Qwen2.5-3B-Instruct; block (c) is 7B. V stays per-token
grouped throughout, so the per-channel result covers K only. Per-channel grouping
is degenerate for single-token decode steps (a group of one reproduces the value
exactly); the residual window that fixes this is specified in Future Work item 7,
and the full-sequence PPL windows used here do not exercise that case.

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

**(d) The fix holds, and the codec's deficit grows, at longer context.** Same
harness, repo-docs corpus (77 KB from 14 root `.md` files, reproducible with
`perf-core/turbo-quant-cuda/tq_codec_eval_corpus.py`), Qwen2.5-3B-Instruct,
2048-token windows, FP16 PPL = 5.820
(`pilot/results/codec_eval_3b_long_b4.json`; the 8-bit control in
`codec_eval_3b_long_b8.json` returns every scheme to within 0.08% of baseline):

| Scheme | Bits/coord | Rel. recon. error | PPL | delta PPL |
|---|---|---|---|---|
| FP16 KV (baseline) | 16.0 | 0 | 5.820 | - |
| uniform RTN g32 (repo codec) | 6.00 | 0.1074 | 9.418 | +61.8% |
| K only, token axis | 6.00 | 0.1099 | 9.337 | +60.4% |
| K only, **per-channel** | 6.00 | 0.0218 | 5.875 | +0.95% |
| V only, token axis | 6.00 | 0.0824 | 5.843 | +0.40% |
| **per-channel K** + token V | 6.00 | 0.0336 | 5.904 | **+1.45%** |
| rotate + repo RTN | 6.00 | 0.0752 | 10.200 | +75.3% |
| rotate + Lloyd-Max 4-bit | 4.125 | 0.0938 | 63.600 | +993% |
| rotate + fixed uniform 4-bit | 4.125 | 0.1152 | 67.554 | +1061% |

The codec's deficit widens with context (+54.5% at 512 tokens to +61.8% at 2048)
while per-channel K stays near-free (+0.6% to +1.45%). That is the regime the
compression exists to serve, so the fix is worth more here, not less. PPL levels
are not comparable across window sizes (5.820 at 2048 tokens versus 20.146 at 512
is a different question, not an improvement); only deltas within a run are.

**(e) What the next bit down costs, and reproducibility.** At 3 bits
(`pilot/results/codec_eval_3b_long_b3.json`, same 2048-token setup, FP16 PPL
5.820) per-channel K is no longer free:

| Scheme | Bits/coord | PPL | delta PPL |
|---|---|---|---|
| FP16 KV (baseline) | 16.0 | 5.820 | - |
| uniform RTN g32 (repo codec) | 5.00 | 323.741 | +5,463% |
| K only, token axis | 5.00 | 253.883 | +4,263% |
| K only, **per-channel** | 5.00 | 6.072 | **+4.33%** |
| V only, token axis | 5.00 | 5.952 | +2.27% |
| **per-channel K** + token V | 5.00 | 6.236 | +7.16% |
| rotate + repo RTN | 5.00 | 417.073 | +7,067% |
| rotate + Lloyd-Max 3-bit | 3.125 | 497.204 | +8,444% |
| rotate + fixed uniform 3-bit | 3.125 | 2777.281 | +47,623% |

So 3 bits buys an 81% KV reduction for roughly +4% PPL at 3B, against 4 bits
being free. The practical floor for K is therefore 3 bits, and it needs its own
per-scale quality gate before any config default changes.

Reproducibility: an independent rerun of the 7B block in a separate process
(`pilot/results/codec_eval_7b_postrope_b4_rerun.json`) reproduces every PPL and
distortion value exactly, FP16 baseline included (17.678863413317), so the 7B
result is not a one-off.

**2 bits is where it breaks.** Same setup (`pilot/results/codec_eval_3b_long_b2.json`;
this point needed a detached scheduled task, because backgrounded children of the
agent process are killed when the session reloads):

| Scheme | Bits/coord | PPL | delta PPL |
|---|---|---|---|
| FP16 KV (baseline) | 16.0 | 5.820 | - |
| uniform RTN g32 (repo codec) | 4.00 | 2641.863 | +45,296% |
| K only, token axis | 4.00 | 1001.585 | +17,111% |
| K only, **per-channel** | 4.00 | 7.617 | **+30.88%** |
| V only, token axis | 4.00 | 6.439 | +10.65% |
| **per-channel K** + token V | 4.00 | 9.063 | **+55.73%** |
| rotate + repo RTN | 4.00 | 12486.466 | +214,459% |
| rotate + Lloyd-Max 2-bit | 2.125 | 6648.270 | +114,139% |
| rotate + fixed uniform 2-bit | 2.125 | 63517.406 | +1,091,338% |

Per-channel K still beats the shipped grouping by more than three orders of
magnitude at 2 bits (+30.9% against +17,111%), so the axis finding holds at every
width tested. But 2 bits is no longer a trade worth making. The ladder for K,
all at 3B and 2048-token windows: **4 bits +0.95%, 3 bits +4.33%, 2 bits
+30.88%** (K only; +1.45% / +7.16% / +55.73% with V also quantized). So 4 bits is
the safe default, 3 bits is an 81% KV reduction for about +4% PPL, and 2 bits is
not usable. The 87% reduction figure near the top of this document is arithmetic;
its quality cost is the +30.9% measured here.

**(f) First resident packed-cache measurement, and why prefill-only is blind.**
transformers 4.57 ships a KIVI-shaped `QuantizedCache` -- an FP16
`residual_length` window plus a quantized prefix, with `axis_key`/`axis_value`
knobs. With the `hqq` backend installed this is the first measurement here of a
genuinely resident packed cache rather than QDQ hooks
(`perf-core/turbo-quant-cuda/tq_packed_cache_eval.py`, raw output
`pilot/results/packed_cache_stream_3b.json`).

**The trap first.** A prefill-only evaluation cannot observe a quantized cache at
all. `QuantizedLayer.update` quantizes into storage on its first call but returns
the *raw* key/value states, so attention never reads quantized data. A first
attempt that fed each window as a single prefill returned byte-identical PPL for
FP16, 4-bit and 3-bit -- 5.8196 for all five configs. The cache has to be driven
as a stream (persistent cache, fed in steps) for the second and later updates to
take the dequantize path. Any packed-cache quality number produced prefill-only is
meaningless.

Streamed results: 3B, 2048 tokens, 256-token steps, `q_group_size` 32,
`residual_length` 128.

| Config | PPL | delta PPL | Peak VRAM |
|---|---|---|---|
| FP16 (`DynamicCache`) | 6.0580 | - | 6.19 GiB |
| hqq 4-bit, `axis_key=0` | 6.5819 | +8.65% | 6.15 GiB |
| hqq 4-bit, `axis_key=1` | 7.8463 | +29.52% | 6.15 GiB |
| hqq 3-bit, `axis_key=0` | 10.5117 | +73.52% | 6.15 GiB |
| hqq 3-bit, `axis_key=1` | failed in HQQ | - | - |

Three things follow. (i) **`axis_key=0` is the axis that keeps K's outlier
channels apart** (+8.65% against +29.52% at identical bits), which is the fix the
hook measurements identified and which four synthetic probes failed to pin down;
this settles it by outcome on the real model. (ii) hqq's packed cache beats the
repo codec's per-token K (+8.65% against +61.8% at 4 bits) but loses to per-channel
K with the repo codec (+0.95% to +1.45%), so adopting a third-party backend is not
obviously the right move. (iii) **No residency win is visible at 2048 tokens**
(6.15 against 6.19 GiB): KV is small next to the weights at this context. That is
precisely why the memory case must be made at long context, and it still has not
been. The 3-bit `axis_key=1` cell raises inside HQQ (`size of tensor a (0) must
match the size of tensor b (2048)` during dequantize) and is recorded as a failed
cell rather than dropped.

### Future Work
1. ~~Port turbo_quant codec to CUDA via libtorch~~ DONE 2026-09-17 (`perf-core/turbo-quant-cuda/turbo_quant_cuda.py`, 4/3/2-bit roundtrip tests pass)
2. Real packed-KV residency: replace Python QDQ hooks with a resident packed cache (cache-layout surgery or Rust FFI), then re-run the 3B/7B A/B
3. Re-run 7B/15B benchmarks on desktop with TurboQuant+ packed-KV active
4. Measure actual quality preservation with MMLU/GPQA subsets
5. Concurrency and long-context sweep: 2048-token windows at 3B are done (item (d), where the codec's deficit grows to +61.8% while per-channel K holds at +1.45%). 8K/16K/32K and batch > 1 are still open
6. PPL gate: adopt perplexity, with a high-bit control run, as the quality gate before any further quantization claim
7. Per-channel K grouping: implement in the codec and re-run the 3B/7B A/B; it is the only measured configuration that holds near baseline. **Confirmed at 3B (+0.6%) and 7B (+0.04%)** -- see 'Codec fidelity' items (b) and (c). This is a *call-site layout* change, not a codec rewrite: `encode_uniform` already groups along a flat slice, so per-channel K only requires each channel's values to be contiguous (transpose K to `[channels, tokens]`, encode, decode, transpose back). The work belongs in the cache path, which is the same surgery that item 2 needs, so doing item 2 first pays for both.

   Verified rather than asserted: `tq_codec_eval_shipped_check.py` round-trips real
   3B K/V through the **shipped** CUDA port (`turbo_quant_cuda.py`) in both
   layouts and reproduces the recorded figures exactly -- token 0.1068869433,
   per-channel 0.0336714164 against the committed 0.1068869428 / 0.0336714164 --
   while confirming the captured activations are bit-identical to the harness's
   own (`max diff 0.0`). No codec change is needed to obtain the channel-axis
   result; only the caller's memory layout.

   It is also **streaming-compatible**, which is the question that decides whether this can ship. The measured configuration groups channel-wise in fixed blocks of `group_size` tokens, and a test asserts that grouping never mixes across those blocks (`channel grouping is block-local (G=32, streaming-safe)` in `tq_codec_eval_selftest.py`). A streaming cache can therefore quantize each block of 32 tokens per channel as it fills and keep the unfilled tail in FP16 -- KIVI's residency structure. Residual cost is at most `group_size - 1` tokens of FP16 K per layer and head, i.e. a fraction `group_size * (16/bits) / seq_len` of the compressed cache: about 128/seq_len at 4 bits, so ~3% of the cache at 4K context and ~0.4% at 32K. It is negligible exactly where the technique matters. Naive single-token decode with no residual window would be degenerate (a group of one reproduces its value exactly), which is why the residual window is not optional.
8. TurboQuant parity: implement the rotation + optimal scalar quantizer + 1-bit QJL pipeline from arXiv:2504.19874, then compare against the current codec before making any paper-vs-implementation claim

## Risk Notes

- Quality impact of 4-bit KV: the lab 0.8B test showed 100% prompt completeness and no corrupted outputs, and the corruption heuristic flagged 0/10 at 3B. Neither is quality evidence. On the same QDQ path a validated perplexity measurement shows +54.5% at nominal 4-bit on 3B and +87,487% on 7B (PPL 17.7 -> 15,484) as the codec stands today, because K is grouped per-token. With K grouped per-channel instead, the same 4 bits cost +0.6% (3B) and +0.04% (7B). The Google TurboQuant paper's "no accuracy loss" result is about *their* rotation + optimal-codebook + QJL scheme at 3.5 bits per channel, not about this uniform codec; do not cite it as support for this codec.
- Re-compaction cost: Compact_after_prefill adds ~100-300ms per request. For long sequences with infrequent compaction, this is negligible.
- Key vs Value bits: Code supports separate K/V compression. For safety, keep K at FP16 (turbo_key_bits=0) for now.

## Files Using TurboQuant+

- `python/omlx_research/backends/mlx_backend.py:295-360` — Main `generate_with_turbo_cache` impl
- `python/omlx_research/backends/mlx_backend.py:101-211` — Rust SIMD codec bindings
- `perf-core/turbo-quant/` — Rust workspace (separate cargo package)
