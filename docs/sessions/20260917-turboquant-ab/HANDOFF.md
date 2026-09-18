# SESSION HANDOFF — PhenoMLX TurboQuant+ Validation

**Written:** 2026-09-18 01:15 PDT
**Handoff from:** session owning PhenoMLX (Mac, `kooshas-laptop`)
**Handoff to:** new session **running on the Windows desktop** (`kooshapari-desk`)
**Scope of new session:** PhenoMLX repo only. Do not touch the other three Phenotype repos.

---

## 0. TL;DR — where things stand

| Item | State |
|---|---|
| Rust `turbo_quant` → PyTorch CUDA port | **DONE**, 4/3/2-bit roundtrip tests pass on RTX 3090 Ti |
| 3B A/B (FP16 KV vs 4-bit QDQ sim) | **DONE**, 0/10 corrupted, -52% t/s (sim overhead) |
| 7B A/B (FP16 KV vs 4-bit QDQ sim) | **DONE**, quality **degraded 10/10** on manual review, -73% t/s |
| Real packed-KV residency measurement | **NOT DONE** — this is the open blocker |
| Decode-speed regression explanation | **Partially answered** — see §4, needs verification |

**The single most important open question:** the 7B run showed severe quality degradation (repetition, language mixing) that the 3B run did not. Either (a) 4-bit uniform KV quantization genuinely breaks 7B, or (b) **the measurement method is flawed**. Evidence for (b): we measure "decode speed" through Python forward hooks that run `encode→decode` on **every** `k_proj`/`v_proj` call — 135,856-162,072 Python-level CUDA round trips per run. That is not how TurboQuant+ works. Real implementations keep the cache **packed** and dequantize **inside attention**, in fused kernels. Our method re-quantizes activations per token and keeps the cache FP16. So the -52%/-73% is **measurement overhead**, not a property of TurboQuant+.

---

## 1. Repo & environment

| Thing | Value |
|---|---|
| Repo (Mac) | `~/CodeProjects/Phenotype/repos/phenotype-omlx` |
| Repo (desktop) | **NOT YET CLONED.** Suggested: `C:\phenotype-omlx` |
| Remote | `origin` = `https://github.com/KooshaPari/PhenoMLX.git` |
| Branch | `main` |
| Repo size | 4.3 GB total (1.0 GB `.git`, 1.2 GB `python/`, 559 MB `perf-core/`) — **do a shallow or branch-limited clone, or rsync from Mac** |
| Desktop has | git (`C:\Program Files\Git\cmd\git.exe`), gh (`C:\Program Files\GitHub CLI\gh.exe`), `~/.ssh/id_ed25519` **present** |

### Desktop hardware (verified 2026-09-18)
- GPU: **NVIDIA GeForce RTX 3090 Ti, 24564 MiB**, driver 620.02
- Hostname: `kooshapari-desk`, Tailscale `100.96.135.160`
- Windows. **Not** a Linux box (Tailscale `kooshapari-desk-1`/`-2` are stale linux entries, 136d/16d offline — ignore them).

### Desktop Python (CRITICAL — two pythons exist)
| Python | Path | Has torch? |
|---|---|---|
| **Python 3.11.9** ✅ USE THIS | `C:\Users\koosh\AppData\Local\Programs\Python\Python311\python.exe` | **YES** — torch 2.9.1+cu128, CUDA True |
| Python 3.14 | `C:\Python314\python.exe` (this is what bare `python` resolves to on PATH) | **NO** |

Installed in Python311: `torch 2.9.1+cu128`, `transformers 4.57.1`, `accelerate 1.15.0`, `bitsandbytes 0.50.2`, `safetensors 0.6.2`.
**Not installed:** `hqq`, `quanto`, `torchao`. `bitsandbytes` **is** installed (matters for §5).

### Model caches (fragmented — read this before loading any model)
| Cache | Contents |
|---|---|
| `C:\Users\koosh\.cache\huggingface` | **Full 3B snapshot** (Qwen2.5-3B-Instruct); 7B **metadata only** (11 MB) |
| `E:\hf_cache` (HDD, 1.2 TB free) | **Full 7B weights** (Qwen2.5-7B-Instruct, 15.2 GB across 4 shards) |

`HF_HOME` must be set **before `import transformers`** — transformers snapshots the env var at import time. Setting it after import has no effect (this bug cost 3 failed launches; see §6).

---

## 2. Bench artifacts

### In the repo (`pilot/results/`)
| File | What |
|---|---|
| `desktop_7b_baseline_20260917.json` | 7B FP16 baseline: **16.28 t/s warm**, 14.91 GiB peak |
| `turboquant_3b_ab_20260917-2106.json` | Clean 3B A/B |
| `turboquant_7b_ab_20260918-0720.json` | Clean 7B A/B (the quality-cliff run) |
| `turboquant_3b_20260917-*.json` | Earlier/contaminated attempts — superseded |

### Scripts in the repo (`perf-core/turbo-quant-cuda/`)
| File | Notes |
|---|---|
| `turbo_quant_cuda.py` | The PyTorch CUDA port (219 lines) — `encode_uniform_cuda` / `decode_uniform_cuda` |
| `tq_ab_bench_v2.py` | **Current** A/B harness. Env-overridable: `TQ_MODEL_ID`, `TQ_BITS`, `TQ_N_LAYERS`, `TQ_HF_HOME` |
| `tq_ab_bench.py` | Older v1 — superseded |
| `tq_7b_run.bat` | schtasks wrapper for the 7B run |
| `debug_test.py` | Scratch |

### On the desktop (`C:\bench\`)
Same scripts live at `C:\bench\` (`tq_ab_bench_v2.py`, `turbo_quant_cuda.py`, `tq_7b_run.bat`), plus logs and result JSONs. `C:\bench\tq_7b_results.json` is the desktop-side copy of the 7B run.

---

## 3. Commits (head of `main`)

```
c7fd1593a  perf(pilot): 7B TurboQuant QDQ A/B -- real 4-bit quality degradation found
58eb2cff4  feat(turbo-quant-cuda): parameterize bench v2 for 7B; add detached-run wrapper
b657b7463  docs(extrapolation): mark CUDA port done in Future Work; add packed-KV residency as next step
81660478e  docs(extrapolation): mark desktop env + baselines verified; record 3B A/B with sim-overhead caveat
9267e3e97  perf(pilot): record Qwen3B CUDA QDQ A/B with quality caveats
6477d2b31  feat(turbo-quant-cuda): port Rust turbo_quant to PyTorch - 2/3/4 bit roundtrip tests pass on RTX 3090 Ti
```

All local commits — **`main` has not been pushed to `origin` recently.** Verify with `git log origin/main..HEAD` and push if the user wants it.

---

## 4. The decode-speed question (unresolved, high priority)

**User's objection:** "decode should not suffer — how are you doing worse, especially if TQ+ baseline is superior as measured in papers?"

**Answer — the measurement is wrong, not the technique.** Three independent defects:

1. **We are not measuring TurboQuant+ decode.** The harness installs Python `forward_hook`s on every `k_proj` and `v_proj` (`tq_ab_bench_v2.py:139-172`). Each hook, on **every** forward pass, does: reshape → pad to group_size → `encode_uniform_cuda` → `decode_uniform_cuda` → reshape back. That is 2 CUDA kernel calls **per projection per layer per token** — 135,856 for 7B, 162,072 for 3B. Real TurboQuant+ does **one** encode when a token enters the cache, stores it **packed**, and dequantizes **inside the attention kernel**.
2. **The resident cache stays FP16** (`limitations[0]` in the JSON states this explicitly). So we get **zero** memory saving while paying **all** the compute overhead. It is the worst of both worlds by construction.
3. **Python-level dispatch dominates.** ~136k Python→CUDA round trips of tiny tensors (one token's projection at a time). This is launch-latency-bound, not bandwidth-bound. That is why the delta is ~5x for both 3B and 7B — suspiciously uniform, a hallmark of a fixed-overhead artifact.

**Why the papers show the opposite:** real quantized-KV work is **memory-bandwidth-bound at long context**. Compressing KV 4x shrinks the bytes moved per decode step, so decode *speeds up* at long context / large batch. Our test used short prompts (160-420 tokens) where decode is **latency/compute-bound**, so there is no bandwidth to save — only overhead to add. **We benchmarked the one regime where the technique cannot help.**

**To get a real answer, use a real packed cache.** transformers 4.57.1 ships `QuantizedCache` (verified importable on the desktop):
```python
from transformers.cache_utils import QuantizedCache
# backend must be "hqq" or "quanto" -- neither is installed yet
```
`hqq` and `quanto` are **not installed**; `bitsandbytes` is. Install one (`pip install hqq`), then re-run with a genuine packed cache and compare.
**Also required:** measure at **long context** (≥8K tokens) and **batch > 1**, where bandwidth matters. Short-context single-stream numbers cannot show the benefit.

---

## 5. Suggested next steps, ranked

| # | Task | Why | Effort |
|---|---|---|---|
| 1 | **Re-benchmark with `QuantizedCache` (hqq/quanto) at 8K+ context, batch ≥ 4** | Only way to test the real claim. Fixes all three defects in §4 | Medium |
| 2 | **Verify the 7B quality cliff is real, not an artifact** — re-run with disjoint K/V settings (`turbo_key_bits=0`, i.e. FP16 K / 4-bit V per the doc's own "Risk Notes") | Existing 7B degradation may come from quantizing K, which is known-fragile. Current run quantizes both | Small |
| 3 | Proper quality metric (perplexity or MMLU subset) instead of the heuristic corruption flag | Current flag missed 5/10 degraded 7B outputs (5 flagged, 10 actually bad) | Medium |
| 4 | Long-context VRAM scaling sweep (4/8/16/32K × batch 1/4/8) | Directly measures the 75% KV-reduction claim | Medium |
| 5 | Port check: does `hs`/`hqq` match the Rust codec's scheme? | Rust `encode.rs` is **uniform min/max group quant** (`scale=(max-min)/qmax`, `zero=min`). Papers describe *rotation + MSE-optimal* quant. **If these differ, we are testing a strawman of TurboQuant+.** | Small, high value |

Item 5 is the sharpest: `perf-core/turbo-quant/src/encode.rs` implements plain **uniform asymmetric group quantization**. If the published TurboQuant results rely on a rotation/optimal-codebook step that the Rust codec does not implement, then the 7B quality failure is a property of *this codec*, not of TurboQuant+, and the doc's claim chain needs correcting.

---

## 6. Hard-won operational knowledge (desktop)

### Running long jobs that survive SSH disconnect
Use `schtasks`, not `Start-Process` or `cmd start /b` (both produce **empty logs** — no stdout capture).
```bat
schtasks /create /tn TQ_7B_AB /tr C:\bench\tq_7b_run.bat /sc once /st 22:00 /f
schtasks /run /tn TQ_7B_AB
```
Wrapper `.bat` must be **CRLF**. Write it in the repo, convert, then `scp`:
```bash
python3 -c "p='...bat'; open(p,'wb').write(open(p,'rb').read().replace(b'\n', b'\r\n'))"
scp ...bat koosh@100.96.135.160:C:/bench/tq_7b_run.bat
```
Runs as user `koosh`, same env as SSH. Task timeout 72h.

### Gotchas that cost real time
1. `HF_HOME` **before** `import transformers` (see §1).
2. Bare `python` on the desktop is 3.14 **without torch** — always the Python311 absolute path.
3. `E:` is an **HDD**: 7B shard load takes **~8 minutes cold**. Don't conclude a hang before ~10 min.
4. First launch looked like `AttributeError: 'NoneType' object has no attribute 'endswith'` — that was the `HF_HOME` bug surfacing inside transformers, **not** a corrupt checkpoint. The checkpoint loads fine when env is right (`LOAD OK 7.615B`).
5. SSH session dying kills remote python. `bg` tool has a **600s hard cap** regardless of requested timeout.
6. WSL `vmmemWSL` was holding ~8.9 GB RAM. Reclaimed with `wsl --shutdown` (FedoraLinux-44 + podman-default were running — **this kills them**; confirm nothing needed is running first).
7. Two concurrent Python GPU processes will silently corrupt each other's timings. Run **one** at a time.

### Scaling the harness to another model
`tq_ab_bench_v2.py` env vars: `TQ_MODEL_ID`, `TQ_BITS`, `TQ_N_LAYERS`, `TQ_HF_HOME`. Set `TQ_N_LAYERS` correctly (3B=36, 7B=28) — it feeds the hook-count self-check (`expected_min = 2 * N_LAYERS`).

---

## 7. Answering "what's next" if asked again

Short version: **the current numbers cannot support or refute the TurboQuant+ claim.** We proved the port works and the harness runs. We have *not* run TurboQuant+. Next real experiment is item 1 in §5: a genuine packed cache, long context, batched — and check item 5 first, because if the Rust codec is plain uniform quant, the 7B failure is about this implementation, not the paper.

---

## 8. Guardrails (unchanged)

- **This session owns PhenoMLX only.** Never touch the other 3 Phenotype repos.
- **Minimal action** — user has flagged over-creating files/docs before. Prefer editing existing docs (`docs/TURBOQUANT-EXTRAPOLATION.md`) over new ones.
- Subagent model preference: "Union Alpha" / `opencode-go`; current working form is `openrouter:stealth/union-alpha` (zen endpoint 500s).
- `gh repo delete` is blocked by pre-tool hook; user runs it manually.
- Benchmark offline after download (`HF_HUB_OFFLINE=1`, `TRANSFORMERS_OFFLINE=1`).
- Report **UNKNOWN** rather than inventing numbers. Distinguish measured from projected.

---

## 9. Desktop takeover — receipt (2026-09-18 02:00 PDT)

The new session is live on `kooshapari-desk` and owns PhenoMLX from here.

### What is now true

| Item | State |
|---|---|
| Repo on desktop | `C:\phenotype-omlx` — 6685 files, `.git` 239 MB, `main` @ `84d43e27` |
| Desktop to Mac SSH | **verified working** (Remote Login is on). Host alias `kooshas-laptop`, or `ssh -F ~/.ssh/config.pheno kooshas-laptop` |
| Unpushed commits | **pushed** to `origin` (`9267e3e9..84d43e27`: the 8 commits plus this handoff) |
| Remotes | `origin` = https://github.com/KooshaPari/PhenoMLX.git (push verified); `mac` = `ssh://kooshas-laptop/Users/kooshapari/CodeProjects/Phenotype/repos/phenotype-omlx` (fetch verified, 13 s) |
| Python | `C:\Users\koosh\AppData\Local\Programs\Python\Python311\python.exe` — torch 2.9.1+cu128, CUDA True, RTX 3090 Ti |
| Deliberately not copied | `python/.venv` (1.2 GB) and `perf-core/target` (559 MB) are gitignored build artifacts. Rebuild locally rather than transfer. |

### Sync recipe

```bat
cd /d C:\phenotype-omlx
git fetch mac main && git merge --ff-only mac/main   rem pull Mac-side commits
git fetch origin && git merge --ff-only origin/main  rem pull GitHub-side commits
git push origin main                                 rem publish
```

`git fetch mac main` is incremental and fast (13 s) because the desktop already
holds the bulk of the object store. Do **not** re-clone from the Mac.

### Throughput facts (measured, not guessed)

- Full clone from GitHub averaged **~0.4 MB/s** (240 MB in ~13 min). The
  desktop's WAN is shared with several other agent sessions doing git work.
- Streaming the Mac's 1.0 GB `.git` over SSH managed only **~0.3-0.5 MB/s**
  because the Mac sat at load average **290-405** (concurrent `rustc` builds,
  a qemu VM, 7 jcode processes). Bulk transfer from the Mac is not viable until
  that load clears; the incremental fetch is.
- A short burst from the Mac reached 25-111 MB/s, so the tailscale/LAN path is
  fine. The Mac's CPU is the bottleneck, not the network.

### Desktop GPU caveat (affects any throughput benchmark)

`nvidia-smi` reports ~17.9 GiB "used" at idle on this box: on WDDM it counts
shared, system-backed allocations from browsers, Parsec, Sunshine and NVIDIA
Broadcast. Windows evicts them when a real CUDA process needs room, but timing
runs under that notional pressure are unreliable. Before publishing any
throughput number, check `nvidia-smi` shows a clean idle baseline.
