#!/usr/bin/env bash
# Self-test of check-p2-pure.sh (story 2.3 Task 6.2, test F.30): every bad
# fixture must be reported, the good fixture and the real
# controller/profile_p2 tree must pass, a usage error exits 2, and the
# banned identifier list must be identical to check-core-pure.sh's (D16:
# the copies may not drift).
# Usage: p2_pure_selftest.sh CHECK_P2_PURE_SH FIRMWARE_DIR
set -euo pipefail

[ "$#" -eq 2 ] || { echo "usage: $0 CHECK_P2_PURE_SH FIRMWARE_DIR" >&2; exit 2; }
check="$1"
fw="$2"
fixtures="${fw}/tests/p2_pure_fixtures"
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

if ! bash "${check}" "${fw}/controller/profile_p2"; then
    echo "FAIL: the real controller/profile_p2 tree is not pure"
    fail=1
fi

rc=0
bash "${check}" a b >/dev/null 2>&1 || rc=$?
[ "${rc}" -eq 2 ] || { echo "FAIL: usage error must exit 2 (got ${rc})"; fail=1; }

# Drift guard: the banned identifier line of both gates must be identical.
core_line="$(grep -E '^banned_word_re=' "${fw}/tools/check-core-pure.sh" || true)"
p2_line="$(grep -E '^banned_word_re=' "${check}" || true)"
if [ -z "${core_line}" ] || [ "${core_line}" != "${p2_line}" ]; then
    echo "FAIL: banned_word_re differs between check-core-pure.sh and check-p2-pure.sh"
    echo "  core: ${core_line}"
    echo "  p2:   ${p2_line}"
    fail=1
fi

[ "${fail}" -eq 0 ] && echo "p2_pure_selftest: ok (${count} bad fixtures, banned list in step with the core gate)"
exit "${fail}"
