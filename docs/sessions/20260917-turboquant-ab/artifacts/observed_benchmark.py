"""Offline CUDA FP16 versus TurboQuant 4-bit projection QDQ benchmark.

QDQ changes KV values, not cache storage: resident KV remains FP16.
"""
import argparse
import gc
import hashlib
import json
import os
from pathlib import Path
import statistics
import sys
import time
from datetime import datetime, timezone

os.environ['HF_HUB_OFFLINE'] = '1'
os.environ['TRANSFORMERS_OFFLINE'] = '1'
os.environ['HF_HUB_DISABLE_TELEMETRY'] = '1'
import torch
import transformers
from transformers import AutoModelForCausalLM, AutoTokenizer

PORT = Path(__file__).parent / 'turbo_quant_cuda.py'
if not PORT.exists():
    PORT = Path(__file__).resolve().parents[1] / 'perf-core/turbo-quant-cuda/turbo_quant_cuda.py'
sys.path.insert(0, str(PORT.parent))
from turbo_quant_cuda import encode_uniform_cuda, decode_uniform_cuda

PROMPTS = [
    'Explain quantum computing in simple terms.',
    'Write a Python function to reverse a linked list.',
    'What are the main differences between Python and Rust?',
    'Summarize the plot of Romeo and Juliet.',
    'Write a haiku about autumn.',
    'Explain how a transformer neural network processes attention. Keep it under 200 words.',
    'Write a Python script that reads a CSV, filters rows, and writes summary stats. Include error handling.',
    'Compare and contrast REST, GraphQL, and gRPC for API design. Provide code examples.',
    'Write a short essay (300 words) on the economic trade-offs of vertical integration.',
    'Explain the cause of the 2008 financial crisis in detail.',
]


def qdq(data):
    flat = data.reshape(-1).float()
    n = flat.numel()
    padding = (-n) % 32
    if padding:
        flat = torch.nn.functional.pad(flat, (0, padding))
    packed, scales, zeros = encode_uniform_cuda(flat, bits=4, group_size=32)
    restored = decode_uniform_cuda(packed, scales, zeros, flat.numel(), bits=4, group_size=32)
    return restored[:n].reshape(data.shape).to(data.dtype)


def smoke():
    torch.manual_seed(42)
    for shape in [(1, 17, 256), (1, 1, 256), (1, 7, 3)]:
        data = torch.randn(shape, device='cuda', dtype=torch.float16)
        restored = qdq(data)
        assert restored.shape == data.shape and restored.dtype == data.dtype
        assert restored.device == data.device and torch.isfinite(restored).all()
        assert not torch.equal(restored, data), 'Negative control: identity QDQ is invalid'
    data = torch.full((32,), 2.0, device='cuda', dtype=torch.float16)
    assert torch.equal(qdq(data), data)
    torch.cuda.synchronize()
    return {'roundtrip_shape_dtype_device_finite': True, 'nonidentity_control': True,
            'constant_group': True, 'shapes': 4}


class FirstTokenClock:
    def __init__(self):
        self.prefill = True
        self.first = None

    def put(self, value):
        if self.prefill:
            self.prefill = False  # generate streams the prompt first
        elif self.first is None:
            torch.cuda.synchronize()
            self.first = time.perf_counter()

    def end(self):
        pass


def flags(text):
    reasons = []
    if not text.strip():
        reasons.append('empty')
    words = text.split()
    for width in (1, 2, 3):
        for i in range(max(0, len(words) - 8 * width + 1)):
            if words[i:i + width] * 8 == words[i:i + 8 * width]:
                reasons.append('repeated_token_spam')
                break
    if text and sum(c.isprintable() or c.isspace() for c in text) / len(text) < .7:
        reasons.append('garbage')
    letters = [c for c in text if c.isalpha()]
    if letters and sum(not c.isascii() for c in letters) / len(letters) > .3:
        reasons.append('non_english_script')
    return sorted(set(reasons))


def generate(model, tokenizer, prompt, cap):
    inputs = tokenizer.apply_chat_template(
        [{'role': 'user', 'content': prompt}], add_generation_prompt=True,
        return_tensors='pt', return_dict=True).to('cuda')
    clock = FirstTokenClock()
    torch.cuda.synchronize()
    start = time.perf_counter()
    with torch.inference_mode():
        output = model.generate(**inputs, max_new_tokens=cap, do_sample=False,
                                streamer=clock, use_cache=True,
                                pad_token_id=tokenizer.eos_token_id)
    torch.cuda.synchronize()
    end = time.perf_counter()
    ids = output[0, inputs['input_ids'].shape[1]:].tolist()
    assert clock.first is not None and ids
    text = tokenizer.decode(ids, skip_special_tokens=True)
    ttft, total = clock.first - start, end - start
    reasons = flags(text)
    return {'new_tokens': len(ids), 'ttft_s': ttft, 'total_s': total,
            'decode_s': total - ttft, 'tokens_per_s': len(ids) / total,
            'decode_tokens_per_s': (len(ids) - 1) / (total - ttft),
            'text': text, 'corruption_reasons': reasons, 'corrupted': bool(reasons),
            'hit_token_cap': len(ids) == cap}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--smoke', action='store_true')
    parser.add_argument('--output', default='results.json')
    parser.add_argument('--checkpoint', required=True)
    args = parser.parse_args()
    checks = smoke()
    print('CUDA QDQ SMOKE PASS ' + json.dumps(checks), flush=True)
    if args.smoke:
        return
    checkpoint = Path(args.checkpoint)
    tokenizer = AutoTokenizer.from_pretrained(checkpoint, local_files_only=True)
    model = AutoModelForCausalLM.from_pretrained(
        checkpoint, dtype=torch.float16, local_files_only=True).to('cuda').eval()
    model.generation_config.temperature = None
    model.generation_config.top_p = None
    model.generation_config.top_k = None
    report = {'timestamp': datetime.now(timezone.utc).isoformat(),
              'model': model.config._name_or_path, 'parameters': model.num_parameters(),
              'checkpoint_revision': checkpoint.name,
              'gpu': torch.cuda.get_device_name(0), 'torch': torch.__version__,
              'transformers': transformers.__version__, 'python': sys.version,
              'port_sha256': hashlib.sha256(PORT.read_bytes()).hexdigest(),
              'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'checks': checks, 'runs': {}, 'bits': 4, 'group_size': 32,
              'limitations': ['QDQ simulates quantization noise, resident KV stays FP16.',
                              'Single sequential A then B pass, no significance inference.',
                              'Streamer copies tokens to CPU identically in both conditions.',
                              'Corruption flags are heuristic, full text retained for review.',
                              'Prompts use explicit user list, not differing pilot/config.json.',
                              'Caps 160/300/420 may truncate answers; truncation is reported.']}
    for label in ('A_fp16', 'B_qdq4'):
        handles, calls = [], [0]
        def hook(module, inputs, output):
            calls[0] += 1
            return qdq(output)
        try:
            if label == 'B_qdq4':
                for layer in model.model.layers:
                    for name in ('k_proj', 'v_proj'):
                        handles.append(getattr(layer.self_attn, name).register_forward_hook(hook))
            for _ in range(2):
                generate(model, tokenizer, 'Say hello.', 8)
            calls[0] = 0
            gc.collect()
            torch.cuda.empty_cache()
            torch.cuda.reset_peak_memory_stats()
            rows = []
            for i, prompt in enumerate(PROMPTS):
                cap = 160 if i < 5 else 300 if i < 7 else 420
                row = generate(model, tokenizer, prompt, cap)
                row.update(prompt_id=i + 1, prompt=prompt, max_new_tokens=cap)
                rows.append(row)
                print(f'{label} {i + 1}/10 {row["tokens_per_s"]:.2f} t/s', flush=True)
            if handles:
                assert calls[0] == len(handles) * sum(r['new_tokens'] for r in rows)
            total = sum(r['total_s'] for r in rows)
            report['runs'][label] = {
                'prompts': rows, 'hook_calls': calls[0], 'hook_count': len(handles),
                'tokens_per_s': sum(r['new_tokens'] for r in rows) / total,
                'decode_tokens_per_s': sum(r['new_tokens'] - 1 for r in rows) /
                                      sum(r['decode_s'] for r in rows),
                'mean_ttft_s': statistics.mean(r['ttft_s'] for r in rows),
                'peak_allocated_gib': torch.cuda.max_memory_allocated() / 1024**3,
                'peak_reserved_gib': torch.cuda.max_memory_reserved() / 1024**3,
                'corrupted_count': sum(r['corrupted'] for r in rows)}
            Path(args.output).write_text(json.dumps(report, indent=2), encoding='utf-8')
        finally:
            for handle in handles:
                handle.remove()
    print('COMPLETE ' + args.output, flush=True)


if __name__ == '__main__':
    main()
