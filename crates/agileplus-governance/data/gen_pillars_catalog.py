#!/usr/bin/env python3
"""Generate PILLARS-CATALOG.json from the v38 audit-catalog markdown.

wraps: stdlib only. One-shot codegen over the phenotype-org-audits v38 catalog
(WORKER-SPEC + pillar files) → the machine-readable rubric the SpecKitty
ScoringEngine consumes (see docs/design/SPECKITTY-SCORECARD-ENFORCEMENT.md §2.1).

New-format files (L30, L81-L122) expose sub-pillars as `### Ln[.m] — Title` and are
parsed into full sub_pillar entries. Survived L0-L80 defs use a prose format and are
emitted as cluster entries with a `defs_ref` to their markdown (sub-pillars not yet
individually enumerable). Usage:

    python3 gen_pillars_catalog.py <catalog_dir> <survived_defs_dir> > PILLARS-CATALOG.json
"""
import json
import re
import sys
from pathlib import Path

SUBPILLAR_RE = re.compile(r"^###\s+(L\d+(?:\.\d+)?)\s+[—-]\s+(.+?)\s*$")
NAME_RE = re.compile(r"^\*\*Name:\*\*\s*(.+?)\s*$")
ACCEPT_RE = re.compile(r"^\*\*Acceptance criterion:\*\*\s*(.+?)\s*$")
SOFT_RE = re.compile(r"^\*\*Soft-optimizing goal:\*\*\s*(.+?)\s*$")

# cluster_id, pillar_range, category, source ('new' file basename or 'defs' ref)
CLUSTERS = [
    ("C00", "L0-L9",     "Architecture + Module",           "defs", "audit-30-pillar-L{0..9}.md"),
    ("C01", "L10-L19",   "CI, DX, Observability",           "defs", "audit-30-pillar-L{10..19}.md"),
    ("C02", "L20-L29",   "Error handling, API, Governance", "defs", "audit-30-pillar-L{20..29}.md"),
    ("C03", "L30",       "Agent Readiness",                 "new",  "L30-agent-readiness.md"),
    ("C04", "L31-L40",   "Security",                        "defs", "audit-30-pillar-L31-L40-security.md"),
    ("C05", "L41-L50",   "Observability (deep)",            "defs", "audit-30-pillar-L41-L50-observability.md"),
    ("C06", "L51-L60",   "Supply Chain",                    "defs", "audit-30-pillar-L51-L60-supply-chain.md"),
    ("C07", "L61-L70",   "DX, QEng, Portability",           "defs", "audit-30-pillar-L61-L70-dx-qeng-portability.md"),
    ("C08", "L71-L80",   "Eval Coverage",                   "defs", "audit-30-pillar-L71-L80-eval-coverage.md"),
    ("C09", "L81-L95",   "Accessibility + UX",              "new",  "X-ax-L81-L95.md"),
    ("C10", "L96-L107",  "Visual Identity",                 "new",  "X-visual-identity-L96-L107.md"),
    ("C11", "L108-L122", "Packaging + Distribution",        "new",  "X-packaging-distribution-L108-L122.md"),
]


def parse_new(md_path: Path):
    """Extract sub_pillars from a new-format catalog file."""
    subs, cur = [], None
    for line in md_path.read_text(encoding="utf-8").splitlines():
        m = SUBPILLAR_RE.match(line)
        if m:
            if cur:
                subs.append(cur)
            cur = {"id": m.group(1), "title": m.group(2), "name": None,
                   "acceptance": None, "soft_goal": None, "evidence_pattern": "file:line"}
            continue
        if cur is None:
            continue
        for regex, key in ((NAME_RE, "name"), (ACCEPT_RE, "acceptance"), (SOFT_RE, "soft_goal")):
            mm = regex.match(line)
            if mm and cur[key] is None:
                cur[key] = mm.group(1)
    if cur:
        subs.append(cur)
    return subs


def main():
    cat_dir = Path(sys.argv[1])
    # sys.argv[2] (defs_dir) accepted for forward-compat but currently unused (all L0-L80 clusters use `defs_ref` strings in CLUSTERS, not a directory scan).
    pillars = []
    total_subs = 0
    for cid, prange, category, kind, src in CLUSTERS:
        entry = {"cluster": cid, "pillar_range": prange, "category": category,
                 "scoring": {"scale": "0-3", "glyphs": {"0": "✗", "1": "△", "2": "~", "3": "✓"},
                             "grade": {"A": 90, "B": 75, "C": 60, "D": 40, "F": 0}}}
        if kind == "new":
            subs = parse_new(cat_dir / src)
            entry["source"] = f"audit-v38/catalog/{src}"
            entry["sub_pillars"] = subs
            total_subs += len(subs)
        else:
            entry["source"] = "audit-30-pillar/"
            entry["defs_ref"] = src
            entry["sub_pillars"] = []  # prose defs; not individually enumerated yet
        pillars.append(entry)
    catalog = {
        "version": "1.0",
        "schema": "phenotype/audit-v38",
        "clusters": len(CLUSTERS),
        "sub_pillars_enumerated": total_subs,
        "note": ("L0-L80 clusters reference prose defs at audit-30-pillar/ "
                 "(sub_pillars enumerated for L30 + L81-L122 only)."),
        "pillars": pillars,
    }
    json.dump(catalog, sys.stdout, ensure_ascii=False, indent=2)
    print()


if __name__ == "__main__":
    main()
