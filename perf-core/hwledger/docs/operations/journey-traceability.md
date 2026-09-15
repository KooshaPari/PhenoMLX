# Journey Traceability

Implements the [phenotype-infra journey-traceability standard](https://github.com/kooshapari/phenotype-infra/blob/main/docs/governance/journey-traceability-standard.md).

## Traceability Model

Every user-facing flow should be traceable across:

1. **FR/NFR** — requirement ID and user story.
2. **Spec** — acceptance criteria and non-regression constraints.
3. **Docs** — operator/user documentation and rich media placeholders.
4. **Code** — crate, binary, or app surface implementing the flow.
5. **Tests/Gates** — unit, integration, BDD, lint, and journey verification acting as autograders.
6. **Evidence** — journey manifest, recording/keyframes, and evaluation verdict.

## User-Facing Flows

| Flow | Requirement | Implementation surface | Autograder gates | Evidence status |
| --- | --- | --- | --- | --- |
| Capacity planner creates a hardware fit estimate | FR-HWL-CAPACITY-001, NFR-HWL-EXPLAINABILITY-001 | `hwledger-core`, `hwledger-cli`, Streamlit UI | cargo tests, CLI smoke, journey manifest, eval verdict | Stubbed |
| Fleet ledger records and compares local devices | FR-HWL-FLEET-001, NFR-HWL-REPRODUCIBILITY-001 | `hwledger-ledger`, `hwledger-probe`, desktop/runtime UI | probe fixture tests, ledger snapshot tests, journey manifest | Stubbed |
| Local inference runtime reports health and constraints | FR-HWL-INFERENCE-001, NFR-HWL-OBSERVABILITY-001 | `hwledger-inference`, `hwledger-server`, sidecar docs | service health tests, log/metric assertions, journey eval | Stubbed |

## Rich Media Stubs

<!-- RICH-MEDIA-STUB type="annotated-screenshot" subject="Capacity planner result with explainability callouts" journey="capacity-fit-estimate" status="TODO" -->
![Capacity planner result — estimated model fit, memory headroom, and next recommended action](../assets/rich-media/hwledger/capacity-fit-estimate.png)

*Expected capture: run the planner against a known fixture device and annotate the model-fit result, headroom calculation, warning states, and the CLI/UI command that reproduces it.*

<!-- RICH-MEDIA-STUB type="animated-gif" subject="Fleet ledger device comparison flow" journey="fleet-ledger-compare" status="TODO" -->
![Fleet ledger comparison — two devices compared with constraints and provenance](../assets/rich-media/hwledger/fleet-ledger-compare.gif)

*Expected capture: add or load two fixture devices, compare their usable inference capacity, and show provenance for each measured field.*

<!-- RICH-MEDIA-STUB type="journey-eval" subject="Inference runtime health verdict" journey="local-runtime-health" status="TODO" -->
![Inference runtime health — service state, constraints, and eval verdict](../assets/rich-media/hwledger/local-runtime-health.png)

*Expected capture: start the local runtime, verify health/constraint reporting, and attach a pass/fail eval verdict that maps back to FR-HWL-INFERENCE-001 and NFR-HWL-OBSERVABILITY-001.*

## Journey Manifests

Journey manifests should live in `docs/journeys/manifests/` and include:

- requirement IDs covered by the journey;
- command or app entrypoint used to reproduce the flow;
- expected screenshots/GIFs/keyframes;
- tests and gates that must pass before the journey is accepted;
- eval verdict schema and pass/fail criteria.

## Autograder Gates

Minimum gates before marking a journey complete:

- `cargo test --workspace` for Rust behavior;
- targeted CLI/app smoke for the user-facing path;
- doc link validation for every referenced rich media asset;
- journey manifest validation via `phenotype-journey verify` when available;
- eval verdict linked to the FR/NFR IDs in the manifest.

## Specs

Functional requirements referenced above live as standalone FR spec files so
they can be linked from plans, PRs, and journeys without duplicating content:

- [FR-HWL-CAPACITY-001 — Capacity Fit Estimate](../specs/FR-HWL-CAPACITY-001.md)
- [FR-HWL-FLEET-001 — Fleet Ledger Compare](../specs/FR-HWL-FLEET-001.md)
- [FR-HWL-INFERENCE-001 — Local Runtime Health](../specs/FR-HWL-INFERENCE-001.md)

## Manifests

Per-flow manifests live under `docs/journeys/manifests/`:

- `capacity-fit-estimate.md`
- `fleet-ledger-compare.md`
- `local-runtime-health.md`

## Status

- [x] Identify initial user-facing flows
- [x] Stub rich media embeds for expected screenshots/GIFs/evals
- [x] Author FR spec files under `docs/specs/`
- [x] Author manifests in `docs/journeys/manifests/`
- [ ] Record journey captures for each flow
- [ ] Run `phenotype-journey verify` in CI
