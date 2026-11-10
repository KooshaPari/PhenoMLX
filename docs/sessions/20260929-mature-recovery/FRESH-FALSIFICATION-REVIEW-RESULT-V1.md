# Fresh falsification review result v1

Date: 2026-10-01.
Method: assume current vNext is incomplete; construct product behavior/lifecycle/configuration alternatives not explained by current contract.

## Findings repaired

### Portage — MATERIAL
Portable task schema omitted secret injection, user/privilege, health/readiness, dependency ordering, volume persistence/permissions and several lifecycle/network semantics. These are not implementation details because backend parity cannot be judged without them.
Repair: PORTABLE-TASK-CONTRACT-SEMANTIC-EXTENSIONS-V1.md.

### PhenoMLX — MATERIAL
Research-runtime contract did not sufficiently bind custom extension API/ABI/version compatibility, activation/fallback lifecycle or controlled-vs-incidental experiment factors.
Repair: RUNTIME-EXTENSION-LIFECYCLE-V1.md.

### PhenoLab — MATERIAL
ExperimentProgram target/budget representation allowed underspecified units/aggregation, vague Pareto/non-inferiority targets and unknown budget consumption ambiguity.
Repair: OBJECTIVE-BUDGET-SEMANTICS-V1.md.

## Re-run after repair

No additional BLOCKING non-runtime interpretation was identified in this pass.

Remaining material uncertainties require empirical execution:
- Portage backend semantic parity/materialization burden;
- PhenoMLX actual extension performance/cross-engine behavior;
- PhenoLab baseline distributions/search effectiveness/usability behavior.

Known inaccessible-history residuals are explicitly bounded and do not prevent the pre-registered experiments.

## Status
Fresh review passes **for non-execution semantic finality only**, conditional on the repaired documents participating normatively.

This does not satisfy the original 100% specification/design gate because that gate requires high-risk architecture unknowns experimentally closed.
