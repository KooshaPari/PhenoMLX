"""Offline self-test for tq_codec_eval.py. No model, no CUDA.

Covers two things that are easy to get wrong and expensive to debug on the GPU:

1. codec math -- Lloyd-Max codebook sanity, the post-rotation scaling
   convention (sigma = ||rot|| / sqrt(d)), and the outlier behaviour of each
   scheme.
2. layout -- every scheme must map both KV tensor layouts to themselves, the
   token and channel grouping axes must be different operations, the channel
   axis must equal the transpose-row construction, and *only* schemes must
   leave the other tensor untouched.

Run: python tq_codec_eval_selftest.py [path/to/tq_codec_eval.py]
"""

import importlib.util
import os
import sys

import torch

HERE = os.path.dirname(os.path.abspath(__file__))
TARGET = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "tq_codec_eval.py")

spec = importlib.util.spec_from_file_location("tqce", TARGET)
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)

failures = []


def check(name, cond, detail=""):
    print(
        f"  {'PASS' if cond else 'FAIL'}  {name}{(' -- ' + detail) if detail else ''}"
    )
    if not cond:
        failures.append(name)


print("codec math")
levels = m.lloyd_max_gaussian(4)
print("  lloyd-max 4-bit levels:", [round(float(v), 5) for v in levels])
check("16 levels", len(levels) == 16)
check("levels strictly increasing", bool((levels.diff() > 0).all()))
check(
    "levels symmetric",
    float((levels + levels.flip(0)).abs().max()) < 1e-3,  # grid discretization is ~1e-4
    f"max asymmetry={float((levels + levels.flip(0)).abs().max()):.2e}",
)

ramp = torch.linspace(-1, 1, 4096)
rel = float(
    ((m.uniform_rtn_qdq(ramp, bits=4, group_size=32) - ramp).norm() / ramp.norm())
)
check("uniform RTN is near-exact on a uniform ramp", rel < 0.002, f"rel_err={rel:.6f}")

torch.manual_seed(0)
x = torch.randn(128)
x[0] = 50.0
u = float(((m.uniform_rtn_qdq(x, bits=4, group_size=32) - x).norm() / x.norm()))
r = float(
    ((m.rht_qdq(x.reshape(1, 128), 128, levels).reshape(-1) - x).norm() / x.norm())
)
check(
    "rotation beats per-token grouping on an outlier vector",
    r < u,
    f"uniform={u:.5f} rht={r:.5f}",
)

errs_u, errs_r = [], []
for s in range(8):
    torch.manual_seed(100 + s)
    y = torch.randn(2048) * 0.5
    errs_u.append(
        float(((m.uniform_rtn_qdq(y, bits=4, group_size=32) - y).norm() / y.norm()))
    )
    errs_r.append(
        float(
            (
                (m.rht_qdq(y.reshape(-1, 128), 128, levels).reshape(-1) - y).norm()
                / y.norm()
            )
        )
    )
print(
    f"  random iid-Gaussian rel_err: uniform={sum(errs_u) / len(errs_u):.5f} "
    f"rht={sum(errs_r) / len(errs_r):.5f}"
)
check(
    "both schemes are within 2x of each other on iid Gaussian data",
    max(sum(errs_u), sum(errs_r)) / min(sum(errs_u), sum(errs_r)) < 2.0,
)
check(
    "uniform metadata costs 6 bits/coord at group_size=32",
    abs(m.uniform_rtn_bits_per_coord() - 6.0) < 1e-9,
)
check(
    "rht metadata costs 4.125 bits/coord at d_head=128",
    abs(m.rht_bits_per_coord(d_head=128) - 4.125) < 1e-9,
)

print("layout")
ctx = m.ShapeCtx(128, 2)
for shape in [(1, 64, 256), (1, 2, 64, 128)]:
    t = torch.randn(*shape)
    ok = all(
        m.apply_scheme(t, n, ctx, levels, is_k=k).shape == t.shape
        for n in m.scheme_names()
        for k in (True, False)
    )
    check(f"shape preserved for {shape}", ok)

t = torch.randn(1, 2, 64, 128)
chan = m.apply_uniform_axis(t, ctx, "channel")
tok = m.apply_uniform_axis(t, ctx, "token")
check("channel and token grouping differ", not torch.equal(chan, tok))
check(
    "grouping is deterministic",
    torch.equal(chan, m.apply_uniform_axis(t, ctx, "channel")),
)
ref = (
    m.uniform_rtn_qdq(t.permute(0, 1, 3, 2).reshape(-1, 64))
    .reshape(1, 2, 128, 64)
    .permute(0, 1, 3, 2)
)
check("channel axis equals transpose-row grouping", torch.equal(chan, ref))

k_only = [n for n in m.scheme_names() if "konly" in n]
v_only = [n for n in m.scheme_names() if "vonly" in n]
check(
    "konly schemes leave V untouched",
    all(torch.equal(m.apply_scheme(t, n, ctx, levels, is_k=False), t) for n in k_only),
)
check(
    "vonly schemes leave K untouched",
    all(torch.equal(m.apply_scheme(t, n, ctx, levels, is_k=True), t) for n in v_only),
)

# The measured per-channel win has to be reachable in a streaming cache. Grouping
# channel-wise must not mix values across group_size-token blocks, so a filled
# block never depends on later tokens and the unfilled tail can stay fp16. If this
# check ever fails, the win would require the whole sequence up front and could
# not be implemented incrementally.
G = m.GROUP_SIZE
big = torch.randn(4 * G, 32)  # [tokens, channels]
whole = m.apply_uniform_axis(big, ctx, "channel")
blocks = torch.cat(
    [m.apply_uniform_axis(big[i * G : (i + 1) * G], ctx, "channel") for i in range(4)],
    dim=0,
)
check(
    f"channel grouping is block-local (G={G}, streaming-safe)",
    torch.equal(whole, blocks),
)

print()
if failures:
    print(f"SELFTEST FAILED ({len(failures)}): {failures}")
    sys.exit(1)
print("SELFTEST OK")
