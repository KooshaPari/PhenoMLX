# PhenoMLX — pass 4 accessible non-main ref falsification

Observation date: 2026-09-29. Frozen product source remains `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`.

## Accessible archived mirror

`KooshaPari/PhenoMLX-temp` exposes only one branch, `main`. Pass 3 proved that exact main head is contained in current PhenoMLX history and current main is 89 commits ahead / 0 behind from it.

Thus the accessible temp remote itself currently exposes no additional non-main branch to inspect.

## Current PhenoMLX non-main branches

GitHub exposes five branches total at observation:
- `main`;
- recovery specification branch;
- `feat/agileplus-snapshot-18b231ee`;
- `fix/metal-fixture-manifest-binding-20260816-v2`;
- `wip/2026-07-28-d-phenotype-omlx`.

### `feat/agileplus-snapshot-18b231ee`

Comparison to current main: **ahead 0, behind 117**. No unique commit needs recovery. Classification: historical snapshot, represented by current main.

### `fix/metal-fixture-manifest-binding-20260816-v2`

Comparison: **ahead 5, behind 115**, merge base `678083a51c67877a8740e08843bb488099d8ca06`.

Unique material includes:
- `scripts/metal_artifact_contract.py`;
- changes to `record_metal_device_fixture.py`;
- a large expansion of `scripts/tests/test_metal_device_fixture.py`;
- CI and mailmap changes.

Provisional semantic classification: **assurance/evidence experiment with potentially reusable acceptance logic**, not automatic product implementation. It directly intersects the mature requirement that performance/kernel evidence bind to an exact device/artifact/profile. Before dropping it, compare its contract semantics against current evidence/fixture machinery.

### `wip/2026-07-28-d-phenotype-omlx`

Comparison: **ahead 13, behind 256**, merge base `e1ada8a4bb4ebda33308e6ae137bca39b81e449b`.

The unique branch is substantial. GitHub file comparison shows:
- bench-cockpit capacity/resolver and VRAM-fit work;
- large recorded Qwen3.5 run/evaluation artifacts;
- Windows dev scripts;
- platform-shell/Tauri scaffolding and generated schemas/icons;
- additional cockpit UI/server changes.

Provisional classification:
- recorded benchmark artifacts: historical evidence requiring identity/quality checks;
- capacity/VRAM-fit/resolver logic: potentially relevant runtime/profile qualification concepts;
- platform-shell/Tauri: client experiment / product-surface proposal;
- generated assets: auxiliary, not product intent;
- all unique commits: **not safe to discard solely because branch is old**.

This branch needs source-level semantic extraction, but does not become current product truth without authority and current-consumer evidence.

## Formal-document contamination remains confirmed

The exact-blob `PRD.md` and `FUNCTIONAL_REQUIREMENTS.md` inherited through temp/current history describe phenotype-shared infrastructure rather than PhenoMLX. Their formality is not authority. Exclude them from the PhenoMLX grading denominator unless a later accepted source explicitly adopts an obligation.

## Falsification result

The proposition “all meaningful non-main history is already represented by current main” is **false** for the currently accessible PhenoMLX repository. At least two non-main branches contain unique commits.

The narrower proposition “accessible PhenoMLX-temp main contains missing post-head commits” remains false: current main contains and advances beyond it.

## Next closure criterion

For the two unique branches, create a semantic delta ledger:
`unique artifact → intent/source authority → current equivalent → accepted obligation candidate → retain/adapt/reject/defer`.

History can be called reasonably exhausted when:
1. every accessible unique branch has that semantic disposition;
2. Registry's old zz-archive all-ref records reveal no additional accessible refs or preserved unique commit set;
3. no formal-looking contaminated document remains in the normative source set.

No merge/cherry-pick was performed.
