# PhenoMLX Controlled Pilot

## Status: BLOCKED — Operator inputs required

Per `docs-3/products/PhenoMLX/PILOT.json`, execution requires:

| Input | Status | Notes |
|-------|--------|-------|
| Versioned subjects/fixtures | PENDING | Which model + quantization to test |
| Predeclared invariants | PENDING | Quality thresholds, pass/fail criteria |
| Non-inferiority margins | PENDING | Max acceptable degradation |
| Resource/trust limits | ✓ | M1 Pro 16GB, offline after download |
| Independent outcome oracle | ✓ | Automated script + operator spot-check |
| Failed-trial policy | ✓ | All failures retained, never re-run until green |

## Usage

### 1. Start server
```bash
python -m omlx_research.harbor_mlx_server --model <model> --port 8766
```

### 2. Run benchmark
```bash
python pilot/run_benchmark.py
# Or with custom URL:
BASE_URL=http://127.0.0.1:8766/v1 python pilot/run_benchmark.py
```

### 3. Evaluate results
```bash
python pilot/evaluate.py --latest
# Or specific run:
python pilot/evaluate.py pilot/results/<run_id>.json
```

### 4. Upstream comparison
```bash
# Start upstream OMLX server on port 8767
python -m mlx_lm.server --model mlx-community/Qwen3.5-0.8B-OptiQ-4bit --port 8767

# Run comparison (both servers)
python pilot/compare_upstream.py --compare

# Or upstream-only
python pilot/compare_upstream.py --upstream-only
```

### 5. Pin upstream baseline
```bash
git fetch jundot-omlx
git merge-base origin/main jundot-omlx/main
# Update pilot/config.json with the SHA
```

## Protocol

See `docs-3/research/COMPARATIVE-PILOT-PROTOCOL.md`

## Baselines (from PILOT.json)

1. Pinned upstream OMLX
2. Current upstream OMLX
3. MLX-LM serving
4. llama.cpp on supported hardware
5. Other relevant local inference server

## Metrics

1. Quality/non-inferiority (first)
2. TTFT and inter-token latency distributions
3. Sustained tokens/s at declared concurrency
4. Peak/residual memory
5. Time to clean install
6. Recovery after pressure/failure

## Failure Cases to Test

1. OOM returns corrupted output
2. Cancelled request persists indefinitely
3. Wrong cache reuse
4. Model unload leaks residency
5. Version injection fails silently
6. Benchmark compares unequal models
