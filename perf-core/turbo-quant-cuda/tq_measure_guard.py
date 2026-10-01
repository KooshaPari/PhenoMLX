"""Gate decode timings on torch's own allocator, not on the driver's free list.

The first version of this guard asked the driver how much memory was free via
torch.cuda.mem_get_info, and required headroom before letting a timing report.
That was wrong on this machine, and measurably so. Here is the measurement
that killed it:

  before load   driver free 22.77 GiB, torch reserved  0.00 GiB
  after load    driver free  0.00 GiB, torch reserved 17.54 GiB
  after prefill driver free  0.00 GiB, torch reserved 18.48 GiB

The driver reports a saturated card the moment a 17.54 GiB model is resident,
even though torch has 5.5 GiB of headroom left on a 23.99 GiB card. Windows
with a display attached runs WDDM, and under WDDM the driver reserves a large
physical arena that it counts as used regardless of what any process allocates.
So "free memory reads 0.00 GiB" says nothing about whether a decode
measurement is safe, and gating on it would either refuse every run or, worse,
be the reason an earlier run looked contaminated when it was fine.

The number that does mean something is torch's own reserved total, because that
is what this process will actually have to fit inside, plus a margin for
allocator fragmentation and the transient prefill peak.

What this guard is for, restated honestly. It cannot detect a competing process
by memory alone on WDDM, because the driver will not say. It can still do two
useful things:

  - refuse to report when THIS process has grown past a fraction of the card,
    which is the condition that actually invalidated the 16K decode numbers;
  - report other CUDA processes by PID so a human or a caller can see them,
    even though the driver's memory attribution is useless here.

And it should be explicit that a clean guard does not certify a timing. It only
certifies that memory was not the obvious cause. Wall-clock on this card still
moves with WDDM, with clock behaviour, and with whatever else the desktop is
doing, which is why every citable number in this project is an ABBA median with
a spread gate rather than a single timing.
"""
import os
import subprocess
import sys

GIB = 2 ** 30


def other_cuda_procs():
    """PIDs with a substantial CUDA footprint, this process excluded.

    WDDM makes the memory column useless (it reports N/A for nearly every
    process), so this is best-effort context for a human reading the log and
    is deliberately not used to fail the gate.
    """
    mine = str(os.getpid())
    try:
        out = subprocess.run(
            ["nvidia-smi", "--query-compute-apps=pid,used_memory",
             "--format=csv,noheader,nounits"],
            capture_output=True, text=True, timeout=30).stdout
    except Exception:
        return None
    pids = set()
    for line in out.splitlines():
        parts = [p.strip() for p in line.split(",")]
        if len(parts) >= 1 and parts[0].isdigit() and parts[0] != mine:
            try:
                mib = int(parts[1])
            except (IndexError, ValueError):
                mib = 0
            if mib > 512:
                pids.add(parts[0])
    return pids


def check(headroom_gib, what="this measurement", limit_gib=None):
    """Decide whether a timing may be reported, from torch's own counters.

    headroom_gib   slack the caller wants below this process's reservation
    limit_gib      optional hard ceiling; defaults to the visible card size

    Returns (ok, info). Callers must not report a timing when ok is False.
    """
    import torch
    free_b, total_b = torch.cuda.mem_get_info()
    card_gib = total_b / GIB
    limit = card_gib if limit_gib is None else limit_gib
    pids = other_cuda_procs()
    pid_note = "n/a" if pids is None else (
        "none" if not pids else ",".join(sorted(pids, key=int)))

    used = torch.cuda.memory_reserved() / GIB
    alloc = torch.cuda.memory_allocated() / GIB
    slack = limit - used

    print(f"[gpu-guard] torch reserved {used:.2f} GiB "
          f"(allocated {alloc:.2f}) of {limit:.2f} GiB limit; "
          f"slack {slack:.2f} GiB; want {headroom_gib:.2f} GiB")
    print(f"[gpu-guard] driver free {free_b / GIB:.2f} GiB "
          f"(WDDM arena, not a reliable signal); other CUDA procs: {pid_note}")

    ok = slack >= headroom_gib
    if not ok:
        print(f"[gpu-guard] REFUSING to report {what}: slack is {slack:.2f} GiB, "
              f"below the {headroom_gib:.2f} GiB wanted.")
        print("[gpu-guard] Under allocator pressure every allocation can become "
              "a cudaMalloc plus a sync, which inflates host timing and makes "
              "a decode number describe the allocator rather than the model.")
    return ok, {"torch_reserved": used, "torch_allocated": alloc,
                "slack": slack, "limit": limit, "pids": pid_note}


def require_headroom(headroom_gib, what="this measurement", limit_gib=None):
    """Read torch's reserved total and gate on it."""
    return check(headroom_gib, what, limit_gib)


if __name__ == "__main__":
    import torch
    torch.zeros(1, device="cuda")
    print(__doc__)
    need = float(sys.argv[1]) if len(sys.argv) > 1 else 2.0
    ok, info = require_headroom(need, "demo")
    raise SystemExit(0 if ok else 2)