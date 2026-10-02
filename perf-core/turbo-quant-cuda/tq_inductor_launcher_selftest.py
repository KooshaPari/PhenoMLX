"""Reproduce and diagnose the Inductor static-launcher OverflowError.

`torch.compile(mode="reduce-overhead")` fails on this Windows build with

    OverflowError: Python int too large to convert to C long
    at torch/_inductor/runtime/static_cuda_launcher.py:244

and that line forwards a CUDA stream HANDLE into a parameter pybind11
declares as a C `long`. A C long is 32 bits on Windows, and this platform's
is 4 bytes, which this file prints. A stream handle is 64-bit and in a
capture it exceeds 2**31, so pybind11 refuses the conversion before the
kernel is ever launched. The kernel function handle is oversized too, but it
lands in a `uint64_t` parameter, so it is a hazard here rather than the
proved cause; the stream is the proved one.

THE PART THAT MISLEADS: the default stream is 0, and 0 converts cleanly, so
probing the default stream makes the stream look innocent. A cudagraph
capture runs on a side stream, so the failing call is never on stream 0.
Measuring the wrong stream is what turned this from a width bug into an
open question for several sessions.

Run it:

    TQWEN35_DIR=... python tq_inductor_launcher_selftest.py

It takes about 40 seconds and needs no model, which is the point: the bug
was previously only ever observed on the full Qwen3.5-9B after minutes of
compiling, so it could not be iterated on.
"""
import os
import sys

sys.stdout.reconfigure(encoding="utf-8")

import torch

CACHE = os.environ.get("TQ_TORCHINDUCTOR_CACHE",
                       os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                    ".tq_inductor_cache"))
os.environ["TORCHINDUCTOR_CACHE_DIR"] = CACHE
os.makedirs(CACHE, exist_ok=True)

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tq_lockstep_guard import rel  # noqa: E402

LIMIT = 2 ** 31


class Tiny(torch.nn.Module):
    """Three matmuls: enough for Inductor to emit kernels, small enough to
    compile in seconds."""

    def __init__(self):
        super().__init__()
        self.a = torch.nn.Linear(256, 512)
        self.b = torch.nn.Linear(512, 512)
        self.c = torch.nn.Linear(512, 256)

    def forward(self, x):
        return self.c(torch.relu(self.b(torch.relu(self.a(x)))))


class Probe(Tiny):
    """A different shape, so this forces a compile instead of a cache hit.

    Step 1 already compiled Tiny with a 64x256 input. Re-compiling the
    same graph hits a warm cache and never enters the static launcher, so
    the instrumentation below would report nothing at all and the
    diagnosis would be unevidenced.
    """

    def __init__(self):
        super().__init__()
        self.a = torch.nn.Linear(192, 384)
        self.b = torch.nn.Linear(384, 384)
        self.c = torch.nn.Linear(384, 192)

    def forward(self, x):
        return self.c(torch.relu(self.b(torch.relu(self.a(x)))))


def attempt(label, kwargs, x, dev, rounds=6):
    torch.manual_seed(0)
    m = Tiny().to(dev).eval()
    with torch.no_grad():
        eager = m(x)
    try:
        fn = torch.compile(m, **kwargs)
        with torch.no_grad():
            got = fn(x)
            torch.cuda.synchronize()
            for _ in range(rounds):
                got = fn(x)
            torch.cuda.synchronize()
        d = rel(got, eager)
        print(f"  {label:<34} OK    relative diff {d:.3e}")
        return ("ok", d)
    except Exception as e:
        msg = str(e).splitlines()[0][:64]
        print(f"  {label:<34} FAIL  {type(e).__name__}: {msg}")
        return ("fail", type(e).__name__)


def capture_failing_call():
    """Record what the launcher is actually handed on the failing call.

    The binding cannot be wrapped, torch._C._StaticCudaLauncher is an
    immutable extension type, so the Python method that forwards into it is
    wrapped instead. Nothing is launched, so this is safe to run.
    """
    import torch._inductor.runtime.static_cuda_launcher as scl
    got = []

    class Sentinel(Exception):
        pass

    real = scl.StaticallyLaunchedCudaKernel.run

    def spy(self, gx, gy, gz, stream, *args):
        got.append({"stream": stream, "args": args,
                    "num_warps": self.num_warps, "shared": self.shared,
                    "arg_tys": self.arg_tys, "function": self.function})
        raise Sentinel("captured")

    scl.StaticallyLaunchedCudaKernel.run = spy
    return real, Sentinel, got


def main():
    dev = torch.device("cuda")
    x = torch.randn(64, 256, device=dev)
    print(f"torch {torch.__version__}  ctypes long is "
          f"{__import__('ctypes').sizeof(__import__('ctypes').c_long)} bytes "
          f"on this platform")
    results = {}

    # 1. Reproduce. This must FAIL or the whole diagnosis is untested.
    print("\n1. does the failure reproduce here, with no model loaded")
    results["repro"] = attempt("mode=reduce-overhead",
                               {"mode": "reduce-overhead"}, x, dev)
    if results["repro"][0] != "fail":
        print("   did NOT reproduce. The bug is version or environment "
              "specific and the rest of this file is not evidence for it.")
        return 1

    # 2. The mode is the trigger, not the model. A three-layer MLP is
    #    nothing like a 9B decode step, so this rules size out.
    print("\n2. mode is the trigger, model size is irrelevant")
    results["default"] = attempt("mode=default", {"mode": "default"}, x, dev)
    results["no_cg"] = attempt("mode=max-autotune-no-cudagraphs",
                               {"mode": "max-autotune-no-cudagraphs"}, x, dev)
    results["autotune"] = attempt("mode=max-autotune",
                                  {"mode": "max-autotune"}, x, dev)

    # 3. The oversized values themselves.
    #    A distinct shape is essential here: step 1 already compiled this
    #    model, and a warm Inductor cache skips the launcher entirely, so
    #    reusing the shape silently reports "no call captured".
    print("\n3. what the launcher is actually handed")
    real, Sentinel, got = capture_failing_call()
    torch.manual_seed(0)
    m = Probe().to(dev).eval()
    fn = torch.compile(m, mode="reduce-overhead")
    try:
        with torch.no_grad():
            fn(torch.randn(64, 192, device=dev))
    except Sentinel:
        pass
    except Exception:
        pass
    finally:
        import torch._inductor.runtime.static_cuda_launcher as scl
        scl.StaticallyLaunchedCudaKernel.run = real
    if not got:
        print("   no call captured, the cache was warm")
    else:
        c = got[0]
        for k in ("grid_x", "stream", "num_warps", "shared", "arg_tys",
                  "function"):
            v = c.get(k)
            mark = ""
            if isinstance(v, int) and v >= LIMIT:
                mark = "   <-- exceeds a 32-bit C long"
            shown = f"{v:#x} ({v})" if isinstance(v, int) else repr(v)
            print(f"   {k:<10} {shown}{mark}")

    print("\nVERDICT")
    ok_no_cg = results["no_cg"][0] == "ok"
    ok_default = results["default"][0] == "ok"
    print(f"   default mode works                  : {ok_default}")
    print(f"   reduce-overhead works               : "
          f"{results['repro'][0] == 'ok'}")
    print(f"   max-autotune-no-cudagraphs works    : {ok_no_cg}")
    if ok_no_cg and results["repro"][0] == "fail":
        print("\n   The bug blocks Inductor's own cudagraph integration and "
              "nothing else.\n   `max-autotune-no-cudagraphs` gives full "
              "Inductor fusion with the static\n   launcher out of the path, "
              "and manual torch.cuda.graph capture supplies the\n   launch "
              "overhead elimination that reduce-overhead would have.")
        print("\n   That combination has since been measured on "
              "Qwen3.5-9B by\n   tq_fusion_graph_abba_bench.py: at "
              "context 1024 it beat eager 3.376x, but it\n   is NOT "
              "bit-identical (worst relative logit difference\n   about "
              "1.5e-02 against eager, under the 0.05 gate) and it\n   still "
              "lands 2.7x above the 7.72 ms weight-read floor. See\n   "
              "tq_block_cache.py under FUSION PLUS GRAPH for the numbers\n   "
              "and the caveats.")
    n_fail = sum(1 for v in results.values() if v[0] == "fail") - 1
    print(f"\n   expected failures: 1 (reduce-overhead), observed: {n_fail}")
    return 0 if (n_fail == 1 and ok_no_cg) else 1


if __name__ == "__main__":
    sys.exit(main())
