# Quality overlays v0

| Q-ID | Property | Applies to | Candidate acceptance |
|---|---|---|---|
| M-Q-01 | Output quality/non-inferiority | Qualification/Extension | fixed oracle/tolerance; quality hard gate before perf win |
| M-Q-02 | Latency | profile/workload | TTFT/ITL distributions under declared context/concurrency/cache state |
| M-Q-03 | Throughput/concurrency | profile | request/token throughput plus saturation/fairness |
| M-Q-04 | Memory correctness | ResourceState | peak/steady weights+state+workspace+reserve+offload accounted |
| M-Q-05 | Lifecycle safety | cancel/unload/restart/hotswap | no unrelated corruption/leak; generation/state identity preserved |
| M-Q-06 | Isolation/security | CacheDomain/StateStore | no unauthorized cross-domain state reuse |
| M-Q-07 | Observability truthfulness | profile/evidence | measured/declared/imported/unknown preserved |
| M-Q-08 | Reproducibility | QualificationAssessment | pinned artifacts/build/workload or explicit uncertainty |
| M-Q-09 | Availability/recovery | runtime | explicit failures/fallback; recovery does not relabel evidence |
| M-Q-10 | Portability | adapters/profile schema | backend-specific semantics preserved without lowest-common-denominator lies |
| M-Q-11 | Maintenance cost | Extension | custom extension justifies ongoing divergence vs engine-native primitive |
| M-Q-12 | Energy/power context | OperatingState | when material/observable, power/thermal state accompanies perf claims |

CVP hard gates: Q01/Q04/Q05/Q07/Q08. Extension promotion additionally gates Q11 and applicable security/perf dimensions.
