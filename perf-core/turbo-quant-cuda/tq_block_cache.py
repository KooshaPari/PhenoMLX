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

Interface
---------
transformers 5.17.0 removed `HybridCache` and made `Cache` a list of per-layer
objects, so quantization cannot live on the cache any more: the cache no longer
owns the state, the layer list does. The quantized state therefore lives in
`BlockQuantLayer`, a `DynamicLayer` that overrides `update()` and returns the
full dequantized sequence, so attention is unaffected. `BlockQuantCache` is now
a thin builder that assembles a layer list from the model config and aggregates
reporting across it.

For a hybrid model (Qwen3.5), only the full-attention layers get a
`BlockQuantLayer`; the linear-attention layers are left as transformers built
them, because their recurrent state is a fixed-size tensor that has nothing to do
with context length. `Cache.get_seq_length` / `get_mask_sizes` dispatch on
`isinstance(layer, CacheLayerMixin)`, which `BlockQuantLayer` satisfies by
inheritance, so no transformers logic needs patching.
"""

import os
import sys

import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import transformers.cache_utils as cu  # noqa: E402

from turbo_quant_cuda import decode_uniform_cuda, encode_uniform_cuda  # noqa: E402


def _layer_types_and_kwargs(config):
    """`(layer_types, layer_kwargs)` for `config`, on transformers 4.x and 5.x.

    5.17.0 exports `get_layer_types_and_kwargs`; 4.57.1 does not, and derives the
    same thing inside `DynamicCache.__init__` with no public entry point. Only the
    count matters here, and the count is `num_hidden_layers` in both versions, so
    prefer the config field and fall back to the 5.x helper for anything that
    needs the per-type dispatch.
    """
    decoder_config = config.get_text_config(decoder=True)
    helper = getattr(cu, "get_layer_types_and_kwargs", None)
    if helper is not None:
        return helper(decoder_config)
    # 4.x: no layer_types field on plain full-attention models, which is the
    # only case `__init__` has to handle (a hybrid model needs `from_config`,
    # which requires 5.x anyway).
    return ["full_attention"] * decoder_config.num_hidden_layers, {}


class BlockQuantLayer(cu.DynamicLayer):
    """Quantize-once KV state for one full-attention layer.

    The codec, block/flush policy and decode paths are unchanged from the
    single-bucket cache this was extracted from; only the ownership changed.
    Each layer owns its state outright, so there is no layer index and no dict
    keyed by one. That also makes the decoded-buffer cache per layer: the old
    design shared one decode buffer across every layer, which was only correct
    because each `update` call rewrote it. Here the buffer belongs to the layer
    that decodes into it, which is both correct and the reason peak memory scales
    with the number of quantizable layers rather than being paid once.
    """

    def __init__(self, block=32, bits=4, v_group=32, meta_dtype=torch.float32,
                 fused_decode=False, **kwargs):
        super().__init__(**kwargs)
        self.block = block
        self.bits = bits
        self.v_group = v_group
        self.meta_dtype = meta_dtype
        self._bucket_data = None
        self._res_data = None
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
    def _bucket(self):
        if self._bucket_data is None:
            self._bucket_data = {
                "k": {"packed": [], "scales": [], "zeros": [], "rows": 0, "blocks": 0},
                "v": {"packed": [], "scales": [], "zeros": [], "rows": 0, "blocks": 0},
            }
            self._res_data = None
        return self._bucket_data

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

        Dispatch to the Triton fused decode when this layer opted in. The
        import is local and the flag is per instance, so a process that never
        constructs `fused_decode=True` never imports Triton and never changes
        behavior. The fused path falls back to `_k_tensor_shipped` for any
        geometry it does not cover (non-4-bit, CPU tensors, exotic dtypes).
        """
        if self.fused_decode:
            from tq_fused_decode import fused_k_tensor

            return fused_k_tensor(self, bucket, dtype)
        return self._k_tensor_shipped(bucket, dtype)

    def _decode_cache(self, bucket, key, dtype, blocks):
        """Fetch (or create) the delta-decode bookkeeping for K or V.

        The buffer is grown in place across calls so each decode only copies the
        newly-decoded tail. `capacity_blocks` is the reservation and `blocks` is
        how much of it is real; `_grow_decode_buffer` keeps those two honest.

        A dtype change or a shrink in the stored block count throws the buffer
        away rather than trying to reconcile it: a stale buffer would otherwise
        report a prefix length the bucket no longer backs.
        """
        cached = bucket.get(key)
        if (cached is None or cached["dtype"] != dtype
                or cached["blocks"] > blocks):
            cached = {"buffer": None, "blocks": 0, "capacity_blocks": 0,
                      "dtype": dtype}
            bucket[key] = cached
        return cached

    def _grow_decode_buffer(self, cached, new, start, new_total_blocks, b, h, s, d):
        """Ensure the decode buffer holds `new_total_blocks` blocks, then copy the
        new tail in.

        The reservation grows by a bounded increment rather than by doubling.
        Doubling reserved up to 2x the tokens actually needed, and because a
        prefill that lands on a block boundary leaves the reservation exactly
        full, the FIRST single-token decode after it doubled the buffer
        immediately. On Qwen3.5-9B that turned a 1.375x footprint into 2.375x the
        moment real decoding started, which is the number a server would see.

        The increment is a multiple of the block count this call added, so a
        streaming prefill still reallocates O(n/blocks) times rather than once
        per token, but decode-after-prefill reserves only a small margin above
        what is used instead of doubling it.
        """
        cap = cached["capacity_blocks"]
        if cap < new_total_blocks:
            added = max(1, new_total_blocks - start)
            # Reserve just the blocks needed, plus a small fixed slack so the
            # single-token decodes that follow a prefill do not reallocate on
            # every one of them. The slack deliberately does NOT scale with the
            # call: an earlier version reserved `new_total + added`, which on
            # the FIRST allocation has start=0 and so reserved exactly 2x. That
            # looked fine under chunked prefill, which grows incrementally, but
            # a single-shot prefill reserved 2x immediately -- traced at
            # capacity 64 blocks for 32 used. The fixed slack keeps a large
            # prefill exact and still absorbs a run of decodes.
            cap = new_total_blocks + min(START_SLACK_BLOCKS, max(1, added))
            cached["capacity_blocks"] = cap
        if cached["buffer"] is None or cached["buffer"].shape[2] < cap * s:
            new_buf = torch.empty(b, h, cap * s, d, dtype=cached["dtype"],
                                   device=new.device)
            if cached["buffer"] is not None:
                new_buf[:, :, :start * s, :].copy_(
                    cached["buffer"][:, :, :start * s, :])
            cached["buffer"] = new_buf
        cached["buffer"][:, :, start * s:new_total_blocks * s, :].copy_(new)
        cached["blocks"] = new_total_blocks

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
        cached = self._decode_cache(bucket, "decoded_k", dtype, g["blocks"])
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
            self._grow_decode_buffer(cached, new, start, start + blocks,
                                     b, h, s, d)
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
        cached = self._decode_cache(bucket, "decoded_v", dtype, g["blocks"])
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
            self._grow_decode_buffer(cached, new, start, start + blocks,
                                     b, h, s, d)
        return cached["buffer"][:, :, :cached["blocks"] * s, :]

    # -- cache API --------------------------------------------------------
    def update(self, key_states, value_states, *args, **kwargs):
        bucket = self._bucket()
        res = self._res_data
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
        self._res_data = (k_res, v_res)
        # Record the trailing FP16 residual as this layer's `keys`/`values`, so
        # the base class and anything that reads them sees real tensors. The
        # packed state, not these, is the resident copy of the prefix.
        self.keys = k_res
        self.values = v_res
        self.is_initialized = True

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

        The layer-level signature takes no index: the state is per instance, and
        `Cache.get_seq_length` calls the layer with none.
        """
        bucket = self._bucket_data
        total = 0
        if bucket:
            total = bucket["k"]["blocks"] * bucket["k"]["shape"][2]
        res = self._res_data
        if res is not None and res[0] is not None:
            total += res[0].shape[-2]
        return total

    def get_mask_sizes(self, query_length, layer_idx=None):
        """So the model builds the causal mask for the right KV length.

        `update` returns the full dequantized sequence, so the mask must be
        sized for the whole prefix. Without this the mask assumes an empty
        prefix even when the cache holds thousands of tokens: correct values,
        garbage output, at every context length.

        `query_length` arrives as a `cache_position` TENSOR on transformers
        4.5x and as a plain int on 5.x, and the two versions disagree about
        whether the first positional argument is a tensor at all. Accepting
        both, by shape when it is a tensor, is what lets one implementation
        serve both: 4.x passes `cache_position` and 5.x passes the already
        unwrapped query length, and both want `(cache + query, 0)`.
        """
        if isinstance(query_length, torch.Tensor):
            query_length = query_length.shape[0]
        return self.get_seq_length() + query_length, 0

    def lazy_initialization(self, key_states, value_states):
        """DynamicLayer's hook. The packed state is dtype/device independent,
        so this only records what the base class records. Kept so a base-class
        call (e.g. `crop`) still finds the layer initialized after an update.
        """
        self.dtype = key_states.dtype
        self.device = key_states.device

    # -- accounting -------------------------------------------------------
    def byte_breakdown(self):
        """Resident bytes for this layer's KV, split by term.

        See `BlockQuantCache.byte_breakdown` for what each term means. The
        `decoded` term is the one that decides whether the packing saves
        anything: it is the materialized FP16 prefix, and it is as large as the
        FP16 cache this replaces.
        """
        payload = metadata = residual = decoded = 0
        bucket = self._bucket_data
        if bucket is not None:
            for kind in ("k", "v"):
                g = bucket[kind]
                payload += sum(p.numel() for p in g["packed"])
                metadata += sum(s.numel() * s.element_size() for s in g["scales"])
                metadata += sum(z.numel() * z.element_size() for z in g["zeros"])
            for key in ("decoded_k", "decoded_v"):
                c = bucket.get(key)
                if c is not None and c.get("buffer") is not None:
                    buf = c["buffer"]
                    decoded += buf.numel() * buf.element_size()
        res = self._res_data
        if res is not None and res[0] is not None:
            residual += res[0].numel() * res[0].element_size()
            residual += res[1].numel() * res[1].element_size()
        return {
            "payload": payload,
            "metadata": metadata,
            "residual": residual,
            "decoded": decoded,
            "packed_only": payload + metadata + residual,
            "total": payload + metadata + residual + decoded,
        }


START_SLACK_BLOCKS = 2


class BlockQuantCache(cu.Cache):
    """A `Cache` whose full-attention layers hold packed quantized KV.

    DEPRECATED as a serving cache, as of 2026-10. Measured on Qwen3.5-9B it
    holds ~1.38x-1.44x the bf16 KV of the same full-attention layers and decodes
    at parity, so it buys nothing while costing memory. Do not wire this into a
    new serving path expecting a 4-bit KV saving; you will get a larger cache.
    The class is kept because it is the working reference implementation and
    because retiring it means deleting the measurements that justify the
    retirement. The codec underneath it is fine on its own -- `BlockQuantLayer`
    and the `tq_codec_eval*` harness round-trip 4-bit KV at ~0.21x of fp16 with a
    0.0164 worst-case relative loss difference -- so the salvageable part is the
    codec, not the cache. To actually make 4-bit KV resident, attention has to
    dequantize inside the kernel instead of receiving a materialized prefix; see
    the module docstring for why nothing cheaper can do it.

    Construct directly for a plain full-attention model (every layer becomes a
    `BlockQuantLayer`). For a hybrid model use `from_config`, which mirrors what
    `DynamicCache(config=...)` does -- build the layer list from
    `config.layer_types` and substitute only the full-attention entries, leaving
    the linear-attention recurrent state as transformers built it.

    `update` keeps the old `(k, v, layer_idx)` signature and delegates to the
    layer. `get_seq_length` / `get_mask_sizes` keep their `layer_idx=None`
    default for the same reason they had one before: the model calls them with
    no argument to size its causal mask, and defaulting to a missing key
    reported 0, which produced correct attention values over a garbage mask at
    every context length. For a hybrid stack the default resolves to the first
    attention layer, which is what the base class does.
    """

    def __init__(self, block=32, bits=4, v_group=32, meta_dtype=torch.float32,
                 fused_decode=False, num_layers=None, config=None, **kwargs):
        self.block = block
        self.bits = bits
        self.v_group = v_group
        self.meta_dtype = meta_dtype
        self.fused_decode = bool(fused_decode)
        # How many layers to build up front. `config` is the reliable way to
        # know, but it is optional: a cache handed a `layer_idx` it has never
        # seen grows to fit (see `update`). That keeps every existing caller
        # working whether or not it passes a config, which matters because a
        # cache that must be told the model up front turns a one-line
        # construction into a per-call-site change.
        if num_layers is None:
            if config is not None:
                num_layers = len(_layer_types_and_kwargs(config)[0])
            else:
                num_layers = 1
        self._layer_kwargs = {}
        layers = [self._new_layer() for _ in range(num_layers)]
        super().__init__(layers=layers, **kwargs)

    def _new_layer(self):
        return BlockQuantLayer(block=self.block, bits=self.bits, v_group=self.v_group,
                               meta_dtype=self.meta_dtype, fused_decode=self.fused_decode,
                               **self._layer_kwargs)

    def update(self, key_states, value_states, layer_idx, *args, **kwargs):
        """Delegate to the owning layer, growing the list if it is short.

        transformers 4.x indexes `self.layers[layer_idx]` directly, so a cache
        built without a config and then asked for layer 3 would raise
        `IndexError`. Growing here keeps the old construction valid: the extra
        layers are never touched until their index arrives, so they cost
        nothing until used.
        """
        while len(self.layers) <= layer_idx:
            self.layers.append(self._new_layer())
        return self.layers[layer_idx].update(key_states, value_states, *args, **kwargs)

    # NOTE: `get_seq_length` is deliberately not overridden here.
    # `cu.Cache.get_seq_length` is already hybrid-aware: when the requested index
    # is not a `CacheLayerMixin` it redirects to the first attention layer (for
    # the default index 0) and raises for an explicit non-attention index. An
    # earlier override indexed `self.layers[layer_idx]` directly, which called
    # `get_seq_length` on a stock `LinearAttentionLayer` -- a class that has no
    # such method in 5.17, only `get_max_length`. Every masked forward pass
    # through a hybrid stack died in `create_causal_mask` before reaching a
    # single matmul. The base implementation already returns the same value for
    # the pure-attention case this cache was written against.


    @classmethod
    def from_config(cls, config, block=32, bits=4, v_group=32,
                    meta_dtype=torch.float32, fused_decode=False):
        """Build a cache for `config`'s actual layer mix.

        Mirrors `DynamicCache.__init__`: get the layer types and kwargs, then
        substitute a `BlockQuantLayer` for every entry whose mapped class is an
        attention layer (`CacheLayerMixin`). Linear/recurrent entries are kept
        as-is, because their state does not grow with context length and is not
        what this cache quantizes.
        """
        layer_types, layer_kwargs = _layer_types_and_kwargs(config)
        layers = []
        for layer_type in layer_types:
            cls_for_type = cu.DYNAMIC_LAYER_TYPE_MAPPING[layer_type]
            if issubclass(cls_for_type, cu.CacheLayerMixin):
                layers.append(BlockQuantLayer(block=block, bits=bits, v_group=v_group,
                                              meta_dtype=meta_dtype,
                                              fused_decode=fused_decode,
                                              **layer_kwargs))
            else:
                layers.append(cls_for_type(**layer_kwargs))
        self = cls(block=block, bits=bits, v_group=v_group,
                   meta_dtype=meta_dtype, fused_decode=fused_decode,
                   num_layers=0)
        # Keep the model's layer kwargs so `update` grows a replacement layer
        # with the same settings if a layer index ever exceeds the list. A
        # hybrid stack's layer count comes from the config, so this should not
        # happen, but a grown layer that silently differed would be a nasty,
        # hard-to-spot difference.
        self._layer_kwargs = layer_kwargs
        cu.Cache.__init__(self, layers=layers)
        return self

    # -- reporting --------------------------------------------------------
    def quant_layers(self):
        """The `BlockQuantLayer`s in this cache, in layer order."""
        return [layer for layer in self.layers if isinstance(layer, BlockQuantLayer)]

    @property
    def _stored(self):
        """Legacy view: {layer_idx: bucket}, read-only by convention.

        The state used to live in a dict on the cache keyed by layer index, and
        the self-tests and the chunked probe read it to assert block counts and
        bit-stability. Keying by index is no longer how anything is stored, so
        this maps the same names onto the layer objects. It exists so the
        existing checks keep working against the ported cache rather than being
        rewritten to match it -- a test that has to change every time ownership
        moves is a test that stops testing.
        """
        return {i: layer._bucket_data
                for i, layer in enumerate(self.layers)
                if isinstance(layer, BlockQuantLayer)}

    @property
    def _residual(self):
        """Legacy view: {layer_idx: (k_res, v_res)} or None. See `_stored`."""
        return {i: layer._res_data
                for i, layer in enumerate(self.layers)
                if isinstance(layer, BlockQuantLayer)}

    def _first_quant_layer(self):
        """The first quantized layer, for the legacy single-layer helpers below."""
        quant = self.quant_layers()
        if not quant:
            raise AttributeError("no BlockQuantLayer in this cache yet")
        return quant[0]

    def _k_tensor(self, bucket=None, dtype=torch.float16):
        """Legacy: decode stored K. Delegates to the first quantized layer.

        The decode methods moved onto the layer with the state. These wrappers
        keep the fused-decode equivalence check working against a cache handle,
        which is how that check is written and how it is worth keeping written:
        it compares the shipped codec against the Triton kernel at the level
        where the two can differ.
        """
        layer = self._first_quant_layer()
        return layer._k_tensor(bucket if bucket is not None else layer._bucket_data, dtype)

    def _v_tensor(self, bucket=None, dtype=torch.float16):
        """Legacy: decode stored V. See `_k_tensor`."""
        layer = self._first_quant_layer()
        return layer._v_tensor(bucket if bucket is not None else layer._bucket_data, dtype)

    def byte_breakdown(self):
        """Resident bytes for the KV this cache holds, split by term.

        Four terms, and the fourth is the one that decides whether this cache
        saves anything at all:

        - `payload`   the packed quantized bits
        - `metadata`  the fp32 scale/zero pair per group
        - `residual`  the partial trailing block kept in FP16
        - `decoded`   the materialized FP16 prefix that `_k_tensor` /
                      `_v_tensor` keep so attention can read contiguous memory

        `decoded` is the same size as the entire FP16 KV cache that this cache
        was built to replace, and it is counted here because leaving it out
        makes this method report a ~2.1x saving while the process is actually
        holding 1.38x the FP16 footprint at 32K. The packing does not replace
        the FP16 storage; it sits on top of it, and the copy that makes the
        packed data usable consumes the whole saving.

        Measured at 32K, 36 layers, block=32, D=128, fp16 as the reference:

            fp16 KV                 1152.0 MiB   1.00x
            packed 4-bit payload     288.0 MiB   0.25x
            metadata                 144.0 MiB   0.12x
            DECODED fp16 buffers    1152.0 MiB   1.00x
            total                   1584.0 MiB   1.38x

        So this cache is a LARGER DynamicCache as it currently stands: the packed
        state is additive, not a replacement. The only fix that makes "4-bit
        KV" true is to stop materializing the prefix and have attention
        dequantize inside the attention kernel.

        On latency it is parity, not a regression or a speedup. Measured with
        paired ABBA trials (5 trials of bf16/block4/block4/bf16, 8 untimed
        warm-up steps per block, 16 timed steps, median of per-trial ratios):

            context   bf16 med   block4 med   ratio med   spread   verdict
              1024     82.67 ms    87.57 ms      1.015x    16.2%    parity
              2048     77.66 ms    80.46 ms      1.006x    20.0%    parity
              4096     83.16 ms    85.37 ms      0.927x    11.9%    parity
              8192     83.62 ms    86.27 ms      0.952x    13.8%    parity

        Every spread exceeds 10%, so no ratio separates from noise and the
        honest statement is parity. An earlier single-sample harness reported
        0.907x-1.270x over the same contexts, including an apparent 1.270x
        speedup at 4096; that was an artifact. Its own bf16 baseline ran
        101.6, 107.3, 101.5 and 75.8 ms for identical 24-token decode work at
        1K/2K/4K/8K, i.e. faster at 8x the context, which cannot be a real
        scaling property -- it measured drift. Under the controlled design the
        bf16 baseline is flat at 77.7-83.6 ms across that same 8x range, which
        is what a memory-bandwidth-bound decode step should look like, and the
        1.270x disappears.

        Treat a ratio whose within-context spread exceeds ~10% as parity rather
        than as a result, and do not report single-sample decode ratios from a
        harness that runs the baseline condition first every time.

        On a hybrid model this aggregates over the full-attention layers only;
        the linear layers' fixed-size recurrent state is not KV and is not
        counted, so the FP16 reference for comparison should also be the
        full-attention layers only.

        Measured on Qwen3.5-9B (32 layers, 8 full attention at indices
        3, 7, ..., 31, 24 linear; kv_heads 4, head_dim 256, bf16 weights;
        transformers 5.17, torch 2.9.1, RTX 3090 Ti), block=32, bits=4:

            context   bf16 kv    packed    decoded     total    total/bf16
              1024     32.0 MiB  12.0 MiB  32.0 MiB   44.0 MiB      1.375x
              2048     64.0 MiB  24.0 MiB  64.0 MiB   88.0 MiB      1.375x
              4096    128.0 MiB  48.0 MiB 128.0 MiB  176.0 MiB      1.375x
              8192    256.0 MiB  96.0 MiB 256.0 MiB  352.0 MiB      1.375x

        1.375x, not a saving, and the ratio is flat in context because the
        decoded term is exactly the bf16 KV and the packed term is a fixed
        0.375x of it. The packing does not displace the prefix it compresses.

        The prefill table above is the BEST case, and a real server never sits at
        it. The decode buffers used to grow by doubling capacity, so a prefill
        that landed exactly on a block boundary left the reservation exactly
        full and the first decoded token doubled it immediately. That is the
        table plus a decoded term of 2x bf16, i.e. ~2.375x instead of 1.375x,
        and it is the number a serving process would actually report.
        `_grow_decode_buffer` now reserves the blocks it needs plus a small fixed
        slack, so the decoded term is ~1.02x of bf16.

        Measured on the real model, single-shot prefill of `ctx` tokens then 24
        decoded tokens, against the bf16 KV of the same 8 full-attention layers.
        `decoded` is the allocated prefix buffer, which is what
        `byte_breakdown()` reports and what the process actually holds:

            context   bf16 kv    packed    decoded    total   total/bf16
              1024     32.0 MiB  12.0 MiB   34.0 MiB  46.0 MiB      1.437x
              2048     64.0 MiB  24.0 MiB   66.0 MiB  90.0 MiB      1.406x
              4096    128.0 MiB  48.0 MiB  130.0 MiB 178.0 MiB      1.391x
              8192    256.0 MiB  96.0 MiB  258.0 MiB 354.0 MiB      1.383x
             16384    512.0 MiB 192.0 MiB  514.0 MiB 706.0 MiB      1.379x

        So the decoded term drops from 2.000x to 1.004x-1.062x and the total
        from ~2.375x to ~1.38x-1.44x, converging on the 1.375x prefill figure
        as context grows because the 2-block slack becomes negligible.

        The 16K decode timings in that sweep are NOT a cache result and should
        not be quoted as one. At 16K the bf16 reference and block4 degrade
        together -- 492.7 versus 490.7 ms/token -- while allocator reserve
        reaches 59.59 GiB on a 23.99 GiB card (17.53 GiB of that is weights).
        The step is spilling to system memory, so both arms are paying the same
        host round-trip and the ratio is meaningless. Reserve tracks the KV
        directly from 1024 up: 18.48, 22.97, 29.33, 59.59 GiB at 1K/4K/8K/16K,
        exceeding device capacity at 8K. Treat anything past 4K on a 24 GiB card
        as unmeasurable for decode latency on this setup, and prefer the 1K-4K
        rows where reserve still fits. The memory ratios above are unaffected,
        since byte_breakdown reports tensor sizes rather than timing.

        That is block=32, bits=4. The verdict does not depend on that choice:
        sweeping block in (16, 32, 64, 128) and bits in (4, 2) at 4096+24 tokens
        puts every configuration above bf16, because the total is packed plus a
        decoded prefix that is ~1.0x of the bf16 KV by construction.

            block  bits    packed   decoded    total
               16     4    0.439x    1.002x   1.441x
               32     4    0.379x    1.010x   1.388x
               64     4    0.348x    1.025x   1.373x
              128     4    0.332x    1.056x   1.388x
               16     2    0.314x    1.002x   1.316x
               32     2    0.254x    1.010x   1.264x
               64     2    0.223x    1.025x   1.249x
              128     2    0.208x    1.056x   1.264x

        The best configuration is block=64 at 1.373x, and it is still larger
        than the cache it replaces. Larger blocks shrink the packed term
        (0.439x -> 0.332x at 4 bits) because the per-group scale and zero
        metadata amortizes over more data, but the decoded prefix stays ~1.0x
        and then grows slightly with the block, so the total barely moves. The
        2-bit rows are shown for completeness and are disqualified anyway: 2-bit
        KV measured 0.0881x worst relative loss against a 0.05 gate, and it is
        still 1.249x or worse on memory. There is no block size that removes a
        prefix which exists because attention consumes it.

        What a fused dequantize-inside-attention kernel could buy, measured
        before writing one, so the project is not started blind. Two ceilings,
        and only one of them is worth chasing.

        Memory: deleting the materialized prefix leaves the packed term alone,
        which is 0.375x of the bf16 KV at block=32 and 0.348x at block=64.
        Verified directly -- packed_only / bf16 kv is 0.375x at 1024, 2048 and
        4096. So a perfect kernel would report 0.375x against the current
        1.379x-1.441x. That is a real 3.7x reduction and the whole reason to
        consider the work.

        Latency: essentially nothing. Growing the prefix 16x, from 8.0 MiB at
        256 tokens to 128.0 MiB at 4096, moves a decode step from 73.4 to
        81.6 ms -- a delta of 8.2 ms, and not even monotonic (1024 measured
        FASTER at 69.9 ms than 256 did). The step is dominated by fixed work:
        the 24 linear-attention layers, whose recurrent state does not grow
        with context, plus 17.53 GiB of weight reads. The KV traffic this kernel
        would eliminate is a small fraction of the step. So the fused kernel
        would cut resident memory about 3.7x and leave decode latency at parity,
        the same parity the shipped and fused-decode paths already show.

        That reframes the remaining work honestly: it is a memory project, not a
        speed project, and the reason to attempt it is capacity on a card where
        a 1.4x cache does not fit and 0.375x might. On the 24 GiB 3090 Ti used
        for these measurements, reserve already exceeds the device at 8K, so
        that constraint is real here and not hypothetical.

        Why this cache could never have paid for itself on this model, measured
        rather than assumed. The decode step of Qwen3.5-9B on this card spends
        almost none of its time moving bytes, so no change to bytes, dtype,
        layout or cache format can help. Three measurements, in order.

        1. The decoder layers are 99% of the step. Per-layer CUDA events over
           36 steady-state steps put the 32 layers at 58.78 ms against a
           59.45 ms wall clock, a 1% gap. There is no hidden cost in the
           bookends, the cache update, or the host.

        2. They are nowhere near the memory bus. The layers read 13.84 GB in
           58.78 ms, which is 235 GB/s against a 1792 GB/s peak, so 13% of peak
           and 7.6x off their own 7.72 ms floor. Split by type at ctx=4096,
           linear_attention is 49.89 ms over 10.48 GB at 11.7% of peak and
           full_attention is 17.24 ms over 3.36 GB at 10.9%. Neither type is
           bandwidth-bound. And growing context 16x, from 8 MiB of KV at 256
           tokens to 128 MiB at 4096, costs only 1.27x and 1.33x respectively,
           so the prefix is not the driver either.

        3. The step is launch-bound. Measured on an uncontended card with the
           model deliberately not loaded, a single layernorm on a [1,1,4096]
           bf16 tensor costs 86.1 us drained, a tiny 16-element elementwise op
           costs 58.8 us, and a decode-shaped GEMV reaches only 244 GB/s, 13.6%
           of peak. Thirty-two layernorms, a conservative stand-in for one
           step's norm work, cost 0.94 ms with 91% of that being host issue
           time rather than GPU work. In-model input_layernorm was attributed
           538 us for the same shape, about six times the isolated cost.

        So the ceiling measured above is the right ceiling: deleting the
        prefix would cut memory about 3.7x and leave a step that is dominated
        by how fast the host can issue hundreds of tiny kernels. On this card,
        for this model, the remedies are CUDA graphs or a compiled graph
        capture, not quantization. And with only 8 of 32 layers carrying KV at
        all, the nominal saving was capped well below 4x in the first place.

        Point 3 predicted the graph win, so the prediction was tested. Manual
        capture with `StaticCache` is bit-identical to eager and does help,
        but the size of the win is context-dependent, and that changes what
        the remedy buys, so it is measured here rather than asserted.

        What a graph actually buys, on the allocator-checked card. The first
        table here was labelled ABBA-paired but the harness ran E,E,G,G per
        round, so it measured eager before graph and graph after eager and
        absorbed any within-round drift. At 1024 its sample spread was 1.129,
        meaning max was 113% above min, so no ratio derived from it could be
        quoted. That table has been replaced with a genuine A B B A schedule
        in which each variant is sampled once at the start of a round and once
        at the end, so drift cancels to first order. Nine rounds, eight inner
        steps, first round discarded:

            ctx     eager    graph    ratio   spread E  spread G
            1024    86.99    26.02    3.34x    0.349     0.041
            4096    72.04    28.62    2.52x    0.288     0.050
            16384  225.50    73.35    3.07x    0.020     0.042

        Spread is (max-min)/median over all 18 samples per variant. Relative
        logit difference was exactly 0.0 at all three contexts, and the
        harness suppresses the ratio if it is not 0.0, so these are usable.

        THE CORRECTION THAT MATTERS: the old table concluded the win SHRINKS
        as context grows, from 2.54x to 1.30x, and read the 16K collapse as
        the signature of a host-side fix. That conclusion does not survive a
        balanced schedule. The win does not shrink with context; it holds
        between 2.5x and 3.3x across a 16x range. The old 1.30x was an
        artifact of the ordering, not a property of graphs at long context.

        The absolute latencies also moved between runs, and the eager baseline
        moved most: 225.50 ms at 16K now against 172.78 ms before. The ratio
        is the robust quantity here; the absolute eager figure on this host
        varies by roughly 30% run to run, so it should not be quoted alone.

        Two caveats still keep this from being read as a 3x speedup for the
        model:

        - The graph step is not bandwidth-bound at any of these contexts. It
          is 3.4x the 7.72 ms weight-read floor at 1024 and 9.5x it at 16384,
          so removing host launch overhead is real and large but does not by
          itself make the step compute the thing that limits it.
        - Capture cost is excluded, as it should be for steady state, but a
          graph holds a private memory pool and the decode shape is fixed at
          capture. The position must be written into a static buffer and
          filled in place, which is why the harness refills a tensor in place
          instead of passing a fresh position each step.

        So graphs are necessary on this card and they are not sufficient. They
        recover most of the launch overhead measured in point 3 at every
        context tested, but fusion on top would be required to reach the
        floor.

        FUSION PLUS GRAPH, and why it is not the answer to the 7.72 ms floor.
        `tq_fusion_graph_abba_bench.py` measures
        `torch.compile(mode="max-autotune-no-cudagraphs")` with
        `use_static_cuda_launcher = False` and manual capture. The static
        launcher MUST be disabled before the first compiled call: every
        Inductor Triton kernel would otherwise route its stream through a
        32-bit C long, and capture's side stream exceeds 2**31. With that
        setting the combination is technically sound and beat eager decisively
        at context 1024. Two independent runs of the real benchmark agree:
        eager 71.295 ms (spread 0.088) versus fusion+graph 21.117 ms (spread
        0.042), a 3.376x speedup, and on a later repeat eager 71.454 ms
        (spread 0.077) versus fusion+graph 20.306 ms (spread 0.066), a 3.519x
        speedup. Both had non-overlapping sample ranges. Eager is stable to
        within 0.2 percent across runs (71.454 / 71.295), so the speedup is
        a property of the configuration rather than of one lucky sample.

        But it does not reach the floor, and it costs exactness to get there.
        The fusion+graph step is still about 2.7x the 7.72 ms weight-read floor
        (20.306 ms on the repeat), so fusion on top of graphs did not approach
        bandwidth either. Worse, this path is numerically APPROXIMATE, not
        allocation-only: against eager the worst relative cache difference is
        about 2.3e-02, and the worst relative logit difference over lockstep
        decode was about 1.5e-02 on the first run and 9.4e-03 on the repeat
        (fusion+graph itself measured 6.4e-03 on the repeat). Both are under
        the 0.05 acceptance bound but neither is 0.0, because max-autotune
        changes the arithmetic. So the honest conclusion is:

        - For a BIT-IDENTICAL result, use manual graph over eager (the
          validated 2.5x-3.3x path above). Fusion cannot be layered on top
          without giving up bit-exactness.
        - Fusion plus graph is a real further speedup at short context but is
          approximate, and it still lands well above the weight-read floor.

        Two caveats on quoting the 1024 fusion number. It is the only context
        where fusion+graph was timed end to end. At 4096 the run does not
        fail fast, it simply never becomes affordable: after 40 minutes it had
        not printed the first measurement line, having burned one full core
        continuously. A separate attempt to run 4096 with a second copy of the
        harness alive exhausted the Windows page file instead (`OSError:
        [WinError 1455]`) and failed inside Triton heuristics. Both are host
        resource limits on max-autotune, not properties of the model or of CUDA
        graphs, and neither is a result. So 4096 and 16384 fusion numbers do not
        exist and must not be inferred from the 1024 one. The harness now
        refuses to start a second concurrent copy, because that is what produced
        the page-file failure.

        The obvious explanation was tested and is wrong, so do not repeat it.
        The natural guess is that 4096 recompiles across many dynamic shapes,
        so `automatic_dynamic_shapes = False`, `assume_static_by_default = True`
        and a raised `cache_size_limit` should collapse the compile time. Under
        exactly that configuration a bounded 600-second run at ctx 4096 still
        emitted no measurement line, with the working set climbing 4.32 GB to
        5.85 GB and a compile worker growing, which is active compilation rather
        than a deadlock. The cost is host max-autotune itself at this sequence
        length, not Dynamo recompilation, so pinning shapes does not make 4096
        affordable. There is also no inductor cache to reuse between runs
        (`torchinductor_root` does not exist under the temp dir), so every
        attempt pays the full compile again.

        The 0.05 bound above is now ENFORCED rather than only described:
        `tq_fusion_graph_abba_bench.py` carries `LOGIT_TOL` and returns a
        non-zero exit code if any context lands outside it, printing
        `acceptance gate` and a per-context `within` / `OUT OF BOUNDS` line.
        Before that, a run exceeding the bound still exited 0 and read as a
        pass. Verified end to end at ctx 1024, where it printed
        `measured 6.977e-03`, `gate: PASS`, exit 0, and separately failed as
        designed for a value of exactly 0.05, for 0.061, for a set where one
        context was good and another was not, and for an empty result set.

        A contention caveat for anyone re-running the 1024 number. Three later
        runs measured eager 77.062 ms (spread 0.216), 77.491 ms (spread 0.233)
        and 78.402 ms (spread 0.259) against 71.295 and 71.454 ms (spread
        0.042-0.088) on the two quiet runs. All three slower runs were taken
        while another agent's pytest suite held the host at 54-57, 83 and then
        99 percent CPU load, and all three are DISCARDED rather than averaged
        in. The tell is the spread: a lazy host inflates the spread as well as
        the mean, so a run whose spread exceeds roughly 0.10 on the eager side
        is not comparable with the quiet runs and should be re-taken on an idle
        machine. Note that correctness is NOT sensitive to this: those three
        runs still passed the enforced gate at worst d 8.511e-03, 6.977e-03
        and 9.709e-03, so the worst correctness seen on this model across five
        runs is 9.709e-03, comfortably inside the 0.05 bound. Only the latency
        half of a contended run is unusable.

        And correctness is always measured against EAGER here. An earlier
        revision compared the fused cache against the GRAPH cache and
        reported 0.0, which was true and useless because both sides run the
        same fused kernels and their drift cancels.

        Two harness defects cost one otherwise-fine attempt at this and are
        worth avoiding, because both are silent. First, a driver that
        captures the benchmark's stdout and prints it only at the end loses
        the entire run when its own wrapper times out: the child keeps
        running to completion with its output going to a dead pipe, so the
        work is unrecoverable even though it finished. Stream and tee the
        output instead, and kill the child in a finally block so an orphan
        cannot sit on 23.7 GiB of GPU or leave a stale lock behind. Second,
        waiting for a quiet host by taking a single CPU-load sample does not
        work, because a foreign pytest suite cycles between roughly 15 and
        99 percent load and a lone sample catches either regime: it commits
        a 10-25 minute GPU run while the suite is merely between test cases,
        and the run then crawls. Require several CONSECUTIVE samples under
        the threshold before committing. Note that free physical memory is
        NOT a usable gate on this host: a 30-second trace taken while the
        foreign suite was running and no model was resident showed free
        physical memory oscillating between 1.34 and 4.03 GiB, so the
        0.98 GiB observed during the crawling run was roughly this host's
        normal contended range rather than evidence of page-file
        exhaustion. That earlier reading was initially misattributed to
        page-file exhaustion because it resembled the ctx 4096 signature
        below; the trace refutes it. The likelier cause of the crawl is
        simply that host contention slowed the host max-autotune compile,
        which is the same cost documented for ctx 4096. Load, not memory,
        is the variable worth sampling, and the spread is the real
        acceptance test.

        `torch.compile(mode="reduce-overhead")` does fail here, with
        `OverflowError: Python int too large to convert to C long` at
        `torch/_inductor/runtime/static_cuda_launcher.py:244`. That diagnosis
        was previously left as "unavailable here" and it is now known. The
        failing call forwards a CUDA stream HANDLE into a parameter pybind11
        declares as a C `long`, and a C long is 32 bits on Windows:

            grid_x 256  grid_y 1  grid_z 1  num_warps 4  shared 0
            arg_tys 'OOi'
            stream   550398715872   (0x802651e7e0)
            function 550384860192   (0x80257e7c20)

        Both the stream and the kernel function are 64-bit pointers and both
        exceed 2**31. The C long is 4 bytes on this platform, which the test
        prints, and only the stream is proved to be the refused argument; the
        function handle goes into a `uint64_t` in the same binding, so it
        being oversized is a hazard rather than the proven cause. The
        reason this looks like anything but a width problem
        is that the DEFAULT stream is 0, and 0 converts cleanly, so probing
        the default stream exonerates the stream entirely. A cudagraph capture
        runs on a side stream, so the failing call is never on stream 0. Forcing
        the stream argument to 0 gets past argument conversion and fails later
        inside the driver instead, which is what confirms the conversion was
        the blocker.

        Reduced-overhead is not a prerequisite for anything above, and the
        scope of the bug is narrow: it is Inductor's OWN cudagraph integration
        that is broken, not Inductor. On a three-layer MLP, 26 seconds to
        compile:

            mode=default                        OK  2.292e-07
            mode=reduce-overhead                FAIL OverflowError
            mode=max-autotune-no-cudagraphs     OK  3.437e-07
            mode=max-autotune                   FAIL OverflowError

        So `mode="max-autotune-no-cudagraphs"` gives full Inductor fusion with
        the static launcher out of the path, and manual `torch.cuda.graph`
        capture supplies the launch-overhead elimination that reduce-overhead
        would have supplied. That combination has now been measured on the 9B,
        and the answer is "it works, it is much faster than eager, and it is NOT
        bit-identical". See `tq_fusion_graph_abba_bench.py`; the results are
        recorded under FUSION PLUS GRAPH, above. It is also not a Triton
        problem: a hand-written Triton decode kernel in this same environment
        compiles and runs.
        Two measurement traps, because both produced confident wrong numbers
        before the numbers above were confirmed:

        - Model scope is not interchangeable here. `tq_block_cache_decode_check.py`
          drives Qwen2.5-3B-Instruct, NOT the 9B: prefill 1024 then 8 decode
          steps, comparing BlockQuantCache against an fp16 `DynamicCache`. It
          passes: the argmax token matches fp16 at every step and the worst
          relative L2 is 0.103, under that script's 0.5 gate. That is a 3B
          decode-path result and must not be quoted as 9B evidence. The 9B
          numbers in this file all come from the scripts named next to them.

        - An intermediate version of the fix reserved `new_total + added`. On
          the FIRST allocation start=0, so `added` was the entire prefill and
          the reservation landed at exactly 2x -- the very bug it was meant to
          remove. Chunked prefill masked it, because incremental growth left only
          1 block of slack; only a single-shot prefill showed 64 blocks reserved
          for 32 used. Measure `buffer.numel()` directly, and drive the cache
          the way the model is actually driven.

        - `tq_block_cache_selftest.py` reports the 0.842x "decoded prefix" from
          the TRUNCATED view `buffer[:, :, :blocks*block, :]`, not from the
          bytes allocated, so it can read below 1.0 while the buffer is
          genuinely over-reserved. `byte_breakdown()["decoded"]` uses
          `buf.numel()` and is the honest number.

        The fix is allocation-only: verified bit-identical to the doubling
        policy over 16 ragged updates mixing prefill and single-token steps, so
        it cannot change any output. On ragged input it also reserves half what
        doubling did (0.632x of bf16 versus 1.053x).

        The verdict is unchanged: still ~1.4x and still not faster, because
        packing does not displace the prefix it compresses. This removes waste,
        not the decoded prefix. Quality is fine, so this is a memory/latency
        result, not a correctness one: over 16 streaming chunks of 256 tokens
        the worst per-chunk relative loss difference against bf16 was 0.0164,
        under the 0.05 gate.

        Per-token decode was 0.97x-1.02x of bf16 across the same contexts, i.e.
        no speedup either. The codec work is negligible next to the extra
        dequantize-and-copy that materializing the prefix costs.
        """
        total = {"payload": 0, "metadata": 0, "residual": 0, "decoded": 0}
        for layer in self.quant_layers():
            for k, v in layer.byte_breakdown().items():
                if k in total:
                    total[k] = total[k] + v
        total["packed_only"] = total["payload"] + total["metadata"] + total["residual"]
        total["total"] = total["packed_only"] + total["decoded"]
        return total

    def effective_ratio(self, fp16_bytes):
        """FP16 bytes / this cache's true resident bytes. Below 1.0 means worse."""
        total = self.byte_breakdown()["total"]
        if total <= 0:
            return float("inf")
        return fp16_bytes / total

    def packed_ratio(self, fp16_bytes):
        """FP16 bytes / packed-only bytes: the codec's ratio, ignoring decode.

        This is the number the cache is designed around, and it is the number
        that made the design look worthwhile. It is NOT the cache's real
        footprint ratio; see `effective_ratio` for that.
        """
        packed = self.byte_breakdown()["packed_only"]
        if packed <= 0:
            return float("inf")
        return fp16_bytes / packed

    def packed_bytes(self):
        """True resident bytes for KV, INCLUDING the materialized FP16 prefix.

        Previously this returned payload + metadata + residual and was
        documented as "total resident bytes", which hid the decode buffer
        entirely. It did not overstate the saving by 1.65x -- it understated the
        cost. At 32K the old value was 432 MiB (0.375x fp16) while the true
        resident total is 1584 MiB (1.375x fp16), so the cache is 1.38x LARGER
        than the fp16 KV it replaces and every ratio derived from the old value
        was wrong by that factor. This now returns the same value as
        `byte_breakdown()["total"]`, so it cannot silently disagree with the
        breakdown. If you want the codec's ratio ignoring the decode buffer, ask
        for `packed_ratio` explicitly, and do not report it as a memory saving.
        """
        return self.byte_breakdown()["total"]


def fp16_kv_bytes(cache_like_dims):
    """Reference: bytes the same sequence would occupy in FP16 KV."""
    b, h, s, d = cache_like_dims
    return 2 * b * h * s * d * 2
