# Research correction — Unsloth Studio and original-runtime thesis

Date: 2026-09-30.

Current external evidence makes Unsloth Studio a high-priority PhenoMLX alternative/prior-art target.

## Current overlap
Official Unsloth project materials describe Studio as cross-platform across Windows, Linux/WSL and macOS, with:
- NVIDIA support;
- AMD support including ROCm paths;
- Vulkan GGUF inference including Intel-compatible GPUs;
- Apple MLX inference;
- multi-GPU;
- local model search/run/train/export;
- API/deployment;
- recent MLX and quantized KV-cache improvements.

This overlaps the later PhenoMLX ambition much more directly than the earlier engine-only competitor matrix captured.

## Required new SOTA pass
Inspect Unsloth Studio architecture for:
- backend selection/packaging and platform abstraction;
- Windows native vs WSL boundaries;
- CUDA/ROCm/Vulkan/MLX/llama.cpp integration;
- model artifact/quantization abstraction;
- inference/training separation;
- dashboard/experiment UX;
- benchmark/evidence/profile persistence;
- speculative decoding;
- KV-cache quantization/offload;
- extension/plugin model;
- update/installer strategy;
- licensing and reusable components.

Then classify each PhenoMLX mature capability USE / INTEGRATE / FORK / ADAPT / LEARN / REJECT / CUSTOM.

## Existence attack
If Unsloth Studio + mature engines already provide cross-platform runtime/studio breadth, PhenoMLX must justify itself through one or more of:
- deeper experimental control/evidence;
- novel speculative decoding/quantization/kernel work;
- reproducible cross-engine qualification;
- features not exposed/composable in Unsloth;
- a substantially better research workflow.

Cross-platform breadth alone is no longer sufficient differentiation.
