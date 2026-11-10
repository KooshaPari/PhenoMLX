# PhenoMLX ontology v1 — post-falsification typed profiles

Status: DRAFT, supersedes v0 as current candidate model.

## Static identities
ModelArtifact; QuantizationProfile; EngineArtifact; HardwareProfile; StateTopology; CachePolicy; SpeculationPolicy; SchedulerConfig; DeploymentTopology; PlacementPlan; StateStore; CacheDomain.

## Profile identities
RequestedProfile — operator/client intent.
RealizedProfile — exact runtime configuration that actually served/qualified work.
SupportEnvelope — lifecycle/versioned declaration: experimental, supported, deprecated, withdrawn, plus incompatibilities/fallbacks.
ObservabilityLevel — which profile facts are measured, declared/imported or unknowable for a provider.

A RequestedProfile may realize differently only under explicit fallback policy; evidence binds RealizedProfile.

## Runtime identities
RuntimeInstance; RuntimeGeneration; InferenceRequest; Stream; StreamArtifact; CapacitySnapshot; AdmissionDecision; ResourceState; OperatingState.

RuntimeGeneration changes on hot swap/rolling engine/model/profile update even if endpoint remains stable. In-flight request remains bound to serving generation.

Stream terminal states distinguish COMPLETE, PARTIAL_FAILED, CANCELLED and other explicit failures. Partial tokens are evidence/artifact, not completed response.

CapacitySnapshot is time-scoped. SupportEnvelope says a profile can run; AdmissionDecision says whether this instance can safely admit this request now.

OperatingState includes dynamic performance-relevant state where measurable: power/thermal mode, throttling, competing load.

CacheDomain binds tenant/security/isolation identity and StateStore. Cross-domain prefix reuse is forbidden unless explicitly safe/authorized.

## Qualification
QualificationWorkload and QualificationAssessment bind RealizedProfile, DeploymentTopology, OperatingState, ObservabilityLevel and full evidence. Unknown remote-provider internals remain unknown rather than invented.

## New obligations to derive
requested-vs-realized fallback; distributed placement; remote cache identity/isolation; hot-swap generation; partial streams; admission; dynamic operating state; support deprecation; observability truthfulness.

v0 is retained as falsified predecessor.
