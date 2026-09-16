# PhenoMLX Atlas Extraction

**Date:** 2026-09-16
**Phase:** B (docs-3 atlas program)
**Status:** Extraction complete — source-evidenced, no guesses

---

## 1. Inference Paths

### 1.1 Public Entrypoints

| Entrypoint | Path | Type |
|---|---|---|
| CLI launcher (bash) | `cli/bin/omlx-research` | Shell script, lines 1-99 |
| CLI proxy (bash) | `cli/bin/omlx-cli` | Shell script, 4 lines |
| Python CLI main | `python/omlx_research/cli/__init__.py` | `main()` function, line 87 |
| pyproject console_scripts | `python/pyproject.toml` line 29 | `omlx-research = "omlx_research.cli:main"` |
| FFI entry (Rust) | `python/ffi/src/lib.rs` | pyo3 `#[pymodule]` exposing `_perf` |

### 1.2 Inference Call Graph (Production)

The production inference path traces from CLI through to MLX/Metal execution:

```
User: omlx-research inference --prompt "..." --policy auto
  │
  ├─ cli/bin/omlx-research (bash)
  │   ├─ sources scripts/phenotype-omlx-env.sh (PYTHONPATH setup)
  │   └─ dispatches to: python3 -m omlx_research.cli inference ...
  │
  ├─ python/omlx_research/cli/__init__.py :: main() → cmd_inference()
  │   [cli/__init__.py line 87-98, _cmd_inference.py line 66-99]
  │
  ├─ DispatchPolicy auto-resolves to best backend
  │   ├─ engines/hybrid_dispatch.py :: HybridDispatch._auto_pick()
  │   │   [hybrid_dispatch.py lines 93-116]
  │   │   Preference order: METAL > MLX > (SGLang/VLLM/TensorRT on CUDA) > LLAMACPP
  │   │
  │   └─ Selected backend's .generate(req) is called
  │
  ├─ PRIMARY PATH: MlxBackend.generate(req)
  │   [backends/mlx_backend.py lines 255-293]
  │   ├─ MlxBackend._load() → mlx_lm.load(model_path)
  │   │   [backends/mlx_backend.py lines 213-221]
  │   │   Returns (model, tokenizer) — model is an mlx.nn.Module
  │   │
  │   ├─ Optional: install custom Qwen gated-delta Metal kernel
  │   │   [backends/mlx_backend.py lines 265-279]
  │   │   backends/qwen_gated_delta_kernel.py :: install()
  │   │   [qwen_gated_delta_kernel.py lines 73-125]
  │   │   Replaces mlx_lm.models.qwen3_5.gated_delta_update with
  │   │   a custom Metal compute pipeline at runtime
  │   │
  │   └─ mlx_lm.generate(model, tokenizer, prompt, max_tokens=...)
  │       [backends/mlx_backend.py lines 283-289]
  │       └─ MLX dispatches to Apple Metal via mlx.core internally
  │
  ├─ TURBOQUANT PATH: MlxBackend.generate_with_turbo_cache(req)
  │   [backends/mlx_backend.py lines 295-436]
  │   ├─ from mlx.nn.layers.turbo_kv_cache import (
  │   │     TurboKVCacheLite, make_turbo_cache, compact_turbo_cache)
  │   │   [mlx_backend.py lines 343-347]
  │   │   Source: injected into MLX framework via TurboQuant+ (TheTom fork)
  │   │   or persistent wrapper at ~/.omlx/turboquant-plus/mlx/nn/layers/
  │   │
  │   ├─ turbo_cache = make_turbo_cache(model, bits=4, key_bits=4, boundary=2)
  │   │   [mlx_backend.py lines 364-369]
  │   │   Wraps inner layers as TurboKVCacheLite; boundary layers stay raw
  │   │
  │   ├─ mlx_lm.generate(..., prompt_cache=turbo_cache)
  │   │   [mlx_backend.py lines 383-390]
  │   │
  │   └─ compact_turbo_cache(turbo_cache)
  │       [mlx_backend.py lines 395-409]
  │       Returns bytes_freed; compresses KV from FP16 to 4-bit packed
  │
  ├─ METAL KERNEL PATH: MetalKernelBackend.generate(req)
  │   [backends/metal_backend.py lines 70-122]
  │   ├─ _run_custom_metal_probe(mx) — validates Metal dispatch works
  │   │   [metal_backend.py lines 17-45]
  │   │   Compiles + executes "phenotype_omlx_runtime_probe" kernel
  │   │   via mx.fast.metal_kernel()
  │   │
  │   └─ Delegates to MlxBackend(model_path=self.model_path).generate(req)
  │       [metal_backend.py lines 80-89]
  │       (MLX handles actual model execution; Metal backend adds probe provenance)
  │
  └─ OTHER BACKENDS (Linux/cloud, not Apple Silicon primary):
      ├─ VllmBackend    [backends/vllm_backend.py]   — vllm.LLM(model=...)
      ├─ TensorrtBackend [backends/tensorrt_backend.py] — tensorrt_llm.runtime.ModelRunner
      ├─ SglangBackend  [backends/sglang_backend.py]   — sglang.Runtime(model_path=...)
      └─ LlamaCppBackend [backends/llamacpp_backend.py] — llama_cpp.Llama(model_path=...)
```

### 1.3 Rust FFI Path (perf-core → Python)

```
Python code
  │
  ├─ import _perf (pyo3 module)
  │   [backends/mlx_backend.py lines 82-98]
  │   FFI lib: python/ffi/src/lib.rs, built as _perf (cdylib)
  │
  ├─ _perf.turbo_quant_encode(data, group_size, bits)
  │   [python/ffi/src/lib.rs → python/ffi/src/turbo_quant_ffi.rs]
  │   → perf-core/turbo-quant/src/encode.rs
  │   ARM64 SIMD encode path
  │
  ├─ _perf.turbo_quant_decode(packed, scales, zeros, n, group_size, bits)
  │   [python/ffi/src/lib.rs → python/ffi/src/turbo_quant_ffi.rs]
  │   → perf-core/turbo-quant/src/decode.rs
  │   ARM64 SIMD decode path
  │
  ├─ _perf.PySpecDecodeConfig / _perf.PyDraftMode
  │   [python/ffi/src/lib.rs lines 28-98]
  │   → perf-core/spec-decode/ — speculative decoding engine
  │
  └─ _perf.PyTreePlan(width, depth)
      [python/ffi/src/lib.rs]
      → perf-core/tree-attention/ — tree causal mask computation
```

### 1.4 Inference Path Classification

| Path | Classification | Evidence |
|---|---|---|
| MLX generate | **Production** — primary path on Apple Silicon | `backends/mlx_backend.py:283`, called from CLI `inference --policy mlx` |
| TurboKV cache + compact | **Production** — KV compression after prefill | `backends/mlx_backend.py:295-436`, test at `tests/test_mlx_backend.py:9` |
| Metal probe | **Production** — custom Metal dispatch validation | `backends/metal_backend.py:17-45` |
| Custom Qwen gated-delta kernel | **Experimental** — opt-in via env `PHENOTYPE_OMLX_ENABLE_CUSTOM_QWEN_KERNEL=1` | `backends/qwen_gated_delta_kernel.py:73-98` |
| Rust turbo_quant_encode/decode | **Production** — SIMD fallback for A/B; primary encode path | `backends/mlx_backend.py:101-211` |
| Rust spec-decode engine | **Production** — via pyo3 FFI | `python/ffi/src/lib.rs:155-200`, Rust crate `perf-core/spec-decode/` |
| vLLM / TensorRT / SGLang | **Linux/cloud** — not Apple Silicon primary | `backends/vllm_backend.py`, `tensorrt_backend.py`, `sglang_backend.py` |
| llama.cpp | **Cross-platform** — CPU+GGUF fallback | `backends/llamacpp_backend.py:11-75` |

---

## 2. Model Compatibility

### 2.1 Pinned Models (from `config/smoke_models.json`)

| Role | Model ID | Format | Source |
|---|---|---|---|
| **readiness / default** | `mlx-community/Qwen3.5-0.8B-OptiQ-4bit` | MLX (4-bit OptiQ) | `config/smoke_models.json` → `defaults.mlx_hf` |
| **NIAH** | `Qwen/Qwen3.5-0.8B` | HuggingFace (native) | `config/smoke_models.json` → `defaults.mlx_upstream` |
| **OpenAI compat** | `openai/Qwen3.5-0.8B` | HuggingFace | `config/smoke_models.json` → `defaults.openai_compat` |
| **Legacy (quarantined)** | `mlx-community/Qwen2.5-0.5B-Instruct-4bit` | MLX 4-bit | `config/smoke_models.json` → `legacy_quarantine.qwen25_mlx` |

### 2.2 Model Acceptance Policy

Evidence from `python/omlx_research/smoke_models.py`:

- **Production acceptance requires Qwen3.5** — `assert_qwen35()` function (line 55-78) rejects any model whose id does not contain `qwen3.5` (case-insensitive).
- **Qwen2.5 is quarantined** — requires env var `OMLX_ALLOW_LEGACY_QWEN25=1` to bypass.
- **Bare Qwen3 (without .5) is rejected** — explicit guard at line 62.
- **Env override** via `OMLX_READY_MODEL` — bypasses SSOT for debug purposes.

### 2.3 Quantization Support

From `perf-core/turbo-quant/src/lib.rs` (lines 18-28, 50-65):

| TurboMode | K bits | V bits | Description |
|---|---|---|---|
| `Asymmetric4` | FP16 | 4 | K kept at FP16, V at 4-bit (default) |
| `Symmetric4` | 4 | 4 | K=V=4-bit (turbo4) |
| `Symmetric3` | 3 | 3 | K=V=3-bit (turbo3) |
| `Symmetric2` | 2 | 2 | K=V=2-bit (turbo2) |

- Group size: default 64 (`QuantConfig::default()` line 63)
- Bits validated: `encode_uniform()` panics if bits not in `2..=4` (line 88)
- Encoding: round-to-nearest uniform quantization, ~1.5% PPL hit vs Lloyd-Max (line 84)

### 2.4 Model Architecture Awareness

From `backends/mlx_backend.py :: kernel_plan()` (lines 223-253):

The MLX backend introspects loaded model layers to classify:
- `dense_attention` — standard dense attention layers
- `gqa_attention` — grouped-query attention (detected via `hasattr(layer, 'self_attn')`)
- `deltanet` — linear-recurrent delta layers (detected via `hasattr(layer, 'linear_attn')`)
- `linear_recurrent` — other recurrent layers

This is used for provenance reporting, not dispatch. Qwen3.5 uses a hybrid of GQA and DeltaNet layers.

### 2.5 KV Cache Memory Estimates

From `python/omlx_research/benchmarks/kv_cache_memory.py`:

| Model | Hidden | Layers | KV Heads | Head Dim | FP16 KV @ 32K ctx |
|---|---|---|---|---|---|
| Qwen3.5-0.8B | 2048 | 28 | 8 | 128 | ~2.1 GB |
| Qwen3.5-4B-Coder | 2560 | 36 | 8 | 128 | ~3.6 GB |

Formula: `2 * num_kv_heads * head_dim * dtype_bytes * num_layers * seq_len`

### 2.6 Carry-Forward Questions

- **Other Qwen3.5 sizes?** Only 0.8B and 4B are referenced. 1.5B, 14B, 32B, 72B sizes exist upstream but are not pinned in this repo's SSOT.
- **Non-Qwen models?** Llama/vLLM backends accept arbitrary model paths, but the acceptance policy gates are Qwen3.5-only.
- **Quantized model formats?** MLX community 4-bit (OptiQ) is primary. GGUF via llama.cpp is supported but unpinned.

---

## 3. Memory Lifecycle

### 3.1 Model Loading

| Step | Code | Evidence |
|---|---|---|
| MLX model load | `mlx_lm.load(model_path)` → `(model, tokenizer)` | `backends/mlx_backend.py:216-218` |
| Lazy loading | `MlxBackend._load()` — one-shot, idempotent | `backends/mlx_backend.py:213-221` |
| Error capture | `_load_error` attribute stores exception string | `backends/mlx_backend.py:54,219-221` |

**No explicit model unloading mechanism exists in this codebase.** The `_model` attribute is set to `None` initially and becomes a live `mlx.nn.Module` after `_load()`. There is no `unload()`, `__del__`, or destructor that releases GPU memory.

### 3.2 KV Cache Management (TurboQuant+)

Production cache lifecycle in `generate_with_turbo_cache()`:

```
1. make_turbo_cache(model, bits=4, boundary=2)
   → Wraps inner layers as TurboKVCacheLite
   → Boundary layers (first 2) stay as raw mlx KVCache
   [mlx_backend.py:364-369]

2. mlx_lm.generate(model, tokenizer, prompt, prompt_cache=turbo_cache)
   → Prefill fills the cache with KV tensors
   → boundary parameter controls which layers get TurboKVCacheLite
   [mlx_backend.py:383-390]

3. compact_turbo_cache(turbo_cache)
   → Calls .compact() on each TurboKVCacheLite entry
   → Quantizes KV from FP16 to 4-bit packed
   → Returns total bytes_freed
   [mlx_backend.py:399-409]
```

### 3.3 Cache Compression Evidence Gate

From `benchmarks/qwen35_state_compression.py`:

- **Fail-closed contract** — `CompressionContractError` raised when:
  - Model is not Qwen3.5 (line 34-39)
  - Cache objects are not runtime objects (line 42-47)
  - No measurable runtime state exposed (line 70-73)
  - TurboKVCache not compacted (line 85-88)
  - Compacted state did not reduce bytes vs FP16 baseline (line 142-145)

- **E3 compression measurement** requires paired execution: one FP16 cache + one compacted cache from identical prompt through same model (line 107-117).

### 3.4 Quantization Provenance

From `backends/mlx_backend.py :: quantization_execution_provenance()` (lines 13-31):

- Standalone Rust FFI quantization probe (`_perf.turbo_quant_encode`) does NOT count as execution evidence
- Only observed `TurboKVCacheLite.compact()` calls on the active prompt cache count as verified compression
- `execution_source` is `"turbo_kv_cache"` only when `compressed_layers > 0`; otherwise `"not_executed"`

### 3.5 Memory Pressure Handling

**No explicit memory pressure handling exists in this codebase.**

- No `gc.collect()` calls
- No `torch.cuda.empty_cache()` (NVIDIA path exists but not wired to memory management)
- No OOM handling or fallback logic
- The `memory_profile.py` benchmark uses `tracemalloc` for measurement only (not production use)

### 3.6 Carry-Forward Questions

- **How is GPU memory reclaimed when model is replaced or session ends?** No evidence of cleanup paths. MLX may handle this internally via Python GC, but this is not verified.
- **Multi-model memory budget?** The `HybridDispatch` can have multiple backends loaded simultaneously; no memory ceiling enforcement observed.
- **KV cache eviction?** `compact_turbo_cache` compresses in-place but does not evict. No TTL or LRU eviction for cache entries.

---

## 4. Build & Distribution

### 4.1 Rust Workspace (perf-core)

**Workspace manifest:** `perf-core/Cargo.toml` (26 workspace members)

| Crate | Purpose | Key deps |
|---|---|---|
| `turbo-quant` | CPU SIMD pack/unpack for KV cache quantization | (no external deps beyond serde) |
| `spec-decode` | Speculative decoding engine (SameModel/DraftModel/Medusa) | `turbo-quant`, `tokio`, `async-trait`, optional `metal` |
| `tree-attention` | Tree causal mask for JetSpec draft trees | `serde` |
| `concurrent-exec` | Concurrent agent scheduler (LatentMAS/TiDAR/SSD/JetSpec) | `tokio`, `dashmap`, `uuid` |
| `fleet-proto` | JSON-RPC peer protocol | `serde`, `tokio` |
| `metal-runtime` | Metal pipeline compilation with cache + fingerprinting | `model-plan`, `model-kernels`, `kernel-registry`, optional `metal` |
| `model-kernels` | Model-family kernel packages (attention, MoE, recurrent) | (minimal) |
| `eval-harness` | Evaluation harness (MMLU/GPQA/terminal-bench) | `pyo3` |

**Workspace config:**
- Edition: 2021
- Rust version: 1.74
- Release profile: `opt-level = 3, lto = "fat", codegen-units = 1, strip = true`
- Key workspace dep: `pyo3 0.29` (for FFI), `metal 0.27` (macOS only)

### 4.2 Python FFI (pyo3 bridge)

**Crate:** `python/ffi/Cargo.toml`
- Name: `omlx-research-perf`
- Library name: `_perf` (cdylib + rlib)
- ABI: `abi3-py311` (stable ABI for Python 3.11+)
- Dependencies: `spec-decode`, `turbo-quant`, `concurrent-exec`, `tree-attention`, `fleet-proto`
- Build: `maturin develop --release --features extension-module`

**CI verification:**
- `ffi-ci.yml`: Builds CPython 3.14 ABI3 wheel on ubuntu-latest
- Smoke test imports `_perf`, runs `turbo_quant_encode/decode` round-trip
- Free-threaded CPython 3.14t also tested

### 4.3 Python Package

**Package:** `python/pyproject.toml`
- Name: `omlx-research`
- Version: `0.1.0`
- Python: `>=3.11`
- Build system: setuptools
- Entry point: `omlx-research = "omlx_research.cli:main"`

**Core dependencies:**
- `mlx>=0.31`
- `mlx-lm>=0.31.3` (3.13+ thread-local stream requirement)
- `transformers>=4.40`
- `numpy`, `scipy`, `cryptography>=42`

**Optional dependency groups:**
- `metal`: (empty — MLX is core)
- `mps`: `torch>=2.2`
- `cuda`: `torch>=2.2, vllm, sglang, tensorrt-llm`
- `dev`: `pytest, pytest-asyncio, ruff, mypy, pydantic>=2.0`

### 4.4 macOS Distribution

**Release workflow:** `.github/workflows/release-macos.yml`
- Trigger: tag push `v*.*.*`
- Runner: self-hosted (zero billed minutes)
- Build: `cargo build --release --locked --target aarch64-apple-darwin`
- Signing: "Developer ID Application" identity
- Entitlements: `Entitlements.plist` (network.client + device.metal)
- Output: raw binary + DMG
- Bundle ID: `app.phenotype.omlx`
- No .app bundle yet (Tauri wrapper planned, per workflow comment)

**Static MLX workflow:** `.github/workflows/darwin-py314-mlx-static.yml`
- Runner: `macos-14` (Apple Silicon)
- Python: 3.14
- Pinned versions: `mlx==0.32.0`, `mlx-lm==0.31.3`, `omlx-research==0.1.0`
- Hash-locked requirements: `python/requirements-darwin-py314.txt`

### 4.5 CI Workflows Summary

| Workflow | Trigger | Purpose |
|---|---|---|
| `ci.yml` | push/PR to main | Multi-language detect + lint + test |
| `perf-core-ci.yml` | push/PR (perf-core paths) | Cargo check/test on ubuntu+macos |
| `ffi-ci.yml` | push/PR (ffi/perf-core paths) | pyo3 FFI build + smoke on ubuntu |
| `darwin-py314-mlx-static.yml` | push/PR (python paths) | Static MLX import on macos-14 |
| `release-macos.yml` | tag push | Build + sign + notarize macOS binary |
| `release.yml` | tag push | Generic cargo build + GH release |
| `qwen35-policy-gate.yml` | UNKNOWN | Qwen3.5 policy gate |
| `trunk-check.yml` | UNKNOWN | Trunk check |
| `scorecard.yml` | UNKNOWN | Security scorecard |

### 4.6 Platform Support Matrix

| Platform | Status | Entry Point | Notes |
|---|---|---|---|
| macOS (Apple Silicon) | **Production** | `cli/bin/omlx-research` + `/Applications/oMLX.app` | Primary platform; requires MLX |
| Linux | **Stub** | `linux-client/omlx-research` | PyTorch + CUDA/ROCm fallback |
| Windows | **Stub** | `windows-client/omlx-research.ps1` | Tauri GUI planned |

---

## 5. Runtime Dependencies

### 5.1 Python Runtime Dependencies

| Dependency | Version | Source | Purpose |
|---|---|---|---|
| `mlx` | >=0.31 (pinned 0.32.0 in CI) | `python/pyproject.toml:12` | Apple Silicon ML framework |
| `mlx-lm` | >=0.31.3 | `python/pyproject.toml:15` | MLX language model inference |
| `transformers` | >=4.40 | `python/pyproject.toml:16` | Tokenizer support |
| `numpy` | (any) | `python/pyproject.toml:17` | Array operations |
| `scipy` | (any) | `python/pyproject.toml:18` | Scientific computing |
| `cryptography` | >=42 | `python/pyproject.toml:11` | Security ops |

**Optional (backend-specific):**
- `vllm` — NVIDIA vLLM (cuda extras)
- `sglang` — SGLang (cuda extras)
- `tensorrt-llm` — TensorRT-LLM (cuda extras)
- `torch` >=2.2 — MPS or CUDA backends
- `llama_cpp` — llama.cpp Python bindings (imported at runtime)

### 5.2 Rust Dependencies (perf-core workspace)

| Dependency | Version | Purpose |
|---|---|---|
| `tokio` | 1 (full features) | Async runtime |
| `serde` + `serde_json` | 1 | Serialization |
| `thiserror` | 1 | Error handling |
| `pyo3` | 0.29 (abi3-py311) | Python FFI |
| `metal` | 0.27 (optional) | Apple Metal API |
| `async-trait` | 0.1 | Async trait support |
| `dashmap` | 6 | Concurrent hash map |
| `sha2` | 0.10 | Hashing |
| `proptest` | 1.4 | Property-based testing |

### 5.3 System Dependencies

| Dependency | Required | Source |
|---|---|---|
| Apple Silicon (arm64) | Yes (for MLX/Metal paths) | `mlx.core.metal.is_available()` check |
| Python >=3.11 | Yes | `python/pyproject.toml:9` |
| Rust toolchain | Yes (for building perf-core) | `perf-core/Cargo.toml:28` (`rust-version = "1.74"`) |
| `/Applications/oMLX.app` | Optional (for upstream CLI proxy) | `cli/bin/omlx-cli:4` |
| HuggingFace cache | Yes (for model download) | `mlx_lm.load()` downloads on first run |

### 5.4 Environment Variables

| Variable | Purpose | Source |
|---|---|---|
| `PHENOTYPE_OMLX_HOME` | Repo root | `scripts/phenotype-omlx-env.sh:17` |
| `OMLX_APP` | Path to oMLX.app | `scripts/phenotype-omlx-env.sh:19` |
| `OMLX_READY_MODEL` | Override default smoke model | `python/omlx_research/smoke_models.py:22` |
| `OMLX_ALLOW_LEGACY_QWEN25` | Bypass Qwen2.5 quarantine | `python/omlx_research/smoke_models.py:20` |
| `PHENOTYPE_OMLX_USE_PYTHON_TQ=1` | A/B: use Python turboquant instead of Rust | `backends/mlx_backend.py:64` |
| `PHENOTYPE_OMLX_ENABLE_CUSTOM_QWEN_KERNEL=1` | Enable custom Qwen gated-delta Metal kernel | `backends/metal_backend.py:79` |
| `PYTHONPATH` | Set by env script to include perf-core, TurboQuant+ | `scripts/phenotype-omlx-env.sh:30-83` |
| `PORTAGE_ROOT` | Required for Portage/Harbor integration | `python/omlx_research/smoke_models.py:109-118` |

### 5.5 Carry-Forward Questions

- **Exact MLX version compatibility range?** Pinned 0.32.0 in CI; minimum 0.31 in pyproject.toml. What breaks between versions is not documented.
- **Rust nightly vs stable?** CI uses nightly for perf-core checks, stable for FFI build. The workspace specifies `rust-version = "1.74"` minimum.
- **App bundling?** No .app bundle exists yet. The release workflow produces a raw binary + DMG. The upstream `/Applications/oMLX.app` is a separate installation.

---

## Appendix: Key File Sizes

| File | Lines | Status |
|---|---|---|
| `python/omlx_research/backends/mlx_backend.py` | 436 | Over 350 target, under 500 limit |
| `python/ffi/src/lib.rs` | 537 | Over 500 limit — candidate for decomposition |
| `perf-core/turbo-quant/src/lib.rs` | 316 | OK |
| `perf-core/spec-decode/src/lib.rs` | 188 | OK |
| `perf-core/tree-attention/src/lib.rs` | 323 | OK |

---

## Appendix: Dossier Cross-Reference

- **Management dossier:** `docs/dossiers/DOSSIER.md` (in-repo working copy)
- **docs-3 product dossier:** Not found at `~/Downloads/docs-3/products/PhenoMLX/` — may need to be created for docs-3 Phase B
- **Atlas questions answered:** Inference paths, model/quantization compatibility, memory lifecycle, build artifacts, runtime deps
- **Remaining carry-forwards:** See section-specific carry-forward questions above
