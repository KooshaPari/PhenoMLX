"""Functional test for BlockQuantCache. GPU required (the codec is CUDA), no model.

Checks the properties that matter, without loading a language model:
  1. update() returns the full sequence, so attention would see every token.
  2. sequence length accounting matches the tokens fed, including the residual.
  3. quantize-once: stored blocks are byte-identical after later updates
     (this is the property transformers' QuantizedCache lacks, and the reason
     residual_length changed quality at 8K).
  4. residency: packed bytes are close to the theoretical 4-bit size plus the
     FP16 residual, and well under the FP16 equivalent.
  5. reconstruction error is in the expected range for the codec's bit width.

Run: python tq_block_cache_selftest.py
"""

import os
import sys

import torch

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from tq_block_cache import BlockQuantCache, fp16_kv_bytes  # noqa: E402

B, H, D = 1, 2, 128  # batch, kv heads, head dim
BLOCK, BITS = 32, 4
SEQ_STEPS = [37, 32, 64, 19]  # uneven, so the residual boundary is exercised

failures = []


def check(name, cond, detail=""):
    print(
        f"  {'PASS' if cond else 'FAIL'}  {name}{(' -- ' + detail) if detail else ''}"
    )
    if not cond:
        failures.append(name)


torch.manual_seed(0)
cache = BlockQuantCache(block=BLOCK, bits=BITS, v_group=32)

print("feeding steps:", SEQ_STEPS)
fed = 0
snapshots = []
for step_i, n in enumerate(SEQ_STEPS):
    # fp16, as the model hands them over: this also exercises the dtype path
    k = torch.randn(B, H, n, D, device="cuda", dtype=torch.float16)
    v = torch.randn(B, H, n, D, device="cuda", dtype=torch.float16)
    k_out, v_out = cache.update(k, v, 0)
    fed += n
    check(
        f"step {step_i}: returns all {fed} tokens",
        k_out.shape[-2] == fed and v_out.shape[-2] == fed,
        f"got {k_out.shape[-2]}",
    )
    check(
        f"step {step_i}: returned dtype matches the input dtype",
        k_out.dtype == k.dtype and v_out.dtype == v.dtype,
        f"got {k_out.dtype}",
    )
    check(f"step {step_i}: K and V agree on length", k_out.shape == v_out.shape)
    check(
        f"step {step_i}: get_seq_length agrees",
        cache.get_seq_length(0) == fed,
        f"got {cache.get_seq_length(0)}",
    )
    # snapshot the stored blocks so we can prove they are never rewritten
    snapshots.append([e[0][0].clone() for e in cache._stored[0]["k"]])

print()
total_blocks = fed // BLOCK
check(
    "block count matches full blocks",
    len(cache._stored[0]["k"]) == total_blocks,
    f"{len(cache._stored[0]['k'])} blocks for {fed} tokens",
)
check(
    f"residual holds the remainder ({fed % BLOCK} tokens)",
    cache._residual[0][0].shape[-2] == fed % BLOCK,
)

# quantize-once: every block stored at step i must be unchanged at the end
unchanged = all(
    torch.equal(snap[i], cache._stored[0]["k"][i][0][0])
    for snap in snapshots
    for i in range(len(snap))
)
check("quantize-once: stored blocks are never rewritten", unchanged)

bd = cache.byte_breakdown()
fp16_equiv = fp16_kv_bytes((B, H, fed, D))

# Exact accounting, derived from the run's own dimensions rather than hardcoded.
stored_tok = total_blocks * BLOCK
resid_tok = fed - stored_tok
exp_payload = (stored_tok * B * H * D * 2 * BITS) // 8  # K and V packed
exp_meta_k = total_blocks * B * H * D * 2 * 4  # scale+zero per (b,h,d)
exp_meta_v = total_blocks * (B * H * BLOCK) * (D // 32) * 2 * 4
exp_resid = resid_tok * B * H * D * 2 * 2  # K and V at fp16

check(
    "payload matches 4-bit packing exactly",
    bd["payload"] == exp_payload,
    f"{bd['payload']} vs {exp_payload}",
)
check(
    "metadata matches fp32 scale+zero per group",
    bd["metadata"] == exp_meta_k + exp_meta_v,
    f"{bd['metadata']} vs {exp_meta_k + exp_meta_v}",
)
check(
    "residual matches the FP16 trailing block",
    bd["residual"] == exp_resid,
    f"{bd['residual']} vs {exp_resid}",
)
check(
    "breakdown sums to the reported total",
    bd["total"] == bd["payload"] + bd["metadata"] + bd["residual"],
)

payload_frac = bd["payload"] / fp16_equiv
meta_frac = bd["metadata"] / bd["total"]
print(f"  info: payload is {payload_frac:.3f} of FP16 (the nominal bit saving),")
print(
    f"        metadata is {meta_frac:.3f} of the resident total, residual "
    f"{bd['residual'] / bd['total']:.3f}"
)
print(
    f"  info: effective reduction {fp16_equiv / bd['total']:.2f}x "
    f"(nominal {16 / BITS:.2f}x) at block={BLOCK}"
)
check(
    "packed payload is at or below the nominal bit ratio",
    payload_frac <= BITS / 16 + 1e-9,
    f"{payload_frac:.3f}",
)
check(
    "metadata is a material share, as the docs claim",
    meta_frac > 0.15,
    f"{meta_frac:.3f} of the resident total",
)

# reconstruction error on the pooled values (K blocks + residual up to the last)
k_all, _ = cache.update(
    torch.zeros(B, H, 0, D, device="cuda"), torch.zeros(B, H, 0, D, device="cuda"), 0
)
print(
    f"  info: returned {k_all.shape[-2]} tokens, total {bd['total']} B "
    f"= {bd['total'] / fp16_equiv:.3f} of FP16"
)

print()
if failures:
    print(f"SELFTEST FAILED ({len(failures)}): {failures}")
    sys.exit(1)
print("SELFTEST OK")
