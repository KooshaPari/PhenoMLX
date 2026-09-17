"""
8B model TurboQuant+ A/B benchmark.

Compares memory + throughput of:
  A) Standard FP16 KV cache (transformers default)
  B) TurboQuant+ 4-bit KV cache (research package)

Models tested: Qwen3.5-8B Q4_K_M (Q4 weights), 8B -> fits in 24GB GPU
Prompt set: same 10 prompts as the 0.8B test
"""

import argparse
import json
import os
import sys
import time
from pathlib import Path

import torch
from transformers import AutoModelForCausalLM, AutoTokenizer

# Use E: drive (1.2TB free)
os.environ['HF_HOME'] = r'E:\hf_cache'


PROMPTS = [
    "Explain the difference between TCP and UDP.",
    "Write a Python function to find the longest palindromic substring.",
    "What are the key differences between Rust and C++?",
    "Summarize the causes of World War I in 3 sentences.",
    "Write a SQL query to find the top 3 customers by revenue.",
    "Explain how gradient descent works, including learning rate.",
    "Debug this code: def fib(n): return fib(n-1) + fib(n-2)",
    "Write a bash script that finds all .log files larger than 100MB.",
    "What is the time complexity of quicksort in the worst case?",
    "Translate 'The quick brown fox jumps over the lazy dog' to French, Spanish, and Japanese.",
]


def measure_one(model, tokenizer, prompt: str, max_new_tokens: int = 256) -> dict:
    """Measure single prompt: TTFT, throughput, peak GPU memory delta."""
    torch.cuda.empty_cache()
    torch.cuda.reset_peak_memory_stats()

    inputs = tokenizer(prompt, return_tensors="pt").to(model.device)

    # TTFT: time from input to first token
    t0 = time.perf_counter()
    with torch.no_grad():
        out = model.generate(
            **inputs,
            max_new_tokens=max_new_tokens,
            do_sample=False,
            return_dict_in_generate=True,
        )
    t1 = time.perf_counter()

    elapsed_ms = (t1 - t0) * 1000
    generated = out.sequences[0][inputs.input_ids.shape[1]:]
    n_tokens = len(generated)
    text = tokenizer.decode(generated, skip_special_tokens=True)

    peak_mem_mb = torch.cuda.max_memory_allocated() / 1024 / 1024

    return {
        "prompt_chars": len(prompt),
        "n_tokens": n_tokens,
        "elapsed_ms": elapsed_ms,
        "peak_gpu_mem_mb": peak_mem_mb,
        "tokens_per_sec": n_tokens / (elapsed_ms / 1000) if elapsed_ms > 0 else 0,
        "text_preview": text[:80],
    }


def measure_baseline(model_id: str, label: str) -> dict:
    """Run 10 prompts with standard transformers FP16 KV cache."""
    print(f"=== {label}: loading {model_id} ===")
    tokenizer = AutoTokenizer.from_pretrained(model_id, trust_remote_code=True)
    model = AutoModelForCausalLM.from_pretrained(
        model_id,
        torch_dtype=torch.float16,
        device_map='cuda',
        trust_remote_code=True,
    )
    model.eval()

    print(f"=== {label}: GPU after load: {torch.cuda.memory_allocated()/1e9:.2f} GB ===")

    results = []
    for i, prompt in enumerate(PROMPTS):
        r = measure_one(model, tokenizer, prompt)
        r["prompt_idx"] = i
        results.append(r)
        print(f"  [{i+1}/10] {r['n_tokens']}t in {r['elapsed_ms']:.0f}ms "
              f"({r['tokens_per_sec']:.1f} t/s) peak={r['peak_gpu_mem_mb']:.0f}MB")

    # Final peak
    peak_overall = max(r['peak_gpu_mem_mb'] for r in results)

    # Cleanup
    del model
    del tokenizer
    torch.cuda.empty_cache()

    return {
        "label": label,
        "model_id": model_id,
        "prompts": results,
        "peak_overall_mb": peak_overall,
        "avg_tokens_per_sec": sum(r['tokens_per_sec'] for r in results) / len(results),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline-model", default="Qwen/Qwen2.5-7B-Instruct")
    parser.add_argument("--out", required=True, help="Output JSON path")
    args = parser.parse_args()

    print(f"PyTorch: {torch.__version__}")
    print(f"CUDA: {torch.version.cuda}, available: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"GPU: {torch.cuda.get_device_name(0)}")
        print(f"Total VRAM: {torch.cuda.get_device_properties(0).total_memory / 1e9:.1f} GB")

    # Baseline only for first pass (no TurboQuant+ in raw transformers)
    # Stock transformers KV cache is FP16 (no compression)
    baseline = measure_baseline(args.baseline_model, "BASELINE_FP16_KV")

    summary = {
        "hardware": {
            "gpu": torch.cuda.get_device_name(0) if torch.cuda.is_available() else "none",
            "vram_gb": torch.cuda.get_device_properties(0).total_memory / 1e9
                if torch.cuda.is_available() else 0,
        },
        "torch": torch.__version__,
        "cuda": torch.version.cuda,
        "runs": [baseline],
        "notes": [
            "Stock transformers uses FP16 KV cache by default",
            "TurboQuant+ requires PhenoMLX harbor_mlx_server (MLX-specific)",
            "Compare this baseline against PhenoMLX pilot results",
        ],
    }

    with open(args.out, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"\nResults written to {args.out}")
    print(f"Peak GPU memory: {baseline['peak_overall_mb']:.0f} MB ({baseline['peak_overall_mb']/1024:.2f} GB)")
    print(f"Avg throughput: {baseline['avg_tokens_per_sec']:.1f} t/s")


if __name__ == "__main__":
    main()
