#!/usr/bin/env bash
# ocb_proto purity check (AD-2, story 2.2 Task 7.5, story 4.0): the
# protocol library stays pure C99 that builds on the host.
#  (a) every #include must be one of <stdint.h> <stddef.h> <stdbool.h>
#      <string.h> <limits.h> or "ocb_proto/<name>.h"; an include line naming
#      pico, hardware/, tusb, ocb_hw or ocb_version.h is refused in any form;
#  (b) no line (code or comment) may name a heap, I/O, clock, sleep,
#      multicore, process-exit (exit( abort( ...), file (fopen), strdup,
#      alloca or inline-assembly identifier of libc or the SDK, a raw
#      register address (a hex literal right after a pointer cast, the
#      cast possibly qualified or doubled, or an 8-digit literal in an
#      RP2040 register or XIP window 0x10-0x15/0x18/0x40/0x50/0xd0/0xe0;
#      exact forms and known false positives in tools/pure-rules.sh), or a section
#      attribute outside a file named ramfunc.h.
# The identifier rules come from tools/pure-rules.sh (story 4.0 D1), shared
# by all five purity gates; the self-test fails if this gate keeps a
# private banned_word_re.
# Scanned: *.c *.h.
#
# Usage: firmware/tools/check-proto-pure.sh [ROOT]
#   ROOT  directory or single file to scan (default: common/proto)
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
root="${1:-${script_dir}/../common/proto}"
[ -e "${root}" ] || { echo "check-proto-pure: no such file or directory: ${root}" >&2; usage; }

if [ -d "${root}" ]; then
    files="$(find "${root}" -type f \( -name '*.c' -o -name '*.h' \) -print | LC_ALL=C sort)"
else
    files="${root}"
fi

include_re='^[[:space:]]*#[[:space:]]*include'
allowed_re='^[[:space:]]*#[[:space:]]*include[[:space:]]*(<(stdint|stddef|stdbool|string|limits)\.h>|"ocb_proto/[A-Za-z0-9_]+\.h")[[:space:]]*(/[*/].*)?$'
banned_re='pico|hardware/|tusb|ocb_hw|ocb_version\.h'
# Identifiers (rule b): nb, banned_word_re, section_re, raw_addr_re and
# pure_ident_hits come from the shared rule file.
. "${script_dir}/pure-rules.sh"

found=0
count=0
while IFS= read -r f; do
    [ -n "${f}" ] || continue
    count=$((count + 1))
    hits="$( {
        grep -n -E "${include_re}" "${f}" | grep -v -E "^[0-9]+:${allowed_re#^}" || true
        grep -n -E "${include_re}" "${f}" | grep -E "${banned_re}" || true
        pure_ident_hits "${f}"
    } | LC_ALL=C sort -t: -k1,1n -u)"
    if [ -n "${hits}" ]; then
        found=1
        while IFS= read -r h; do
            printf '%s:%s\n' "${f}" "${h}"
        done <<<"${hits}"
    fi
done <<<"${files}"

if [ "${count}" -eq 0 ]; then
    echo "check-proto-pure: no .c or .h files under ${root}" >&2
    exit 1
fi
if [ "${found}" -ne 0 ]; then
    echo "check-proto-pure: ocb_proto must stay pure C99 (AD-2): allowed includes are <stdint.h> <stddef.h> <stdbool.h> <string.h> <limits.h> and \"ocb_proto/...\"; no heap, I/O, clock, sleep, multicore, process-exit, assembly, section or register-address identifiers" >&2
    exit 1
fi
echo "check-proto-pure: ok (${count} files)"
