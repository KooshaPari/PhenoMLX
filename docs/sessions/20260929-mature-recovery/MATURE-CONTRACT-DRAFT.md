# PhenoMLX mature contract — provisional reconstruction

Source `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`. **Unaccepted draft, incomplete source reconciliation, not a grading catalogue.**

## Product thesis and existence alternative

Working thesis: make justified inference/runtime extensions usable and measurable under explicit model, backend, hardware and deployment profiles, while preserving the user-intended multi-engine research breadth. A crate, import or kernel name is not a supported inference path. Qualification concerns actual quality, memory residency, execution behavior, installability and recovery; raw tokens/s is not sufficient.

Best current alternative-stack hypothesis: upstream oMLX over MLX-LM for Apple-native serving and management, MLX-LM directly for experiments needing its API, and llama.cpp or another justified engine for a supported non-Apple profile; PhenoLab or established evaluation tooling supplies comparisons. The task is to prove which focused extension cannot be served as an upstream patch or adapter. This is not permission to remove existing features or declare a universal winning engine.

## Ontology and independent projections

Domain objects: model artifact/quantization/tokenizer, backend capability, hardware profile, deployment artifact, configured runtime instance, inference request/stream, session/cache lineage, memory resource budget, extension/kernel version, measurement and qualification.

Experience projection: install/doctor/configure, choose supported profile, infer/stream/cancel, manage models and caches, inspect resource/performance evidence, update/rollback. Runtime projection: FFI, backend selection, scheduling, cache state, kernels, cancellation/unload/recovery. Research projection: hypothesis, matched baseline, workload, negative result and experimental-vs-supported status. Assurance projection: provenance, quality gates, support matrix, security, licenses and failure boundaries.

These are related graphs, not equal-sized feature branches. Shared quality/evidence constraints are referenced, not cloned onto each kernel or backend.

## Distinct obligation seeds for source reconciliation

| ID | Candidate obligation / provenance | Concrete positive acceptance | Counterexample / negative control | Journey and work-surface candidates |
|---|---|---|---|---|
| M-O-INSTALL | A declared installation resolves the intended candidate and dependencies; M-S01/02/04/08 | Clean host follows supported recipe and reports exact app/package/backend builds | Adjacent checkout, missing app or an incompatible bundled Python cannot yield ready/supported | M-J-INFER; launcher, env script, packaging and Python load paths |
| M-O-PROFILE | Expose truthful model/backend/hardware support and execute the selected profile; M-S02/03/08 | Named supported profile produces valid stream with actual backend/model identity | Unsupported model, fallback engine or mismatched quantization cannot inherit qualification | M-J-INFER; backend plugins/FFI, actual mounts pending |
| M-O-LIFECYCLE | Cancel, unload and restart without corrupting other requests or confusing cache identities; M-S01/08 | Cancel mid-stream, unload, restart and obtain explicit bounded states | Wrong-prefix cache reuse, resident leak, hung cancellation or incomplete recovery remains failure | M-J-RECOVER; scheduling/cache/persistence paths pending |
| M-O-QUALIFY | Qualify extensions against an identified baseline at acceptable output quality; M-S01/05/08 | Matched workloads preserve approved quality and report end-to-end memory/latency distributions | Packed bytes alone, warm-vs-cold mismatch, changed weights or missing raw data cannot establish a win | M-J-QUALIFY; benchmark runner, telemetry, cache accounting |
| M-O-EXPERIMENT | Preserve failed/negative experiments and distinguish experiments from supported product behavior; M-S01/05/08 | The 1.375x result remains inspectable and restricted to its profile | Passing codec self-test cannot label a slower/larger runtime an improvement | M-J-QUALIFY; result records/docs, raw artifacts still missing |

Promotion requires accepted provenance, rationale, parent capability, dependencies, stage/journey mapping, concrete quality overlays, trace requirements, exact implementation surfaces and growth disposition. No accepted obligation is deleted because this seed list omits it.

## Journeys and mature-first stage projections

M-J-INFER: operator installs the candidate → chooses a supported profile → sends a request through the actual machine/human entrypoint → observes a useful valid result and exact identity. M-J-RECOVER: operator cancels/unloads/loses a process → resource state and durable configuration remain interpretable → reopens service and verifies unrelated requests/cache isolation. M-J-QUALIFY: researcher selects a hypothesis and matched baseline → freezes inputs → collects quality and resource evidence → publishes improvement, regression or uncertainty without hiding failures.

CVP proposal: one real supported profile closing install/infer/cancel/restart with retained evidence, using the mature model/cache/configuration identities. MVP proposal: close qualification/recovery and widen supported adapters without replacing the core state model. Beta/GA add only justified platform/model/deployment breadth and operational guarantees. Stage projection and numerical performance envelopes are not accepted yet.

## Quality overlays and transition debt

Quality non-inferiority is a hard gate on the exact workload, not a weighted reward. Resource measurements distinguish packed payload, metadata, residuals, decoded buffers, allocator/workspace peaks and resident memory after unload. Latency separates prefill, TTFT, inter-token behavior and sustained concurrency; cache warmth and content identity are declared. Security, accessibility and reliability apply to relevant interfaces/profiles with source-backed targets, not copied generic rows.

Transition debt includes external app injection, Python ABI/version assumptions, backend-selection semantics, cache API evolution, experimental namespaces, root/upstream license lineage and duplicated cockpit responsibility. Cost these before selecting a rewrite. Preserve consumer interfaces during any justified migration.
