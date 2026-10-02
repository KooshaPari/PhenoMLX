# PhenoMLX — first findings

Source `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`, 2026-09-29. These observations do not freeze the architecture.

## Identity and recovered lineage

Current identity: PhenoMLX, ID 1214745478. README explicitly associates `phenotype-omlx`; current launcher/package terms include `omlx-research`, `omlx_research`, `TurboQuant+`, `turboquant_plus` and `perf-core`. These are search terms with different roles, not interchangeable repository identities. AGENTS cites a canonical cutover from temporary `phenotype-omlx-tmp` / `phenotype-omlx-temp` remotes and warns against archived clones. The current account also exposes a distinct archived PhenoMLX-temp candidate; exact historical relationship has not been proved.

The registry's retained/unverified record is not proof of deletion. The source revision proves current access. Prior conversation retrieval did not return sufficient direct design material to certify the mature product horizon. Repo policy nevertheless expressly preserves multi-engine and polyglot scope; reducing this to an Apple-only server from its name would be unjustified.

## M-F01 — web argument construction

`cli/bin/omlx-research`, blob `c3d78cf445369ea1de4c2173c2905018a1f5ef9e`, selects `cmd` from `$1`. In its `web)` branch it reads `port` from the still-unshifted `$1`, then shifts once. The documented `web 8080` invocation therefore constructs `--port web 8080` in a direct branch replay. `web` alone constructs `--port web`.

Offline Bash characterization reproduced both argv sequences. The replay replaced exec with a local printing function and did not source the real environment or run the web server. The argument defect is strongly localized; clean-install/native path verification remains open. Do not relabel a doctor/import check as a closed web-admin journey.

## M-F02 — negative cache result must remain visible

The latest commit adds a measurement report to `perf-core/turbo-quant-cuda/tq_block_cache.py::byte_breakdown` (blob `5ce3080e5329b2ab56bddc8d0595b1c2d04df456`). It records Qwen3.5-9B, bf16 weights, eight full-attention layers, transformers 5.17, torch 2.9.1, RTX 3090 Ti, block 32, four-bit packing. Across contexts 1024/2048/4096/8192, decoded KV remains equal to the entire bf16 reference while packed state adds 0.375 of that reference. Total is 1.375 times baseline, not a saving. Reported decode ratio is 0.92–1.03 times baseline; reported worst relative loss difference over sixteen 256-token chunks is 0.0164 against a 0.05 limit.

Classification: **source-recorded measurement, not independently reproduced evidence**. Raw data, exact run IDs, input dataset and evaluator artifacts have not been inspected. This directly challenges memory/speed claims for this implementation/profile. It does not falsify every quantizer, a future fused design, other models, the MLX/Metal path or PhenoMLX's whole existence.

Recent history correctly changes the applicability of old Qwen2.5 conclusions. Commit `025142a986e2c3ca2c909d2e9a7b2fd4e9d1f614` records an operator restriction against Qwen3, Qwen2.5 and older evaluation. This pass runs no models and does not revive those historical benchmarks. Exact current test policy still requires authority reconciliation before native execution.

## M-F03 — fork advantage and deployment identity are unresolved

Current upstream oMLX documents continuous batching, tiered KV caching, model management, an app/CLI and an admin benchmark surface. MLX-LM documents generation, streaming, fine-tuning, prompt caching and configurable prefill/cache tradeoffs. Those are realistic bootstrap candidates, not evidence that the owned fork's advertised capabilities are unique.

The local README and launcher depend on an externally installed oMLX application and sourced environment. Determine whether the supported product is a focused extension, independently packaged runtime, or both with named profiles. Do not silently substitute a host's installed upstream package for the evaluated candidate.

## M-F04 — licensing provenance gap

Local LICENSE is MIT; current upstream oMLX identifies Apache-2.0. This is a notice/lineage audit trigger, not proof of a violation. Establish the actual inherited source revision, preserved notices, modifications and distribution boundaries before claiming a qualified distributable fork.

## M-F05 — boundaries and platform claims need separate profiles

Custom CUDA cache work, Apple Metal kernels, cross-engine plugins and bench-cockpit are not one interchangeable evidence subject. Proposed division: PhenoMLX owns runtime implementations and truthful support/performance envelopes; PhenoLab owns comparative experiment decisions; Portage owns agent-task execution where consumed. Retain specialized microbenchmarks until consumer-compatible ownership is established. No code relocation follows from this proposal.

## Characterization receipt

Observation `2026-09-29T18:33:38.057726+00:00`; combined offline probe script SHA-256 `61d16edb579d60396419deab2ada70c59412aa1c531bef9a313b32c897b1f71d`; Linux x86_64 / Python 3.13.5 invoking Bash. Cases `PhenoMLX/argv-web` and `PhenoMLX/argv-web-8080`. Raw script/results are retained in the authenticated PhenoLab companion session `probes/`. Native application, environment sourcing, model inference and product acceptance: NOT_RUN/NOT_EVALUATED.

External primary reads on 2026-09-29: https://github.com/jundot/omlx ; https://github.com/ml-explore/mlx-lm ; https://github.com/ggml-org/llama.cpp . Web documentation is not a pinned installed artifact. Full SOTA mechanisms, project-health and licensing passes remain open.
