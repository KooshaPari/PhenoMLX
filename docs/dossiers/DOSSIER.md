# PhenoMLX Dossier

**Source:** docs-3/products/PhenoMLX/DOSSIER.md (management discussion)
**Created:** 2026-09-16
**Status:** Converged — PR #227 merged, PR #232 pending

---

## 1. Product Identity

| Field | Value |
|---|---|
| Name | PhenoMLX |
| Role | Owned MLX/OMLX inference extension and research product |
| Stable GitHub ID | 1214745478 |
| Class | product-or-lab |
| Seats | 2 (implementation + assurance/comparison) |
| Repository | `~/CodeProjects/Phenotype/repos/phenotype-omlx` |

---

## 2. Current Verified State

| Gate | Status | Evidence |
|---|---|---|
| Build | PASS | Trunk check passes, CI green |
| PR #227 (release-macos.yml) | MERGED | Release workflow on main |
| PR #232 (Entitlements.plist fix) | OPEN | Duplicate network.client key removed |
| Entitlements.plist | CLEAN | 1 network.client, 1 metal |
| Working tree | CLEAN | 0 uncommitted changes |
| Default branch | main | ca182cdf |

---

## 3. Atlas Questions (from docs-3)

- Actual inference paths versus experiments
- Model/quantization/backend compatibility
- Memory/cache lifecycle
- Application injection versus self-contained package
- Quality/performance source and measurement boundary

**For each answer:** identify actual build roots, public entrypoints, owned state, packages/symbols, source anchors, runtime dependencies, native shells and distribution artifacts. Use real manifests and producer receipts.

---

## 4. Carry-Forward Questions

- Prior installation required local oMLX app and adjacent code
- A faster result can be a different quantization or cached workload
- Research placeholders must not be advertised as supported inference

---

## 5. Quality Gates (docs-3 QA)

- 85% structural/behavioral coverage per family (unit, integration, E2E)
- Independent negative controls required
- Clean install outside source tree
- Actual Apple Silicon binary with signing + notarization
- Model/quantization pinning for benchmarks
- TTFT, latency distributions, memory measurements
- Cancel/unload/restart behavior verified

---

## 6. Advantage Hypothesis

A reproducible quality-preserving throughput, memory or workflow improvement over the actual upstream baseline.

---

## 7. Next Deliverables

1. PR #232 merge (Entitlements.plist fix)
2. SHIP.sh operator run for release signing
3. Atlas extraction: map inference paths, model compatibility, memory lifecycle
4. Controlled pilot: cold/warm request sets, cancel/unload/restart
5. Comparative baseline: pinned upstream OMLX

---

## 8. Cross-Ecosystem Acceptance (rev 1.1)

Apply ecosystem-first evolution: inspect owned and external reuse, preserve current consumers, distinguish planned/speculative uses, account for downstream cost.
