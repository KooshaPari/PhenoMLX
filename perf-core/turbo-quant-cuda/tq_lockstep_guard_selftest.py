"""Self-test for tq_lockstep_guard, run against the real Qwen3.5-9B.

The guard exists because a specific mistake nearly ended the CUDA-graph work, so
a self-test that does not exercise that mistake proves nothing. Every check
here corresponds to a way these comparisons have actually gone wrong.

  1  two eager caches, identical history, agree at exactly 0.0
  2  an intentionally asymmetric pair is DETECTED, not silently passed
  3  GraphHarness refuses to compare before match_priming
  4  after match_priming, 8 lockstep steps are bit-identical
  5  the graph reads live cache memory, not a frozen copy
  6  the census finds the 24 linear-attention states, not only the KV buffers
  7  cache addresses survive a decode step

Run with the qwen35 venv, from anywhere:

    python tq_lockstep_guard_selftest.py
"""
import sys
import types

sys.stdout.reconfigure(encoding="utf-8")

try:
    import torch
except ImportError:
    print("torch is required; run this under the qwen35 venv")
    sys.exit(2)

MODEL_DIR = r"C:\Users\koosh\agents\sandbox\tq-eval\qwen35-9b"
CTX = 1024
MAXLEN = 1152
WARM = 4
LOCK = 8
TOL = 1e-2


def stub_cache(n_layers=3):
    """A cache-shaped object for the model-free checks."""
    c = types.SimpleNamespace()
    c.layers = []
    for _ in range(n_layers):
        layer = types.SimpleNamespace()
        layer.keys = torch.zeros(1, 2, 8, 4)
        layer.values = torch.zeros(1, 2, 8, 4)
        layer.conv_states = [torch.zeros(1, 2, 4), torch.zeros(1, 2, 5)]
        layer.recurrent_states = [torch.zeros(1, 2, 4)]
        c.layers.append(layer)
    return c


def model_free_checks():
    """Checks that need no model, so a regression is caught in seconds."""
    from tq_lockstep_guard import (
        _get, assert_cache_reads_are_live, moved_addresses, rel, tensor_census,
    )
    results = []

    rel(torch.ones(4), torch.ones(4))
    results.append(("rel runs", True))

    cache = stub_cache()
    census = tensor_census(cache)
    # 3 layers x (keys, values, 2 conv_states, 1 recurrent_states) = 15.
    results.append((f"census found {len(census)} tensors", len(census) == 15))

    unresolved = [p for p in census
                  if not isinstance(_get(cache, p), torch.Tensor)]
    results.append((f"all {len(census)} census paths resolve",
                    not unresolved))

    moved = moved_addresses({"a": 1, "b": 2}, {"a": 1, "b": 3})
    results.append(("moved_addresses finds the change", moved == ["b"]))

    calls = {"n": 0}

    def replay():
        calls["n"] += 1
        return torch.full((1, 8), float(calls["n"]))

    # The live-memory probe zeroes full-attention KV only, deliberately: the
    # linear-attention state is recurrent and zeroing it would not distinguish a
    # live read from a stale one. 3 layers x (keys, values) = 6.
    shift, n_mut = assert_cache_reads_are_live(replay, cache)
    results.append((f"live-memory check mutates {n_mut} KV tensors and sees it",
                    n_mut == 6 and shift > 0))

    return results


def main():
    results = model_free_checks()

    import transformers.cache_utils as cu
    from tq_lockstep_guard import (
        GraphHarness, LockstepPair, moved_addresses, tensor_census,
    )
    from transformers import AutoConfig
    from transformers.models.qwen3_5 import Qwen3_5ForConditionalGeneration
    from transformers.utils import logging as hf_logging

    hf_logging.set_verbosity_error()
    hf_logging.disable_progress_bar()

    cfg = AutoConfig.from_pretrained(MODEL_DIR, local_files_only=True)
    if cfg.model_type != "qwen3_5":
        print(f"refusing model_type={cfg.model_type!r}; this self-test is "
              f"written against Qwen3.5")
        return 1
    model = Qwen3_5ForConditionalGeneration.from_pretrained(
        MODEL_DIR, dtype=torch.bfloat16, device_map="cuda", local_files_only=True)
    model.eval()
    dev = model.device

    def factory():
        return cu.StaticCache(config=model.config, max_cache_len=MAXLEN)

    def eager_step(cache, token, pos):
        with torch.no_grad():
            return model(token.view(1, 1), past_key_values=cache,
                         use_cache=True,
                         cache_position=pos).logits[:, -1].float().clone()

    ids = torch.randint(1000, 50000, (1, CTX), device=dev)
    pos0 = torch.arange(CTX, device=dev)
    gcpu = torch.Generator().manual_seed(17)
    warm_t = torch.randint(1000, 50000, (WARM, 1), generator=gcpu).to(dev)
    test_t = torch.randint(1000, 50000, (1, 1), generator=gcpu).to(dev)
    p_c = torch.tensor([CTX + WARM], device=dev)

    print("\n1. two eager caches, symmetric history")
    pair = LockstepPair(model, ids, pos0, factory, label="eager-vs-eager")
    for i in range(WARM):
        pair.step(warm_t[i], CTX + i)
    r = pair.compare()
    print(f"   after {pair.n} symmetric steps: {r:.3e}")
    results.append(("eager-vs-eager reports zero", r == 0.0))
    del pair
    torch.cuda.empty_cache()

    print("\n2. asymmetric pair must be DETECTED")
    bad = LockstepPair(model, ids, pos0, factory, label="asymmetric")
    for i in range(3):
        bad.step_one(0, test_t, CTX + 30 + i)
    bad.step(test_t, p_c)
    rb = bad.compare()
    print(f"   3-step offset reports: {rb:.3e}")
    results.append(("asymmetry detected", rb > TOL))
    del bad
    torch.cuda.empty_cache()

    print("\n3. GraphHarness refuses to compare before match_priming")
    cg, ce = factory(), factory()
    for c in (cg, ce):
        with torch.no_grad():
            model(ids, past_key_values=c, use_cache=True, cache_position=pos0)
    for i in range(WARM):
        tp = torch.tensor([CTX + i], device=dev)
        eager_step(cg, warm_t[i], tp)
        eager_step(ce, warm_t[i], tp)
    inp = torch.zeros(1, 1, dtype=torch.long, device=dev)
    inp.copy_(test_t)
    h = GraphHarness(model, cg, inp, p_c, ce)
    refused = False
    try:
        h.compare()
    except ValueError:
        refused = True
    print(f"   {'REFUSED as intended' if refused else 'DID NOT REFUSE'}")
    results.append(("unmatched compare refused", refused))

    print("\n4. after match_priming, an 8-step lockstep")
    h.match_priming(test_t)
    worst = 0.0
    for k in range(LOCK):
        h.pos.fill_(CTX + WARM + k)
        worst = max(worst, h.compare())
    print(f"   worst over {LOCK} steps: {worst:.3e}")
    results.append(("graph bit-identical over lockstep", worst == 0.0))

    print("\n5. the graph reads live cache memory")
    h.graph.replay()
    torch.cuda.synchronize()
    before = h.out.logits[:, -1].float().clone()
    n_mut = 0
    for layer in getattr(cg, "layers", []):
        for nm in ("keys", "values"):
            t = getattr(layer, nm, None)
            if isinstance(t, torch.Tensor) and t.is_floating_point():
                t.mul_(0.0)
                n_mut += 1
    torch.cuda.synchronize()
    h.graph.replay()
    torch.cuda.synchronize()
    shift = (h.out.logits[:, -1].float() - before).abs().max().item()
    print(f"   zeroing {n_mut} KV tensors moved the output by {shift:.3e}")
    results.append(("graph reads live cache memory", shift > 1e-3))
    del h
    torch.cuda.empty_cache()

    print("\n6. tensor census finds the linear-attention state")
    census = tensor_census(ce)
    lin = sum(1 for k in census
              if "conv_states" in k or "recurrent_states" in k)
    print(f"   tensors reachable: {len(census)}")
    print(f"   linear-attention state tensors: {lin}")
    results.append(("census sees linear state",
                    len(census) >= 70 and lin >= 40))

    print("\n7. addresses stable across a decode step")
    before_c = tensor_census(ce)
    eager_step(ce, test_t, p_c)
    moved = moved_addresses(before_c, tensor_census(ce))
    print(f"   moved: {len(moved)} of {len(before_c)}")
    results.append(("addresses stable", not moved))

    print("\nLOCKSTEP GUARD SELFTEST")
    for name, good in results:
        print(f"  {'PASS' if good else 'FAIL'}  {name}")
    n_fail = sum(1 for _, g in results if not g)
    print(f"\n  {len(results) - n_fail}/{len(results)} passed")
    if n_fail:
        print("\n  FAILED. If a graph comparison is needed right now, do not "
              "trust it until this passes.")
    else:
        print("\n  The one-sided-priming trap is now unreachable by forgetting: "
              "GraphHarness raises rather than reporting a false divergence.")
    return 0 if n_fail == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
