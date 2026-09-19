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

    def __init__(self, block=32, bits=4, v_group=32, meta_dtype=torch.float32):
        super().__init__()
        self.block = block
        self.bits = bits
        self.v_group = v_group
        self.meta_dtype = meta_dtype
        self._stored = {}  # layer -> dict with 'k' and 'v' block lists
        self._residual = {}  # layer -> (k_res, v_res)

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

    def _decode_all(self, bucket, group, row_width, group_shape):
        """Decode every stored block in ONE codec call.

        Encoding happens per block, so the concatenated buffers are in block-major
        group order: block0's groups, then block1's, and so on. That order is
        exactly what the codec expects, so the blocks can be decoded together and
        the result reordered with a reshape/permute on the decoded floats -- no
        bit-level work. Doing this per block instead cost 256 codec calls per
        layer per step, which is what made the host-side cache non-viable.

        `row_width` is the number of values per row (the block size for K, the
        head dim for V); it is not always the group size, which is why it is a
        separate argument.
        """
        packed = torch.cat(bucket["packed"])
        scales = torch.cat(bucket["scales"]).to(torch.float32)
        zeros = torch.cat(bucket["zeros"]).to(torch.float32)
        blocks = bucket["blocks"]
        rows = bucket["rows"]
        n = blocks * rows * row_width
        flat = decode_uniform_cuda(packed, scales, zeros, n, self.bits, group)
        return flat.reshape(blocks, rows, *group_shape), blocks, rows

    def _flush_k_block(self, bucket, k_block):
        """k_block [B,H,block,D] -> one group per channel, along the token axis."""
        b, h, s, d = k_block.shape
        packed, scales, zeros, _ = self._encode(
            k_block.permute(0, 1, 3, 2).reshape(b * h * d, s), s
        )
        g = bucket["k"]
        g["packed"].append(packed)
        g["scales"].append(scales)
        g["zeros"].append(zeros)
        g["rows"], g["blocks"], g["shape"] = b * h * d, g["blocks"] + 1, (b, h, s, d)

    def _flush_v_block(self, bucket, v_block):
        """v_block [B,H,block,D] -> groups along channels within each token."""
        b, h, s, d = v_block.shape
        packed, scales, zeros, _ = self._encode(
            v_block.reshape(b * h * s, d), self.v_group
        )
        g = bucket["v"]
        g["packed"].append(packed)
        g["scales"].append(scales)
        g["zeros"].append(zeros)
        g["rows"], g["blocks"], g["shape"] = b * h * s, g["blocks"] + 1, (b, h, s, d)

    def _k_tensor(self, bucket):
        """Stored K blocks as [B,H,blocks*block,D], decoded in one codec call."""
        g = bucket["k"]
        if g["blocks"] == 0:
            return None
        b, h, s, d = g["shape"]
        groups, blocks, _rows = self._decode_all(g, s, s, (s,))
        return (
            groups.reshape(blocks, b, h, d, s)
            .permute(1, 2, 0, 4, 3)
            .reshape(b, h, blocks * s, d)
        )

    def _v_tensor(self, bucket):
        """Stored V blocks as [B,H,blocks*block,D], decoded in one codec call."""
        g = bucket["v"]
        if g["blocks"] == 0:
            return None
        b, h, s, d = g["shape"]
        per_row = d // self.v_group
        groups, blocks, _rows = self._decode_all(
            g, self.v_group, d, (per_row, self.v_group)
        )
        return (
            groups.reshape(blocks, b, h, s, per_row, self.v_group)
            .reshape(blocks, b, h, s, d)
            .permute(1, 2, 0, 3, 4)
            .reshape(b, h, blocks * s, d)
        )

    # -- cache API --------------------------------------------------------
    def update(self, key_states, value_states, layer_idx, cache_kwargs=None):
        bucket = self._bucket(layer_idx)
        res = self._residual[layer_idx]
        if res is None:
            k_res, v_res = key_states, value_states
        else:
            k_res = torch.cat([res[0], key_states], dim=-2)
            v_res = torch.cat([res[1], value_states], dim=-2)

        # Flush every complete block. Encoded once; never re-encoded.
        while k_res.shape[-2] >= self.block:
            self._flush_k_block(bucket, k_res[..., : self.block, :])
            self._flush_v_block(bucket, v_res[..., : self.block, :])
            k_res = k_res[..., self.block :, :]
            v_res = v_res[..., self.block :, :]
        self._residual[layer_idx] = (k_res, v_res)

        # One decode call per layer for all stored blocks, instead of one per
        # block. Decoded values come back in float32, so cast to the caller's
        # dtype (fp16 in the model) before concatenating with the residual.
        k_stored, v_stored = self._k_tensor(bucket), self._v_tensor(bucket)
        k_parts = [] if k_stored is None else [k_stored.to(k_res.dtype)]
        v_parts = [] if v_stored is None else [v_stored.to(v_res.dtype)]
        k_parts.append(k_res)
        v_parts.append(v_res)
        return torch.cat(k_parts, dim=-2), torch.cat(v_parts, dim=-2)

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
