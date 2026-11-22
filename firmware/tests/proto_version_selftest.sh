#!/usr/bin/env bash
# Self-test of check-proto-version.sh (story 2.2 D4): the real tree passes;
# copies with a changed OCB_PROTO_VERSION, a wrong vector header or a missing
# vector header fail. Copies live in WORK_DIR (the build tree), never in the
# source tree.
# Usage: proto_version_selftest.sh CHECK_PROTO_VERSION_SH FIRMWARE_DIR WORK_DIR
set -euo pipefail

[ "$#" -eq 3 ] || { echo "usage: $0 CHECK_PROTO_VERSION_SH FIRMWARE_DIR WORK_DIR" >&2; exit 2; }
check="$1"
fw="$2"
work="$3"
header="${fw}/common/proto/include/ocb_proto/consts.h"
vectors="${fw}/common/proto/vectors"
fail=0

expect_rc() { # expected_rc description command...
    local want="$1" what="$2" rc=0
    shift 2
    "$@" >/dev/null 2>&1 || rc=$?
    if [ "${rc}" -ne "${want}" ]; then
        echo "FAIL: ${what}: exit ${rc}, expected ${want}"
        fail=1
    fi
}

# Real tree.
if ! bash "${check}"; then
    echo "FAIL: the real tree does not pass"
    fail=1
fi

rm -rf "${work}"
mkdir -p "${work}/vectors"
cp "${vectors}"/*.txt "${work}/vectors/"
cp "${header}" "${work}/consts.h"
expect_rc 0 "unchanged copies" bash "${check}" "${work}/consts.h" "${work}/vectors"

# 1. OCB_PROTO_VERSION changed in the header.
sed 's/^#define OCB_PROTO_VERSION .*/#define OCB_PROTO_VERSION 2/' "${header}" > "${work}/consts_v2.h"
grep -q '^#define OCB_PROTO_VERSION 2$' "${work}/consts_v2.h" || { echo "FAIL: could not patch the header copy"; fail=1; }
expect_rc 1 "header bumped to 2" bash "${check}" "${work}/consts_v2.h" "${work}/vectors"

# 2. One vector file with a wrong header.
first=""
for f in "${vectors}"/*.txt; do
    first="$(basename "${f}")"
    break
done
sed 's/^proto_version: .*/proto_version: 7f/' "${work}/vectors/${first}" > "${work}/tmp.txt"
mv "${work}/tmp.txt" "${work}/vectors/${first}"
expect_rc 1 "one vector header wrong" bash "${check}" "${work}/consts.h" "${work}/vectors"
cp "${vectors}/${first}" "${work}/vectors/${first}"

# 3. One vector file with the header removed.
grep -v '^proto_version:' "${vectors}/${first}" > "${work}/vectors/${first}"
expect_rc 1 "one vector header missing" bash "${check}" "${work}/consts.h" "${work}/vectors"

# Usage error.
expect_rc 2 "usage error" bash "${check}" a b c

rm -rf "${work}"
[ "${fail}" -eq 0 ] && echo "proto_version_selftest: ok"
exit "${fail}"
