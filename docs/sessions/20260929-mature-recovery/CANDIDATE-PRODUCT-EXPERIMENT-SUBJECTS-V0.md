# Candidate product-experiment subjects v0

Date: 2026-09-30. Candidates only; final experiment subject requires runnable environment.

## Portage EXP-P1 candidate

### Primary candidate: existing meaningful Harbor task on Apple Container ARM64 vs Docker reference
Why:
- Apple Container backend already exists and hard-requires Apple Silicon arm64;
- it uses a materially different runtime implementation;
- yet it still consumes Dockerfile/image semantics, directly testing the suspected task-materialization portability debt.

Selection requirements:
- task has verifier + artifact;
- preferably network/resource policy;
- avoid trivial hello-world;
- task already passes on Docker/reference;
- run unchanged TaskContract/environment semantics where possible.

Experiment question:
Can the same task/verifier/artifact contract run on Apple Container with only backend selection changed? If task definition must be rewritten due Docker-shaped assumptions, record exact portability debt.

### Secondary candidate
Native/bare-metal adapter for a benchmark whose environment can be represented as filesystem bundle + process/services. This more directly tests the original non-container goal but requires new adapter implementation.

Do not claim Portage value from Apple Container alone; it is a probe of Harbor's current extension seam.

## PhenoLab EXP-L1 candidate

### Primary candidate: Qwen3.5-0.8B local model + existing stock-vs-ours/performance harness
Existing repo evidence already contains:
- pinned Qwen3.5-0.8B source/revision;
- stock MLX vs Pheno Metal comparison machinery;
- 10-suite stock-vs-ours runs;
- TTFT/ITL/tokens/sec probes;
- explicit evidence that some Pheno kernel paths are not yet full end-to-end comparable;
- self-improvement gates including serving stability.

This is ideal because the baseline is nontrivial and historical failures/non-comparability already exist.

Candidate target form:
**quality non-inferiority + measurable performance/resource target**, not an artificially easy scalar.

Example structure (thresholds must be derived from baseline evidence before freeze):
- hard: no semantic quality regression beyond declared tolerance;
- objective: improve a real measured serving/runtime metric (TTFT/decode/throughput/memory);
- budget: bounded interventions/compute/time;
- interventions: kernel/runtime/quantization/speculation/harness as authorized.

A valid outcome may be target reached OR budget exhausted with negative learning.

### Avoid
Do not use the repo's synthetic/dry 5-minute matrix as the sole EXP-L1 evidence.
Do not use approximate non-streaming TTFT convention if the target is specifically TTFT and a streaming-capable path is available.

## PhenoMLX EXP-M1
Speculative target/draft qualification remains a candidate. A Qwen3.5 family target is attractive because both Pheno repos already contain pinned Qwen work, but exact target/drafter pair must be selected based on available speculative support and runnable backend, not convenience.
