# Candidate baseline readiness v0

Date 2026-09-30. Target v0.2.

## Contract coverage
M-OB-001..020 are represented conceptually by v0.2 after source-of-truth boundary revision.

## Source-family status
- intent/conversations: PARTIAL-HIGH, direct multi-engine intent recovered.
- formal docs: PARTIAL; contaminated phenotype-shared PRD excluded.
- source/backends/runtime: HIGH for adapter/perf-core inventory, lower for end-to-end request lifecycle.
- UI/cockpit: PARTIAL-HIGH; capacity/eval projections inspected.
- evidence/benchmarks: PARTIAL-HIGH; provenance envelopes and negative cache evidence inspected, raw reproduction open.
- tests: PARTIAL; many contract tests exist but mature-profile oracle not native-run.
- CI/deployment/release/security: OPEN/PARTIAL.
- history: PARTIAL-HIGH; accessible refs classified, unavailable zz-archive uncertainty retained.
- SOTA: HIGH first pass across MLX-LM/vLLM/SGLang/TRT-LLM/llama.cpp; exact versions/licenses/health incomplete.
- consumers/source-of-truth: PARTIAL; hwledger/routing/Portage boundaries proposed, not validated.

## Baseline blockers
B1 real two-engine typed-profile experiment.
B2 exact profile/support schema.
B3 clean install and lifecycle qualification.
B4 hwledger/routing consumer boundary validation.
B5 exact engine version/model/license/project-health matrix.
B6 evidence-store/generalized qualification receipt.
B7 native lifecycle/admission/partial-stream oracles.

Decision: **NOT READY FOR BASELINE PROMOTION.**
