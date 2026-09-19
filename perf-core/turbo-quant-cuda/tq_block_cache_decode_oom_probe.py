"""Find the decode-step memory ceiling for BlockQuantCache.

The cache stores packed KV in resident memory and re-decodes the entire
stored prefix every step. That is the design's cost: O(stored_blocks) per
step, paid in fp32 buffer + concat temporaries.

Earlier 8K prefill + decode hit CUDA OOM on step 0 inside _decode_all's
scales concat. Quantify where the threshold is so the docs can name it
honestly.

Run: python tq_block_cache_decode_oom_probe.py
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
CORPUS = r"C:\Users\koosh\agents\sandbox\tq-eval\corpus_repodocs.txt"
BLOCK = 32


def probe(prefill, decode_steps=4):
    tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True
    )
    model.eval()
    text = open(CORPUS, encoding="utf-8", errors="replace").read()
    ids = (
        tok(text, return_tensors="pt")
        .input_ids[:, : prefill + decode_steps]
        .to(model.device)
    )

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
    for prefill in (1024, 2048, 4096, 6144, 8192, 12288, 16384, 24576):
        last_ok = probe(prefill)
        gc.collect()
        torch.cuda.empty_cache()
        # Reset between probes by exiting and respawning would be cleanest, but
        # CUDA caching allocator doesn't release back to OS. We accept that the
        # 8192 probe runs against whatever the smaller probes left fragmented.
        if last_ok < 0:
            print(f"  -> decode failed at prefill {prefill}\n")
        else:
            print(f"  -> decode survived {last_ok + 1} steps at prefill {prefill}\n")


if __name__ == "__main__":
    main()
