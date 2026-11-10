# PhenoMLX — pass 5 semantic disposition of unique accessible branches

Observation date: 2026-09-29. Frozen source remains `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`. This pass classifies historical material; it does not merge/cherry-pick it.

## Branch: fix/metal-fixture-manifest-binding-20260816-v2

Graph: 5 commits unique relative to its merge base; 115 commits behind current main.

### Unique family A — `scripts/metal_artifact_contract.py`

**Semantic role:** evidence/fixture identity contract for real Metal-device artifacts.

**Mature-contract relevance:** HIGH. It aligns with the independently recovered requirement that runtime/kernel evidence bind to exact hardware/profile/artifact identity and that synthetic or wrong-device evidence not qualify a claim.

**Disposition:** **LEARN FROM / ADAPT if current main lacks equivalent semantics.** Do not resurrect branch wholesale. Compare its manifest fields and fail-closed behavior with current artifact/evidence machinery.

### Unique family B — fixture recorder + expanded tests

**Semantic role:** stronger recording/negative-control coverage around Metal fixture provenance.

**Disposition:** **HISTORICAL ASSURANCE EXPERIMENT, candidate for selective port after current-equivalent mapping.** Tests are useful oracle ideas even if implementation APIs changed.

### Unique family C — CI/mailmap

**Disposition:** auxiliary/hygiene unless a current CI obligation depends on it. Not product differentiation.

## Branch: wip/2026-07-28-d-phenotype-omlx

Graph: 13 commits unique relative to merge base; 256 commits behind current main.

### Unique family A — large Qwen3.5 run artifacts

Files include very large recorded run/contract JSON.

**Semantic role:** historical benchmark evidence.

**Disposition:** **PRESERVE AS HISTORICAL EVIDENCE; NOT CURRENT QUALIFICATION** until candidate/model/backend/hardware/verifier identities and raw-generation method are validated. Never derive current performance from filename alone.

### Unique family B — bench-cockpit capacity / VRAM-fit / resolver

**Semantic role:** capacity estimation and model/runtime fit surfaced to the cockpit.

**Mature relevance:** MEDIUM-HIGH as a human projection over qualified runtime profiles. The underlying capacity/fit model belongs with runtime/profile evidence; UI is secondary.

**Disposition:** **LEARN FROM / RECONCILE WITH CURRENT COCKPIT.** Retain semantic requirement candidate (“operator can determine whether a declared model/profile fits a hardware budget from qualified data”), but do not import old implementation until current equivalents are traced.

### Unique family C — Windows dev start/stop scripts

**Semantic role:** development/operator convenience.

**Disposition:** **HISTORICAL IMPLEMENTATION / transition-debt evidence.** Only retain if current supported Windows profile still needs this path. Not core mature identity.

### Unique family D — platform-shell / Tauri scaffolding

**Semantic role:** desktop/native shell experiment.

**Authority:** no direct evidence in this pass that a native Tauri shell is accepted mature product intent.

**Disposition:** **EXPERIMENT / DEFER.** It must not create UI/native-app requirements by existence alone.

### Unique family E — generated schemas/icons

**Disposition:** AUXILIARY/GENERATED. Ineligible as semantic obligations.

## Falsification result

The unique branches contain **useful semantic evidence**, but no inspected family requires merging an old branch as the product baseline.

What survives into mature-contract recovery:
1. exact device/artifact provenance for hardware/kernel evidence;
2. capacity/VRAM-fit as a possible operator journey over qualified profiles;
3. preservation of negative/historical benchmark results.

What does not survive automatically:
- Tauri/native shell;
- old Windows dev scripts;
- generated assets;
- old benchmark claims as current evidence;
- branch-specific implementation APIs.

## W1 closure assessment for accessible GitHub refs

Accessible current-repository branches have now been enumerated and semantically classified at family level. Accessible `PhenoMLX-temp` exposes only main and is contained in current history.

Remaining historical uncertainty is limited to Registry-described older zz-archive refs that are not exposed by the currently accessible temp repository. Unless those refs become accessible or Registry provides a unique-commit manifest, treat them as **known unavailable historical sources**, not an indefinite blocker.

W1 can therefore transition from broad archaeology to **targeted residual** status. SOTA/profile architecture work may proceed, with the unavailable-ref uncertainty carried explicitly.

No historical branch was merged or modified.
