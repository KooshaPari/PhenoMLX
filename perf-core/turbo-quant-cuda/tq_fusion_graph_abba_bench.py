"""ABBA decode latency with Inductor fusion plus manual CUDA-graph capture.

This is the measurement the Inductor static-launcher investigation was
blocking. Its two open questions are now answered on Qwen3.5-9B:

  1. Can fusion and manual graph capture coexist on this build?  Yes, but
     only with `inductor.config.use_static_cuda_launcher = False`.  With the
     static launcher on, EVERY Inductor Triton kernel routes through
     `_StaticCudaLauncher`, whose stream parameter is a 32-bit C long.  CUDA
     graph capture always runs on a side stream, whose 64-bit handle exceeds
     2**31, so the two are mutually exclusive unless the launcher is off.
     Verified on a side stream, where the bug reproduces, and NOT on the
     default stream, where the handle is 0 and nothing fails.

  2. Is the result correct?  Yes, and NOT bit-identically. Against eager,
     worst relative cache difference is 2.3e-02 and worst relative logit
     difference over lockstep decode steps is 1.5e-02, both under the 0.05
     acceptance bound but not zero. `max-autotune-no-cudagraphs` changes the
     arithmetic, so this is a numerically-approximate optimization and not
     an allocation-only one. It cannot be combined with the bit-identical
     manual-graph-over-eager path without giving up bit-exactness.

What this file deliberately does NOT reuse is the timing loop from the first
fusion probe, which advanced three caches at three different rates and then
cross-compared them.  That produced a confident "real divergence" reading
which was an off-by-one step count in the probe's own bookkeeping.  Instead
the structure comes from tq_graph_abba_bench.py, which has already survived
equal-history, warmup and step-budget review, with only the callable
swapped for the compiled one.

Every cache in this file is built by `build(n_steps)`, which takes an
explicit step count and refuses to guess.  Three separate wrong numbers in
this investigation came from an untracked step count, and one of them was
published as a hardware finding before being caught.

Correctness is always measured against EAGER.  An earlier revision used
GraphHarness to compare the fused cache against the graph cache and reported
exactly 0.0, which was true and useless: both sides run the same fused
kernels, so they share the same drift from eager and cancel each other out.

Usage:

    TQWEN35_DIR=/path/to/qwen35-9b python tq_fusion_graph_abba_bench.py

Both contexts have now completed end to end.  A run that finished on
2026-10-02 reported:

    ctx   eager ms   fuse+CG    e/CG   worst d  credible
   1024     78.184    21.394   3.65x 8.709e-03  yes
   4096     68.799    23.794   2.89x 8.547e-03  yes
  acceptance gate: worst d must be < 0.05, measured 8.709e-03, 8.547e-03
  gate: PASS

So ctx-4096 correctness IS established: 8.547e-03 against eager, inside
the bound.  Its latency of 2.891x was taken on a loaded host (45% CPU, 9
python processes, 23,989 of 24,564 MiB VRAM in use) and is not a
quiet-host number; the clean-host figures remain 3.376x and 3.519x at
ctx-1024.  At ctx-4096 the graph did not add drift: fusion alone was
8.547e-03 and fusion+graph was lower at 7.874e-03.

Reaching 4096 cost about 2 hours 4 minutes of wall clock, nearly all of it
host max-autotune, so budget for it.  Running two copies of this harness
at once additionally exhausts the Windows page file inside Triton
heuristics, which is why claim_single_instance() exists.

The specific cost was located rather than inferred, from the run's own
log:

    AUTOTUNE mm(4096x4096, 4096x248320)
    SingleProcess AUTOTUNE benchmarking takes 771.0088 seconds

771 seconds for one shape, because 248,320 is the KV entry count that a
4096-token context produces on this model and the projection consuming it
is benchmarked across 16 Triton configs plus an eager `mm` baseline, each
timed on a real 4096x4096x248,320 GEMM.  Across that whole log, 267
autotune blocks over 7 distinct shapes totalled 842 seconds, so this single
block was 92% of all autotune time, and it appears exactly once.

Worth knowing before trying to suppress it: the settings below turn off
GEMM BACKEND SELECTION, not GEMM AUTOTUNING.  `max_autotune_gemm = False`
and a pinned `max_autotune_gemm_backends` skip choosing between ATEN and
Triton, but mode="max-autotune-no-cudagraphs" still coordinate-descent
tunes every ordinary matmul, so each mm shape is still benchmarked 16 ways
on the real GPU.  Lowering or removing that cost means changing what is
being measured, so it is left alone here.

Compiling Qwen3.5-9B takes 10 to 70 seconds depending on Inductor cache
state, so a cold run is not quick.  It happens once, not per context.
"""
import os
import statistics
import sys
import tempfile
import time

sys.stdout.reconfigure(encoding="utf-8")

# Keeps the single-instance lock file open for the process lifetime.  See
# claim_single_instance() for why the handle cannot be a local.
_LOCK_FH = None

import torch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

MODEL_DIR = os.environ.get(
    "TQWEN35_DIR", r"C:\Users\koosh\agents\sandbox\tq-eval\qwen35-9b")
ROUNDS = 7
WARMUP = 4
INNER = 8
CONTEXTS = tuple(int(c) for c in
                 os.environ.get("TQ_CONTEXTS", "1024,4096").split(","))
MAXLEN_PAD = 128
LOCK_CHECKS = 4
COMPILE_MODE = os.environ.get("TQ_COMPILE_MODE", "max-autotune-no-cudagraphs")
# The acceptance bound on worst relative logit difference against eager, as
# documented in tq_block_cache.py. A run outside it must not report success.
LOGIT_TOL = 0.05


def log(m):
    print(f"[{time.strftime('%H:%M:%S')}] {m}", flush=True)


def sync():
    torch.cuda.synchronize()


def maxlen_for(ctx):
    """Cache room for the context AND for every step this file takes.

    The eager side advances by WARMUP + LOCK_CHECKS + ROUNDS * 2 * INNER
    steps. Sizing for the context alone ran index_copy_ off the end of the
    buffer, which is a device-side assert rather than a clean failure.
    """
    budget = WARMUP + LOCK_CHECKS + 3 + ROUNDS * 2 * INNER + 8
    return ctx + budget + MAXLEN_PAD


def claim_single_instance(path=None):
    """Refuse to start if another copy of this benchmark is already running.

    Two concurrent copies exhausted the Windows page file during Inductor
    compilation at context 4096, which surfaced as
    `OSError: [WinError 1455] The paging file is too small` deep inside
    Triton heuristics rather than as an obvious "you started this twice".
    Compilation holds far more host memory than the GPU work does, so the
    pressure is not visible from GPU memory.

    This is an exclusive-create on a lock file, which the OS releases when
    the process exits, so a crashed run cannot leave the benchmark
    permanently unrunnable.  An earlier revision scanned WMI for other
    python.exe processes; it was discarded because `$PID` inside
    `powershell -Command` is the *PowerShell* PID, not the caller's, so the
    script matched itself and aborted every launch.

    `path` exists so `tq_lock_selftest.py` can prove the mutual exclusion
    without stealing the lock of a real run that may be in flight.  It
    defaults to the shared temp path that main() uses, overridable with
    TQ_LOCK_PATH.
    """
    import msvcrt
    if path is None:
        path = os.environ.get(
            "TQ_LOCK_PATH",
            os.path.join(tempfile.gettempdir(), "tq_fusion_abba.lock"))
    try:
        fh = open(path, "r+b")
    except FileNotFoundError:
        fh = open(path, "w+b")
    # msvcrt.locking locks the byte range starting at the current file
    # position, so the position must be pinned or two processes lock
    # different bytes and never contend.  "a+b" opens at EOF, which is why
    # the first version let a second instance sail straight through.
    fh.seek(0)
    try:
        msvcrt.locking(fh.fileno(), msvcrt.LK_NBLCK, 1)
    except OSError:
        log(f"ABORT: another copy of this benchmark holds {path}. "
            f"Concurrent copies exhaust the page file during Triton "
            f"compilation. Wait for it to finish before starting a new run.")
        fh.close()
        return False
    fh.seek(0)
    fh.truncate()
    fh.write(f"{os.getpid()}\n".encode())
    fh.flush()
    # The handle MUST outlive this function.  msvcrt locks are released when
    # the file is closed, and a local goes out of scope the moment we
    # return, which silently drops the lock and lets a second copy start.
    # An earlier version made exactly that mistake.
    global _LOCK_FH
    _LOCK_FH = fh
    log(f"single-instance lock acquired: {path}")
    return True


def main():
    if not os.path.isdir(MODEL_DIR):
        log(f"model directory not found: {MODEL_DIR}")
        return 2
    if not claim_single_instance():
        return 3
    from transformers import AutoConfig
    from transformers.models.qwen3_5 import Qwen3_5ForConditionalGeneration
    from transformers.utils import logging as hf_logging

    # Before any compiled call. Inductor reads this at codegen time.
    torch._inductor.config.use_static_cuda_launcher = False
    log("inductor.use_static_cuda_launcher = False "
        "(required: capture runs on a side stream, whose handle does not "
        "fit the 32-bit long in the static launcher)")

    hf_logging.set_verbosity_error()
    hf_logging.disable_progress_bar()
    # max-autotune prints thousands of lines of kernel benchmarking, which
    # buried every actual result line in the first run. Keep the log
    # readable so a real failure is visible in a tail.
    try:
        torch._inductor.config.max_autotune_gemm_backends = "ATEN,TRITON"
    except Exception:
        pass
    try:
        torch._inductor.config.max_autotune_gemm = False
    except Exception:
        pass
    cfg = AutoConfig.from_pretrained(MODEL_DIR, local_files_only=True)
    model = Qwen3_5ForConditionalGeneration.from_pretrained(
        MODEL_DIR, dtype=torch.bfloat16, device_map="cuda",
        local_files_only=True)
    model.eval()
    dev = model.device
    log(f"loaded on {dev}, torch {torch.__version__}")

    log(f"compiling with mode={COMPILE_MODE}, this is the slow part")
    t0 = time.perf_counter()
    compiled = torch.compile(model, mode=COMPILE_MODE, fullgraph=False)
    log(f"torch.compile object built in {time.perf_counter() - t0:.1f}s")

    results = []
    for ctx in CONTEXTS:
        log("")
        log(f"=== context {ctx} ===")
        r = run_ctx(model, compiled, cfg, dev, ctx)
        if r:
            results.append(r)

    log("")
    log("FUSION + MANUAL GRAPH, ABBA DECODE LATENCY, Qwen3.5-9B")
    log(f"{'ctx':>7} {'eager ms':>10} {'fuse+CG':>9} "
        f"{'e/CG':>7} {'spr E':>7} {'spr CG':>7} {'worst d':>9}  credible")
    for r in results:
        log(f"{r['ctx']:>7} {r['eager']:>10.3f} "
            f"{r['fused_cg']:>9.3f} "
            f"{r['ratio_eager_cg']:>6.2f}x "
            f"{r['spread_eager']:>7.3f} {r['spread_cg']:>7.3f} "
            f"{r['worst']:>9.3e}  "
            f"{'no' if r['overlap'] else 'yes'}")
    log("")
    log("worst d is the worst relative logit difference against EAGER, "
        "which is the only reference here that fusion is not "
        "indistinguishable from.")
    log("A credible speedup requires the two sample sets not to overlap.")

    # The 0.05 acceptance bound is documented in tq_block_cache.py but was
    # only ever printed, never enforced, so a run that exceeded it still
    # exited 0 and looked like a pass. Enforce it here: a result outside the
    # bound must not report success, however fast it was.
    within = [r for r in results if r["worst"] < LOGIT_TOL]
    measured = ", ".join("{:.3e}".format(r["worst"]) for r in results)
    # An aborted context (run_ctx returned None because it exceeded the
    # bound) is NOT a pass. It must not vanish from `results` and leave the
    # gate reporting PASS for whatever contexts did survive, or a run with
    # TQ_CONTEXTS=1024,4096 would exit 0 while 4096 was never measured at
    # all. Every requested context has to produce a result.
    requested = len(CONTEXTS)
    measured_ctxs = len(results)
    complete = measured_ctxs == requested
    log("")
    log(f"acceptance gate: worst d must be < {LOGIT_TOL}, measured "
        f"{measured or '(nothing measured)'}")
    for r in results:
        verdict = "within" if r["worst"] < LOGIT_TOL else "OUT OF BOUNDS"
        log(f"  ctx {r['ctx']}: {r['worst']:.3e} {verdict}")
    if not complete:
        log(f"  contexts requested {requested}, measured {measured_ctxs}: "
            f"an aborted or unmeasured context is NOT a pass")
    ok = bool(results) and complete and len(within) == len(results)
    log(f"gate: {'PASS' if ok else 'FAIL'}")
    return 0 if ok else 1


def rel_diff(a, b):
    """Worst relative difference, normalized by the reference magnitude.

    The reference is the EAGER logits. Normalizing by the reference's own
    magnitude is what makes the result a scale-free number comparable with
    the 0.05 acceptance bound used elsewhere in this directory.
    """
    a = a.float()
    b = b.float()
    scale = a.abs().max().clamp_min(1e-6)
    return float((a - b).abs().max() / scale)


def run_ctx(model, compiled, cfg, dev, ctx):
    """One context, measured as three variants under an A B B A schedule.

    Variants share the model and the compiled callable but each has its own
    cache, built with an explicit step count, because a cache advanced at a
    different rate than its reference invalidates the comparison in ways
    that read like numerical error.

    Two rules this function follows, both learned the hard way here:

    Fusion is never compared against fusion. `max-autotune-no-cudagraphs`
    is not bit-identical to eager on this model, so every comparison is
    against the eager cache. An earlier version compared the fused cache
    against the graph cache, which share the same numerical error and so
    reported a clean 0.0 while hiding a real 1.5e-2 drift from eager.
    """
    import transformers.cache_utils as cu

    maxlen = maxlen_for(ctx)
    gcpu = torch.Generator().manual_seed(101 + ctx)
    toks = torch.randint(1000, 50000, (1, ctx), device=dev)
    pos0 = torch.arange(ctx, device=dev)
    test = torch.randint(1000, 50000, (1, 1), generator=gcpu).to(dev)

    def build(callable_, n_steps):
        """A cache prefilled at ctx, then advanced n_steps times.

        n_steps is explicit and never inferred. Every variant uses the same
        ascending position sequence ctx, ctx+1, ..., so the histories are
        identical by construction.

        An earlier version took a capture_pos argument that pinned the warmup
        steps to one position. That silently produced a different history
        from eager, and assert_histories_match correctly refused it with a
        0.75 worst difference. Pinning positions is not a capture
        requirement: capture only needs the position buffer to be a stable
        address, which p_buf already provides.
        """
        c = cu.StaticCache(config=cfg, max_cache_len=maxlen)
        with torch.no_grad():
            callable_(toks, past_key_values=c, use_cache=True,
                      cache_position=pos0)
        for i in range(n_steps):
            p = torch.tensor([ctx + i], device=dev)
            with torch.no_grad():
                callable_(test.view(1, 1), past_key_values=c, use_cache=True,
                          cache_position=p)
        return c

    def step(callable_, cache, pos, token=None):
        tok = test.view(1, 1) if token is None else token.view(1, 1)
        with torch.no_grad():
            return callable_(tok, past_key_values=cache, use_cache=True,
                             cache_position=pos).logits[:, -1].float().clone()

    # Correctness is measured against EAGER, in lockstep, with a separate
    # cache per variant. The eager cache is the reference for both the
    # fused cache and the graph cache.
    cap_pos = ctx + WARMUP
    ref = build(model, WARMUP)
    fused = build(compiled, WARMUP)
    log(f"  eager and fused caches built at {WARMUP} steps; fused is NOT "
        f"bit-identical to eager, which is expected and is measured below")

    p_buf = torch.tensor([cap_pos], device=dev)
    inp = torch.zeros(1, 1, dtype=torch.long, device=dev)
    inp.copy_(test)

    # Step the eager and fused caches forward together and compare logits.
    # Each keeps its own cache so neither write contaminates the other.
    worst_fuse = 0.0
    for k in range(LOCK_CHECKS):
        i = WARMUP + k
        pos = torch.tensor([ctx + i], device=dev)
        a = step(model, ref, pos)
        b = step(compiled, fused, pos)
        worst_fuse = max(worst_fuse, rel_diff(a, b))
    log(f"  fusion vs eager: worst relative logit diff over "
        f"{LOCK_CHECKS} lockstep steps = {worst_fuse:.3e}")

    # Capture on a third cache, primed one step ahead exactly as capture
    # requires, with the eager reference advanced to match so the two stay
    # on the same history.
    prime = torch.tensor([ctx + WARMUP + LOCK_CHECKS], device=dev)
    p_buf.copy_(prime)
    gcache = build(compiled, WARMUP + LOCK_CHECKS)
    s = torch.cuda.Stream()
    s.wait_stream(torch.cuda.current_stream())
    with torch.cuda.stream(s):
        with torch.no_grad():
            compiled(inp, past_key_values=gcache, use_cache=True,
                     cache_position=p_buf)
    torch.cuda.current_stream().wait_stream(s)
    sync()
    step(model, ref, prime)

    g = torch.cuda.CUDAGraph()
    with torch.no_grad():
        with torch.cuda.graph(g):
            out = compiled(inp, past_key_values=gcache, use_cache=True,
                           cache_position=p_buf)
    sync()

    worst_cg = 0.0
    for k in range(LOCK_CHECKS):
        p_buf.fill_(int(prime) + 1 + k)
        a = step(model, ref, p_buf)
        g.replay()
        sync()
        b = out.logits[:, -1].float().clone()
        worst_cg = max(worst_cg, rel_diff(a, b))
    log(f"  fusion+graph vs eager: worst relative logit diff over "
        f"{LOCK_CHECKS} lockstep steps = {worst_cg:.3e}")

    worst = max(worst_fuse, worst_cg)
    # Uses LOGIT_TOL rather than a second literal 0.05. This check and the
    # main() acceptance gate have to agree by construction: if the tolerance
    # were ever changed in one place only, this would abort at a different
    # value than the one reported, and the mismatch would be silent.
    if worst >= LOGIT_TOL:
        log(f"  ABORT: at or above the {LOGIT_TOL} bound, "
            f"refusing to publish a speedup")
        del g, out, ref, fused, gcache
        torch.cuda.empty_cache()
        return None

    # Free the correctness-phase caches BEFORE building the timing ones. The
    # first version of this file held ref, fused, t_eager and t_fuse alive at
    # the same time, four StaticCaches at maxlen ~1170, and the process died
    # silently during the timing prefill with no traceback and no CUDA OOM
    # message. On a 24 GB card that is a plain host OOM kill, and it looked
    # exactly like a hang. Two caches at a time is what actually fits.
    del fused, ref
    torch.cuda.empty_cache()
    torch.cuda.synchronize()
    free, total = torch.cuda.mem_get_info()
    log(f"  correctness phase done, freed its caches; "
        f"{free / 2**30:.1f} GiB free of {total / 2**30:.1f} GiB")

    # The graph cache is promoted to be the timing cache. It has already
    # been captured and its history matches eager, so rebuilding it would
    # only cost another 10-minute prefill.
    n_timing = WARMUP + LOCK_CHECKS + 1 + ROUNDS * 2 * INNER
    t_eager = build(model, n_timing)
    log(f"  eager timing cache built at {n_timing} steps; graph cache "
        f"reused as-is from capture")

    pos = p_buf

    def t_eager_run(inner):
        sync()
        t0 = time.perf_counter()
        for _ in range(inner):
            step(model, t_eager, pos)
        sync()
        return (time.perf_counter() - t0) * 1e3 / inner

    def t_cg_run(inner):
        sync()
        t0 = time.perf_counter()
        for _ in range(inner):
            g.replay()
        sync()
        return (time.perf_counter() - t0) * 1e3 / inner

    # Discard the first pass of each: the allocator settles and the first
    # timed launch after capture is not representative.
    t_eager_run(INNER)
    t_cg_run(INNER)

    E, C = [], []
    for _ in range(ROUNDS):
        E.append(t_eager_run(INNER))
        C.append(t_cg_run(INNER))
        C.append(t_cg_run(INNER))
        E.append(t_eager_run(INNER))

    me = statistics.median(E)
    mc = statistics.median(C)
    se = (max(E) - min(E)) / me
    sc = (max(C) - min(C)) / mc
    overlap = not (max(E) < min(C) or max(C) < min(E))
    log("")
    log(f"  eager               {me:8.3f} ms  spread {se:.3f}")
    log(f"  fusion + graph      {mc:8.3f} ms  spread {sc:.3f}")
    log(f"  eager / fusion+graph = {me / mc:.3f}x "
        f"[{'OVERLAPPING, not credible' if overlap else 'clean'}]")
    log("  fusion-only latency is not timed separately: it shares the "
        "graph's numerical drift from eager, and the graph already removes "
        "the launch overhead that fusion cannot.")

    del out, g, t_eager, gcache
    torch.cuda.empty_cache()
    return {"ctx": ctx, "eager": me, "fused_cg": mc,
            "ratio_eager_cg": me / mc, "spread_eager": se, "spread_cg": sc,
            "overlap": overlap, "worst": worst}


if __name__ == "__main__":
    sys.exit(main())
