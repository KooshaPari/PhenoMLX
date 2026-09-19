"""Long-context quality scaling for quantize-once BlockQuantCache.

The 8K eval (`tq_block_cache_eval.py`) established the headline result: 3B at 8192
tokens lands at +0.97% PPL vs fp16. This script extends the same `ppl_streaming`
driver along the context axis, with hqq dropped because its 8K collapse is
already documented (`hqq +166% at 8K` in docs/TURBOQUANT-EXTRAPOLATION.md item
(g)) and it would dominate the runtime at 16-32K without contributing new
information.

Each measurement produces a per-step loss trace alongside the summary PPL, so
the comparison can pinpoint where (if anywhere) the cache drifts.

Run: python tq_block_cache_eval_long.py --max-tokens 16384 --out block_cache_16k.json
Run: python tq_block_cache_eval_long.py --max-tokens 32768 --out block_cache_32k.json
"""

import argparse
import json
import os
import sys
import time
from datetime import datetime, timezone

os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
os.environ.setdefault(
    "HF_HOME", os.environ.get("TQ_HF_HOME") or r"C:\Users\koosh\.cache\huggingface"
)

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import torch  # noqa: E402
import transformers.cache_utils as cu  # noqa: E402
from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402

import tq_packed_cache_eval as base  # noqa: E402  (reuse ppl_streaming)
from tq_block_cache import BlockQuantCache  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--corpus",
        default=r"C:\Users\koosh\agents\sandbox\tq-eval\corpus_repodocs.txt",
        help="Plain text corpus. PPL is over the first --max-tokens tokens.",
    )
    ap.add_argument(
        "--max-tokens", type=int, default=16384, help="Context length to evaluate at."
    )
    ap.add_argument("--step", type=int, default=256, help="Streaming chunk size.")
    ap.add_argument("--block", type=int, default=32, help="BlockQuantCache block size.")
    ap.add_argument("--bits", type=int, default=4, help="BlockQuantCache bit width.")
    ap.add_argument(
        "--out",
        default="block_cache_long.json",
        help="Output report path (single file per context length).",
    )
    args = ap.parse_args()

    t0 = time.perf_counter()
    print(
        f"torch {torch.__version__} | {torch.cuda.get_device_name(0)} | "
        f"target {args.max_tokens} step {args.step}",
        flush=True,
    )
    tok = AutoTokenizer.from_pretrained(base.MODEL_ID, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        base.MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
    )
    model.eval()
    text = open(args.corpus, encoding="utf-8", errors="replace").read()

    # hqq is intentionally omitted: the 8K result (+166%) and 4K codec sweep
    # already establish its scaling behaviour, and re-running per residual fill
    # at 32K would consume the whole budget without moving the comparison.
    configs = [
        ("fp16", lambda: cu.DynamicCache()),
        (
            f"block{args.bits}",
            lambda: BlockQuantCache(block=args.block, bits=args.bits),
        ),
    ]

    results = {}
    for label, factory in configs:
        try:
            results[label] = base.ppl_streaming(
                model, tok, text, args.max_tokens, args.step, factory, label
            )
        except Exception as exc:  # noqa: BLE001 -- a bad cell must not kill the run
            msg = f"{type(exc).__name__}: {exc}"
            print(f"  {label:16s} FAILED  {msg}", flush=True)
            results[label] = {"error": msg}
        torch.cuda.empty_cache()

    base_ppl = None
    if "error" not in results["fp16"]:
        base_ppl = results["fp16"]["perplexity"]
        for name, r in results.items():
            if "error" not in r:
                r["ppl_delta_pct"] = round(
                    (r["perplexity"] - base_ppl) / base_ppl * 100, 3
                )

    # Step-trace quality summary. Step rel diff is dominated by loss saturation
    # when the corpus contains repeats (or any text the LM has seen before) --
    # if fp16's per-step loss is 0.0005 and block4's is 0.0010 the rel diff is
    # 100% but the *quality* difference is well below noise. We exclude steps
    # where either side is below a saturation threshold (default 0.05 nats)
    # before computing mean / max rel diff, so the figure tracks the same
    # thing at every context length and on every corpus.
    fp_losses = (
        results.get("fp16", {}).get("step_losses", [])
        if "error" not in results.get("fp16", {})
        else []
    )
    for name, r in results.items():
        if "error" in r or name == "fp16" or not fp_losses:
            continue
        bk_losses = r.get("step_losses", [])
        if len(bk_losses) != len(fp_losses):
            continue
        sat_thresh = 0.05  # nats; below this both configurations are at ceiling
        rel_diffs = []
        for f, b in zip(fp_losses, bk_losses):
            denom = max(abs(f), 1e-9)
            if abs(f) < sat_thresh or abs(b) < sat_thresh:
                continue
            rel_diffs.append(abs(b - f) / denom)
        if rel_diffs:
            r["step_rel_diff_mean"] = round(sum(rel_diffs) / len(rel_diffs) * 100, 4)
            r["step_rel_diff_max"] = round(max(rel_diffs) * 100, 4)
            r["step_rel_diff_used"] = len(rel_diffs)
            r["step_rel_diff_skipped_saturated"] = len(fp_losses) - len(rel_diffs)

    report = {
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "model": base.MODEL_ID,
        "gpu": torch.cuda.get_device_name(0),
        "question": "does BlockQuantCache hold the +0.97% 8K figure as context "
        "extends to 16K/32K?",
        "config": {
            "max_tokens": args.max_tokens,
            "step": args.step,
            "block": args.block,
            "bits": args.bits,
        },
        "results": results,
        "caveats": [
            "Same ppl_streaming driver and corpus as tq_block_cache_eval.py; only the "
            "context length differs.",
            "hqq intentionally omitted -- its 8K collapse is already documented.",
            "Peak VRAM is dominated by the 3B weights at this context.",
            "Cache decode is host-side; per-step wall-clock grows with stored blocks.",
        ],
        "wall_clock_s": round(time.perf_counter() - t0, 1),
    }
    out_path = args.out
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)
    print(f"WROTE {out_path}\n", flush=True)

    print(f"=== quantize-once at {args.max_tokens} tokens ===")
    for name, r in results.items():
        if "error" in r:
            print(f"{name:16s} FAILED   {r['error'][:80]}")
            continue
        line = f"{name:16s} PPL={r['perplexity']:9.4f}"
        if base_ppl is not None and name != "fp16":
            line += f" delta={r['ppl_delta_pct']:+8.2f}%"
            if "step_rel_diff_mean" in r:
                line += (
                    f"  step_rel_diff (unsat) mean={r['step_rel_diff_mean']:.2f}% "
                    f"max={r['step_rel_diff_max']:.2f}% "
                    f"used={r['step_rel_diff_used']}/{len(r['step_losses'])}"
                )
        line += f"  peak {r['peak_alloc_gib']:.2f} GiB"
        print(line)

    if base_ppl is not None and "error" not in results.get(f"block{args.bits}", {}):
        block_r = results[f"block{args.bits}"]
        if block_r["ppl_delta_pct"] > 5.0:
            print(
                f"\nWARN: block{args.bits} PPL is {block_r['ppl_delta_pct']:+.2f}% "
                f"vs fp16 (>5% threshold). Investigate scaling."
            )


if __name__ == "__main__":
    main()
