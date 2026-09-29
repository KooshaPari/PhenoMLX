# PhenoMLX — pass 6 cache/profile SOTA attack

Research date: 2026-09-29. Frozen source remains `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`.

## The custom-cache burden is now much higher

Current runtime ecosystems already expose many mechanisms that older PhenoMLX experiments were trying to create or approximate.

### MLX-LM

Current MLX-LM source exposes prompt-cache persistence and explicit KV-cache quantization controls including `kv_bits`, group size and a configurable step at which quantized KV begins.

Consequence: on Apple/MLX profiles, “quantized prompt/KV cache exists” is commodity. A custom PhenoMLX cache must prove an end-to-end advantage over the current MLX-LM primitive under the same model/input/quality/lifecycle conditions.

### vLLM

Current vLLM documentation exposes:
- hybrid KV cache groups for models combining full attention with sliding-window/local/Mamba-like layers;
- group-aware cache capacity/concurrency;
- automatic prefix caching with explicit block identity;
- optional per-request cache salts for isolation;
- FP8 KV-cache quantization, including per-tensor and some per-head pathways;
- layer patterns that can skip KV quantization;
- CPU/tiered KV offloading;
- hybrid/Mamba cache modes and replay/checkpoint controls.

This materially overlaps PhenoMLX's historical cache/profile ambitions. It also validates one architectural concern from our own findings: **hybrid models cannot be measured as if one uniform “KV bytes/token” number describes every layer/state type.**

### Current hybrid-model limitation is still useful differentiation research

vLLM's hybrid manager documentation describes current constraints around combinations of cache groups/types and work-in-progress areas. These are legitimate places to investigate custom/adapted mechanisms—but only after proving the target PhenoMLX model/profile actually hits that limitation.

## Consequence for the Qwen3.5 CUDA block-cache experiment

The source-recorded PhenoMLX result retained the entire decoded prefix while adding packed state, producing 1.375× baseline KV footprint for that profile. Current engine primitives make a “store packed plus full decoded” architecture even harder to justify.

A future custom design needs to demonstrate at least one of:
1. decoded state is displaced/reconstructed rather than duplicated;
2. quantized-domain attention/decode avoids full persistent decode;
3. a hybrid-state representation handles a target model/profile better than current engine support;
4. a tiered/offload policy produces a real concurrency/latency win;
5. another accepted operational property unavailable from current primitives.

A theoretical codec ratio is not enough.

## Mature profile architecture sharpened

PhenoMLX should model **qualified runtime profiles**, not one universal cache implementation:

```
RuntimeProfile
  model + tokenizer + quantization
  engine + engine version
  hardware + driver/runtime
  attention/state topology
  cache policy
  prefix policy
  speculation policy
  concurrency envelope
  quality envelope
  evidence
```

Different engines may legitimately win different profiles. The product value can be the profile/qualification/extension layer rather than replacing every engine.

## Required cache accounting oracle

Every cache claim must measure separately:
- persistent model weights;
- full-attention KV;
- sliding/local attention KV;
- recurrent/Mamba/SSM state where applicable;
- packed/quantized payload;
- scales/metadata/residuals;
- decoded/dequantized persistent buffers;
- temporary/workspace allocations;
- allocator reserve;
- offloaded host/tier memory;
- peak and steady-state resident memory;
- cleanup after cancel/unload.

Also bind cache identity to model/tokenizer/quantization/prefix/security domain. Prefix reuse across an invalid identity boundary is a correctness/security failure even if it improves latency.

## Performance oracle

For each supported profile measure:
- cold/warm prefill;
- TTFT;
- inter-token latency distribution;
- decode throughput;
- batch/concurrency sweep;
- context-length sweep;
- prefix-hit/miss behavior;
- quality/non-inferiority;
- cancellation/unload/restart;
- memory at steady state and peak.

Do not compare a warm candidate to cold baseline or different weights/tokenizers.

## Bootstrap decisions, pass 1

- Apple prompt/KV quantization: **USE/ADAPT MLX-LM first**.
- Generic prefix cache: **USE engine primitive first**.
- NVIDIA FP8 KV: **USE/QUALIFY vLLM primitive before custom**.
- Hybrid model cache management: **USE/QUALIFY current engine support; CUSTOM only for demonstrated unsupported target topology or measured deficiency**.
- KV offload/tiering: **USE existing engine primitive before custom**.
- PhenoMLX custom cache/kernel: **EXPERIMENTAL until matched end-to-end evidence wins**.
- Cross-engine profile registry/qualification: **candidate PhenoMLX differentiation**.

## High-risk experiment now specified

M-X02/M-X03 should compare one current target profile across:
1. stock engine idiomatic cache policy;
2. best stock quantized/tiered policy;
3. current PhenoMLX extension where compatible.

Same model artifact, tokenizer, prompt corpus, request schedule, quality oracle and hardware profile. Record full allocation accounting and lifecycle cleanup.

If the custom extension does not win a declared accepted dimension without failing hard gates, classify it historical/experimental rather than preserving it from sunk cost.

## Research limits

This pass inspected current primary documentation/source surfaces for MLX-LM and vLLM. It does not yet qualify SGLang/TensorRT-LLM/llama.cpp equivalents, exact Qwen3.5 support, or engine-version compatibility with this repository. Those remain required before final profile architecture freeze.

## Gate movement

Generic cache uniqueness: **falsified**.
Custom cache necessity: **unproven / burden raised**.
Cross-engine profile/qualification thesis: **strengthened**.
PhenoMLX existence/architecture gate: **OPEN**.
