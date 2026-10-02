# Request lifecycle and persistence consumers — direct mapping update

Date 2026-09-30.

## Current generic Python adapter layer
BackendBase exposes generate/is_available and GenerateResponse(text,tokens,elapsed,backend,metadata). This remains intentionally thin and does not persist request lifecycle/profile identity.

## Stronger specialized persistence exists elsewhere
KernelRegistry owns immutable tuning records and deterministic selection/rejection for kernel candidates. This is extension evidence, not generic inference request persistence.

Historical Qwen runtime envelopes provide artifact-bound runtime evidence, but are file artifacts rather than a general profile registry.

## Tree finding
No obvious dedicated `RuntimeProfile`/`SupportEnvelope` persistent store is present in the current PhenoMLX-specific Python adapter surface. The repository contains many unrelated AgilePlus storage crates; these must **not** be promoted as PhenoMLX profile storage merely because they coexist in the monorepo.

This applies the cross-repo/subsystem authority rule at intra-repo scope: co-location does not imply product ownership.

## Current classification
- request lifecycle: PARTIAL — generate path mapped; typed stream/cancel/generation semantics not generalized.
- kernel extension evidence persistence: HIGH and semantically strong.
- general runtime-profile/support persistence: OPEN.
- historical file evidence envelopes: SUPPORTING/STRONG, not store.

## Architecture consequence
VS-01 should introduce the smallest profile/evidence persistence boundary necessary for the experiment, preferably append/versioned artifacts first, rather than adopting unrelated AgilePlus database machinery by convenience.
