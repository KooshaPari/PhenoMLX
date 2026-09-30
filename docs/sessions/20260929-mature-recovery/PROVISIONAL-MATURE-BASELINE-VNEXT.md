# Provisional mature baseline vNext — restored experimental runtime thesis

Status: PROVISIONAL. Supersedes prior profile-layer-only product identity.

## Product thesis
PhenoMLX is an experimental LLM runtime/research studio for rapidly implementing, comparing and validating inference/runtime ideas across engines, platforms and hardware.

It originated from extending/rewriting MLX-oriented runtime capabilities and later expanded toward Windows/Linux plus CUDA/Vulkan/ROCm and other backends.

Typed RuntimeProfile/Qualification history is foundational research infrastructure, not sufficient differentiation by itself.

## Mature capability pillars

### M1 Experimentable runtime
Low-friction hooks to alter decoding, speculative methods, quantization, KV/cache/state, kernels/operators, scheduling/batching, memory/offload and model loading.

### M2 Cross-platform/backend substrate
MLX/Metal, CUDA, ROCm, Vulkan/llama.cpp and mature serving engines through explicit adapters where technically meaningful. Do not pretend all mechanisms map identically.

### M3 Research studio
Configure experiment matrices, launch workloads, inspect runtime state, compare variants, visualize metrics/evidence and retain negative results.

### M4 Runtime-profile truth
Requested vs realized configuration, generation/effective config, observability, qualification and support history.

### M5 Novel-extension gate
Custom implementation exists only when direct use/integration/fork/adaptation of mature engines/Unsloth/other libraries cannot satisfy the research need or when the experiment itself is testing a novel mechanism.

### M6 Reproducible qualification
Matched model/tokenizer/workload/hardware, quality hard gates, lifecycle evidence, uncertainty and exact build/runtime identity.

### M7 Packaging/portability
Install/update/backend availability and explicit degradation across supported OS/hardware stacks.

## Existence alternatives
A Unsloth Studio + mature engines.
B specialized engine-native research only.
C PhenoMLX studio orchestrating existing engines.
D PhenoMLX hybrid: orchestration + custom experimental runtime extensions.
E broad custom cross-platform runtime rewrite.

D is currently a strong candidate, but not selected.

## CVP
One research question (e.g. speculative/quantization/cache mechanism) can be configured, run and compared against best engine-native baseline with exact realized state, quality/perf evidence and reproducibility. A second backend/platform demonstrates that the research model is not MLX-only where the mechanism is portable.

## Supporting infrastructure
Profile store, QualificationReceipt, SupportEnvelope, evidence authority and prior VS-01 work become **INFRA-01**.

## Blockers
deep Unsloth architecture; adjacent studio/runtime comparison; novel mechanism experiment; cross-platform packaging experiment; two-engine qualification; dashboard/research workflow; source/current runtime mapping.
