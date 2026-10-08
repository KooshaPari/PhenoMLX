# PhenoMLX journeys and stage projections v0

Status: **DRAFT.**

## Mature journeys
### M-J01 — Install/resolve candidate
Supported installation proves exact PhenoMLX source/package plus external engine/app dependencies.
### M-J02 — Discover/select RuntimeProfile
Consumer sees compatible model×engine×hardware×cache/speculation combinations, unsupported combinations and assumptions.
### M-J03 — Launch RuntimeInstance
Selected profile starts with exact identity and bounded resource/configuration state.
### M-J04 — Infer/stream
Client receives correct generation/stream behavior; cancellation/error semantics explicit.
### M-J05 — Inspect profile/resource identity
Resolve actual model/tokenizer/quantization/engine/build/hardware/cache/state and resource accounting.
### M-J06 — Cancel safely
Cancel one request without corrupting other requests/cache identities or leaking unbounded resources.
### M-J07 — Unload/reclaim
Unload and demonstrate declared cleanup/residual resource state.
### M-J08 — Restart/recover
Restart and distinguish reusable durable configuration/cache from stale/incompatible state.
### M-J09 — Qualify RuntimeProfile
Versioned workload with quality hard gates and full latency/throughput/memory accounting.
### M-J10 — Compare Extension
Stock idiomatic engine vs extension under matched conditions; record win, loss or incomparable.
### M-J11 — Update/rollback
Change engine/extension/profile version with compatibility validation and reversible rollback.
### M-J12 — Export support/evidence
Consumer receives exact profile support/qualification rather than generic feature checkbox.

## Stage projections
### CVP
One real profile closes M-J01–M-J09 and M-J12 on one hardware population, including cancel/unload/restart and a negative/unsupported profile. No custom kernel required.
### MVP
Adds M-J10/M-J11 and at least one second engine/profile or hardware population through same profile/evidence spine.
### Beta
Wider models/backends/concurrency/context/cache policies, incompatibility matrix, qualified operations and security/cache-isolation controls.
### GA
Supported install/update/rollback, stable adapter/API contracts, qualified performance/quality envelopes, observability and failure recovery.
### Mature
Evidence-backed cross-engine selection plus only custom Extensions that survive best-stock comparisons.

## Current journey state
M-J01 is not closed: launcher relies on external app/env and has known web argument defect. Historical benchmarks do not close M-J09. No stage qualification claimed.