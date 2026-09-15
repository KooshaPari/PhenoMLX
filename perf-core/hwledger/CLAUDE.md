# CLAUDE.md — hwLedger

LLM capacity planner + fleet ledger + desktop inference runtime.

## Stack

| Layer | Technology |
|-------|------------|
| Core | Rust workspace (`hwledger-core`, `-arch`, `-ingest`, `-probe`, `-inference`, `-ledger`, `-fleet-proto`, `-agent`, `-server`, `-cli`, `-ffi`) |
| Sidecar | `sidecars/omlx-fork/` — fat fork of [jundot/omlx](https://github.com/jundot/omlx), Apache-2.0 |
| Native GUIs | SwiftUI (macOS), WinUI 3 (.NET 9), Qt 6 + Slint (Linux) |
| Fleet wire | Axum, rustls mTLS, russh, deadpool, reqwest, tailscale |
| Web fallback | Streamlit (`apps/streamlit/`) |
| License | Apache-2.0 |

## Dev Commands

```bash
# CLI (fastest path)
cargo install --path crates/hwledger-cli
hwledger --help

# FFI + server + streamlit (one-liner)
cargo run -p hwledger-devtools -- up

# Build workspace
cargo build --workspace

# Test
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings
cargo fmt
```

## Architecture Notes

- **Math core** is architecture-keyed: dispatches per `AttentionKind` (MHA/GQA/MQA/MLA/Sliding/SSM/Hybrid/Sink) and tracks resident-vs-active parameters for MoE.
- **Inference sidecar**: forked oMlx with SSD-paged KV cache for Apple Silicon.
- **Fleet ledger**: event-sourced audit log over heterogeneous compute (Apple Silicon, NVIDIA/AMD, cloud rentals).
- **FFI core**: shared Rust library consumed by SwiftUI/WinUI/Qt native GUIs via UniFFI/cxx-qt.

## Quality Standards

- Clippy zero warnings (`-D warnings`)
- `cargo fmt` before commit
- Tests must exist for new features; failing test required before bug fix
- Max function length: 40 lines (Rust core)
- No placeholder TODOs in committed code

## Governance

- Tracked in AgilePlus: feature `hwledger-v1-macos-mvp`
- Reference: `/Users/kooshapari/CodeProjects/Phenotype/repos/AgilePlus`
- Key docs: `PLAN.md`, `ADR.md`, `PRD.md`, `CHARTER.md`, `docs/adr/`, `docs/research/`

## Branch Discipline

Feature work goes in `.worktrees/<topic>/`. Canonical repo stays on `main`.

## Homebrew Formula

Updates via `Formula/omlx.rb` (see phenotype-omlx sibling repo for formula structure).
