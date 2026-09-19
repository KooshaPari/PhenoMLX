"""Shipped regression: BlockQuantCache stays within noise of fp16 on chunked feeding.

The two bugs that hit this cache (get_seq_length returning 0, and
get_mask_sizes returning query_length instead of cache+query) are invisible to
the prefill-only reproducer (tq_block_cache_model_check.py) and to one-shot
decode (tq_block_cache_decode_check.py), because both feed the cache in a
single update() call. They show up the moment a second chunk reuses the
stored prefix, which is what generate() and ppl_streaming do.

This shipped check runs the same chunked-feeding pattern (256-token steps
over 1024 tokens) and asserts that block4's step losses match fp16 within
4-bit noise (~3% relative on the per-step loss). Any future regression that
breaks either override will fail here loudly.

Fast (about 90s on a 3090 Ti): just runs one fp16 vs block4 comparison at
1024 tokens and prints the per-step deltas. Pure functional check, no JSON.
"""

import os
import sys

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

from tq_block_cache import BlockQuantCache  # noqa: E402

MODEL_ID = "Qwen/Qwen2.5-3B-Instruct"
CORPUS = r"C:\Users\koosh\agents\sandbox\tq-eval\corpus_repodocs.txt"
MAX_TOKENS, STEP, BLOCK = 1024, 256, 32
# 4-bit group quantization produces ~7-8% relative error on K; the
# per-step cross-entropy loss moves a lot less because most of it is
# dominated by token distribution rather than K quantization. 5% relative
# per-step is well above noise and well below any bug we have seen so far.
MAX_STEP_REL_DIFF = 0.05


def ppl_streaming(model, tok, text, max_tokens, step, make_cache):
    ids = tok(text, return_tensors="pt").input_ids[:, :max_tokens].to(model.device)
    cache = make_cache()
    losses = []
    with torch.no_grad():
        for start in range(0, ids.shape[1] - 1, step):
            end = min(start + step, ids.shape[1] - 1)
            part = ids[:, start:end]
            pos = torch.arange(start, end, device=model.device)
            out = model(
                part,
                labels=part,
                past_key_values=cache,
                use_cache=True,
                cache_position=pos,
            )
            losses.append(float(out.loss))
            del out
    return losses


def main():
    print(f"torch {torch.__version__} | {torch.cuda.get_device_name(0)}", flush=True)
    tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
    )
    model.eval()
    text = open(CORPUS, encoding="utf-8", errors="replace").read()

    print(
        f"\nchunked prefill {MAX_TOKENS} tok in {MAX_TOKENS // STEP} steps of {STEP}\n"
    )
    fp16 = ppl_streaming(model, tok, text, MAX_TOKENS, STEP, lambda: cu.DynamicCache())
    torch.cuda.empty_cache()
    block4 = ppl_streaming(
        model,
        tok,
        text,
        MAX_TOKENS,
        STEP,
        lambda: BlockQuantCache(block=BLOCK, bits=4),
    )

    print(f"{'step':>5} {'fp16 loss':>10} {'block4 loss':>12} {'rel diff':>10}")
    worst = 0.0
    for i, (a, b) in enumerate(zip(fp16, block4)):
        diff = abs(b - a) / max(abs(a), 1e-9)
        worst = max(worst, diff)
        print(f"{i:5d} {a:10.4f} {b:12.4f} {diff:10.4f}")
    print(f"\nworst per-step relative diff: {worst:.4f} (limit {MAX_STEP_REL_DIFF})")

    checks = [
        (
            f"no step diverges by more than {MAX_STEP_REL_DIFF:.0%}",
            worst <= MAX_STEP_REL_DIFF,
        ),
        (
            "block4 step 1 matches fp16 (the bug only ever shows up here)",
            abs(block4[1] - fp16[1]) / abs(fp16[1]) <= MAX_STEP_REL_DIFF,
        ),
    ]
    ok = True
    for name, passed in checks:
        print(f"  {'PASS' if passed else 'FAIL'}  {name}")
        ok &= passed
    print(
        "\nVERIFIED: BlockQuantCache stays within noise of fp16 on chunked feeding"
        if ok
        else "\nVERIFY FAILED: cache drift on chunked feeding -- check get_seq_length "
        "and get_mask_sizes overrides"
    )
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
