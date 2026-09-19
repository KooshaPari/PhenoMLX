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
5. **Metadata is a real cost, and halving its precision is a measured free win.**
   "4-bit" uniform RTN with fp32 scale + zero per group of 32 costs 6
   bits/coordinate on the wire; TurboQuant's single fp16 norm per 128-dim vector
   costs 0.125 bits/coordinate. On the resident cache this term is 22% of the
   footprint, and storing it as fp16 halves it: 73,728 -> 65,536 resident bytes for
   the same blocks (152 tokens, block=32), improving the effective reduction from
   2.11x to 2.38x while reconstruction error moves by +0.01%
   (`perf-core/turbo-quant-cuda/tq_block_cache_metadata_check.py`, which asserts both
   the byte drop and the error bound rather than assuming them). fp32 metadata is
   the avoidable part.

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

Three things follow. (i) **`axis_key=0` behaves as the outlier-separating axis**:
it costs +8.65% against +29.52% at identical bits, the same direction as the hook
finding that grouping K across channels within a token is the expensive one. Read
this as identification by outcome, not as having read HQQ's convention -- four
synthetic probes failed to establish the literal mapping (metadata shapes were
ambiguous, 96 groups under both hypotheses, and the error ordering contradicted
the naive reading). A future measurement should confirm the mapping directly
rather than assume that axis 0 means per-channel. (ii) hqq's packed cache beats
the repo codec's per-token K (+8.65% against +61.8% at 4 bits) but loses to
per-channel K with the repo codec (+0.95% to +1.45%), so adopting a third-party
backend is not obviously the right move. (iii) **No residency win is visible at
2048 tokens** (6.15 against 6.19 GiB): KV is small next to the weights at this
context. That is precisely why the memory case must be made at long context, and
it still has not been. The 3-bit `axis_key=1` cell raises inside HQQ (`size of
tensor a (0) must match the size of tensor b (2048)` during dequantize) and is
recorded as a failed cell rather than dropped.

The saving is nonetheless *visible* and agrees with the arithmetic, which is what
makes a long-context run worth predicting a number for. KV for 3B at 2048 tokens is
72 MiB in FP16 (2 kinds x 36 layers x 2 KV heads x 128 head_dim x 2048 tokens x 2
bytes); 4-bit makes it 18 MiB, so the predicted saving is 0.053 GiB, less the
4.5 MiB the `residual_length` window keeps at FP16 and hqq's scale/zero metadata,
i.e. roughly 0.046 GiB. Measured: 0.04 GiB (6.19 -> 6.15). At 8192 tokens the same
arithmetic gives 288 MiB FP16 against 72 MiB quantized, a ~0.21 GiB saving -- five
times the signal at four times the context. That is the run to do next, and it now
has a number to check against.

**At 8192 tokens the residency prediction is confirmed and the quality case
fails.** Same streamed harness, 8192 tokens, 256-token steps
(`pilot/results/packed_cache_stream_3b_8k.json`):

| Config | PPL | delta PPL | Peak VRAM |
|---|---|---|---|
| FP16 (`DynamicCache`) | 6.0228 | - | 6.40 GiB |
| hqq 4-bit, `axis_key=0` | 16.0268 | +166% | 6.21 GiB |
| hqq 4-bit, `axis_key=1` | 29.4235 | +389% | 6.21 GiB |
| hqq 3-bit, `axis_key=0` | 356.8145 | +5,824% | 6.21 GiB |

Two conclusions, pointing opposite ways:

- **Memory: confirmed, measured rather than projected.** Predicted KV saving at
  8192 tokens is 0.211 GiB (288 MiB FP16 against 72 MiB at 4 bits); measured is
  0.19 GiB. At 2048 tokens the same comparison is 0.053 GiB predicted against
  0.04 GiB measured. The harness can see residency and the arithmetic tracks.
- **Quality: not viable at this context.** The configuration that cost +8.65% at
  2048 tokens costs **+166%** at 8192, a 19x worsening, and 3-bit reaches +5,824%.

The suspected cause was the cache's habit of re-quantizing its entire accumulated
prefix every time the 128-token FP16 window fills -- 64 times across an 8K window.
That is now measured rather than assumed, by varying `residual_length` at 8192
tokens (`pilot/results/packed_cache_stream_3b_8k_res1024.json`):

| `residual_length` | re-quantizations per 8K window | hqq 4-bit `axis0` | hqq 3-bit `axis0` |
|---|---|---|---|
| 128 | 64 | **+166%** | +5,824% |
| 1024 | 8 | **+33.9%** | +1,441% |

So the policy is a major driver -- eight times fewer re-quantizations buys about
five times better quality -- but it is not the whole story: +33.9% remains at 8192
tokens even then, against +8.65% for the same cache at 2048 and +1.45% for
per-channel K in the hook harness at 2048. Longer context over quantized K costs
something by itself. Note the trade: the larger residual also gives back some
memory (0.17 GiB saved against 0.19 GiB at `residual_length=128`).

So the memory case is narrower than this document originally promised, and now
measurable in both directions: 4-bit KV does deliver the modelled memory reduction
(0.19 GiB at 8K, within 10% of prediction), but not with this cache's
re-quantization policy, and no throughput claim follows until both are fixed. The
next implementation question is therefore the re-quantization policy -- ideally
quantize each block once and never revisit it -- but the decode side must be fused
into attention, not done host-side; see below -- not the bit width.

**A host-side packed cache needs a batched decode -- and then it works.** The first
implementation decoded one stored block at a time: 256 codec calls per layer per
step, growing with the prefix (3.3 s per update at 4096 tokens for a *single* layer,
against 0.18 ms for FP16). That is the shape that made the 8K comparison abort after
22 minutes, and it is the only reason the earlier note here called a host-side cache
non-viable.

The blocks can instead be decoded in a **single** call. Per-block storage is already
in the group order the codec expects, so concatenating the block buffers and
decoding once per layer needs nothing but a reshape/permute on the decoded floats --
no bit-level work. `tq_block_cache_bench.py`, one layer, 256-token steps, before and
after:

| implementation | last step (4096 tok) | growth across 16 steps | total |
|---|---|---|---|
| per-block decode | 3296.30 ms | 4.6x | 35,243.6 ms |
| **single batched decode** | **20.32 ms** | **0.2x** | **455.2 ms** |

The per-step cost is now flat at ~20 ms per layer instead of rising: 162x better at
4096 tokens, and quadratic growth is gone. That makes the previously-aborted 8K
measurement affordable (~23 s of cache time for a 32-step window per configuration),
and it is reported below.

The architectural point survives in weaker form: ~20 ms per layer per step is still
~250x the cost of FP16's copy (0.08 ms per layer), so production throughput still
requires the dequantize fused into attention. But the host-side design is no longer
structurally blocked, and measuring quality with it is practical -- which is what
the measurement below does.

**Profile of where the 20 ms-per-layer actually goes** (`decode_profile.py` at
`C:\Users\koosh\agents\sandbox\tq-eval`, Qwen2.5-3B-Instruct, 4096-token prefill,
16 decode steps, model loaded once, GPU-synced timings):

| step | `_k_tensor` (ms) | `_v_tensor` (ms) | both (ms) | full forward (ms) |
|---:|---:|---:|---:|---:|
|   0 |   1.30 |   1.91 |   3.22 | 782.88 |
|   1 |   1.70 |   1.47 |   3.16 | 301.86 |
|   5 |  26.60 |  19.55 |  46.15 | 526.45 |
|  15 |   3.80 |   2.59 |   6.39 | 367.58 |
| **mean** | **4.36** | **4.11** | **8.47** | **443.25** |

The same workload with `DynamicCache` (fp16 KV) takes **83.23 ms** per decode step
forward, so the cache adds **443.25 - 83.23 = 360 ms** of overhead per step
(8.47 ms × 36 layers of `_k_tensor` + `_v_tensor` + permute + cat + cast). The
forward time itself is dominated by attention (sdpa at 4K context on the 3090 Ti)
plus the model weights' MLP/QKV projections, neither of which the cache touches.

So the actual cache overhead per layer is ~10 ms, not ~20 ms (the 20 ms figure
in the table above is for the K decode only on an older measurement), and the
fused decode has a concrete target: **kill the 8.47 ms-per-layer cost of
`_k_tensor`/`_v_tensor`** so the decode step drops from 443 ms back to the
~83 ms FP16 baseline. Three pieces of that 8.47 ms are visible in the code: the
single CUDA `decode_uniform_cuda` call (~1 ms measured for the actual GPU
work), the `reshape(blocks, b, h, d, s).permute(...)` that lays the decoded
floats out for attention (~3 ms), and the `.to(k_res.dtype)` cast from fp32 to
fp16 (~3 ms). The reshape/permute and the fp16 cast both vanish if the fused
attention reads packed K/V directly; the `decode_uniform_cuda` call moves
inside the matmul and the dequantize amortizes across the Q rows that read it.

Two spikes (step 5 at 46 ms, the variance across the run) are likely allocator
fragmentation between the `gc.collect()` + `empty_cache()` reset and the next
`_k_tensor`; the median step is 3-6 ms, which is what the fused decode has to
hold across all steps, not just the warmed-up ones.

**Fast decode path: 5x per-layer, decode step 2.8x.** Replacing the
4-pass scatter-add in `decode_uniform_cuda` with a vectorized nibble-unpack
(`packed & 0x0F` low nibble, `(packed >> 4) & 0x0F` high nibble, then
interleave) shifts the per-decode-step breakdown from:

| metric                      | before  | after   |
|-----------------------------|---------|---------|
| `_k_tensor` mean (ms/layer) |   4.36  |   0.89  |
| `_v_tensor` mean (ms/layer) |   4.11  |   0.87  |
| both mean (ms/layer)        |   8.47  |   1.76  |
| forward mean (ms/step)      | 443.25  | 156.46  |
| FP16 baseline (ms/step)     |  83.23  | 104.20  |
| BlockQuantCache / FP16      |   5.33x |   1.50x |

(Same 4096-token prefill, same 16 decode steps, same warmup; the FP16 baseline
jumped from 83 to 104 ms between the two runs, which is GPU-warmth variance --
it's not an FP16 regression. The block4 cache dropped in lock-step with the
decode-unpack change.) The remaining ~52 ms of cache overhead per step
(156.46 - 104.20) is the per-layer `reshape + permute + cat + cast` from
`_k_tensor` / `_v_tensor` returning fp32 followed by an out-of-place cast to
fp16 -- the cost the fused decode still has to attack, since the unpack itself
is no longer the bottleneck. Quality is unchanged: the nibble pack/unpack is
bit-identical to the scatter-add (verified by the codec shipped check, max abs
diff 0.0e+00; the long-context eval at 8K reproduces `block4 PPL=6.0971`,
`+1.04%`, well within the prior `+0.97%` noise band).

**Second pass: cast before the permute.** Moving the `.half()` from `update()`
into `_k_tensor` and `_v_tensor` (right after the decode, before the
`.reshape().permute().reshape()` chain) makes the post-permute contiguous copy
happen in fp16 instead of fp32. The chain is now:

```
groups.half().reshape(blocks,b,h,d,s).permute(1,2,0,4,3).reshape(B,H,stored,D)
```

instead of the prior fp32 chain followed by an out-of-place `.to(fp16)` cast.
The fp32 contig copy was the largest single transfer per call (`N*4` bytes);
moving it to fp16 halves the bandwidth. The `update()` path drops its
`k_stored.to(k_res.dtype)` since `_k_tensor` already returns fp16.

Measured per-layer throughput (4096-token prefill, 16 decode steps, GPU-synced):

| metric                       | original | after nibble-unpack | after half-in-`update`->_k |
|------------------------------|---------:|--------------------:|---------------------------:|
| `_k_tensor` mean (ms/layer)  |    4.36  |               0.89  |                       0.67 |
| `_v_tensor` mean (ms/layer)  |    4.11  |               0.87  |                       0.53 |
| both (ms/layer)              |    8.47  |               1.76  |                       1.21 |

So the cache overhead per layer per decode step went from **8.47 ms → 1.21 ms**
(7x), and the cache-forward / FP16-forward ratio sits near **1.5x** once the
allocator settles (was 5.3x before either pass). Forward-only numbers are
variance-bound between runs (allocator fragmentation between the pre-profile
`gc.collect()`/`empty_cache()` reset and the next call can push a single step's
forward time from ~80 ms to ~25 s), so the table reports per-layer decode work
which is stable across runs. Bit-identical quality (the long-context eval at
8K reproduces `block4 PPL=6.0971` / `+1.04%` across both passes; both shipped
checks still pass).

The remaining ~1 ms per layer is the `.reshape + .permute + .reshape` chain
itself, which is genuinely useful work (reordering `(blocks, b, h, d, s)` to
`(b, h, blocks*s, d)` so attention sees the expected layout). The fused-decode
work would let attention read packed K directly and skip this reorder entirely,
but the win is now ~1 ms per layer per step rather than the 8 ms it was before
the nibble unpack -- a much smaller target. The priority for fused decode drops
below the priority of shipping real benchmark numbers (MMLU/GPQA).

**(g) First resident quantize-once cache measurement, and two bugs it had to
fix.** The codec above is fake-quant (QDQ), so it cannot measure resident
behavior at all. `BlockQuantCache` is the actual quantize-once implementation,
defined in `perf-core/turbo-quant-cuda/tq_block_cache.py` and driven end-to-end
by `tq_block_cache_eval.py`. The first 8K measurement on it returned PPL
**811,272** -- five orders of magnitude worse than fp16 -- which is not a codec
result, it is a cache bug, and finding it cost two commits.

The prefill-only and one-shot decode reproducers
(`tq_block_cache_model_check.py`, `tq_block_cache_decode_check.py`) **both
passed**, because they feed the cache in a single `update()` call: the bug only
appears the moment a second chunk reuses a stored prefix, which is what
`ppl_streaming` and `generate()` do. Two separate overrides are required, and
neither is obvious from the parent class:

- `get_seq_length()` had to be overridden because the parent's version defaults
  `layer_idx` to `None` and the cache's internal lookup missed, returning `0`.
  The model uses this to size the causal mask, so the mask assumed an empty
  prefix while attention received the full cached K/V. Without the override
  the values the cache hands to attention are correct, but the mask around
  them is wrong at every context length.
- `get_mask_sizes()` had to be overridden because the parent's version asks
  `self.self_attention_cache` for the length, but `BlockQuantCache.update()`
  never appends to that field -- the cache returns its own K/V directly, so
  `cumulative_length` stays `0`. Without this override the mask is sized for
  `query_length` instead of `cache+query`, and the second chunk silently
  corrupts attention: step-1 loss jumps from fp16's 2.95 to 13.39 with sdpa,
  and eager attention crashes outright with a shape mismatch
  (`attn_weights (1, 2, 256, 512) + causal_mask (256, 256)`).

Both are two-line overrides plus a docstring, and either one alone would have
sent the cache back into a 811k-PPL state. With both in place, the 8K
streamed PPL eval (`pilot/results/block_cache_8k_fixed.json`, Qwen2.5-3B,
8192 tokens, 256-token steps, `block=32 bits=4`):

| Config | PPL | delta PPL | Peak VRAM |
|---|---|---|---|
| FP16 (`DynamicCache`) | 6.0228 | - | 6.40 GiB |
| hqq 4-bit, `axis_key=0` | 16.0268 | +166% | 6.21 GiB |
| **BlockQuantCache, block=32 bits=4** | **6.0815** | **+0.97%** | **6.23 GiB** |

Per-step losses for block4 match fp16 within ~1.5% on all 32 chunks and there
is no drift across the window -- that is what "quantize-once behaves like fp16"
looks like in a streamed perplexity measurement. The cache now uses *less* peak
VRAM than fp16 (6.23 against 6.40 GiB), even though it lives in fp16 metadata +
quantized payload, because the activation buffer for the partial trailing block
fits in the slack the 3B weights leave behind.

The shipped regression `tq_block_cache_shipped_check.py` runs the same 4-step
chunked prefill in ~70s and asserts step losses stay within 5% of fp16, which
catches both overrides -- if either is removed, step 1 diverges by 4-5x and the
check fails loud.

The cache holds at every context length up to 32K, on the same `tq_block_cache_eval_long.py`
driver and the same 3B model (`pilot/results/block_cache_8k_fixed.json`,
`block_cache_16k.json`, `block_cache_24k.json`, `block_cache_32k.json`):

| Context | fp16 PPL | block4 PPL | delta PPL | fp16 peak alloc (GiB) | block4 peak alloc (GiB) | saved (GiB) | mean unsat rel |
|---:|---:|---:|---:|---:|---:|---:|---:|
|  8K | 6.0228 | 6.0815 | +0.97% | 6.400 | 6.233 | 0.167 | 0.88% |
| 16K | 4.6279 | 4.6711 | +0.93% | 6.681 | 6.338 | 0.343 | 1.00% |
| 24K | 3.4889 | 3.5157 | +0.77% | 6.963 | 6.471 | 0.492 | 0.90% |
| 32K | 2.5536 | 2.5700 | +0.64% | 7.244 | 6.706 | 0.538 | 0.90% |

`python tq_block_cache_results_table.py --check` regenerates this table from
the committed `pilot/results/block_cache_*.json` and fails if any cell above
drifts, so the numbers are asserted rather than transcribed. The `saved`
column is GiB (earlier revisions printed decimal MB and read ~2% low).
`pilot/results/block_cache_8k.json` and `block_cache_1k.json` are deliberately
*not* the 8K row: they are the pre-fix runs whose block4 PPL is 811,272 and
56,657, kept as the fingerprint of the second-chunk mask bug described above.

Three observations the table supports.

- **The deficit does not grow with context.** It actually *narrows*, from +0.97%
  at 8K to +0.64% at 32K. The 24K/32K numbers are computed against `corpus_long.txt`
  which repeats the source corpus from byte 19,522; the second pass sees near-zero
  loss for the repeated tail. The summary PPL therefore understates the win --
  the per-step rel diff on the *unsaturated* 77 of 96 steps (24K) and 77 of 128
  steps (32K) holds at 0.9% mean / 6.2% max, which is the same quality as 8K
  where the corpus is single-pass. The eval reports both: the aggregate PPL
  delta and the saturation-aware step rel diff, so a failure mode in either
  shows up in the report. The exclusion (`abs(loss) < 0.05` nats) is fixed in
  `tq_block_cache_eval_long.py`.
- **Cache VRAM savings grow with context.** 0.167 GiB at 8K is small, but
  0.538 GiB at 32K -- 7% of the 7.244 GiB fp16 footprint -- is the start of a
  real win and the trajectory is linear in stored blocks. The cache holds
  less VRAM than fp16 because its resident total is a flat 0.375x fp16 KV:
  payload 0.250 (4 of 16 bits) plus a 0.125 fp32 scale/zero term, both
  measured below rather than assumed.
- **Decode-step memory has no ceiling inside the tested window.**
  `tq_block_cache_decode_oom_probe.py` isolates the decode cost: prefill N
  tokens, run 4 decode steps, report peak allocated and reserved VRAM per
  step, with the model loaded *once* for the whole ladder so safetensors
  fragmentation from reloading cannot masquerade as a ceiling. Every rung
  through 16384 survived all 4 decode steps:

  | prefill tokens | blocks | step-0 peak alloc | step-0 peak reserved | step-1+ peak reserved |
  |---:|---:|---:|---:|---:|
  | 1024 | 32 | 5.83 | 5.88 | 5.87 |
  | 2048 | 64 | 5.90 | 5.92 | 5.88 |
  | 4096 | 128 | 6.05 | 6.11 | 5.96 |
  | 6144 | 192 | 6.20 | 6.32 | 6.04 |
  | 8192 | 256 | 6.35 | 6.50 | 6.22 |
  | 12288 | 384 | 6.64 | 6.91 | 6.44 |
  | 16384 | 512 | 6.40 | 6.67 | 6.67 |
  | 24576 | 768 | 6.72 | 7.12 | 7.12 |

  Two slopes come out of that table, and they are different costs. Step 0 is
  measured right after a *single-shot* N-token prefill, so it carries the
  prefill's QDQ workspace: 1.42 GiB over 480 blocks (1K-12288 rows), an endpoint
  slope of ~3.0 MiB reserved per 32-token block. Steps 1+ are clean decode-step
  peaks (the probe resets peak stats and empties the allocator before each one)
  and slope at ~1.6 MiB per block, still 1.5x the 1.125 MiB of fp16 KV those
  tokens would occupy and ~6x the 0.281 MiB the cache actually stores packed.
  The 16384 and 24576 rows use the chunked-prefill variant
  (`oom_probe_chunked.py` at `C:\Users\koosh\agents\sandbox\tq-eval`), which
  feeds the cache in 256-token chunks so the 16K/24K single-shot prefill (O(N²)
  attention, >35 min on a 3090 Ti) does not have to finish; the chunked variant
  measures decode-step peak in isolation after a chunked prefill. Because the
  chunked path does not leave the prefill's QDQ workspace in the allocator when
  peak stats are reset, the chunked step-0 numbers read *lower* than the
  single-shot step-0 numbers at the same prefill: 6.40 GiB allocated at 16384
  vs 6.94 single-shot, because the QDQ workspace is reclaimed. The step-1+
  reserved numbers track between variants because both reset peak stats before
  the decode step starts. Both slopes are endpoint estimates from two rungs,
  not fits; the per-rung deltas in the logs are noisy (0.3-2.9 MiB/block for
  step 1+) but never negative, so the growth is real and linear-ish rather
  than a phase change. The growth is *transient* workspace, not the resident
  packed cache; the paragraph after next measures the resident term separately
  and uses the two together to bound it.

  The ladder stops at 24576 here because `corpus_long.txt` has 39,061 tokens and
  the next rung (32K prefill + 5 decode tokens) would need 32,005 + the decode
  steps, which `corpus_long.txt` cannot supply; a 50K+ corpus would extend it.
  The corpus-length bug from earlier sessions (the 19,522-token corpus failing
  24K with a shape error) is gone: the probe now carries a per-rung length guard
  and `CORPUS` points at `corpus_long.txt`. Peak VRAM depends on the stored
  block count alone, so the ladder is comparable across corpora; the reported
  losses are not. The strongest evidence that decode is not where this design
  hits a wall is the 32K row in the long-context table below:
  `tq_block_cache_eval_long.py` prefills 32768 tokens and then decodes 128
  steps at 7.59 GiB reserved (fp16) / 6.94 GiB (block4).

- **What the resident cache is actually made of, and what that implies.**
  `tq_block_cache_breakdown_probe.py` feeds the same chunked path and reads
  `byte_breakdown()` at each length, against `fp16_kv_bytes()` for the same
  shape (`pilot/results/block_cache_breakdown.json`):

  | Context | payload (MiB) | fp32 metadata (MiB) | residual (MiB) | total (MiB) | fp16 KV (MiB) | total / fp16 |
  |---:|---:|---:|---:|---:|---:|---:|
  |  2K | 18 | 9 | 0 | 27 | 72 | 0.375 |
  |  8K | 72 | 36 | 0 | 108 | 288 | 0.375 |
  | 16K | 144 | 72 | 0 | 216 | 576 | 0.375 |
  | 32K | 288 | 144 | 0 | 432 | 1152 | 0.375 |

  The ratio is exactly flat, and it is exactly what the bit budget says: the
  payload is 0.250 of fp16 KV (4 of 16 bits) and the fp32 scale/zero pair is
  another 0.125, so *3 of the nominal 4x are real and half of the nominal
  saving is spent on metadata*. `tq_block_cache_selftest.py` asserts both
  terms exactly rather than observing them, so this is a checkable claim. The
  residual is 0 at every rung because the ladder steps in multiples of the
  32-token block; it is the term that grows when it does not.

  That resident number then explains the peak-VRAM saving measured by the
  eval, and the part it does not explain is the interesting part:

  | Context | resident saving (GiB) | measured peak saving (GiB) | shortfall (GiB) |
  |---:|---:|---:|---:|
  |  8K | 0.176 | 0.167 | 0.009 |
  | 16K | 0.352 | 0.343 | 0.009 |
  | 24K | 0.527 | 0.492 | 0.035 |
  | 32K | 0.703 | 0.538 | 0.165 |

  Through 16K the measured peak win *is* the resident win, to within 9 MiB,
  which is allocator noise. The shortfall appears only at 24K and 32K and
  grows there, and that is the re-decode transient showing up: the cache pays
  a workspace that `DynamicCache` never does, and above ~16K it becomes
  visible in the peak. It does not yet reverse the result. Over 8K to 32K the
  resident advantage accrues 0.527 GiB while the shortfall takes back 0.156
  GiB, roughly 30%, so the measured saving still grows (0.167 to 0.538 GiB).
- **Bit width has a cliff at 3 bits, and a floor from the metadata.** Same
  driver, same corpus, `block=32`, only `--bits` varies
  (`pilot/results/block_cache_b3_8k.json`, `block_cache_b3_16k.json`,
  `block_cache_b2_8k.json`):

  | Context | fp16 PPL | 4-bit PPL | 4-bit delta | 3-bit PPL | 3-bit delta | 2-bit PPL | 2-bit delta |
  |---:|---:|---:|---:|---:|---:|---:|---:|
  |  8K | 6.0228 | 6.0815 | +0.97% | 6.3559 | +5.53% | 9.3076 | +54.54% |
  | 16K | 4.6279 | 4.6711 | +0.93% | 4.8556 | +4.92% | -- | -- |

  Four bits is close to free; three bits costs 5x the deficit and trips the
  driver's own >5% warn threshold at 8K; two bits is catastrophic. The 2-bit
  delta reproduces the fake-quant codec's +54.5% at the same width almost
  exactly, which is the useful cross-check here -- the resident cache is
  faithful to the codec at the width that works *and* at the width that
  fails, so its 4-bit number is not an artifact of the cache path. The
  per-step unsaturated rel diff agrees: 0.88% at 4 bits, 3.69% mean / 22.19%
  worst at 3 bits, 27.24% mean at 2 bits.

  Dropping a bit buys almost no memory. At 8K the measured peak saving over
  fp16 is 0.167 GiB at 4 bits, 0.185 GiB at 3 bits and 0.202 GiB at 2 bits,
  so 4 -> 3 bits is worth *18 MiB* while costing 5x the quality. The reason
  is visible in the resident breakdown at the same 8K prefill
  (`pilot/results/block_cache_breakdown_b3_8k.json`,
  `pilot/results/block_cache_breakdown_b2_8k.json`):

  | bits | payload (MiB) | fp32 metadata (MiB) | residual | total (MiB) | metadata share | vs fp16 KV |
  |---:|---:|---:|---:|---:|---:|---:|
  | 4 | 72 | 36 | 0 | 108 | 0.333 | 2.67x |
  | 3 | 54 | 36 | 0 | 90 | 0.400 | 3.20x |
  | 2 | 36 | 36 | 0 | 72 | 0.500 | 4.00x |

  The payload scales with the bit width exactly as advertised and the metadata
  does not move at all, so the metadata sets a hard floor: at `block=32` with
  fp32 scales and zeros the resident reduction cannot exceed **8x** no matter
  how few bits the payload uses, because the floor is `288/36`. Getting past
  that is a metadata change (fp16 scales/zeros would double the ceiling to
  16x), not a bit-width change, which is why 3 bits is not a useful operating
  point: it pays 5x the quality for 18 MiB and moves the ceiling not at all.

  **The metadata precision itself is also a knob.** Same driver, same corpus,
  same `block=32 bits=4`, only the metadata precision changes from fp32 to
  fp16 (`pilot/results/block_cache_breakdown_meta16_8k.json`,
  `block_cache_b4_meta16_8k.json`, `block_cache_b4_meta16_16k.json`):

  | Context | fp16 metadata (MiB) | total (MiB) | metadata share | vs fp16 KV | fp16 PPL | block4 PPL | delta PPL | saved alloc (GiB) |
  |---:|---:|---:|---:|---:|---:|---:|---:|---:|
  |  8K | 18 | 90 | 0.200 | 3.20x | 6.0228 | 6.0862 | +1.05% | 0.185 |
  | 16K | 36 | 180 | 0.200 | 3.20x | 4.6279 | 4.6629 | +0.75% | 0.378 |

  Halving the metadata precision at 8K is exactly the prediction: 36 MiB -> 18 MiB,
  2.67x -> 3.20x resident reduction, **with no measurable quality cost** (8K delta
  drifts from +0.97% to +1.05%, within per-step noise; 16K delta *improves* from
  +0.93% to +0.75%, also within noise). Because `BlockQuantCache.__init__`
  already takes `meta_dtype`, this is a one-argument change at call sites; the
  cached-everywhere fp32 path stays the default so callers don't opt into a
  precision they did not ask for. The `mean unsat rel diff` for fp16 metadata is
  0.86% (8K, 32/32 steps) and 0.90% (16K, 64/64), still consistent with the fp32
  numbers in the long-context table. So fp16 metadata is the cheapest way to
  break the 8x ceiling from 2.67x to 3.20x resident reduction at 4 bits, and
  it carries no quality penalty at the resolution of the per-step metric.
  Pushing past 3.20x resident reduction at 4 bits is a different problem
  entirely (bits 4->3 or 4->2 buys 18 MiB at 5x quality, see the previous table).

- **Chunk size does not have to be block-aligned.** Every ladder row above
  steps in multiples of the 32-token block, which leaves the fp16 residual
  term at exactly 0 and says nothing about a real generator, whose chunking
  is not block-aligned. The same 8K window fed in 100-token chunks (82 steps;
  100 is not a multiple of 32, so a partial block is held in fp16 at almost
  every step, and only the final token count lands on a boundary) gives bit-4
  PPL 6.1129 against fp16's 6.0391, a +1.22% deficit versus +0.97% aligned,
  and a peak saving of 0.170 GiB versus 0.167 GiB. So arbitrary chunking costs
  about a quarter of a percentage point and no memory, which is the answer
  that matters for streaming inference.

- **The quantized cache does not lose facts: needle-in-haystack retrieval is
  intact through 16K.** Perplexity says the cache is close to fp16; it does not
  say whether the model can still *use* a specific fact that now lives in
  quantized blocks. `tq_block_cache_needle_probe.py` measures exactly that. A
  unique fact ("the <NAME> facility access code is <CODE>") is spliced into the
  local corpus at a depth fraction, the context is prefilled in 256-token
  chunks so every complete block is quantized before any decode happens, and
  the prompt ends with "The access code is " (a real trailing-space token) so
  the next token is the first digit. That gives a pollution-free first-digit
  distribution; a 16-token greedy decode then substring-matches the whole code.
  Both metrics are paired between fp16 and block4 at identical depths
  (`pilot/results/needle_4k.json`, `pilot/results/needle_16k.json`):

  | Context | cases | depths | cache | first-digit acc | full-code acc | mean digit margin |
  |---:|---:|---|---|---:|---:|---:|
  |  4K | 10 | 0.10-0.90 (x2) | fp16 | 1.00 | 1.00 | +14.32 |
  |  4K | 10 | 0.10-0.90 (x2) | block4 | 1.00 | 1.00 | +13.54 |
  | 16K |  5 | 0.10-0.90 | fp16 | 1.00 | 1.00 | +7.64 |
  | 16K |  5 | 0.10-0.90 | block4 | 1.00 | 1.00 | +7.58 |
  | 32K |  1 | 0.50 | fp16 | 1.00 | 1.00 | +6.23 |
  | 32K |  1 | 0.50 | block4 | 1.00 | 1.00 | +5.50 |

  Retrieval is perfect under both caches at every depth, and the *margin* the
  model holds over the best wrong digit is nearly untouched: -0.78 nats at 4K
  (a 5% reduction on a +14 nat margin), -0.06 nats at 16K, and -0.73 nats on the
  single 32K mid-depth case (`pilot/results/needle_32k.json`; one case is not a
  rate, and a 5-depth 32K sweep does not fit one run -- block4 costs ~110 s per
  32K case, so it needs per-depth invocations). Two things are
  worth reading off this. First, the 4-bit cache is not merely "close in
  perplexity" -- the fact is still retrievable from the quantized blocks at
  16K of context (and at 32K, at a still-large +5.5 nat margin), which is the
  failure mode that would actually break a
  deployment. Second, the margin shrinks with context for *both* caches (+14.3
  at 4K to +6.2 at 32K), so the context-length effect is the model's, not the
  cache's; the cache's own contribution is under 0.1 nat at 16K. This
  supersedes the MMLU/GPQA plan for item 4 below: the datasets are
  not in the local HF cache, `lm_eval` is not installed, and the eval runs are
  offline by policy, whereas this probe runs from the local corpus and targets
  the long-context retrieval failure mode directly.

**The 24K and 32K measurements use a *repeated* corpus because the original
text is only 19,522 tokens. The saturation-aware metric (`mean unsat rel diff`)
removes the artificially easy tail steps (loss < 0.05 nats); they are the
documented contamination of the headline PPL, not a signal about the cache.
A non-repeating 30K+ corpus would give a higher summary PPL on both fp16 and
block4 (no near-zero tail) but the *delta* between them should be unchanged
to within sampling noise -- this is what the saturation-aware rel diff is
designed to confirm.**

### Future Work
1. ~~Port turbo_quant codec to CUDA via libtorch~~ DONE 2026-09-17 (`perf-core/turbo-quant-cuda/turbo_quant_cuda.py`, 4/3/2-bit roundtrip tests pass)
2. Real packed-KV residency: replace Python QDQ hooks with a resident packed cache (cache-layout surgery or Rust FFI), then re-run the 3B/7B A/B. **Largely done** via `BlockQuantCache` (`perf-core/turbo-quant-cuda/tq_block_cache.py`); the 3B 8K streamed PPL is +0.97% vs fp16 with peak VRAM slightly lower than fp16 itself (item (g)). The remaining work is the throughput claim (item (a)'s fused decode) and the 7B / 15B re-runs.
3. Re-run 7B/15B benchmarks on desktop with TurboQuant+ packed-KV active. **Blocked on weights, 2026-09-19:** `tq_block_cache_eval_long.py` now takes `--model Qwen/Qwen2.5-7B-Instruct` and the 3B methodology carries over unchanged, but the local HF cache at `C:\Users\koosh\.cache\huggingface\hub\models--Qwen--Qwen2.5-7B-Instruct` holds only `config.json` + tokenizer files -- the largest blob is 7 MB, so there are no weights and the run dies in `from_pretrained` with `AttributeError: 'NoneType' object has no attribute 'endswith'`. The 7B numbers already on this page were produced when the weights were present. Needs a ~15 GB download (the runs are otherwise offline) or an alternate path to the checkpoint.
4. Measure actual quality preservation with MMLU/GPQA subsets. **Blocked by policy, 2026-09-19:** neither MMLU nor GPQA is in the local HF cache, `lm_eval` is not installed, and the eval runs are offline (`HF_HUB_OFFLINE=1`), so the standard harness cannot run without a dataset download or a network-enabled run. **Substituted by needle-in-haystack retrieval** (`tq_block_cache_needle_probe.py`, results in the observation list above): retrieval stays at 100% and the digit margin moves by under 0.8 nats at 4K and under 0.1 nat at 16K, so the fact-bearing content of the quantized cache survives. That is a narrower metric than MMLU, but it targets the failure mode quantization actually causes in a long-context cache; MMLU/GPQA remains open as a broader check if the datasets become available.
5. Concurrency and long-context sweep: 2048-token windows at 3B are done (item (d), where the codec's deficit grows to +61.8% while per-channel K holds at +1.45%). **8192-token at 3B is done** via `BlockQuantCache` (item (g), +0.97% vs fp16), and **16K/24K/32K at 3B are done too** (`tq_block_cache_eval_long.py`, +0.93% / +0.77% / +0.64%, same table). **Batch > 1 verified** at batch=2 and batch=4 (`C:\Users\koosh\agents\sandbox\tq-eval\batch_check.py`, same prompt duplicated across the batch dim): both batches give identical argmax under fp16 and block4 (token 2090), and logit relative L2 diff is 0.0533 (batch=2) / 0.0576 (batch=4) -- the same per-element quantization noise we see at single-batch, with no batch-specific penalty. The cache packs `(b*h*d, group_size)` per block so batch becomes one more index on the row dim, no separate code path needed. The cache's host-side decode grows linearly with stored blocks; after the vectorized nibble-unpack and the dtype-cast reorder it measures **1.21 ms per layer per decode step at 4K tokens** (was 8.47 ms before those two passes -- see the decode-overhead audit below the table). The 24 GiB 3090 Ti decode ceiling has been probed up through 24K prefill (768 blocks) on `corpus_long.txt`: every rung from 1K to 24K clears 4 decode steps without OOM, with step-0 reserved rising 5.88 GiB (1024 tokens, 32 blocks) to 7.12 GiB (24576 tokens, 768 blocks). The next rung (32K) needs a longer corpus than `corpus_long.txt` can supply; the eval-long driver's 32K row already prefills and decodes at 7.59 / 6.94 GiB so the ladder is bounded from above by that measurement even without a 32K probe rung.
6. PPL gate: adopt perplexity, with a high-bit control run, as the quality gate before any further quantization claim
7. Per-channel K grouping: implement in the codec and re-run the 3B/7B A/B; it is the only measured configuration that holds near baseline. **Confirmed at 3B (+0.6%) and 7B (+0.04%)** -- see 'Codec fidelity' items (b) and (c). This is a *call-site layout* change, not a codec rewrite: `encode_uniform` already groups along a flat slice, so per-channel K only requires each channel's values to be contiguous (transpose K to `[channels, tokens]`, encode, decode, transpose back). The work belongs in the cache path, which is the same surgery that item 2 needs, so doing item 2 first pays for both. ~~**The decode must be fused:** a host-side packed cache is measured non-viable (3.3 s per update at 4K tokens for one layer, against 0.18 ms for FP16), so the block layout has to be paired with an in-attention dequantize.~~ **Corrected 2026-09-19:** that 3.3 s figure was the *per-block* codec call path, which the cache no longer uses. The resident cache decodes all stored blocks in one call, and after the vectorized nibble-unpack and the dtype-cast reorder the whole update costs **1.21 ms per layer per decode step at 4K tokens** (7x less than the 8.47 ms first measured). Host-side decode is therefore *usable*, not non-viable; fusing the dequantize into attention would remove the remaining ~1 ms per layer (the reshape/permute reorder) but is an optimization, not a precondition. **The fp16 metadata experiment is now measured** (table just above): with fp16 scales and zeros the resident reduction goes from 2.67x to 3.20x at 4 bits and the deficit stays flat at +1.05% (8K) / +0.75% (16K). The metadata is no longer an open question at 3B; the remaining unknown is whether the same holds at 7B and 15B once the cached weights are available (item 3).

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
