#!/usr/bin/env bash
# Self-test of check-core-pure.sh (story 2.4 Task 7.2, test F.13): every bad
# fixture must be reported, the good fixture and the real controller/core
# tree must pass, and a usage error exits 2.
# Usage: core_pure_selftest.sh CHECK_CORE_PURE_SH FIRMWARE_DIR
set -euo pipefail

[ "$#" -eq 2 ] || { echo "usage: $0 CHECK_CORE_PURE_SH FIRMWARE_DIR" >&2; exit 2; }
check="$1"
fw="$2"
fixtures="${fw}/tests/core_pure_fixtures"
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

if ! bash "${check}" "${fw}/controller/core"; then
    echo "FAIL: the real controller/core tree is not pure"
    fail=1
fi

rc=0
bash "${check}" a b >/dev/null 2>&1 || rc=$?
[ "${rc}" -eq 2 ] || { echo "FAIL: usage error must exit 2 (got ${rc})"; fail=1; }

[ "${fail}" -eq 0 ] && echo "core_pure_selftest: ok (${count} bad fixtures)"
exit "${fail}"
