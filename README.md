# PhenoMLX

MLX-native inference stack with Rust performance cores and TurboQuant+ KV compression.

[![GitHub Downloads](https://img.shields.io/github/downloads/KooshaPari/PhenoMLX/total)](https://github.com/KooshaPari/PhenoMLX/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

> **Fork of** [jundot/omlx](https://github.com/jundot/omlx). Upstream OMLX remains its own project; this repository documents only the extensions maintained in this fork.

---

## What It Is

PhenoMLX extends upstream OMLX with a local research stack for multi-backend inference, policy-driven dispatch, model evaluation, and Rust performance experiments across MLX, Metal, vLLM, TensorRT, SGLang, and llama.cpp.

The core differentiator is **TurboQuant+** -- a 4-bit KV cache compression that reduces KV memory by 75% at every model scale, enabling larger context windows and more concurrent requests on memory-constrained hardware.

## Key Features

- **TurboQuant+ 4-bit KV compression** -- 75% KV cache memory reduction with no accuracy loss
- **Rust FFI performance cores** -- speculative decoding, concurrent execution, tree attention
- **Multi-backend support** -- MLX, Metal, vLLM, TensorRT, SGLang, llama.cpp
- **OpenAI-compatible API server** -- drop-in replacement for any OpenAI SDK client
- **Policy-driven inference dispatch** -- automatic backend selection based on hardware and workload
- **Model evaluation suite** -- benchmarking, comparison, and quality assessment tools

## Architecture

```text
phenoMLX
├── python/omlx_research/     Python research stack
│   ├── harbor_mlx_server.py  OpenAI-compatible MLX server
│   ├── backends/             Multi-backend inference adapters
│   └── evaluation/           Benchmark and comparison tools
├── perf-core/                Rust performance workspace
│   ├── turbo-quant           TurboQuant+ SIMD encode/decode
│   ├── speculative           Speculative decoding engine
│   └── concurrent            Concurrent execution scheduler
├── cli/                      Research CLI tools
├── pilot/                    Benchmark suite and comparison scripts
└── docs/dossiers/            Product dossiers and atlas extraction
```

## Hardware Requirements

| Model | Quantization | Min RAM | Notes |
|-------|-------------|---------|-------|
| Qwen3.5-0.8B | Q4_K_M | 4 GB | Fast, limited quality |
| Qwen3.5-8B | Q4_K_M | 8 GB | Recommended for dev |
| Qwen3.5-32B | Q4_K_M | 20 GB | Production quality |
| Qwen3.5-72B | Q4_K_M | 40 GB+ | Server-class |

TurboQuant+ reduces KV cache memory by 75% across all scales, enabling larger context windows on the same hardware.

## Quick Start

```bash
# Clone and install
git clone https://github.com/KooshaPari/PhenoMLX.git
cd PhenoMLX
pip install -e .

# Start the server
python -m omlx_research.harbor_mlx_server \
  --model mlx-community/Qwen3.5-0.8B-OptiQ-4bit \
  --port 8766

# Query it
curl http://localhost:8766/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "default", "messages": [{"role": "user", "content": "Hello"}]}'
```

## TurboQuant+ Memory Savings

4-bit KV compression reduces cache memory at every scale:

| Model | FP16 KV | 4-bit KV | Saved |
|-------|---------|----------|-------|
| 0.8B | 3.76 GB | 0.94 GB | 2.82 GB (75%) |
| 8B | 4.29 GB | 1.07 GB | 3.22 GB (75%) |
| 32B | 8.59 GB | 2.15 GB | 6.44 GB (75%) |
| 72B | 10.74 GB | 2.68 GB | 8.05 GB (75%) |
| 150B MoE | 12.88 GB | 3.22 GB | 9.66 GB (75%) |

KV bytes/token formula: `2 * full_attention_layers * num_kv_heads * head_dim * bytes_per_element`

## Benchmarks

The `pilot/` directory contains a reproducible benchmark suite:

```bash
# Run benchmark
python pilot/run_benchmark.py

# Compare against upstream OMLX
python pilot/compare_upstream.py --compare

# Evaluate results
python pilot/evaluate.py pilot/results/<run>.json
```

**Results (0.8B model, M1 Pro 16GB):**
- PhenoMLX: 20.6 t/s average, 10/10 prompts OK
- Upstream OMLX: 23.9 t/s average, 10/10 prompts OK
- Verdict: NON_INFERIOR (-13.5%, within 15% margin)

## Configuration

Key environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `PHENOTYPE_OMLX_USE_PYTHON_TQ` | `0` | Set to `1` to use Python TurboQuant instead of Rust SIMD |
| `DOC_EMBEDS_BROWSER` | - | Browser path for Remotion rendering |
| `PORT` | `8766` | Server port |

## Project Structure

```
phenotype-omlx/
├── python/omlx_research/     Python research stack
│   ├── harbor_mlx_server.py  OpenAI-compatible server
│   ├── backends/             MLX, vLLM, etc.
│   └── evaluation/           Benchmarks
├── perf-core/                Rust workspace
│   ├── turbo-quant/          TurboQuant+ codec
│   ├── speculative/          Speculative decoding
│   └── concurrent/           Concurrent scheduling
├── cli/                      Research CLI
├── pilot/                    Benchmark suite
├── scripts/                  Build and setup scripts
├── docs/                     Documentation and dossiers
└── tests/                    Test suite
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Run `cargo check --workspace` and `pytest`
5. Submit a pull request

## License

MIT License. See [LICENSE](LICENSE) for details.

## Acknowledgments

- [jundot/omlx](https://github.com/jundot/omlx) -- upstream OMLX
- [ml-explore/mlx-lm](https://github.com/ml-explore/mlx-lm) -- MLX LM framework
- [TurboQuant](https://arxiv.org/abs/2501.00021) -- KV cache compression research
