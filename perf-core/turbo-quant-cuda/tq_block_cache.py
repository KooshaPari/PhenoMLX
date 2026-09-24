"""A resident packed KV cache that quantizes each block exactly once.

Why this exists
---------------
transformers' `QuantizedCache` re-quantizes its entire accumulated prefix every
time its FP16 residual window fills, measured at 64 round-trips across an 8K
window. Varying `residual_length` showed that policy is a major driver of the 8K
quality collapse (8x fewer re-quantizations bought ~5x better quality), so the
obvious next test is the design the docs already specified: quantize each block
once when it fills and never revisit it.

This also puts the repo's own codec (`turbo_quant_cuda.py`) into a resident cache
for the first time, grouped the way the hook experiments say it should be:

  K   : per-channel blocks -- a block of `block` consecutive tokens is encoded
        group-wise along the token axis, so one outlier channel cannot dictate
        the scale of its neighbours (the fix the hook harness identified)
  V   : per-token grouping along channels, which the hooks measured as free
  both: a partial trailing block stays in FP16, and stored blocks are only ever
        decoded, never re-encoded

Interface: subclasses `DynamicCache` and returns the full dequantized sequence
from `update`, so attention is unaffected.
"""

import os
import sys

import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import transformers.cache_utils as cu  # noqa: E402

from turbo_quant_cuda import decode_uniform_cuda, encode_uniform_cuda  # noqa: E402


class BlockQuantCache(cu.DynamicCache):
    """Quantize-once KV cache with per-channel K and per-token V."""

    def __init__(self, block=32, bits=4, v_group=32, meta_dtype=torch.float32,
                 fused_decode=False):
        super().__init__()
        self.block = block
        self.bits = bits
        self.v_group = v_group
        self.meta_dtype = meta_dtype
        self._stored = {}  # layer -> dict with 'k' and 'v' block lists
        self._residual = {}  # layer -> (k_res, v_res)
        # Opt in to the Triton fused decode (tq_fused_decode). This is an
        # INSTANCE attribute, not a class-level install: patching the class
        # would make the first opt-in silently change every other cache in the
        # process, including the reference caches the equivalence tests compare
        # against. Output is bit-identical to the shipped path. It is ~2.2x
        # faster on the cache's own delta decode, but that decode is only ~0.1%
        # of a streaming chunk's CUDA time, so expect roughly parity end to end
        # rather than a visible model-level speedup. Worth enabling because it
        # costs nothing and the resident set is the cache's real purpose.
        self.fused_decode = bool(fused_decode)

    # -- internal helpers -------------------------------------------------
    def _bucket(self, layer_idx):
        if layer_idx not in self._stored:
            self._stored[layer_idx] = {
                "k": {"packed": [], "scales": [], "zeros": [], "rows": 0, "blocks": 0},
                "v": {"packed": [], "scales": [], "zeros": [], "rows": 0, "blocks": 0},
            }
            self._residual[layer_idx] = None
        return self._stored[layer_idx]

    def _encode(self, tensor_rows, group):
        # The CUDA codec works in float32; the model hands us fp16.
        rows = tensor_rows.reshape(-1, group).contiguous().to(torch.float32)
        packed, scales, zeros = encode_uniform_cuda(rows.reshape(-1), self.bits, group)
        # Metadata is the second-largest term in the resident footprint, so let the
        # caller choose its precision (fp16 halves it) rather than hardcoding fp32.
        if self.meta_dtype != torch.float32:
            scales = scales.to(self.meta_dtype)
            zeros = zeros.to(self.meta_dtype)
        return packed, scales, zeros, rows.shape

    def _decode_all(self, bucket, group, row_width, group_shape, from_block=0):
        """Decode stored blocks [from_block:] in ONE codec call.

        Encoding happens per block, so the concatenated buffers are in block-major
        group order: block0's groups, then block1's, and so on. That order is
        exactly what the codec expects, so the blocks can be decoded together and
        the result reordered with a reshape/permute on the decoded floats -- no
        bit-level work. Doing this per block instead cost 256 codec calls per
        layer per step, which is what made the host-side cache non-viable.

        `row_width` is the number of values per row (the block size for K, the
        head dim for V); it is not always the group size, which is why it is a
        separate argument.

        `from_block` skips the first N blocks: with the per-dtype decoded
        tensor cached (see `_k_tensor`), only the tail needs decoding.
        """
        packed = torch.cat(bucket["packed"][from_block:])
        scales = torch.cat(bucket["scales"][from_block:]).to(torch.float32)
        zeros = torch.cat(bucket["zeros"][from_block:]).to(torch.float32)
        blocks = bucket["blocks"] - from_block
        rows = bucket["rows"]
        n = blocks * rows * row_width
        flat = decode_uniform_cuda(packed, scales, zeros, n, self.bits, group)
        return flat.reshape(blocks, rows, *group_shape), blocks, from_block

    def _flush_k_block(self, bucket, k_block):
        """k_block [B,H,m*block,D] -> one group per channel, along the token axis.

        Accepts any whole number of blocks m >= 1 and encodes them in ONE codec
        call: at the real flush shape the codec is launch-bound, not
        bandwidth-bound (0.46 ms at 114,688 elements and 0.46 ms at 917,504 --
        see encode_scale_bench.py), so batching the up-to-8 blocks a chunk
        produces is nearly free bandwidth and saves 7 kernel-launch round
        trips. The output is split back into per-block entries, so what lands
        in the bucket lists is bit-identical to the per-block version.
        """
        b, h, tok, d = k_block.shape
        m = tok // self.block
        s = self.block
        # Blocks must stay outermost in the group order, exactly as m
        # consecutive per-block calls would append them: group axis is
        # (block, b, h, d), so the permute pairs each channel with the m
        # token slabs of its own block only.
        packed, scales, zeros, _ = self._encode(
            k_block.reshape(b, h, m, s, d)
            .permute(2, 0, 1, 4, 3)
            .reshape(b * h * m * d, s),
            s,
        )
        g = bucket["k"]
        g["packed"].extend(packed.split(b * h * d * s * self.bits // 8))
        g["scales"].extend(scales.split(b * h * d))
        g["zeros"].extend(zeros.split(b * h * d))
        g["rows"], g["blocks"], g["shape"] = b * h * d, g["blocks"] + m, (b, h, s, d)

    def _flush_v_block(self, bucket, v_block):
        """v_block [B,H,m*block,D] -> groups along channels within each token.

        Same one-call batching as `_flush_k_block`.
        """
        b, h, tok, d = v_block.shape
        m = tok // self.block
        rows = b * h * self.block
        # Same block-outer ordering as K: per-block calls appended (b, h,
        # token-within-block) rows, so the batched flatten must be
        # (block, b, h, token, channel), not (b, h, token-across-blocks).
        packed, scales, zeros, _ = self._encode(
            v_block.reshape(b, h, m, self.block, d)
            .permute(2, 0, 1, 3, 4)
            .reshape(m * rows, d),
            self.v_group,
        )
        g = bucket["v"]
        g["packed"].extend(packed.split(rows * d * self.bits // 8))
        g["scales"].extend(scales.split(rows * d // self.v_group))
        g["zeros"].extend(zeros.split(rows * d // self.v_group))
        g["rows"], g["blocks"], g["shape"] = rows, g["blocks"] + m, (b, h, self.block, d)

    def _k_tensor(self, bucket, dtype=torch.float16):
        """Stored K blocks as [B,H,blocks*block,D], decoded incrementally.

        Dispatch to the Triton fused decode when this cache opted in. The
        import is local and the flag is per instance, so a process that never
        constructs `fused_decode=True` never imports Triton and never changes
        behavior. The fused path falls back to `_k_tensor_shipped` for any
        geometry it does not cover (non-4-bit, CPU tensors, exotic dtypes).
        """
        if self.fused_decode:
            from tq_fused_decode import fused_k_tensor

            return fused_k_tensor(self, bucket, dtype)
        return self._k_tensor_shipped(bucket, dtype)

    def _k_tensor_shipped(self, bucket, dtype=torch.float16):
        """Stored K blocks as [B,H,blocks*block,D], decoded incrementally.

        Stored blocks are immutable once encoded, so the decoded tensor is
        cached per dtype and only the NEW blocks are decoded each call (delta
        decode); the full re-decode every chunk made prefill O(n^2) in codec
        work -- 4,224 block-decodes per layer over an 8K prefill against 256
        encodes. The cache is invalidated if `blocks` shrinks (never happens
        in the cache lifecycle, but keeps the invariant honest).

        The buffer grows geometrically: each new decode only copies the
        newly-decoded tail into the buffer (no full-prefix cat), so the
        per-call cost stays constant instead of growing with prefix length.
        """
        g = bucket["k"]
        if g["blocks"] == 0:
            return None
        b, h, s, d = g["shape"]
        cached = bucket.get("decoded_k")
        if cached is None or cached["dtype"] != dtype or cached["blocks"] > g["blocks"]:
            cached = {"buffer": None, "blocks": 0, "capacity_blocks": 0, "dtype": dtype}
            bucket["decoded_k"] = cached
        start = cached["blocks"]
        if start < g["blocks"]:
            # Decode new tail into a temporary
            groups, blocks, _rows = self._decode_all(
                g, s, s, (s,), from_block=start
            )
            new = (
                groups.to(dtype).reshape(blocks, b, h, d, s)
                .permute(1, 2, 0, 4, 3)
                .reshape(b, h, blocks * s, d)
            )
            # Grow buffer geometrically if needed
            new_total_blocks = start + blocks
            cap = cached["capacity_blocks"]
            while cap < new_total_blocks:
                cap = cap * 2 if cap > 0 else blocks
            if cached["buffer"] is None or cached["capacity_blocks"] < cap:
                new_buf = torch.empty(b, h, cap * s, d, dtype=dtype,
                                       device=new.device)
                if cached["buffer"] is not None:
                    new_buf[:, :, :start * s, :].copy_(
                        cached["buffer"][:, :, :start * s, :])
                cached["buffer"] = new_buf
                cached["capacity_blocks"] = cap
            # Write new tail
            cached["buffer"][:, :, start * s:new_total_blocks * s, :].copy_(new)
            cached["blocks"] = new_total_blocks
        return cached["buffer"][:, :, :cached["blocks"] * s, :]

    def _v_tensor(self, bucket, dtype=torch.float16):
        """Stored V blocks, dispatched like `_k_tensor` above."""
        if self.fused_decode:
            from tq_fused_decode import fused_v_tensor

            return fused_v_tensor(self, bucket, dtype)
        return self._v_tensor_shipped(bucket, dtype)

    def _v_tensor_shipped(self, bucket, dtype=torch.float16):
        """Stored V blocks as [B,H,blocks*block,D], decoded incrementally.

        Same delta-decode strategy as `_k_tensor`, with the same growing
        buffer so the per-call cost stays constant.
        """
        g = bucket["v"]
        if g["blocks"] == 0:
            return None
        b, h, s, d = g["shape"]
        per_row = d // self.v_group
        cached = bucket.get("decoded_v")
        if cached is None or cached["dtype"] != dtype or cached["blocks"] > g["blocks"]:
            cached = {"buffer": None, "blocks": 0, "capacity_blocks": 0, "dtype": dtype}
            bucket["decoded_v"] = cached
        start = cached["blocks"]
        if start < g["blocks"]:
            groups, blocks, _rows = self._decode_all(
                g, self.v_group, d, (per_row, self.v_group), from_block=start
            )
            new = (
                groups.to(dtype).reshape(blocks, b, h, s, per_row, self.v_group)
                .reshape(blocks, b, h, s, d)
                .permute(1, 2, 0, 3, 4)
                .reshape(b, h, blocks * s, d)
            )
            new_total_blocks = start + blocks
            cap = cached["capacity_blocks"]
            while cap < new_total_blocks:
                cap = cap * 2 if cap > 0 else blocks
            if cached["buffer"] is None or cached["capacity_blocks"] < cap:
                new_buf = torch.empty(b, h, cap * s, d, dtype=dtype,
                                       device=new.device)
                if cached["buffer"] is not None:
                    new_buf[:, :, :start * s, :].copy_(
                        cached["buffer"][:, :, :start * s, :])
                cached["buffer"] = new_buf
                cached["capacity_blocks"] = cap
            cached["buffer"][:, :, start * s:new_total_blocks * s, :].copy_(new)
            cached["blocks"] = new_total_blocks
        return cached["buffer"][:, :, :cached["blocks"] * s, :]

    # -- cache API --------------------------------------------------------
    def update(self, key_states, value_states, layer_idx, cache_kwargs=None):
        bucket = self._bucket(layer_idx)
        res = self._residual[layer_idx]
        if res is None:
            k_res, v_res = key_states, value_states
        else:
            k_res = torch.cat([res[0], key_states], dim=-2)
            v_res = torch.cat([res[1], value_states], dim=-2)

        # Flush every complete block. Encoded once; never re-encoded. All
        # complete blocks are flushed in ONE codec call per K and per V
        # (batched -- see `_flush_k_block`); the trailing partial block stays
        # in FP16 as before.
        n_complete = k_res.shape[-2] // self.block
        if n_complete:
            m = n_complete * self.block
            self._flush_k_block(bucket, k_res[..., :m, :])
            self._flush_v_block(bucket, v_res[..., :m, :])
            k_res = k_res[..., m:, :]
            v_res = v_res[..., m:, :]
        self._residual[layer_idx] = (k_res, v_res)

        # One decode call per layer for all stored blocks, instead of one per
        # block. _k_tensor / _v_tensor decode in fp32 and cast to the caller's
        # dtype before the permute (so the post-permute contig copy is in the
        # caller's dtype, not fp32 -- see `_k_tensor`); concatenate with the
        # residual, which is already in that dtype. When the residual is empty
        # (the shipped config has STEP % BLOCK == 0 so this is every call) and
        # we already have a stored tensor, the final torch.cat would copy zero
        # bytes at full launch overhead -- return the cached tensor directly.
        # The cache_cat inside _k_tensor still runs (it joins newly-decoded
        # blocks to the cached full tensor); only the *return* cat is skipped.
        k_stored = self._k_tensor(bucket, k_res.dtype)
        v_stored = self._v_tensor(bucket, v_res.dtype)
        if k_stored is None:
            k_out = k_res
        elif k_res.shape[-2] == 0:
            k_out = k_stored
        else:
            k_out = torch.cat([k_stored, k_res], dim=-2)
        if v_stored is None:
            v_out = v_res
        elif v_res.shape[-2] == 0:
            v_out = v_stored
        else:
            v_out = torch.cat([v_stored, v_res], dim=-2)
        return k_out, v_out

    def get_seq_length(self, layer_idx=None):
        """Tokens currently cached, including the FP16 residual.

        Must default the same way the parent does: the model calls this with no
        argument to size its causal mask. Defaulting to None made the lookup miss
        and report 0, so the mask assumed an empty prefix while attention received
        the full cached K/V -- correct values, garbage output, at every context
        length. Any layer's cache has the same length, so 0 is a safe default.
        """
        if layer_idx is None:
            layer_idx = 0
        bucket = self._stored.get(layer_idx)
        total = 0
        if bucket:
            total = bucket["k"]["blocks"] * bucket["k"]["shape"][2]
        res = self._residual.get(layer_idx)
        if res is not None and res[0] is not None:
            total += res[0].shape[-2]
        return total

    def get_mask_sizes(self, cache_position, layer_idx=None):
        """Override so the model builds the causal mask for the right KV length.

        The parent's version asks `self.self_attention_cache` for the length,
        but `update` here never appends to that field (we return our own K/V
        directly). Without this override the mask is sized for an empty prefix
        even when the cache holds thousands of tokens, which is the second half
        of the "garbage at every context length" failure -- the first half is
        `get_seq_length` returning 0.
        """
        if layer_idx is None:
            layer_idx = 0
        query_length = cache_position.shape[0]
        return self.get_seq_length(layer_idx) + query_length, 0

    # -- accounting -------------------------------------------------------
    def byte_breakdown(self):
        """Resident bytes, split so the metadata cost is visible.

        Payload is the packed quantized bits; metadata is the fp32 scale/zero
        pair per group; residual is the partial trailing block kept in FP16.
        Measured at block=32 the metadata is ~22% of the total, which is why the
        effective reduction is well short of the nominal 4x. Switching the
        metadata to fp16 would halve that term.
        """
        payload = metadata = residual = 0
        for bucket in self._stored.values():
            for kind in ("k", "v"):
                g = bucket[kind]
                payload += sum(p.numel() for p in g["packed"])
                metadata += sum(s.numel() * s.element_size() for s in g["scales"])
                metadata += sum(z.numel() * z.element_size() for z in g["zeros"])
        for res in self._residual.values():
            if res is not None and res[0] is not None:
                residual += res[0].numel() * res[0].element_size()
                residual += res[1].numel() * res[1].element_size()
        return {
            "payload": payload,
            "metadata": metadata,
            "residual": residual,
            "total": payload + metadata + residual,
        }

    def packed_bytes(self):
        """Total resident bytes for KV (payload + metadata + residual)."""
        return self.byte_breakdown()["total"]


def fp16_kv_bytes(cache_like_dims):
    """Reference: bytes the same sequence would occupy in FP16 KV."""
    b, h, s, d = cache_like_dims
    return 2 * b * h * s * d * 2
