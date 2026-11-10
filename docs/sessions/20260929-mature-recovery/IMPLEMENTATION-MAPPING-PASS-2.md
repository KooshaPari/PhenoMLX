# Implementation mapping pass 2 — PhenoMLX

Date: 2026-09-30. Frozen source `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`.

## Search/mapping limitation

Connector search for generic backend/capacity/cancel/unload terms returned little despite README/branch evidence of such surfaces. Treat search absence as tooling limitation, not code absence.

## Verified current surfaces

- `omlx-research` launcher binds an external oMLX app, bundled Python paths and sourced repo env;
- launcher has deterministic web-port argument defect;
- source-recorded CUDA cache accounting for one Qwen3.5 profile, including negative 1.375× total;
- historical unique branch contains capacity/VRAM-fit/resolver concepts;
- custom extension/kernel families exist;
- multi-engine intent is documented.

## Delegation candidates

Engine-native mechanisms should remain delegated unless measured deficiency:
- MLX-LM/oMLX Apple serving/cache;
- vLLM/SGLang/TensorRT-LLM/llama.cpp native cache/scheduler/runtime mechanisms;
- engine-specific admission and cache implementations.

## Genuine semantic gaps until deeper direct mapping

- exact RequestedProfile/RealizedProfile identity;
- RuntimeGeneration on hot swap;
- DeploymentTopology/PlacementPlan;
- CacheDomain/StateStore authority;
- explicit ObservabilityLevel;
- typed SupportEnvelope lifecycle;
- engine-neutral but truthful stream terminal states;
- evidence-bound CapacitySnapshot/AdmissionDecision;
- dynamic OperatingState qualification.

These belong in the profile/control/evidence layer if PhenoMLX survives in that form, not necessarily inside engine internals.

## Architecture consequence

Prefer a thin typed adapter/profile contract preserving backend-specific fields over a normalized lowest-common-denominator runtime API. A field may be NOT_OBSERVABLE/NOT_APPLICABLE rather than fabricated.

Next direct mapping should inspect current backend plugin interfaces, cockpit resolver/capacity code, request lifecycle and evidence schemas file-by-file.
