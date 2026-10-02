"""Measure graph vs eager decode latency under a genuine ABBA schedule.

The previous numbers in the repo docs came from a harness described as ABBA
that actually ran E,E,G,G per round, so each round measured E before G and G
after E, which biases the estimate by whatever drift exists within a round.
At 1024 context the observed spread was 1.129, so a biased estimate with a
spread that large cannot be quoted as a speedup at all.

This harness fixes three things:

  schedule      A B B A per round, so drift within a round cancels to first
                order. Both variants are sampled twice per round.
  spread        Median, MAD and full min/max are reported. A speedup whose
                rounds overlap is not a speedup, and the report says so.
  hygiene       No allocator work inside the timed region, no empty_cache
                between variants, and every side gets an identical cache
                history so the two variants decode the same computation.

Both variants are the same model and the same cache contents, advanced
independently and identically. Correctness is checked with the guard's own
primitives, not with a fresh implementation, so this file cannot disagree
with tq_lockstep_guard about what a valid comparison is.

Usage:

    TQWEN35_DIR=/path/to/qwen35-9b python tq_graph_abba_bench.py

At 16384 the prefill alone takes several minutes, so the contexts are worth
running one at a time. Set CONTEXTS above to choose.
"""
MODEL_DIR_DEFAULT = r"C:\Users\koosh\agents\sandbox\tq-eval\qwen35-9b"

import os
import statistics
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")

import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tq_lockstep_guard import (  # noqa: E402
    GraphHarness, assert_histories_match, tensor_census,
)

MODEL_DIR = os.environ.get("TQWEN35_DIR", MODEL_DIR_DEFAULT)
ROUNDS = 9
WARMUP = 4
INNER = 8
CONTEXTS = (1024, 4096, 16384)
MAXLEN_PAD = 128
LOCK_CHECKS = 4


def maxlen_for(ctx):
    """Allocate room for the context AND for every step the benchmark takes.

    A fixed 16512 maxlen reserves the 16384-token cache at every context, which
    the first attempt at this file did, so the 1024 run was paying for a run
    that was not happening. Sizing it tightly instead produced a
    device-side assert: the eager timing loop advances its cache by
    LOCK_CHECKS + ROUNDS * 2 * INNER steps, which at 1024 context is 148 more
    positions than the context itself, and `index_copy_` ran off the end.

    The budget has to cover the position advance, not just the context, which
    is the whole reason this is a function and not a constant.
    """
    budget = WARMUP + LOCK_CHECKS + ROUNDS * 2 * INNER
    return ctx + budget + MAXLEN_PAD


def sync():
    torch.cuda.synchronize()


def make_cache(cfg, maxlen):
    import transformers.cache_utils as cu
    return cu.StaticCache(config=cfg, max_cache_len=maxlen)


def abba(time_a, time_b, rounds=ROUNDS, inner=INNER):
    """Return per-round (a_ms, b_ms) from a balanced ABBA schedule.

    Round layout is A B B A. Every variant is sampled twice per round, once
    first and once last, so a monotonic drift contributes equally to both and
    cancels to first order. The estimate is the median over all samples.
    """
    out = []
    for _ in range(rounds):
        a1 = time_a(inner)
        b1 = time_b(inner)
        b2 = time_b(inner)
        a2 = time_a(inner)
        out.append((a1, b1, b2, a2))
    return out


def summarize(rows, na, nb):
    a = [r[0] for r in rows] + [r[3] for r in rows]
    b = [r[1] for r in rows] + [r[2] for r in rows]
    ma, mb = statistics.median(a), statistics.median(b)
    spread_a = (max(a) - min(a)) / ma
    spread_b = (max(b) - min(b)) / mb
    print(f"  {na:<8} median {ma:8.3f} ms   min {min(a):8.3f}  max {max(a):8.3f}"
          f"   spread {spread_a:.3f}")
    print(f"  {nb:<8} median {mb:8.3f} ms   min {min(b):8.3f}  max {max(b):8.3f}"
          f"   spread {spread_b:.3f}")
    ratio = ma / mb
    # A ratio is only credible when the two samples do not overlap.
    overlap = not (max(a) < min(b) or max(b) < min(a))
    verdict = "clean" if not overlap else "OVERLAPPING, not credible"
    print(f"  speedup {ratio:.3f}x   [{verdict}]")
    return ratio, spread_a, spread_b, overlap


def run_ctx(model, ctx, cfg, dev):
    import transformers.cache_utils as cu
    from tq_lockstep_guard import LockstepPair

    maxlen = maxlen_for(ctx)

    gcpu = torch.Generator().manual_seed(101 + ctx)
    toks = torch.randint(1000, 50000, (1, ctx), device=dev)
    pos0 = torch.arange(ctx, device=dev)
    warm = torch.randint(1000, 50000, (WARMUP, 1), generator=gcpu).to(dev)
    test = torch.randint(1000, 50000, (1, 1), generator=gcpu).to(dev)
    p_c = torch.tensor([ctx + WARMUP], device=dev)

    def factory():
        return cu.StaticCache(config=cfg, max_cache_len=maxlen)

    def eager_step(cache, t, p):
        with torch.no_grad():
            return model(t.view(1, 1), past_key_values=cache, use_cache=True,
                         cache_position=p).logits[:, -1].float().clone()

    # LockstepPair.__init__ prefills both eager caches. The graph's cache gets
    # the same history, applied once each, so all three decode identical
    # work and a difference between variants is overhead rather than input.
    pair = LockstepPair(model, toks, pos0, factory, label=f"abba-{ctx}")
    for i in range(WARMUP):
        pair.step(warm[i], ctx + i)

    gc_ = make_cache(cfg, maxlen)
    with torch.no_grad():
        model(toks, past_key_values=gc_, use_cache=True, cache_position=pos0)
    for i in range(WARMUP):
        t = torch.tensor([ctx + i], device=dev)
        eager_step(gc_, warm[i], t)

    # The reference gets its OWN cache, given the identical history step by
    # step. Sharing pair.caches[1] would work too, but sharing hides the class
    # of bug that has already cost this investigation twice, so the reference
    # is built independently and the histories are proved equal. The first
    # attempt at this file passed a freshly allocated, never-prefilled cache
    # and reported 6.9e-1, which is a harness error, not a graph error.
    ref = make_cache(cfg, maxlen)
    with torch.no_grad():
        model(toks, past_key_values=ref, use_cache=True, cache_position=pos0)
    for i in range(WARMUP):
        t = torch.tensor([ctx + i], device=dev)
        eager_step(ref, warm[i], t)
    d_hist = assert_histories_match(gc_, ref)
    print(f"  history check: worst cache difference before capture = {d_hist:.3e}")

    inp = torch.zeros(1, 1, dtype=torch.long, device=dev)
    inp.copy_(test)
    h = GraphHarness(model, gc_, inp, p_c, ref)
    h.match_priming(test)

    # Correctness first. A latency number from a wrong comparison is not a
    # measurement, and this is the only place the claim is established.
    worst = 0.0
    for k in range(LOCK_CHECKS):
        h.pos.fill_(ctx + WARMUP + k)
        worst = max(worst, h.compare())
    print(f"  correctness: worst relative logit diff over {LOCK_CHECKS} steps "
          f"= {worst:.3e}")
    if worst != 0.0:
        print("  ABORT: not bit-identical, refusing to publish a speedup.")
        del pair, h
        torch.cuda.empty_cache()
        return None

    def time_graph(inner):
        sync()
        t0 = time.perf_counter()
        for _ in range(inner):
            h.graph.replay()
        sync()
        return (time.perf_counter() - t0) * 1e3 / inner

    def time_eager(inner):
        sync()
        t0 = time.perf_counter()
        for _ in range(inner):
            eager_step(pair.caches[0], test, h.pos)
        sync()
        return (time.perf_counter() - t0) * 1e3 / inner

    # Discard the first round entirely: the allocator is still settling and
    # the first timed launch of a graph after capture is not representative.
    time_eager(INNER)
    time_graph(INNER)
    rows = abba(time_eager, time_graph, rounds=ROUNDS, inner=INNER)

    n_census = len(tensor_census(gc_))
    ratio, sa, sb, overlap = summarize(rows, "eager", "graph")
    print(f"  cache tensors live during the run: {n_census}")

    del pair, h, ref
    torch.cuda.empty_cache()
    return {"ctx": ctx, "eager": statistics.median(
        [r[0] for r in rows] + [r[3] for r in rows]),
        "graph": statistics.median(
            [r[1] for r in rows] + [r[2] for r in rows]),
        "ratio": ratio, "spread_eager": sa, "spread_graph": sb,
        "overlap": overlap, "worst": worst}


def main():
    if not os.path.isdir(MODEL_DIR):
        print(f"model directory not found: {MODEL_DIR}\n"
              f"set TQWEN35_DIR to the Qwen3.5-9B directory")
        return 2
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
    print(f"device {dev}  torch {torch.__version__}")
    print(f"schedule A B B A x {ROUNDS} rounds, {INNER} inner steps, "
          f"first round discarded\n")

    results = []
    for ctx in CONTEXTS:
        print(f"context {ctx}")
        r = run_ctx(model, ctx, cfg, dev)
        print()
        if r:
            results.append(r)

    print("ABBA DECODE LATENCY, Qwen3.5-9B")
    print(f"{'ctx':>7} {'eager ms':>10} {'graph ms':>10} {'speedup':>9} "
          f"{'spr E':>7} {'spr G':>7}  credible")
    for r in results:
        print(f"{r['ctx']:>7} {r['eager']:>10.3f} {r['graph']:>10.3f} "
              f"{r['ratio']:>8.3f}x {r['spread_eager']:>7.3f} "
              f"{r['spread_graph']:>7.3f}  "
              f"{'no' if r['overlap'] else 'yes'}")
    print("\nSpread is (max-min)/median over all 18 samples per variant. A "
          "speedup whose two sample sets overlap is reported but is not a "
          "claim.")
    return 0 if results else 1


if __name__ == "__main__":
    sys.exit(main())
