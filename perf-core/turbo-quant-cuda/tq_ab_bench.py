"""TurboQuant+ A/B benchmark: FP16 KV vs 4-bit quantized KV.

Run A: stock transformers generation (FP16 KV baseline).
Run B: k_proj/v_proj forward hooks quantize outputs to 4-bit (group_size=32)
       via turbo_quant encode/decode, simulating a compressed KV cache.

Outputs JSON to stdout path arg. Model: Qwen2.5-3B-Instruct (falls back to 1.5B).
"""
import gc
import json
import os
import sys
import time
from datetime import datetime, timezone

# Offline after load
os.environ.setdefault("HF_HUB_OFFLINE", "0")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "0")

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from turbo_quant_cuda import encode_uniform_cuda, decode_uniform_cuda

PROMPTS = [
    "Explain quantum computing in simple terms.",
    "Write a Python function to reverse a linked list.",
    "What are the main differences between Python and Rust?",
    "Summarize the plot of Romeo and Juliet.",
    "Write a haiku about autumn.",
    "Explain how a transformer neural network processes attention. Keep it under 200 words.",
    "Write a Python script that reads a CSV, filters rows, and writes summary stats. Include error handling.",
    "Compare and contrast REST, GraphQL, and gRPC for API design. Provide code examples.",
    "Write a short essay (300 words) on the economic trade-offs of vertical integration.",
    "Explain the cause of the 2008 financial crisis in detail.",
]

MAX_NEW_TOKENS = 128
BITS = 4
GROUP_SIZE = 32


def try_load_model():
    for name in ("Qwen/Qwen2.5-3B-Instruct", "Qwen/Qwen2.5-1.5B-Instruct"):
        try:
            tok = AutoTokenizer.from_pretrained(name)
            model = AutoModelForCausalLM.from_pretrained(
                name, torch_dtype=torch.float16, device_map="cuda"
            )
            model.eval()
            return name, tok, model
        except Exception as e:
            print(f"load {name} failed: {e}", flush=True)
    raise RuntimeError("no model loadable")


def is_corrupted(text: str) -> bool:
    if not text or not text.strip():
        return True
    words = text.split()
    if len(words) >= 12:
        # repeated token spam: same token 8+ times consecutively
        run = best = 1
        for i in range(1, len(words)):
            run = run + 1 if words[i] == words[i - 1] else 1
            best = max(best, run)
        if best >= 8:
            return True
    printable = sum(c.isprintable() for c in text) / max(len(text), 1)
    return printable < 0.7


def run_suite(tok, model, hooks_on: bool, hooks=None):
    """Generate for all prompts; return per-prompt metrics."""
    results = []
    torch.cuda.reset_peak_memory_stats()
    torch.cuda.synchronize()
    for prompt in PROMPTS:
        messages = [{"role": "user", "content": prompt}]
        inputs = tok.apply_chat_template(
            messages, return_tensors="pt", add_generation_prompt=True
        ).to(model.device)
        torch.cuda.synchronize()
        t0 = time.perf_counter()
        with torch.no_grad():
            out = model.generate(
                inputs,
                max_new_tokens=MAX_NEW_TOKENS,
                do_sample=False,
                pad_token_id=tok.pad_token_id or tok.eos_token_id,
            )
        torch.cuda.synchronize()
        dt = time.perf_counter() - t0
        gen = out[0][inputs.shape[1]:]
        n_tok = int(gen.shape[0])
        text = tok.decode(gen, skip_special_tokens=True)
        results.append(
            {
                "prompt": prompt[:60],
                "new_tokens": n_tok,
                "wall_s": round(dt, 3),
                "tokens_per_s": round(n_tok / dt, 2) if dt > 0 else 0.0,
                "ttft_proxy_s": round(dt / max(n_tok, 1) * n_tok, 3),  # full-gen; TTFT needs streaming
                "corrupted": is_corrupted(text),
                "preview": text[:120],
            }
        )
    peak = torch.cuda.max_memory_allocated() / 1024**3
    return results, peak


def install_hooks(model):
    handles = []
    for layer in model.model.layers:
        for proj_name in ("k_proj", "v_proj"):
            proj = getattr(layer.self_attn, proj_name)

            def hook(mod, inp, output, _proj=proj):
                # output may be tuple; take first element
                data = output[0] if isinstance(output, tuple) else output
                orig_dtype = data.dtype
                shape = data.shape
                # flatten + pad to a multiple of group_size, cast to fp32 (encode contract)
                flat = data.reshape(-1).to(torch.float32)
                n = flat.numel()
                pad = (-n) % GROUP_SIZE
                if pad:
                    flat = torch.cat([flat, flat.new_zeros(pad)])
                packed, scales, zeros = encode_uniform_cuda(flat, BITS, GROUP_SIZE)
                recon = decode_uniform_cuda(packed, scales, zeros, flat.numel(), BITS, GROUP_SIZE)[:n]
                recon = recon.view(shape).to(orig_dtype)
                new_out = (recon,) + tuple(output[1:]) if isinstance(output, tuple) else recon
                return new_out

            handles.append(proj.register_forward_hook(hook))
    return handles


def remove_hooks(handles):
    for h in handles:
        h.remove()


def main():
    model_name, tok, model = try_load_model()
    torch.cuda.reset_peak_memory_stats()
    print(f"model={model_name} gpu={torch.cuda.get_device_name(0)}", flush=True)

    # warmup (compile paths, cudnn autotune)
    warm = tok("warmup", return_tensors="pt").to(model.device)
    with torch.no_grad():
        model(**warm)

    # Run A: FP16 baseline
    a_results, a_peak = run_suite(tok, model, hooks_on=False)
    print(f"run A done: peak {a_peak:.2f} GB", flush=True)

    # Run B: 4-bit KV via hooks
    handles = install_hooks(model)
    b_results, b_peak = run_suite(tok, model, hooks_on=True, hooks=handles)
    remove_hooks(handles)
    print(f"run B done: peak {b_peak:.2f} GB", flush=True)

    def agg(rows):
        ok = [r for r in rows if r["new_tokens"] > 0]
        return {
            "avg_tokens_per_s": round(sum(r["tokens_per_s"] for r in ok) / len(ok), 2) if ok else 0,
            "corrupted_count": sum(1 for r in rows if r["corrupted"]),
            "total_wall_s": round(sum(r["wall_s"] for r in rows), 2),
        }

    report = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "gpu": torch.cuda.get_device_name(0),
        "model": model_name,
        "config": {"bits": BITS, "group_size": GROUP_SIZE, "max_new_tokens": MAX_NEW_TOKENS},
        "run_a_fp16": {"aggregate": agg(a_results), "peak_gb": round(a_peak, 2), "prompts": a_results},
        "run_b_turboquant_4bit": {"aggregate": agg(b_results), "peak_gb": round(b_peak, 2), "prompts": b_results},
        "notes": "Run B hooks k_proj/v_proj outputs through 4-bit quantize->dequantize each forward, simulating compressed-KV compute cost. Memory delta understated: packed tensors are transient (encode/decode per step) so peak GB reflects compute overhead, not cache residency savings.",
    }
    out_path = sys.argv[1] if len(sys.argv) > 1 else "tq_ab_results.json"
    with open(out_path, "w") as f:
        json.dump(report, f, indent=2)
    print(f"WROTE {out_path}", flush=True)

    A = report["run_a_fp16"]["aggregate"]
    B = report["run_b_turboquant_4bit"]["aggregate"]
    print(f"A: {A['avg_tokens_per_s']} t/s, corrupted {A['corrupted_count']}, peak {a_peak:.2f} GB")
    print(f"B: {B['avg_tokens_per_s']} t/s, corrupted {B['corrupted_count']}, peak {b_peak:.2f} GB")
    delta = (B["avg_tokens_per_s"] - A["avg_tokens_per_s"]) / max(A["avg_tokens_per_s"], 1e-9) * 100
    print(f"throughput delta: {delta:+.1f}%  (B vs A)")


if __name__ == "__main__":
    main()
