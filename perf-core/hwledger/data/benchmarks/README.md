# Benchmark Data

## Available Data

### audit_scorecard.json
- **Source**: repo root `audit_scorecard.json`
- **Type**: Repo-wide audit scorecard
- **Contains**: Overall score, per-area grading, pass/fail indicators

## Related Data (Other Directories)

- `data/runs/` — Raw run output JSONs (multi-variant runs, etc.)
- `data/fixtures/` — Static eval reports, smoke results, harbor oracle outputs

## Usage

```python
import json
from pathlib import Path

DATA_DIR = Path(__file__).parent.parent

def load_scorecard():
    return json.loads((DATA_DIR / "benchmarks" / "audit_scorecard.json").read_text())

def load_fixture(name: str):
    return json.loads((DATA_DIR / "fixtures" / name).read_text())

def list_runs():
    return sorted((DATA_DIR / "runs").glob("*.json"))
```
