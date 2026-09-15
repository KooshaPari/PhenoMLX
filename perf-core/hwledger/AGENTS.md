# hwLedger — AGENTS.md

> **Bucket (per ADR-035A, L5-105, 2026-06-18):** CONDITIONAL — federated service.
> Local `hwLedger-2nd` worktree preserved; app-level work proceeds; lib extraction is a separate track.
> See `docs/adr/2026-06-18/ADR-035A-hwledger-reclassification.md` and `docs/integrations/pheno-capacity.md`.

## Project Overview

LLM capacity planner + fleet ledger + desktop inference runtime. Pre-alpha Phase 0; Rust core + per-OS native GUIs.

## Stack

- Language: Rust (per GitHub language detection)
- Platform: Desktop / Embedded
- Build system: Cargo (verify `Cargo.toml`)

## Key Commands

```bash
# Verify project structure
ls -la Cargo.toml Cargo.lock rust-toolchain.toml 2>/dev/null

# Build
cargo build --release

# Test
cargo test

# Lint
cargo clippy
```

## Substrate dependencies (per ADR-023 + ADR-035A)

| Substrate | Purpose | Integration status |
| :-- | :-- | :-- |
| [`KooshaPari/pheno-capacity`](https://github.com/KooshaPari/pheno-capacity) | Pure math: VRAM estimation, model-fit scoring, Chinchilla tokens, optimizer state. no_std-compatible. | **Active (Phase 1, this turn).** HwLedger Streamlit Planner/WhatIf pages will consume this crate in Phase 2 (replaces historical `apps/streamlit/lib/cost_model.py`). |
| `phenotype-config` (planned) | App config | Not yet wired. |
| `pheno-tracing` (planned) | OTLP export | Not yet wired. |

## Notes

- **Bucket:** CONDITIONAL (per ADR-035A). Federated service; lib extraction is separate.
- App-level work proceeds; lib extraction is a separate ADR-035A deliverable.
- See `docs/integrations/pheno-capacity.md` for the integration contract.
- See `docs/integrations/cost-model-migration.md` for the Phase 2 migration playbook (Streamlit → pheno-capacity consumer).
- **Phase 2 blocker:** Streamlit is a Python runtime; the `pheno-capacity` crate is Rust. Bridge options being evaluated: (a) `pyo3`/`maturin` Python bindings, (b) re-implement consumer in Rust and call from Python via PyO3, (c) keep Python consumer, re-implement math in pure Python `pheno_capacity` pip package. Recommendation deferred to Phase 2 kickoff.
