"""8K quality comparison: quantize-once BlockQuantCache vs fp16 and hqq.

The open question this answers. transformers' QuantizedCache re-encodes its whole
prefix every 128-token fill, which the residual_length sweep showed is a major
driver of its 8K collapse (8x fewer re-quantizations bought ~5x better quality).
BlockQuantCache encodes each block exactly once and groups K per-channel / V
per-token, so if the collapse was mainly the re-quantization policy this should
land far closer to the hook-harness figure (+1.45% at 2048 tokens) than hqq's
+166% at 8192.

Reuses `ppl_streaming` from tq_packed_cache_eval.py so the comparison is
apples-to-apples: same corpus, same step, same loss definition.

Run: python tq_block_cache_eval.py --max-tokens 8192 --step 256
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
        "--corpus", default=r"C:\Users\koosh\agents\sandbox\tq-eval\corpus_repodocs.txt"
    )
    ap.add_argument("--max-tokens", type=int, default=8192)
    ap.add_argument("--step", type=int, default=256)
    ap.add_argument("--block", type=int, default=32)
    ap.add_argument("--bits", type=int, default=4)
    ap.add_argument("--hqq-bits", type=int, default=4)
    ap.add_argument("--residual", type=int, default=128)
    ap.add_argument("--out", default="block_cache_8k.json")
    args = ap.parse_args()

    t0 = time.perf_counter()
    print(f"torch {torch.__version__} | {torch.cuda.get_device_name(0)}", flush=True)
    tok = AutoTokenizer.from_pretrained(base.MODEL_ID, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        base.MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
    )
    model.eval()
    text = open(args.corpus, encoding="utf-8", errors="replace").read()

    configs = [
        ("fp16", lambda: cu.DynamicCache()),
        (
            f"hqq{args.hqq_bits}x0",
            lambda: cu.QuantizedCache(
                backend="hqq",
                config=model.config,
                nbits=args.hqq_bits,
                axis_key=0,
                axis_value=0,
                q_group_size=32,
                residual_length=args.residual,
            ),
        ),
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

    base_ppl = results["fp16"]["perplexity"]
    for name, r in results.items():
        if "error" not in r:
            r["ppl_delta_pct"] = round((r["perplexity"] - base_ppl) / base_ppl * 100, 3)

    report = {
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "model": base.MODEL_ID,
        "gpu": torch.cuda.get_device_name(0),
        "question": "does quantize-once grouping recover the 8K quality that "
        "transformers' re-quantizing cache loses?",
        "config": {
            "max_tokens": args.max_tokens,
            "step": args.step,
            "block": args.block,
            "bits": args.bits,
            "hqq_bits": args.hqq_bits,
            "hqq_residual_length": args.residual,
        },
        "results": results,
        "caveats": [
            "BlockQuantCache encodes each block once; QuantizedCache re-encodes its whole prefix per residual fill.",
            "K is grouped per-channel and V per-token in BlockQuantCache, matching the hook finding.",
            "Peak VRAM is dominated by the 3B weights at this context.",
        ],
        "wall_clock_s": round(time.perf_counter() - t0, 1),
    }
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)
    print(f"WROTE {args.out}", flush=True)

    print(f"\n=== quantize-once vs re-quantizing at {args.max_tokens} tokens ===")
    for name, r in results.items():
        if "error" in r:
            print(f"{name:16s} FAILED   {r['error'][:60]}")
            continue
        print(
            f"{name:16s} PPL={r['perplexity']:9.4f} delta={r['ppl_delta_pct']:+8.2f}%  "
            f"peak {r['peak_alloc_gib']:.2f} GiB"
        )


if __name__ == "__main__":
    main()
