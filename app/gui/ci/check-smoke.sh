#!/usr/bin/env bash
# G.7 (Story 4.1): check one `bonooru --smoke-test` output file against the
# AC -> measurement table: exactly one OCB_SMOKE line, `ok`, frames >= 3,
# measured inner >= 1024x680, min=1024x680, theme=dark and a backend that
# matches the OS's expected one. The app judges itself too (exit 4 on a
# fail line); this is the independent second check run by CI.
#
# Usage: check-smoke.sh <output-file> <backend-regex>
#   e.g. check-smoke.sh smoke.txt '(Xlib|Xcb)'
# Exit 0 if every field holds, 1 otherwise (with a ::error:: annotation),
# 2 on a usage error.
set -u
if [ "$#" -ne 2 ]; then
  echo "usage: $0 <output-file> <backend-regex>" >&2
  exit 2
fi
file=$1
backend_re=$2
fail() { echo "::error::smoke check: $*"; exit 1; }

[ -r "$file" ] || fail "cannot read $file"
lines=$(grep -c '^OCB_SMOKE ' "$file")
[ "$lines" -eq 1 ] || fail "expected exactly one OCB_SMOKE line, got $lines"
line=$(grep '^OCB_SMOKE ' "$file")
re='^OCB_SMOKE ok frames=([0-9]+) ppp=([0-9]+(\.[0-9]+)?) inner=([0-9]+)x([0-9]+) min=1024x680 theme=dark backend=([A-Za-z0-9]+)$'
[[ "$line" =~ $re ]] || fail "not an ok line with every field: $line"
frames=${BASH_REMATCH[1]}
w=${BASH_REMATCH[4]}
h=${BASH_REMATCH[5]}
backend=${BASH_REMATCH[6]}
[ "$frames" -ge 3 ] || fail "frames=$frames < 3"
[ "$w" -ge 1024 ] && [ "$h" -ge 680 ] || fail "inner=${w}x${h} below 1024x680"
[[ "$backend" =~ ^${backend_re}$ ]] || fail "backend=$backend does not match $backend_re"
echo "smoke check ok: $line"
