# Developer handoff — VS-01 typed two-engine profile

Status: IMPLEMENTATION-READY bounded architecture experiment.

## Goal
Wrap existing BackendBase adapters with typed RequestedProfile/RealizedProfile/QualificationReceipt while preserving backend-specific semantics. Prove with two real engines/common model.

## Allowed production scope
Add profile/evidence types and adapter wrapper. Do not rewrite engine internals/cache/scheduler.

## Required records
RequestedProfile v1; RealizedProfile v1; RuntimeGeneration/effective-config fingerprint; BackendSourceReceipt; QualificationWorkload; QualificationReceipt; SupportEnvelope; CapacitySnapshot/AdmissionDecision; Observability map.

## Phase 1 dependency-light
Wrap existing LlamaCpp/Vllm/Sglang/etc. constructors/capabilities/availability/version. Explicit unsupported/not-observable fields.

## Phase 2 real run
Choose two engines actually installed/supported on same hardware and one exact common model/tokenizer/artifact. Pin versions.

Run matched prompts with:
- cold/warm state declared;
- context lengths;
- at least a small concurrency sweep;
- quality oracle;
- TTFT/elapsed/token counts;
- memory if observable;
- cancel/unload/restart where supported.

## Mandatory adversarial tests
fallback identity; unknown engine version; partial stream; hot-swap generation; support true + admission false; cache-domain mismatch; custom extension quality regression; historical evidence not relabelled; runtime auto-tune/effective-config change creates new observation identity.

## Must not do
- no cross-hardware performance winner;
- no mechanism claim when field is unknown;
- no static BackendCapabilities treated as qualification;
- no custom engine/cache implementation unless experiment proves deficiency;
- no cockpit OK used as acceptance authority.

## Done for VS-01
Two real profile receipts exist under matched workload, schema exposes meaningful backend difference/unsupported fields, lifecycle evidence is captured, and at least one ambiguity of current adapters is eliminated.

Return raw evidence + limitations. Do not claim PhenoMLX complete.
