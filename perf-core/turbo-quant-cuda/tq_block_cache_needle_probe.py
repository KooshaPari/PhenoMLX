"""Needle-in-a-haystack retrieval under BlockQuantCache.

Why this instead of MMLU/GPQA: the MMLU/GPQA datasets are not in the local HF
cache and the runs are offline (HF_HUB_OFFLINE=1), and `lm_eval` is not
installed. More to the point, a long-context KV cache has a more specific
failure mode than average task accuracy: quantizing the stored K/V can make the
model lose a fact that sits in the quantized blocks. That is exactly what
needle-in-a-haystack measures, and it needs only the local corpus.

Protocol per case:
  1. Tokenize the local corpus and take `--context` tokens as the haystack.
  2. Splice a unique fact ("the <NAME> facility access code is <CODE>") into the
     haystack at a depth fraction.
  3. Prefill with a cache in 256-token chunks; every complete block is quantized
     before any decode happens (residual < block size).
  4. Ask for the code. The prompt ends with "The access code is " (a real
     trailing space token), so the very next token is the first digit. One
     forward then yields the digit distribution with no cache pollution.

Two metrics, both paired between fp16 and block4 at identical depths:
  - first-digit hit and margin: is the model's preferred first digit correct,
    and by how much does it beat the best *other* digit? This is the clean
    signal and costs one forward.
  - full-code hit: greedy-decode 16 tokens and substring-match the 4-digit code.

Only single-token digits are used for the margin (Qwen2.5 tokenizes "0" as
token 15 and " 0" as [220, 15], so the bare digit ids are the ones that can
follow a trailing-space prompt).
"""

import argparse
import gc
import json
import os
import sys
import time

import torch

os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
os.environ.setdefault(
    "HF_HOME", os.environ.get("TQ_HF_HOME") or r"C:\Users\koosh\.cache\huggingface"
)

sys.path.insert(0, r"C:\phenotype-omlx\perf-core\turbo-quant-cuda")
import transformers.cache_utils as cu  # noqa: E402
from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402

from tq_block_cache import BlockQuantCache  # noqa: E402

NAMES = [
    "TURBINE",
    "ORCHID",
    "QUARTZ",
    "VESPER",
    "CINDER",
    "HALCYON",
    "MERIDIAN",
    "LANTERN",
    "OBSIDIAN",
    "FALCON",
]
DEPTHS = [0.10, 0.30, 0.50, 0.70, 0.90]


def build_case(tok, haystack_ids, depth, name, code):
    total = haystack_ids.shape[0]
    pos = max(1, min(total - 2, int(total * depth)))
    needle = tok(
        f"\n\nNote: the {name} facility access code is {code}.\n\n",
        add_special_tokens=False,
    )["input_ids"]
    ids = torch.cat(
        [
            haystack_ids[:pos],
            torch.tensor(needle, dtype=haystack_ids.dtype),
            haystack_ids[pos:],
        ]
    )
    return ids, pos


def run_case(model, tok, cache, ids, name, code, step, device, digit_ids, greedy=16):
    """Prefill, then read the first-digit distribution and greedy-decode.

    Mirrors the proven driver's convention (`ppl_streaming` in
    tq_packed_cache_eval.py): torch.no_grad() plus explicit cache_position.
    Without no_grad every prefill chunk builds an autograd graph, which makes a
    single case take tens of minutes.
    """
    q = (
        f"\n\nQuestion: What is the access code for the {name} facility?"
        f"\nThe access code is "
    )
    qids = torch.tensor(
        tok(q, add_special_tokens=False)["input_ids"], dtype=ids.dtype, device=device
    ).unsqueeze(0)
    ctx = ids.to(device).unsqueeze(0)

    with torch.no_grad():
        for start in range(0, ctx.shape[1], step):
            end = min(start + step, ctx.shape[1])
            model(
                ctx[:, start:end],
                past_key_values=cache,
                use_cache=True,
                cache_position=torch.arange(start, end, device=device),
            )

        q0 = ctx.shape[1]
        out = model(
            qids,
            past_key_values=cache,
            use_cache=True,
            cache_position=torch.arange(q0, q0 + qids.shape[1], device=device),
        )
        logits = out.logits[:, -1, :].float()
        del out

        # Pollution-free first-digit metric.
        first = str(code)[0]
        dlog = {d: logits[0, tid].item() for d, tid in digit_ids.items()}
        pred = max(dlog, key=dlog.get)
        others = [v for d, v in dlog.items() if d != first]
        margin = dlog[first] - max(others)

        # Greedy decode for the full-code string match (pollutes the cache, so
        # it runs last).
        total = q0 + qids.shape[1]
        toks, nxt = [], logits.argmax(-1, keepdim=True)
        for _ in range(greedy):
            toks.append(nxt.item())
            if nxt.item() == tok.eos_token_id:
                break
            out = model(
                nxt,
                past_key_values=cache,
                use_cache=True,
                cache_position=torch.tensor([total], device=device),
            )
            nxt = out.logits[:, -1, :].argmax(-1, keepdim=True)
            total += 1
    return {
        "pred_digit": pred,
        "digit_hit": pred == first,
        "margin": margin,
        "code_hit": code in tok.decode(toks, skip_special_tokens=True),
        "text": tok.decode(toks, skip_special_tokens=True),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--context", type=int, default=4096)
    ap.add_argument("--step", type=int, default=256)
    ap.add_argument(
        "--corpus", default=r"C:\Users\koosh\agents\sandbox\tq-eval\corpus_long.txt"
    )
    ap.add_argument("--model", default="Qwen/Qwen2.5-3B-Instruct")
    ap.add_argument("--cases", type=int, default=10)
    ap.add_argument(
        "--out",
        default=None,
        help="Optional JSON artifact path. Records per-case rows plus the "
        "fp16/block4 summary so the table in docs/TURBOQUANT-EXTRAPOLATION.md "
        "is reproducible.",
    )
    args = ap.parse_args()

    device = "cuda"
    torch.manual_seed(1234)
    print(
        f"torch {torch.__version__} | {torch.cuda.get_device_name(0)} | "
        f"ctx={args.context} step={args.step} cases={args.cases} | {args.model}",
        flush=True,
    )

    tok = AutoTokenizer.from_pretrained(args.model, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        args.model, dtype=torch.float16, device_map="cuda", local_files_only=True
    )
    model.eval()

    digit_ids = {}
    for d in "0123456789":
        tt = tok(d, add_special_tokens=False)["input_ids"]
        if len(tt) == 1:
            digit_ids[d] = tt[0]
    assert len(digit_ids) == 10, f"expected 10 single-token digits, got {digit_ids}"

    text = open(args.corpus, encoding="utf-8", errors="replace").read()
    hay = tok(text, add_special_tokens=False)["input_ids"]
    if len(hay) < args.context:
        raise SystemExit(f"corpus has {len(hay)} tokens, need {args.context}")
    haystack = torch.tensor(hay[: args.context], dtype=torch.long)

    cases = []
    for i in range(args.cases):
        depth = DEPTHS[i % len(DEPTHS)]
        name = NAMES[i % len(NAMES)]
        code = str(1000 + (i * 1207 + 31) % 9000)
        ids, pos = build_case(tok, haystack, depth, name, code)
        cases.append((depth, name, code, ids, pos))

    results = {}
    for label, factory in (
        ("fp16", lambda: cu.DynamicCache()),
        (
            "block4",
            lambda: BlockQuantCache(
                block=32, bits=4, v_group=32, meta_dtype=torch.float32
            ),
        ),
    ):
        rows = []
        print(f"\n=== {label} ===", flush=True)
        for i, (depth, name, code, ids, pos) in enumerate(cases):
            gc.collect()
            torch.cuda.empty_cache()
            cache = factory()
            t0 = time.perf_counter()
            r = run_case(
                model, tok, cache, ids, name, code, args.step, device, digit_ids
            )
            dt = time.perf_counter() - t0
            rows.append((depth, code, r))
            print(
                f"  [{i:2d}] depth={depth:4.2f} code={code} "
                f"digit_hit={str(r['digit_hit']):<5} pred={r['pred_digit']} "
                f"margin={r['margin']:+6.2f} code_hit={str(r['code_hit']):<5} "
                f"({dt:4.1f}s) {r['text'][:34]!r}",
                flush=True,
            )
            del cache
        results[label] = rows

    print("\n=== summary ===")
    for label, rows in results.items():
        dh = sum(r["digit_hit"] for _, _, r in rows) / len(rows)
        ch = sum(r["code_hit"] for _, _, r in rows) / len(rows)
        mm = sum(r["margin"] for _, _, r in rows) / len(rows)
        print(
            f"  {label:<8} first-digit acc={dh:.2f}  full-code acc={ch:.2f}  "
            f"mean digit margin={mm:+.2f}"
        )

    fp, q4 = results["fp16"], results["block4"]
    print(
        f"\n  first-digit acc delta (block4 - fp16): "
        f"{sum(r['digit_hit'] for _, _, r in q4) / len(q4) - sum(r['digit_hit'] for _, _, r in fp) / len(fp):+.2f}"
    )
    flips = [
        (d, c, a["digit_hit"], b["digit_hit"], a["pred_digit"], b["pred_digit"])
        for (d, c, a), (_, _, b) in zip(fp, q4)
        if a["digit_hit"] != b["digit_hit"]
    ]
    if flips:
        print("  first-digit flips (depth, code, fp_hit, q4_hit, fp_pred, q4_pred):")
        for f in flips:
            print(f"    {f}")
    else:
        print("  no first-digit flips between fp16 and block4")

    if args.out:

        def pack(rows):
            return {
                "first_digit_acc": round(
                    sum(r["digit_hit"] for _, _, r in rows) / len(rows), 4
                ),
                "full_code_acc": round(
                    sum(r["code_hit"] for _, _, r in rows) / len(rows), 4
                ),
                "mean_digit_margin": round(
                    sum(r["margin"] for _, _, r in rows) / len(rows), 4
                ),
                "rows": [
                    {
                        "depth": d,
                        "code": c,
                        "digit_hit": r["digit_hit"],
                        "pred_digit": r["pred_digit"],
                        "margin": round(r["margin"], 4),
                        "code_hit": r["code_hit"],
                        "text": r["text"],
                    }
                    for d, c, r in rows
                ],
            }

        art = {
            "probe": "needle_in_haystack",
            "model": args.model,
            "context_tokens": args.context,
            "step": args.step,
            "cases": args.cases,
            "block": 32,
            "bits": 4,
            "v_group": 32,
            "meta_dtype": "fp32",
            "depths": [c[0] for c in cases],
            "fp16": pack(fp),
            "block4": pack(q4),
            "margin_delta": round(
                (sum(r["margin"] for _, _, r in q4) / len(q4))
                - (sum(r["margin"] for _, _, r in fp) / len(fp)),
                4,
            ),
        }
        with open(args.out, "w", encoding="utf-8") as f:
            json.dump(art, f, indent=2)
        print(f"\nWROTE {args.out}")


if __name__ == "__main__":
    main()
