#!/usr/bin/env bash
# ocb_link purity check (AD-2, story 2.6 D16): the controller link layer
# stays pure C99 that builds on the host.
#  (a) every #include must be one of <stdint.h> <stddef.h> <stdbool.h>
#      <string.h> <limits.h>, "ocb_proto/<name>.h", "ocb_core/<name>.h" or
#      "ocb_link/<name>.h";
#      an include line naming pico, hardware/, tusb, ocb_hw or ocb_version.h
#      is refused in any form;
#  (b) no line (code or comment) may name a heap, I/O, clock, sleep,
#      multicore, process-exit (exit( abort( ...), file (fopen), strdup,
#      alloca or inline-assembly identifier of libc or the SDK, a raw
#      register address (a hex literal right after a pointer cast, the
#      cast possibly qualified or doubled, or an 8-digit literal in an
#      RP2040 register or XIP window 0x10-0x15/0x18/0x40/0x50/0xd0/0xe0;
#      exact forms and known false positives in tools/pure-rules.sh), or a section
#      attribute outside a file named ramfunc.h: the link takes the time as
#      an argument, allocates nothing and does no I/O (the UART glue lives in
#      ocb_hw and controller/platform). RAM placement goes
#      through the OCB_CORE_RAMFUNC macro, not the SDK one;
#  (c) a CMakeLists.txt in the tree must not link or name an SDK, hardware
#      or board library (pico_*, hardware_*, ocb_hw) outside a # comment;
#      ocb_core and ocb_proto are allowed.
# The identifier rules come from tools/pure-rules.sh (story 4.0 D1), shared
# by all five purity gates; the self-test fails if this gate keeps a
# private banned_word_re.
# Scanned: *.c *.h *.cpp *.cc *.S *.inc (rules a and b) and CMakeLists.txt
# (rule c).
#
# Usage: firmware/tools/check-link-pure.sh [ROOT]
#   ROOT  directory or single file to scan (default: controller/link)
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
root="${1:-${script_dir}/../controller/link}"
[ -e "${root}" ] || { echo "check-link-pure: no such file or directory: ${root}" >&2; usage; }

if [ -d "${root}" ]; then
    files="$(find "${root}" -type f \( -name '*.c' -o -name '*.h' -o -name '*.cpp' -o -name '*.cc' \
        -o -name '*.S' -o -name '*.inc' -o -name 'CMakeLists.txt' \) -print | LC_ALL=C sort)"
else
    files="${root}"
fi

include_re='^[[:space:]]*#[[:space:]]*include'
allowed_re='^[[:space:]]*#[[:space:]]*include[[:space:]]*(<(stdint|stddef|stdbool|string|limits)\.h>|"ocb_(proto|core|link)/[A-Za-z0-9_]+\.h")[[:space:]]*(/[*/].*)?$'
banned_include_re='pico|hardware/|tusb|ocb_hw|ocb_version\.h'
# Identifiers (rule b): nb, banned_word_re, section_re, raw_addr_re and
# pure_ident_hits come from the shared rule file.
. "${script_dir}/pure-rules.sh"
# CMake: an SDK, hardware or board library named on a non-comment line.
cmake_comment_re='^[0-9]+:[[:space:]]*#'
cmake_banned_re="${nb}(pico_|hardware_|ocb_hw)"

found=0
count=0
while IFS= read -r f; do
    [ -n "${f}" ] || continue
    count=$((count + 1))
    if [ "$(basename "${f}")" = "CMakeLists.txt" ]; then
        hits="$(grep -n -E "${cmake_banned_re}" "${f}" | grep -v -E "${cmake_comment_re}" || true)"
    else
        hits="$( {
            grep -n -E "${include_re}" "${f}" | grep -v -E "^[0-9]+:${allowed_re#^}" || true
            grep -n -E "${include_re}" "${f}" | grep -E "${banned_include_re}" || true
            pure_ident_hits "${f}"
        } | LC_ALL=C sort -t: -k1,1n -u)"
    fi
    if [ -n "${hits}" ]; then
        found=1
        while IFS= read -r h; do
            printf '%s:%s\n' "${f}" "${h}"
        done <<<"${hits}"
    fi
done <<<"${files}"

if [ "${count}" -eq 0 ]; then
    echo "check-link-pure: no source files under ${root}" >&2
    exit 1
fi
if [ "${found}" -ne 0 ]; then
    echo "check-link-pure: ocb_link must stay pure C99 (AD-2): allowed includes are <stdint.h> <stddef.h> <stdbool.h> <string.h> <limits.h>, \"ocb_proto/...\", \"ocb_core/...\" and \"ocb_link/...\"; no heap, I/O, clock, sleep, multicore, process-exit, assembly, section or register-address identifiers; no SDK, hardware or board library in CMakeLists.txt" >&2
    exit 1
fi
echo "check-link-pure: ok (${count} files)"
