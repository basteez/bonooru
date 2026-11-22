#!/usr/bin/env bash
# PROTO_VERSION check (AD-14, story 2.2 D4): every conformance vector file must
# start with the header `proto_version: <OCB_PROTO_VERSION as two hex digits>`.
# Textual: compiles nothing, so it also runs when the build is broken.
#
# Usage: firmware/tools/check-proto-version.sh [HEADER] [VECTORS_DIR]
#   HEADER       consts.h to read (default: common/proto/include/ocb_proto/consts.h)
#   VECTORS_DIR  directory of the *.txt vectors (default: common/proto/vectors)
# Exit:   0 all headers match, 1 mismatch or missing header (file named),
#         2 usage error.
#
# Portable POSIX grep/sed only, same behaviour with GNU and BSD tools.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fw="${script_dir}/.."

usage() {
    echo "usage: $0 [HEADER] [VECTORS_DIR]" >&2
    exit 2
}

[ "$#" -le 2 ] || usage
header="${1:-${fw}/common/proto/include/ocb_proto/consts.h}"
vectors="${2:-${fw}/common/proto/vectors}"
[ -f "${header}" ] || { echo "check-proto-version: no such header: ${header}" >&2; usage; }
[ -d "${vectors}" ] || { echo "check-proto-version: no such directory: ${vectors}" >&2; usage; }

ver="$(sed -n 's/^#define[[:space:]][[:space:]]*OCB_PROTO_VERSION[[:space:]][[:space:]]*\([0-9][0-9]*\)[[:space:]]*$/\1/p' "${header}")"
if [ -z "${ver}" ] || [ "$(printf '%s\n' "${ver}" | wc -l | tr -d ' ')" -ne 1 ]; then
    echo "check-proto-version: ${header}: expected exactly one '#define OCB_PROTO_VERSION <decimal>'" >&2
    exit 1
fi
if [ "${ver}" -gt 255 ]; then
    echo "check-proto-version: ${header}: OCB_PROTO_VERSION ${ver} does not fit in a u8" >&2
    exit 1
fi
want="$(printf '%02x' "${ver}")"

fail=0
count=0
for f in "${vectors}"/*.txt; do
    [ -e "${f}" ] || continue
    count=$((count + 1))
    # The first line that is neither blank nor a comment must be the header.
    first="$(grep -v -E '^[[:space:]]*(#|$)' "${f}" | sed -n '1p')"
    got="$(printf '%s\n' "${first}" | sed -n 's/^[[:space:]]*proto_version[[:space:]]*:[[:space:]]*\([0-9A-Fa-f][0-9A-Fa-f]\)[[:space:]]*$/\1/p' | tr 'A-F' 'a-f')"
    if [ -z "${got}" ]; then
        echo "check-proto-version: ${f}: missing 'proto_version: ${want}' header" >&2
        fail=1
    elif [ "${got}" != "${want}" ]; then
        echo "check-proto-version: ${f}: proto_version ${got} != OCB_PROTO_VERSION ${want} (${header})" >&2
        fail=1
    fi
done
if [ "${count}" -eq 0 ]; then
    echo "check-proto-version: no *.txt vector files in ${vectors}" >&2
    exit 1
fi
[ "${fail}" -eq 0 ] || exit 1
echo "check-proto-version: ok (proto_version ${want}, ${count} files)"
