# ADR-035A: HwLedger Reclassification — Federated Service with Extractable pheno-capacity Lib

## Status
Accepted

## Context
HwLedger was added to the Phenotype monorepo via the 2026-06-12 sparse-checkout as an `app-level` repo with no defined substrate placement (ADR-023 Rule 3 default = PAUSED bucket). Three sessions of deferred work (L5-105, "HwLedger reclassification") converged on the 2026-06-18 decision documented here.

HwLedger is a multi-stack federated service for hardware-ledger tracking with:
- **Rust core** (`pheno-memory`, `pheno-memvid`, `pheno-vid` crates) — capacity estimation + memory modeling
- **Native UIs** — macOS (SwiftUI), Windows (WinUI 3.0), Linux (Qt/Slint)
- **Web apps** — `apps/landing` (Astro), `apps/streamlit` (Python Streamlit dashboard)
- **Sidecar** — `sidecars/omlx-fork` (llama.cpp orchestration)
- **Tool** — `tools/journey-remotion` (Remotion-based marketing video generator)
- **Docs site** — `docs-site` submodule

## Decision
HwLedger is classified as **CONDITIONAL** (per ADR-023 § Active/Paused app-level repo triage) and a **federated service** (per ADR-023 § App substrate placement table).

**Rationale:**
1. HwLedger is not a single substrate candidate — its components span 5+ stacks (Rust, Swift, C#, Python, TS) and serve different consumer populations.
2. The Rust math core (capacity / VRAM estimation) is the only genuinely reusable logic; it belongs in a pheno-* lib, not as a federated service primitive.
3. The native UIs, web apps, sidecar, and tools are application-level concerns — they belong with the federated service that uses them.

**Substrate split (extractable):**
- **`pheno-capacity` (NEW, pheno-*-lib tier)** — Extract VRAM estimation rules and capacity math from `apps/streamlit/rules.py` (270 LOC) into a pure-Python lib crate. Pure reusable library, single concern (capacity math), language-specific (Python first, Rust port later).
- **HwLedger (remainder)** — KEEP as federated service. Adopt `pheno-capacity` as a workspace dependency.

**Bucket change worklog entry:** `bucket_change: from=PAUSED to=CONDITIONAL reason=multi-stack federated service with extractable math lib (ADR-035)`

## Consequences

### Positive
- Capacity math becomes reusable across the fleet (other apps that need VRAM / memory modeling can adopt `pheno-capacity` directly)
- HwLedger native UIs and web apps remain unified (single deployment unit)
- ADR-023 Rule 3 P0 deliverable achieved (first concrete reclassification)
- ADR-035A creates the canonical pattern for multi-stack app repos: extract math → pheno-*-lib; keep UI → federated service

### Negative
- New repo `pheno-capacity` requires its own meta-bundle, CI, coverage gate (80% lib bar per ADR-023 Rule 3.1), and worklog v2.1 schema
- Streamlit app migration to use `pheno-capacity` (rather than local `rules.py`) is a breaking change for the dashboard — requires dashboard re-validation

### Neutral
- ADR-035A does NOT delete any HwLedger code; it only adds the `pheno-capacity` extraction as a future migration path
- ADR-035A does NOT change the ADR-023 PAUSED default for new app-level repos (it remains the safe default until app structure is audited)

## Implementation

### Phase 1 — Extract pheno-capacity (P1, ~2h)
1. Create `KooshaPari/pheno-capacity` repo with meta-bundle (AGENTS.md + llms.txt + WORKLOG.md + CHANGELOG.md + LICENSE-MIT)
2. Port `apps/streamlit/rules.py` (270 LOC) into `src/pheno_capacity/` with full docstrings + type hints
3. Write test suite (≥80% coverage per ADR-023 Rule 3.1 quality bar)
4. Add to `pheno-ci-templates` CI matrix
5. Open PR to `pheno-flake` for fleet-wide inclusion

### Phase 2 — Migrate HwLedger streamlit (P2, ~1h)
1. Add `pheno-capacity` to `apps/streamlit/pyproject.toml` as workspace dependency
2. Replace `from rules import ...` with `from pheno_capacity import ...`
3. Validate dashboard regression tests pass
4. Delete `apps/streamlit/rules.py` (now in pheno-capacity)

### Phase 3 — Document + adopt (P3, ongoing)
1. Add `pheno-capacity` to ADR-023 § App substrate placement table
2. Cross-link from HwLedger README + ADR-0001 reference list
3. Track adoption in v9+ fleet audits

## Reference
- ADR-023 (L5-101) — Agent-effort governance, app-level triage + substrate placement
- ADR-029 (L5-104) — Dmouse92 → KooshaPari migration governance
- L5-105 — Worklog entry for this reclassification
- HwLedger/README.md — Top-level app description
- HwLedger/PLAN.md — Component map (referenced by ADR-0001 rich-media stub)

---

## Rich Media Stubs

<!-- RICH-MEDIA-STUB type="annotated-screenshot" subject="HwLedger multi-stack component map with pheno-capacity extraction path" journey="L5-105" status="TODO" -->
> **[RICH MEDIA PLACEHOLDER]** *Component diagram showing Rust core → pheno-capacity extraction; native UIs + web apps + sidecars remaining as federated service.*
<!-- END-RICH-MEDIA-STUB -->

<!-- RICH-MEDIA-STUB type="annotated-screenshot" subject="pheno-capacity API surface (extracted from rules.py)" journey="L5-105" status="TODO" -->
> **[RICH MEDIA PLACEHOLDER]** *Public API of pheno_capacity package (VramRule, CapacityEstimate, ModelProfile dataclasses).*
<!-- END-RICH-MEDIA-STUB -->
