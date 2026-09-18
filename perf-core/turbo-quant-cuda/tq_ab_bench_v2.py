"""TurboQuant+ A/B benchmark: FP16 KV (Run A) vs 4-bit quantized KV simulation (Run B).

Run A: stock transformers generation, FP16 weights, FP16 KV cache.
Run B: forward hooks on every self_attn.k_proj / v_proj that quantize the output
       to 4-bit (group_size=32) via turbo_quant encode/decode (fake-quant QDQ),
       simulating compressed-KV value noise. Resident KV cache remains FP16
       (hooks do not change cache layout); peak-VRAM delta therefore reflects
       compute overhead only. See notes field in JSON.

Offline: models are pre-cached in C:\\Users\\koosh\\.cache\\huggingface\\hub
(Qwen2.5-3B-Instruct snapshot verified complete with 2 safetensors shards).
HF_HUB_OFFLINE=1 and TRANSFORMERS_OFFLINE=1 are forced.
"""
import gc
import json
import os
import sys
import time
from datetime import datetime, timezone

os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
# HF cache root: E:\hf_cache holds the complete 7B weights (15.2 GB blobs).
# C:\Users\koosh\.cache\huggingface has the full 3B snapshot but only 7B metadata.
# Must be set BEFORE importing transformers (it snapshots HF_HOME at import time).
os.environ["HF_HOME"] = os.environ.get(
    "TQ_HF_HOME", r"C:\Users\koosh\.cache\huggingface"
)

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from turbo_quant_cuda import encode_uniform_cuda, decode_uniform_cuda

MODEL_ID = os.environ.get("TQ_MODEL_ID", "Qwen/Qwen2.5-3B-Instruct")
BITS = int(os.environ.get("TQ_BITS", "4"))
N_LAYERS = int(os.environ.get("TQ_N_LAYERS", "36"))
GROUP_SIZE = 32
MAX_NEW_TOKENS = {"short": 160, "medium": 300, "long": 420}

PROMPTS = [
    ("short", 1, "Explain quantum computing in simple terms."),
    ("short", 2, "Write a Python function to reverse a linked list."),
    ("short", 3, "What are the main differences between Python and Rust?"),
    ("short", 4, "Summarize the plot of Romeo and Juliet."),
    ("short", 5, "Write a haiku about autumn."),
    ("medium", 6, "Explain how a transformer neural network processes attention. Keep it under 200 words."),
    ("medium", 7, "Write a Python script that reads a CSV, filters rows, and writes summary stats. Include error handling."),
    ("long", 8, "Compare and contrast REST, GraphQL, and gRPC for API design. Provide code examples."),
    ("long", 9, "Write a short essay (300 words) on the economic trade-offs of vertical integration."),
    ("long", 10, "Explain the cause of the 2008 financial crisis in detail."),
]

HOOK_CALLS = {"count": 0}


def is_corrupted(text: str) -> bool:
    """Heuristic corruption detector: empty, token-spam, wrong-language, gibberish."""
    if not text or not text.strip():
        return True
    words = text.split()
    if len(words) >= 12:
        # same word repeated 8+ times consecutively
        run = best = 1
        for i in range(1, len(words)):
            run = run + 1 if words[i] == words[i - 1] else 1
            best = max(best, run)
        if best >= 8:
            return True
        # short 1-3 gram spam: >60% of text is one repeated gram
        for g in (1, 2, 3):
            grams = [tuple(words[i:i + g]) for i in range(len(words) - g + 1)]
            if grams:
                top = max(set(grams), key=grams.count)
                if grams.count(top) / len(grams) > 0.6:
                    return True
    n = len(text)
    cjk = sum(1 for c in text if '\u4e00' <= c <= '\u9fff' or '\u3040' <= c <= '\u30ff' or '\uac00' <= c <= '\ud7af')
    if cjk / n > 0.3:
        return True  # English prompts, CJK-heavy output
    printable = sum(c.isprintable() for c in text) / max(n, 1)
    return printable < 0.7


def ttft_and_generate(tok, model, prompt: str, max_new: int):
    """Greedy generation with TTFT measured via a manual prefill + first-token step.

    Returns (text, n_new, ttft_s, decode_s, total_s).
    """
    messages = [{"role": "user", "content": prompt}]
    inputs = tok.apply_chat_template(messages, return_tensors="pt", add_generation_prompt=True).to(model.device)
    torch.cuda.synchronize()
    t0 = time.perf_counter()
    with torch.no_grad():
        # prefill -> first token
        out1 = model.generate(inputs, max_new_tokens=1, do_sample=False,
                              pad_token_id=tok.pad_token_id or tok.eos_token_id)
        torch.cuda.synchronize()
        ttft = time.perf_counter() - t0
        t1 = time.perf_counter()
        out = model.generate(inputs, max_new_tokens=max_new, do_sample=False,
                             pad_token_id=tok.pad_token_id or tok.eos_token_id)
    torch.cuda.synchronize()
    total = time.perf_counter() - t0
    decode_s = time.perf_counter() - t1
    gen = out[0][inputs.shape[1]:]
    n_new = int(gen.shape[0])
    text = tok.decode(gen, skip_special_tokens=True)
    return text, n_new, ttft, decode_s, total


def run_suite(tok, model, label: str):
    results = []
    torch.cuda.reset_peak_memory_stats()
    torch.cuda.synchronize()
    for cat, pid, prompt in PROMPTS:
        text, n_new, ttft, decode_s, total = ttft_and_generate(tok, model, prompt, MAX_NEW_TOKENS[cat])
        tps_decode = (n_new - 1) / decode_s if decode_s > 0 and n_new > 1 else 0.0
        tps_overall = n_new / total if total > 0 else 0.0
        results.append({
            "prompt_id": pid,
            "category": cat,
            "prompt": prompt,
            "new_tokens": n_new,
            "ttft_s": round(ttft, 4),
            "decode_s": round(decode_s, 3),
            "total_s": round(total, 3),
            "tokens_per_s_decode": round(tps_decode, 2),
            "tokens_per_s_overall": round(tps_overall, 2),
            "corrupted": is_corrupted(text),
            "preview": text[:160],
        })
        print(f"  [{label}] p{pid} ({cat}) {n_new} tok, ttft {ttft:.3f}s, "
              f"{tps_decode:.1f} t/s decode, corrupted={results[-1]['corrupted']}", flush=True)
    peak_alloc = torch.cuda.max_memory_allocated() / 1024**3
    peak_res = torch.cuda.max_memory_reserved() / 1024**3
    return results, peak_alloc, peak_res


def install_hooks(model):
    handles = []

    def make_hook(orig_dtype_shape):
        def hook(mod, inp, output):
            HOOK_CALLS["count"] += 1
            data = output[0] if isinstance(output, tuple) else output
            orig_shape = data.shape
            orig_dtype = data.dtype
            flat = data.reshape(-1).to(torch.float32)
            n = flat.numel()
            pad = (GROUP_SIZE - n % GROUP_SIZE) % GROUP_SIZE
            if pad:
                flat = torch.cat([flat, flat[-pad:]])
            try:
                packed, scales, zeros = encode_uniform_cuda(flat, BITS, GROUP_SIZE)
                recon = decode_uniform_cuda(packed, scales, zeros, flat.numel(), BITS, GROUP_SIZE)
            finally:
                pass
            recon = recon[:n].reshape(orig_shape).to(orig_dtype)
            if isinstance(output, tuple):
                return (recon,) + tuple(output[1:])
            return recon
        return hook

    n_hooks = 0
    for i, layer in enumerate(model.model.layers):
        for proj_name in ("k_proj", "v_proj"):
            proj = getattr(layer.self_attn, proj_name)
            h = proj.register_forward_hook(make_hook(torch.float16))
            handles.append(h)
            n_hooks += 1
    print(f"hooks installed: {n_hooks}", flush=True)
    return handles


def main():
    t_start = time.perf_counter()
    print(f"PyTorch {torch.__version__}", flush=True)
    print(f"GPU: {torch.cuda.get_device_name(0)}", flush=True)
    tok = AutoTokenizer.from_pretrained(MODEL_ID, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        MODEL_ID, dtype=torch.float16, device_map="cuda", local_files_only=True,
    )
    model.eval()
    print(f"model loaded: {MODEL_ID} ({torch.cuda.memory_allocated()/1024**3:.2f} GiB allocated)", flush=True)

    # warmup both paths (cudnn autotune, kernel cache) - short dummy generations
    warm_in = tok.apply_chat_template([{"role": "user", "content": "Say hello."}],
                                      return_tensors="pt", add_generation_prompt=True).to(model.device)
    with torch.no_grad():
        model.generate(warm_in, max_new_tokens=8, do_sample=False,
                       pad_token_id=tok.pad_token_id or tok.eos_token_id)
        handles = install_hooks(model)
        model.generate(warm_in, max_new_tokens=8, do_sample=False,
                       pad_token_id=tok.pad_token_id or tok.eos_token_id)
    remove_hooks(handles)
    HOOK_CALLS["count"] = 0
    torch.cuda.empty_cache()
    gc.collect()
    print("warmup done", flush=True)

    # Run A: FP16 baseline
    print("=== RUN A: FP16 KV baseline ===", flush=True)
    a_results, a_alloc, a_res = run_suite(tok, model, "A")
    print(f"run A done: peak {a_alloc:.2f} GiB alloc / {a_res:.2f} GiB reserved", flush=True)

    # Run B: 4-bit KV QDQ hooks
    print("=== RUN B: 4-bit KV (TurboQuant QDQ hooks) ===", flush=True)
    handles = install_hooks(model)
    hook_calls_at_start = HOOK_CALLS["count"]
    b_results, b_alloc, b_res = run_suite(tok, model, "B")
    hook_calls_b = HOOK_CALLS["count"] - hook_calls_at_start
    remove_hooks(handles)
    print(f"run B done: peak {b_alloc:.2f} GiB alloc / {b_res:.2f} GiB reserved, hook calls={hook_calls_b}", flush=True)

    def agg(rows):
        valid = [r for r in rows if not r["corrupted"] and r["new_tokens"] > 0]
        n = max(len(valid), 1)
        return {
            "avg_ttft_s": round(sum(r["ttft_s"] for r in valid) / n, 4),
            "avg_tokens_per_s_decode": round(sum(r["tokens_per_s_decode"] for r in valid) / n, 2),
            "avg_tokens_per_s_overall": round(sum(r["tokens_per_s_overall"] for r in valid) / n, 2),
            "total_wall_s": round(sum(r["total_s"] for r in rows), 2),
            "corrupted_count": sum(1 for r in rows if r["corrupted"]),
            "total_new_tokens": sum(r["new_tokens"] for r in rows),
        }

    A, B = agg(a_results), agg(b_results)
    delta = {k: round((B[k] - A[k]) / A[k] * 100, 2) if A[k] else 0.0
             for k in ("avg_ttft_s", "avg_tokens_per_s_decode", "avg_tokens_per_s_overall")}

    report = {
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "benchmark": "turboquant_3b_ab",
        "pilot": "PILOT-PRD-PHENOMLX",
        "model": MODEL_ID,
        "hardware": {"gpu": torch.cuda.get_device_name(0), "vram_gb": 24,
                     "host": "desktop RTX 3090 Ti (100.96.135.160)"},
        "software": {"torch": torch.__version__, "transformers": __import__("transformers").__version__,
                     "python": sys.version.split()[0]},
        "config": {"bits": BITS, "group_size": GROUP_SIZE,
                   "max_new_tokens": MAX_NEW_TOKENS, "greedy": True,
                   "kv_mode": "fake-quant QDQ on k_proj/v_proj outputs (resident KV stays FP16)"},
        "hook_verification": {"hook_calls_run_b": hook_calls_b,
                              "expected_min": 2 * N_LAYERS * 1,
                              "note": f"2 projections x {N_LAYERS} layers x (prefill+decode steps)"},
        "run_a_fp16_kv": {"aggregate": A, "peak_vram_alloc_gib": round(a_alloc, 3),
                          "peak_vram_reserved_gib": round(a_res, 3), "prompts": a_results},
        "run_b_turboquant_4bit_kv_sim": {"aggregate": B, "peak_vram_alloc_gib": round(b_alloc, 3),
                                         "peak_vram_reserved_gib": round(b_res, 3), "prompts": b_results},
        "deltas_pct_b_vs_a": delta,
        "limitations": [
            "Run B hooks apply encode->decode (QDQ) to k_proj/v_proj outputs each forward pass; "
            "this injects 4-bit uniform-quantization noise into the values that populate the KV cache "
            "but does NOT reduce resident KV memory (cache stays FP16).",
            "Peak VRAM delta therefore measures compute overhead of the QDQ path, not cache savings.",
            "True packed-cache residency would require cache-layout surgery inside transformers; out of scope for this hook-based pilot.",
            "Corruption detection is heuristic (token spam, CJK ratio, printability); previews retained for manual review.",
            "TTFT measured as wall time of a 1-token generate call (includes one full forward pass); a second full generate call follows for the decode-rate window.",
        ],
        "wall_clock_total_s": round(time.perf_counter() - t_start, 1),
    }
    out_path = sys.argv[1] if len(sys.argv) > 1 else "tq_ab_results.json"
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)
    print(f"WROTE {out_path}", flush=True)

    print("\n=== SUMMARY (Qwen2.5-3B-Instruct, RTX 3090 Ti) ===")
    print(f"model: {MODEL_ID}")
    print(f"A (FP16 KV):   {A['avg_tokens_per_s_decode']:.1f} t/s decode | "
          f"TTFT {A['avg_ttft_s']*1000:.0f} ms | peak {a_alloc:.2f} GiB | corrupted {A['corrupted_count']}/10")
    print(f"B (4-bit sim): {B['avg_tokens_per_s_decode']:.1f} t/s decode | "
          f"TTFT {B['avg_ttft_s']*1000:.0f} ms | peak {b_alloc:.2f} GiB | corrupted {B['corrupted_count']}/10")
    print(f"hook calls in run B: {hook_calls_b}")
    print(f"deltas: {delta}")


def remove_hooks(handles):
    for h in handles:
        h.remove()


if __name__ == "__main__":
    main()
