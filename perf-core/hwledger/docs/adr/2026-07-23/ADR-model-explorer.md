# MADR 3.0 — ADR-037: Model Explorer Search Layer in hwLedger

> **Status:** Accepted
> **Date:** 2026-07-23
> **Deciders:** hwLedger engineering (KooshaPari)
> **Supersedes:** none
> **Superseded by:** none

## Context and Problem Statement

hwLedger tracks hardware fleet audit and provenance for ML workloads. The
top-level app description (README §"Plans", §"Reconciles") promises two
capabilities that today have no backing surface:

1. **Plan** — VRAM and throughput estimates keyed off a live, *corpus-aware*
   model taxonomy (architecture family, MoE residency, quant format,
   parameter bucket, license, provenance, use-case fit).
2. **Reconcile** — match plan output against an inventory that knows which
   models actually exist on HuggingFace right now, with their metadata,
   tags, and downstream consumers.

The existing Rust workspace only models the math side (the `pheno-capacity`
extraction per ADR-035A) and the inference-runtime side (`hwledger-core`,
`-probe`, `-inference`, `-server`, …). It has nothing that can answer the
question *"what model should I plan for?"* — the inventory layer is missing.

The model-explorer search layer fills that gap: a standalone Rust workspace
that ingests upstream model sources (HF Hub v1), runs heuristic taggers,
indexes them in a BM25 store with structured facets, exposes a CLI / server /
MCP surface, and a pluggable reranker pipeline. The Plan step can then
call `model for-use-case` to get a corpus-grounded candidate list before
applying capacity math.

How should this layer be organized within hwLedger so that it stays
shippable in narrow phases, doesn't pull the rest of the workspace into a
rebuild, and leaves a clean seam for a dense vector index (LanceDB) and a
real embedding backend (ORT)?

## Decision Drivers

- **D2.1 Inventory-first, math-second.** Reconcile needs a corpus; plan
  needs a corpus. We cannot land the Phase-1 math without first answering
  *which model?*.
- **D2.2 Dependency-light core.** The search layer must be importable
  from a synchronous CLI, an async Axum server, and an MCP binary without
  dragging all of Tantivy / LanceDB into `hwledger-search-core`.
- **D2.3 Stable public surface.** Once v1 ships, the entry points
  (`run_hybrid`, `build_seed_index`, the CLI subcommands, the MCP tool
  names) are part of the operator contract. Phased rollout (BM25 today,
  BM25+dense tomorrow) must not force an API churn.
- **D2.4 Determinism for tests.** The vector-stub embedder, the lazy
  populate gate, and the seed builder must all be reproducible across
  runs so CI can catch regressions without hitting the network.
- **D2.5 Phased deferred work.** ORT-based embeddings and LanceDB dense
  indexes are known unknowns with heavyweight native dependencies. They
  must be visible in the contract as Deferred 3 / Deferred 4 so they
  don't surprise the next operator.
- **D2.6 Repo placement.** Stay inside the existing hwLedger monorepo
  under `apps/model-explorer/` rather than spinning up a new repo —
  this is an app-level component, not a substrate lib (ADR-035A).

## Considered Options

| # | Option | Summary |
| - | ------ | ------- |
| 1 | **Single mega-crate** | One `hwledger-model-explorer` crate containing tantivy, lancedb, ort, the CLI, the server, and the MCP surface. |
| 2 | **Service split, shared schema** | One `Rust` binary + one TS/Next.js web app at `apps/model-explorer/web/`, sharing a JSON schema only. |
| 3 | **Workspace of thin crates + dependency-light core (chosen)** | A Cargo workspace with one dependency-light `hwledger-search-core`, five thin feature crates (`-tags`, `-index`, `-ingest`, `-rag`, `-evals`), a skill registry, and three thin front-ends (`-cli`, `-server`, `-mcp`). The web app, when it lands, is a separate consumer. |
| 4 | Standalone new repo | Create a new top-level `hwledger-model-explorer` repo (sibling to `pheno-capacity`). |

## Decision Outcome

**Chosen option: 3 — Workspace of thin crates + dependency-light core.**

Concretely:

- **New top-level app dir:** `apps/model-explorer/` containing a single Rust
  workspace today and a sibling `web/` TypeScript app when the GUI lands.
- **Workspace crate layout (current):**
  - `hwledger-search-core` — taxonomy, traits, RRF, skill registry.
    Dependencies: `serde`, `serde_json`, `anyhow`, `thiserror`. **No
    Tantivy, no LanceDB, no ORT.**
  - `hwledger-search-tags` — heuristic taggers (arch, moe, quant, param,
    license, modelkind, reap, provenance, usecase-fit) + composite
    `tag_all` orchestrator.
  - `hwledger-search-index` — Tantivy BM25 store + collapse rule +
    hybrid query driver (`run_hybrid`). LanceDB is **not** linked.
  - `hwledger-search-ingest` — HF source adapter, seed builder, lazy
    populate cache (`PopulateGate`), v1 neighborhood expansion stub.
  - `hwledger-search-rag` — chunker, deterministic stub embedder
    (`StubEmbedder`, FNV-1a + LCG → `[-1, 1]`), cosine `retrieve`. ORT is
    **not** linked.
  - `hwledger-search-evals` — extractors over an indexed model
    (`model_index`, `card_table`, `readme_results`) for the eval harness.
  - `hwledger-search-skills` — placeholder for the reranker registry
    surface (skill trait lives in `-core`; this crate reserves room for
    built-in skills).
  - `hwledger-cli` — `model …` + `seed …` + `model-ask` subcommands
    (`clap`-driven, `--json` everywhere, comfy-table by default).
  - `hwledger-server` — Axum HTTP surface (placeholder; will bind to
    `run_hybrid`).
  - `hwledger-mcp` — MCP binary (placeholder; will bind to the same
    surface).
- **Operator contract** — subcommand names, the `Query` / `FusedResult`
  shapes, the env vars (`HWLEDGER_INDEX`, `HF_TOKEN`, `HF_HUB_URL`), and
  the `--json` flag are all stable from v1.
- **Deferred but reserved:**
  - **Deferred 3 — ORT-based embedder backend.** The `Embedder` trait
    in `hwledger-search-rag` is the seam. `StubEmbedder` ships today.
    ORT lands when we can justify the native build / signing cost.
  - **Deferred 4 — LanceDB dense index + RRF fusion.** `run_hybrid` is
    BM25-only today; its signature is stable so the BM25 + LanceDB
    fusion can land without an API churn. `rrf_fuse` (`k = 60`) is
    implemented in `hwledger-search-core` and unit-tested but not wired
    into `run_hybrid` until LanceDB exists.

### Consequences

#### Positive

- **P3.1** `hwledger-search-core` stays dependency-light, so the math
  side (ADR-035A pheno-capacity) and the inference-runtime side can both
  consume it without dragging tantivy/lancedb along.
- **P3.2** CLI, server, and MCP share the exact same `run_hybrid` and
  `build_seed_index` calls — three front-ends, one engine.
- **P3.3** Each thin crate is independently testable; CI can fail fast
  on the core trait surface without compiling tantivy.
- **P3.4** The `Embedder` trait and the BM25-only `run_hybrid` give us
  stable seams for Deferred 3 (ORT) and Deferred 4 (LanceDB + RRF).
- **P3.5** Phased bootstrap is possible: Phases 1–6 each shipped as
  one commit (search-core, -tags, -index, -ingest, -rag, -evals, -cli),
  keeping the diff small enough to review in one sitting.

#### Negative

- **N3.6** The BM25-only v1 means `model-ask` and `model for-use-case`
  return useful but not *great* results — semantics is what makes
  those queries sing. Until ORT/LanceDB lands, quality is bounded.
- **N3.7** Seven thin crates is more surface than a single mega-crate.
  New contributors have to learn the layering before they can land a
  change. Mitigated by a one-page module map in `hwledger-search-tags`
  and `hwledger-search-index` lib.rs comments.
- **N3.8** The v1 neighborhood expansion is a stub (`expand_neighborhood`
  returns seeds unchanged). Operators must know this before they wire
  it into a CI job.

#### Neutral

- **Neu3.9** The search layer is **inside** hwLedger (per ADR-035A,
  this stays a federated service). It is not extracted into a substrate
  repo.
- **Neu3.10** The web app at `apps/model-explorer/web/` is a separate
  consumer; it shares the JSON schema but not the Cargo workspace.

## Pros and Cons of the Options

### Option 1 — Single mega-crate

- **Pro:** simplest mental model; one `Cargo.toml`.
- **Con:** every consumer (CLI, server, MCP, future web glue) pulls in
  tantivy + lance + ort regardless of whether it uses them.
- **Con:** build times balloon; cold `cargo check` exceeds the
  phenotype-infra 90-second CI budget.
- **Verdict:** rejected.

### Option 2 — Service split, shared schema only

- **Pro:** cleanest possible front-end / back-end boundary.
- **Con:** Phase 1 has no front-end yet; building the schema first is
  premature.
- **Con:** Doesn't solve the dependency-light math-side problem.
- **Verdict:** rejected for Phase 1; the web app (when it lands) will
  use this split against the `hwledger-server`.

### Option 3 — Workspace of thin crates + dependency-light core *(chosen)*

- **Pro:** Each crate can be reviewed in isolation; `search-core` is
  importable from any other workspace crate.
- **Pro:** Three front-ends (`cli`, `server`, `mcp`) share one engine.
- **Pro:** Reserved seams for ORT and LanceDB without committing to
  them today.
- **Con:** Seven crates is more boilerplate than option 1.
- **Verdict:** chosen.

### Option 4 — Standalone new repo

- **Pro:** Cleanest version isolation; one PR review per repo.
- **Con:** Contradicts ADR-035A — model-explorer is an app-level
  component, not a substrate. The math lib extraction is the only
  thing that warrants a new repo (already done: `pheno-capacity`).
- **Verdict:** rejected.

## Implementation Phases

The phased rollout referenced by this ADR is documented end-to-end in
`docs/superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md`.
Briefly:

1. **Phase 1 — `search-core`** traits, taxonomy, RRF, skill registry.
2. **Phase 2 — `search-tags`** heuristic taggers + composite
   orchestrator.
3. **Phase 3 — `search-index`** Tantivy BM25 store + collapse + hybrid
   query driver.
4. **Phase 4 — `search-ingest`** HF adapter + seed builder + lazy
   populate + v1 expansion stub.
5. **Phase 5 — `search-rag`** chunker + `StubEmbedder` + cosine retrieve.
6. **Phase 6 — `search-evals`** eval extractors over the indexed model.
7. **Phase 7 — `cli`** `model …` + `seed …` + `model-ask` front-end.
8. **Phase 8 — operator runbook** (this ADR's sibling doc,
   `docs/operations/model-explorer-ops.md`).
9. **Phase 9 — server + MCP** thin front-ends over the same engine.
10. **Phase 10 — web app** `apps/model-explorer/web/` (deferred; out of
    scope for this ADR).

## Links

- ADR-0001 — Record architecture decisions (project-wide convention).
- ADR-035A — HwLedger reclassification (federated service).
- `docs/operations/model-explorer-ops.md` — operator runbook.
- `docs/superpowers/specs/acceptance/2026-07-23-hwledger-model-explorer.md`
  — phased acceptance skeleton.
- `apps/model-explorer/web/README.md` — front-end project intro.

## Rich Media Stubs

<!-- RICH-MEDIA-STUB type="annotated-screenshot" subject="Model explorer workspace crate dependency graph" journey="model-explorer-bootstrap" status="TODO" -->
> **[RICH MEDIA PLACEHOLDER]** *Annotated diagram: `search-core` at the centre, `tags`/`index`/`ingest`/`rag`/`evals` as the inner ring, `cli`/`server`/`mcp`/`web` as the outer ring. Show that `search-core` has no tantivy/lancedb/ort edges.*
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="annotated-screenshot" subject="Deferred 3 / Deferred 4 seams" journey="model-explorer-bootstrap" status="TODO" -->
> **[RICH MEDIA PLACEHOLDER]** *Callout boxes: "Deferred 3 — ORT embedder (trait seam in `hwledger-search-rag::Embedder`)" and "Deferred 4 — LanceDB dense index + RRF (seam at `run_hybrid`)".*
<!-- END-RICH-MEDIA-STUB -->