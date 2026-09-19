"""Decode-loop reproducer for BlockQuantCache.

The prefill reproducer (tq_block_cache_model_check.py) showed:
  - cache_seq_len reads correctly after the fix
  - K values agree with fp16 within 4-bit noise
  - so the fault must be in decode, not prefill

This script extends the comparison past prefill:

  1. prefill 512 tokens with each cache (fp16, block4)
  2. decode N new tokens one at a time
  3. at each step compare the *next-token logit vector* and the cache's
     get_seq_length()

If decode is the problem, the logits diverge on the first decode step or
shortly after. If masking/lengths is the problem, get_seq_length or the
returned K shape will mismatch.
"""

import os
import sys

os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
os.environ.setdefault(
    "HF_HOME", os.environ.get("TQ_HF_HOME") or r"C:\Users\koosh\.cache\huggingface"
)

HERE = r"C:\phenotype-omlx\perf-core\turbo-quant-cuda"
sys.path.insert(0, HERE)

import torch  # noqa: E402
import transformers.cache_utils as cu  # noqa: E402
from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402

from tq_block_cache import BlockQuantCache  # noqa: E402

MODEL_ID = "Qwen/Qwen2.5-3B-Instruct"
CORPUS = r"C:\Users\koosh\agents\sandbox\tq-eval\corpus_repodocs.txt"
PREFILL, DECODE_STEPS, BLOCK = 1024, 8, 32

tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
model = AutoModelForCausalLM.from_pretrained(
    MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
)
model.eval()
text = open(CORPUS, encoding="utf-8", errors="replace").read()
ids = (
    tok(text, return_tensors="pt")
    .input_ids[:, : PREFILL + DECODE_STEPS]
    .to(model.device)
)


def run(cache_factory, label):
    cache = cache_factory()
    pos = torch.arange(0, PREFILL, device=model.device)

    # prefill (with labels so we get loss; decode steps are logits only)
    with torch.no_grad():
        out = model(
            ids[:, :PREFILL],
            labels=ids[:, :PREFILL],
            past_key_values=cache,
            use_cache=True,
            cache_position=pos,
        )
    prefill_loss = float(out.loss)
    print(
        f"  {label:8s} prefill loss={prefill_loss:8.4f}  "
        f"cache_seq_len={cache.get_seq_length()}",
        flush=True,
    )
    logits_history = [out.logits[:, -1, :].detach().float().clone()]
    last = ids[:, PREFILL - 1 : PREFILL]

    # decode one token at a time
    for step in range(DECODE_STEPS):
        pos = torch.tensor([PREFILL + step], device=model.device)
        with torch.no_grad():
            out = model(
                last,
                past_key_values=cache,
                use_cache=True,
                cache_position=pos,
            )
        logits_history.append(out.logits[:, -1, :].detach().float().clone())
        last = ids[:, PREFILL + step : PREFILL + step + 1]
        if step < 4 or step == DECODE_STEPS - 1:
            print(
                f"  step {step:2d} cache_seq_len={cache.get_seq_length()}  "
                f"last_row_argmax={int(out.logits[:, -1, :].argmax())}",
                flush=True,
            )
    return logits_history


def make_fp16():
    return cu.DynamicCache()


def make_block4():
    return BlockQuantCache(block=BLOCK, bits=4)


print(f"\nprefill {PREFILL} + decode {DECODE_STEPS} tokens, fp16 vs block4\n")
plain = run(make_fp16, "fp16")
packed = run(make_block4, "block4")

print("\nlogit deltas per step (relative L2):")
worst = 0.0
worst_step = -1
for i, (a, b) in enumerate(zip(plain, packed)):
    rel = float(((b - a).norm() / a.norm()))
    worst = max(worst, rel)
    if rel == worst:
        worst_step = i
    print(f"  step {i:2d}  rel={rel:.5f}")
print(f"\nworst step {worst_step}: rel={worst:.5f}")
if worst > 0.5:
    print("\nLogits diverge sharply on decode -> fault is in the decode path.")
else:
    print("\nLogits agree -> the cache is decoding cleanly past prefill.")
