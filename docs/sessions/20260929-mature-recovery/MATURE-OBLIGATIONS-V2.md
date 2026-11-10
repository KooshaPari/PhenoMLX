# Mature obligation expansion v2

Status: PROVISIONAL decomposition beyond M-OB-001..020.

| ID | Obligation | Journey/stage | Quality | Verification |
|---|---|---|---|---|
| M-OB-021 | Profile schema/version migration preserves historical profile identity and support evidence. | J02/J11; MVP+ | Q07/Q08/Q10 | old receipt migration fixture |
| M-OB-022 | Adapter capability values carry authority/source and may differ by engine version/build/model. | J02/J05/J09; CVP | Q07/Q10 | source-declared vs verified capability fixture |
| M-OB-023 | Model/tokenizer compatibility is explicit; tokenizer or chat-template mismatch cannot reuse qualification. | J02/J04/J09; CVP | Q01/Q08 | tokenizer/template mismatch negative |
| M-OB-024 | Scheduler/concurrency policy participates in profile identity when it changes observable behavior/resource use. | J03/J09; CVP | Q02/Q03/Q04/Q08 | scheduler change → new profile/assessment |
| M-OB-025 | Prefix/cache warmth state is declared per qualification phase; warm evidence cannot silently qualify cold behavior. | J09/J10; CVP | Q02/Q04/Q08 | warm/cold paired workload |
| M-OB-026 | Resource measurements define scope/collector method and distinguish process allocation from device/global allocation. | J05/J09; CVP | Q04/Q07 | dual collector reconciliation |
| M-OB-027 | Unsupported operation returns structured unsupported state, not generic runtime failure. | J02/J03/J06/J07; CVP | Q05/Q07/Q10 | capability-negative fixtures |
| M-OB-028 | Adapter fallback/degradation policy is declared per operation and cannot silently change precision/model/engine. | J03/J04/J09; CVP | Q01/Q05/Q07 | precision/engine fallback negatives |
| M-OB-029 | Qualification evidence includes exact workload population/distribution, not only aggregate prompt count. | J09/J10; CVP/MVP | Q01/Q02/Q03/Q08 | workload digest/distribution mutation |
| M-OB-030 | Support decision records limitations and excluded configurations, not only a boolean/lifecycle label. | J02/J09/J11; MVP+ | Q07/Q10 | exclusion query fixture |
| M-OB-031 | Extension compatibility declares engine/model/hardware bounds and safe fallback/disable behavior. | J10/J11; MVP+ | Q05/Q11 | unsupported extension profile fixture |
| M-OB-032 | Profile selection/ranking exposes objective/constraints and never converts incomparable profiles into an unexplained winner. | J02/J10; Beta+ | Q01/Q10 | incomparable selection fixture |

## Suggested next slices
VS-02 profile schema/capability compatibility (021/022/023/027/028).
VS-03 qualification measurement rigor (024/025/026/029).
VS-04 support/extension/selection lifecycle (030/031/032).
