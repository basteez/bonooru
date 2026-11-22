#!/usr/bin/env bash
# ocb_core purity check (AD-2, story 2.4 D9): the controller core stays pure
# C99 that builds on the host.
#  (a) every #include must be one of <stdint.h> <stddef.h> <stdbool.h>
#      <string.h> <limits.h>, "ocb_proto/<name>.h" or "ocb_core/<name>.h";
#      an include line naming pico, hardware/, tusb, ocb_hw or ocb_version.h
#      is refused in any form;
#  (b) no line (code or comment) may name a heap, I/O, clock, sleep or
#      multicore identifier of libc or the SDK: the core takes the time as
#      an argument, allocates nothing and does no I/O. RAM placement goes
#      through the OCB_CORE_RAMFUNC macro, not the SDK one.
#
# Usage: firmware/tools/check-core-pure.sh [ROOT]
#   ROOT  directory or single file to scan (default: controller/core)
# Exit:   0 clean, 1 violation(s) (printed as file:line:text), 2 usage error.
#
# Portable POSIX grep/sed only, same behaviour with GNU and BSD tools.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
    echo "usage: $0 [ROOT]" >&2
    exit 2
}

[ "$#" -le 1 ] || usage
root="${1:-${script_dir}/../controller/core}"
[ -e "${root}" ] || { echo "check-core-pure: no such file or directory: ${root}" >&2; usage; }

if [ -d "${root}" ]; then
    files="$(find "${root}" -type f \( -name '*.c' -o -name '*.h' \) -print | LC_ALL=C sort)"
else
    files="${root}"
fi

include_re='^[[:space:]]*#[[:space:]]*include'
allowed_re='^[[:space:]]*#[[:space:]]*include[[:space:]]*(<(stdint|stddef|stdbool|string|limits)\.h>|"ocb_(proto|core)/[A-Za-z0-9_]+\.h")[[:space:]]*(/[*/].*)?$'
banned_include_re='pico|hardware/|tusb|ocb_hw|ocb_version\.h'
# Identifiers: a word boundary is spelled out (no \b in POSIX ERE), so
# names like ocb_queue_free( stay legal while a bare free( does not.
nb='(^|[^A-Za-z0-9_])'
banned_word_re="${nb}(malloc|calloc|realloc|free[[:space:]]*\\(|[a-z]*printf|time_us|get_absolute_time|sleep_|busy_wait|multicore_|mutex_|spin_lock|__not_in_flash_func)"

found=0
count=0
while IFS= read -r f; do
    [ -n "${f}" ] || continue
    count=$((count + 1))
    hits="$( {
        grep -n -E "${include_re}" "${f}" | grep -v -E "^[0-9]+:${allowed_re#^}" || true
        grep -n -E "${include_re}" "${f}" | grep -E "${banned_include_re}" || true
        grep -n -E "${banned_word_re}" "${f}" || true
    } | LC_ALL=C sort -t: -k1,1n -u)"
    if [ -n "${hits}" ]; then
        found=1
        while IFS= read -r h; do
            printf '%s:%s\n' "${f}" "${h}"
        done <<<"${hits}"
    fi
done <<<"${files}"

if [ "${count}" -eq 0 ]; then
    echo "check-core-pure: no .c or .h files under ${root}" >&2
    exit 1
fi
if [ "${found}" -ne 0 ]; then
    echo "check-core-pure: ocb_core must stay pure C99 (AD-2): allowed includes are <stdint.h> <stddef.h> <stdbool.h> <string.h> <limits.h>, \"ocb_proto/...\" and \"ocb_core/...\"; no heap, I/O, clock, sleep or multicore identifiers" >&2
    exit 1
fi
echo "check-core-pure: ok (${count} files)"
