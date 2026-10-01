# SOTA closure — vLLM speculative telemetry

Date: 2026-10-01.

Current vLLM documentation exposes experimental per-request speculative metrics when enabled:
- mean acceptance length;
- draft acceptance rate;
- acceptance histogram;
- speculative steps;
- accepted draft tokens;
- proposed draft tokens;
- configured speculative token count;
and aggregate Prometheus counters.

## Bootstrap decision
For vLLM: **USE/ADAPT native metrics**, version-pinned. Do not custom-patch acceptance counters first.

PhenoMLX TrialReceipt adapter should:
- enable summary/detailed per-request metrics where supported;
- capture exact vLLM version;
- map native fields losslessly;
- reconcile per-request sums with aggregate counters for controlled n=1 workloads;
- mark schema/version experimental;
- keep unavailable draft/verify timing fields null unless another native source provides them.

## Differentiation consequence
"Measure speculative acceptance" by itself is no longer PhenoMLX differentiation on current vLLM. Candidate differentiation moves to experiment design/prediction, cross-engine normalization, richer runtime research and mechanisms not already instrumented.

This supersedes any prior claim that vLLM necessarily requires custom instrumentation for accepted/proposed counts.
