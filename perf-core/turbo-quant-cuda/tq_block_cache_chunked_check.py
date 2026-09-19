"""Chunked-feeding reproducer for BlockQuantCache.

block_cache_eval.py uses 256-token chunks fed one at a time. The first chunk's
loss matches fp16 (good), the second chunk's loss explodes to 13.4. We need to
find out what's different about the second chunk.

This script mirrors ppl_streaming exactly:
  for start in range(0, ids.shape[1] - 1, step):
      pos = torch.arange(start, end, device=...)
      out = model(part, ..., past_key_values=cache, use_cache=True,
                  cache_position=pos)

At each step we print: cache.get_seq_length(), the cache's stored+residual
token counts, the K shape the cache returned, and the loss.

If step 2's K shape is wrong (smaller than expected), the cache is the bug.
If the shape is right but loss explodes, the bug is in how attention uses it.
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
MAX_TOKENS, STEP, BLOCK = 1024, 256, 32

tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
model = AutoModelForCausalLM.from_pretrained(
    MODEL_ID,
    dtype=torch.float16,
    device_map="cuda",
    local_files_only=True,
)
model.eval()
text = open(CORPUS, encoding="utf-8", errors="replace").read()
ids = tok(text, return_tensors="pt").input_ids[:, :MAX_TOKENS].to(model.device)


def capture_k(cache, store):
    """Hook k_proj to capture the K that goes INTO the cache.update call."""

    def hook_factory(target):
        def hook(mod, inp, out):
            x = out[0] if isinstance(out, tuple) else out
            target.append(x.detach().float().reshape(-1, x.shape[-1]))

        return hook

    handles = []
    for layer in model.model.layers:
        proj = layer.self_attn.k_proj
        handles.append(proj.register_forward_hook(hook_factory(store)))
    return handles


def run(make_cache, label):
    cache = make_cache()
    k_in_log = []  # what the cache.update() receives as K (from k_proj post-rope?)

    handles = capture_k(cache, k_in_log)
    try:
        with torch.no_grad():
            for start in range(0, ids.shape[1] - 1, STEP):
                end = min(start + STEP, ids.shape[1] - 1)
                part = ids[:, start:end]
                pos = torch.arange(start, end, device=model.device)
                out = model(
                    part,
                    labels=part,
                    past_key_values=cache,
                    use_cache=True,
                    cache_position=pos,
                )
                seq_len = cache.get_seq_length()
                # the cached k/v at this layer (layer 0)
                layer0 = getattr(cache, "_stored", {}).get(0, {})
                k_blocks = layer0.get("k", {}).get("blocks", 0) if layer0 else 0
                res = getattr(cache, "_residual", {}).get(0)
                res_len = 0 if res is None else res[0].shape[-2]
                # k shape that went into the cache this step
                k_in_shape = k_in_log[-1].shape if k_in_log else "no_hook"
                if (start // STEP) < 3 or start // STEP == (MAX_TOKENS // STEP) - 1:
                    print(
                        f"  step {start // STEP:2d} [{start}..{end}) "
                        f"loss={float(out.loss):7.4f} cache_seq_len={seq_len} "
                        f"stored={k_blocks} res={res_len} k_in_shape={k_in_shape}",
                        flush=True,
                    )
                del out
    finally:
        for h in handles:
            h.remove()


print(
    f"\nchunked prefill: {MAX_TOKENS} tokens in {MAX_TOKENS // STEP} steps of {STEP}\n"
)
run(lambda: cu.DynamicCache(), "fp16")
print()
run(lambda: BlockQuantCache(block=BLOCK, bits=4), "block4")
