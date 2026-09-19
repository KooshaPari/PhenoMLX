"""Find the decode-step memory ceiling for BlockQuantCache.

The cache stores packed KV in resident memory and re-decodes the entire
stored prefix every step. That is the design's cost: O(stored_blocks) per
step, paid in fp32 buffer + concat temporaries.

Earlier 8K prefill + decode hit CUDA OOM on step 0 inside _decode_all's
scales concat. Quantify where the threshold is so the docs can name it
honestly.

The model is loaded once outside the probe loop. Reloading it per probe
fragments VRAM (the safetensors mmap never gets fully released back to
the OS even with empty_cache) and the second probe at 4K would OOM where
a single-load run would not. Documented in the commit that introduced
this load-once layout.

The corpus must be at least `max(prefill) + decode_steps + 1` tokens long.
`corpus_long.txt` (39,061 tokens) covers the full ladder through 24576;
`corpus_repodocs.txt` (19,522 tokens) only covers through 12288, and a
prefill past its end raises a tensor-shape error inside attention rather
than telling you the corpus was short. A per-size length guard now prints
an explicit SKIP instead.

Peak VRAM depends only on the number of stored blocks, i.e. on the token
count, not on the corpus text, so the ladder is comparable across corpora.
The reported losses are not.

Run: python tq_block_cache_decode_oom_probe.py [prefill ...]
     (no args runs the full ladder PREFILL_LADDER)
"""

import gc
import os
import sys

os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")
os.environ.setdefault(
    "HF_HOME", os.environ.get("TQ_HF_HOME") or r"C:\Users\koosh\.cache\huggingface"
)

HERE = r"C:\phenotype-omlx\perf-core\turbo-quant-cuda"
sys.path.insert(0, HERE)

import torch  # noqa: E402
from transformers import AutoModelForCausalLM, AutoTokenizer  # noqa: E402

from tq_block_cache import BlockQuantCache  # noqa: E402

MODEL_ID = "Qwen/Qwen2.5-3B-Instruct"
CORPUS = r"C:\Users\koosh\agents\sandbox\tq-eval\corpus_long.txt"
BLOCK = 32
PREFILL_LADDER = (1024, 2048, 4096, 6144, 8192, 12288, 16384, 24576)
DECODE_STEPS = 4


def probe(model, tok, ids, prefill, decode_steps=DECODE_STEPS):
    cache = BlockQuantCache(block=BLOCK, bits=4)
    pos = torch.arange(0, prefill, device=model.device)
    with torch.no_grad():
        out = model(
            ids[:, :prefill],
            labels=ids[:, :prefill],
            past_key_values=cache,
            use_cache=True,
            cache_position=pos,
        )
    prefill_loss = float(out.loss)
    blocks = cache._stored[0]["k"]["blocks"]
    seq = cache.get_seq_length()
    print(
        f"  prefill {prefill} OK loss={prefill_loss:.4f} blocks={blocks} seq_len={seq}",
        flush=True,
    )
    del out
    gc.collect()
    torch.cuda.empty_cache()

    last = ids[:, prefill - 1 : prefill]
    last_ok = -1
    for step in range(decode_steps):
        gc.collect()
        torch.cuda.empty_cache()
        torch.cuda.reset_peak_memory_stats()
        pos = torch.tensor([prefill + step], device=model.device)
        try:
            with torch.no_grad():
                out = model(
                    last, past_key_values=cache, use_cache=True, cache_position=pos
                )
            a = torch.cuda.max_memory_allocated() / 1024**3
            r = torch.cuda.max_memory_reserved() / 1024**3
            print(
                f"  step {step} OK peak_alloc={a:.2f} reserved={r:.2f} GiB", flush=True
            )
            last_ok = step
            last = ids[:, prefill + step : prefill + step + 1]
            del out
        except Exception as e:
            print(
                f"  step {step} FAILED: {type(e).__name__}: {str(e)[:200]}", flush=True
            )
            return last_ok
    return last_ok


def main():
    print(f"torch {torch.__version__} | {torch.cuda.get_device_name(0)}\n")
    print("decode-step memory ceiling probe (block=32, bits=4)\n")
    tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
    )
    model.eval()
    text = open(CORPUS, encoding="utf-8", errors="replace").read()
    ids_all = tok(text, return_tensors="pt").input_ids
    ladder = tuple(int(a) for a in sys.argv[1:]) or PREFILL_LADDER
    max_prefill = max(ladder)
    need = max_prefill + DECODE_STEPS + 1
    print(
        f"corpus {len(ids_all[0])} tokens | ladder {list(ladder)} | need {need}\n",
        flush=True,
    )
    if len(ids_all[0]) < need:
        print(
            f"  corpus too short for the ladder ({len(ids_all[0])} < {need}); "
            f"shortest sizes still run, the rest SKIP\n",
            flush=True,
        )
    ids = ids_all.to(model.device)

    for prefill in ladder:
        if prefill + DECODE_STEPS + 1 > ids.shape[1]:
            print(
                f"  prefill {prefill} SKIPPED: corpus has {ids.shape[1]} tokens, "
                f"need {prefill + DECODE_STEPS + 1}\n",
                flush=True,
            )
            continue
        try:
            last_ok = probe(model, tok, ids, prefill)
        except Exception as e:
            print(f"  prefill {prefill} FAILED pre-decode: {e}", flush=True)
            gc.collect()
            torch.cuda.empty_cache()
            continue
        gc.collect()
        torch.cuda.empty_cache()
        if last_ok < 0:
            print(f"  -> decode failed at prefill {prefill}\n", flush=True)
        else:
            print(
                f"  -> decode survived {last_ok + 1} steps at prefill {prefill}\n",
                flush=True,
            )


if __name__ == "__main__":
    main()
