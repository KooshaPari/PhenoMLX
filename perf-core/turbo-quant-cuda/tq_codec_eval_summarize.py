"""Print a compact table from tq_codec_eval JSON outputs."""

import glob
import json
import os
import sys

paths = sys.argv[1:] or sorted(
    glob.glob(os.path.join(os.path.dirname(__file__), "*.json"))
)
for path in paths:
    if os.path.basename(path) == "codec_eval_3b.json":
        continue
    try:
        d = json.load(open(path, encoding="utf-8"))
    except Exception as e:  # noqa: BLE001
        print(f"skip {path}: {e}")
        continue
    p = d.get("perplexity_qdq_hooked")
    if not p:
        continue
    dist = d.get("distortion_on_real_kv", {})
    bpc = d.get("bits_per_coordinate_including_metadata", {})
    print(
        f"\n=== {os.path.basename(path)} | {d['model']} | "
        f"k_mode={d['config']['k_mode']} | {d['config']['bits']}-bit ==="
    )
    print(
        f"{'scheme':34s} {'bits/coord':>10s} {'rel_err':>9s} {'PPL':>9s} {'delta%':>8s}"
    )
    for k, v in p.items():
        rel = dist.get(k, {}).get("relative_error", float("nan"))
        print(
            f"{k:34s} {bpc.get(k, float('nan')):10.3f} {rel:9.5f} "
            f"{v['perplexity']:9.3f} {v['ppl_delta_pct']:+8.2f}"
        )
