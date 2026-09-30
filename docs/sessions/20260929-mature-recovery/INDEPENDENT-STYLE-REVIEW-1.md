# Independent-style semantic review pass 1

Assume v0.3 is wrong.

## Attacks
1. Engine auto-tunes kernel/cache after startup without generation change. VALID clarification: RealizedProfile needs runtime-effective config fingerprint or generation/observation epoch when acceptance-relevant auto-tuning changes.
2. Driver/runtime silently updates between runs. Already Hardware/Engine/OperatingState evidence.
3. Same engine version compiled with different flags/plugins. EngineArtifact build identity covers.
4. Remote provider lies about declared model. Observability/authority handles as imported assertion; outcome claims only.
5. Multi-model server routes request to different model. RealizedProfile per request handles if captured; otherwise evidence invalid/unknown.
6. Shared cache state from prior workload changes performance. QualificationWorkload/cache warmth and CacheDomain cover.
7. Runtime profile is valid but routing policy changes. Routing is consumer; profile qualification remains separate. Good.
8. hwledger disappears. Hardware observations can be direct/imported; source-of-truth delegation is not semantic dependency. Good.

## Finding
Add requirement that acceptance-relevant runtime auto-tuning/effective config be captured as part of RealizedProfile or an explicit RuntimeObservationEpoch.

Verdict: **SEMANTICALLY STABLE WITH ONE CLARIFICATION; native two-engine gate remains.**
