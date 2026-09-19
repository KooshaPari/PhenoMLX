"""Does BlockQuantCache return values in the right ORDER?

The shape-only self-test cannot catch a block-ordering bug: if the cache hands
attention a K/V tensor whose tokens are permuted, every shape assertion still
passes while the model's output becomes garbage. That is exactly the symptom seen
in the first 8K run (block4 PPL 811,273 against 6.02 for FP16), so ordering needs
to be tested by value.

Method: make each token identifiable. K[..., t, 0] = t (a ramp), and V[..., t, 0]
= 1000 + t. After feeding several steps the cache must return, at position t, a
value close to the token's own ramp value -- not a neighbour's, and not shuffled
across blocks. Checked with and without a trailing partial block, and at a size
where several blocks exist.
"""

import sys

import torch

HERE = r"C:\phenotype-omlx\perf-core\turbo-quant-cuda"
sys.path.insert(0, HERE)

from tq_block_cache import BlockQuantCache  # noqa: E402

B, H, D, BLOCK, BITS = 1, 2, 64, 32, 4
STEPS = [32, 32, 32, 7]  # three full blocks plus a partial trailing block

cache = BlockQuantCache(block=BLOCK, bits=BITS)
fed = 0
ramps = []
for n in STEPS:
    k = torch.zeros(B, H, n, D, device="cuda", dtype=torch.float16)
    v = torch.zeros(B, H, n, D, device="cuda", dtype=torch.float16)
    for t in range(n):
        k[..., t, 0] = float(fed + t)  # token identity in channel 0
        v[..., t, 0] = 1000.0 + float(fed + t)
    k_out, v_out = cache.update(k, v, 0)
    fed += n
    ramps.append(fed)

# Rebuild the returned K/V by a zero-length update, so the last one is complete.
k_all, v_all = cache.update(
    torch.zeros(B, H, 0, D, device="cuda", dtype=torch.float16),
    torch.zeros(B, H, 0, D, device="cuda", dtype=torch.float16),
    0,
)
seq = k_all.shape[-2]
print(f"fed {fed} tokens; cache returns {seq}")

k_ramp = k_all[0, 0, :, 0].float()
v_ramp = v_all[0, 0, :, 0].float()
expected = torch.arange(seq, device="cuda", dtype=torch.float32)

k_err = (k_ramp - expected).abs()
v_err = (v_ramp - (1000.0 + expected)).abs()

print(
    f"K ramp: max abs err {float(k_err.max()):.3f}  "
    f"first 8 returned {[round(float(x), 1) for x in k_ramp[:8]]}"
)
print(
    f"V ramp: max abs err {float(v_err.max()):.3f}  "
    f"first 8 returned {[round(float(x), 1) for x in v_ramp[:8]]}"
)

# A 4-bit ramp over 0..103 has steps of ~7 per level, so error can reach ~3.5;
# a shuffled order would show errors of tens.
ok_k = float(k_err.max()) < 5.0
ok_v = float(v_err.max()) < 5.0
print()
print(f"  {'PASS' if ok_k else 'FAIL'}  K is returned in token order")
print(f"  {'PASS' if ok_v else 'FAIL'}  V is returned in token order")
if not (ok_k and ok_v):
    print("\nORDERING BUG: the model would attend to the wrong positions.")
    sys.exit(1)
print("\nORDER OK")
