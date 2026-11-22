#!/usr/bin/env bash
# Self-test of check-link-pure.sh (story 2.6 Task 8.2, test F.18): every
# bad fixture must be reported, the good fixture and the real
# controller/link tree must pass, a usage error exits 2, and the banned
# identifier list must be identical to check-core-pure.sh's (D16: the two
# copies may not drift).
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

# Drift guard: the banned identifier line of both gates must be identical.
core_line="$(grep -E '^banned_word_re=' "${fw}/tools/check-core-pure.sh" || true)"
link_line="$(grep -E '^banned_word_re=' "${check}" || true)"
if [ -z "${core_line}" ] || [ "${core_line}" != "${link_line}" ]; then
    echo "FAIL: banned_word_re differs between check-core-pure.sh and check-link-pure.sh"
    echo "  core: ${core_line}"
    echo "  link: ${link_line}"
    fail=1
fi

[ "${fail}" -eq 0 ] && echo "link_pure_selftest: ok (${count} bad fixtures, banned list in step with the core gate)"
exit "${fail}"
