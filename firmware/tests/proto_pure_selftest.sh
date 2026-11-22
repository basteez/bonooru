#!/usr/bin/env bash
# Self-test of check-proto-pure.sh (story 2.2 Task 7.5): every bad fixture
# must be reported, the good fixture and the real ocb_proto tree must pass.
# Usage: proto_pure_selftest.sh CHECK_PROTO_PURE_SH FIRMWARE_DIR
set -euo pipefail

[ "$#" -eq 2 ] || { echo "usage: $0 CHECK_PROTO_PURE_SH FIRMWARE_DIR" >&2; exit 2; }
check="$1"
fw="$2"
fixtures="${fw}/tests/proto_pure_fixtures"
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

if ! bash "${check}" "${fw}/common/proto"; then
    echo "FAIL: the real ocb_proto tree includes a forbidden header"
    fail=1
fi

rc=0
bash "${check}" a b >/dev/null 2>&1 || rc=$?
[ "${rc}" -eq 2 ] || { echo "FAIL: usage error must exit 2 (got ${rc})"; fail=1; }

[ "${fail}" -eq 0 ] && echo "proto_pure_selftest: ok (${count} bad fixtures)"
exit "${fail}"
