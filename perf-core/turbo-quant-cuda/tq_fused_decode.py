"""Triton fused KV decode for BlockQuantCache. Opt-in; falls back per call.

Why
---
The shipped decode chain in `BlockQuantCache._k_tensor` / `_v_tensor` is:
`decode_uniform_cuda` (a generic, geometry-blind codec) -> `.to(dtype)` ->
reshape/permute/reshape -> one `.copy_()` into the prealloc buffer. At the real
serving geometry (Qwen2.5-3B has 2 KV heads) each of those is a small,
launch-bound op, so launch count dominates, not bandwidth. Measured on an RTX
3090 Ti at the shipping geometry (B=1, H=2, D=128, block=32, 256-token chunks,
8K total), the shipped chain costs 2.27 ms per chunk; these kernels cost
0.21 ms for the same bytes, a 9.8x reduction, because each element is written
straight to its final position and the intermediate tensors never exist.

Two constraints make this safe rather than merely fast. Both are enforced by
`tq_block_cache_fused_decode_check.py`.

1. NO RECOMPILE STORM. Every value that changes from chunk to chunk (start
   token, block count, buffer size, destination strides) is a RUNTIME SCALAR,
   never `tl.constexpr`. Triton specializes on constexpr values, so making them
   constexpr recompiles the kernel on every streaming call. The first version of
   this kernel did exactly that: streaming 32 chunks produced 32 compiled
   variants at a 507 ms median per call, which is worse than the entire shipped
   decode budget for a whole 8K prefill. Only values fixed for a model's
   lifetime (batch, heads, head dim, block size, group, output dtype) are
   constexpr. With offsets as runtime args, 32 chunks produce exactly 1 variant.

2. NO FMA CONTRACTION. The shipped path dequantizes as
   `(quant_fp32 * scales_fp32) + zeros_fp32`: two separate fp32 ops, so the
   product is rounded to fp32 BEFORE the add. A plain `q * s + z` in Triton lets
   the compiler contract that into an FMA, which rounds once and lands 1 ULP
   away on about 41 values per million. The kernels use explicit `mul.rn.f32`
   and `add.rn.f32`, which round their own results and reproduce the shipped
   arithmetic exactly, so the output is bit-identical.

Usage: construct `BlockQuantCache(fused_decode=True)`, which routes `_k_tensor`
and `_v_tensor` through these functions and falls back per call.

How much this is worth
----------------------
The speedup is real but small in context. In the cache-isolated streaming
harness this is ~2.2x at 12K-32K. In a full model forward it is roughly
parity, because the cache's decode is only ~0.1% of a streaming chunk's CUDA
kernel time (measured at both 8K and 32K; attention is ~80% of the chunk).
Enable it because it is free and bit-identical, not because it will show up
as a model-level speedup.
"""

import torch

try:
    import triton
    import triton.language as tl

    TRITON_FUSED_AVAILABLE = True
except ImportError:  # Triton is optional; the shipped PyTorch path always works
    TRITON_FUSED_AVAILABLE = False


# Output dtype as a Triton type object. The cast happens inside the kernel, so
# the destination pointer is written in the caller's dtype with no separate
# `.to()` pass. These must be real `tl.dtype` objects, not strings: `val.to()`
# calls `.scalar` on the argument.
if TRITON_FUSED_AVAILABLE:
    _TRITON_DTYPE = {
        torch.float16: tl.float16,
        torch.bfloat16: tl.bfloat16,
        torch.float32: tl.float32,
    }
else:
    _TRITON_DTYPE = {}


if TRITON_FUSED_AVAILABLE:

    @triton.jit
    def _dequant_round_then_add(q, scale, zero):
        """`(q * scale) + zero` with the product rounded before the add.

        The shipped PyTorch path is two separate fp32 ops, so `q * scale` is
        rounded to fp32 on its own before the add. Writing `q * scale + zero`
        lets the compiler contract it into a single FMA, which rounds only once
        and lands 1 ULP away on ~41 of every million values. `mul.rn.f32` and
        `add.rn.f32` are the round-to-nearest-even scalar forms: each rounds
        its own result, so the intermediate rounding matches and the output is
        bit-identical.
        """
        prod = tl.inline_asm_elementwise(
            "mul.rn.f32 $0, $1, $2;", "=f,f,f", [q, scale],
            dtype=tl.float32, is_pure=True, pack=1,
        )
        return tl.inline_asm_elementwise(
            "add.rn.f32 $0, $1, $2;", "=f,f,f", [prod, zero],
            dtype=tl.float32, is_pure=True, pack=1,
        )

    @triton.jit
    def fused_k_kernel(
        packed_ptr, scales_ptr, zeros_ptr, dst_ptr,
        n_elements,
        start_tok, total_tok, dst_stride_tok, dst_stride_b, blocks,
        B: tl.constexpr, H: tl.constexpr, D: tl.constexpr, S: tl.constexpr,
        BLOCK_SIZE: tl.constexpr, DST_DTYPE: tl.constexpr,
    ):
        """Decode K blocks [start_tok, start_tok + blocks*S) into dst.

        K is encoded per channel: a quantization group is the `S` tokens of one
        block along one (batch, head, channel), so the packed layout is
        block-major then row-within-block. The destination is [B, H, total_tok,
        D], and the kernel computes each element's destination offset directly.

        `n_elements` indexes the OUTPUT in [B, H, blocks*S, D] order, which is
        also the order the packed blocks are stored in, so the input side needs
        no gather -- just the nibble unpack.
        """
        offs = tl.program_id(0) * BLOCK_SIZE + tl.arange(0, BLOCK_SIZE)
        mask = offs < n_elements

        d = offs % D
        rem = offs // D
        tok_local = rem % (blocks * S)
        rem = rem // (blocks * S)
        h = rem % H
        b = rem // H

        # K group order: (block, b, h, d), token within block.
        row_within = b * (H * D) + h * D + d
        block_idx = tok_local // S
        s_idx = tok_local % S

        # 4-bit fast path: two nibbles per byte, the encoder writes the even
        # element low and the odd element high.
        linear = (block_idx * (B * H * D) + row_within) * S + s_idx
        byte_idx = linear >> 1
        is_high = (linear & 1) != 0
        byte_val = tl.load(packed_ptr + byte_idx, mask=mask, other=0)
        nib = tl.where(is_high, (byte_val >> 4) & 0x0F, byte_val & 0x0F).to(tl.float32)

        group_idx = block_idx * (B * H * D) + row_within
        scale = tl.load(scales_ptr + group_idx, mask=mask, other=0.0)
        zero = tl.load(zeros_ptr + group_idx, mask=mask, other=0.0)
        val = _dequant_round_then_add(nib, scale, zero)

        dst_off = (
            b * dst_stride_b
            + h * (dst_stride_tok * total_tok)
            + (start_tok + tok_local) * dst_stride_tok
            + d
        )
        tl.store(dst_ptr + dst_off, val.to(DST_DTYPE), mask=mask)

    @triton.jit
    def fused_v_kernel(
        packed_ptr, scales_ptr, zeros_ptr, dst_ptr,
        n_elements,
        start_tok, total_tok, dst_stride_tok, dst_stride_b, blocks,
        B: tl.constexpr, H: tl.constexpr, D: tl.constexpr, S: tl.constexpr,
        V_GROUP: tl.constexpr, BLOCK_SIZE: tl.constexpr, DST_DTYPE: tl.constexpr,
    ):
        """Decode V blocks [start_tok, start_tok + blocks*S) into dst.

        V is encoded per token: a quantization group is the `V_GROUP` channels
        of one (block, batch, head, token-within-block), so the packed layout
        is block-major then row-within-block. The destination is [B, H,
        total_tok, D], identical to K.
        """
        offs = tl.program_id(0) * BLOCK_SIZE + tl.arange(0, BLOCK_SIZE)
        mask = offs < n_elements

        d = offs % D
        rem = offs // D
        tok_local = rem % (blocks * S)
        rem = rem // (blocks * S)
        h = rem % H
        b = rem // H

        # V group order: (block, b, h, token_within_block), channels within.
        s_idx = tok_local % S
        row_within = b * (H * S) + h * S + s_idx
        block_idx = tok_local // S

        linear = block_idx * (B * H * S) * D + row_within * D + d
        byte_idx = linear >> 1
        is_high = (linear & 1) != 0
        byte_val = tl.load(packed_ptr + byte_idx, mask=mask, other=0)
        nib = tl.where(is_high, (byte_val >> 4) & 0x0F, byte_val & 0x0F).to(tl.float32)

        group_idx = (
            (block_idx * (B * H * S) + row_within) * (D // V_GROUP) + d // V_GROUP
        )
        scale = tl.load(scales_ptr + group_idx, mask=mask, other=0.0)
        zero = tl.load(zeros_ptr + group_idx, mask=mask, other=0.0)
        val = _dequant_round_then_add(nib, scale, zero)

        dst_off = (
            b * dst_stride_b
            + h * (dst_stride_tok * total_tok)
            + (start_tok + tok_local) * dst_stride_tok
            + d
        )
        tl.store(dst_ptr + dst_off, val.to(DST_DTYPE), mask=mask)

else:  # pragma: no cover - only reached on builds without Triton
    fused_k_kernel = None
    fused_v_kernel = None


def _supported(cache, bucket, dtype):
    """Whether the kernels can handle this call; anything else uses the shipped path.

    The kernels implement the 4-bit nibble layout only. 2-bit and 3-bit pack
    differently (3-bit straddles byte boundaries) and go to the shipped codec,
    which is correct for them. CPU tensors and unknown dtypes likewise.
    """
    if not TRITON_FUSED_AVAILABLE or cache.bits != 4:
        return False
    if dtype not in _TRITON_DTYPE:
        return False
    group = bucket["k"]
    if group["blocks"] == 0 or not group["packed"]:
        return False
    if not group["packed"][0].is_cuda:
        return False
    b, h, s, d = group["shape"]
    # The nibble unpack reads two adjacent elements per byte, so the row width
    # must be even; head dim is the only width involved in either layout.
    return b > 0 and h > 0 and s > 0 and d > 0 and d % 2 == 0


def _fused_decode(cache, bucket, kind, dtype):
    """Decode stored blocks incrementally, fused, into the prealloc buffer.

    The bookkeeping here is deliberately identical to the shipped
    `_k_tensor` / `_v_tensor`: same per-dtype cache dict, same geometric buffer
    growth, same invalidation when the block count shrinks. Only the body
    differs -- where the shipped version calls the generic codec and then
    copies the result into the buffer, this launches one kernel that writes the
    new tail in place, so no intermediate tensor and no `.copy_()` exist.
    """
    group = bucket[kind]
    b, h, s, d = group["shape"]
    key = "decoded_k" if kind == "k" else "decoded_v"
    cached = bucket.get(key)
    if cached is None or cached["dtype"] != dtype or cached["blocks"] > group["blocks"]:
        cached = {"buffer": None, "blocks": 0, "capacity_blocks": 0, "dtype": dtype}
        bucket[key] = cached

    start = cached["blocks"]
    if start < group["blocks"]:
        blocks = group["blocks"] - start

        # Grow geometrically, carrying the live prefix forward. The prefix copy
        # is the same unavoidable copy the shipped path pays when the buffer
        # reallocates; it happens O(log n) times, not per chunk.
        cap = cached["capacity_blocks"]
        while cap < group["blocks"]:
            cap = cap * 2 if cap > 0 else blocks
        if cached["buffer"] is None or cached["capacity_blocks"] < cap:
            new_buf = torch.empty(
                b, h, cap * s, d, dtype=dtype, device=group["packed"][0].device
            )
            if cached["buffer"] is not None:
                new_buf[:, :, : start * s, :].copy_(cached["buffer"][:, :, : start * s, :])
            cached["buffer"] = new_buf
            cached["capacity_blocks"] = cap

        buffer = cached["buffer"]
        total_tok = cap * s

        packed = torch.cat(group["packed"][start:])
        scales = torch.cat(group["scales"][start:]).to(torch.float32)
        zeros = torch.cat(group["zeros"][start:]).to(torch.float32)
        # One element per output value; `rows` already folds in the batch and
        # head count, and the trailing factor is the row width.
        n_elements = blocks * group["rows"] * (s if kind == "k" else d)
        grid = (triton.cdiv(n_elements, 1024),)
        triton_dtype = _TRITON_DTYPE[dtype]

        if kind == "k":
            fused_k_kernel[grid](
                packed, scales, zeros, buffer, n_elements,
                start * s, total_tok, d, h * total_tok * d, blocks,
                B=b, H=h, D=d, S=s, BLOCK_SIZE=1024, DST_DTYPE=triton_dtype,
            )
        else:
            fused_v_kernel[grid](
                packed, scales, zeros, buffer, n_elements,
                start * s, total_tok, d, h * total_tok * d, blocks,
                B=b, H=h, D=d, S=s, V_GROUP=cache.v_group, BLOCK_SIZE=1024,
                DST_DTYPE=triton_dtype,
            )
        cached["blocks"] = group["blocks"]

    return cached["buffer"][:, :, : cached["blocks"] * s, :]


def fused_k_tensor(self, bucket, dtype=torch.float16):
    """Fused decode for K, with a per-call fallback to the shipped path.

    Falls back for any call the kernels do not cover: non-4-bit layouts (2 and
    3 bit pack differently, and 3-bit straddles byte boundaries), CPU tensors,
    or a dtype the kernel has no cast target for. The shipped path is correct
    for all of those, so the fallback is per call rather than per cache.
    """
    if not _supported(self, bucket, dtype):
        return self._k_tensor_shipped(bucket, dtype)
    return _fused_decode(self, bucket, "k", dtype)


def fused_v_tensor(self, bucket, dtype=torch.float16):
    """Fused decode for V, with the same per-call fallback as `fused_k_tensor`."""
    if not _supported(self, bucket, dtype):
        return self._v_tensor_shipped(bucket, dtype)
    return _fused_decode(self, bucket, "v", dtype)


def fused_decode_available():
    """True when the fused decode can run on this build."""
    return TRITON_FUSED_AVAILABLE and torch.cuda.is_available()
