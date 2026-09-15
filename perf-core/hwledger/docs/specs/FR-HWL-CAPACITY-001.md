# FR-HWL-CAPACITY-001 — Capacity Fit Estimate

> Functional requirement for the capacity-planner user-facing flow.
> Linked from `docs/operations/journey-traceability.md`.

## Statement

A user MUST be able to produce a hardware fit estimate for a candidate model on
a target device and receive explainable reasoning for the verdict.

## User Story

> As a **capacity planner**, I want to estimate whether a model fits on a
> device and understand *why*, so that I can pick hardware confidently.

## Acceptance Criteria

1. CLI (`hwledger fit <model> <device>`) returns a structured verdict
   (`fit | tight | fail`) plus per-axis breakdowns: parameters, KV cache, memory
   headroom, and throughput estimate.
2. Streamlit capacity page renders the same verdict and links each value back to
   the input that produced it.
3. Output includes a reproducible command string that the user can paste to
   re-run the fit.
4. Every measurement cell is annotated with its source (probe, vendor spec,
   user override) per `NFR-HWL-REPRODUCIBILITY-001`.

## Non-Regression Constraints

- `NFR-HWL-EXPLAINABILITY-001`: every score is paired with the calculation
  inputs. Silent defaults are forbidden.
- Verdict thresholds live in `hwledger-core::capacity::policy`; the same policy
  is consumed by CLI and Streamlit to avoid drift.
- Backwards-compatible JSON schema for the verdict output.

## Linked Surfaces

- Code: `crates/hwledger-core`, `crates/hwledger-cli`, `apps/streamlit`
- Docs: `docs/operations/journey-traceability.md#user-facing-flows`
- Journey: `docs/journeys/manifests/capacity-fit-estimate.md`
- Evidence: `../assets/rich-media/hwledger/capacity-fit-estimate.png` (TODO)

## Test / Gate Coverage

- `cargo test -p hwledger-core capacity` — unit cases for each `AttentionKind`.
- `cargo test -p hwledger-cli fit` — CLI snapshot tests for the verdict.
- Streamlit snapshot test for the capacity page (manual today; TODO in CI).
- Journey manifest validation: `docs/journeys/manifests/capacity-fit-estimate.md`.

## Status

- [x] Requirement authored
- [ ] Manifest written
- [ ] Capture recorded
- [ ] Eval verdict linked
