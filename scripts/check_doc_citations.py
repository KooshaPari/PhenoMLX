"""Fail if docs cite result artifacts that are not committed.

The block-cache tables were cited in `docs/TURBOQUANT-EXTRAPOLATION.md` for
weeks while the JSONs behind them lived only in a scratch directory, so every
one of those paths was dangling and no number on the page could be reproduced
from the repo. A citation that does not resolve is worse than no citation: it
looks verifiable and is not.

This walks `docs/*.md` for backticked `.../results/...json` references,
expands `{a,b}` brace groups, and requires each path to exist in `git ls-files`
(glob patterns must match at least one tracked file).

Run:
    python scripts/check_doc_citations.py            # repo inferred from __file__
    python scripts/check_doc_citations.py <repo>     # explicit
"""

import glob
import os
import re
import subprocess
import sys

DEFAULT_REPO = os.path.abspath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
)

CITE = re.compile(r"`([^`]*?(?:results|pilot|evals)/[^`]*?\.json)`")


def expand(rel):
    """Expand one `{a,b}` group, as docs write `block_cache_{8k,16k}.json`."""
    m = re.search(r"\{([^}]*)\}", rel)
    if not m:
        return [rel]
    out = []
    for opt in m.group(1).split(","):
        out.extend(expand(rel[: m.start()] + opt + rel[m.end() :]))
    return out


def matches(rel, tracked):
    if "*" in rel or "?" in rel:
        pat = re.compile(
            "^" + re.escape(rel).replace(r"\*", ".*").replace(r"\?", ".") + "$"
        )
        return [t for t in tracked if pat.match(t)]
    return [rel] if rel in tracked else []


def main():
    repo = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_REPO
    docs = os.path.join(repo, "docs")
    if not os.path.isdir(docs):
        raise SystemExit(f"no docs directory under {repo}")

    tracked = set(
        subprocess.run(
            ["git", "-C", repo, "ls-files"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.split()
    )

    problems = []
    for path in sorted(glob.glob(os.path.join(docs, "*.md"))):
        name = os.path.relpath(path, repo)
        for lineno, line in enumerate(open(path, encoding="utf-8"), 1):
            for raw in CITE.findall(line):
                for rel in expand(raw):
                    if not matches(rel, tracked):
                        problems.append((name, lineno, raw, rel))

    if not problems:
        print("OK: every cited result artifact in docs/ resolves to a tracked file")
        return 0

    print(f"FOUND {len(problems)} citation(s) with no tracked file:\n")
    for name, lineno, raw, rel in problems:
        print(f"  {name}:{lineno}\n    cited : {raw}")
        if raw != rel:
            print(f"    tried : {rel}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
