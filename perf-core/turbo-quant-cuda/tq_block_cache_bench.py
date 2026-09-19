"""Time BlockQuantCache.update() against a plain FP16 cache, at growing prefix.

The 8K comparison run spent 22+ minutes on the quantize-once config while the
re-quantizing hqq config finished in seconds, which needs explaining rather than
guessing. Both caches must materialize the full K/V prefix for attention each
step, so the difference is what the packed cache does on top of that.

This isolates the cache's own cost from the model: synthetic K/V, one layer, and
the same step pattern the PPL harness uses.

Run: python bench_block_cache.py
"""

import sys
import time

import torch

sys.path.insert(0, r"C:\phenotype-omlx\perf-core\turbo-quant-cuda")

import transformers.cache_utils as cu  # noqa: E402

from tq_block_cache import BlockQuantCache  # noqa: E402

B, H, D, STEP, BLOCK, STEPS = 1, 2, 128, 256, 32, 16


def run(label, make):
    cache = make()
    per_step = []
    for _ in range(STEPS):
        k = torch.randn(B, H, STEP, D, device="cuda", dtype=torch.float16)
        v = torch.randn(B, H, STEP, D, device="cuda", dtype=torch.float16)
        torch.cuda.synchronize()
        t0 = time.perf_counter()
        cache.update(k, v, 0)
        torch.cuda.synchronize()
        per_step.append((time.perf_counter() - t0) * 1000)
    print(
        f"{label:24s} total {sum(per_step):8.1f} ms  first {per_step[0]:7.2f} ms  "
        f"last {per_step[-1]:8.2f} ms  growth {per_step[-1] / max(per_step[0], 1e-9):5.1f}x"
    )
    return per_step


print(f"steps of {STEP} tokens, block={BLOCK}, single layer, tf32-free fp16\n")
fp16 = run("fp16 (DynamicCache)", lambda: cu.DynamicCache())
packed = run("block4 (quantize-once)", lambda: BlockQuantCache(block=BLOCK, bits=4))

print()
print(f"{'step':>5} {'tokens':>7} {'fp16 ms':>9} {'block4 ms':>11} {'ratio':>7}")
for i in (0, 3, 7, 11, STEPS - 1):
    r = packed[i] / max(fp16[i], 1e-9)
    print(f"{i + 1:5d} {(i + 1) * STEP:7d} {fp16[i]:9.2f} {packed[i]:11.2f} {r:6.1f}x")

print()
print("Reading: a flat ratio means the packed cache costs a constant multiple; a")
print("rising ratio means it re-materializes stored blocks every step, which is the")
print("O(n^2) shape that makes a host-side dequantize non-viable at long context.")
