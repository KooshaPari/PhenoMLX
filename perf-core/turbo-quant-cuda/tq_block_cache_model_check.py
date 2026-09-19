"""Localize the BlockQuantCache model-interaction bug.

Established so far: the cache's own value handling is fine (self-test and a
token-ramp order check both pass), and the codec round-trips at every size up to
33.5M values. Yet through a real model the cache gives garbage at 1024 tokens
(PPL 56,657) and at 8192 (811,273), while fp16 and hqq are sane.

So the fault is in how the cache behaves *inside* the model. This compares, on
identical real inputs, what each cache returns:

  1. fp16 DynamicCache vs BlockQuantCache: the K/V each hands to attention for the
     same prefill. They must agree to within 4-bit noise (~8% relative), not differ
     wildly.
  2. Whether BlockQuantCache's returned length matches what the model asked for,
     via cache_position.

Run: python tq_block_cache_model_check.py
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
TOKENS, STEP, BLOCK = 512, 512, 32

tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
model = AutoModelForCausalLM.from_pretrained(
    MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
)
model.eval()
text = open(CORPUS, encoding="utf-8", errors="replace").read()
ids = tok(text, return_tensors="pt").input_ids[:, :TOKENS].to(model.device)

captured = {}


def capture_into(store):
    """Hook the rotary patch point to record the K that reaches attention."""
    handles = []
    for layer in model.model.layers:
        proj = layer.self_attn.k_proj

        def hook(mod, inp, out, _s=store):
            x = out[0] if isinstance(out, tuple) else out
            _s.append(x.detach().float().reshape(-1, x.shape[-1]))

        handles.append(proj.register_forward_hook(hook))
    return handles


def run(cache, label):
    store = []
    handles = capture_into(store)
    pos = torch.arange(0, ids.shape[1], device=model.device)
    try:
        with torch.no_grad():
            out = model(
                ids,
                labels=ids,
                past_key_values=cache,
                use_cache=True,
                cache_position=pos,
            )
    finally:
        for h in handles:
            h.remove()
    print(
        f"  {label:16s} loss={float(out.loss):8.4f}  "
        f"cache_seq_len={cache.get_seq_length()}",
        flush=True,
    )
    return store


print(f"prefill {TOKENS} tokens through the model, capturing k_proj outputs\n")
plain = run(cu.DynamicCache(), "fp16")
packed = run(BlockQuantCache(block=BLOCK, bits=4), "block4")

print()
worst = 0.0
for i, (a, b) in enumerate(zip(plain, packed)):
    if a.shape != b.shape:
        print(f"  layer {i}: SHAPE MISMATCH {tuple(a.shape)} vs {tuple(b.shape)}")
        continue
    rel = float(((b - a).norm() / a.norm()))
    worst = max(worst, rel)
print(f"k_proj outputs compared: {len(plain)} tensors; worst relative diff {worst:.5f}")
print()
if worst > 0.5:
    print("The cache changes K far beyond 4-bit noise -> fault is in the cache.")
else:
    print("K agrees with fp16 to within quantization noise -> the fault is NOT in")
    print("the values the cache produces; look at masking/lengths instead.")
