# Model Explorer — Operator Runbook

> **Subsystem:** `apps/model-explorer/` (hwLedger, Phase 12 bootstrap)
> **Audience:** operators, on-call engineers, anyone running `hwledger-cli`
> in CI or production.
> **Pair docs:**
> [`docs/adr/2026-07-23/ADR-model-explorer.md`](../adr/2026-07-23/ADR-model-explorer.md)
> ·
> [`docs/superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md`](../superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md)

This runbook covers day-2 operations for the model-explorer search layer:
how to populate the index, how to expand it around a known seed set, how
to keep `HF_TOKEN` safe, and what to do when something breaks.

## 1. What this subsystem is

The model explorer is the *inventory* half of hwLedger's two-step
**Plan + Reconcile** promise:

- **Plan** — produces a hardware fit estimate for one candidate model.
- **Reconcile** — picks the candidate out of a real corpus. That's us.

We ingest the HuggingFace Hub (HF) into a local Tantivy BM25 store,
heuristically tag each model (architecture, MoE, quant, params, license,
provenance, use-case fit), and expose `model …` + `seed …` subcommands.
A web front-end at `apps/model-explorer/web/` will consume the same
engine via `hwledger-server` once it ships; today the CLI is the
primary operator surface.

The full workspace layout, the dependency-light `search-core` contract,
and the Deferred 3 (ORT) / Deferred 4 (LanceDB) seams are documented in
[ADR-037](../adr/2026-07-23/ADR-model-explorer.md). This runbook is the
*how*; the ADR is the *why*.

## 2. Build & test

All commands are run from the worktree root unless noted.

```bash
# One-shot workspace build (debug)
cargo build --manifest-path apps/model-explorer/rust/Cargo.toml

# Release build for a deployment artifact
cargo build --release --manifest-path apps/model-explorer/rust/Cargo.toml

# Whole-workspace test run (includes stub adapter fixtures, no network)
cargo test  --manifest-path apps/model-explorer/rust/Cargo.toml

# Single crate (faster loop while iterating)
cargo test  -p hwledger-search-rag --manifest-path apps/model-explorer/rust/Cargo.toml
cargo test  -p hwledger-search-ingest --manifest-path apps/model-explorer/rust/Cargo.toml

# Lint (workspace-wide)
cargo clippy --manifest-path apps/model-explorer/rust/Cargo.toml --all-targets -- -D warnings
```

The `seed build` and `model search` flows do hit the network; CI jobs
that exercise them must either inject a fake `SourceAdapter` (see
`crates/hwledger-search-ingest/tests/seed_size.rs`) or run with a
cached `HF_TOKEN` and accept rate limits.

## 3. `seed build` — populate a fresh index

### What it does

Given a list of HF search queries, fan out per-query candidate lists,
fetch each model's raw payload (`/api/models/{id}`,
`/api/models/{id}/tree/main`, `/api/models/{id}/raw/main/README.md`),
and push parsed models into the Tantivy sink. The default query set
covers the families we care about for fleet capacity planning:

```text
qwen2.5, llama-3.1, deepseek-v3, gemma-2,
mistral-nemo, phi-3, codestral, bge-large
```

(See `crates/hwledger-search-ingest/src/seed_builder.rs:DEFAULT_SEED_QUERIES`.)

### Standard invocation

```bash
hwledger-cli \
    --index ./hwledger-index \
    seed build \
    --size 2000
```

`--size` is a **soft cap** on total indexed models. The builder divides
that evenly across queries (floor of 1 per query), so the empty-query
case still produces a sensible run.

### Append vs. wipe

The default is to **wipe** the index directory before building. To
preserve an existing index and merge new models into it:

```bash
hwledger-cli \
    --index ./hwledger-index \
    seed build \
    --append \
    --queries codestral,bge-large \
    --size 500
```

`--append` is what you want for incremental refresh. Forgetting the
flag and re-running on a populated index will silently wipe it — there
is no undo.

### JSON output for piping

```bash
hwledger-cli --json --index ./hwledger-index seed build --size 2000 \
    | tee seed-build.json
```

The JSON envelope is:

```json
{
  "models_indexed": 1847,
  "errors": 0,
  "queries_run": 8
}
```

### Exit codes / failure modes

| Symptom | Cause | Action |
| ------- | ----- | ------ |
| `failed to wipe existing index at <path>` | perms / disk / path is a symlink to a non-empty dir | Inspect `--index`; do not symlink the index dir across machines. |
| `tantivy commit failed` | disk full, fsync failure, or the index is locked by another process | Free space, then re-run. CI must not share the index dir across parallel jobs. |
| non-zero `errors`, zero `models_indexed` | every request returned 401/403/429 | See §6 *HF_TOKEN hygiene* and §7 *Known limitations*. |
| `failed to build HF adapter` | `HF_HUB_URL` is unparseable | unset or fix the env var. |

## 4. `seed expand` — neighborhood expansion (v1 stub)

### What it does (today)

```bash
hwledger-cli \
    --index ./hwledger-index \
    seed expand \
    --seeds hf::meta-llama/Llama-3.1-8B,hf::meta-llama/Llama-3.1-8B-Instruct
```

In v1, `expand_neighborhood` is a **stub**: it accepts the seed list,
opens the index to validate the path, then returns the seeds
unchanged. The function signature and the JSON envelope are stable
(`{"seeds": [...], "expanded": [...]}` where `expanded == seeds`) so
the operator contract doesn't churn when the real crawl lands.

Operators see the stub status in the logs:

```text
INFO expansion deferred to lazy populate + neighborhood crawl seed_count=2
```

(See `crates/hwledger-search-ingest/src/expansion.rs:43`.)

### When the real expansion lands

- The `--seeds` list will be enriched with related models from the
  upstream source (forks, "used by", family siblings).
- `ExpansionConfig::max_neighbors` (default `10`) will cap the
  per-seed expansion width.
- The same `--json` envelope will gain an `expanded_from_seeds` field
  in addition to the final `expanded` list.

Until that ships, treat `seed expand` as a *contract test*: if you
expect it to do work and it silently no-ops, that's the v1 behavior.

## 5. Index lifecycle

### Layout

A built index is a Tantivy directory at the path passed via
`--index` (env: `HWLEDGER_INDEX`, default `./hwledger-index`).
`hwledger-cli` opens it with `open_or_create_store`, which creates an
empty index if the directory is missing.

### Wipe & rebuild

```bash
rm -rf ./hwledger-index
hwledger-cli seed build --size 2000
```

### Inspect a row

```bash
hwledger-cli --index ./hwledger-index model detail hf::meta-llama/Llama-3.1-8B
hwledger-cli --json --index ./hwledger-index model detail hf::meta-llama/Llama-3.1-8B
```

The CLI assumes `hf::` when no source prefix is present.

### Backup / restore

Stop all writers (there should only be one), then `tar -C ./ -czf
hwledger-index.tgz hwledger-index/`. To restore, extract into the
same path and re-run. Cross-machine restores work because Tantivy is
deterministic across platforms for the same `tantivy` version.

## 6. HF_TOKEN hygiene

### Why a token helps

The HF Hub endpoints we hit are public; **no token is required** to
list candidates, fetch metadata, list trees, or pull README cards.
A token only widens rate limits and unlocks gated models. The
adapter attaches the token as a `Bearer` header on every request when
present (`HuggingFaceAdapter::authed`,
`crates/hwledger-search-ingest/src/huggingface.rs:103`).

### How the token is read

`HuggingFaceAdapter::from_env()` reads two env vars:

| Env var | Default | Effect |
| ------- | ------- | ------ |
| `HF_TOKEN` | (unset) | `Bearer` auth attached when set & non-empty. **Empty string is treated as unset.** |
| `HF_HUB_URL` | `https://huggingface.co` | Override the upstream base URL — used by tests and CI fixtures. |

Neither value is logged. The token is stored in the adapter as a
`String` and surfaced via `token_snapshot()` (returns `Option<&str>`).
Do not log the result of `token_snapshot()` to anywhere persistent.

### Storage rules

- **Do** put `HF_TOKEN` in your shell's secret store (1Password CLI,
  `pass`, macOS Keychain via `security`, Windows Credential Manager).
- **Do** set it per-shell or per-job; do not export it globally.
- **Do** use a **read-only** HF token. The CLI never calls write
  endpoints, so a write-capable token is overkill and increases blast
  radius if leaked.
- **Don't** commit it to the repo, paste it into a GitHub issue, or
  echo it in a debug log line.
- **Don't** pass it as a positional CLI arg. The CLI does not accept
  a `--token` flag by design — env-only keeps it out of `ps`/`history`.
- **Don't** leave it set in a long-lived CI runner. CI jobs should
  `unset HF_TOKEN` in teardown and use masked secrets with a tight
  scope.

### Rotating

1. Issue a new read-only HF token.
2. Update the secret store / CI secret.
3. The next `seed build` or `lazy_populate` call picks up the new
   token automatically. No CLI restart needed.
4. Revoke the old token in HF Hub settings.

### When the token is wrong

| Signal | Decode |
| ------ | ------ |
| HTTP 401 | token is invalid or revoked |
| HTTP 403 | token is valid but lacks access (gated model) |
| HTTP 429 | rate limited — token helps, but backing off helps more |

These map to `IngestError::AuthRequired` and `IngestError::RateLimited`
(`crates/hwledger-search-ingest/src/huggingface.rs:111`). The seed
builder logs the failure at `warn!` and increments `report.errors`;
the run continues with the next candidate.

## 7. Known limitations

These are real today. None of them are blockers for the inventory
phase, but each one should be cited when a downstream consumer asks
*"why doesn't my query return what I expected?"*

| # | Limitation | Where it lives | Status | Notes |
| - | ---------- | -------------- | ------ | ----- |
| L7.1 | **`model-ask` returns BM25 hits only.** | `crates/hwledger-cli/src/main.rs:443` (RAG v1 stub) | Known limitation | Snippets are empty; a real pipeline will chunk the README card and run cosine retrieval. See Deferred 4. |
| L7.2 | **`seed expand` is a no-op stub.** | `crates/hwledger-search-ingest/src/expansion.rs:33` | Known limitation | Returns `seeds` unchanged so the operator contract doesn't churn when the real crawl lands. |
| L7.3 | **Dense embeddings ship as a deterministic stub.** | `crates/hwledger-search-rag/src/embedder.rs:46` (`StubEmbedder`) | **Deferred 3 — ORT embedder** | FNV-1a + LCG → `[-1, 1]`, L2-normalized. Deterministic per input, dependency-free, ideal for golden tests. The real embedder backend (FastEmbed / candle / ORT) plugs in via the `Embedder` trait. |
| L7.4 | **Hybrid search is BM25-only on the v1 build.** | `crates/hwledger-search-index/src/query.rs:32` (`run_hybrid`) | **Deferred 4 — LanceDB dense index** | Default build (no cargo features) is BM25-only: `run_hybrid` returns BM25 hits wrapped in `FusedResult`. With `cargo run --features lancedb`, the same function gains two extra arguments (`Option<&LanceStore>`, `&[f32]`) and fuses BM25 + LanceDB ANN with RRF (`k = 60`) via `hwledger-search-core::rrf_fuse`. The OFF signature is stable so callers transition to fused results without API churn. |
| L7.5 | **Facet filter only honors `kinds`.** | `crates/hwledger-search-index/src/query.rs:57` | Known limitation | `modalities`, `arch_kinds`, `attention_kinds`, numeric ranges, `license`, `provenance`, `quants` are accepted on the `Query` but not yet wired through tantivy's filter layer. v1 silently skips them so a result that matches the unstructured query is never dropped. |
| L7.6 | **Empty free-text returns no rows.** | `crates/hwledger-search-index/src/query.rs:38` | Known limitation | Empty text against an empty string would parse to a MatchAll on every doc; v1 returns `[]` so `--text ""` is predictable. |
| L7.7 | **`model for-use-case` is a kind filter.** | `crates/hwledger-cli/src/main.rs:371` | Known limitation | The rich `agentic_fit`/`coding_fit` numerics from `hwledger-search-evals` aren't wired into the sort yet; v1 short-circuits on the kind facet. |
| L7.8 | **LanceDB is wired behind a feature flag.** | `crates/hwledger-search-index/Cargo.toml:21` (`lancedb = ["dep:lancedb", "dep:futures"]`) | **Deferred 4** | The `lancedb 0.31` dependency is declared as `optional = true` and only linked when the `lancedb` cargo feature is enabled (default OFF, so the v1 BM25-only build does not pull LanceDB / arrow / DataFusion). With the feature on, `LanceStore::insert` materialises an `embeddings` table (id + `FixedSizeList<Float32, N>`) and `LanceStore::ann` runs cosine nearest-neighbour against it; `run_hybrid` fuses the two via RRF. The feature name `lancedb` is the public toggle; the `lance-ann` / stub-branching seams from earlier drafts have been removed. |

### Reserved seams (deferred but stable)

| ID | What lands later | Where the seam is today |
| -- | ---------------- | ----------------------- |
| **Deferred 3** | ORT-based embedder backend behind `hwledger_search_rag::Embedder` | `crates/hwledger-search-rag/src/embedder.rs:34` |
| **Deferred 4** | LanceDB dense index + RRF fusion wired into `run_hybrid` | `crates/hwledger-search-index/src/query.rs:32` |

The Deferred numbering is project-internal: **3 = ORT**, **4 =
LanceDB**. They are tracked independently so an operator can see
exactly what is missing and where the wiring will happen.

## 7b. Rate limiting (HF Hub unverified-account tier)

**Status:** Verified during the live `seed build --size=2000` run on
2026-07-23. Free-tier / unverified HF accounts (~no CC card, freshly
created) hit HTTP 429 on every request after the first ~10 within a
few minutes. The current `HuggingFaceAdapter` does **not** throttle
itself and does **not** honor `Retry-After` — each 429 is logged at
`warn!` and counted in `report.errors`, but the loop continues.

**Why it's a deferred item, not a blocker:** the live run still
ingested 101 real models across 8 curated queries before HF
started returning 429. The pipeline degrades gracefully — models that
*did* succeed are committed to Tantivy; rate-limited candidates are
skipped. The operator can re-run later or upgrade the account.

**Follow-up shape** (when this lands, the design doc should record it):

| Aspect | Recommendation |
| ------ | -------------- |
| Flag on `seed build` | `--rate-limit-ms <N>` (default 1000, set to 15000 for free tier) |
| Backoff on 429 | Honor `Retry-After` header; exponential up to 60s; bail after 5 retries |
| Concurrency | `--max-concurrent <N>` (default 1 for free tier) |
| Visibility | `tracing::info!` on every sleep ("rate-limited, sleeping 12s") so operator sees why |
| Default ON | `--retry-on-429` (default true); `--no-retry-on-429` for CI/no-network |
| Layer | The wait happens at the HTTP layer in `HuggingFaceAdapter`, not in the seed builder, so `lazy_populate` benefits too |
| Test | Mock reqwest middleware returns `429 + Retry-After: 5`; assert one retry succeeds |

**Operator workaround today:** upgrade the HF account (free — verify
email + phone in https://huggingface.co/settings/account — no CC
needed for verification, only for paid Pro tier). Same token still
works; just the account tier behind it changes. Free-tier rate limits
are documented at https://huggingface.co/docs/api-inference/en/rate-limits
and the verified tier is roughly 20× higher.

A WIP-rate-limit-broken-state was attempted this session; the adapter
struct shape drifted across multiple patches and the commit was
abandoned. The work-in-progress was dropped from the tree before
close-out — see `git log --diff-filter=D --since="2026-07-23"` if
you need to recover the failed approach as a starting point.

## 8. Logging & observability

- Logs go to **stderr** by default; `--json` output goes to **stdout**.
  This means a JSON consumer can pipe `--json` straight to `jq` without
  log noise on stdout.
- The log level is controlled by the standard `RUST_LOG` env var
  (e.g. `RUST_LOG=info,hwledger_search_ingest=debug`). Default is
  `info`.
- Per-request tracing is not yet wired. Each HF request logs at the
  call site (via `tracing::warn!` on transport failure) but successful
  requests are silent.

## 9. Quick recipes

```bash
# Build a fresh index from the default query set, then search.
hwledger-cli --index ./idx seed build --size 2000
hwledger-cli --index ./idx model search "small instruct coder" --limit 10

# Inspect a single model's metadata as JSON (pipe to jq).
hwledger-cli --json --index ./idx model detail meta-llama/Llama-3.1-8B-Instruct

# Filter by kind.
hwledger-cli --index ./idx model search "agent" --kind agentic --limit 5

# Use-case filter (v1 = kind short-circuit).
hwledger-cli --index ./idx model for-use-case coding --text "python" --limit 10

# Append a new family to an existing index.
hwledger-cli --index ./idx seed build --append --queries codestral,bge-large --size 500
```

## 10. Escalation

If you hit something this runbook doesn't cover:

1. Check the per-crate doc comments — every public item has a
   `//!` module preamble and `///` doc on the public surface
   (`#![deny(missing_docs)]` is enforced workspace-wide).
2. Read the acceptance skeleton at
   `docs/superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md`
   for the phased contract.
3. File an issue with the operator runbook output (`hwledger-cli
   --json …`), the commit hash of the binary you ran, and the
   exact `RUST_LOG` output.