"""Attribute BlockQuantCache's resident footprint by context length.

The long-context table in `docs/TURBOQUANT-EXTRAPOLATION.md` item (g) reports a
peak-VRAM saving over fp16 that grows with context, and the OOM ladder reports a
decode-step peak that grows faster than the resident payload would explain.
Neither says what the resident cache is actually made of. `byte_breakdown()`
does, and comparing its total against `fp16_kv_bytes()` for the same shape is
what tells you whether the saving survives at length.

The prefill is fed in `--step` token chunks, the same path the eval uses. A
single-shot prefill of N tokens is far slower: the encode loop runs once per
32-token block either way, but the model forward is O(N^2) and the loss head
sees all N positions at once.

Run:
    python tq_block_cache_breakdown_probe.py                     # default ladder
    python tq_block_cache_breakdown_probe.py 4096 16384 32768
    python tq_block_cache_breakdown_probe.py --json OUT.json
"""

import argparse
import gc
import json
import os
import sys
import time

os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
os.environ.setdefault(
    "HF_HOME", os.environ.get("TQ_HF_HOME") or r"C:\Users\koosh\.cache\huggingface"
)

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import torch  # noqa: E402
from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402

from tq_block_cache import BlockQuantCache, fp16_kv_bytes  # noqa: E402

MODEL_ID = "Qwen/Qwen2.5-3B-Instruct"
CORPUS = r"C:\Users\koosh\agents\sandbox\tq-eval\corpus_long.txt"
BLOCK = 32
BITS = 4
STEP = 256
LADDER = (2048, 8192, 16384, 32768)
MIB = 1024**2


def feed(model, cache, ids, step=STEP):
    """Chunked prefill; returns the number of tokens actually fed."""
    fed = 0
    with torch.no_grad():
        for start in range(0, ids.shape[1] - 1, step):
            end = min(start + step, ids.shape[1] - 1)
            if end <= start:
                continue
            part = ids[:, start:end]
            pos = torch.arange(start, end, device=model.device)
            out = model(
                part,
                past_key_values=cache,
                use_cache=True,
                cache_position=pos,
            )
            del out
            fed = end
    return fed


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("lengths", nargs="*", type=int, default=[])
    ap.add_argument("--json")
    ap.add_argument("--step", type=int, default=STEP)
    args = ap.parse_args()
    ladder = tuple(args.lengths) if args.lengths else LADDER

    print(f"torch {torch.__version__} | {torch.cuda.get_device_name(0)}")
    print(f"block={BLOCK} bits={BITS} step={args.step}\n", flush=True)
    tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
    )
    model.eval()
    text = open(CORPUS, encoding="utf-8", errors="replace").read()
    ids_all = tok(text, return_tensors="pt").input_ids.to(model.device)
    n_avail = ids_all.shape[1]
    print(f"corpus {n_avail} tokens\n", flush=True)

    cfg = model.config
    rows = {}
    for want in ladder:
        n = min(want, n_avail)
        if n < want:
            print(f"  {want}: corpus only has {n_avail}, using {n}\n", flush=True)
        ids = ids_all[:, : n + 1]
        cache = BlockQuantCache(block=BLOCK, bits=BITS)
        t0 = time.perf_counter()
        fed = feed(model, cache, ids, args.step)
        torch.cuda.synchronize()
        wall = time.perf_counter() - t0
        bd = cache.byte_breakdown()
        seq = cache.get_seq_length()
        blocks = sum(b["k"]["blocks"] for b in cache._stored.values())
        f16 = (
            fp16_kv_bytes(
                (
                    1,
                    cfg.num_key_value_heads,
                    seq,
                    cfg.hidden_size // cfg.num_attention_heads,
                )
            )
            * cfg.num_hidden_layers
        )
        total = bd["total"]
        row = {
            "requested_tokens": want,
            "fed_tokens": fed,
            "seq_len": seq,
            "layers": len(cache._stored),
            "blocks_total": blocks,
            "payload_mib": round(bd["payload"] / MIB, 3),
            "metadata_mib": round(bd["metadata"] / MIB, 3),
            "residual_mib": round(bd["residual"] / MIB, 3),
            "total_mib": round(total / MIB, 3),
            "payload_share": round(bd["payload"] / total, 4) if total else None,
            "metadata_share": round(bd["metadata"] / total, 4) if total else None,
            "residual_share": round(bd["residual"] / total, 4) if total else None,
            "fp16_kv_mib": round(f16 / MIB, 3),
            "reduction_vs_fp16": round(f16 / total, 4) if total else None,
            "mib_per_block": round(total / MIB / blocks, 4) if blocks else None,
            "wall_clock_s": round(wall, 1),
        }
        rows[want] = row
        print(
            f"  {want:>6} tokens: seq={seq} blocks={blocks} "
            f"payload={row['payload_mib']:9.2f} metadata={row['metadata_mib']:9.2f} "
            f"residual={row['residual_mib']:6.2f} MiB | total={row['total_mib']:9.2f} "
            f"MiB vs fp16 {row['fp16_kv_mib']:9.2f} MiB = "
            f"{row['reduction_vs_fp16']:.3f}x smaller "
            f"| {row['mib_per_block']:.3f} MiB/block | {wall:.1f}s",
            flush=True,
        )
        print(flush=True)
        del cache
        gc.collect()
        torch.cuda.empty_cache()

    if args.json:
        out = {
            "model": MODEL_ID,
            "block": BLOCK,
            "bits": BITS,
            "step": args.step,
            "corpus": os.path.basename(CORPUS),
            "rows": rows,
        }
        with open(args.json, "w", encoding="utf-8") as fh:
            json.dump(out, fh, indent=2, sort_keys=True)
            fh.write("\n")
        print(f"wrote {args.json}")


if __name__ == "__main__":
    main()
