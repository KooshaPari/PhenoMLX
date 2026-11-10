# PhenoMLX — pass 7 remaining engine alternatives

Research date: 2026-09-30. Frozen source remains `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`.

This extends the MLX-LM/vLLM cache pass to SGLang, TensorRT-LLM and llama.cpp. It is still a mechanism/architecture pass, not a benchmark claiming one universal winner.

## SGLang

Current SGLang HiCache documentation describes:
- RadixAttention prefix-KV reuse in GPU memory;
- HiCache L1 GPU + L2 host + L3 distributed/external storage;
- local prefix matching via HiRadixTree;
- configurable prefetch policies and page layouts;
- async/write-through/write-back policies and CPU↔GPU transfer optimization;
- distributed backends such as Mooncake/3FS/NIXL/AIBrix-style integrations.

This is especially relevant to the user's repeated-prefix, long-agent-session and high-concurrency workload. It means hierarchical prefix-state reuse is not a PhenoMLX-unique subsystem.

**Disposition:** USE/QUALIFY SGLang for shared-prefix/agent-session profiles before custom hierarchy. Learn from its prefix-first identity model.

## TensorRT-LLM

Current NVIDIA docs expose a broad KV-cache system:
- paged/block cache and reuse across matching prefixes;
- multiple pools for differing attention windows/head structures;
- prioritized eviction;
- host-memory offload;
- FP8 and NVFP4 KV-cache modes;
- explicit KV-cache compression framework including cold-page quantization;
- connector API for external/disaggregated KV storage.

The docs also expose meaningful incompatibility/failure boundaries: some cache/connector combinations, quantization algorithms, SM versions, hybrid linear models, speculative decoding or eviction policies can disable or conflict with reuse/connector semantics.

That is directly useful to PhenoMLX's profile thesis: a profile must encode **supported combinations and failure boundaries**, not just individual feature flags.

**Disposition:** USE/QUALIFY for stable NVIDIA profiles where build/compile cost is justified; LEARN FROM its explicit cache compatibility matrix and connector contracts. Custom PhenoMLX cache needs a target deficiency, not feature parity.

## llama.cpp

Current llama.cpp surfaces separate K/V cache types, including lower-precision formats, and draft-model cache type controls for speculative decoding. Current source also exposes explicit K/V cache state/layout APIs and SWA-related behavior.

This makes llama.cpp a credible low-friction GGUF/heterogeneous-hardware baseline and another existing quantized-cache implementation. It should be part of comparison profiles rather than treated merely as compatibility fallback.

**Disposition:** USE/QUALIFY for GGUF and lower-overhead local profiles; use as a baseline for cache-type and speculative-draft experiments.

## Cross-engine conclusion

Across MLX-LM, vLLM, SGLang, TensorRT-LLM and llama.cpp, the following are already commodity/contested:
- KV quantization;
- prefix reuse;
- paged/block cache;
- hierarchical/offloaded cache;
- hybrid/state-aware cache management;
- speculative/draft cache variants;
- cache persistence/reuse;
- external/disaggregated cache connectors.

PhenoMLX should not own these merely to expose a common checkbox.

## Surviving PhenoMLX product role

The defensible role is increasingly:

1. **Capability/profile registry** — what combinations actually work for model × engine × hardware × quantization × state topology × cache/speculation policy.
2. **Qualification harness** — matched quality/performance/memory/lifecycle evidence.
3. **Adapter/control layer** — stable invocation and evidence identity without pretending engines are semantically identical.
4. **Experimental extension lane** — custom kernels/cache only where a measured engine deficiency exists.
5. **Decision surface** — select/profile a runtime for a workload rather than claim one global engine.

This is compatible with direct historical multi-engine intent and avoids replacing mature serving engines.

## Profile incompatibility is first-class

A mature RuntimeProfile must be able to express:
- unsupported feature combinations;
- engine/version/hardware restrictions;
- degraded/fallback behavior;
- cache identity/security domain;
- compilation/build prerequisites;
- cancellation/unload semantics;
- evidence freshness.

A “feature supported” boolean is inadequate.

## First native comparison profile

For the user's NVIDIA desktop lane, the first bounded comparison should use one model that is actually supported by all selected engines at pinned versions. Do not force Qwen3.5 if a candidate engine cannot run the exact architecture/quantization.

Candidate engines:
- vLLM;
- SGLang;
- llama.cpp;
- TensorRT-LLM only if a comparable build can be produced without changing the model semantics.

Measure stock idiomatic configurations first. Then compare any PhenoMLX extension.

For Apple, run a separate MLX-LM/oMLX profile rather than mixing hardware populations.

## Architecture gate consequence

A universal engine abstraction that hides cache/state semantics is rejected as the default hypothesis. A **typed profile/adapter system preserving backend-specific capability and evidence** is the preferred hypothesis for experiment.

Custom runtime/kernel subsystems remain experimental until they beat best engine-native mechanisms on declared hard-gated workloads.

## Gate movement

Major cache/runtime alternative families: **first mechanism pass complete**.
Exact versions/model support/license/health/integration costs: **still open**.
Generic custom-cache differentiation: **falsified**.
Typed profile/qualification differentiation: **survives strongly**.
Architecture freeze: **not yet final, but ontology work may proceed**.
