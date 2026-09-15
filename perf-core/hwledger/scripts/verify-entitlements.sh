#!/usr/bin/env bash
# Gate: entitlements plist must be well-formed XML with unique keys.
# Exits non-zero on malformed plist or duplicate entitlement keys.
set -euo pipefail

PLIST="${1:?usage: verify-entitlements.sh <path-to-plist>}"
python3 - "$PLIST" <<'PY'
import plistlib, re, sys

path = sys.argv[1]
data = open(path, "rb").read()

# 1. Well-formed, Apple-parseable plist.
try:
    parsed = plistlib.loads(data)
except Exception as exc:
    print(f"FAIL: malformed plist: {exc}")
    sys.exit(1)

# 2. Duplicate <key> entries are rejected by codesign; fail on any.
keys = re.findall(rb"<key>([^<]+)</key>", data)
dups = sorted({k.decode() for k in keys if keys.count(k) > 1})
if dups:
    print(f"FAIL: duplicate entitlement keys: {dups}")
    sys.exit(1)

print(f"OK: {path} well-formed, {len(keys)} unique keys")
sys.exit(0)
PY
