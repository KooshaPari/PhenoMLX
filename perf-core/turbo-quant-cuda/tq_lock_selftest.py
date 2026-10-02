"""Prove the fusion benchmark's single-instance lock actually works.

Two concurrent copies of tq_fusion_graph_abba_bench.py are what exhausted
the Windows page file inside Triton heuristics at context 4096, surfacing as
`OSError: [WinError 1455]` with nothing to say "you started this twice".
The lock exists to prevent that, and a lock that does not lock is worse than
no lock at all, because it looks like protection.

Every version of this lock that failed did so silently, and each was caught by
running it rather than by reading it:

  1. A WMI scan for other python.exe processes.  It compared against `$PID`
     inside `powershell -Command`, but that is the PowerShell shell's own
     PID, not the caller's, so the scan always matched itself and aborted
     every launch.  Replaced with an OS file lock, which needs no PID
     handoff across a process boundary.

  2. `open(path, "a+b")`.  msvcrt.locking locks the byte range starting at
     the CURRENT file position, and "a" positions at EOF.  The holder locked
     byte 0 of an empty file; a later process positioned at EOF and locked a
     different byte, so the two never contended and a second instance sailed
     straight through.  Fixed by seeking to 0 before locking.

  3. A function-local file handle.  msvcrt releases its locks when the file is
     closed, and a local goes out of scope the moment the function returns,
     which silently dropped the lock immediately after acquiring it.  Fixed by
     parking the handle in a module global.

This file checks all three properties, in subprocesses, because none of them
are observable within a single process:

  - a second live holder is refused,
  - the lock is released when the holder exits, so a crashed run does not
    leave the benchmark permanently unrunnable,
  - the recorded PID in the lock file matches the holder that owns it.

It uses its OWN lock path for the subprocess checks, never the benchmark's,
so it cannot steal the lock of a real measurement that is in flight.  It needs
no GPU and no model, and takes about fifteen seconds.

    python tq_lock_selftest.py
"""
import os
import subprocess
import sys
import tempfile

sys.stdout.reconfigure(encoding="utf-8")

HERE = os.path.dirname(os.path.abspath(__file__))
LOCK = os.path.join(tempfile.gettempdir(), "tq_lock_selftest.lock")
BENCH = os.path.join(HERE, "tq_fusion_graph_abba_bench.py")
BENCH_LOCK = os.path.join(tempfile.gettempdir(), "tq_fusion_abba.lock")

# Import by path rather than importing the benchmark normally: the benchmark
# module pulls in torch at import time, which is slow but harmless here, and
# this keeps the test runnable even if the benchmark's own imports are broken.
CHILD = """
import sys
sys.path.insert(0, {here!r})
import tq_fusion_graph_abba_bench as b
acquired = b.claim_single_instance({lock!r})
print("ACQUIRED" if acquired else "REFUSED", flush=True)
if acquired:
    import time
    time.sleep({hold:d})
""".strip()


def run_child(hold, tag):
    """Start one child holding (or attempting) the lock, return its handle.

    The child's claim_single_instance() logs before printing, and log() stamps
    a timestamp, so the marker is matched as a substring of a line rather than
    by exact equality.
    """
    src = CHILD.format(here=HERE, lock=LOCK, hold=hold)
    p = subprocess.Popen([sys.executable, "-c", src],
                         stdout=subprocess.PIPE, text=True)
    verdict = ""
    for _ in range(6):
        ln = p.stdout.readline().strip()
        if not ln:
            break
        if "ACQUIRED" in ln or "REFUSED" in ln:
            verdict = "ACQUIRED" if "ACQUIRED" in ln else "REFUSED"
            break
    return p, verdict, tag


# An INDEPENDENT contender, deliberately not calling claim_single_instance().
#
# This exists because a self-test whose contender is the code under test
# cannot detect a wrong-OFFSET defect. Both processes then compute the same
# wrong byte and stay symmetric, so they contend perfectly and the test goes
# green on a broken lock. Measured: with `open(path, "a+b")` on a 10-byte
# file, both sides position at byte 10 and both acquire, and a third party
# using read+seek(0) on byte 0 also acquires alongside the incumbent. Only
# a contender that locks a KNOWN byte can see the difference.
CHALLENGER = """
import msvcrt, sys, time
path = sys.argv[1]
fh = open(path, "r+b")
fh.seek(0)                      # the one byte every correct implementation uses
try:
    msvcrt.locking(fh.fileno(), msvcrt.LK_NBLCK, 1)
    print("ACQUIRED", flush=True)
except OSError:
    print("REFUSED", flush=True)
time.sleep(4)
""".strip()


def run_independent_challenger(path):
    """Attempt the lock from code that does not use claim_single_instance.

    Returns True if the contender was correctly refused.
    """
    src = CHALLENGER.replace("sys.argv[1]", repr(path))
    p = subprocess.Popen([sys.executable, "-c", src],
                         stdout=subprocess.PIPE, text=True)
    verdict = ""
    for _ in range(4):
        ln = p.stdout.readline().strip()
        if "ACQUIRED" in ln or "REFUSED" in ln:
            verdict = "ACQUIRED" if "ACQUIRED" in ln else "REFUSED"
            break
    p.wait()
    return verdict == "REFUSED", verdict


def check_contention_on_a_populated_lock_file():
    """Require contention when the lock file already has content in it.

    This is the check that distinguishes a real lock from an accidental one.
    msvcrt.locking contends only on the exact byte range being locked, and
    `open(path, "a+b")` positions at EOF. On a fresh, empty lock file EOF is
    byte 0, so append mode and an explicit seek(0) coincidentally agree and
    the lock works by accident. The moment the file holds anything -- and it
    always does, because the holder writes its PID into it -- EOF moves, and
    a challenger positioning at EOF locks a byte nobody holds, so both
    processes acquire and neither is protected.

    Measured directly: two children locking byte 0 of a 10-byte file give
    ACQUIRED then REFUSED, while byte 0 versus byte 10 (EOF) gives ACQUIRED
    twice. That difference is the whole bug, and testing only against a
    deleted lock file cannot see it.

    So this case deliberately does NOT delete the lock file first.
    """
    if os.path.exists(LOCK):
        os.remove(LOCK)
    # Populate it the way the benchmark itself does, with a PID in it.
    with open(LOCK, "wb") as fh:
        fh.write(b"999999\n")
    size = os.path.getsize(LOCK)
    print(f"  lock file pre-populated to {size} bytes so EOF != 0")

    holder, holder_line, _ = run_child(30, "holder-populated")
    acquired = "ACQUIRED" in holder_line
    # Run the same-code contender FIRST and wait for it, then the independent
    # one. The independent contender sleeps only a few seconds, so if it ran
    # first it could let the holder exit before the second contender ever
    # started, and that contender would acquire against a free lock and look
    # like a failure of the lock rather than of the ordering.
    second, second_line, _ = run_child(0, "second-populated")
    second.wait()
    refused = "REFUSED" in second_line
    # Independent contender: locks byte 0 explicitly instead of going
    # through claim_single_instance(). If the holder locked some other byte,
    # such as EOF, this acquires alongside it and the lock is proven broken.
    ind_refused, ind_line = run_independent_challenger(LOCK)
    holder.kill()
    holder.wait()
    if os.path.exists(LOCK):
        os.remove(LOCK)
    print(f"  contention on a populated lock file: "
          f"holder={holder_line!r} same-code second={second_line!r} "
          f"independent={ind_line!r} -> "
          f"{'ok' if (acquired and refused and ind_refused) else 'BROKEN'}")
    return acquired and refused and ind_refused


def check_real_benchmark_refuses():
    """Run the real benchmark with the real lock held and require refusal.

    The subprocess checks above prove claim_single_instance() behaves. This
    proves main() actually calls it before doing any work, which is the part
    that would regress silently: if the guard were ever moved after model
    load, a second copy would still pass every unit-level check while
    exhausting the page file again.

    main() claims the lock and returns 3 before it imports transformers, so
    this needs no GPU and no weights. TQWEN35_DIR is pointed at an existing
    directory because main() checks the model path first and returns 2 if it
    is absent, which would mask the guard.
    """
    if os.path.exists(BENCH_LOCK):
        os.remove(BENCH_LOCK)
    holder_src = (
        "import sys;sys.path.insert(0,%r);"
        "import tq_fusion_graph_abba_bench as b;"
        "assert b.claim_single_instance(%r) is True;"
        "print('HELD',flush=True);"
        "import time;time.sleep(40)" % (HERE, BENCH_LOCK)
    )
    h = subprocess.Popen([sys.executable, "-c", holder_src],
                         stdout=subprocess.PIPE, text=True, cwd=HERE)
    for _ in range(8):
        ln = h.stdout.readline().strip()
        if "HELD" in ln or "lock acquired" in ln:
            break
    try:
        env = dict(os.environ, TQ_LOCK_PATH=BENCH_LOCK,
                   TQWEN35_DIR=tempfile.gettempdir())
        r = subprocess.run([sys.executable, "-u", BENCH],
                           capture_output=True, text=True, env=env,
                           cwd=HERE, timeout=600)
    finally:
        h.kill()
        h.wait()
        if os.path.exists(BENCH_LOCK):
            os.remove(BENCH_LOCK)
    refused = (r.returncode == 3) and ("ABORT: another copy" in r.stdout)
    print(f"  real benchmark refused while locked: {refused} "
          f"(exit {r.returncode}, 3 == refused)")
    return refused


def main():
    if os.path.exists(LOCK):
        os.remove(LOCK)
    print(f"selftest lock path: {LOCK}")

    # Hold the lock in a live child.
    holder, first, _ = run_child(6, "holder")
    if "ACQUIRED" not in first:
        print(f"FAIL: could not acquire a free lock, got {first!r}")
        return 1
    print(f"  holder acquired the lock on a free path: {first!r}")

    # A second child must be refused while the first is alive. This is the
    # property that does not survive a naive implementation.
    second, second_line, _ = run_child(0, "second")
    second.wait()
    refused = "REFUSED" in second_line
    print(f"  second live instance refused: {refused} ({second_line!r})")

    holder.wait()

    # Only after the holder exits. While it is alive it holds a deny-write
    # lock over byte 0, and on Windows that makes even a plain read of the
    # lock file raise PermissionError, so the recorded owner has to be read
    # once the lock is free.
    with open(LOCK, "r", encoding="utf-8", errors="replace") as fh:
        recorded = fh.read().strip()
    recorded_ok = recorded == str(holder.pid)
    print(f"  lock file records pid {recorded!r}, holder was {holder.pid}: "
          f"{recorded_ok}")

    # After the holder exits the lock must be free again, or a crashed run
    # would make the benchmark permanently unrunnable.
    third, third_line, _ = run_child(0, "third")
    third.wait()
    released = "ACQUIRED" in third_line
    print(f"  lock released after holder exit: {released} ({third_line!r})")

    if os.path.exists(LOCK):
        os.remove(LOCK)

    # The offset defect is only visible once the lock file has content, so
    # this must run against a populated file rather than a fresh one.
    populated_ok = check_contention_on_a_populated_lock_file()

    # The final and most important check: the real benchmark entry point.
    real_ok = check_real_benchmark_refuses()

    ok = refused and released and recorded_ok and populated_ok and real_ok
    print("OK" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    # sys.exit(main()) alone would still report success on an uncaught
    # exception path in some shells, so the verdict is echoed on the last
    # line and can be grepped.
    sys.exit(main())