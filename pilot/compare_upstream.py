#!/usr/bin/env python3
"""PhenoMLX Pilot — Upstream Comparison

Runs the same benchmark suite against stock upstream OMLX (mlx_lm.server)
to establish the baseline for non-inferiority comparison.

Usage:
    1. Install upstream: pip install mlx-lm
    2. Start upstream server: python -m mlx_lm.server --model <model> --port 8767
    3. Run this script:  python pilot/compare_upstream.py
    4. Or compare both:  python pilot/compare_upstream.py --compare

Env:
    UPSTREAM_URL    Upstream server URL (default: http://127.0.0.1:8767/v1)
    PHENOMLX_URL    PhenoMLX server URL (default: http://127.0.0.1:8766/v1)
"""
from __future__ import annotations

import json
import os
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
RESULTS_DIR = REPO_ROOT / "pilot" / "results"

PROMPTS = [
    "Explain the difference between TCP and UDP.",
    "Write a Python function to find the longest palindromic substring.",
    "What are the key differences between Rust and C++?",
    "Summarize the causes of World War I in 3 sentences.",
    "Write a SQL query to find the top 3 customers by revenue.",
    "Explain how gradient descent works, including learning rate.",
    "Debug this code: def fib(n): return fib(n-1) + fib(n-2)",
    "Write a bash script that finds all .log files larger than 100MB.",
    "What is the time complexity of quicksort in the worst case?",
    "Translate 'The quick brown fox jumps over the lazy dog' to French, Spanish, and Japanese.",
]

MARGINS = {"ttft_pct": 20, "throughput_pct": 15, "peak_memory_pct": 25}


def _get_json(url: str, timeout: int = 10) -> dict | None:
    try:
        req = urllib.request.Request(url, headers={"Authorization": "Bearer omlx"})
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return json.loads(resp.read().decode())
    except Exception:
        return None


def _post_json(url: str, body: dict, timeout: int = 120) -> dict:
    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json", "Authorization": "Bearer omlx"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode())


def _extract_content(payload: dict) -> str:
    try:
        msg = payload["choices"][0]["message"]
        content = msg.get("content", "")
        if isinstance(content, str) and content.strip():
            return content
        for key in ("reasoning_content", "reasoning", "text"):
            val = msg.get(key)
            if isinstance(val, str) and val.strip():
                return val
        return ""
    except (KeyError, IndexError, TypeError):
        return ""


def run_benchmark(base_url: str, label: str) -> dict:
    """Run the 10-prompt benchmark against a server."""
    print(f"\n=== Benchmarking: {label} ({base_url}) ===")

    # Health check
    models_resp = _get_json(f"{base_url}/models")
    if not models_resp:
        print(f"  ERROR: Cannot reach {base_url}")
        return {"label": label, "status": "UNREACHABLE", "results": []}

    model = models_resp.get("data", [{}])[0].get("id", "unknown")
    print(f"  Model: {model}")

    results = []
    for i, prompt in enumerate(PROMPTS):
        body = {
            "model": model,
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": prompt},
            ],
            "temperature": 0,
            "max_tokens": 512,
            "chat_template_kwargs": {"enable_thinking": False},
        }

        t0 = time.perf_counter()
        try:
            resp = _post_json(f"{base_url}/chat/completions", body, timeout=120)
            t1 = time.perf_counter()

            content = _extract_content(resp)
            usage = resp.get("usage", {})
            prompt_tokens = usage.get("prompt_tokens", 0)
            completion_tokens = usage.get("completion_tokens", 0)
            total_time = t1 - t0
            throughput = completion_tokens / total_time if total_time > 0 else 0

            results.append({
                "prompt_index": i,
                "status": "OK",
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_time_s": round(total_time, 3),
                "throughput_tps": round(throughput, 2),
                "content_length": len(content),
                "is_corrupted": not bool(content.strip()),
            })
            print(f"  [{i}] OK {total_time:.1f}s {completion_tokens}t {throughput:.1f}t/s")
        except Exception as e:
            t1 = time.perf_counter()
            results.append({
                "prompt_index": i,
                "status": "ERROR",
                "error": str(e),
                "total_time_s": round(t1 - t0, 3),
            })
            print(f"  [{i}] ERROR {t1-t0:.1f}s {e}")

        time.sleep(0.5)

    ok = [r for r in results if r["status"] == "OK"]
    avg_time = sum(r["total_time_s"] for r in ok) / len(ok) if ok else 0
    avg_tps = sum(r["throughput_tps"] for r in ok) / len(ok) if ok else 0
    avg_tokens = sum(r["completion_tokens"] for r in ok) / len(ok) if ok else 0

    return {
        "label": label,
        "status": "OK",
        "model": model,
        "results": results,
        "summary": {
            "ok": len(ok),
            "failed": len(results) - len(ok),
            "avg_time_s": round(avg_time, 3),
            "avg_throughput_tps": round(avg_tps, 2),
            "avg_completion_tokens": round(avg_tokens, 0),
        },
    }


def compare(phenomlx: dict, upstream: dict) -> dict:
    """Compare PhenoMLX vs upstream results."""
    pm = phenomlx.get("summary", {})
    up = upstream.get("summary", {})

    comparisons = []
    for key in ["ok", "avg_time_s", "avg_throughput_tps", "avg_completion_tokens"]:
        pm_val = pm.get(key, 0)
        up_val = up.get(key, 0)
        if up_val > 0:
            diff_pct = ((pm_val - up_val) / up_val) * 100
        else:
            diff_pct = 0
        comparisons.append({
            "metric": key,
            "phenomlx": pm_val,
            "upstream": up_val,
            "diff_pct": round(diff_pct, 1),
        })

    # Per-prompt comparison
    prompt_diffs = []
    pm_results = {r["prompt_index"]: r for r in phenomlx.get("results", []) if r["status"] == "OK"}
    up_results = {r["prompt_index"]: r for r in upstream.get("results", []) if r["status"] == "OK"}
    for idx in sorted(set(pm_results.keys()) & set(up_results.keys())):
        pm_r = pm_results[idx]
        up_r = up_results[idx]
        tps_diff = pm_r["throughput_tps"] - up_r["throughput_tps"]
        time_diff = pm_r["total_time_s"] - up_r["total_time_s"]
        prompt_diffs.append({
            "prompt_index": idx,
            "phenomlx_tps": pm_r["throughput_tps"],
            "upstream_tps": up_r["throughput_tps"],
            "tps_diff": round(tps_diff, 2),
            "phenomlx_time_s": pm_r["total_time_s"],
            "upstream_time_s": up_r["total_time_s"],
            "time_diff_s": round(time_diff, 3),
        })

    # Non-inferiority check
    throughput_diff = next((c["diff_pct"] for c in comparisons if c["metric"] == "avg_throughput_tps"), 0)
    verdict = "NON_INFERIOR" if throughput_diff >= -MARGINS["throughput_pct"] else "INFERIORITY_DETECTED"

    return {
        "verdict": verdict,
        "margins": MARGINS,
        "overall_comparisons": comparisons,
        "per_prompt_diffs": prompt_diffs,
        "phenomlx_summary": pm,
        "upstream_summary": up,
    }


def main():
    upstream_url = os.environ.get("UPSTREAM_URL", "http://127.0.0.1:8767/v1")
    phenomlx_url = os.environ.get("PHENOMLX_URL", "http://127.0.0.1:8766/v1")
    run_id = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")

    if "--compare" in sys.argv:
        # Run both benchmarks
        phenomlx_result = run_benchmark(phenomlx_url, "PhenoMLX")
        upstream_result = run_benchmark(upstream_url, "Upstream OMLX")
        comparison = compare(phenomlx_result, upstream_result)

        output = {
            "run_id": run_id,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "phenomlx": phenomlx_result,
            "upstream": upstream_result,
            "comparison": comparison,
        }

        RESULTS_DIR.mkdir(parents=True, exist_ok=True)
        out_path = RESULTS_DIR / f"compare_{run_id}.json"
        out_path.write_text(json.dumps(output, indent=2) + "\n")

        print(f"\n=== Comparison ===")
        print(f"Verdict: {comparison['verdict']}")
        for c in comparison["overall_comparisons"]:
            print(f"  {c['metric']}: PhenoMLX={c['phenomlx']}, Upstream={c['upstream']}, diff={c['diff_pct']:+.1f}%")
        print(f"\nResults: {out_path}")

    elif "--upstream-only" in sys.argv:
        result = run_benchmark(upstream_url, "Upstream OMLX")
        RESULTS_DIR.mkdir(parents=True, exist_ok=True)
        out_path = RESULTS_DIR / f"upstream_{run_id}.json"
        out_path.write_text(json.dumps(result, indent=2) + "\n")
        print(f"\nResults: {out_path}")

    else:
        result = run_benchmark(upstream_url, "Upstream OMLX")
        RESULTS_DIR.mkdir(parents=True, exist_ok=True)
        out_path = RESULTS_DIR / f"upstream_{run_id}.json"
        out_path.write_text(json.dumps(result, indent=2) + "\n")
        print(f"\nResults: {out_path}")


if __name__ == "__main__":
    main()
