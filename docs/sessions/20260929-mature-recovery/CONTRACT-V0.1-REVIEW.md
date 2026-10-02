# Mature contract v0.1 review and architecture attack

Date: 2026-09-30.

## Coverage reconciliation

v0.1 covers M-OB-001–020 at concept level: profile identity/support, install/invocation, matched qualification, memory/quality/lifecycle, bootstrap-before-custom, negative evidence, capacity, requested/realized, topology, cache domain, generation, stream state, admission, operating state, support lifecycle and observability.

Under-specified:
1. installation/distribution candidate identity is in CVP but not mature spine;
2. update/rollback and profile migration need stronger contract language;
3. security principal/authorization for CacheDomain is implied but not explicitly modeled as shared IAM delegation;
4. qualification must explicitly distinguish observed outcome claims from mechanism claims when ObservabilityLevel is limited;
5. backend adapter should not imply every engine can expose every operation.

## Alternative architecture attack

### A — universal PhenoMLX inference engine
Own scheduler/cache/kernels across hardware.
**Can satisfy:** yes in theory.
**Rejected as default hypothesis:** enormous duplication; SOTA falsifies generic necessity.

### B — typed adapter/control library
Thin library/daemon maps native engines into profile/support/evidence schema.
**Can satisfy:** most mature contract and is preferred hypothesis.

### C — qualification registry only
No runtime control; ingest engine-native benchmark/evidence and publish SupportEnvelopes.
**Can satisfy:** profile qualification/selection but not launch/cancel/unload journeys unless delegated clients are explicitly part of product composition.

### D — OmniRoute/profile metadata owns selection; PhenoMLX only experimental kernels
**Can satisfy some user outcomes:** yes.
**Risk:** splits qualification truth from extension implementation and may make PhenoMLX unnecessary as a product layer.

### E — no PhenoMLX product
Use MLX-LM/vLLM/SGLang/TRT-LLM/llama.cpp directly, with generic benchmark tooling.
**Can satisfy basic serving:** yes.
**Fails current intended thesis if:** no existing system supplies cross-engine typed support/qualification/provenance needed by the ecosystem. This remains to be empirically established.

### F — hardware capability registry + engine adapters
Use hwledger/capacity as hardware truth, engine-native capabilities as software truth, PhenoMLX composes them.
**Can satisfy:** strongly, and current repo already contains both adjacent pieces. This may be better than a monolithic profile registry.

## Falsification finding

v0.1 is architecture-neutral enough to permit B/C/F and product absence E. Good.

But the boundary with **hwledger/OmniRoute/Portage** is under-specified. Without ownership rules, the same RuntimeProfile/SupportEnvelope could be independently reimplemented in several repos.

## Baseline decision

**DO NOT PROMOTE v0.1 yet.**

v0.2 must:
- define ownership/source-of-truth boundaries with hwledger, routing layer and Portage;
- explicitly support delegated operations/not-applicable capabilities;
- strengthen install/update/rollback;
- make observed-outcome vs mechanism-claim scope explicit;
- define cache-domain authorization as delegated IAM relation;
- require a two-engine profile experiment proving cross-engine abstraction adds value.

The existence gate remains open.
