# PhenoMLX obligations v1 delta — post-falsification

Status: DRAFT additions; no target count.

| ID | Obligation | Positive acceptance | Negative/counterexample | Journeys |
|---|---|---|---|---|
| M-OB-012 | RequestedProfile and RealizedProfile are distinct when fallback/substitution occurs; evidence binds realized execution. | Requested accelerator falls back explicitly and result names fallback profile. | Silent CPU/other engine fallback inherits accelerator qualification. | J02/J03/J05/J09 |
| M-OB-013 | DeploymentTopology/PlacementPlan identify multi-device/host placement and interconnect relevant to behavior/performance. | TP/PP/EP layout and devices are evidence-bound. | “4 GPUs” comparison ignores topology/interconnect difference. | J02/J03/J09 |
| M-OB-014 | StateStore and CacheDomain identify remote/tiered/disaggregated state plus tenant/security isolation. | Prefix reuse only within authorized cache domain; remote store/version known. | Cross-tenant prefix state reused because hash matches. | J03/J05/J06/J08 |
| M-OB-015 | RuntimeGeneration changes on hot swap/rolling profile update; in-flight requests remain bound to serving generation. | Request started on gen A reports A after endpoint moves to B. | Evidence labels all requests with latest endpoint profile. | J03/J04/J08/J11 |
| M-OB-016 | Stream terminal state distinguishes complete, partial-failed and cancelled outputs. | Partial tokens retained with failure state. | Truncated stream serialized as successful complete response. | J04/J06/J08 |
| M-OB-017 | SupportEnvelope and AdmissionDecision are distinct; current CapacitySnapshot governs safe admission. | Supported profile rejects new request under current memory pressure with explicit reason. | “Supported” forces admission and OOMs/corrupts workload. | J02/J03/J05 |
| M-OB-018 | Performance evidence binds dynamic OperatingState when material and observable. | Power/thermal/throttling/competing load recorded or marked unknown. | Throttled baseline compared to unthrottled candidate as valid speedup. | J05/J09/J10 |
| M-OB-019 | SupportEnvelope has lifecycle/deprecation/effective-version semantics and migration/rollback relation. | Withdrawn profile stops new qualification while historical evidence remains. | Old green silently implies current support after engine/OS update. | J02/J11 |
| M-OB-020 | ObservabilityLevel distinguishes measured, declared/imported and unknowable profile facts. | Remote API hardware/cache fields remain unknown/imported. | Product fabricates precise GPU/cache identity for opaque provider. | J05/J09 |

## Interaction attack

### Fallback × cache identity
Fallback to another engine/profile invalidates cache identity unless explicitly compatible. Do not reuse requested-profile cache evidence for realized fallback.

### Rolling update × stream
Endpoint update cannot relabel in-flight stream. RuntimeGeneration is sticky for request/evidence lifetime.

### Admission × support
A supported profile may reject now. Repeated admission rejection is not proof the profile is unsupported; it is capacity/operating evidence.

### Remote cache × tenant × rollback
Rollback to older generation must not reconnect to incompatible or unauthorized cache namespace merely because keys match.

### Observability × comparison
Unknown remote-provider internals do not automatically make comparison impossible, but claims must be limited to observable outcomes. Memory/cache-mechanism claims are invalid when internals are unknown.

### Deprecation × historical evidence
Deprecation does not delete historical QualificationAssessment. Historical green cannot be promoted to current support without current evidence.

## Mapping state

These v1 obligations are largely architectural gaps in the currently inspected launcher/benchmark surfaces. They become implementation requirements only after accepted product boundary review.
