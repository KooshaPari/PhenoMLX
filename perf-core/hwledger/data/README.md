# Unified Benchmark Data

Centralized location for all hwLedger benchmark data, replacing the previously
scattered `.runs/` and `fixtures/` directories.

## Directory Structure

```
data/
├── benchmarks/         # Scorecards and benchmark summaries
│   └── audit_scorecard.json
├── fixtures/           # Static test fixtures and eval reports
│   ├── eval_report_mini.json
│   ├── harbor_oracle_results.json
│   ├── smoke_lint_demo.json
│   ├── smoke_results.json
│   └── smoke-results.json
└── runs/               # Run output artifacts
    └── run-20260724-014524-multi-variant.json
```

## Source Locations (Original)

| File | Original Location |
|------|-------------------|
| `runs/*.json` | `apps/bench-matrix/.runs/` |
| `fixtures/smoke-results.json` | `apps/bench-matrix/.runs/smoke-results.json` |
| `fixtures/*.json` | `sidecars/bench-cockpit/fixtures/` |
| `benchmarks/audit_scorecard.json` | repo root `audit_scorecard.json` |

## Symlinks

`apps/bench-matrix/.runs` → `../../data/runs`  
Existing code that reads from `.runs/` continues to work without changes.

## Adding New Benchmarks

1. Place run outputs in `data/runs/`
2. Place fixtures in `data/fixtures/`
3. Place summaries/scorecards in `data/benchmarks/`
4. Update this README if adding new sources
