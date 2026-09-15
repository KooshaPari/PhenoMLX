# Acceptance Walkthrough — Model Explorer (hwLedger, 2026-07-23)

> **Source spec:** [`docs/superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md`](2026-07-23-hwledger-model-explorer.md)
> **Method:** one line per assertion. ID, evidence command, status.
> **Status legend:** PASS = green today; FAIL = broken today; NOTE = passes with a caveat; DEFERRED = no current coverage.

The walkthrough author synthesised the 21 assertions listed below from the
ten phased acceptance sections of the source spec; the spec itself does not
assign `A-INDEX-*` style IDs. Categories follow the spec's crate layout.

## How evidence was gathered

- `cargo test --manifest-path apps/model-explorer/rust/Cargo.toml --workspace`
  (run twice — once as-is, once with `hwledger-mcp` excluded from the workspace
  `members` list to gather per-crate evidence for the other nine crates).
- `cd apps/model-explorer/server && npx vitest run`
- `cd apps/model-explorer/web && npx vitest run`
- `fs_search` / grep over the Rust source for capability assertions
  (e.g. `ModelKind::Chat`, `pub mod arch_tagger`, `pub fn rrf_fuse`).

Raw logs are in `/tmp/hwledger_cargo_test.log` (workspace-as-is),
`/tmp/hwledger_cargo_test_excl_mcp.log` (workspace minus `hwledger-mcp`),
`/tmp/hwledger_server_vitest.log`, and `/tmp/hwledger_web_vitest.log`.

## Top-line result

- `apps/model-explorer/server` vitest: **22 / 22 pass**
- `apps/model-explorer/web` vitest: **6 / 6 pass**
- Workspace `cargo test --workspace` (as-is): **FAIL** — see W-0.
- Workspace `cargo test --workspace` with `hwledger-mcp` excluded from
  `[workspace] members`: **all green** (see per-crate counts in W-1 … W-10).

## Headline blocker (W-0)

The `hwledger-mcp` crate does not build against the committed `Cargo.toml`.
`cargo test --workspace` fails before any test runs:

1. `apps/model-explorer/rust/crates/hwledger-mcp/Cargo.toml` declares
   `[[bin]] name = "hwledger-mcp-http"` at `src/bin/hwledger_mcp_http.rs`,
   but no such file exists in the worktree (verified by `ls`).
2. The same `Cargo.toml` requests `axum = { workspace = true, features = ["macros", "sse"] }`.
   `axum v0.7.9` (the workspace-pinned version) does not export an `sse` feature;
   cargo errors with `package 'hwledger-mcp' depends on 'axum' with feature 'sse'
   but 'axum' does not have that feature`.
3. The worktree also has a *modified* `crates/hwledger-mcp/src/lib.rs` adding
   `pub mod transport_http` and an untracked source file
   `src/transport_http.rs` (22 KB) — both clearly part of an HTTP+SSE transport
   that is partially scaffolded but uncommitted. The committed `lib.rs` at HEAD
   does not include this module.

Net effect: every workspace `cargo` command fails at dependency resolution.
Per-crate evidence below was gathered by temporarily excluding
`hwledger-mcp` from the workspace `members` list (then reverting); the
exclusion does not change any other crate's behaviour.

## Per-assertion walkthrough

| ID | What it asserts | Evidence command / source grep | Status |
| -- | --------------- | ------------------------------ | ------ |
| **A-INDEX-1** | `TantivyStore` uses per-field BM25 boosts `name^3, org^2, kind^2, family^2, arch^1, quants^1, card_snippet^1` | `grep -n "name\^3, org\^2, kind\^2, family\^2, arch\^1, quants\^1, card_snippet\^1" apps/model-explorer/rust/crates/hwledger-search-index/src/tantivy_store.rs` → matches at lines 12, 360, 361 | **PASS** |
| **A-INDEX-2** | `CollapseRule` + `collapse_variants` + `collapse_key` collapse BM25 hits that share a quantized base id | `cargo test -p hwledger-search-index --test collapse` → `3 passed; 0 failed` (from `/tmp/hwledger_cargo_test_excl_mcp.log`) | **PASS** |
| **A-INDEX-3** | `run_hybrid` is `async`, signature-stable, BM25-only v1 | `grep -n "pub async fn run_hybrid" apps/model-explorer/rust/crates/hwledger-search-index/src/query.rs` → match at line 32; `cargo test -p hwledger-search-index --test query` → `2 passed; 0 failed` | **PASS** |
| **A-TAG-1** | Nine heuristic taggers present: `arch_tagger`, `moe_tagger`, `quant_tagger`, `param_tagger`, `license_tagger`, `modelkind_tagger`, `reap_tagger`, `provenance_tagger`, `usecase_fit_tagger` | `grep -n "pub mod arch_tagger\|pub mod moe_tagger\|pub mod quant_tagger\|pub mod param_tagger\|pub mod license_tagger\|pub mod modelkind_tagger\|pub mod reap_tagger\|pub mod provenance_tagger\|pub mod usecase_fit_tagger" apps/model-explorer/rust/crates/hwledger-search-tags/src/lib.rs` → all nine modules declared | **PASS** |
| **A-TAG-2** | Composite orchestrator `tag_all` produces `AllTags` | `grep -n "pub fn tag_all\|pub struct AllTags" apps/model-explorer/rust/crates/hwledger-search-tags/src/orchestrator.rs` → matches at lines 24, 58; `cargo test -p hwledger-search-tags --test orchestrator` → `2 passed; 0 failed` | **PASS** |
| **A-TAG-3** | `TaggerContext::from_id(id, org)` documented and unit-tested | `grep -n "pub fn from_id" apps/model-explorer/rust/crates/hwledger-search-tags/src/tager_context.rs` → match at line 48; `cargo test -p hwledger-search-tags` (unittests) → `28 passed; 0 failed` | **PASS** |
| **A-SEARCH-1** | Public taxonomy `ModelKind`, `ArchKind`, `AttentionKind`, `MlpKind`, `RopeVariant`, `Modality`, `Facets` — all `Serialize + Deserialize`, `Display` | `ls apps/model-explorer/rust/crates/hwledger-search-core/src/taxonomy/` → `arch.rs`, `faceted.rs`, `model_kind.rs`, `modality.rs`; `grep -n "pub enum ModelKind\|pub enum ArchKind\|pub enum Modality\|pub struct Facets"` → all present; `cargo test -p hwledger-search-core --test unit` → `3 passed; 0 failed` | **PASS** |
| **A-SEARCH-2** | `rrf_fuse` with canonical `k = 60`, deterministic tie-break by id ascending | `grep -n "k = 60\|pub fn rrf_fuse" apps/model-explorer/rust/crates/hwledger-search-core/src/fusion.rs` → matches at lines 6, 42, 51; `cargo test -p hwledger-search-core --test fusion_rrf` → `3 passed; 0 failed` | **PASS** |
| **A-SEARCH-3** | `SourceAdapter` trait with `name`, `list_candidates`, `fetch_raw` | `grep -n "pub trait SourceAdapter\|fn name\|fn list_candidates\|fn fetch_raw" apps/model-explorer/rust/crates/hwledger-search-core/src/source_adapter.rs` → matches at lines 111, 114, 122, 128; `cargo test -p hwledger-search-ingest --test source_adapter` → `1 passed; 0 failed` (HuggingFace adapter round-trip) | **PASS** |
| **A-RAG-1** | `Chunker` produces `Chunk`s with stable `index`, `section`, `text`, `token_offset` | `grep -n "pub struct Chunk\|pub fn chunk" apps/model-explorer/rust/crates/hwledger-search-rag/src/chunker.rs` → matches at lines 14, 40, 83; `cargo test -p hwledger-search-rag --test chunker` → `3 passed; 0 failed` | **PASS** |
| **A-RAG-2** | `StubEmbedder` is deterministic (FNV-1a + LCG → `[-1, 1]` then L2-normalized), dependency-free | `grep -n "FNV_OFFSET\|FNV_PRIME\|StubEmbedder\|embedding_is_l2_normalized" apps/model-explorer/rust/crates/hwledger-search-rag/src/embedder.rs` → all present; `cargo test -p hwledger-search-rag --test embedder` → `3 passed; 0 failed` | **PASS** |
| **A-RAG-3** | `retrieve` ranks chunks by cosine similarity, returns top-`k` descending, with stable rank tie-break by source index | `grep -n "pub async fn retrieve\|cosine" apps/model-explorer/rust/crates/hwledger-search-rag/src/rag.rs` → matches at lines 63, 56; `cargo test -p hwledger-search-rag --test rag` → `2 passed; 0 failed` | **PASS** |
| **A-SKILL-1** | `SearchSkill` trait + `SkillRegistry::run_all` short-circuiting on first error | `grep -n "pub trait SearchSkill\|pub fn run_all\|run_all_short_circuits_on_error" apps/model-explorer/rust/crates/hwledger-search-core/src/skills.rs` → matches at lines 68, 128, 198; `cargo test -p hwledger-search-core --test skills_registry` → `2 passed; 0 failed` | **PASS** |
| **A-SKILL-2** | `AgenticFitRerank` + `LlmSummarizer` built-in skills | `grep -n "pub struct AgenticFitRerank\|pub struct LlmSummarizer" apps/model-explorer/rust/crates/hwledger-search-skills/src/agentic_fit.rs apps/model-explorer/rust/crates/hwledger-search-skills/src/llm_summarizer.rs` → both present; `cargo test -p hwledger-search-skills` (unittests) → `8 passed; 0 failed`; `cargo test -p hwledger-search-skills --test registry` → `5 passed; 0 failed` | **PASS** |
| **A-SKILL-3** | `default_registry()` registers `AgenticFitRerank` then `LlmSummarizer` | `grep -n "pub fn default_registry\|register(Box::new(AgenticFitRerank\|register(Box::new(LlmSummarizer" apps/model-explorer/rust/crates/hwledger-search-skills/src/lib.rs` → matches at lines 61, 63, 64 | **PASS** |
| **A-PERF-1** | `search-evals` README benchmark extractor (no criterion benchmarks in repo) | `grep -n "pub fn parse_readme_results\|pub struct ReadmeEval" apps/model-explorer/rust/crates/hwledger-search-evals/src/readme_results.rs` → matches at lines 24, 71; `cargo test -p hwledger-search-evals --test readme_results` → `3 passed; 0 failed` | **PASS** *with a NOTE: no `criterion`/`benchmark` perf suite exists in the repo; "perf" coverage here is just the README-score extractor. A true perf budget is DEFERRED.* |
| **A-PERF-2** | `SeedBuild::default()` provides a 2000-row seed budget with eight default queries | `grep -n "size: 2000\|SeedBuild::default\|fn default" apps/model-explorer/rust/crates/hwledger-search-ingest/src/seed_builder.rs` → matches at lines 41, 44, 129; `cargo test -p hwledger-search-ingest --test seed_size` → `1 passed; 0 failed` | **PASS** |
| **A-PERF-3** | `PopulateGate` is sync (`std::sync::Mutex`), shared across async + sync callers | `grep -n "pub struct PopulateGate\|std::sync::Mutex\|inner: Arc<Mutex<HashMap" apps/model-explorer/rust/crates/hwledger-search-ingest/src/lazy_populate.rs` → matches at lines 12, 21, 22; `cargo test -p hwledger-search-ingest --test lazy_populate_cache` → `2 passed; 0 failed` | **PASS** |
| **A-CLI/REST/MCP-1** | CLI exposes `model search`, `model detail`, `model quants`, `model similar`, `model for-use-case` | `grep -n "model search\|model detail\|model quants\|model similar\|model for-use-case" apps/model-explorer/rust/crates/hwledger-cli/src/main.rs` → matches at lines 214, 246, 302, 329, 368; `cargo test -p hwledger-cli --test model_subcommands` → `6 passed; 0 failed` | **PASS** |
| **A-CLI/REST/MCP-2** | Hono proxy exposes `GET /healthz`, `POST /v1/search`, `GET /v1/models/:id`, `GET /v1/use-case/:use_case`, `POST /v1/model-ask` (with `x-upstream` stamping + Zod validation) | `cd apps/model-explorer/server && npx vitest run` → `22 / 22 passed` (covers healthz, search, models, use-case, model-ask, CORS, Zod 400, synthesized fallback, upstream forwarding) | **PASS** |
| **A-CLI/REST/MCP-3** | `hwledger-mcp` exposes the six tools `model_search`, `model_detail`, `model_quants`, `model_similar`, `model_for_use_case`, `model_ask` over JSON-RPC 2.0 | `grep -n "pub fn tool_definitions\|\"model_search\"\|\"model_detail\"\|\"model_quants\"\|\"model_similar\"\|\"model_for_use_case\"\|\"model_ask\"" apps/model-explorer/rust/crates/hwledger-mcp/src/tools.rs backend.rs` → all six tools + `tool_definitions` present; `cargo test -p hwledger-mcp --tests` → **FAIL** (workspace build breaks — see W-0) | **FAIL** |

## Test counts by crate (workspace *minus* `hwledger-mcp`)

| Crate | Unit tests | Integration tests | Status |
| ----- | ---------- | ----------------- | ------ |
| `hwledger-search-core` | 22 | 8 (fusion_rrf=3, skills_registry=2, unit=3) | PASS |
| `hwledger-search-tags` | 28 | 12 (arch=2, fit=2, modelkind=2, moe=1, orchestrator=2, provenance=1, quant=2) | PASS |
| `hwledger-search-index` | 9 | 9 (collapse=3, ingest=1, query=2, tantivy_crud=3) | PASS |
| `hwledger-search-ingest` | 13 | 9 (config_moeqwen=1, config_qwen=2, lazy_populate_cache=2, seed_size=1, source_adapter=1, tree_quant_gguf=2) | PASS |
| `hwledger-search-rag` | 11 | 8 (chunker=3, embedder=3, rag=2) | PASS |
| `hwledger-search-evals` | 10 | 7 (card_table=2, model_index=2, readme_results=3) | PASS |
| `hwledger-search-skills` | 8 | 5 (registry=5) | PASS |
| `hwledger-cli` | 0 | 13 (model_ask=3, model_subcommands=6, seed=4) | PASS |
| `hwledger-server` | 5 | 22 (admin=3, ask=2, common=0, detail=5, for_use_case=2, health=2, search=4, similar=2) | PASS |
| `hwledger-mcp` | — | — | **FAIL** (W-0) |

Plus 1 doctest (`hwledger-search-tags/src/lib.rs:32`) — passes.

## What this walkthrough does *not* cover

- **No criterion / no perf budgets.** The spec mentions Deferred 3 (ORT
  embedder) and Deferred 4 (LanceDB dense index) but no perf suite exists.
  A-PERF-1..3 are placeholder rows pinning the closest available evidence.
- **No live HF network tests.** All integration tests use stub adapters or
  fixtures; `cargo test --workspace` does not touch the network.
- **No web-app screenshot / DOM coverage.** The web vitest run covers
  `ApiClient` only (6 tests). The three-pane Svelte UI itself is not
  acceptance-tested below this line.
- **Deferred 3 (ORT) and Deferred 4 (LanceDB)** are explicitly out of scope
  per the spec's §4 "Deferred work — contract reservation" table and are
  not part of these 21 assertions.

## Suggested follow-ups

1. **Fix the `hwledger-mcp` build (W-0)** before relying on A-CLI/REST/MCP-3.
   Either: drop the `hwledger-mcp-http` bin entry + the `sse` axum feature
   (reset the worktree to the committed state), or commit the missing
   `src/bin/hwledger_mcp_http.rs` + the `transport_http` module + module
   declaration in `lib.rs` and select the correct axum feature name.
2. **Add a perf budget.** None exists today; A-PERF-1..3 are evidence
   placeholders. Consider adding a `criterion` bench target for
   `run_hybrid` and `seed build` so the "perf" category has real coverage.
3. **Re-run this walkthrough after fixing W-0** to flip A-CLI/REST/MCP-3 from
   FAIL to PASS.
