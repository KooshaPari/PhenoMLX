# Unsloth Studio architecture comparison v0 — PhenoMLX existence gate

Date: 2026-09-30.

## Why it matters
Unsloth Studio now overlaps substantial later PhenoMLX intent, not merely UI:
- native Windows installer plus Linux/WSL/macOS;
- selectable llama.cpp CPU/CUDA/ROCm/Vulkan backends;
- Apple MLX;
- AMD/NVIDIA paths;
- multi-GPU;
- model load/run/train/export/deployment UI;
- explicit requested-vs-applied runtime fields;
- MLX KV-cache quantization including TurboQuant vocabulary;
- speculative decoding modes including MTP/DSpark/ngram combinations;
- GPU memory strategy and fallback reasons.

## Architecture observations from current source
Studio's inference state already records several concepts independently rediscovered in the PhenoMLX recovery:
- requested vs actually applied MLX KV quantization;
- reason when request cannot be honored;
- requested speculative mode;
- backend identity (MLX/NPU etc.);
- GPU placement/tensor parallel state;
- chat-template override + refusal reason;
- CPU/Vulkan fallback reason;
- context/backend-specific settings.

This is strong evidence that typed requested-vs-realized runtime state is commodity/necessary infrastructure, not sufficient PhenoMLX differentiation.

## Bootstrap decisions

### USE/INTEGRATE/LEARN
- cross-platform installation/backend selection patterns;
- llama.cpp backend packaging for CUDA/ROCm/Vulkan/CPU;
- MLX integration;
- explicit requested/applied/fallback runtime state;
- memory/load configuration UX;
- model-management and studio workflow patterns.

### DIRECT COMPETITION / EXISTENCE ATTACK
- cross-platform local LLM studio;
- basic runtime configuration dashboard;
- common quantization/KV controls;
- ordinary speculative decoding exposure.

### Candidate PhenoMLX differentiation requiring evidence
- research-grade experiment matrix across engines/features;
- deeper novel speculative decoding variants not already exposed;
- novel quantization/kernel work with qualified gains;
- exact reproducible RuntimeProfile/Qualification history across backends;
- automated falsification/comparison/oracle system;
- low-level MLX/runtime research that Unsloth delegates to existing backends;
- richer measurement of state/KV/cache/scheduler internals.

## Important correction
The typed profile work remains valuable, but it is now partly **table stakes**: Unsloth itself tracks requested/applied/fallback state. PhenoMLX must go beyond merely having those fields.

## Next source inspection
Need deeper Studio backend boundaries, persistence/experiment history, training/inference separation, benchmark tooling and extension/plugin model before final existence decision.
