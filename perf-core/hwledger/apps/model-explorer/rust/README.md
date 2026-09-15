# `apps/model-explorer/rust` — HwLedger Model Explorer (Rust workspace)

A Cargo workspace housing the Rust side of the HwLedger model-explorer search
stack: a tantivy-backed index, a Hugging Face ingest pipeline, a tiny RAG
shim, and three front-end binaries (CLI, HTTP server, MCP stdio server). The
workspace is deliberately split into 11 small crates so each concern — facets,
indexing, ingestion, RAG, evals, agentic skills, transport — can evolve and
be tested in isolation.

## Workspace layout

The workspace is declared in `Cargo.toml:3-14` and rooted at this directory.
All crates share `Cargo.toml:16-21` workspace metadata (`version = "0.1.0"`,
`edition = "2021"`, `license = "Apache-2.0"`, `publish = false`) and pull
shared dependencies from `[workspace.dependencies]` (`Cargo.toml:23-58`).

The 11 crates, grouped by role:

**Front-end binaries (entry points)**

| Crate | Bin | Purpose |
| --- | --- | --- |
| `crates/hwledger-cli` | `hwledger-cli` | Clap-based CLI — `model search`, `model detail`, `model quants`, `model similar`, `model for-use-case`, `model-ask` (RAG stub), `seed build`, `seed expand`. Every subcommand honors `--json` for scripting. |
| `crates/hwledger-server` | `hwledger-server` | Standalone Axum HTTP server bundling the search crates. CORS + `TraceLayer` enabled, health + admin routes included. |
| `crates/hwledger-mcp` | `hwledger-mcp` | MCP 2024-11-05 server over JSON-RPC 2.0 on stdio. Hand-rolled transport (~200 LOC) to dodge a `PointeeSized` issue with `rmcp` on Rust 1.95. |

**Library crates (shared internals)**

| Crate | Purpose |
| --- | --- |
| `crates/hwledger-search-core` | Core types: `Query`, `Facets`, `ModelKind`, search skill traits. Dependency-free apart from `serde`, `anyhow`, `thiserror`. |
| `crates/hwledger-search-index` | Tantivy schema + `run_hybrid` BM25 hybrid search + variant collapse. |
| `crates/hwledger-search-tags` | Tag/facet extraction and normalization. |
| `crates/hwledger-search-ingest` | Hugging Face adapter, `build_seed_index`, `expand_neighborhood`, `PopulateGate`. |
| `crates/hwledger-search-rag` | RAG v1 stub — NL question → top-K context. |
| `crates/hwledger-search-evals` | YAML-driven eval harness against the search core. |
| `crates/hwledger-search-skills` | Built-in `SearchSkill` implementations (e.g. `AgenticFitRerank`, `LlmSummarizer`) + the default registry. |

## Running tests

From this directory:

```bash
# All crates, all tests (unit + integration).
cargo test --workspace

# A single crate.
cargo test -p hwledger-search-core

# A single test by name substring.
cargo test -p hwledger-search-index hybrid
```

Integration tests live alongside each crate in `tests/`; CLI tests use
`assert_cmd` + `predicates` (declared in `crates/hwledger-cli/Cargo.toml:29-31`).

## Entry-point binaries

### 1. `hwledger-cli` — CLI

Human + machine interface for the search stack. Reads/writes a tantivy index
at `--index` (default `./hwledger-index`, overridable via `HWLEDGER_INDEX`).

```bash
cargo run -p hwledger-cli --bin hwledger-cli -- model search "qwen3 coder"
cargo run -p hwledger-cli --bin hwledger-cli -- --json model detail Qwen/Qwen3-Coder
cargo run -p hwledger-cli --bin hwledger-cli -- seed build --limit 100
```

### 2. `hwledger-server` — HTTP server

Bundles every search crate behind an Axum router. CORS is wide-open
(`Any`) and every request flows through `tower_http::trace::TraceLayer`.

```bash
cargo run -p hwledger-server --bin hwledger-server
# GET http://localhost:8080/healthz
```

### 3. `hwledger-mcp` — MCP stdio server

JSON-RPC 2.0 over stdio implementing MCP 2024-11-05. Six stub tools +
their schemas, plus the `initialize` / `tools/list` / `tools/call` flow.

```bash
cargo run -p hwledger-mcp --bin hwledger-mcp
# Pipe JSON-RPC envelopes on stdin; responses come on stdout.
```

### 4. MCP HTTP transport — _placeholder_

A future MCP-over-HTTP transport (Streamable HTTP transport from the 2025
spec revision) is planned but not yet implemented. When it lands it will
live in `crates/hwledger-mcp` as a sibling of `transport.rs` (likely
`transport_http.rs`) and expose a new `[[bin]]` entry wired into the same
`McpServer` + `tools` modules. Until then, the stdio transport above is the
only MCP surface.

## Quick start

```bash
# 1. Build everything once.
cargo build --workspace --release

# 2. Populate a fresh tantivy index from Hugging Face.
cargo run -p hwledger-cli --bin hwledger-cli -- seed build --limit 200

# 3. Query it.
cargo run -p hwledger-cli --bin hwledger-cli -- model search "deepseek coder"

# 4. (Optional) Serve it over HTTP.
cargo run -p hwledger-server --bin hwledger-server

# 5. (Optional) Expose it as MCP tools to an LLM client.
cargo run -p hwledger-mcp --bin hwledger-mcp
```

`target/` and `Cargo.lock` are git-ignored (see `.gitignore`); refresh the
lockfile with `cargo update -w` when bumping dependencies.
