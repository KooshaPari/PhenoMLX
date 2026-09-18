"""Compare the PhenoMLX turbo_quant codec against TurboQuant-style codecs.

Motivation
----------
`perf-core/turbo-quant/src/encode.rs` implements plain uniform asymmetric
round-to-nearest group quantization (scale=(max-min)/qmax, zero=min, no
rotation, no codebook). Published TurboQuant (arXiv:2504.19874) instead randomly
rotates each vector so coordinates become concentrated and near-independent,
then applies an optimal *scalar* quantizer per coordinate with one stored norm
per vector.

Scheme matrix (all at matched nominal bit-width, same measurement path):

  uniform_rtn_g32_bN        the repo codec. Groups run along the channel axis
                            inside one token, for both K and V.
  uniform_rtn_chanK_g32_bN  same codec, but K groups run along the *token* axis
                            (one scale per channel instead of per token), which
                            is the axis KIVI uses for K.
  rht_rtn_bN                rotate per head-vector, then the repo's own RTN.
                            Isolates the rotation factor alone.
  rht_uniform_bN            rotate, then a FIXED uniform codebook in units of
                            sigma (unbiased, data-oblivious, 4 bits + fp16
                            sigma). Isolates codebook shape from rotation.
  rht_lloyd_bN              rotate, then the MSE-optimal Gaussian codebook
                            (4 bits + fp16 sigma). This is TurboQuant's core
                            WITHOUT the 1-bit QJL residual stage.

--k-mode pre-rope quantizes the k_proj output (what tq_ab_bench_v2.py does
today); post-rope quantizes K after rotary embedding, i.e. the tensor that
actually enters the cache.

Caveats recorded in the JSON, not hidden:
  - QDQ measures quality only. The resident cache stays FP16; no throughput
    claim is made anywhere here.
  - rht_lloyd omits QJL. MSE-optimal scalar quantizers bias inner products, and
    QJL exists in the paper specifically to remove that bias.
  - PPL windows are chunked; no window sees beyond `chunk` tokens.
  - Always run a bit-width control (TQ_BITS=8) alongside 4-bit numbers. If the
    control does not return to the fp16 baseline, the harness is broken, not
    the codec.
"""

import argparse
import json
import math
import os
import sys
import time
from datetime import datetime, timezone

os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
# Must be set BEFORE importing transformers: it snapshots HF_HOME at import.
os.environ.setdefault(
    "HF_HOME", os.environ.get("TQ_HF_HOME", r"C:\Users\koosh\.cache\huggingface")
)

import torch  # noqa: E402
from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402

MODEL_ID = os.environ.get("TQ_MODEL_ID", "Qwen/Qwen2.5-3B-Instruct")
BITS = int(os.environ.get("TQ_BITS", "4"))
GROUP_SIZE = int(os.environ.get("TQ_GROUP_SIZE", "32"))


# --------------------------------------------------------------------------
# the repo codec: uniform asymmetric RTN per group
# --------------------------------------------------------------------------
def uniform_rtn_qdq(x, bits=BITS, group_size=GROUP_SIZE):
    """x: [..., width] -> fake-quantized float32, same shape.

    Mirrors perf-core/turbo-quant/src/encode.rs and turbo_quant_cuda.py.
    """
    orig_shape = x.shape
    flat = x.reshape(-1).float()
    n = flat.numel()
    pad = (group_size - n % group_size) % group_size
    if pad:
        flat = torch.cat([flat, flat[-pad:]])
    g = flat.reshape(-1, group_size)
    qmax = float((1 << bits) - 1)
    lo = g.min(dim=1, keepdim=True).values
    hi = g.max(dim=1, keepdim=True).values
    scale = ((hi - lo) / qmax).clamp_min(1e-12)
    q = torch.round((g - lo) / scale).clamp_(0.0, qmax)
    rec = q * scale + lo
    return rec.reshape(-1)[:n].reshape(orig_shape)


def uniform_rtn_bits_per_coord(bits=BITS, group_size=GROUP_SIZE, meta_bits=32):
    """Nominal bits PLUS the scale/zero metadata actually carried per coordinate.

    The Rust/CUDA codec stores scale and zero as f32 per group, i.e. 2*32 bits
    per `group_size` coordinates. At group_size=32 that is +2 bits/coordinate,
    so a "4-bit" tensor costs 6 bits/coordinate on the wire.
    """
    return bits + 2.0 * meta_bits / group_size


# --------------------------------------------------------------------------
# TurboQuant-style codecs: randomized Hadamard rotation, then a scalar codebook
# --------------------------------------------------------------------------
def hadamard_matrix(n):
    """Normalized Sylvester-Hadamard matrix of size n (n a power of two)."""
    assert n & (n - 1) == 0, "n must be a power of two"
    h = torch.ones(1, 1)
    while h.shape[0] < n:
        h = torch.cat([torch.cat([h, h], dim=1), torch.cat([h, -h], dim=1)], dim=0)
    return h / math.sqrt(n)


def _rot_cache(d_head, device):
    """Cached (H, signs): the randomized Hadamard transform for d_head."""
    if not hasattr(_rot_cache, "_cache"):
        _rot_cache._cache = {}
    key = (d_head, str(device))
    if key not in _rot_cache._cache:
        h = hadamard_matrix(d_head).to(device)
        gen = torch.Generator(device="cpu").manual_seed(1234)
        signs = (
            (torch.randint(0, 2, (d_head,), generator=gen) * 2 - 1).float().to(device)
        )
        _rot_cache._cache[key] = (h, signs)
    return _rot_cache._cache[key]


def rotate(x, h, signs):
    return (x * signs) @ h


def unrotate(x, h, signs):
    return (x @ h.t()) * signs


def lloyd_max_gaussian(bits, iters=200, n_grid=40001, span=8.0):
    """Lloyd-Max codebook for a standard normal marginal (data-oblivious).

    After RHT each coordinate of a rotated vector is ~N(0, sigma^2), so a
    standard-normal quantizer plus one stored sigma is the TurboQuant shape.
    """
    n_levels = 1 << bits
    probs = (torch.arange(n_levels, dtype=torch.float64) + 0.5) / n_levels
    levels = torch.erfinv(2 * probs - 1) * math.sqrt(2.0)
    grid = torch.linspace(-span, span, n_grid, dtype=torch.float64)
    gw = torch.exp(-(grid**2) / 2.0) / math.sqrt(2 * math.pi)
    for _ in range(iters):
        edges = (levels[:-1] + levels[1:]) / 2.0
        idx = torch.bucketize(grid, edges)
        acc_w = torch.zeros(n_levels, dtype=torch.float64)
        acc_wx = torch.zeros(n_levels, dtype=torch.float64)
        acc_w.index_add_(0, idx, gw)
        acc_wx.index_add_(0, idx, gw * grid)
        new_levels = acc_wx / acc_w.clamp_min(1e-300)
        delta = float((new_levels - levels).abs().max())
        levels = new_levels
        if delta < 1e-11:
            break
    return levels.float()


def _edges(levels, device):
    if not hasattr(_edges, "_cache"):
        _edges._cache = {}
    key = (int(levels.shape[0]), str(device))
    if key not in _edges._cache:
        _edges._cache[key] = ((levels[:-1] + levels[1:]) / 2.0).to(device)
    return _edges._cache[key]


def rht_qdq(x, d_head, levels):
    """Rotate per head-vector, quantize with the Gaussian Lloyd-Max codebook."""
    flat = x.reshape(-1, d_head).float()
    h, signs = _rot_cache(d_head, flat.device)
    rot = rotate(flat, h, signs)
    sigma = (rot.norm(dim=1, keepdim=True) / math.sqrt(d_head)).clamp_min(1e-12)
    z = rot / sigma
    idx = torch.bucketize(z.reshape(-1), _edges(levels, flat.device)).reshape(z.shape)
    rec = levels.to(flat.device)[idx] * sigma
    return unrotate(rec, h, signs).reshape(x.shape)


def rht_uniform_fixed_qdq(x, d_head, span=3.0, n_levels=1 << BITS):
    """Rotate, then a FIXED uniform codebook over [-span, span] * sigma.

    Same bit budget as rht_qdq (bits + one fp16 sigma per vector) but the
    codebook is symmetric/unbiased, isolating codebook shape from rotation.
    """
    flat = x.reshape(-1, d_head).float()
    h, signs = _rot_cache(d_head, flat.device)
    rot = rotate(flat, h, signs)
    sigma = (rot.norm(dim=1, keepdim=True) / math.sqrt(d_head)).clamp_min(1e-12)
    z = (rot / sigma).clamp(-span, span)
    step = 2 * span / (n_levels - 1)
    q = torch.round((z + span) / step).clamp(0, n_levels - 1)
    rec = (q * step - span) * sigma
    return unrotate(rec, h, signs).reshape(x.shape)


def rht_rtn_qdq(x, d_head):
    """Rotate, then the repo's own uniform RTN codec, then rotate back.

    Isolates the rotation factor: same quantizer as the repo codec, applied to
    rotated coordinates.
    """
    flat = x.reshape(-1, d_head).float()
    h, signs = _rot_cache(d_head, flat.device)
    rot = rotate(flat, h, signs)
    rec = uniform_rtn_qdq(rot)
    return unrotate(rec, h, signs).reshape(x.shape)


def rht_bits_per_coord(bits=BITS, d_head=128, meta_bits=16):
    return bits + 1.0 * meta_bits / d_head


def rht_rtn_bits_per_coord(bits=BITS, group_size=GROUP_SIZE, meta_bits=32):
    return bits + 2.0 * meta_bits / group_size


# --------------------------------------------------------------------------
# layout handling: pre-RoPE projections are [b, s, kv_width]; post-RoPE cache
# tensors are [b, kv_heads, s, d_head]
# --------------------------------------------------------------------------
class ShapeCtx:
    def __init__(self, d_head, n_kv_heads):
        self.d_head = d_head
        self.n_kv_heads = n_kv_heads
        self.width = d_head * n_kv_heads

    def to_rows(self, t):
        """[b, heads, s, d] or [b, s, w] -> (rows [n, width], restore_meta)."""
        if t.dim() == 4:
            b, h, s, d = t.shape
            return t.permute(0, 2, 1, 3).reshape(-1, h * d), (b, h, s, d)
        return t.reshape(-1, t.shape[-1]), tuple(t.shape)

    def from_rows(self, rows, meta):
        if meta is None:
            return rows
        if len(meta) == 4:
            b, h, s, d = meta
            return rows.reshape(b, s, h, d).permute(0, 2, 1, 3)
        return rows.reshape(meta)


def apply_uniform_axis(t, ctx, axis):
    """Uniform RTN grouping along the token axis or the channel axis.

    axis='token'   groups across channels within one token (repo behaviour)
    axis='channel' groups across tokens for one channel (KIVI's choice for K:
                   K is outlier-heavy per channel, so per-token grouping lets a
                   single outlier channel dictate the scale of its whole group)
    """
    if axis == "token":
        # Grouping runs along the last dimension, so flattening is equivalent to
        # per-row grouping whenever the last dim is a multiple of group_size.
        return uniform_rtn_qdq(t)
    if t.dim() == 4:
        b, h, s, d = t.shape
        rows = t.permute(0, 1, 3, 2).reshape(-1, s)
        return uniform_rtn_qdq(rows).reshape(b, h, d, s).permute(0, 1, 3, 2)
    if t.dim() == 3:
        b, s, w = t.shape
        rows = t.permute(0, 2, 1).reshape(-1, s)
        return uniform_rtn_qdq(rows).reshape(b, w, s).permute(0, 2, 1)
    # captured rows [n_tokens, width]: group across tokens, per channel
    return (
        uniform_rtn_qdq(t.permute(1, 0).reshape(-1, t.shape[0]))
        .reshape(t.shape[1], t.shape[0])
        .permute(1, 0)
    )


def apply_scheme(t, kind, ctx, levels, is_k=False):
    """QDQ `t` in place of the real tensor, using scheme `kind`."""
    if kind == "fp16_kv":
        return t
    if not scheme_applies_to(kind, "k" if is_k else "v"):
        return t
    orig_dtype = t.dtype
    if kind.startswith("uniform"):
        axis = "channel" if (is_k and "chanK" in kind) else "token"
        rec = apply_uniform_axis(t, ctx, axis)
    else:
        rows, meta = ctx.to_rows(t)
        if kind.startswith("rht_lloyd"):
            rec = ctx.from_rows(rht_qdq(rows, ctx.d_head, levels), meta)
        elif kind.startswith("rht_uniform"):
            rec = ctx.from_rows(rht_uniform_fixed_qdq(rows, ctx.d_head), meta)
        elif kind.startswith("rht_rtn"):
            rec = ctx.from_rows(rht_rtn_qdq(rows, ctx.d_head), meta)
        else:
            raise ValueError(kind)
    return rec.to(orig_dtype)


def scheme_names():
    return [
        "fp16_kv",
        f"uniform_rtn_g{GROUP_SIZE}_b{BITS}",
        f"uniform_rtn_chanK_g{GROUP_SIZE}_b{BITS}",
        f"uniform_rtn_konly_g{GROUP_SIZE}_b{BITS}",
        f"uniform_rtn_chanK_konly_g{GROUP_SIZE}_b{BITS}",
        f"uniform_rtn_vonly_g{GROUP_SIZE}_b{BITS}",
        f"rht_rtn_b{BITS}",
        f"rht_uniform_b{BITS}",
        f"rht_lloyd_b{BITS}",
    ]


# --------------------------------------------------------------------------
# measurement
# --------------------------------------------------------------------------
def capture_kv(model, ids, ctx):
    """One forward pass; per-layer K/V projection outputs in their native layout."""
    store = []
    handles = []

    def make(kind):
        def hook(mod, inp, out):
            t = out[0] if isinstance(out, tuple) else out
            store.append((kind, t.detach().float()))

        return hook

    for layer in model.model.layers:
        handles.append(layer.self_attn.k_proj.register_forward_hook(make("k")))
        handles.append(layer.self_attn.v_proj.register_forward_hook(make("v")))
    with torch.no_grad():
        model(ids)
    for h in handles:
        h.remove()
    return store


def distortion_report(tensors, ctx, levels):
    out = {}
    for name in scheme_names():
        if name == "fp16_kv":
            continue
        sq_err = sq_ref = 0.0
        worst_vec = 0.0
        n = 0
        for kind, t in tensors:
            if not scheme_applies_to(name, kind):
                continue
            rec = apply_scheme(t, name, ctx, levels, is_k=(kind == "k"))
            err = rec - t
            sq_err += float((err**2).sum())
            sq_ref += float((t**2).sum())
            if name.startswith("rht"):
                vref = t.reshape(-1, ctx.d_head).norm(dim=1)
                verr = err.reshape(-1, ctx.d_head).norm(dim=1)
            else:
                vref = t.reshape(-1, t.shape[-1]).norm(dim=1)
                verr = err.reshape(-1, t.shape[-1]).norm(dim=1)
            worst_vec = max(worst_vec, float((verr / vref.clamp_min(1e-6)).max()))
            n += int(t.numel())
        out[name] = {
            "mse": sq_err / n,
            "relative_error": math.sqrt(sq_err / max(sq_ref, 1e-30)),
            "worst_row_relative_error": worst_vec,
        }
    return out


def scheme_applies_to(name, kind):
    """Which of K/V a scheme touches, for the distortion report."""
    if kind == "k" and "vonly" in name:
        return False
    if kind == "v" and "konly" in name:
        return False
    return True


def bits_per_coord_report(ctx):
    rht = rht_bits_per_coord(d_head=ctx.d_head)
    rht_rtn = rht_rtn_bits_per_coord()
    uniform = uniform_rtn_bits_per_coord()
    out = {"fp16_kv": 16.0}
    for name in scheme_names():
        if name == "fp16_kv":
            continue
        if name.startswith("rht_lloyd") or name.startswith("rht_uniform"):
            out[name] = rht
        elif name.startswith("rht_rtn"):
            out[name] = rht_rtn
        else:
            out[name] = uniform
    return out


class _RopeRestore:
    """Restores the module-level rotary function when hooks are removed."""

    def __init__(self, mod):
        self.mod = mod

    def remove(self):
        self.mod.apply_rotary_pos_emb = self.mod._tq_orig_rope


def install_hooks(model, kind, ctx, levels, k_mode):
    """QDQ hooks.

    V has no rotary step, so it is always quantized at v_proj.
    k_mode == "pre-rope": K quantized at the k_proj output.
    k_mode == "post-rope": K quantized after rotary embedding, i.e. the tensor
    that is actually written into the KV cache.
    """
    if kind == "fp16_kv":
        return []
    handles = []

    def v_hook(mod, inp, out):
        t = out[0] if isinstance(out, tuple) else out
        rec = apply_scheme(t, kind, ctx, levels, is_k=False)
        return (rec,) + tuple(out[1:]) if isinstance(out, tuple) else rec

    def k_hook(mod, inp, out):
        t = out[0] if isinstance(out, tuple) else out
        rec = apply_scheme(t, kind, ctx, levels, is_k=True)
        return (rec,) + tuple(out[1:]) if isinstance(out, tuple) else rec

    for layer in model.model.layers:
        handles.append(layer.self_attn.v_proj.register_forward_hook(v_hook))
        if k_mode == "pre-rope":
            handles.append(layer.self_attn.k_proj.register_forward_hook(k_hook))

    if k_mode == "post-rope":
        import transformers.models.qwen2.modeling_qwen2 as q2

        if not hasattr(q2, "_tq_orig_rope"):
            q2._tq_orig_rope = q2.apply_rotary_pos_emb
        state = {"kind": kind, "ctx": ctx, "levels": levels}

        def patched(q, k, cos, sin, unsqueeze_dim=1):
            q, k = q2._tq_orig_rope(q, k, cos, sin, unsqueeze_dim)
            return q, apply_scheme(
                k, state["kind"], state["ctx"], state["levels"], is_k=True
            )

        q2.apply_rotary_pos_emb = patched
        handles.append(_RopeRestore(q2))
    return handles


def perplexity(model, tok, text, max_tokens, chunk, kind, ctx, levels, k_mode):
    handles = install_hooks(model, kind, ctx, levels, k_mode)
    ids = tok(text, return_tensors="pt").input_ids[:, :max_tokens].to(model.device)
    losses = []
    total_loss = 0.0
    total_tok = 0
    try:
        with torch.no_grad():
            for i in range(0, ids.shape[1] - 1, chunk):
                part = ids[:, i : i + chunk + 1]
                if part.shape[1] < 2:
                    break
                loss = model(part, labels=part).loss
                k = part.shape[1] - 1
                losses.append(round(float(loss), 5))
                total_loss += float(loss) * k
                total_tok += k
    finally:
        for h in handles:
            h.remove()
    return math.exp(total_loss / max(total_tok, 1)), total_tok, losses


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", required=True, help="local text file used for PPL")
    ap.add_argument("--max-tokens", type=int, default=1024)
    ap.add_argument("--chunk", type=int, default=512)
    ap.add_argument("--capture-tokens", type=int, default=512)
    ap.add_argument("--k-mode", choices=["pre-rope", "post-rope"], default="post-rope")
    ap.add_argument("--out", default="tq_codec_eval.json")
    args = ap.parse_args()

    t0 = time.perf_counter()
    print(f"torch {torch.__version__} | {torch.cuda.get_device_name(0)}", flush=True)
    tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
    )
    model.eval()
    cfg = model.config
    ctx = ShapeCtx(
        cfg.hidden_size // cfg.num_attention_heads,
        getattr(cfg, "num_key_value_heads", cfg.num_attention_heads),
    )
    print(
        f"loaded {MODEL_ID}: {cfg.num_hidden_layers} layers, head_dim={ctx.d_head}, "
        f"kv_heads={ctx.n_kv_heads}",
        flush=True,
    )

    levels = lloyd_max_gaussian(BITS)
    print(
        f"lloyd-max {BITS}-bit gaussian levels: {[round(float(v), 4) for v in levels]}",
        flush=True,
    )

    text = open(args.corpus, encoding="utf-8", errors="replace").read()
    ids = (
        tok(text, return_tensors="pt")
        .input_ids[:, : args.capture_tokens]
        .to(model.device)
    )
    tensors = capture_kv(model, ids, ctx)
    print(
        f"captured {len(tensors)} projection outputs from {ids.shape[1]} tokens",
        flush=True,
    )

    dist = distortion_report(tensors, ctx, levels)
    bpc = bits_per_coord_report(ctx)

    ppl = {}
    for name in scheme_names():
        p, ntok, losses = perplexity(
            model,
            tok,
            text,
            args.max_tokens,
            args.chunk,
            name,
            ctx,
            levels,
            args.k_mode,
        )
        ppl[name] = {"perplexity": p, "tokens": ntok, "window_losses": losses}
        print(
            f"  PPL[{name}] = {p:.4f} over {ntok} tokens  windows={losses}", flush=True
        )

    base = ppl["fp16_kv"]["perplexity"]
    for name in ppl:
        ppl[name]["ppl_delta_pct"] = round(
            (ppl[name]["perplexity"] - base) / base * 100, 3
        )

    report = {
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "model": MODEL_ID,
        "gpu": torch.cuda.get_device_name(0),
        "corpus": args.corpus,
        "config": {
            "bits": BITS,
            "group_size": GROUP_SIZE,
            "k_mode": args.k_mode,
            "eval_tokens": args.max_tokens,
            "chunk": args.chunk,
            "capture_tokens": args.capture_tokens,
            "d_head": ctx.d_head,
            "kv_heads": ctx.n_kv_heads,
        },
        "bits_per_coordinate_including_metadata": bpc,
        "distortion_on_real_kv": dist,
        "perplexity_qdq_hooked": ppl,
        "limitations": [
            "QDQ measures quality only; the resident cache stays FP16 and no decode-speed claim is made.",
            "rht_lloyd is TurboQuant's core (RHT + per-coordinate optimal scalar quantizer + fp16 norm) WITHOUT the 1-bit QJL residual stage, which exists to remove the inner-product bias of an MSE-optimal quantizer.",
            "PPL uses chunked windows; no window sees beyond `chunk` tokens of context.",
            "A high-bit control run (TQ_BITS=8) is required to validate the harness before trusting any 4-bit delta.",
        ],
        "wall_clock_s": round(time.perf_counter() - t0, 1),
    }
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)
    print(f"WROTE {args.out}", flush=True)

    print(f"\n=== codec comparison, k_mode={args.k_mode}, nominal {BITS}-bit ===")
    for name in scheme_names():
        if name == "fp16_kv":
            continue
        d = dist[name]
        print(
            f"{name:34s} rel_err={d['relative_error']:.5f} "
            f"bits/coord={bpc[name]:.3f} ppl_delta={ppl[name]['ppl_delta_pct']:+.2f}%"
        )


if __name__ == "__main__":
    sys.exit(main())
