# apps/model-explorer/web

> **Status:** scaffold only. The web app itself ships under
> [Phase 10 of the model-explorer acceptance skeleton](../../../../docs/superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md#phase-10--web-app-future).
> This README is the canonical dev-onboarding doc *for when the app
> lands*. Today's operator surface is the Rust CLI / server / MCP
> binaries in `apps/model-explorer/rust/`.

This directory will host the TypeScript / Next.js web front-end for
the **Model Explorer** subsystem of hwLedger. The front-end consumes
the same engine the CLI ships against — a Tantivy BM25 store, a
heuristic tagger pipeline, and a HuggingFace source adapter — over
HTTP, via the upcoming `hwledger-server` binary.

It is intentionally placed alongside `apps/model-explorer/rust/`
rather than inside it: the two share a JSON schema, not a Cargo
workspace, so each side can iterate independently.

## Pair docs

- Project intro for the *engine* — `apps/model-explorer/rust/`
  (workspace root has no README yet; cargo crate docs are the
  primary surface).
- Architecture decision —
  [`docs/adr/2026-07-23/ADR-model-explorer.md`](../../../../docs/adr/2026-07-23/ADR-model-explorer.md).
- Operator runbook —
  [`docs/operations/model-explorer-ops.md`](../../../../docs/operations/model-explorer-ops.md).
- Phased acceptance —
  [`docs/superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md`](../../../../docs/superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md).

## What this front-end will do

- Render a faceted search UI over the indexed model corpus
  (`/model search`, `/model detail`, `/model quants`, `/model similar`,
  `/model for-use-case`).
- Render an "ask the corpus" panel backed by the RAG surface
  (`/model-ask`) — useful results land once **Deferred 4**
  (LanceDB dense index + RRF fusion) ships; today the surface is
  BM25-only.
- Surface the architectural / MoE / quant / parameter / license /
  provenance / use-case-fit tags produced by
  `hwledger-search-tags::tag_all`.
- Drive `hwledger-cli seed build` / `seed expand` from a "Rebuild
  index" admin page (the web front-end never holds the token — it
  proxies to the server).

## Dev setup *(provisional — final commands land with Phase 10)*

The web app is not yet implemented; commands below are the **planned**
surface. They will be wired up once `hwledger-server` exposes the JSON
contract documented in the operator runbook.

```bash
# Install dependencies (planned).
pnpm install

# Start a dev server against a local hwledger-server.
pnpm dev

# Build a production bundle.
pnpm build

# Type-check + lint.
pnpm typecheck
pnpm lint

# Unit tests (vitest).
pnpm test
```

The dev server proxies `/api/*` to a locally running
`hwledger-server` (default `http://localhost:20128`). Start the server
first:

```bash
# 1. Build a populated index from the Rust side.
cargo run --manifest-path apps/model-explorer/rust/Cargo.toml \
    -p hwledger-cli -- \
    --index ./hwledger-index seed build --size 2000

# 2. Run the server (binary lands in Phase 9).
cargo run --manifest-path apps/model-explorer/rust/Cargo.toml \
    -p hwledger-server -- --index ./hwledger-index

# 3. Run the web dev server.
pnpm dev
```

## Build *(provisional)*

```bash
# CI build artifact.
pnpm build
pnpm --filter ./apps/model-explorer/web build

# Output lands in apps/model-explorer/web/.next/ (or dist/, depending
# on the chosen bundler).
```

The web build never embeds the index, the tantivy store, or any HF
token — the front-end is a pure consumer of `hwledger-server`.

## Environment variables

The web app reads the following env vars. **None of them are
secrets.** HF tokens are server-side only.

| Env var | Default | Effect |
| ------- | ------- | ------ |
| `NEXT_PUBLIC_HWLEDGER_API` | `http://localhost:20128` | Base URL of `hwledger-server`. The web app reads this in the browser; use `NEXT_PUBLIC_` prefix to expose it to the client bundle. |
| `HWLEDGER_WEB_PORT` | `3000` | Local dev-server port. |
| `HWLEDGER_WEB_LOG_LEVEL` | `info` | Client log level (`error`, `warn`, `info`, `debug`). |

### Server-side env vars (consumed by `hwledger-server`, *not* the web app)

For completeness — the web front-end never sees these, but the
operator running the full stack needs them. See
[`docs/operations/model-explorer-ops.md`](../../../../docs/operations/model-explorer-ops.md#6-hf_token-hygiene)
for the full hygiene rules.

| Env var | Default | Effect |
| ------- | ------- | ------ |
| `HWLEDGER_INDEX` | `./hwledger-index` | Path to the Tantivy index directory. |
| `HF_TOKEN` | (unset) | HuggingFace read-only token. Bearer-auth attached to every HF request when set & non-empty. **Server-side only.** |
| `HF_HUB_URL` | `https://huggingface.co` | Override the HF Hub base URL — used by tests / CI fixtures. |
| `RUST_LOG` | `info` | Standard `tracing` log filter (e.g. `info,hwledger_search_ingest=debug`). |

## Repo layout (planned)

```text
apps/model-explorer/
├── rust/                       # Cargo workspace (shipped, see ADR-037)
│   ├── Cargo.toml
│   └── crates/
│       ├── hwledger-search-core/   # taxonomy, traits, RRF, skills
│       ├── hwledger-search-tags/   # 9 heuristic taggers + orchestrator
│       ├── hwledger-search-index/  # Tantivy BM25 store + run_hybrid
│       ├── hwledger-search-ingest/ # HF adapter + seed builder + lazy gate
│       ├── hwledger-search-rag/    # chunker + StubEmbedder + retrieve
│       ├── hwledger-search-evals/  # eval extractors
│       ├── hwledger-search-skills/ # skill implementations (skeleton)
│       ├── hwledger-cli/           # CLI binary
│       ├── hwledger-server/        # Axum HTTP binary (scaffolded)
│       └── hwledger-mcp/           # MCP binary (scaffolded)
└── web/                        # THIS directory — TypeScript front-end
    ├── README.md               # ← you are here
    ├── package.json
    ├── tsconfig.json
    └── src/
```

## Status

| Item | Status |
| ---- | ------ |
| Cargo workspace (Phases 1–7) | shipped |
| Operator docs (Phase 8) | shipped this turn |
| Web app (Phase 10) | scaffolded, not implemented |
| `hwledger-server` + `hwledger-mcp` (Phase 9) | scaffolded only |
| Deferred 3 — ORT embedder backend | seam landed, impl pending |
| Deferred 4 — LanceDB dense index + RRF fusion | seam landed, impl pending |

See the [acceptance skeleton](../../../../docs/superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md)
for the full phase breakdown and the deferred-work contract.