# Implementation mapping pass 3 — direct tree/file traversal

Date 2026-09-30. Frozen source `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`.

Direct recursive-tree traversal materially changes the sparse-search map.

## Existing implementation breadth confirmed

Current source contains:
- `python/omlx_research/backends/base.py` plus llama.cpp and Metal backends;
- bench-cockpit capacity, cell assignment/metrics/raw/RLVR, eval report, matrix import and Portage bridge;
- historical/current candidate provenance/evaluation envelopes;
- `perf-core` eval backend, hardware ledger/capacity, kernel registry, Metal runtime, speculative decode, turbo-quant and many contract tests;
- ADRs for tiered runtime and multi-engine dispatch.

This breadth is evidence of experiments/primitives, not automatic mature scope.

## Backend abstraction is real but thin

The backend base defines GenerateRequest/GenerateResponse, abstract generate/is_available and lazy load/error capture.

**Correction:** engine adapter abstraction is IMPLEMENTED at a basic generation layer.

It does not encode mature RequestedProfile/RealizedProfile, topology, support lifecycle, cache domain, runtime generation or qualification identity.

## Capacity UI is implemented but deliberately heuristic

`apps/bench-cockpit/server/capacity.go` estimates VRAM essentially as params × dtype bytes, with model-name heuristics and a default 24 GiB capacity.

**Classification:** IMPLEMENTED HEURISTIC PROJECTION, not qualified CapacitySnapshot/AdmissionDecision. It validates M-OB-011's need to separate operator hint from evidence-backed fit.

## EvaluationReport has useful evidence labels but lossy semantics

Cockpit eval-report import includes contract version, run ID/variant/model/evidence_label/executed_by/command, suite/task status/judge/perf fields and optional verified-pass metadata.

But its conversion derives `OK` as status=="ok" **or passAt1 >= 0.999**, defaults evidence label to "reported", and computes summaries. This is a visualization/import projection, not an acceptance authority.

**Risk:** do not let cockpit `OK` become product green when verifier/evidence identity is incomplete.

## Candidate provenance is stronger than expected

Historical candidate-provenance JSON explicitly records repository/branch/head, clean-at-compile, artifact SHA-256 binding, compile-only status, device fingerprint unknown, Harbor evidence pending, evidence_complete=false and promotion blocked.

**Correction:** evidence/provenance discipline is already present in historical artifacts and should be generalized rather than reinvented.

It also demonstrates truthful unknowns: device fingerprint null/unknown and compile-only evidence does not become runtime qualification.

## Surviving gaps

- typed RequestedProfile/RealizedProfile;
- SupportEnvelope lifecycle and incompatibilities;
- RuntimeGeneration;
- DeploymentTopology/PlacementPlan;
- CacheDomain/StateStore authority;
- stream terminal semantics;
- evidence-bound dynamic CapacitySnapshot/AdmissionDecision;
- OperatingState;
- generalized QualificationAssessment linking the strong provenance envelope to actual runtime/profile identity.

## Architecture implication

PhenoMLX already has many engine/runtime/kernel experiments. Mature contract should **constrain and qualify** them, not canonize every implementation as product scope. The strongest reusable pieces are backend adapters, provenance envelopes, evaluation/cockpit projections and perf-core experiments; mature spine remains typed profile + qualification/evidence.
