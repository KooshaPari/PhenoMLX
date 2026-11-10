# Real-class integration review

Date 2026-09-30. Prototype commit `3d90a41366a340fe62a67b6281d2c599c6212db2`.

## What is now integrated
The prototype imports real `LlamaCppBackend` and `VllmBackend` classes without loading models.

It demonstrates:
- two real backend classes expose materially different native capability facts (speculative decode false vs true);
- requested vLLM realized as llama.cpp can be represented explicitly as fallback;
- engine version can remain explicit unknown rather than fabricated.

## Important limitation
These static BackendCapabilities are class declarations, not qualification evidence. They establish adapter semantic differences, not that installed engine/version/model actually supports the capability at runtime.

## Contract implication
No new ontology identity required. SupportEnvelope must distinguish **declared adapter capability** from **verified profile capability** and carry evidence/authority state.

This is analogous to the broader authority doctrine: static source declaration is deterministic source fact, not verified observation.

## CI
No workflow receipt returned for the integration commit at query time. No green claimed.

## Baseline consequence
Cross-engine typed-profile value is stronger: current real adapters expose differences that the common GenerateResponse does not bind to exact profile identity. Real model/runtime qualification remains mandatory.
