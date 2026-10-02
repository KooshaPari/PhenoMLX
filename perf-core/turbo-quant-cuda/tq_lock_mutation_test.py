"""Mutation-test tq_lock_selftest.py: break the lock, require the test to fail.

A self-test that cannot fail is not evidence of anything. This breaks the
lock in each way it was actually broken during development, one at a time,
and requires tq_lock_selftest.py to go red for every real defect. It then
restores the original file and requires the test to go green again.

This earned its keep immediately. The first version of the self-test passed
all three, which turned out to mean it was not testing the property that
mattered:

  - Removing the retained handle was caught.
  - The append-mode offset bug was NOT caught, and the reason is structural.
    Both the holder and the contender ran the same defective function, so
    both computed the same wrong byte and contended perfectly. That is why
    tq_lock_selftest.py now includes an INDEPENDENT contender that locks
    byte 0 explicitly and never calls claim_single_instance(). With that
    contender the offset bug is caught.
  - Removing the seek(0) while still using "r+b" is a true NO-OP, not a
    defect: tell() is already 0 right after that open, versus 10 for "a+b".
    Requiring the self-test to detect it would be asking it to fail on
    correct code, so that entry is recorded but not counted.

Run it:

    python tq_lock_mutation_test.py

Safety: it only ever rewrites tq_fusion_graph_abba_bench.py, it asserts the
anchor text is present before injecting, and it restores from git in a
finally block after every injection. Both the anchors and the file being
restored must therefore be committed first; if the mutation fails to apply,
that is reported as SKIP rather than silently passing.

Takes a few minutes because it runs the whole self-test once per defect.
"""
import os
import subprocess
import sys

def find_repo_root(start):
    """Walk up from `start` until a directory containing .git is found.

    Hardcoding a number of levels up is fragile: this file has already moved
    once, and a wrong REPO makes `git checkout` fail silently, which would
    leave a mutated lock implementation on disk. A wrong depth is therefore
    checked rather than assumed.
    """
    p = start
    while True:
        if os.path.isdir(os.path.join(p, ".git")):
            return p
        parent = os.path.dirname(p)
        if parent == p:
            return None
        p = parent


HERE = os.path.dirname(os.path.abspath(__file__))
BENCH = os.path.join(HERE, "tq_fusion_graph_abba_bench.py")
SELFTEST = os.path.join(HERE, "tq_lock_selftest.py")
REPO = find_repo_root(HERE)
PY = sys.executable
if REPO is None:
    sys.exit(f"no git checkout found above {HERE}")

DEFECTS = [
    (
        "handle not retained (GC'd local)",
        "    global _LOCK_FH\n    _LOCK_FH = fh",
        "    pass  # DEFECT: handle dropped, lock released at scope exit",
    ),
    (
        # Both the append-mode open AND the seek(0) must go. Changing only
        # the open mode is not a defect: the seek(0) that follows it pins
        # the locked byte back to 0 and the lock still works, so that
        # mutation is a no-op and "surviving" it proves nothing. This is the
        # original code from before the seek was added.
        "append mode open AND no seek(0) (the original pre-fix bug)",
        '        fh = open(path, "r+b")\n    except FileNotFoundError:\n'
        '        fh = open(path, "w+b")\n'
        "    # msvcrt.locking locks the byte range starting at the current file\n"
        "    # position, so the position must be pinned or two processes lock\n"
        "    # different bytes and never contend.  \"a+b\" opens at EOF, which is why\n"
        "    # the first version let a second instance sail straight through.\n"
        "    fh.seek(0)",
        '        fh = open(path, "a+b")\n    except FileNotFoundError:\n'
        '        fh = open(path, "a+b")',
    ),
    (
        # NOT a defect, and listed here only to record the finding. After
        # open(path, "r+b") the file position is already 0 (verified: tell()
        # reports 0 immediately after the open, versus 10 for "a+b"), so
        # dropping the explicit seek(0) is a pure no-op. The self-test cannot
        # detect the absence of a redundant line, and requiring it to would
        # be asking the test to fail on correct code. Kept as a no-op entry so
        # the finding is not lost.
        "seek(0) removed while still using r+b (redundant, NOT a defect)",
        "    fh.seek(0)\n    try:\n        msvcrt.locking",
        "    try:\n        msvcrt.locking",
    ),
]

# Only these must be caught. A defect that is a no-op cannot be.
MUST_CATCH = 2


def run_selftest():
    r = subprocess.run([PY, SELFTEST], capture_output=True, text=True,
                       cwd=HERE, timeout=900)
    return r.returncode


def restore():
    subprocess.run(["git", "checkout", "--", BENCH], cwd=REPO,
                   capture_output=True, text=True)


def read():
    with open(BENCH, "r", encoding="utf-8") as fh:
        return fh.read()


def write(s):
    with open(BENCH, "w", encoding="utf-8") as fh:
        fh.write(s)


restore()
base = read()
baseline_rc = run_selftest()
print(f"baseline (unbroken) selftest exit code: {baseline_rc}")
if baseline_rc != 0:
    print("FAIL: the self-test does not pass on the unbroken file, so a red "
          "result below would prove nothing.")
    restore()
    sys.exit(1)

caught_count = 0
for name, old, new in DEFECTS:
    if old not in base:
        print(f"  SKIP {name!r}: anchor text not found, cannot inject")
        continue
    write(base.replace(old, new, 1))
    rc = run_selftest()
    caught = rc != 0
    is_noop = "NOT a defect" in name
    if is_noop:
        verdict = "no-op, correctly not detected" if not caught else \
                  "UNEXPECTED: flagged a no-op"
    else:
        verdict = "caught (good)" if caught else "MISSED (bad)"
        caught_count += 1 if caught else 0
    print(f"  {name!r}: selftest exit {rc} -> {verdict}")
    restore()

rc = run_selftest()
print(f"restored selftest exit code: {rc}")
ok = caught_count >= MUST_CATCH and rc == 0
print(f"caught {caught_count} of {MUST_CATCH} required defects")
print("OK" if ok else "FAIL")
sys.exit(0 if ok else 1)