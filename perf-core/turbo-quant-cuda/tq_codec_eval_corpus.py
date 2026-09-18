"""Build the long-context PPL corpus deterministically from the repo's own docs.

Provenance over convenience: anyone with the checkout can rebuild this file, and
the source list is fixed below. Output is plain text, LF, so it can be passed
straight to tq_codec_eval.py --corpus.

Usage: python build_corpus.py [repo_root] [out_path]
"""

import os
import sys

ROOT = sys.argv[1] if len(sys.argv) > 1 else r"C:\phenotype-omlx"
OUT = (
    sys.argv[2]
    if len(sys.argv) > 2
    else r"C:\Users\koosh\agents\sandbox\tq-eval\corpus_repodocs.txt"
)

# Fixed order so the corpus is reproducible. Only root-level prose docs: no
# generated files, no fixtures, no third-party text.
SOURCES = [
    "README.md",
    "ARCHITECTURE.md",
    "PLAN.md",
    "PRD.md",
    "ADR.md",
    "COMPARISON.md",
    "FUNCTIONAL_REQUIREMENTS.md",
    "USER_JOURNEYS.md",
    "TEST_COVERAGE_MATRIX.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "CHANGELOG.md",
    "AGENTS.md",
    "CLAUDE.md",
]

parts = []
used = []
for name in SOURCES:
    path = os.path.join(ROOT, name)
    if not os.path.exists(path):
        continue
    text = open(path, encoding="utf-8", errors="replace").read()
    parts.append(f"===== {name} =====\n{text}")
    used.append((name, len(text)))

corpus = "\n\n".join(parts).replace("\r\n", "\n")
with open(OUT, "w", encoding="utf-8", newline="\n") as f:
    f.write(corpus)

print(f"wrote {OUT}")
print(f"  bytes: {len(corpus)}  sources: {len(used)}")
for name, n in used:
    print(f"    {name:32s} {n:8d} bytes")
