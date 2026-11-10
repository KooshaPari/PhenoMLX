# PhenoMLX journeys/stages v2 delta

Adds/strengthens:
- M-J02 profile declaration now produces RequestedProfile + explicit support/fallback policy.
- M-J03 launch resolves RealizedProfile, RuntimeGeneration and DeploymentTopology.
- M-J04 stream preserves generation and explicit COMPLETE/PARTIAL_FAILED/CANCELLED terminal state.
- M-J05 inspection includes ObservabilityLevel, CapacitySnapshot, OperatingState and CacheDomain where applicable.
- M-J11 update/rollback includes support lifecycle/deprecation and generation transition.
- M-J12 distributed/cache-domain administration: configure placement/state store/isolation and validate cross-domain safety.

CVP for a single local non-distributed profile may mark distributed fields NOT_APPLICABLE with reason, but cannot omit Requested-vs-Realized, stream terminal state, admission/capacity or observability semantics.

MVP's second profile should intentionally differ in engine/platform or remote-vs-local observability so the abstraction is falsified if it only wraps one runtime.
