# Acceptance Skeleton — Model Explorer (hwLedger, 2026-07-23)

> **Filename convention:** `docs/superpowers/specs/acceptance/YYYY-MM-DD-<repo>-<feature>.md`
> **Scope:** the `apps/model-explorer/` subsystem under `hwLedger`.
> **Pair docs:**
> [`docs/adr/2026-07-23/ADR-model-explorer.md`](../../../adr/2026-07-23/ADR-model-explorer.md)
> ·
> [`docs/operations/model-explorer-ops.md`](../../../operations/model-explorer-ops.md)
> ·
> [`apps/model-explorer/web/README.md`](../../../../apps/model-explorer/web/README.md)

This skeleton defines what *done* means for the model-explorer bootstrap.
Each phase lists its delivery surface, the acceptance criteria, the
autograder gates, and the doc touch-points. Phases 1–10 are all shipped
in commits `d9e2856 → b9520a1`; this skeleton is the single source of
truth for whether a later change keeps the contract intact.

## 1. Scope

### 1.1 In scope

- Cargo workspace at `apps/model-explorer/rust/` with seven thin crates
  + three thin front-ends (see §3 below).
- HuggingFace adapter, seed builder, lazy-populate gate, v1 expansion
  stub.
- Tantivy BM25 store with structured facets (kind only wired today)
  and a `run_hybrid` driver that is BM25-only today and BM25 + LanceDB
  tomorrow (Deferred 4).
- CLI surface (`model …` + `seed …` + `model-ask`), `--json` everywhere,
  comfy-table by default.
- Operator docs (this skeleton + ADR-037 + the ops runbook).
- Web project README at `apps/model-explorer/web/` (the web app itself
  is **out of scope** for this ADR; it ships under Phase 10).

### 1.2 Out of scope

- Dense vector embeddings (Deferred 3 — ORT embedder).
- Dense vector index (Deferred 4 — LanceDB).
- Web UI implementation (Phase 10 — `apps/model-explorer/web/`).
- `hwledger-server` and `hwledger-mcp` binaries are scaffolded only;
  they wire to the same engine in a later phase.

## 2. Functional requirements covered

This subsystem contributes to the broader hwLedger requirements. Each
phase maps back to one or more of:

| Requirement (existing FR specs) | Phases that touch it |
| -------------------------------- | -------------------- |
| (no formal FR exists for *inventory* yet) | Phases 1–7 |
| `FR-HWL-CAPACITY-001` — Capacity Fit Estimate | consumes `model for-use-case` output (downstream) |
| `FR-HWL-FLEET-001` — Fleet Ledger Compare | consumes `model detail` output (downstream) |
| `NFR-HWL-REPRODUCIBILITY-001` | served by the deterministic stub embedder (Deferred 3) and the tantivy segment layout |

A formal FR for the model-explorer *inventory* flow is **TODO** and
should be authored as part of the next planning cycle. The acceptance
gates below are the stand-in until that FR lands.

## 3. Phase-by-phase acceptance

Each phase must satisfy **all** of:

- The listed crates compile under `cargo build`.
- The listed tests pass under `cargo test -p <crate>`.
- The listed CLI behavior is observable end-to-end.
- The listed docs are present and cross-linked.

### Phase 1 — `hwledger-search-core` *(shipped, `d9e2856`)*

**Surface:** `apps/model-explorer/rust/crates/hwledger-search-core/`

**Acceptance criteria**

- [x] Workspace dependency-light: only `serde`, `serde_json`, `anyhow`,
      `thiserror`. No tantivy, no lancedb, no ort.
- [x] Public taxonomy: `ModelKind`, `ArchKind`, `AttentionKind`,
      `MlpKind`, `RopeVariant`, `Modality`, `Facets` — all `Serialize +
      Deserialize`, all `Display` for faceting.
- [x] Source-adapter trait `SourceAdapter` with `name`, `list_candidates`,
      `fetch_raw`.
- [x] `rrf_fuse` with canonical `k = 60`, deterministic tie-break by
      id ascending.
- [x] Skill registry: `SearchSkill` trait + `SkillRegistry::run_all`
      short-circuiting on the first error.

**Gates**

- `cargo test -p hwledger-search-core`
- `cargo clippy -p hwledger-search-core --all-targets -- -D warnings`

### Phase 2 — `hwledger-search-tags` *(shipped, `4500e76`)*

**Surface:** `apps/model-explorer/rust/crates/hwledger-search-tags/`

**Acceptance criteria**

- [x] Nine heuristic taggers: `arch_tagger`, `moe_tagger`, `quant_tagger`,
      `param_tagger`, `license_tagger`, `modelkind_tagger`,
      `reap_tagger`, `provenance_tagger`, `usecase_fit_tagger`.
- [x] Composite orchestrator `tag_all` producing `AllTags`.
- [x] `TaggerContext::from_id("meta-llama/Llama-3.1-8B", "meta-llama")`
      is documented and unit-tested.

**Gates**

- `cargo test -p hwledger-search-tags`

### Phase 3 — `hwledger-search-index` *(shipped, `727234b`)*

**Surface:** `apps/model-explorer/rust/crates/hwledger-search-index/`

**Acceptance criteria**

- [x] `TantivyStore` with per-field BM25 boosts:
      `name^3, org^2, kind^2, family^2, arch^1, quants^1, card_snippet^1`.
- [x] `CollapseRule` + `collapse_variants` + `collapse_key` collapse
      BM25 hits that share a quantized base id into one family row.
- [x] `run_hybrid(store, query, k)` is `async` for signature stability;
      v1 returns BM25-only results wrapped in `FusedResult`.
- [x] Kind facet filter is the **only** facet wired today; other facets
      are accepted but skipped (no silent drops).

**Gates**

- `cargo test -p hwledger-search-index`
- `cargo test -p hwledger-search-index --test tantivy_crud`
- `cargo test -p hwledger-search-index --test query`

### Phase 4 — `hwledger-search-ingest` *(shipped, `a2cc835`)*

**Surface:** `apps/model-explorer/rust/crates/hwledger-search-ingest/`

**Acceptance criteria**

- [x] `HuggingFaceAdapter` covers the four public HF endpoints
      (`/api/models?search=`, `/api/models/{id}`, `/api/models/{id}/tree/main`,
      `/api/models/{id}/raw/main/README.md`).
- [x] `from_env()` honors `HF_TOKEN` and `HF_HUB_URL`. Empty `HF_TOKEN`
      is treated as unset (not as an error).
- [x] `build_seed_index` with `SeedBuild::default()` covers the eight
      default queries (`qwen2.5, llama-3.1, deepseek-v3, gemma-2,
      mistral-nemo, phi-3, codestral, bge-large`).
- [x] `PopulateGate` is sync (`std::sync::Mutex`) and shared across
      async + sync callers without a separate async primitive.
- [x] `expand_neighborhood` is the v1 stub: returns seeds unchanged,
      logs `"expansion deferred to lazy populate + neighborhood crawl"`.

**Gates**

- `cargo test -p hwledger-search-ingest`
- `cargo test -p hwledger-search-ingest --test seed_size`
- `cargo test -p hwledger-search-ingest --test source_adapter`

### Phase 5 — `hwledger-search-rag` *(shipped, `7a99470`)*

**Surface:** `apps/model-explorer/rust/crates/hwledger-search-rag/`

**Acceptance criteria**

- [x] `Chunker` produces `Chunk`s with stable `index`, `section`,
      `text`, `token_offset`.
- [x] `StubEmbedder` is deterministic (FNV-1a + LCG → `[-1, 1]` then
      L2-normalized), dependency-free.
- [x] `Embedder` trait exposes `embed`, `dim`, `name` — the seam for
      **Deferred 3** ORT backend.
- [x] `retrieve` ranks chunks by cosine similarity, returns top-`k`
      descending, with stable rank tie-break by source index.

**Gates**

- `cargo test -p hwledger-search-rag`
- `cargo test -p hwledger-search-rag --test rag`
- `cargo test -p hwledger-search-rag --test embedder`

### Phase 6 — `hwledger-search-evals` *(shipped, `044dee2`)*

**Surface:** `apps/model-explorer/rust/crates/hwledger-search-evals/`

**Acceptance criteria**

- [x] `model_index`, `card_table`, `readme_results` extractors operate
      on an indexed model.
- [x] Extractors are independent: a regression in one doesn't block
      the others.

**Gates**

- `cargo test -p hwledger-search-evals`
- `cargo test -p hwledger-search-evals --test readme_results`

### Phase 7 — `hwledger-cli` *(shipped, `232f2c2`)*

**Surface:** `apps/model-explorer/rust/crates/hwledger-cli/`

**Acceptance criteria**

- [x] `model search <text> [--kind <kinds>] [--limit N]`
- [x] `model detail <id>` (assumes `hf::` when no source prefix)
- [x] `model quants <id>`
- [x] `model similar <id> [--limit N]`
- [x] `model for-use-case <use-case> [--text <text>] [--limit N]`
      (`agentic`, `coding`, `reasoning`, `embedding`)
- [x] `model-ask <question> [--limit N]` *(RAG v1 stub — see L7.1)*
- [x] `seed build [--queries <q,…>] [--size N] [--append]`
- [x] `seed expand --seeds <id,…>` *(v1 stub — see L7.2)*
- [x] `--json` on every subcommand; `--index` global, env
      `HWLEDGER_INDEX` (default `./hwledger-index`).

**Gates**

- `cargo test -p hwledger-cli`
- `cargo test -p hwledger-cli --test model_subcommands`
- `cargo test -p hwledger-cli --test seed`

### Phase 8 — Operator docs *(this turn)*

**Surface:**
- `docs/adr/2026-07-23/ADR-model-explorer.md`
- `docs/operations/model-explorer-ops.md`
- `docs/superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md`
- `apps/model-explorer/web/README.md`

**Acceptance criteria**

- [x] ADR in MADR 3.0 format with context, decision drivers, four
      considered options (single mega-crate, service split, workspace
      of thin crates *(chosen)*, standalone new repo), pros/cons,
      consequences, deferred seams (3 ORT, 4 LanceDB).
- [x] Operator runbook covers `seed build`, `seed expand`,
      `HF_TOKEN` hygiene, and a known-limitations table with Deferred
      3 (ORT) and Deferred 4 (LanceDB) status rows.
- [x] This acceptance skeleton references every phase and links to
      the ADR + runbook + web README.
- [x] Web project README at `apps/model-explorer/web/README.md` with
      intro, dev/build commands, and env vars.

**Gates**

- All four files exist at the listed paths.
- All cross-references resolve.
- Three commits land with the agreed messages:
  - `docs(adr): ADR-037 Model Explorer search layer in hwLedger`
  - `docs(operations): Model Explorer operator runbook`
  - `docs(web): apps/model-explorer README`

### Phase 9 — `hwledger-server` + `hwledger-mcp` *(shipped, `eed7961`/`273feed`/`9aaccd1`)*

**Surface:**
- `apps/model-explorer/rust/crates/hwledger-server/`
- `apps/model-explorer/rust/crates/hwledger-mcp/`
- `apps/model-explorer/server/` (Hono proxy)

**Acceptance criteria**

- [x] `hwledger-server` exposes `run_hybrid` over Axum with the same
      JSON contract as the CLI's `--json` output.
- [x] `hwledger-mcp` exposes the same engine as MCP tools
      (`model_search`, `model_detail`, `model_quants`, `model_similar`,
      `model_for_use_case`, `model_ask`) over JSON-RPC 2.0 on stdio.
- [x] A Hono proxy under `apps/model-explorer/server/` fronts the Axum
      service for the web app and any external HTTP consumer.
- [x] Both binaries share `HWLEDGER_INDEX`, `HF_TOKEN`, `HF_HUB_URL`.

### Phase 10 — Web app *(shipped, `e0db83f`)*

**Surface:** `apps/model-explorer/web/`

**Acceptance criteria**

- [x] `apps/model-explorer/web/` ships a Svelte 5 + SvelteKit
      TypeScript front-end that consumes `hwledger-server` (via the Hono
      proxy) over HTTP.
- [x] Three-pane search layout: query input + facet sidebar on the
      left, hit list in the middle, model detail panel on the right.
- [x] The web README is the canonical dev-onboarding doc for the
      front-end (authored in Phase 8, updated as the app grows).

## 4. Deferred work — contract reservation

These items are **explicitly deferred** but the seams are wired today
so future work lands without an API churn.

| ID | Item | Seam location | Acceptance for "seam landed" (no implementation yet) |
| -- | ---- | ------------- | --------------------------------------------------- |
| **Deferred 3** | ORT-based embedder backend | `crates/hwledger-search-rag/src/embedder.rs:34` (`Embedder` trait) | Trait is `Send + Sync`, exposes `embed`, `dim`, `name`. `StubEmbedder` ships as the reference impl. |
| **Deferred 4** | LanceDB dense index + RRF fusion wired into `run_hybrid` | `crates/hwledger-search-index/src/query.rs:32` (`run_hybrid`) | `run_hybrid` is `async` and signature-stable. `rrf_fuse` (`k = 60`) is implemented in `hwledger-search-core` and unit-tested. The dense side returns BM25 hits today; tomorrow it returns fused results without a call-site change. |

## 5. Global autograder gates

The minimum bar before any change to this subsystem can be marked
*done*:

```bash
# Workspace-wide build, debug.
cargo build  --manifest-path apps/model-explorer/rust/Cargo.toml

# Workspace-wide tests, including stub adapter fixtures (no network).
cargo test   --manifest-path apps/model-explorer/rust/Cargo.toml

# Workspace-wide clippy, deny warnings.
cargo clippy --manifest-path apps/model-explorer/rust/Cargo.toml \
    --all-targets -- -D warnings

# Workspace-wide docs (every public item has a `///` doc).
cargo doc   --manifest-path apps/model-explorer/rust/Cargo.toml --no-deps
```

Each crate also has crate-level `#![deny(missing_docs)]` and
`#![deny(rust_2018_idioms)]`, so a missing doc comment is a compile
error, not a lint warning.

## 6. Cross-references

- ADR — [`docs/adr/2026-07-23/ADR-model-explorer.md`](../../../adr/2026-07-23/ADR-model-explorer.md)
- Operator runbook — [`docs/operations/model-explorer-ops.md`](../../../operations/model-explorer-ops.md)
- Web project intro — [`apps/model-explorer/web/README.md`](../../../../apps/model-explorer/web/README.md)
- Project index — [`docs/index.md`](../../../index.md) *(regenerate after this commit so the new files show up)*
- Existing FR specs that this subsystem feeds into — [`docs/specs/`](../../)
- Wider hwLedger journey traceability —
  [`docs/operations/journey-traceability.md`](../../operations/journey-traceability.md)

## 7. Status snapshot

- [x] Phase 1 — `search-core`
- [x] Phase 2 — `search-tags`
- [x] Phase 3 — `search-index`
- [x] Phase 4 — `search-ingest`
- [x] Phase 5 — `search-rag`
- [x] Phase 6 — `search-evals`
- [x] Phase 7 — `cli`
- [x] Phase 8 — operator docs (ADR + runbook + acceptance skeleton + web README)
- [x] Phase 9 — server + MCP binaries (`hwledger-server` Axum + Hono proxy; `hwledger-mcp` JSON-RPC over stdio)
- [x] Phase 10 — web app (Svelte 5 + SvelteKit three-pane search UI)
- [ ] Deferred 3 — ORT embedder backend (seam landed)
- [ ] Deferred 4 — LanceDB dense index + RRF fusion (seam landed)

## 8. Change log

| Date | Change | Commit |
| ---- | ------ | ------ |
| 2026-07-23 | Initial skeleton landed with Phase 8 docs drop. | (this turn) |
| 2026-07-23 | `feat(search-core)` — Phase 1 landed. | `d9e2856` |
| 2026-07-23 | `feat(search-rag)` — Phase 5 landed. | `7a99470` |
| 2026-07-23 | `feat(search-evals)` — Phase 6 landed. | `044dee2` |
| 2026-07-23 | `feat(search-index)` — Phase 3 landed. | `727234b` |
| 2026-07-23 | `feat(search-tags)` — Phase 2 landed. | `4500e76` |
| 2026-07-23 | `feat(search-ingest)` — Phase 4 landed. | `a2cc835` |
| 2026-07-23 | `feat(cli)` — Phase 7 landed. | `232f2c2` |
| 2026-07-23 | `feat(search-skills)` — built-in AgenticFitRerank + LlmSummarizer + default registry. | `38ea86d` |
| 2026-07-23 | `docs(adr)` — ADR-037 Model Explorer search layer. | `2df9ebf` |
| 2026-07-23 | `docs(operations)` — Model Explorer operator runbook. | `9c32155` |
| 2026-07-23 | `docs(web)` — `apps/model-explorer` README. | `5e46d79` |
| 2026-07-23 | `feat(mcp)` — `hwledger-mcp` JSON-RPC 2.0 server (Phase 9). | `eed7961` |
| 2026-07-23 | `feat(server)` — standalone `hwledger-server` Axum binary (Phase 9). | `273feed` |
| 2026-07-23 | `feat(server)` — Hono proxy in front of Axum for the web app (Phase 9). | `9aaccd1` |
| 2026-07-23 | `feat(web)` — Svelte 5 + SvelteKit three-pane search UI (Phase 10). | `e0db83f` |
| 2026-07-23 | `style(mcp)` — rustfmt pass on `hwledger-mcp`. | `b9520a1` |
| 2026-07-23 | `chore(search-index)` — introduce `IndexedDoc` payload struct + autograder cleanups (PathBuf→Path, let-unit-value, if-same-then-else, doc links); gates green. | (this turn) |