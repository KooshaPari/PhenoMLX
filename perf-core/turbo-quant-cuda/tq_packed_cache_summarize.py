"""Print the packed-cache result JSON (its schema differs from the harness's)."""
import json
import sys

path = sys.argv[1] if len(sys.argv) > 1 else "packed_cache_stream.json"
d = json.load(open(path, encoding="utf-8"))
print(f"backend: {d.get('backend')}")
print(f"config:  {d.get('config')}")
print(f"wall:    {d.get('wall_clock_s')} s\n")
print(f"{'config':16s} {'PPL':>10s} {'delta':>9s} {'peak GiB':>9s}")
for name, r in d["results"].items():
    if "error" in r:
        print(f"{name:16s} {'FAILED':>10s}  {r['error'][:60]}")
        continue
    delta = r.get("ppl_delta_pct")
    print(f"{name:16s} {r['perplexity']:10.4f} "
          f"{'-' if delta is None else f'{delta:+.2f}%':>9s} "
          f"{r['peak_alloc_gib']:9.2f}")
