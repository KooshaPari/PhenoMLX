# PhenoMLX

**MLX inference engine: Rust performance cores, multi-backend routing, evaluation tooling**

[![Python](https://img.shields.io/badge/python-3.11+-blue.svg)](https://www.python.org/downloads/)
[![Rust](https://img.shields.io/badge/rust-1.82+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![GitHub Downloads](https://img.shields.io/github/downloads/KooshaPari/PhenoMLX/total.svg)](https://github.com/KooshaPari/PhenoMLX/releases)

---

## What this repo does

**PhenoMLX** (phenotype-omlx) is a fork of the upstream [jundot/omlx](https://github.com/jundot/omlx) project, extended with MLX-native performance cores and the Phenotype ecosystem tooling. It provides:

- **Rust performance cores** — Speculative decoding, concurrent execution, TurboQuant quantization, tree attention, fleet protocol
- **Python FFI & research launchers** — `omlx-research` unified CLI bridging MLX framework, omlx CLI, and oMLX GUI/web admin
- **Multi-backend routing** — Automatic backend selection across MLX, llama.cpp, and other inference engines
- **Evaluation tooling** — Bench-cockpit (Go/TypeScript) for benchmarking, evals, and calibration
- **70+ Rust crates** — `crates/` (52 workspace crates: config, telemetry, MCP SDK, spec-driven development, CI templates) + `perf-core/` (18+ performance cores: TurboQuant, spec-decode, tree-attention, fleet-proto, metal-runtime, concurrent-exec, and more)
- **Metal runtime bundling** — Build scripts for Metal/MPS kernels on Apple Silicon

---

## Quick start

```bash
# Verify the full stack is wired (12 component checks)
./scripts/phenotype-omlx-ready

# Diagnose the research stack (Python env, MLX, omlx CLI, GUI)
./cli/bin/omlx-research doctor

# Run inference with automatic backend selection
./cli/bin/omlx-research inference --prompt "Hello" --policy auto

# Start interactive Python REPL with perf-core modules loaded
./cli/bin/omlx-research

# Launch oMLX GUI with admin extensions
./cli/bin/omlx-research gui

# Start local web admin on port 8080
./cli/bin/omlx-research web 8080
```

> **Requires:** oMLX.app installed at `/Applications/oMLX.app` (macOS), Python 3.11+, Rust 1.82+

---

## Structure

```
phenotype-omlx/
├── apps/
│   └── bench-cockpit/       # Go + TypeScript benchmarking dashboard
│       ├── server/          # Go backend: evals, capacity, RLVR
│       └── src/             # React/TypeScript frontend components
├── cli/
│   └── bin/
│       ├── omlx-research    # Unified launcher (bash)
│       └── omlx-cli         # Proxy to system omlx with perf-core env
├── crates/                  # 52 Rust crates (workspace)
│   ├── agileplus-*          # Spec-driven dev: config, events, graph, MCP, etc.
│   ├── pheno-*              # Feature flags, CI templates, SSOT templates
│   └── phenotype-*          # Config guard, dep guard, MCP SDK, sandbox
├── scripts/
│   ├── phenotype-omlx-ready          # Stack readiness check
│   ├── build_metal_runtime_bundle.sh # Metal kernel bundling
│   ├── build_moe_metallibs.sh        # MoE Metal libs
│   ├── cross_repo_smoke.sh           # Cross-repo integration smoke
│   ├── niah_benchmark.py             # Needle-in-haystack benchmark
│   ├── perf_turboquant.py            # TurboQuant performance
│   └── ...                           # Evaluation, profiling, deployment scripts
└── turboquant_plus/                 # Python venv for research stack
```

---

## Crates

| Crate | Purpose |
|-------|---------|
| **pheno-flags** | Typed feature-flag resolver (env → .env → default) |
| **phenotype-mcp-sdk-rs** | Rust MCP SDK: server trait, tools, resources, stdio/SSE transports |
| **phenotype-config** | Configuration management for Phenotype services |
| **phenotype-dep-guard** | Dependency guard for build-time validation |
| **phenotype-sandbox** | Sandbox isolation for untrusted code execution |
| **shared-traceability** | Shared traceability types across crates |
| **traceability-core** | Core traceability engine |
| **clap-ext** | Extended Clap derive macros and helpers |
| **agileplus-benchmarks** | Criterion benchmarks for all AgilePlus subsystems |
| **agileplus-cli** | CLI framework for AgilePlus tools |
| **agileplus-config** | Configuration primitives for AgilePlus |
| **agileplus-events** | Event sourcing infrastructure |
| **agileplus-graph** | Graph data structures and algorithms |
| **agileplus-mcp-intent** | MCP intent classification and routing |
| **agileplus-telemetry** | OpenTelemetry integration |
| **agileplus-sqlite** | SQLite persistence layer |
| **agileplus-api / agileplus-api-types** | API layer and shared types |
| **agileplus-validate / agileplus-trace-validator** | Validation and trace validation |
| **agileplus-factory / agileplus-plugin-core** | Factory pattern and plugin core |
| **agileplus-pipeline / agileplus-proto** | Pipeline orchestration and protobuf |
| **agileplus-git / agileplus-github** | Git operations and GitHub API |
| **agileplus-dashboard / agileplus-application** | Dashboard and application scaffolding |
| **agileplus-grpc / agileplus-nats / agileplus-p2p** | Transport layers |
| **agileplus-witness / agileplus-triage** | Witness recording and triage |
| **agileplus-governance / agileplus-spec-harmonizer** | Governance and spec harmonization |
| **agileplus-cache / agileplus-convoy / agileplus-hook** | Caching, convoy, hooks |
| **pheno-ci-templates / pheno-ssot-template / pheno-vibecoding-guard** | CI templates, SSOT, vibecoding guard |

*All crates use workspace version, edition, and license.*

---

## Development

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run benchmarks
cargo bench --workspace -p agileplus-benchmarks

# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Build Metal runtime bundle
./scripts/build_metal_runtime_bundle.sh

# Run cross-repo smoke test
./scripts/cross_repo_smoke.sh
```

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:

- Code style (Rust: `cargo fmt` / `clippy`; TypeScript: ESLint/Prettier)
- Commit conventions (Conventional Commits + ledger metadata)
- PR process and review requirements
- Running the full verification suite

---

## License

MIT License — see [LICENSE](LICENSE) for details.

---

## Links

- **Upstream:** [jundot/omlx](https://github.com/jundot/omlx)
- **Fork:** [KooshaPari/PhenoMLX](https://github.com/KooshaPari/PhenoMLX)
- **Phenotype ecosystem:** [phenotype.dev](https://phenotype.dev)