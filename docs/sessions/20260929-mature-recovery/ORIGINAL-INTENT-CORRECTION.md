# Original fork/repository intent — authoritative recovery correction

Date: 2026-09-30.
Authority: direct user statement in recovery session.

This document separates **ORIGINAL USER INTENT** from later agent-added scope and current mature differentiation.

## Portage — why the Harbor fork existed

### Original user intent
Harbor was initially too constrained for the desired execution environment:
- effectively Docker-oriented;
- x86-oriented;
- wanted ARM64;
- wanted bare-metal execution;
- wanted trivial interchange among alternative containerization/sandboxing/runtime libraries.

Then scope expanded through user-requested changes:
- additional feature requests/enhancements as friction appeared;
- integrations/compatibility against Harbor competitors;
- incorporate/learn from useful changes in peer DeepSea's Harbor fork.

### Agent-added scope
Agents were explicitly given broad freedom to add enhancements and optimizations. Therefore large parts of the fork may reflect autonomous expansion rather than core user intent.

### Recovery consequence
Portage's mature contract MUST evaluate at least two theses, not only the recent thin-evidence thesis:
A. **portable Harbor superset/compatibility fork** — execution portability + architecture/backend abstraction + competitor/fork integration;
B. **thin compatibility/evidence layer over modern Harbor** — if upstream Harbor now solves most original portability gaps.

The existence gate must determine which survives in 2026. It is invalid to prune portability/backend work merely because modern Harbor later gained alternatives.

Original intent and current differentiation are separate axes.

## PhenoMLX — why the MLX rewrite/fork existed

### Original user intent
The initial goal was substantially more ambitious than a typed profile layer:
- rewrite/extend MLX-oriented LLM runtime for our experimentation needs;
- optimize for extensive experimentation and dashboards;
- add/hand-roll LLM runtime features not conveniently available in the desired combination, including extended speculative-decoding methods, multiple quantization forms and related inference/runtime experimentation;
- integrate those capabilities across different options/backends.

Then cross-platform ambition became central:
- Windows and Linux in addition to Apple/macOS;
- CUDA;
- Vulkan;
- ROCm;
- other protocols/platforms where useful.

Unsloth Studio emerged later and is especially relevant prior art/alternative because it now overlaps cross-platform local LLM studio/runtime goals.

### Recovery consequence
The mature existence gate must test multiple product forms:
A. experimental MLX/runtime rewrite with novel kernels/features;
B. cross-platform experimental LLM runtime/studio;
C. typed qualification/control layer over existing engines;
D. combination where novel runtime work exists only for experimentally justified gaps.

Do not collapse the original rewrite goal into C before testing A/B/D against current SOTA.

## PhenoLab / PhenoLM — why the repo existed

### Original user intent
PhenoLab began effectively as a scratch/R&D repository where many agents could generate kernels and other experimental components.

The governing outcome was:
**given a target model + harness/workload, experiment until it reaches a declared target such as speed, accuracy, or another measurable objective.**

The repo is therefore fundamentally:
- LLM-oriented experimentation/R&D;
- a place for aggressive generated experiments/kernels/optimizations;
- target-driven rather than product-feature-driven;
- allowed to contain failed/temporary experiments.

### Recovery consequence
Do not over-productize the scratch repo by treating every historical subsystem as mature product surface.

The mature product, if any, is the **experiment/control/evidence system that lets generated work reliably move a model+harness toward a target**. Kernels/optimizers/runtime modifications are experiment outputs/components, not necessarily permanent product capabilities.

This also reinforces the correction that Tracera has no product relationship to PhenoLab.

## Shared recovery rule

For all three repositories maintain separate columns:
1. ORIGINAL USER INTENT;
2. LATER USER-REQUESTED EXPANSION;
3. AGENT-AUTONOMOUS EXPANSION;
4. CURRENT IMPLEMENTATION;
5. CURRENT SOTA/COMMODITY;
6. SURVIVING 2026 DIFFERENTIATION.

A mature-first recovery may conclude the best 2026 product is narrower/different than the original fork, but it may not rewrite history to make the surviving differentiation look like the original intent.
