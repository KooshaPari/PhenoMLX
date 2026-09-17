# Handoff: PhenoMLX

**Date:** 2026-09-16
**Status:** Phase B in progress

## Repo
- Path: `~/CodeProjects/Phenotype/repos/phenotype-omlx`
- Branch: main (e1f2840cd)
- Remote: https://github.com/<REDACTED>/PhenoMLX-temp (temp — original accidentally deleted, awaiting GH support restore)

## Current State
- Build: PASS
- Release workflow: ON MAIN (PR #227 merged)
- Entitlements.plist: PR #232 MERGED (e1f2840cd)
- Atlas extraction: COMPLETE (docs/dossiers/ATLAS_EXTRACTION.md, 459 lines)
- Working tree: CLEAN (1 untracked: docs/dossiers/)
- Controlled pilot: BLOCKED (needs versioned subjects, invariants, oracle)

## What Was Done This Session
1. PR #232 merged — Entitlements.plist duplicate network.client key fix
2. Atlas extraction — 38+ files read, 5 sections mapped with source evidence
3. Peacock fixed PhenoShared vendor regression (separate chat)
4. Mouse working on HeliosLab TS regression (separate chat)

## Phase B Next Steps
1. Controlled pilot setup — needs versioned subjects, predeclared invariants, resource limits, independent oracle
2. python/ffi/src/lib.rs (537 lines) — over 500-line limit, needs decomposition
3. Release signing — operator action: create + push tag to trigger release-macos.yml

## Key Files
- docs/dossiers/DOSSIER.md — product dossier
- docs/dossiers/ATLAS_EXTRACTION.md — Phase B atlas extraction
- .github/workflows/release-macos.yml — release workflow
- Entitlements.plist — signing entitlements

## Carry-Forward Gaps (from Atlas Extraction)
- No model unloading mechanism
- No memory pressure handling
- No OOM fallback
- No .app bundle yet (raw binary + DMG only)
- Qwen3.5 only (0.8B default, 4B secondary)

## docs-3 Reference
- Management dossier: ~/Downloads/docs-3/products/PhenoMLX/DOSSIER.md
- Pilot spec: ~/Downloads/docs-3/products/PhenoMLX/PILOT.json
- Quality gates: ~/Downloads/docs-3/qa/
- Ecosystem rules: ~/Downloads/docs-3/architecture/ECOSYSTEM-FIRST-EVOLUTION.md
