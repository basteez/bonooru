#!/usr/bin/env bash
# Self-test of check-link-pure.sh (story 2.6 Task 8.2, test F.18): every
# bad fixture must be reported, the good fixture and the real
# controller/link tree must pass, a usage error exits 2, and the gate must
# take its identifier rules from tools/pure-rules.sh (story 4.0 D1: no
# private copy that could drift).
# Usage: link_pure_selftest.sh CHECK_LINK_PURE_SH FIRMWARE_DIR
set -euo pipefail

[ "$#" -eq 2 ] || { echo "usage: $0 CHECK_LINK_PURE_SH FIRMWARE_DIR" >&2; exit 2; }
check="$1"
fw="$2"
fixtures="${fw}/tests/link_pure_fixtures"
fail=0
count=0

for f in "${fixtures}"/bad/*; do
    count=$((count + 1))
    rc=0
    out="$(bash "${check}" "${f}" 2>/dev/null)" || rc=$?
    if [ "${rc}" -ne 1 ] || [ -z "${out}" ]; then
        echo "FAIL: bad fixture not reported (rc=${rc}): ${f}"
        fail=1
    fi
done
[ "${count}" -gt 0 ] || { echo "FAIL: no bad fixtures found"; fail=1; }

if ! bash "${check}" "${fixtures}/good" >/dev/null; then
    echo "FAIL: good fixture reported"
    fail=1
fi

if ! bash "${check}" "${fw}/controller/link"; then
    echo "FAIL: the real controller/link tree is not pure"
    fail=1
fi

rc=0
bash "${check}" a b >/dev/null 2>&1 || rc=$?
[ "${rc}" -eq 2 ] || { echo "FAIL: usage error must exit 2 (got ${rc})"; fail=1; }

# Shared-list guard (story 4.0 D1): the gate sources tools/pure-rules.sh
# and keeps no private banned_word_re that could drift from the others.
if ! grep -q -E '^[[:space:]]*\.[[:space:]]+"\$\{script_dir\}/pure-rules\.sh"' "${check}"; then
    echo "FAIL: $(basename "${check}") does not source tools/pure-rules.sh"
    fail=1
fi
if grep -q -E '^[[:space:]]*banned_word_re=' "${check}"; then
    echo "FAIL: $(basename "${check}") defines a private banned_word_re"
    fail=1
fi

[ "${fail}" -eq 0 ] && echo "link_pure_selftest: ok (${count} bad fixtures, rules from tools/pure-rules.sh)"
exit "${fail}"
