# model-explorer server

Hono-based HTTP proxy that sits in front of the Rust `hwledger-server`
binary, exposing the same wire contract the Rust CLI already produces
under `--json`.

## Endpoints

| Method | Path | Purpose |
| :-- | :-- | :-- |
| `GET`  | `/healthz` | Liveness probe; also returns the resolved upstream URL. |
| `POST` | `/v1/search` | Hybrid BM25 search; body matches `hwledger_search_core::Query`. |
| `GET`  | `/v1/models/:id` | Single-model detail envelope. |
| `GET`  | `/v1/models/:id/quants` | List quantization tags for a model. |
| `GET`  | `/v1/models/:id/similar` | More-like-this lookup. |
| `GET`  | `/v1/use-case/:use_case` | Agentic / coding / reasoning / embedding filter. |
| `POST` | `/v1/model-ask` | NL question → RAG stub (top-K BM25 hits). |

Every successful response carries an `x-upstream` response header:

- `x-upstream: rust` — payload came from `hwledger-server`.
- `x-upstream: synthesized` — payload came from the in-process fallback
  (used when Rust is offline, mis-configured, or returning 5xx).

The fallback is **feature-for-feature identical in shape** to the Rust
output, so the Svelte UI and CLI consumers behave identically in dev / CI.

## Env

| Variable | Default | Meaning |
| :-- | :-- | :-- |
| `PORT` | `8787` | Port the proxy listens on. |
| `HOST` | `127.0.0.1` | Bind address. |
| `HWLEDGER_UPSTREAM_URL` | `http://127.0.0.1:8080` | Rust server base URL. |
| `HWLEDGER_UPSTREAM_TIMEOUT_MS` | `4000` | Per-request upstream timeout. |

## Develop

```bash
npm install --include=dev
npm run dev           # tsx watch on the source tree
npm test              # vitest run
```

## Build

```bash
npm run build         # tsc → dist/
node dist/index.js    # run the compiled output
```
