# PhenoMLX — architecture experiments and acceptance oracles

Source `1ab4ed250e6b3a1ed9025a674d4814339d32fcd3`. Only the argv fragment replay was executed here. Native hardware and installed candidate qualification remain blocked.

## Evidence identity and worker independence

A qualification binds product/capability, accepted contract/criterion, source and binary candidate digests, model/tokenizer/quantization, backend/kernel, workload/input/seed, hardware/OS/driver/library profile, cache warmth, verifier policy/version, run/time and raw results. The worker, its messages or a benchmark script's zero exit status cannot substitute for those bindings. Product state survives development effort and worker replacement. Preserve old assessments and append explicit supersession when a candidate changes.

Optimizer feedback may be dense and multidimensional, but missing evidence, collector failure, wrong candidate and critical quality regression cannot average into success. Separate functional, memory/performance, reliability, security, traceability, usable journeys and uncertainty. Slope/asymptote inference requires enough comparable observations; this pass supplies none.

## High-risk architecture experiments

M-X01 clean installation and argv: run native documented CLI/web/admin paths in a disposable qualified host profile with no sibling checkout. Verify web default and explicit port, missing app, wrong Python ABI and unavailable backend. Record actual executable/module identity. The current source-fragment result is a localization aid, not this experiment's completion.

M-X02 matched cache accounting: retrieve the raw Qwen3.5-9B report and exact scripts; reproduce only under current authorized model/hardware policy. Compare full bf16 and packed-plus-decoded resident allocation with same inputs, candidate, process state and scheduler. Include cold/warm prefill and steady decode; count workspace/peak as well as payload. Quality gates precede memory/speed claims. Keep CUDA and Apple profiles separate. Investigate whether any design displaces the decoded prefix rather than storing both.

M-X03 bootstrap before custom kernel: pin current upstream oMLX, MLX-LM and relevant non-Apple alternative, then profile the actual bottleneck and estimate attainable end-to-end payoff. Test upstream options idiomatically as well as in matched mechanism-isolation conditions. No new fused kernel is justified only by a theoretical codec ratio or a smaller source diff.

M-X04 lifecycle and concurrency: interleave requests with shared and different prefixes, cancel one, unload a model, kill/restart the process, and inject memory pressure. Correctness, isolation, bounded cleanup and evidence identity are hard constraints. Retain failing artifacts and intervention time. Never deliberately exhaust a production host.

M-X05 distribution/license lineage: compare the true fork ancestor and current imported paths with their notices; verify clean package/app construction, consumer compatibility and rollback. Current MIT/Apache labels are a trigger to inspect, not a legal verdict.

## Oracle matrix

| Behavior | Positive | Negative/adversarial | Expected verdict boundary |
|---|---|---|---|
| Launch/configuration | Correct explicit/default port and candidate identity | `--port web`, missing app/env, stale installed module | Invalid invocation/profile, never readiness |
| Output quality | Approved fixed-input oracle under chosen model/profile | Changed quantization, wrong tokenizer, corrupted cache, incompatible kernel | Fail or incomparable; no speed reward can compensate |
| Memory claim | Full allocation accounting and residual cleanup | Omit decoded prefix/workspace; compare cold baseline to warm candidate | Reject claim as incomplete/incomparable |
| Cache isolation | Same valid prefix reused and different prefix independent | Stale/mismatched cache, model change, concurrent cancellation | Invalid cache or failed behavior with provenance |
| Recovery | Restart with correct durable configuration and fresh request identity | Old response reused after collector failure, partially persisted state | Not-run/invalid/failure, never inherited pass |
| Worker replacement | New worker continues durable experiment under same approved policy | Worker edits grader/quality gate or removes difficult cases | Scope change invalidates comparison; independent acceptance required |

## Vertical slice and deferred work

A useful first slice is an actual clean install → supported inference → cancellation/unload → restart → independently interpretable quality/resource receipt. Keep machine entrypoints real; a bench-cockpit component without a mounted producer/consumer does not close it. Mature identities and profile contracts exist from this slice onward; later stages widen adapters and quality envelopes.

Full native test generation, comprehensive mapping, publication and any post-build pilot remain separate. No paid compute, model download, shared-environment mutation, release or destructive operation was authorized by this pass. Independent review must seek alternative boundaries and invalid performance conclusions before this specification can become accepted.
