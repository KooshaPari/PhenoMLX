# FR-HWL-INFERENCE-001 — Local Runtime Health

> Functional requirement for the local-inference-runtime user-facing flow.
> Linked from `docs/operations/journey-traceability.md`.

## Statement

A user MUST be able to start the local inference runtime and observe its health,
resource constraints, and a pass/fail eval verdict for a candidate workload.

## User Story

> As a **runtime operator**, I want to know the runtime is healthy and the
> constraints it is operating under, so that I can trust the answers it gives.

## Acceptance Criteria

1. `hwledger-devtools up` starts the inference sidecar, the FFI server, and the
   Streamlit UI; `/healthz` returns `200 OK` with a JSON health document.
2. The health document includes: runtime version, model id, KV cache state,
   resident memory, thermal state, and constraint flags.
3. `phenotype-journey verify local-runtime-health` runs an end-to-end probe
   and writes a verdict JSON that maps to FR-HWL-INFERENCE-001 +
   NFR-HWL-OBSERVABILITY-001.
4. Constraint violations surface as explicit, named warnings — never as
   reduced functionality (no graceful degradation).

## Non-Regression Constraints

- `NFR-HWL-OBSERVABILITY-001`: required fields are non-nullable; missing data
  is a hard failure, not a logged warning.
- Health endpoint must remain idempotent under repeated calls.
- Verdict schema is versioned (`verdict_schema_version`); mismatches fail
  the gate rather than silently reinterpret.

## Linked Surfaces

- Code: `crates/hwledger-inference`, `crates/hwledger-server`, sidecar fork
  (`sidecars/omlx-fork`)
- Docs: `docs/operations/journey-traceability.md#user-facing-flows`
- Journey: `docs/journeys/manifests/local-runtime-health.md`
- Evidence: `../assets/rich-media/hwledger/local-runtime-health.png` (TODO)

## Test / Gate Coverage

- `cargo test -p hwledger-inference health` — health document schema
- `cargo test -p hwledger-server healthz` — endpoint smoke
- Sidecar integration: `cargo test -p hwledger-inference sidecar_health` —
  reachability + KV cache assertions
- Journey manifest validation: `docs/journeys/manifests/local-runtime-health.md`

## Status

- [x] Requirement authored
- [ ] Manifest written
- [ ] Capture recorded
- [ ] Eval verdict linked
