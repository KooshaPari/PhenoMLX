# PhenoMLX mature product contract v0.1

Status: **CANDIDATE DRAFT — not accepted, not implementation authorization.**

## Product thesis
PhenoMLX is a typed cross-engine runtime-profile qualification/control layer with an experimental extension lane. It does not need to replace mature inference engines.

## Mature spine
ModelArtifact + QuantizationProfile + EngineArtifact + Hardware/DeploymentTopology + StateTopology + cache/speculation/scheduler policy → RequestedProfile → RealizedProfile/RuntimeGeneration → request/stream/resource state → QualificationAssessment → SupportEnvelope.

Supporting identities: CacheDomain/StateStore, CapacitySnapshot/AdmissionDecision, OperatingState, ObservabilityLevel, Extension.

## Ownership boundary
**Delegate to engines:** native scheduler, cache, prefix reuse, KV quantization, offload/tiering, speculation and engine-specific runtime mechanisms unless a measured deficiency justifies Extension.

**PhenoMLX candidate ownership:** typed profile/capability/support registry; truthful requested-vs-realized identity; adapters preserving backend-specific semantics; qualification workloads/evidence; support/deprecation lifecycle; profile selection/decision surfaces; experimentally justified extensions.

## Non-negotiable invariants
Evidence binds exact realized profile; unsupported/fallback/degraded states explicit; unknown remote internals remain unknown; partial stream != complete; support != current admission; cross-domain cache reuse requires valid isolation; historical negative evidence retained; custom extension must beat qualified engine-native baseline on declared accepted dimensions without hard-gate regression.

## CVP
One real local profile closes install→declare→launch→infer→inspect→cancel→unload→restart→qualify. MVP proves abstraction with a second materially different profile/engine and stock-vs-extension comparison.

## Existing reusable spine
Backend interface, cockpit/eval-report projections, perf-core experiments and candidate-provenance envelopes. Heuristic capacity UI remains advisory, not qualification.

## Blocking unknowns
exact current backend/request lifecycle mapping; accepted support/profile schema; native clean-install/profile experiment; exact versions/model compatibility across selected engines; license/health/integration-cost completion; web launcher defect; current evidence-store generalization.

Detailed obligations/journeys remain traceable in session files.
