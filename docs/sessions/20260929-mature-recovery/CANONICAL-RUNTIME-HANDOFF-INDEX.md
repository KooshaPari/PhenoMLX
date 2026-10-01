# Canonical runtime handoff index v1

Date: 2026-10-01.

Runtime/code agents should start here. Do **not** reconstruct product intent by reading session files chronologically.

## Authority order
1. direct latest user-intent corrections;
2. PROVISIONAL-MATURE-BASELINE-VNEXT.md;
3. VNEXT-CAPABILITY-DELTA-V0.md;
4. fresh-review semantic repair docs + v1 schemas;
5. INFRA-VS-PRODUCT-GATE-V0.md;
6. product experiment schemas/traces;
7. bounded developer work units;
8. NONEXEC-FINAL-STATUS-V1.md.

Earlier files are archaeology/evidence unless explicitly referenced by the above.

## Do not regress
- Portage: original portability thesis; Harbor already has BaseEnvironment and backend-specific env definitions; product question is portable materialization/parity, not "replace Harbor".
- PhenoMLX: experimental runtime/research studio; profiles are infrastructure; use native engine metrics/primitives before custom; Unsloth is a primary alternative.
- PhenoLab: target-driven LLM R&D; generated kernels/patches are experiment artifacts; Tracera has no product-authority relationship.

## Execution entry
Read trace/execution-gate-state.json and the relevant DEVELOPER-WORK-UNIT-* document.
Emit the required machine receipt.
Never promote work-unit completion into product validation.
On contradiction, stop and reopen spec rather than choosing an older session document.

## Spec reopen triggers
runtime falsification; new authoritative intent; material SOTA change; newly recovered contradictory history; concrete fresh-review counterexample.
