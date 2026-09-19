"""Is fp16 metadata a free win? Measures bytes saved against error added.

Metadata is 22% of the resident footprint at block=32 (fp32 scale+zero per group),
so halving its precision should cut the total materially. The question is what that
costs: fp16 has ~3 decimal digits, and the scale/zero relative error it introduces
(~5e-4) should sit far below the 4-bit quantization error itself (~6%), but that is
an argument, not a measurement.

Feeds identical data through two caches that differ only in meta_dtype, then
compares resident bytes and reconstruction error.
"""

import sys

import torch

HERE = r"C:\phenotype-omlx\perf-core\turbo-quant-cuda"
sys.path.insert(0, HERE)

from tq_block_cache import BlockQuantCache  # noqa: E402

B, H, D, BLOCK, BITS, STEPS = 1, 2, 128, 32, 4, [37, 32, 64, 19]

torch.manual_seed(0)
data = [
    (
        torch.randn(B, H, n, D, device="cuda", dtype=torch.float16),
        torch.randn(B, H, n, D, device="cuda", dtype=torch.float16),
    )
    for n in STEPS
]

results = {}
for label, meta_dtype in (
    ("fp32 metadata", torch.float32),
    ("fp16 metadata", torch.float16),
):
    cache = BlockQuantCache(block=BLOCK, bits=BITS, meta_dtype=meta_dtype)
    sq_err = sq_ref = 0.0
    for k, v in data:
        k_out, v_out = cache.update(k, v, 0)
        for got, ref in ((k_out, k), (v_out, v)):
            # `got` is the whole cached sequence; the chunk just fed is its tail.
            # Compare that tail against the true values it was built from.
            n = ref.shape[-2]
            got_tail = got[..., got.shape[-2] - n :, :]
            err = (got_tail - ref).to(torch.float32)
            sq_err += float((err**2).sum())
            sq_ref += float(ref.to(torch.float32).pow(2).sum())
    bd = cache.byte_breakdown()
    results[label] = (bd, (sq_err / sq_ref) ** 0.5)
    print(
        f"{label:14s} payload {bd['payload']:7d}  metadata {bd['metadata']:7d}  "
        f"residual {bd['residual']:7d}  total {bd['total']:7d}  "
        f"rel_err {results[label][1]:.6f}"
    )

a, b = results["fp32 metadata"], results["fp16 metadata"]
fp16_equiv = 2 * B * H * sum(STEPS) * D * 2
print()
print(f"fp16 KV equivalent      {fp16_equiv:7d} B")
print(
    f"metadata saving         {a[0]['metadata'] - b[0]['metadata']:7d} B "
    f"({(a[0]['metadata'] - b[0]['metadata']) / a[0]['metadata'] * 100:.1f}% of the metadata term)"
)
print(
    f"total                   {a[0]['total']:7d} -> {b[0]['total']:7d} B "
    f"(effective reduction {fp16_equiv / a[0]['total']:.2f}x -> {fp16_equiv / b[0]['total']:.2f}x)"
)
print(
    f"reconstruction rel_err  {a[1]:.6f} -> {b[1]:.6f} "
    f"({(b[1] - a[1]) / a[1] * 100:+.2f}%)"
)

assert b[0]["total"] < a[0]["total"], "fp16 metadata should reduce resident bytes"
assert abs(b[1] - a[1]) / a[1] < 0.02, "fp16 metadata should not move error by >2%"
print("\nCONFIRMED: fp16 metadata cuts resident bytes and leaves error within 2%")
