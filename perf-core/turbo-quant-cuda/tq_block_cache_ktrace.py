"""Compare the K the cache RETURNS to attention against the K the model WOULD
have computed without a cache, at each chunked step.

The chunked reproducer showed step 1 loss jumps from 2.10 to 13.39 even though
the cache's bookkeeping looks right (stored=16, res=0, cache_seq_len=512). So
the bug is not in the bookkeeping -- it's either in the K values the cache
returns or in how the model uses them.

Strategy: capture the K that enters attention in two ways:
  * WITHOUT a cache: run a single prefill, hook attention's K input.
  * WITH the cache:  run the same chunked feeding, hook attention's K input.
Then compare per-step. If the cached K differs from the no-cache K beyond 4-bit
noise, the bug is in the cache. If they're close, the bug is in masking or
positional encoding.

Run: python tq_block_cache_ktrace.py
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
    MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
)
model.eval()
text = open(CORPUS, encoding="utf-8", errors="replace").read()
ids = tok(text, return_tensors="pt").input_ids[:, :MAX_TOKENS].to(model.device)


def capture_k_to_attention(store):
    """Hook the K that goes INTO attention. The simplest stable point is the
    K variable right after `cache.update` returns. We hook `eager_attention_forward`
    by patching it. Easier: hook the variable assignment in modeling_qwen2 by
    intercepting `key_states` arg to `attention_interface`.

    Simpler still: hook `apply_rotary_pos_emb` to capture PRE-rope K, but rope
    output is what we want. Hook the o_proj to capture the attention OUTPUT --
    but that's a downstream effect.

    Cleanest approach: capture the K right after `cache.update` by hooking the
    attention forward's `key_states` argument. We do that by overriding the
    layer's attention forward with a wrapper.
    """
    raise NotImplementedError("see alternative below")


def run_and_capture(cache_factory, label):
    """For each chunked step, capture the K that enters attention on layer 0."""
    cache = cache_factory()
    captured_k = []  # list of tensors; one per (step, layer) after cache update

    # Hook: monkey-patch the attention forward on every layer to record key_states
    # right after the `cache.update` call (which is the K that attention sees).
    from transformers.models.qwen2 import modeling_qwen2 as m2

    original_forward = m2.Qwen2Attention.forward

    def patched_forward(
        self,
        hidden_states,
        position_embeddings,
        attention_mask,
        past_key_values=None,
        cache_position=None,
        **kwargs,
    ):
        input_shape = hidden_states.shape[:-1]
        hidden_shape = (*input_shape, -1, self.head_dim)
        q = self.q_proj(hidden_states).view(hidden_shape).transpose(1, 2)
        k = self.k_proj(hidden_states).view(hidden_shape).transpose(1, 2)
        v = self.v_proj(hidden_states).view(hidden_shape).transpose(1, 2)
        cos, sin = position_embeddings
        q, k = m2.apply_rotary_pos_emb(q, k, cos, sin)
        if past_key_values is not None:
            ck = {"sin": sin, "cos": cos, "cache_position": cache_position}
            k, v = past_key_values.update(k, v, self.layer_idx, ck)
        # capture the K the cache returned, at layer 0 only, on every step
        if self.layer_idx == 0:
            captured_k.append(k.detach().float().cpu())
        # Continue with the original attention computation
        attn_out, _ = m2.eager_attention_forward(
            self,
            q,
            k,
            v,
            attention_mask,
            dropout=0.0,
            scaling=self.scaling,
            sliding_window=self.sliding_window,
            **kwargs,
        )
        attn_out = attn_out.reshape(*input_shape, -1).contiguous()
        attn_out = self.o_proj(attn_out)
        return attn_out, None

    m2.Qwen2Attention.forward = patched_forward

    step_losses = []
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
                step_losses.append(float(out.loss))
                del out
    finally:
        m2.Qwen2Attention.forward = original_forward

    print(f"  {label}: step_losses={[round(x, 3) for x in step_losses]}")
    print(f"  {label}: K shapes per step: {[tuple(k.shape) for k in captured_k]}")
    return captured_k, step_losses


print("\nchunked: 1024 tokens in 4 steps of 256\n")
fp16_k, fp16_losses = run_and_capture(lambda: cu.DynamicCache(), "fp16")
block_k, block_losses = run_and_capture(
    lambda: BlockQuantCache(block=BLOCK, bits=4), "block4"
)

print("\nper-step K comparison (cache's K vs no-cache reference would be ideal,")
print("but here we compare block4 K shape vs fp16 K shape per step):")
for i, (fk, bk) in enumerate(zip(fp16_k, block_k)):
    print(f"  step {i}: fp16 K {tuple(fk.shape)}  block4 K {tuple(bk.shape)}")
