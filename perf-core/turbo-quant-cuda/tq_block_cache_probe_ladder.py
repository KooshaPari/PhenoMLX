"""Parse BlockQuantCache decode-ceiling probe logs into a citable ladder.

`tq_block_cache_decode_oom_probe.py` writes plain text. The docs quote peaks
from that text, which means the numbers were transcribed by hand and could not
be checked. This turns the logs into JSON so the table is generated, and
`--check` asserts the values the docs quote.

Run:
    python tq_block_cache_probe_ladder.py LOG [LOG ...]              # print
    python tq_block_cache_probe_ladder.py LOG ... --json OUT.json     # write
    python tq_block_cache_probe_ladder.py LOG ... --check             # assert
"""

import argparse
import json
import os
import re

RE_PREFILL = re.compile(
    r"^\s+prefill (\d+) OK loss=([\d.]+) blocks=(\d+) seq_len=(\d+)\s*$"
)
RE_SKIP = re.compile(r"^\s+prefill (\d+) SKIPPED: (.+)$")
RE_PREDECODE_FAIL = re.compile(r"^\s+prefill (\d+) FAILED pre-decode: (.+)$")
RE_STEP = re.compile(r"^\s+step (\d+) OK peak_alloc=([\d.]+) reserved=([\d.]+) GiB\s*$")
RE_STEP_FAIL = re.compile(r"^\s+step (\d+) FAILED: (.+)$")
RE_SURVIVED = re.compile(r"^\s+-> decode survived (\d+) steps at prefill (\d+)\s*$")
RE_DECODE_FAIL = re.compile(r"^\s+-> decode failed at prefill (\d+)\s*$")
RE_LADDER = re.compile(r"^corpus (\d+) tokens \| ladder \[([\d, ]*)\] \| need (\d+)$")

# The exact reserved-GiB step-0 peaks quoted in docs/TURBOQUANT-EXTRAPOLATION.md
# item (g), for the rungs that were measured when that text was written.
DOC_LADDER = {
    1024: 5.88,
    2048: 5.92,
    4096: 6.11,
    6144: 6.32,
    8192: 6.50,
    12288: 6.91,
    16384: 7.30,
}
GIB_TOL = 0.006


def parse_logs(paths):
    rows = {}
    for path in paths:
        corpus_tokens = None
        cur = None
        with open(path, encoding="utf-8", errors="replace") as fh:
            for raw in fh:
                line = raw.rstrip("\n")
                m = RE_LADDER.match(line)
                if m:
                    corpus_tokens = int(m.group(1))
                    continue
                m = RE_PREFILL.match(line)
                if m:
                    cur = {
                        "prefill": int(m.group(1)),
                        "loss": float(m.group(2)),
                        "blocks": int(m.group(3)),
                        "seq_len": int(m.group(4)),
                        "steps": [],
                        "status": "prefilled",
                        "source": os.path.basename(path),
                        "corpus_tokens": corpus_tokens,
                    }
                    continue
                m = RE_STEP.match(line)
                if m and cur is not None:
                    cur["steps"].append(
                        {
                            "step": int(m.group(1)),
                            "peak_alloc_gib": float(m.group(2)),
                            "reserved_gib": float(m.group(3)),
                        }
                    )
                    continue
                m = RE_STEP_FAIL.match(line)
                if m and cur is not None:
                    cur["status"] = f"decode failed at step {m.group(1)}"
                    cur["error"] = m.group(2)
                    continue
                m = RE_SURVIVED.match(line)
                if m and cur is not None:
                    cur["status"] = f"survived {m.group(1)} steps"
                    rows[cur["prefill"]] = cur
                    cur = None
                    continue
                m = RE_DECODE_FAIL.match(line)
                if m:
                    prefill = int(m.group(1))
                    if cur is not None and cur["prefill"] == prefill:
                        rows[prefill] = cur
                        cur = None
                    continue
                m = RE_SKIP.match(line)
                if m:
                    rows[int(m.group(1))] = {
                        "prefill": int(m.group(1)),
                        "status": "skipped",
                        "reason": m.group(2).strip(),
                        "steps": [],
                        "source": os.path.basename(path),
                        "corpus_tokens": corpus_tokens,
                    }
                    continue
                m = RE_PREDECODE_FAIL.match(line)
                if m:
                    rows[int(m.group(1))] = {
                        "prefill": int(m.group(1)),
                        "status": "failed before decode",
                        "reason": m.group(2).strip(),
                        "steps": [],
                        "source": os.path.basename(path),
                        "corpus_tokens": corpus_tokens,
                    }
                    continue
    return dict(sorted(rows.items()))


def render(rows):
    header = "prefill  blocks  step0 alloc  step0 reserved  peak reserved  status"
    print(header)
    print("-" * len(header))
    for prefill, r in rows.items():
        steps = r.get("steps") or []
        a0 = f"{steps[0]['peak_alloc_gib']:.2f}" if steps else "-"
        r0 = f"{steps[0]['reserved_gib']:.2f}" if steps else "-"
        peak = f"{max(s['reserved_gib'] for s in steps):.2f}" if steps else "-"
        status = r.get("status", "?")
        if r.get("reason"):
            status = f"{status}: {r['reason'][:60]}"
        print(
            f"{prefill:>7}  {r.get('blocks', '-'):>6}  {a0:>11}  {r0:>14}  "
            f"{peak:>13}  {status}"
        )


def check(rows):
    failures = []
    for prefill, want in sorted(DOC_LADDER.items()):
        r = rows.get(prefill)
        if r is None:
            failures.append(f"  prefill {prefill}: documented but not measured")
            continue
        steps = r.get("steps") or []
        if not steps:
            failures.append(
                f"  prefill {prefill}: documented peak {want} but "
                f"status is {r.get('status')}"
            )
            continue
        got = steps[0]["reserved_gib"]
        if abs(got - want) > GIB_TOL:
            failures.append(
                f"  prefill {prefill}: doc step-0 reserved {want} vs measured {got}"
            )
    return failures


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("logs", nargs="+")
    ap.add_argument("--json")
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    rows = parse_logs(args.logs)
    if not rows:
        raise SystemExit("no probe rows parsed")
    render(rows)

    if args.json:
        out = {
            "model": "Qwen/Qwen2.5-3B-Instruct",
            "block": 32,
            "bits": 4,
            "decode_steps": 4,
            "source_logs": [os.path.basename(p) for p in args.logs],
            "ladder": rows,
        }
        with open(args.json, "w", encoding="utf-8") as fh:
            json.dump(out, fh, indent=2, sort_keys=True)
            fh.write("\n")
        print(f"\nwrote {args.json}")

    if args.check:
        failures = check(rows)
        if failures:
            print(f"\nFAIL: {len(failures)} documented ladder mismatch(es)")
            for line in failures:
                print(line)
            raise SystemExit(1)
        print(f"\nOK: documented ladder peaks match the probe logs ({len(rows)} rows)")


if __name__ == "__main__":
    main()
