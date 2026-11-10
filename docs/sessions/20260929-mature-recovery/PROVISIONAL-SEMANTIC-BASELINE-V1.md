# Provisional semantic baseline v1

Status: **PROVISIONAL CANDIDATE BASELINE.**

## Product identity
PhenoMLX is the typed cross-engine runtime-profile qualification/control layer plus experimentally justified extensions. It composes mature engines and hardware observations rather than replacing them.

## Source-of-truth boundary
Hardware/device observations may come from hwledger/direct evidence. Engine adapters own engine-native facts. PhenoMLX composes LLM RuntimeProfile/QualificationAssessment/SupportEnvelope truth. Routers consume it. Portage may evaluate it but does not own profile support truth.

## Canonical spine
ModelArtifact + QuantizationProfile + EngineArtifact + HardwareProfile/DeploymentTopology + StateTopology + cache/speculation/scheduler
→ RequestedProfile
→ RealizedProfile + RuntimeGeneration / effective-config observation epoch
→ InferenceRequest / Stream
→ ResourceState / CapacitySnapshot / AdmissionDecision / OperatingState
→ QualificationAssessment
→ versioned SupportEnvelope.

Supporting: StateStore, CacheDomain, ObservabilityLevel, Extension.

## Core invariants
- evidence binds exact realized profile/generation/effective config;
- fallback is explicit;
- static capability declaration is not runtime qualification;
- unknown/not-observable facts remain unknown;
- partial stream is not complete;
- support is not instantaneous admission;
- CapacitySnapshot is time/generation/environment scoped;
- support withdrawal never rewrites historical profile/evidence;
- cache-domain isolation participates in state identity;
- historical negative evidence is retained;
- custom Extension must beat best qualified engine-native baseline on declared dimensions without quality/hard-gate regression.

## Delegation
Engine-native scheduling/cache/prefix/KV quantization/offload/speculation remain engine-owned by default. Hardware inventory is not duplicated. IAM is delegated while CacheDomain references security authority.

## Stages
CVP: one local profile closes install→declare→launch→infer→inspect→cancel→unload→restart→qualify.
MVP: second materially different engine/profile, stock-vs-extension comparison, update/rollback.
Later: breadth/distributed/remote/ops.

## Provisional blockers
- real two-engine/common-model experiment;
- exact profile/support schema stabilization;
- clean install/lifecycle qualification;
- validate hwledger/routing consumer boundary;
- exact engine/model/version/license/health matrix;
- generalized evidence store;
- native lifecycle/admission/stream tests.

This baseline authorizes decomposition and bounded adapter/profile work, not claims of cross-engine value or completed architecture.
