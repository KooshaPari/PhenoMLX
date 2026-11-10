# Speculative telemetry hook map v0

Goal: produce Speculative TrialReceipt from engine/runtime facts, not client-side inference.

## Required canonical signals
- method requested/applied;
- activation/config proof;
- proposed draft tokens;
- accepted draft tokens;
- acceptance rate / accepted tokens per step;
- draft time;
- target verification time;
- TTFT/ITL/decode throughput;
- memory/resource state;
- fallback/unsupported reason.

## Engine integration strategy

### vLLM
Prefer native request/engine metrics/logging or instrument speculative decode worker/model-runner internals at a version-pinned seam. Do not infer acceptance solely from output speed. Existing ecosystem exposes speculative configuration and implementation-specific metrics vary by version; adapter must declare collector/version.

### SGLang
Prefer scheduler/speculative algorithm telemetry at server runtime; current decode matrix already treats SGLang as a first-class DFlash/DSpark lane. Instrument exact method-specific counters where available, otherwise mark fields unknown.

### llama.cpp
Use server/runtime speculative counters if exposed by selected build; otherwise patch/instrument the draft/verify loop behind a build digest. Client output alone is insufficient.

### MLX/custom Pheno runtime
This is the easiest place to make the receipt first-class: proposal/verification loop should emit counters/timers directly with exact generation/config identity.

## Truth rule
If an engine does not expose a field and no pinned instrumentation exists, value = null/unknown. Never reconstruct "accepted tokens" from tok/s uplift.

## Activation proof
One of:
- engine structured runtime state/config endpoint;
- startup config + method-specific runtime counter > 0;
- instrumented code-path receipt tied to build digest.

Startup flags alone are insufficient when fallback is possible.

## First implementation order
1. custom/MLX path where direct counters can be added;
2. one engine with accessible speculative telemetry;
3. second engine for cross-engine comparability;
4. only then broad adapters.

PhenoLab consumes normalized TrialReceipt; engine-specific collection belongs in PhenoMLX/runtime adapters.
