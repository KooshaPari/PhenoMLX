# V2 oracle matrix

| Obligation | Positive oracle | Negative oracle | Evidence |
|---|---|---|---|
| M-OB-021 | migrate old profile receipt preserving historical identity links | migration rewrites old profile ID | schema migration |
| M-OB-022 | capability states carry authority/version/model scope | source declaration rendered verified qualification | schema/integration |
| M-OB-023 | exact tokenizer/template match qualifies | tokenizer/template mismatch reuses qualification | native/integration |
| M-OB-024 | scheduler/concurrency change creates new profile/evidence subject | changed batching treated identical | native |
| M-OB-025 | cold/warm phases explicit | warm result advertised cold | workload fixture |
| M-OB-026 | collector scope/method explicit and reconciled | process memory presented as device total | measurement fixture |
| M-OB-027 | unsupported operation returns structured unsupported | generic exception interpreted profile failure | adapter unit |
| M-OB-028 | degradation/fallback explicit new realized profile | precision/engine silently changes | adapter/native |
| M-OB-029 | workload distribution digest changes when population changes | same aggregate count hides different distribution | deterministic workload |
| M-OB-030 | support lists excluded configurations/limitations | supported boolean implies universal support | schema/query |
| M-OB-031 | extension disables/fails explicitly outside envelope | unsupported extension silently runs | native/integration |
| M-OB-032 | incomparable profiles produce incomparable/explanation | selector forces unexplained winner | selection fixture |

Mutation ideas: drop tokenizer digest; ignore warm/cold; remove authority field; map unsupported→false success; remove exclusion list.
