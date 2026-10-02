# Shared mature-recovery invariants v0

Status: **DRAFT semantic constraints; not yet accepted grading policy.**

These invariants are referenced by product obligations rather than mechanically duplicated into every feature.

| ID | Invariant | Counterexample that must fail |
|---|---|---|
| INV-AUTH-01 | Authority and confidence are independent. Inference/import does not become an authorized decision because confidence is high. | LLM-inferred edge with 0.99 confidence silently changes accepted product state. |
| INV-EVID-01 | Acceptance evidence binds exact product, subject, contract/criterion, candidate/configuration, environment, verifier/policy, run/assessment, time and raw provenance as applicable. | Evidence from candidate A qualifies candidate B. |
| INV-EVID-02 | Missing, skipped, collector-failed, verifier-unavailable or structurally invalid required evidence is never PASS. | Empty result/failed collector renders green. |
| INV-EVID-03 | Historical evidence remains historical and is never silently relabelled current. | Old benchmark result qualifies changed model/runtime. |
| INV-STATE-01 | WorkerAttempt, durable development/experiment effort and product state have separate identities/lifetimes. | Killing worker deletes accepted product truth or completed effort. |
| INV-HIST-01 | Accepted history is append/supersede, not destructive rewrite. | Regrade overwrites original execution result. |
| INV-GRADE-01 | Candidate implementation cannot weaken its own acceptance policy and receive engineering credit for the resulting green. | Patch deletes negative test/lowers threshold and passes. |
| INV-SCOPE-01 | Contract/scope delta is separate from engineering delta against unchanged scope. | Removing requirement counts as implementation progress. |
| INV-FAIL-01 | Dependency/infrastructure/collector failures remain distinguishable from subject behavioral failure. | Provider outage scored as wrong model answer. |
| INV-TRACE-01 | Trace relations preserve provenance and validation/authority state bidirectionally. | Test points to requirement but requirement has no corresponding verified test relation. |
| INV-RECOV-01 | Retry/restart/worker replacement creates explicit attempt lineage and cannot duplicate or inherit acceptance accidentally. | Failed retry reuses previous pass. |
| INV-COMP-01 | Comparison requires declared comparability; material model/config/workload/environment differences can yield INCOMPARABLE rather than forced ranking. | Warm optimized run compared to cold baseline as a valid speed win. |
# PhenoMLX semantic obligations v0

| ID | Distinct obligation | Provenance / rationale | Positive acceptance | Negative acceptance | Current mapping |
|---|---|---|---|---|---|
| M-OB-001 | RuntimeProfile binds exact model/tokenizer/quantization/engine/build/hardware/state/cache/speculation/scheduler identity. | Multi-engine intent + SOTA. | Assessment resolves all acceptance-relevant profile components. | Evidence for different engine/model/cache policy qualifies current profile. | Current launcher/environment injects components but no unified exact profile identity found; GAP/PARTIAL. |
| M-OB-002 | SupportEnvelope is profile-scoped and records unsupported/incompatible/degraded combinations. | Engine compatibility research. | Unsupported combination fails explicitly before qualification. | Engine-level feature flag implies every model/hardware supports it. | Backend status surfaces exist; semantic mapping OPEN. |
| M-OB-003 | A documented installation path resolves the intended candidate rather than adjacent checkout/system app accidentally. | Launcher/source findings. | Clean install reports exact candidate and dependencies. | External oMLX app silently substitutes unrelated version. | `omlx-research` relies on external app/env; PARTIAL/RISK. |
| M-OB-004 | Human/machine invocation forwards configuration correctly. | Web-port defect. | `web 8080` starts configured port 8080. | Subcommand token becomes port/config. | `cli/bin/omlx-research`; DEFECT. |
| M-OB-005 | Qualification compares profiles under matched model/input/workload/quality/environment conditions. | INV-COMP-01. | Baseline/candidate differ only in declared intervention. | Changed weights/cache warmth/hardware produce claimed speedup. | Benchmark artifacts exist; enforcement OPEN. |
| M-OB-006 | Memory accounting includes all persistent/temporary/state/offload allocations relevant to claim. | 1.375× negative cache result. | Report separates decoded+packed+metadata+workspace+tiers+peak/steady. | Packed bytes alone claimed as total saving. | Latest CUDA report improved byte accounting for one profile; PARTIAL. |
| M-OB-007 | Optimization cannot qualify if accepted output quality regresses beyond profile threshold. | Quality hard gate. | Fixed oracle/non-inferiority passes before speed/memory win. | Faster corrupted output declared improvement. | Some source-recorded quality checks exist; accepted policy OPEN. |
| M-OB-008 | Cancel/unload/restart releases or correctly preserves resources and never confuses cache/request identities. | Runtime lifecycle. | Concurrent request unaffected; cleanup/restart explicit. | Stale prefix/model cache reused across invalid boundary. | OPEN. |
| M-OB-009 | Existing engine primitive is preferred unless a custom Extension demonstrates a target deficiency and accepted win. | SOTA existence gate. | Extension links deficiency, baseline, evidence and maintenance cost. | Custom cache survives despite losing stock engine comparison. | Historical custom extensions currently experimental. |
| M-OB-010 | Historical/negative experiments remain visible and profile-bounded. | CUDA cache finding + INV-EVID-03. | 1.375× result remains discoverable for exact profile after later work. | Negative result deleted/generalized/rewritten. | Source docstring preserves one result; broader evidence store OPEN. |
| M-OB-011 | Capacity/fit decisions derive from qualified profile evidence, not static guess alone. | Historical VRAM-fit branch + operator journey. | Operator sees fit with assumptions/confidence/profile. | UI says fits while omitted cache/workspace causes OOM. | Historical branch prior art; current equivalent OPEN. |

## High-risk implementation mapping

Current `omlx-research` launcher hard-binds an external oMLX app path, bundled Python paths and sourced repository environment. That is useful development integration but not yet a self-identifying qualified RuntimeProfile.

The inspected `web)` branch still reads `port` before shifting the subcommand, violating M-OB-004.

SOTA establishes that cache quantization/prefix/tiering/hybrid handling is not itself a PhenoMLX obligation to custom-build. M-OB-009 controls custom ownership.

## Promotion rule

No obligation becomes accepted merely because an old branch implements it or a benchmark exists. Bind authority, profile, journey, oracle and current surface first.