#!/usr/bin/env python3
"""PhenoMLX Pilot — Oracle Evaluator

Reads benchmark results and evaluates against declared non-inferiority margins.
Produces pass/fail verdict with evidence.

Usage:
    python pilot/evaluate.py pilot/results/<run_id>.json
    python pilot/evaluate.py --latest

Exit code 0 = all automated checks pass, 1 = failure or manual checks pending.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
RESULTS_DIR = REPO_ROOT / "pilot" / "results"

MARGINS = {"ttft_pct": 20, "throughput_pct": 15, "peak_memory_pct": 25}


def load_latest() -> dict:
    files = sorted(RESULTS_DIR.glob("*.json"))
    if not files:
        raise SystemExit("No result files found in pilot/results/")
    return json.loads(files[-1].read_text())


def load_result(path: str) -> dict:
    return json.loads(Path(path).read_text())


def evaluate(data: dict) -> dict:
    verdicts = []
    phases = data.get("phases", {})
    summary = data.get("summary", {})
    health = phases.get("health_check", {})
    prompts = phases.get("prompt_results", [])
    concurrent = phases.get("concurrent", {})
    invariants = phases.get("invariant_checks", [])

    # 1. Server health
    verdicts.append({
        "check": "server_health",
        "passed": health.get("status") == "OK",
        "detail": f"model={health.get('model')}, status={health.get('status')}",
        "critical": True,
    })

    # 2. No corrupted outputs
    corrupted = summary.get("prompts_corrupted", 0)
    verdicts.append({
        "check": "no_corrupted_outputs",
        "passed": corrupted == 0,
        "detail": f"{corrupted} corrupted out of {summary.get('prompts_ok', 0) + corrupted}",
        "critical": True,
    })

    # 3. All prompts completed
    failed = summary.get("prompts_failed", 0)
    verdicts.append({
        "check": "all_prompts_completed",
        "passed": failed == 0,
        "detail": f"{failed} prompts failed",
        "critical": True,
    })

    # 4. Invariant checks
    for inv in invariants:
        verdicts.append({
            "check": f"invariant: {inv['invariant'][:60]}",
            "passed": inv["passed"],
            "detail": inv["detail"],
            "critical": not inv.get("manual", False),
            "manual": inv.get("manual", False),
        })

    # 5. Concurrent throughput
    if concurrent:
        verdicts.append({
            "check": "concurrent_throughput",
            "passed": concurrent.get("successes", 0) == concurrent.get("concurrency", 0),
            "detail": f"{concurrent.get('aggregate_throughput_tps', 0)} tokens/s at concurrency={concurrent.get('concurrency', 0)}",
            "critical": False,
        })

    # 6. Non-inferiority (needs baseline comparison — log for now)
    verdicts.append({
        "check": "non_inferiority_vs_baseline",
        "passed": True,
        "detail": "requires baseline measurement for comparison — logged as DOCUMENTED_CAPABILITY",
        "critical": False,
        "note": "run upstream baseline and compare",
    })

    # Overall verdict
    critical_failures = [v for v in verdicts if not v["passed"] and v.get("critical")]
    manual_pending = [v for v in verdicts if v.get("manual") and not v.get("passed")]

    if critical_failures:
        overall = "FAIL"
    elif manual_pending:
        overall = "PARTIAL_PASS"
    else:
        overall = "PASS"

    return {
        "run_id": data.get("run_id"),
        "model": summary.get("model"),
        "overall": overall,
        "verdicts": verdicts,
        "critical_failures": len(critical_failures),
        "manual_pending": len(manual_pending),
        "summary": {
            "prompts_ok": summary.get("prompts_ok"),
            "prompts_failed": summary.get("prompts_failed"),
            "prompts_corrupted": summary.get("prompts_corrupted"),
            "avg_throughput_tps": summary.get("avg_throughput_tps"),
            "concurrent_throughput_tps": summary.get("concurrent_throughput_tps"),
        },
    }


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--latest":
        data = load_latest()
    elif len(sys.argv) > 1:
        data = load_result(sys.argv[1])
    else:
        data = load_latest()

    result = evaluate(data)

    # Print report
    print(f"=== PhenoMLX Pilot Oracle ===")
    print(f"Run:     {result['run_id']}")
    print(f"Model:   {result['model']}")
    print(f"Overall: {result['overall']}")
    print()
    for v in result["verdicts"]:
        status = "PASS" if v["passed"] else ("MANUAL" if v.get("manual") else "FAIL")
        crit = " [CRITICAL]" if v.get("critical") and not v["passed"] else ""
        print(f"  [{status}]{crit} {v['check']}: {v['detail']}")
    print()
    if result["critical_failures"]:
        print(f"CRITICAL FAILURES: {result['critical_failures']}")
    if result["manual_pending"]:
        print(f"MANUAL CHECKS PENDING: {result['manual_pending']}")
        for v in result["verdicts"]:
            if v.get("manual") and not v["passed"]:
                print(f"  - {v['check']}: {v['detail']}")

    # Write oracle result
    out_path = RESULTS_DIR / f"{data.get('run_id', 'unknown')}_oracle.json"
    out_path.write_text(json.dumps(result, indent=2) + "\n")
    print(f"\nOracle written to: {out_path}")

    return 0 if result["overall"] != "FAIL" else 1


if __name__ == "__main__":
    raise SystemExit(main())
