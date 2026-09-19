"""Render (and optionally verify) the BlockQuantCache long-context table.

The table in `docs/TURBOQUANT-EXTRAPOLATION.md` item (g) is generated from the
result JSONs committed under `pilot/results/block_cache_*.json`. Those files
used to live only in a scratch directory, so every path the docs cited was
dangling. They are committed now and this script is the thing that reads them,
which makes the table reproducible instead of transcribed.

Run:
    python tq_block_cache_results_table.py           # print the table
    python tq_block_cache_results_table.py --check    # assert the doc values
"""

import argparse
import glob
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
RESULTS = os.path.join(REPO, "pilot", "results")

# The exact numbers quoted in docs/TURBOQUANT-EXTRAPOLATION.md item (g).
# context -> (fp16 ppl, block4 ppl, delta pct, fp16 peak alloc, block4 peak alloc,
#             saved GiB)
DOC_TABLE = {
    8192: (6.0228, 6.0815, 0.97, 6.400, 6.233, 0.167),
    16384: (4.6279, 4.6711, 0.93, 6.681, 6.338, 0.343),
    24576: (3.4889, 3.5157, 0.77, 6.963, 6.471, 0.492),
    32768: (2.5536, 2.5700, 0.64, 7.244, 6.706, 0.538),
}

PPL_TOL = 0.0001
GIB_TOL = 0.0006
DELTA_TOL = 0.006

# The resident-attribution table in the same docs section, from
# pilot/results/block_cache_breakdown.json:
# context -> (payload MiB, metadata MiB, residual MiB, total MiB, fp16 KV MiB)
DOC_BREAKDOWN = {
    2048: (18, 9, 0, 27, 72),
    8192: (72, 36, 0, 108, 288),
    16384: (144, 72, 0, 216, 576),
    32768: (288, 144, 0, 432, 1152),
}
MIB_TOL = 0.006

# Runs captured *before* the get_seq_length/get_mask_sizes overrides landed.
# They are kept on purpose: block4 PPL of 811k (8K) and 57k (1K) is the
# fingerprint of the second-chunk mask bug, and `block_cache_8k_fixed.json`
# exists precisely to be the same measurement with the fix in place. They are
# reported separately and are never part of the item (g) table.
PREFIX_BROKEN = {
    "block_cache_1k.json",
    "block_cache_8k.json",
}
# Among several artifacts for one context length, this one is the table row.
PREFERRED = {
    8192: "block_cache_8k_fixed.json",
}


def load_rows():
    broken, fixed = [], []
    for path in glob.glob(os.path.join(RESULTS, "block_cache_*.json")):
        with open(path, encoding="utf-8") as fh:
            doc = json.load(fh)
        if "rows" in doc:
            continue
        cfg = doc.get("config", {})
        res = doc.get("results", {})
        ctx = cfg.get("max_tokens")
        if ctx is None or "fp16" not in res or "block4" not in res:
            continue
        name = os.path.basename(path)
        row = (ctx, name, res["fp16"], res["block4"])
        (broken if name in PREFIX_BROKEN else fixed).append(row)

    # One row per context: the preferred artifact, else the only one.
    by_ctx = {}
    for ctx, name, fl, b4 in fixed:
        want = PREFERRED.get(ctx)
        if want is not None and name != want:
            continue
        by_ctx[ctx] = (ctx, name, fl, b4)
    fixed = sorted(by_ctx.values())
    broken.sort()
    return broken, fixed


def load_breakdown():
    path = os.path.join(RESULTS, "block_cache_breakdown.json")
    if not os.path.exists(path):
        return {}
    with open(path, encoding="utf-8") as fh:
        doc = json.load(fh)
    return {int(k): v for k, v in doc.get("rows", {}).items()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    broken, rows = load_rows()
    if not rows:
        raise SystemExit(f"no block_cache_*.json found under {RESULTS}")

    header = (
        "context  fp16 PPL  block4 PPL  delta     "
        "fp16 alloc  block4 alloc  saved alloc  fp16 res  block4 res"
    )
    print(header)
    print("-" * len(header))
    failures = []
    for ctx, name, fl, b4 in rows:
        saved = fl["peak_alloc_gib"] - b4["peak_alloc_gib"]
        delta = (b4["perplexity"] / fl["perplexity"] - 1.0) * 100.0
        print(
            f"{ctx:>7}  {fl['perplexity']:8.4f}  {b4['perplexity']:10.4f}  "
            f"{delta:+5.2f}%  {fl['peak_alloc_gib']:10.3f}  "
            f"{b4['peak_alloc_gib']:12.3f}  {saved:11.3f}  "
            f"{fl['peak_reserved_gib']:8.3f}  {b4['peak_reserved_gib']:10.3f}"
        )
        want = DOC_TABLE.get(ctx)
        if want is None:
            continue
        w_fp16, w_b4, w_delta, w_fa, w_ba, w_saved = want
        for label, got, exp, tol in (
            ("fp16 ppl", fl["perplexity"], w_fp16, PPL_TOL),
            ("block4 ppl", b4["perplexity"], w_b4, PPL_TOL),
            ("delta pct", delta, w_delta, DELTA_TOL),
            ("fp16 alloc", fl["peak_alloc_gib"], w_fa, GIB_TOL),
            ("block4 alloc", b4["peak_alloc_gib"], w_ba, GIB_TOL),
            ("saved", saved, w_saved, GIB_TOL),
        ):
            if abs(got - exp) > tol:
                failures.append(
                    f"  ctx {ctx} [{name}] {label}: doc={exp} results={got}"
                )

    if broken:
        print("\npre-fix artifacts (kept as bug evidence, excluded from the table):")
        for ctx, name, fl, b4 in broken:
            ratio = b4["perplexity"] / fl["perplexity"]
            print(
                f"  {name:<24} ctx {ctx:>6}  fp16 PPL {fl['perplexity']:.4f}"
                f"  block4 PPL {b4['perplexity']:.4f}  ({ratio:,.0f}x)"
            )

    breakdown = load_breakdown()
    if breakdown:
        print("\nresident attribution (MiB)")
        print(
            "  context     payload  metadata  residual     total      fp16  reduction"
        )
        for ctx, row in sorted(breakdown.items()):
            print(
                f"  {ctx:>7}  {row['payload_mib']:10.2f} {row['metadata_mib']:9.2f} "
                f"{row['residual_mib']:9.2f} {row['total_mib']:9.2f} "
                f"{row['fp16_kv_mib']:9.2f} {row['reduction_vs_fp16']:11.3f}"
            )
            want = DOC_BREAKDOWN.get(ctx)
            if want is None:
                continue
            got = (
                row["payload_mib"],
                row["metadata_mib"],
                row["residual_mib"],
                row["total_mib"],
                row["fp16_kv_mib"],
            )
            for label, g, e in zip(
                ("payload", "metadata", "residual", "total", "fp16"), got, want
            ):
                if abs(g - e) > MIB_TOL:
                    failures.append(
                        f"  ctx {ctx} [breakdown] {label}: doc={e} results={g}"
                    )
        print("\nresident saving vs measured peak saving (GiB)")
        # DOC_TABLE is (fp16 ppl, block4 ppl, delta, fp16 alloc, block4 alloc, saved)
        fp16_peak = {ctx: v[3] for ctx, v in DOC_TABLE.items()}
        bk_peak = {ctx: v[4] for ctx, v in DOC_TABLE.items()}
        for ctx in sorted(set(breakdown) & set(fp16_peak)):
            resident = (
                breakdown[ctx]["fp16_kv_mib"] - breakdown[ctx]["total_mib"]
            ) / 1024
            measured = fp16_peak[ctx] - bk_peak[ctx]
            print(
                f"  {ctx:>7}   resident {resident:.3f}   measured {measured:.3f}   "
                f"shortfall {resident - measured:.3f}"
            )

    if args.check:
        if failures:
            print(f"\nFAIL: {len(failures)} doc/result mismatch(es)")
            for line in failures:
                print(line)
            raise SystemExit(1)
        print(
            f"\nOK: every documented row matches the committed results "
            f"({len(rows)} eval rows, {len(breakdown)} breakdown rows)"
        )


if __name__ == "__main__":
    main()
