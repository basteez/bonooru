#!/usr/bin/env bash
# Self-test of the pin grep (Story 2.1 Task 7.3, test-plan F.0):
# every bad fixture must be reported, the good fixtures and the real
# firmware tree must pass.
# Usage: pin_grep_selftest.sh CHECK_PINS_SH FIRMWARE_DIR
set -euo pipefail

[ "$#" -eq 2 ] || { echo "usage: $0 CHECK_PINS_SH FIRMWARE_DIR" >&2; exit 2; }
check="$1"
fw="$2"
fixtures="${fw}/tests/pin_check_fixtures"
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
    echo "FAIL: good fixtures reported"
    fail=1
fi

if ! bash "${check}" "${fw}"; then
    echo "FAIL: the real firmware tree has GPIO numbers outside pins.h"
    fail=1
fi

rc=0
bash "${check}" a b >/dev/null 2>&1 || rc=$?
[ "${rc}" -eq 2 ] || { echo "FAIL: usage error must exit 2 (got ${rc})"; fail=1; }

[ "${fail}" -eq 0 ] && echo "pin_grep_selftest: ok (${count} bad fixtures)"
exit "${fail}"
