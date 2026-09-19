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
            self._stored[layer_idx] = {"k": [], "v": []}
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
        return (packed, scales, zeros, rows.shape)

    def _decode(self, entry, group):
        packed, scales, zeros, row_shape = entry
        n = 1
        for dim in row_shape:
            n *= int(dim)
        # The codec decodes in float32, so upcast whatever precision we stored.
        scales = scales.to(torch.float32)
        zeros = zeros.to(torch.float32)
        flat = decode_uniform_cuda(packed, scales, zeros, n, self.bits, group)
        return flat.reshape(row_shape)

    def _quantize_k_block(self, k_block):
        """k_block: [B, H, block, D] -> per-channel encode (group along tokens)."""
        b, h, s, d = k_block.shape
        rows = k_block.permute(0, 1, 3, 2).reshape(b * h * d, s)
        return self._encode(rows, s), (b, h, s, d)

    def _quantize_v_block(self, v_block):
        """v_block: [B, H, block, D] -> per-token encode (group along channels)."""
        b, h, s, d = v_block.shape
        rows = v_block.reshape(b * h * s, d)
        return self._encode(rows, self.v_group), (b, h, s, d)

    def _dequantize_k_block(self, entry, meta):
        b, h, s, d = meta
        rows = self._decode(entry, s)
        return rows.reshape(b, h, d, s).permute(0, 1, 3, 2)

    def _dequantize_v_block(self, entry, meta):
        b, h, s, d = meta
        rows = self._decode(entry, self.v_group)
        return rows.reshape(b, h, s, d)

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
            k_blk = k_res[..., : self.block, :]
            v_blk = v_res[..., : self.block, :]
            k_entry, k_meta = self._quantize_k_block(k_blk)
            v_entry, v_meta = self._quantize_v_block(v_blk)
            bucket["k"].append((k_entry, k_meta))
            bucket["v"].append((v_entry, v_meta))
            k_res = k_res[..., self.block :, :]
            v_res = v_res[..., self.block :, :]
        self._residual[layer_idx] = (k_res, v_res)

        k_parts = [self._dequantize_k_block(e, m) for e, m in bucket["k"]]
        v_parts = [self._dequantize_v_block(e, m) for e, m in bucket["v"]]
        # Decoded blocks come back in float32; concatenate in the caller's dtype
        # (fp16 in the model) so attention receives what it expects.
        k_parts = [p.to(k_res.dtype) for p in k_parts]
        v_parts = [p.to(v_res.dtype) for p in v_parts]
        k_parts.append(k_res)
        v_parts.append(v_res)
        return torch.cat(k_parts, dim=-2), torch.cat(v_parts, dim=-2)

    def get_seq_length(self, layer_idx=None):
        """Tokens currently cached, including the FP16 residual."""
        buckets = self._stored.get(layer_idx, {"k": [], "v": []})
        total = sum(meta[2] for _entry, meta in buckets["k"])
        res = self._residual.get(layer_idx)
        if res is not None and res[0] is not None:
            total += res[0].shape[-2]
        return total

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
            for group in ("k", "v"):
                for entry, _meta in bucket[group]:
                    packed, scales, zeros, _shape = entry
                    payload += packed.numel()
                    metadata += scales.numel() * scales.element_size()
                    metadata += zeros.numel() * zeros.element_size()
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
