# Profile persistence / evidence-store closure pass

Date 2026-09-30.

## Existing strong primitive: kernel-registry tuning evidence

`perf-core/kernel-registry` already models immutable TuningRecord evidence:
- CandidateId + KernelKey;
- per-sample latency;
- optional energy/dispatch;
- median/p95/p99/variance;
- compiler + compiler version;
- source revision;
- capture time;
- optional expiry;
- quality attachment.

Selector:
- rejects stale evidence explicitly;
- records no-evidence and capability/dtype/shape rejection reasons;
- can require production quality evidence;
- deterministically orders candidates;
- preserves reference fallback behavior.

This is highly aligned with mature PhenoMLX evidence doctrine.

## Boundary

KernelRegistry is an **extension/kernel tuning evidence store**, not a complete RuntimeProfile/SupportEnvelope store.

It does not by itself identify:
- full model/tokenizer/quantization;
- engine package/build;
- deployment topology;
- runtime generation;
- cache domain;
- scheduler;
- stream/request lifecycle;
- full qualification workload.

Therefore do not stretch it into the universal profile registry without evidence.

## Existing runtime envelope

The Qwen3.5 E3 runtime-state envelope already demonstrates:
- schema version;
- live_verified label;
- timestamp/scope/model;
- synthetic=false;
- concrete state byte metrics;
- artifact SHA-256s;
- release-gate boundaries;
- explicit non-claims;
- canonical hash.

This is another strong reusable evidence-envelope pattern.

## Architecture consequence

A general QualificationReceipt/Profile store should likely **compose/reuse the same evidence principles**, and may reference KernelRegistry records for extension-level claims, rather than replacing KernelRegistry.

Source-family movement:
- evidence/benchmark persistence can move closer to CLOSED for extension/kernel evidence;
- canonical RuntimeProfile/Support persistence remains OPEN/PARTIAL.
