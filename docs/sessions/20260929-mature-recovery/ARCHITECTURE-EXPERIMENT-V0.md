# Architecture experiment package v0 — two-engine typed profile

Date: 2026-09-30.

## Question
Does a typed cross-engine profile/evidence layer add real value over direct engine use?

## Existing adapters
Current source already contains BackendBase plus:
- MlxBackend
- MetalKernelBackend
- LlamaCppBackend
- VllmBackend
- SglangBackend
- TensorrtBackend

This makes the architecture experiment an **identity/qualification wrapper experiment**, not new engine integration.

## Critical current limitation
BackendCapabilities are static declarations (cuda/metal/batching/streaming/turboquant/spec-decode). GenerateResponse primarily returns text/tokens/elapsed/backend plus sparse metadata.

Static capability declaration does not prove:
- exact engine package/build/version;
- exact model/tokenizer/quantization;
- realized fallback;
- hardware/deployment;
- cache/speculation policy actually used;
- runtime generation;
- support lifecycle;
- observability.

## Minimal profile envelope

```
RequestedProfile {
 model_ref, tokenizer_ref, quantization_ref,
 engine_requested, hardware_constraints,
 cache/speculation/scheduler request
}
RealizedProfile {
 requested_profile_digest,
 engine_name, engine_version/build,
 model_artifact_digest,
 tokenizer_digest,
 hardware/deployment,
 realized cache/speculation/scheduler,
 runtime_generation,
 observability: field -> measured|declared|imported|unknown
}
QualificationReceipt {
 realized_profile_digest,
 workload_digest,
 quality_oracle_digest,
 resource evidence,
 latency/throughput evidence,
 support result,
 raw provenance
}
```

## Phase A — adapter identity experiment without heavy models
Use two installed/available adapters or mocked native clients only to prove:
- same abstract GenerateRequest does not erase backend-specific profile fields;
- unsupported/not-observable fields survive explicitly;
- fallback creates different RealizedProfile;
- response binds runtime generation/profile digest;
- static capability claim is not treated as qualification.

This phase is semantic and can run without performance claims.

## Phase B — real two-engine profile
Choose a model actually supported by two pinned engines on one hardware population. NVIDIA candidates: vLLM vs SGLang or llama.cpp; Apple: MLX-LM/oMLX vs llama.cpp Metal. Do not cross hardware populations for a single performance comparison.

Measure matched:
model/tokenizer/artifact; prompts; context distribution; concurrency; cache warmth; output-quality oracle; TTFT/ITL/throughput; peak/steady memory; cancel/unload/restart.

## Adversarial fixtures
M-E1 requested vLLM but realized fallback llama.cpp → cannot inherit vLLM support.
M-E2 unknown engine version → outcome may be observed, mechanism qualification blocked.
M-E3 partial stream → not complete.
M-E4 same endpoint after hot swap → old in-flight receipt remains generation A.
M-E5 support true but capacity exhausted → explicit admission reject.
M-E6 tenant/cache domain differs → no prefix-state identity reuse.
M-E7 custom extension faster but quality hard gate fails → not promoted.
M-E8 historical negative cache evidence stays bound to old profile.

## Success criterion
The typed envelope must expose meaningful cross-engine differences and prevent at least one ambiguity that direct current adapters permit. Otherwise profile-layer value is weak and scope should shrink.

## Immediate source finding
Current adapters make this likely: they normalize generation while omitting exact realized identity. The experiment targets a real semantic gap rather than invented abstraction.
