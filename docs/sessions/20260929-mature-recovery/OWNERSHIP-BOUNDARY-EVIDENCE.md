# Ownership-boundary evidence update

Recovered ecosystem context materially supports, but does not fully prove, the provisional boundary.

Historical architecture discussions consistently place **hwLedger** as the resource/capacity oracle for VRAM, GPUs, KV occupancy, bandwidth, TTFT/throughput, cache, thermal/power and heterogeneous fleet/cost planning. Runtime/router layers optimize current allocation/routing using infrastructure facts.

Therefore the provisional PhenoMLX boundary is strengthened:
- hwLedger: reusable hardware/fleet/resource observations and planning truth;
- engine adapters: engine-native capability/runtime facts;
- PhenoMLX: LLM-specific RuntimeProfile composition, qualification evidence and SupportEnvelope;
- routing/provider abstraction (OmniRoute/related): consumes qualified profiles plus current economics/policy to select routes; does not redefine qualification evidence.

This is consistent with the older separation between infrastructure facts and runtime/router decisions.

## Remaining falsification requirement
Inspect actual current hwLedger and OmniRoute consumer schemas before declaring this CLOSED. Historical architecture supports the boundary, but current code could have converged differently.

Classification moves from PARTIAL to **PARTIAL-HIGH / historically supported, current-consumer validation open**.
