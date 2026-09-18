"""Check that per-channel K is a call-site layout change, not a codec change.

`docs/TURBOQUANT-EXTRAPOLATION.md` claims the existing encoder already groups
along a flat slice, so per-channel K only requires each channel's values to be
contiguous (transpose K to [channels, tokens], encode, decode, transpose back).
This script tests that claim against the *shipped* CUDA port on real K/V
activations, and cross-checks the harness's own code path on the same tensors.

Recorded reference for Qwen2.5-3B-Instruct (`pilot/results/codec_eval_3b_postrope_b4.json`):

    uniform_rtn_g32_b4          relative_error 0.1068869427918155
    uniform_rtn_chanK_g32_b4    relative_error 0.03367141639967468

Both aggregate over all 36 layers x {k, v}; the channel axis is applied to K
only, exactly as the harness does.

WATCH OUT -- tensor labels differ between the harness and ad-hoc scripts:
`capture_kv` labels tensors "k"/"v", while a hand-rolled hook usually labels them
"k_proj"/"v_proj". Code that writes `if kind == "v"` silently takes the wrong
branch against "v_proj" and quantizes V with the channel axis, giving a
plausible-looking but wrong number (0.0326 instead of 0.0337). Worse,
`distortion_report` decides `is_k=(kind == "k")`, so feeding it "k_proj" labels
makes the channel axis never apply and silently returns the token figure for
both. This script therefore uses the harness's own `capture_kv` for the
cross-check instead of a local hook.

Run: python tq_codec_eval_shipped_check.py [--corpus PATH]
"""

import argparse
import importlib.util
import math
import os
import sys

os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
# Must be set BEFORE importing transformers: it snapshots HF_HOME at import time.
os.environ.setdefault(
    "HF_HOME", os.environ.get("TQ_HF_HOME", r"C:\Users\koosh\.cache\huggingface")
)

import torch  # noqa: E402
from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
MODEL_ID = os.environ.get("TQ_MODEL_ID", "Qwen/Qwen2.5-3B-Instruct")
BITS, GROUP = 4, 32
TOKENS = 512
REF_TOKEN = 0.1068869427918155
REF_CHANK = 0.03367141639967468


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


harness = load("tqce", os.path.join(HERE, "tq_codec_eval.py"))
shipped = load("tqc", os.path.join(HERE, "turbo_quant_cuda.py"))


def shipped_qdq(flat):
    """Round-trip a flat CUDA float32 tensor through the shipped port."""
    assert flat.numel() % GROUP == 0, f"{flat.numel()} not a multiple of {GROUP}"
    packed, scales, zeros = shipped.encode_uniform_cuda(
        flat, bits=BITS, group_size=GROUP
    )
    return shipped.decode_uniform_cuda(packed, scales, zeros, flat.numel(), BITS, GROUP)


def channel_major(t):
    """[..., tokens, channels] -> flat with each channel's values contiguous."""
    if t.dim() == 2:
        w, s = t.shape[1], t.shape[0]
        return t.permute(1, 0).reshape(-1), (w, s)
    b, s, w = t.shape
    return t.permute(0, 2, 1).reshape(-1), (b, s, w)


def restore_channel(flat, t):
    if t.dim() == 2:
        return flat.reshape(t.shape[1], t.shape[0]).permute(1, 0)
    b, s, w = t.shape
    return flat.reshape(b, w, s).permute(0, 2, 1)


def capture(model, ids):
    """Local capture, labelled "k"/"v" to match the harness's `capture_kv`."""
    store, handles = [], []
    for layer in model.model.layers:
        for name in ("k", "v"):
            proj = getattr(layer.self_attn, name + "_proj")

            def hook(mod, inp, out, _n=name):
                x = out[0] if isinstance(out, tuple) else out
                store.append((_n, x.detach().float()))
                return out

            handles.append(proj.register_forward_hook(hook))
    with torch.no_grad():
        model(ids)
    for h in handles:
        h.remove()
    return store


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default=r"C:\Users\koosh\PHENOTYPE_MASTER_ROADMAP.md")
    ap.add_argument("--capture-tokens", type=int, default=TOKENS)
    args = ap.parse_args()

    print(f"torch {torch.__version__} | {torch.cuda.get_device_name(0)}", flush=True)
    tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
    )
    model.eval()
    ctx = harness.ShapeCtx(
        model.config.hidden_size // model.config.num_attention_heads,
        getattr(model.config, "num_key_value_heads", model.config.num_attention_heads),
    )
    text = open(args.corpus, encoding="utf-8", errors="replace").read()
    ids = (
        tok(text, return_tensors="pt")
        .input_ids[:, : args.capture_tokens]
        .to(model.device)
    )
    store = capture(model, ids)
    print(
        f"captured {len(store)} projection outputs from {ids.shape[1]} tokens",
        flush=True,
    )

    results = {}
    for scheme in ("token", "chanK"):
        sq_err = sq_ref = 0.0
        for kind, t in store:
            if scheme == "token" or kind.startswith("v"):  # see label warning above
                rec = shipped_qdq(t.reshape(-1)).reshape(t.shape)
            else:
                flat, _ = channel_major(t)
                rec = restore_channel(shipped_qdq(flat), t)
            err = (rec - t).double()
            sq_err += float((err**2).sum())
            sq_ref += float(t.double().pow(2).sum())
        results[scheme] = math.sqrt(sq_err / sq_ref)
        print(
            f"  shipped codec {scheme:5s}: rel_err = {results[scheme]:.16f}", flush=True
        )

    # Cross-check the harness's own capture and code path on the same activations.
    # Using its capture keeps the label convention identical (`is_k=(kind == "k")`).
    theirs = harness.capture_kv(model, ids, ctx)
    assert len(theirs) == len(store), (len(theirs), len(store))
    maxdiff = max(float((a - b).abs().max()) for (_, a), (_, b) in zip(store, theirs))
    print(
        f"  captured data identical to harness: {maxdiff == 0.0} (max diff {maxdiff:.1e})",
        flush=True,
    )

    levels = harness.lloyd_max_gaussian(BITS)
    dist = harness.distortion_report(theirs, ctx, levels)
    key_token = f"uniform_rtn_g{GROUP}_b{BITS}"
    key_chank = f"uniform_rtn_chanK_g{GROUP}_b{BITS}"
    print(
        f"  harness code path token: rel_err = {dist[key_token]['relative_error']:.16f}",
        flush=True,
    )
    print(
        f"  harness code path chanK: rel_err = {dist[key_chank]['relative_error']:.16f}",
        flush=True,
    )

    print(f"\nrecorded reference: token {REF_TOKEN}  chanK {REF_CHANK}")
    checks = [
        (
            "reproduces the recorded token figure",
            abs(results["token"] - REF_TOKEN) < 1e-6,
        ),
        (
            "reproduces the recorded chanK figure",
            abs(results["chanK"] - REF_CHANK) < 1e-6,
        ),
        ("per-channel K beats per-token K", results["chanK"] < results["token"]),
        (
            "shipped codec matches the harness path",
            abs(results["chanK"] - dist[key_chank]["relative_error"]) < 1e-6,
        ),
    ]
    ok = True
    for name, passed in checks:
        print(f"  {'PASS' if passed else 'FAIL'}  {name}")
        ok &= passed
    print(
        "\nVERIFIED: per-channel K is a call-site layout change, not a codec change"
        if ok
        else "\nVERIFY FAILED"
    )
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
