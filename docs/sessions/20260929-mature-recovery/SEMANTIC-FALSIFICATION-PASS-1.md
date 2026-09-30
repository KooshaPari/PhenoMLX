# PhenoMLX semantic falsification pass 1

Date: 2026-09-30. Target: RuntimeProfile ontology/obligations/journeys. Goal: find valid unexplained states.

## Valid counterexamples

### M-FALS-01 — distributed topology
A runtime profile can span tensor/pipeline/expert parallel GPUs or multiple hosts. HardwareProfile as a flat device list is insufficient to explain topology/interconnect/placement.

**VALID.** Add DeploymentTopology / PlacementPlan relation.

### M-FALS-02 — dynamic fallback changes the actual engine/profile
Requested accelerator/kernel fails and runtime silently falls back to another backend or precision.

**VALID.** Distinguish RequestedProfile from RealizedProfile; evidence binds realized execution. Silent fallback cannot inherit qualification.

### M-FALS-03 — remote/disaggregated cache authority
KV/state may live in host memory, another process/host or external cache service. CachePolicy alone does not identify remote store version/security/tenant namespace.

**VALID.** Add StateStore/CacheDomain identity and authorization/isolation relation.

### M-FALS-04 — model hot swap / rolling update
RuntimeInstance may change model/engine build while service endpoint remains stable.

**VALID.** Instance generation/profile revision must be explicit; in-flight request remains bound to the generation that served it.

### M-FALS-05 — partial stream then failure
Request emits valid tokens then engine crashes/cancels. Current request/stream lifecycle lacks partial-output acceptance semantics.

**VALID.** Stream terminal state and partial artifact identity required. Partial response cannot masquerade as completed inference.

### M-FALS-06 — admission/rejection under memory pressure
Profile is supported, but current capacity cannot admit another request without violating memory/SLO.

**VALID.** SupportEnvelope != instantaneous AdmissionDecision. Add capacity snapshot/admission semantics.

### M-FALS-07 — thermal/power state changes qualification
Laptop/desktop throttling or power mode materially changes performance without changing model/engine.

**VALID.** Environment/Hardware observation needs dynamic operating state for performance evidence.

### M-FALS-08 — engine security isolation / cross-tenant prefix cache
Same prefix hash across tenants can expose/reuse state unless isolation domain participates in cache identity.

**VALID.** CacheDomain/security principal becomes hard identity, not optional performance setting.

### M-FALS-09 — profile deprecation
A previously qualified profile becomes unsupported after engine/OS/model update.

**VALID.** SupportEnvelope needs lifecycle: experimental/supported/deprecated/withdrawn with effective evidence/version and migration/rollback.

### M-FALS-10 — remote API engine
PhenoMLX profile may target a remote provider where exact hardware/cache internals are unobservable.

**VALID.** Profile ontology needs evidence-observability level; unknown internals cannot be represented as measured facts.

## Covered/rejected attacks

- changed model/tokenizer/quantization: RuntimeProfile already covers.
- cache warmth mismatch: QualificationWorkload/evidence doctrine.
- unload/restart: existing journeys.
- custom extension loses baseline: M-OB-009.

## Contract deltas

Add DeploymentTopology, RequestedProfile vs RealizedProfile, StateStore/CacheDomain, RuntimeGeneration, stream terminal states, AdmissionDecision/CapacitySnapshot, dynamic OperatingState, support lifecycle and ObservabilityLevel.

The product becomes more clearly a typed profile/qualification system rather than a universal engine.
