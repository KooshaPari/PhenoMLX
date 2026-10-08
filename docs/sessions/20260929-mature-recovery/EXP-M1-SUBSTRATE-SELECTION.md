# EXP-M1 substrate selection v0

## Existing experimental substrate
PhenoLab already contains:
- scripts/specdec_trial.py with n-gram, EAGLE-2 and MTP trial modes;
- config/decode_acceleration_matrix.yaml as current authority for decode experiments;
- exact base/draft compatibility policy and quarantined proxy pairings;
- engine matrix spanning SGLang/vLLM/llama.cpp and broader speculative families;
- target-driven evidence/gating machinery.

Do not duplicate this orchestration inside PhenoMLX.

## Product boundary
PhenoLab: runs ExperimentProgram, budgets/interventions/evidence/learning.
PhenoMLX: implements/exposes runtime mechanism, exact realized runtime state, measurement hooks and qualification primitives.
This is a typed integration relation, not ownership.

## EXP-M1 candidate
First reuse n-gram/MTP/engine-native speculative modes to validate acceptance telemetry and prediction machinery without requiring a new trained drafter.

Then, only if exact compatible artifacts are available, evaluate model-draft pair qualification.

Candidate measurements:
acceptance rate; accepted tokens/step; draft cost; verification cost; predicted vs realized speedup; quality; TTFT/ITL/decode; memory.

## Selection gate
No target/drafter pair is frozen until the current decode matrix confirms compatibility and the required model artifacts are runnable. Proxy pairings remain invalid for product evidence.
