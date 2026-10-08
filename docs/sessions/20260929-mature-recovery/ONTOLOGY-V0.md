# PhenoMLX ontology v0 — typed runtime profiles

Date: 2026-09-30. **DRAFT; not accepted contract or grading denominator.**

SOTA now makes a universal homegrown inference/cache engine a weak default. The ontology therefore preserves backend-specific semantics while giving the product a common qualification/control model.

## Core identities

### ModelArtifact
Exact weights/model revision, architecture, tokenizer and model-level metadata.

### QuantizationProfile
Weight/activation/KV/state quantization and calibration identity. Separate from ModelArtifact because one model can have several qualified representations.

### EngineArtifact
Engine/runtime name, exact version/build/commit, compile flags/plugins and distribution identity.

### HardwareProfile
Device(s), memory, architecture/SM/Apple generation, CPU/RAM/interconnect, driver/runtime and OS.

### StateTopology
The model's inference state structure: full attention, local/sliding attention, recurrent/Mamba/SSM, MLA/GQA/MQA or hybrid composition as applicable.

### CachePolicy
Engine-specific cache representation/reuse/offload/tiering/prefix/security policy. Not flattened to one universal schema; capability adapters preserve native semantics.

### SpeculationPolicy
Draft/self/tree/multi-token/etc. policy plus draft model/config where applicable.

### RuntimeProfile
The primary qualified identity:
`ModelArtifact × QuantizationProfile × EngineArtifact × HardwareProfile × StateTopology × CachePolicy × SpeculationPolicy × Scheduler/ConcurrencyConfig`.

A materially changed factor is a new profile/evidence subject.

### RuntimeInstance
A running realization of a RuntimeProfile with process/device/resource identity.

### InferenceRequest / Stream
Request and streaming lifecycle with exact RuntimeInstance/profile binding.

### ResourceState
Measured allocations and lifecycle state: weights, active state/KV, packed payload, metadata/scales, decoded buffers, workspace, allocator reserve, host/offload tiers, peak/steady/cleanup.

### QualificationWorkload
Versioned prompt/request schedule, context/concurrency/cache-warmth distribution and quality oracle.

### QualificationAssessment
Evidence-backed behavior/performance/quality result for RuntimeProfile × Workload.

### Extension
PhenoMLX-owned kernel/cache/adapter/optimization with explicit compatibility surface and baseline.

### SupportEnvelope
Accepted set of model/engine/hardware/config combinations and known unsupported/degraded/fallback combinations.

## Relations

```
RuntimeProfile -> exact component identities
RuntimeInstance realizes RuntimeProfile
InferenceRequest -> RuntimeInstance
QualificationAssessment -> RuntimeProfile + Workload + raw evidence
Extension modifies one or more RuntimeProfile components
SupportEnvelope includes/excludes RuntimeProfiles or capability predicates
```

## Capability semantics

Common capability names may include generation, streaming, batching, prefix reuse, KV/state quantization, offload, speculation, cancellation and unload, but support is **profile-scoped**.

Never infer:
`engine supports feature X => every model/hardware/config combination supports X`.

## Evidence hard gates

Wrong model/tokenizer/weights, changed cache warmth, changed hardware, incompatible engine build, missing quality evidence or incomplete allocation accounting makes a performance comparison invalid/incomparable.

## Product boundary hypothesis

PhenoMLX owns:
- profile/capability registry;
- qualification;
- adapters/control;
- evidence;
- measured extensions.

Existing engines own their native schedulers/cache/runtime implementations unless a custom Extension is experimentally justified.

## Journey projections

M-J01 install candidate; M-J02 discover/declare profile; M-J03 launch instance; M-J04 infer/stream; M-J05 inspect identity/resources; M-J06 cancel; M-J07 unload; M-J08 restart; M-J09 qualify profile; M-J10 compare extension; M-J11 update/rollback.

## Open ontology questions

- whether engine build and deployment artifact need separate identities;
- exact remote/distributed cache identity;
- cross-host/multi-GPU topology representation;
- support-envelope versioning and deprecation;
- how profile selection is exposed to PhenoLab/OmniRoute consumers.

No universal engine abstraction or requirement count is implied.
