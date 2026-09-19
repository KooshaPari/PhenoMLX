"""Does the CUDA codec round-trip correctly at LARGE sizes?

The packed cache is correct at 152 tokens (self-test and token-order check both
pass) but produced PPL 811,272 at 8192 tokens -- deterministically, identically
under two different cache implementations. The only thing that scales is the size
of the decoded slab: at 8192 tokens a K layer decodes 256 blocks x 256 rows x 32 =
2,097,152 values in one call, against 32,768 at 152 tokens.

So this tests the codec itself, with no cache and no model: encode/decode a flat
buffer at growing lengths and check that the reconstruction error stays at the
level expected for the bit width. A size-dependent blow-up would explain the 8K
result while leaving every smaller measurement valid.

Run: python tq_codec_size_check.py
"""

import sys

import torch

sys.path.insert(0, r"C:\phenotype-omlx\perf-core\turbo-quant-cuda")

from turbo_quant_cuda import decode_uniform_cuda, encode_uniform_cuda  # noqa: E402

BITS, GROUP = 4, 32

print(f"codec round-trip vs size, bits={BITS} group={GROUP}")
print(f"{'values':>10} {'groups':>8} {'rel_err':>10}  verdict")
print("(K at 8192 tok needs 2.1M values; V needs 16.8M -- both must be covered)")
bad = []
for shift in (15, 17, 20, 21, 22, 23, 24, 25):
    n = 1 << shift
    n -= n % GROUP  # keep whole groups
    torch.manual_seed(shift)
    x = torch.randn(n, device="cuda", dtype=torch.float32)
    packed, scales, zeros = encode_uniform_cuda(x, BITS, GROUP)
    rec = decode_uniform_cuda(packed, scales, zeros, n, BITS, GROUP)
    rel = float(((rec - x).norm() / x.norm()))
    ok = rel < 0.2  # 4-bit uniform on Gaussian data lands near 0.10
    if not ok:
        bad.append(n)
    print(f"{n:10d} {n // GROUP:8d} {rel:10.5f}  {'ok' if ok else 'BROKEN'}")

print()
if bad:
    print(f"SIZE-DEPENDENT FAILURE at values >= {min(bad)}")
    sys.exit(1)
print("Round-trip holds at every size tested: the codec is not the cause.")
