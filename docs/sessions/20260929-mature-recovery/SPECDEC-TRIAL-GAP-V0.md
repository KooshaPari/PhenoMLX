# Existing specdec_trial vs EXP-M1 measurement gap v0

Date: 2026-10-01.

## Existing runner strengths
scripts/specdec_trial.py already:
- has baseline + n-gram + EAGLE-2 + MTP named trials;
- records engine/model/n;
- records elapsed p50 and decode tok/s p50;
- returns explicit errors;
- can reuse current serving endpoints.

## Critical limitations
1. n-gram trial explicitly says it is **not a true acceptance-ratio measurement**; it changes the prompt with hints and infers uplift.
2. EAGLE trial does not verify EAGLE is actually active; it assumes engine startup config.
3. MTP trial does not verify MTP is active; +2 max tokens is not activation evidence.
4. no exact engine version/build/effective config digest;
5. no target/draft artifact identity;
6. no tokenizer/workload population digest;
7. no actual accepted/rejected draft-token telemetry;
8. no draft cost vs target verification cost;
9. no TTFT/ITL distribution;
10. no quality oracle;
11. no memory/resource evidence;
12. no predicted-vs-realized speedup model;
13. unsupported method may look like ordinary zero uplift rather than structured unsupported;
14. single seed prompt default is not representative research population.

## Reuse decision
KEEP the runner as a thin orchestration/legacy probe, but EXP-M1 needs an instrumentation adapter from PhenoMLX/engine telemetry.

Do not rewrite PhenoLab orchestration. Add a richer TrialReceipt produced by the runtime/instrumentation layer and consumed by PhenoLab.

## First safe experiment
Current decode matrix contains a measured local control:
qwen35_ngram_simple_3090 — exact outputs equal, baseline 106.856896 tok/s, ngram 103.753880 tok/s, status negative_control.

This is valuable as a regression/negative fixture, not evidence of useful speculation.

The matrix also contains exact-pair review candidates, but execution is gated. Do not bypass those activation requirements.

## Required EXP-M1 TrialReceipt additions
method_active proof; requested/applied method; engine/build/config; target/draft digests; workload digest; per-request accepted/proposed token counts; draft/verify timing; quality; resource metrics; structured unsupported/fallback; raw evidence refs.
