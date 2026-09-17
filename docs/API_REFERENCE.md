# PhenoMLX API Reference

## OpenAI-Compatible Server

PhenoMLX runs an OpenAI-compatible inference server via `harbor_mlx_server.py`.

### Start Server

```bash
python -m omlx_research.harbor_mlx_server \
  --model mlx-community/Qwen3.5-0.8B-OptiQ-4bit \
  --host 0.0.0.0 \
  --port 8766
```

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/chat/completions` | Chat completions (OpenAI-compatible) |
| GET | `/v1/models` | List loaded models |
| GET | `/health` | Health check |

### POST /v1/chat/completions

**Request body:**

```json
{
  "model": "default",
  "messages": [
    {"role": "system", "content": "You are a helpful assistant."},
    {"role": "user", "content": "Hello"}
  ],
  "max_tokens": 512,
  "temperature": 0.7,
  "stream": false
}
```

**Response:**

```json
{
  "id": "chatcmpl-xxx",
  "object": "chat.completion",
  "choices": [
    {
      "index": 0,
      "message": {"role": "assistant", "content": "Hello!"},
      "finish_reason": "stop"
    }
  ],
  "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
}
```

### Streaming

Set `"stream": true` to receive SSE responses:

```
data: {"choices": [{"delta": {"content": "Hello"}}]}
data: {"choices": [{"delta": {"content": "!"}}]}
data: [DONE]
```

## CLI Tools

### omlx-research

Main research CLI. Requires `phenotype-omlx env ready` (auto-activated).

| Command | Description |
|---------|-------------|
| `doctor` | Diagnose runtime environment (Python, MLX, kernels, ABI) |
| `inference` | Run inference with policy selection |
| `bench` | Run benchmarks |
| `eval` | Run evaluation harness (MMLU, GPQA, etc.) |
| `inspect` | Load and validate a model plan JSON |
| `compare` | Side-by-side comparison of two execution traces |
| `evidence` | Generate evidence bundle (plan + validation + trace) |
| `promote` | Validate candidate against gates, write PromotionRecord |
| `quarantine` | Append Hold/Rollback audit entry |
| `gates` | CRUD for per-kernel quality-gate configurations |
| `status` | Show current status |
| `fleet` | Fleet protocol operations |

### Inference Options

```bash
omlx-research inference \
  --prompt "Your prompt" \
  --model mlx-community/Qwen3.5-8B-OptiQ-4bit \
  --policy auto \
  --max-tokens 512 \
  --temperature 0.7
```

**Policies:** `auto`, `mlx`, `metal`, `vllm`, `tensorrt`, `sglang`, `llamacpp`, `fanout`, `spec_decode`, `tidar`

## Rust FFI (perf-core)

### TurboQuant+ Encode/Decode

```rust
// Encode FP16 KV to 4-bit
let packed = turbo_quant_encode(data, group_size, bits);

// Decode back to FP16
let decoded = turbo_quant_decode(packed, scales, zeros, n, group_size, bits);
```

### Python FFI

```python
import perf_core

# Encode
packed, scales, zeros = perf_core.turbo_quant_encode(fp16_data, group_size=32, bits=4)

# Decode
decoded = perf_core.turbo_quant_decode(packed, scales, zeros, n, group_size=32, bits=4)
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PHENOTYPE_OMLX_USE_PYTHON_TQ` | `0` | Use Python TurboQuant instead of Rust SIMD |
| `PORT` | `8766` | Server port |
| `MODEL` | - | Model path or HuggingFace ID |
