# PhenoMLX mature product contract v0.2 candidate

Status: CANDIDATE DRAFT; not accepted baseline.

## Identity and purpose
PhenoMLX is the typed cross-engine runtime-profile qualification/control layer for the ecosystem, plus an experimental extension lane. It composes engine-native runtimes and hardware truth rather than duplicating them.

## Source-of-truth boundaries
- **hwledger** supplies/owns reusable hardware/capital/device capability observations where available.
- engine adapters supply engine-native capability/runtime facts.
- routing systems such as OmniRoute consume qualified runtime profiles for selection/routing; they do not become the canonical qualification source merely by routing.
- Portage may evaluate a runtime subject but does not own PhenoMLX support/profile truth.
- PhenoMLX composes these inputs into versioned RuntimeProfile/QualificationAssessment/SupportEnvelope truth for LLM runtime use.

This is a candidate boundary derived from current ecosystem roles and must remain compatible with future canonical registry decisions.

## Mature spine
ModelArtifact + QuantizationProfile + EngineArtifact + hardware/deployment/state/cache/speculation/scheduler inputs → RequestedProfile → RealizedProfile + RuntimeGeneration → request/stream/resource/admission state → QualificationAssessment → SupportEnvelope.

Supporting: StateStore/CacheDomain, CapacitySnapshot, OperatingState, ObservabilityLevel, Extension.

## Delegation
Engine-native scheduler/cache/prefix/KV quantization/offload/speculation stay engine-owned unless an Extension demonstrates a measured deficiency and accepted win.

Hardware discovery/capital inventory should reuse hwledger rather than duplicate it. IAM/secret/account authority is delegated but CacheDomain references the relevant security principal/domain.

## Capability truthfulness
An adapter may mark operations/capabilities NOT_SUPPORTED, NOT_APPLICABLE or NOT_OBSERVABLE. Lowest-common-denominator fabrication is forbidden.

RequestedProfile fallback produces an explicit RealizedProfile. Evidence binds realized generation/profile.

Observed outcome claims (latency/quality/API behavior) may be valid for opaque providers; mechanism claims (cache bytes/GPU/kernel) require corresponding observability.

## Lifecycle
Partial stream != complete. Support != instantaneous admission. Hot swap creates RuntimeGeneration and never relabels in-flight requests. SupportEnvelope is versioned with experimental/supported/deprecated/withdrawn states and migration/rollback relations.

## Stages
CVP: one real local profile closes install→declare→launch→infer→inspect→cancel→unload→restart→qualify.
MVP: second materially different engine/profile plus stock-vs-extension comparison and update/rollback.
Distributed/remote fields may be explicitly N/A at CVP, never silently omitted.

## Existence gate
A two-engine profile experiment must demonstrate that shared typed qualification/support semantics reduce ambiguity/integration burden versus direct engine use. If hwledger + routing + generic benchmark tooling already supply the contract without PhenoMLX, product scope must shrink or disappear.

## Remaining blockers
two-engine experiment; exact boundary with hwledger/routing validated in code/consumers; clean install/profile run; engine version/model/license/health matrix; profile/support schema; evidence-store generalization; launcher defect.

No implementation authorization follows.
