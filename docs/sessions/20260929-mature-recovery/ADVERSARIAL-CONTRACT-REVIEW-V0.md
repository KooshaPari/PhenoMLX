# PhenoMLX adversarial contract review v0

Assume the typed RuntimeProfile ontology is incomplete.

| Attack | Finding | Contract consequence |
|---|---|---|
| Distributed tensor/pipeline/data-parallel runtime | HardwareProfile alone may not express topology/interconnect/placement. | Add DeploymentTopology identity. |
| Remote/disaggregated KV store changes independently | CachePolicy needs external store/connector identity and consistency/security semantics. | Add StateStore/Connector binding. |
| Runtime silently falls back from custom kernel/cache | Profile evidence would lie if fallback not recorded. | Require realized-profile identity, not requested-profile only. |
| Engine auto-tunes scheduler/memory parameters | Exact runtime behavior may differ from declared config. | Capture effective configuration in assessment. |
| Model supports multiple chat templates/tokenizers | ModelArtifact identity must bind preprocessing/template. | Expand request/model identity. |
| Quantization calibrated from dataset | QuantizationProfile needs calibration provenance. | Add calibration source/version where applicable. |
| Speculative decoding uses separate draft model | Draft artifact/tokenizer/config is first-class profile component. | Already implied; make explicit in obligations. |
| Prefix cache shared across tenants | Security domain/salt is acceptance-relevant. | Add isolation-domain hard constraint. |
| Hot model update while service remains running | RuntimeInstance can change artifact beneath requests. | Require epoch/version binding per request. |
| Remote API engine has provider-side opaque changes | Exact build unavailable. | SupportEnvelope must represent opaque provider revision/observation uncertainty. |
| Quality differs by workload language/domain | One global quality envelope is invalid. | QualificationAssessment population/workload scope first-class. |
| Power/thermal throttling | Hardware identity alone insufficient for performance comparability. | Add operating-condition telemetry where relevant. |

## New obligation candidates
M-OB-012 Realized RuntimeProfile/effective config is recorded, including fallback/autotuning.
M-OB-013 Distributed DeploymentTopology and external state connectors are identity-bearing where used.
M-OB-014 Cache/prefix isolation domain prevents cross-tenant/invalid identity reuse.
M-OB-015 Quantization/speculation auxiliary artifacts and calibration provenance are bound.
M-OB-016 Performance evidence declares operating conditions/population scope.

These are semantic additions, not cloned generic quality rows.