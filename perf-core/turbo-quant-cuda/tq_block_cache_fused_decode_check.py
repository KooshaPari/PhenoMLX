"""Shipped regression for the opt-in Triton fused KV decode.

The fused kernel replaces the shipped decode chain in `_k_tensor` / `_v_tensor`:
decode + dtype cast + reshape/permute/reshape + cache-buffer copy, fused into
one launch that writes each element straight to its final position in the
destination buffer, so the intermediate tensors and the trailing `.copy_()`
disappear.

Every assertion is bit-identity against the shipped `BlockQuantCache`. That is
the whole safety argument: the fused path must produce the exact same bytes as
the path it replaces, or it is not allowed to run.

Two properties are checked here that a microbenchmark does not catch, and both
were real failures during development:

  1. NO RECOMPILE STORM. Every per-chunk-varying value (start token, block
     count, buffer size, destination strides) must be a runtime scalar.
     Streaming N chunks must not grow Triton's compiled-variant store by N. The
     first prototype constexpr'd all of them and recompiled on every call:
     32 chunks -> 32 variants, 507 ms median, which is several times the
     shipped cost of decoding the entire 8K prefix.

  2. NO FMA CONTRACTION. The shipped path dequantizes as two separate fp32 ops,
     so the product is rounded BEFORE the add. A plain `q * s + z` in Triton
     gets contracted into an FMA and lands 1 ULP away on ~41 values per
     million. The kernel must use explicit `mul.rn.f32` then `add.rn.f32`.

Run: python tq_block_cache_fused_decode_check.py
"""

import os
import sys

import torch

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from tq_block_cache import BlockQuantCache  # noqa: E402
from tq_fused_decode import (  # noqa: E402
    TRITON_FUSED_AVAILABLE,
    fused_decode_available,
    fused_k_kernel,
    fused_v_kernel,
)

failures = []

B, H, D = 1, 2, 128


def check(name, cond, detail=""):
    print(f"  {'PASS' if cond else 'FAIL'}  {name}{(' -- ' + detail) if detail else ''}")
    if not cond:
        failures.append(name)


def bits_identical(a, b):
    """Exact payload equality via the integer view, so 0.0 vs -0.0 also counts."""
    if a is None or b is None:
        return a is None and b is None
    if a.dtype != b.dtype or a.shape != b.shape:
        return False
    width = {torch.float16: torch.int16, torch.bfloat16: torch.int16,
             torch.float32: torch.int32}[a.dtype]
    return torch.equal(a.view(width), b.view(width))


def variant_count(kernel):
    if kernel is None:
        return 0
    total = 0
    for per_device in kernel.device_caches.values():
        try:
            total += len(per_device[0])
        except (TypeError, IndexError, KeyError):
            total += len(per_device)
    return total


class FusedCache(BlockQuantCache):
    """A cache whose decode goes through the fused kernels.

    A subclass rather than a monkey-patch on `BlockQuantCache`, so enabling the
    fused path on one instance can never change the reference cache used to
    produce the expected bytes. `BlockQuantCache` dispatches to the fused
    kernels itself when `fused_decode=True`, so there is no method override
    here and no risk of saving an overridden method as its own fallback.
    """

    def __init__(self, *args, **kwargs):
        kwargs["fused_decode"] = True
        super().__init__(*args, **kwargs)


def payloads(n_chunks, chunk, dtype=torch.float16, seed=0, batch=B):
    """Deterministic K/V chunk inputs, identical for reference and fused caches."""
    torch.manual_seed(seed)
    return [
        (
            (torch.randn(batch, H, chunk, D, device="cuda") * 0.5).to(dtype),
            (torch.randn(batch, H, chunk, D, device="cuda") * 0.5).to(dtype),
        )
        for _ in range(n_chunks)
    ]


def feed(cache, steps, dtype=torch.float16, seed=0, batch=B, chunk=None):
    """Feed a cache through the real update() path so blocks are codec output."""
    torch.manual_seed(seed)
    for n in steps:
        n = n if chunk is None else chunk
        k = (torch.randn(batch, H, n, D, device="cuda") * 0.5).to(dtype)
        v = (torch.randn(batch, H, n, D, device="cuda") * 0.5).to(dtype)
        cache.update(k, v, 0)
    return cache


def main():
    print(f"torch {torch.__version__} | {torch.cuda.get_device_name(0)}")
    print(f"geometry B={B} H={H} D={D} block=32")

    print("\n=== availability ===")
    check("TRITON_FUSED_AVAILABLE on this build", bool(TRITON_FUSED_AVAILABLE))
    check("fused_decode_available() on a CUDA build", fused_decode_available())

    # ---- fresh decode bit-identity across prefix lengths ----
    print("\n=== fresh decode bit-identity (block=32) ===")
    for n_chunks in (1, 2, 4, 8, 16, 32):
        steps = [256] * n_chunks
        ref = feed(BlockQuantCache(block=32, bits=4, v_group=32), steps, seed=n_chunks)
        fused = feed(FusedCache(block=32, bits=4, v_group=32), steps, seed=n_chunks)
        if ref._stored[0]["k"]["blocks"] == 0:
            continue
        r = ref._k_tensor(ref._stored[0], torch.float16).clone()
        f = fused._k_tensor(fused._stored[0], torch.float16)
        check(f"{n_chunks * 256:5d} tok: K fresh decode bit-identical",
              bits_identical(r, f),
              f"max diff {(r.float() - f.float()).abs().max().item():.6f}")
        r = ref._v_tensor(ref._stored[0], torch.float16).clone()
        f = fused._v_tensor(fused._stored[0], torch.float16)
        check(f"{n_chunks * 256:5d} tok: V fresh decode bit-identical",
              bits_identical(r, f),
              f"max diff {(r.float() - f.float()).abs().max().item():.6f}")

    # ---- streaming through update(), the way it is actually used ----
    print("\n=== streaming update() equivalence ===")
    for label, steps in (
        ("8K uniform", [256] * 32),
        ("uneven", [37, 32, 64, 19]),
        ("sub-block", [33, 33, 33]),
        ("one-at-a-time", [1] * 40),
    ):
        ref = BlockQuantCache(block=32, bits=4, v_group=32)
        fused = FusedCache(block=32, bits=4, v_group=32)
        fed = 0
        all_match = True
        first_bad = None
        for n in steps:
            k = (torch.randn(B, H, n, D, device="cuda") * 0.5).to(torch.float16)
            v = (torch.randn(B, H, n, D, device="cuda") * 0.5).to(torch.float16)
            rk, rv = ref.update(k, v, 0)
            fk, fv = fused.update(k, v, 0)
            fed += n
            if not (bits_identical(rk, fk) and bits_identical(rv, fv)):
                all_match = False
                first_bad = (fed, (rk.float() - fk.float()).abs().max().item())
                break
        check(f"{label} ({fed} tok): every update() output bit-identical", all_match,
              f"first mismatch at fed={first_bad[0]}, diff={first_bad[1]:.6f}"
              if first_bad else "")
        check(f"{label}: get_seq_length agrees",
              ref.get_seq_length(0) == fused.get_seq_length(0) == fed)

    # ---- dtype support: the cast must happen inside the kernel ----
    print("\n=== dtype support (output dtype follows the caller) ===")
    for dtype in (torch.float16, torch.bfloat16, torch.float32):
        steps = [256] * 8
        ref = feed(BlockQuantCache(block=32, bits=4, v_group=32), steps, dtype, seed=7)
        fused = feed(FusedCache(block=32, bits=4, v_group=32), steps, dtype, seed=7)
        r = ref._k_tensor(ref._stored[0], dtype).clone()
        f = fused._k_tensor(fused._stored[0], dtype)
        check(f"{str(dtype):16s} K: bit-identical and dtype preserved",
              bits_identical(r, f) and f.dtype == dtype)
        r = ref._v_tensor(ref._stored[0], dtype).clone()
        f = fused._v_tensor(fused._stored[0], dtype)
        check(f"{str(dtype):16s} V: bit-identical and dtype preserved",
              bits_identical(r, f) and f.dtype == dtype)

    # ---- dtype SWITCH mid-stream: the per-dtype cache must invalidate ----
    print("\n=== dtype switch mid-stream ===")
    ref = BlockQuantCache(block=32, bits=4, v_group=32)
    fused = FusedCache(block=32, bits=4, v_group=32)
    ok = True
    for i, dtype in enumerate([torch.float16, torch.float32, torch.bfloat16,
                               torch.float16, torch.float32]):
        k = (torch.randn(B, H, 256, D, device="cuda") * 0.5).to(dtype)
        v = (torch.randn(B, H, 256, D, device="cuda") * 0.5).to(dtype)
        rk, rv = ref.update(k, v, 0)
        fk, fv = fused.update(k, v, 0)
        if not (bits_identical(rk, fk) and bits_identical(rv, fv)):
            ok = False
            break
    check("switching fp16->fp32->bf16->fp16->fp32 stays bit-identical", ok)

    # ---- batch > 1 ----
    print("\n=== batch > 1 ===")
    for batch in (2, 4):
        steps = [256] * 8
        ref = feed(BlockQuantCache(block=32, bits=4, v_group=32), steps,
                   seed=batch, batch=batch)
        fused = feed(FusedCache(block=32, bits=4, v_group=32), steps,
                     seed=batch, batch=batch)
        r = ref._k_tensor(ref._stored[0], torch.float16).clone()
        f = fused._k_tensor(fused._stored[0], torch.float16)
        check(f"batch={batch} K: bit-identical", bits_identical(r, f),
              f"shapes {tuple(r.shape)} vs {tuple(f.shape)}")
        r = ref._v_tensor(ref._stored[0], torch.float16).clone()
        f = fused._v_tensor(fused._stored[0], torch.float16)
        check(f"batch={batch} V: bit-identical", bits_identical(r, f))

    # ---- geometry generality ----
    print("\n=== geometry generality (block, v_group) ===")
    for block, v_group in ((32, 32), (16, 32), (64, 64), (32, 16), (8, 8)):
        chunk = max(block * 4, 64)
        steps = [chunk] * 6
        ref = feed(BlockQuantCache(block=block, bits=4, v_group=v_group), steps,
                   seed=block + v_group)
        fused = feed(FusedCache(block=block, bits=4, v_group=v_group), steps,
                     seed=block + v_group)
        r = ref._k_tensor(ref._stored[0], torch.float16).clone()
        f = fused._k_tensor(fused._stored[0], torch.float16)
        check(f"block={block:2d} v_group={v_group:2d} K: bit-identical",
              bits_identical(r, f))
        r = ref._v_tensor(ref._stored[0], torch.float16).clone()
        f = fused._v_tensor(fused._stored[0], torch.float16)
        check(f"block={block:2d} v_group={v_group:2d} V: bit-identical",
              bits_identical(r, f))

    # ---- fallback: 2-bit and 3-bit must use the shipped codec ----
    print("\n=== fallback for unsupported bit widths ===")
    for bits in (2, 3):
        steps = [256] * 4
        ref = feed(BlockQuantCache(block=32, bits=bits, v_group=32), steps, seed=bits)
        fused = feed(FusedCache(block=32, bits=bits, v_group=32), steps, seed=bits)
        r = ref._k_tensor(ref._stored[0], torch.float16).clone()
        f = fused._k_tensor(fused._stored[0], torch.float16)
        check(f"bits={bits} K: falls back, bit-identical", bits_identical(r, f))
        r = ref._v_tensor(ref._stored[0], torch.float16).clone()
        f = fused._v_tensor(fused._stored[0], torch.float16)
        check(f"bits={bits} V: falls back, bit-identical", bits_identical(r, f))

    # ---- THE REGRESSION THAT MATTERS: no recompile storm ----
    print("\n=== no recompile storm ===")
    fused = FusedCache(block=32, bits=4, v_group=32)
    # Warm every kernel variant that any geometry in this test needs, so the
    # count below reflects the streaming loop alone.
    for block, v_group in ((32, 32), (16, 32), (64, 64), (32, 16), (8, 8)):
        feed(FusedCache(block=block, bits=4, v_group=v_group), [max(block * 4, 64)] * 2)
    for dtype in (torch.float16, torch.bfloat16, torch.float32):
        feed(FusedCache(block=32, bits=4, v_group=32), [256] * 2, dtype)
    for batch in (2, 4):
        feed(FusedCache(block=32, bits=4, v_group=32), [256] * 2, batch=batch)
    warm_k, warm_v = variant_count(fused_k_kernel), variant_count(fused_v_kernel)
    print(f"  variants after warming all tested geometries: K={warm_k} V={warm_v}")

    torch.manual_seed(3)
    stream = FusedCache(block=32, bits=4, v_group=32)
    for _ in range(32):
        k = (torch.randn(B, H, 256, D, device="cuda") * 0.5).to(torch.float16)
        v = (torch.randn(B, H, 256, D, device="cuda") * 0.5).to(torch.float16)
        stream.update(k, v, 0)
    after_k, after_v = variant_count(fused_k_kernel), variant_count(fused_v_kernel)
    print(f"  variants after 32 streaming chunks:            K={after_k} V={after_v}")
    check("streaming 32 chunks adds no K variants", after_k == warm_k,
          f"{warm_k} -> {after_k}")
    check("streaming 32 chunks adds no V variants", after_v == warm_v,
          f"{warm_v} -> {after_v}")
    check("32 chunks did NOT produce 32 K variants (the old bug)", after_k < 32,
          f"{after_k} variants for 32 chunks")

    # ---- opt-in is per instance, never per class ----
    # The old API patched the class, so a single opt-in leaked into every other
    # cache in the process, including the reference cache an equivalence test is
    # comparing against. There is nothing to install and nothing to uninstall
    # now, so the check is that constructing a fused cache leaves the class and
    # every other instance alone.
    print("\n=== opt-in isolation ===")
    before = (BlockQuantCache._k_tensor, BlockQuantCache._v_tensor)
    fused = FusedCache()
    default = BlockQuantCache()
    check("opt-in does not patch the class",
          (BlockQuantCache._k_tensor, BlockQuantCache._v_tensor) == before)
    check("default cache stays on the shipped path", default.fused_decode is False)
    check("fused cache records the flag", fused.fused_decode is True)
    check("enabling one cache does not enable another",
          default.fused_decode is False and fused.fused_decode is True)
    check("no leftover monkey-patch bookkeeping",
          not hasattr(BlockQuantCache, "_shipped_k_tensor")
          and not hasattr(BlockQuantCache, "_shipped_v_tensor"))

    # ---- device correctness: never hardcode a device ----
    print("\n=== device correctness ===")
    torch.manual_seed(5)
    c = FusedCache(block=32, bits=4, v_group=32)
    k = (torch.randn(B, H, 64, D, device="cuda") * 0.5).to(torch.float16)
    v = (torch.randn(B, H, 64, D, device="cuda") * 0.5).to(torch.float16)
    k_out, v_out = c.update(k, v, 0)
    check("output stays on the input device",
          k_out.device == k.device and v_out.device == v.device,
          f"{k_out.device} vs {k.device}")
    check("output is contiguous for attention",
          k_out.is_contiguous() and v_out.is_contiguous())

    print()
    if failures:
        print(f"VERIFY FAILED: {len(failures)} check(s) failed")
        for name in failures:
            print(f"  - {name}")
        return 1
    print("VERIFIED: fused decode is bit-identical to the shipped path, does not "
          "recompile per chunk, and preserves dtype/device")
    return 0


if __name__ == "__main__":
    sys.exit(main())
