# Delivered A/B observation, 2026-09-17 12:46 PDT

The later diagnostic completed BOTH ten-prompt conditions at 12:45 PDT. Result: `pilot/results/turboquant_3b_20260917-193300.json`. A/B generation throughput 13.9148 / 3.6775 tokens/s, TTFT 85.145 / 283.760 ms, peak allocated 5.8927 / 5.8927 GiB. Throughput -73.57%, TTFT +233.27%. Hook verification: 72 hooks, 161352 calls, matching all B generated steps. Script SHA256 matches preserved observed source. Logs retained. The remote process continued after SSH ten-minute timeout and finished without a traceback. This disproves an immediate hook crash for the corrected script.

Automated corruption: 0/10 each. Manual review: A0/B1 (B prompt4 fragmented garbage), plus invalid Python in B2/B7 and malformed code/repetition in B8. Full texts retained. Seven outputs per condition hit token caps. These warnings prevent any quality-equivalence claim. Concurrent v2 benchmark PID725848 was observed, so timing is not isolated. QDQ still stores FP16 KV and proves no cache memory savings.

Current canonical script differs from observed source only by atomic checkpoint writer integration (locally tested, not rerun on GPU). Earlier partial evidence retained below with its original observation time. No more remote attempts needed.

---

# TurboQuant CUDA A/B attempt, 2026-09-17

## Outcome: partial, NOT a completed A/B benchmark

Only A_fp16 has ten measured prompts. B metrics are UNKNOWN. No speedup or memory saving can be inferred. Hardware RTX 3090 Ti, torch 2.9.1+cu128, transformers 4.57.1, Python 3.11. Model Qwen2.5-3B-Instruct, 3,085,938,688 parameters (nominal 3B, not strictly <3B). Exact snapshot aa8e72537993ba99e69dfaafa59ed015b17504d1 loaded offline.

| Observed 2026-09-17 11:17 PDT | A FP16 | B 4-bit QDQ |
|---|---:|---:|
| Prompts | 10/10 | UNKNOWN |
| Weighted generation tokens/s | 14.9073 | UNKNOWN |
| Weighted decode tokens/s | 14.9188 | UNKNOWN |
| Mean TTFT | 79.363 ms | UNKNOWN |
| Peak allocated GPU GiB | 5.8927 | UNKNOWN |
| Peak reserved GPU GiB | 6.0391 | UNKNOWN |
| Heuristically corrupted | 0/10 | UNKNOWN |

Artifact: `pilot/results/turboquant_3b_20260917-181700.json`. Retrieved bytes preserved unchanged. Its absence of B means partial despite historical filename. Exact measured script preserved in `artifacts/observed_benchmark.py`; its SHA256 matches artifact. Current canonical script adds atomic checkpoint publication and was only syntax-tested after that change.

## Research and decisions

The port requires flat float32 CUDA data and explicit n before bits in decode. Early coordinator script omitted n, shape/dtype restoration and genuine TTFT. A subsequent attempt hit NameError orig_dtype during hook warmup. Corrected qdq function preserves shape/device/dtype, pads to group32 and uses proper decode arguments. CUDA tests passed four shapes including constant group, padded short tensor, and nonidentity negative control. FirstTokenClock measures the first generated token in the SAME generate call; no second prefill. Both conditions have two warmups, identical greedy generation, caps 160/300/420 and full texts. Prompts use explicit user list because pilot/config.json contains different prompts.

QDQ does NOT compress resident KV storage. K/V projection outputs are quantized and immediately restored to FP16. Peak allocated memory is PyTorch-owned allocations, not total device-wide usage. Streamer token transfer overhead exists identically in both conditions. Single A then B pass cannot establish statistical significance or production readiness.

Manual review of all A texts found no empty, garbage, repetition spam or wrong-language output. Several hit caps (7/10) and are incomplete. Factual accuracy and code correctness were not certified. Corruption zero is not a semantic-quality gate.

## Failures and limitations

Repeated SSH/SFTP disconnects interrupted observability. Legacy scp -O successfully transferred source and baseline result. The corrected run emitted CUDA QDQ SMOKE PASS, loaded both shards and saved A. SSH disconnected and the process was absent afterward. No B traceback captured, so cause is UNKNOWN, not proven hook failure. Last detached launch PID 726004 had empty logs and was absent on observation. No more retries are scheduled. Earlier 7B run.log was stale and unrelated, not evidence for this run. Mac Darwin arm64 cannot locally execute this CUDA-only port.

## Validation and next steps

Atomic writer tests: A-only partial does not publish final filename; B9/10 negative control remains partial; B10/10 publishes final; NaN failure preserves previous file and removes temp. Both canonical Python files syntax-compile. These are local synthetic publication checks, not CUDA acceptance. CLI does not expose this standalone benchmark workflow.

Dependency: restore stable desktop execution/log ownership -> capture exact B warmup outcome -> ten B measurements in matched full A/B -> review complete JSON -> publish measured comparison. Start 03:42 PDT, last observation ~12:31 PDT. Elapsed ~8h49 wall-clock including harness inactivity. Finish UNKNOWN.

No screenshot taken, no model download intended by corrected script, offline env forced and direct cached checkpoint passed. No claim that original early script network behavior was verified. No unrelated untracked files included.
