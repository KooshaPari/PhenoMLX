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

# The documented item (g) table is 3B-only. Other models in pilot/results (the
# 7B re-run) are reported separately and never checked against these numbers,
# because keying rows on context length alone silently collides once a second
# model is measured at the same lengths.
DOC_MODEL = "Qwen/Qwen2.5-3B-Instruct"
# The documented table is one operating point: 4-bit, 256-token chunks. Other
# artifacts at the same (model, context) are a *different* config -- a bits
# sweep, or a non-block-aligned step that exercises the fp16 residual -- and
# must not be mistaken for the table row. Keying on (model, context) alone let
# the last file read win.
DOC_BITS = 4
DOC_STEP = 256

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

# The same 8K prefill at other bit widths, to pin the part of the resident
# footprint that does *not* move: file -> (payload, metadata, residual, total,
# fp16 KV) in MiB. The payload should scale with the bit width and the fp32
# metadata should not, which is what makes the metadata a floor.
DOC_BREAKDOWN_BITS = {
    "block_cache_breakdown_b3_8k.json": (54, 36, 0, 90, 288),
    "block_cache_breakdown_b2_8k.json": (36, 36, 0, 72, 288),
}

# The bit-width and chunking rows documented alongside the main table, keyed by
# artifact: name -> (quantized PPL, delta vs fp16 percent).
DOC_OTHER = {
    "block_cache_b3_8k.json": (6.3559, 5.53),
    "block_cache_b3_16k.json": (4.8556, 4.92),
    "block_cache_b2_8k.json": (9.3076, 54.54),
    "block_cache_b4_step100_8k.json": (6.1129, 1.22),
}

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
    (DOC_MODEL, 8192): "block_cache_8k_fixed.json",
}


def load_rows():
    broken, doc_candidates, others = [], [], []
    for path in glob.glob(os.path.join(RESULTS, "block_cache_*.json")):
        with open(path, encoding="utf-8") as fh:
            doc = json.load(fh)
        if "rows" in doc:
            continue
        cfg = doc.get("config", {})
        res = doc.get("results", {})
        ctx = cfg.get("max_tokens")
        # The quantized config is labelled `block<bits>` by the driver, so a
        # 3-bit sweep lands under `block3`. Hardcoding `block4` silently skipped
        # every file from a different bit width.
        quant_keys = sorted(k for k in res if k != "fp16")
        if ctx is None or "fp16" not in res or not quant_keys:
            continue
        name = os.path.basename(path)
        model = doc.get("model", DOC_MODEL)
        qkey = quant_keys[0]
        row = (ctx, name, model, res["fp16"], res[qkey], cfg, qkey)
        if name in PREFIX_BROKEN:
            broken.append(row)
        elif (
            model == DOC_MODEL
            and cfg.get("bits") == DOC_BITS
            and cfg.get("step") == DOC_STEP
        ):
            doc_candidates.append(row)
        else:
            others.append(row)

    # One row per (model, context): the preferred artifact, else the only one.
    by_ctx = {}
    for ctx, name, model, fl, q, _cfg, _qkey in doc_candidates:
        want = PREFERRED.get((model, ctx))
        if want is not None and name != want:
            continue
        by_ctx[(model, ctx)] = (ctx, name, model, fl, q)
    fixed = sorted(by_ctx.values())
    broken.sort()
    others.sort(key=lambda r: (r[2], r[5].get("bits", 0), r[5].get("step", 0), r[0]))
    return broken, fixed, others


def load_breakdown():
    path = os.path.join(RESULTS, "block_cache_breakdown.json")
    if not os.path.exists(path):
        return {}
    with open(path, encoding="utf-8") as fh:
        doc = json.load(fh)
    return {int(k): v for k, v in doc.get("rows", {}).items()}


def load_breakdown_bits():
    """Same 8K prefill at other bit widths, keyed by bits."""
    out = {}
    for name in DOC_BREAKDOWN_BITS:
        path = os.path.join(RESULTS, name)
        if not os.path.exists(path):
            continue
        with open(path, encoding="utf-8") as fh:
            doc = json.load(fh)
        rows = doc.get("rows", {})
        for row in rows.values():
            out[(doc.get("bits"), int(row["requested_tokens"]))] = (name, row)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    broken, rows, others = load_rows()
    if not rows:
        raise SystemExit(f"no block_cache_*.json found under {RESULTS}")

    header = (
        "context  fp16 PPL  block4 PPL  delta     "
        "fp16 alloc  block4 alloc  saved alloc  fp16 res  block4 res"
    )
    print(header)
    print("-" * len(header))
    failures = []
    for ctx, name, model, fl, b4 in rows:
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
        for ctx, name, _model, fl, q, _cfg, _qkey in broken:
            ratio = q["perplexity"] / fl["perplexity"]
            print(
                f"  {name:<24} ctx {ctx:>6}  fp16 PPL {fl['perplexity']:.4f}"
                f"  quant PPL {q['perplexity']:.4f}  ({ratio:,.0f}x)"
            )

    if others:
        print(
            f"\nother operating points (not checked against the "
            f"{DOC_MODEL} bits={DOC_BITS} step={DOC_STEP} table):"
        )
        for ctx, name, _model, fl, q, cfg, qkey in others:
            delta = (q["perplexity"] / fl["perplexity"] - 1.0) * 100.0
            print(
                f"  {name:<34} ctx {ctx:>6} {qkey:<7} "
                f"step={cfg.get('step'):>4}  fp16 PPL {fl['perplexity']:8.4f}  "
                f"quant PPL {q['perplexity']:10.4f}  {delta:+8.2f}%  "
                f"saved {fl['peak_alloc_gib'] - q['peak_alloc_gib']:6.3f} GiB"
            )
            want = DOC_OTHER.get(name)
            if want is None:
                continue
            w_ppl, w_delta = want
            for label, got, exp, tol in (
                ("ppl", q["perplexity"], w_ppl, PPL_TOL),
                ("delta pct", delta, w_delta, DELTA_TOL),
            ):
                if abs(got - exp) > tol:
                    failures.append(
                        f"  {name} [other] {label}: doc={exp} results={got}"
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

    breakdown_bits = load_breakdown_bits()
    # Seed with the 4-bit 8K row, which lives in the main breakdown artifact,
    # so the printed table covers 4/3/2 and not just the extra bit widths.
    if 8192 in breakdown:
        breakdown_bits[(4, 8192)] = ("block_cache_breakdown.json", breakdown[8192])
    if breakdown_bits:
        print("\nresident attribution by bit width (8K prefill, MiB)")
        print("   bits     payload  metadata  residual     total  meta share")
        for (bits, ctx), (name, row) in sorted(breakdown_bits.items()):
            total = row["total_mib"]
            share = row["metadata_mib"] / total if total else 0.0
            print(
                f"  {bits:>4}  {row['payload_mib']:10.2f} {row['metadata_mib']:9.2f} "
                f"{row['residual_mib']:9.2f} {total:9.2f} {share:11.3f}"
            )
            want = DOC_BREAKDOWN_BITS.get(name)
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
                        f"  {name} [breakdown bits={bits}] {label}: doc={e} results={g}"
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
