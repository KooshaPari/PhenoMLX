# PhenoMLX Controlled Pilot

## Status: BLOCKED — Operator inputs required

Per `docs-3/products/PhenoMLX/PILOT.json`, execution requires:

| Input | Status | Notes |
|-------|--------|-------|
| Versioned subjects/fixtures | PENDING | Which model + quantization to test |
| Predeclared invariants | PENDING | Quality thresholds, pass/fail criteria |
| Non-inferiority margins | PENDING | Max acceptable degradation |
| Resource/trust limits | PENDING | Hardware constraints, security bounds |
| Independent outcome oracle | PENDING | Who/what validates results |
| Failed-trial policy | PENDING | How failures are retained/reported |

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
