# Unsloth Studio gap ledger v1 — PhenoMLX EXP-M1

Date 2026-09-30. Current public source/release inspection.

## Strong overlap / commodity
Unsloth Studio already provides:
- persistent subprocess inference orchestration with version-sensitive worker replacement;
- CPU/CUDA/ROCm/Vulkan llama.cpp packaging paths;
- MLX inference;
- requested/applied runtime fields and fallback/refusal reasons;
- speculative modes including MTP/DSpark/ngram combinations;
- MLX KV quantization/TurboQuant vocabulary;
- tensor parallel / GPU placement;
- model load memory estimation;
- training/inference VRAM coordination;
- side-by-side model comparison/battle;
- local run/train/export and external API/server connections.

These capabilities cannot be claimed as PhenoMLX differentiation without a deeper implementation/evidence advantage.

## Important observed research gaps/opportunities

### G1 speculative-decoding *measurement*
An open September 2026 Unsloth issue asks for target/draft speculative acceptance-rate measurement; current serving can use drafters but lacks a direct pre-serving measure of whether the draft is worthwhile.

Potential PhenoMLX research opportunity: acceptance-rate / expected-speedup qualification and automated drafter selection.

### G2 experiment provenance/history
Studio models detailed active runtime state, but current inspection has not established an append-only research history equivalent to PhenoMLX QualificationRecord/Support history across arbitrary experiments.

Need falsification: search Studio DB/session/history schemas before claiming gap.

### G3 arbitrary research extension harness
Studio contains many optimized implementations, but no general mechanism has yet been established for declaring an arbitrary runtime hypothesis, factor matrix, oracle and custom extension then grading it against stock baselines.

This is a candidate PhenoMLX differentiator, not yet proven absent.

### G4 low-level cross-engine mechanism comparability
Studio exposes multiple backends, but a common research contract for comparing the *same mechanism* across MLX/CUDA/ROCm/Vulkan engines has not yet been established.

### G5 negative-result retention
No evidence yet that failed runtime hypotheses are first-class searchable research artifacts rather than logs/issues.

## Architecture lessons to reuse
- subprocess isolation for incompatible transformer/runtime versions;
- explicit requested/applied/fallback state;
- fail-closed explicit quantization requests;
- capability freshness/retry semantics;
- VRAM coordination across training/inference;
- backend-specific settings rather than false lowest-common-denominator abstraction.

## Licensing
Studio backend/UI source inspected here is AGPL-3.0-only. Learn from architecture freely, but direct code reuse/integration has materially different licensing implications than Apache-2.0 core components.

## EXP-M1 candidate
Speculative drafter qualification is now a strong first experiment:
1. exact target/draft/model/workload;
2. measure acceptance probability / expected speedup before serving;
3. compare predicted vs realized speedup/quality;
4. compare against Unsloth/llama.cpp existing speculative behavior;
5. retain negative drafter pairs;
6. determine whether automated selection provides value beyond Studio controls.

This is only one candidate; do not lock the product to speculative decoding if another mechanism has stronger evidence.
