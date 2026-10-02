"""Verify assert_histories_match in both directions, before trusting it.

The ABBA benchmark handed its reference a freshly allocated, never-prefilled
cache and reported a relative logit difference of 6.9e-1. That reads as a
broken CUDA graph and is not one, so `assert_histories_match` now refuses that
construction at build time. A guard that has only been shown accepting
correct input has not been shown to work, so both directions are checked here:

  1  two caches given the identical history are ACCEPTED, at 0.0
  2  a never-prefilled reference is REJECTED, naming the worst path
  3  a reference one step behind is REJECTED (this is the original trap)
  4  a reference with a perturbed KV block is REJECTED
  5  GraphHarness refuses to build a harness with a mismatched reference
  6  match_history=False still permits building one, for measuring divergence

Check 3 is the important one: it must reject the exact construction that
produced four consecutive false "graphs are broken" reports.
"""
import sys

sys.stdout.reconfigure(encoding="utf-8")

import torch

sys.path.insert(0, r"C:\phenotype-omlx\perf-core\turbo-quant-cuda")

MODEL_DIR = r"C:\Users\koosh\agents\sandbox\tq-eval\qwen35-9b"
CTX = 512
MAXLEN = 640
WARM = 4
results = []


def main():
    import transformers.cache_utils as cu
    from tq_lockstep_guard import (
        GraphHarness, assert_histories_match, rel,
    )
    from transformers import AutoConfig
    from transformers.models.qwen3_5 import Qwen3_5ForConditionalGeneration
    from transformers.utils import logging as hf_logging

    hf_logging.set_verbosity_error()
    hf_logging.disable_progress_bar()
    cfg = AutoConfig.from_pretrained(MODEL_DIR, local_files_only=True)
    model = Qwen3_5ForConditionalGeneration.from_pretrained(
        MODEL_DIR, dtype=torch.bfloat16, device_map="cuda",
        local_files_only=True)
    model.eval()
    dev = model.device

    def factory():
        return cu.StaticCache(config=cfg, max_cache_len=MAXLEN)

    def step(cache, t, p):
        with torch.no_grad():
            return model(t.view(1, 1), past_key_values=cache, use_cache=True,
                         cache_position=p).logits[:, -1].float().clone()

    g = torch.Generator().manual_seed(7)
    toks = torch.randint(1000, 50000, (1, CTX), device=dev)
    pos0 = torch.arange(CTX, device=dev)
    warm = torch.randint(1000, 50000, (WARM, 1), generator=g).to(dev)
    test = torch.randint(1000, 50000, (1, 1), generator=g).to(dev)

    def build(history=True):
        """A cache given `history` identical steps of the same warmup."""
        c = factory()
        if history:
            with torch.no_grad():
                model(toks, past_key_values=c, use_cache=True,
                      cache_position=pos0)
            for i in range(WARM):
                step(c, warm[i], torch.tensor([CTX + i], device=dev))
        return c

    a = build()
    b = build()
    del a, b
    torch.cuda.empty_cache()

    print("\n1. identical histories are ACCEPTED")
    a, b = build(), build()
    d = assert_histories_match(a, b)
    print(f"   worst difference: {d:.3e}")
    results.append(("identical histories accepted", d == 0.0))

    print("\n2. a never-prefilled reference is REJECTED")
    fresh = build(history=False)
    try:
        assert_histories_match(a, fresh)
        print("   ACCEPTED, which is wrong")
        results.append(("empty reference rejected", False))
    except ValueError as e:
        first = str(e).split(".")[0]
        print(f"   rejected: {first[:110]}")
        results.append(("empty reference rejected", True))
    del fresh
    torch.cuda.empty_cache()

    print("\n3. a reference ONE STEP BEHIND is REJECTED")
    # This is the original trap: one cache advanced further than the other.
    step(b, test, torch.tensor([CTX + WARM], device=dev))
    try:
        assert_histories_match(a, b)
        print("   ACCEPTED, which is wrong")
        results.append(("one-step offset rejected", False))
    except ValueError as e:
        first = str(e).split(".")[0]
        print(f"   rejected: {first[:110]}")
        results.append(("one-step offset rejected", True))

    print("\n   and the two really do disagree numerically")
    p = torch.tensor([CTX + WARM], device=dev)
    r = rel(step(a, test, p), step(b, test, p))
    print(f"   relative logit difference at that point: {r:.3e}")
    results.append(("the offset is a real divergence", r > 1e-2))
    del a, b
    torch.cuda.empty_cache()

    print("\n4. a single perturbed KV block is REJECTED")
    a, b = build(), build()
    lay = a.layers[3]
    lay.keys[:, :, CTX // 2, :] += 1.0
    try:
        assert_histories_match(a, b)
        print("   ACCEPTED, which is wrong")
        results.append(("perturbed KV rejected", False))
    except ValueError as e:
        first = str(e).split(".")[0]
        print(f"   rejected: {first[:110]}")
        results.append(("perturbed KV rejected", True))
    del a, b
    torch.cuda.empty_cache()

    print("\n5. GraphHarness refuses a mismatched reference")
    gc_, ref = build(), build(history=False)
    inp = torch.zeros(1, 1, dtype=torch.long, device=dev)
    inp.copy_(test)
    pos = torch.tensor([CTX + WARM], device=dev)
    refused = False
    try:
        GraphHarness(model, gc_, inp, pos, ref)
    except ValueError:
        refused = True
    print(f"   {'REFUSED as intended' if refused else 'DID NOT REFUSE'}")
    results.append(("GraphHarness refuses mismatched history", refused))
    del gc_, ref
    torch.cuda.empty_cache()

    print("\n6. match_history=False still permits building one")
    gc_, ref = build(), build(history=False)
    ok = False
    try:
        h = GraphHarness(model, gc_, inp, pos, ref, match_history=False)
        h.match_priming(test)
        r = h.compare()
        ok = True
        print(f"   built, and it reports the real divergence: {r:.3e}")
        results.append(("opt-out still reports divergence", r > 1e-2))
        del h
    except Exception as e:
        print(f"   failed to build: {type(e).__name__}: {e}")
        results.append(("opt-out still reports divergence", False))

    print("\nHISTORY GUARD SELFTEST")
    for name, good in results:
        print(f"  {'PASS' if good else 'FAIL'}  {name}")
    n_fail = sum(1 for _, v in results if not v)
    print(f"\n  {len(results) - n_fail}/{len(results)} passed")
    return 0 if n_fail == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
