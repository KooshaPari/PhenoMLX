"""Resident packed KV cache measurement, v2 -- drive the cache as a stream.

Why v1 produced a null result
-----------------------------
The first version fed each window as a single prefill with a fresh
QuantizedCache and got byte-identical PPL for fp16, hqq 4-bit and hqq 3-bit. That
is not a bug in the cache: `QuantizedLayer.update` has a lazy-init path that
quantizes into storage but RETURNS THE RAW key/value states, so on a single
prefill the attention never sees quantized data. Any packed-cache quality
measurement done as prefill-only is therefore blind to the compression. This is
the mirror image of the hook harness, which always injects noise.

What v2 does instead
--------------------
One persistent cache per window, fed in steps of `--step` tokens. From the second
step on, `update` takes the dequantize path, so attention reads the quantized
prefix and the injected error is real. Loss is taken over the new tokens of each
step only (the streaming case), via `cache_position`.

Configs: fp16 (DynamicCache) vs QuantizedCache(hqq, nbits) at each axis, so the
axis is identified by what it does to perplexity on the real model rather than by
synthetic probes, four of which failed to pin HQQ's convention.

Caveats kept in the JSON: hqq is not the repo codec (use the axis contrast, not
absolute numbers); peak VRAM is dominated by weights at this context; the cache
re-quantizes the whole prefix whenever the residual window fills, so cost grows
with sequence length.
"""
import argparse
import json
import math
import os
import time
from datetime import datetime, timezone

os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
os.environ.setdefault(
    "HF_HOME", os.environ.get("TQ_HF_HOME") or r"C:\Users\koosh\.cache\huggingface"
)

import torch  # noqa: E402
import transformers.cache_utils as cu  # noqa: E402
from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402

MODEL_ID = os.environ.get("TQ_MODEL_ID", "Qwen/Qwen2.5-3B-Instruct")


def ppl_streaming(model, tok, text, max_tokens, step, make_cache, label):
    """Persistent cache, fed in steps; loss over the new tokens of each step."""
    ids = tok(text, return_tensors="pt").input_ids[:, :max_tokens].to(model.device)
    cache = make_cache()
    torch.cuda.synchronize()
    torch.cuda.reset_peak_memory_stats()
    total_loss, total_tok, losses = 0.0, 0, []
    with torch.no_grad():
        for start in range(0, ids.shape[1] - 1, step):
            end = min(start + step, ids.shape[1] - 1)
            part = ids[:, start:end]
            if part.shape[1] == 0:
                continue
            pos = torch.arange(start, end, device=model.device)
            out = model(part, labels=part, past_key_values=cache, use_cache=True,
                        cache_position=pos)
            k = part.shape[1]
            losses.append(round(float(out.loss), 5))
            total_loss += float(out.loss) * k
            total_tok += k
            del out
    torch.cuda.synchronize()
    ppl = math.exp(total_loss / max(total_tok, 1))
    alloc = torch.cuda.max_memory_allocated() / 1024**3
    reserved = torch.cuda.max_memory_reserved() / 1024**3
    print(f"  {label:16s} PPL={ppl:9.4f}  peak {alloc:6.2f}/{reserved:6.2f} GiB "
          f"steps={len(losses)}", flush=True)
    return {"perplexity": round(ppl, 5), "tokens": total_tok,
            "step_losses": losses, "peak_alloc_gib": round(alloc, 3),
            "peak_reserved_gib": round(reserved, 3)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default=r"C:\Users\koosh\agents\sandbox\tq-eval\corpus_repodocs.txt")
    ap.add_argument("--max-tokens", type=int, default=2048)
    ap.add_argument("--step", type=int, default=128)
    ap.add_argument("--group", type=int, default=32)
    ap.add_argument("--residual", type=int, default=128)
    ap.add_argument("--out", default="packed_cache_stream.json")
    args = ap.parse_args()

    t0 = time.perf_counter()
    print(f"torch {torch.__version__} | {torch.cuda.get_device_name(0)}", flush=True)
    tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
    )
    model.eval()
    text = open(args.corpus, encoding="utf-8", errors="replace").read()

    def qcache(nbits, axis):
        return cu.QuantizedCache(
            backend="hqq", config=model.config, nbits=nbits, axis_key=axis,
            axis_value=axis, q_group_size=args.group, residual_length=args.residual,
        )

    results = {}
    results["fp16"] = ppl_streaming(
        model, tok, text, args.max_tokens, args.step, lambda: cu.DynamicCache(), "fp16"
    )
    base = results["fp16"]["perplexity"]

    for nbits in (4, 3):
        for axis in (0, 1):
            label = f"hqq{nbits}_axis{axis}"
            try:
                results[label] = ppl_streaming(
                    model, tok, text, args.max_tokens, args.step,
                    (lambda nb=nbits, ax=axis: qcache(nb, ax)), label,
                )
            except Exception as exc:  # noqa: BLE001 -- a bad cell must not kill the run
                msg = f"{type(exc).__name__}: {exc}"
                print(f"  {label:16s} FAILED  {msg}", flush=True)
                results[label] = {"error": msg}
                torch.cuda.empty_cache()
                continue
            results[label]["ppl_delta_pct"] = round(
                (results[label]["perplexity"] - base) / base * 100, 3
            )
            torch.cuda.empty_cache()

    report = {
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "model": MODEL_ID,
        "gpu": torch.cuda.get_device_name(0),
        "backend": "transformers QuantizedCache + hqq, driven as a stream",
        "config": {"max_tokens": args.max_tokens, "step": args.step,
                   "q_group_size": args.group, "residual_length": args.residual},
        "results": results,
        "why_streaming": [
            "QuantizedLayer.update returns the RAW key/value on its first call, so a prefill-only evaluation cannot observe a QuantizedCache's effect at all.",
            "From the second update on, attention reads the dequantized prefix and the injected error is real.",
        ],
        "caveats": [
            "hqq is a third-party quantizer, not the repo codec: use the axis CONTRAST, not these absolute numbers.",
            "Peak VRAM here is dominated by the 3B weights; a KV residency difference is only visible at long context.",
            "The cache re-quantizes the whole quantized prefix whenever the residual window fills, so cost grows with sequence length.",
        ],
        "wall_clock_s": round(time.perf_counter() - t0, 1),
    }
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)
    print(f"WROTE {args.out}", flush=True)

    print("\n=== resident packed cache, streamed, deltas vs fp16 ===")
    for name, r in results.items():
        if "error" in r:
            print(f"{name:16s} FAILED   {r['error'][:70]}")
            continue
        d = r.get("ppl_delta_pct")
        print(f"{name:16s} PPL={r['perplexity']:9.4f} "
              f"delta={'   -   ' if d is None else f'{d:+.2f}%':>8s}  "
              f"peak {r['peak_alloc_gib']:.2f} GiB")


if __name__ == "__main__":
    main()
