# Corrected next experiments v0

Date: 2026-09-30.

## Portage — EXP-P1 portability falsification
Goal: determine whether a backend-neutral Portage execution layer is still justified.

Select one benchmark task with verifier/artifacts/network behavior. Execute semantically equivalent task on:
1. current Harbor Docker/reference path;
2. one materially different substrate: ARM64, native/bare-metal, Podman/OCI alternative, Apple Container or provider path that does not merely hide the same Docker assumptions.

Record primitive/capability mapping, task changes required, evidence equivalence, failures, performance/operating burden.

Hard question: can the same TaskContract remain stable while only backend adapter changes?

If no, identify whether Harbor task semantics themselves are Docker-shaped.

## PhenoMLX — EXP-M1 novel mechanism research loop
Goal: prove PhenoMLX adds research capability beyond Unsloth Studio + mature engines.

Choose one mechanism with a real hypothesis, preferably speculative decoding / KV quantization / cache/state optimization.

Compare:
- best current Unsloth/engine-native implementation;
- PhenoMLX experiment/extension.

Require exact model/tokenizer/hardware/workload, quality gate, runtime state, perf/resource evidence, negative cases and reproducibility.

A dashboard/profile wrapper alone cannot pass.

## PhenoLab — EXP-L1 target-reaching program
Goal: validate the lab product thesis end-to-end.

Declare one ModelSystem + Harness/Workload + ObjectiveTarget + hard constraints + budget.

Run at least multiple distinct InterventionSets using replaceable workers/optimizers. Preserve failed attempts and learning. Independently validate candidate evidence. Kill/replace a worker during the program.

Pass if:
- target is reached with qualifying evidence, OR
- budget/search terminates with a defensible failure record and reusable negative learning;
AND durable state/evidence/decision survives replacement.

This is different from requiring a candidate promotion: trustworthy failed R&D is a valid outcome.

## Relationship to prior VS-01
Prior VS-01 packages are renamed conceptually INFRA-01:
- Portage evidence envelope;
- PhenoMLX profile/evidence store;
- PhenoLab typed experiment/evidence/decision durability.

They support these experiments but do not themselves validate product existence.
