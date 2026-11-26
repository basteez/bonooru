#!/usr/bin/env bash
# ocb_p2 purity check (AD-2, story 2.3 D16): the controller's P2 profile
# stays pure C99 that builds on the host, so it is tested and fuzzed in
# ctest. A copy of check-link-pure.sh with a narrower allow-list.
#  (a) every #include must be one of <stdint.h> <stddef.h> <stdbool.h>
#      <string.h> <limits.h>, "ocb_proto/<name>.h" or "ocb_p2/<name>.h"
#      (no ocb_core, no ocb_link: the profile reads only ocb_state);
#      an include line naming pico, hardware/, tusb, class/hid, ocb_hw or
#      ocb_version.h is refused in any form;
#  (b) no line (code or comment) may name a heap, I/O, clock, sleep,
#      multicore, process-exit (exit( abort( ...), file (fopen), strdup,
#      alloca or inline-assembly identifier of libc or the SDK, a raw
#      register address (a hex literal right after a pointer cast, the
#      cast possibly qualified or doubled, or an 8-digit literal in an
#      RP2040 register or XIP window 0x10-0x15/0x18/0x40/0x50/0xd0/0xe0;
#      exact forms and known false positives in tools/pure-rules.sh), or a section
#      attribute outside a file named ramfunc.h: the profile takes the time
#      as an argument, allocates nothing and does no I/O (the TinyUSB glue
#      lives in controller/platform/usb_p2.c);
#  (c) a CMakeLists.txt in the tree must not link or name an SDK, hardware,
#      board or USB library (pico_*, hardware_*, ocb_hw, tinyusb*, tusb)
#      outside a # comment; ocb_proto is allowed.
# The identifier rules come from tools/pure-rules.sh (story 4.0 D1), shared
# by all five purity gates; the self-test fails if this gate keeps a
# private banned_word_re.
# Scanned: *.c *.h *.cpp *.cc *.S *.inc (rules a and b) and CMakeLists.txt
# (rule c).
#
# Usage: firmware/tools/check-p2-pure.sh [ROOT]
#   ROOT  directory or single file to scan (default: controller/profile_p2)
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
root="${1:-${script_dir}/../controller/profile_p2}"
[ -e "${root}" ] || { echo "check-p2-pure: no such file or directory: ${root}" >&2; usage; }

if [ -d "${root}" ]; then
    files="$(find "${root}" -type f \( -name '*.c' -o -name '*.h' -o -name '*.cpp' -o -name '*.cc' \
        -o -name '*.S' -o -name '*.inc' -o -name 'CMakeLists.txt' \) -print | LC_ALL=C sort)"
else
    files="${root}"
fi

include_re='^[[:space:]]*#[[:space:]]*include'
allowed_re='^[[:space:]]*#[[:space:]]*include[[:space:]]*(<(stdint|stddef|stdbool|string|limits)\.h>|"ocb_(proto|p2)/[A-Za-z0-9_]+\.h")[[:space:]]*(/[*/].*)?$'
banned_include_re='pico|hardware/|tusb|class/hid|ocb_hw|ocb_core|ocb_link|ocb_version\.h'
# Identifiers (rule b): nb, banned_word_re, section_re, raw_addr_re and
# pure_ident_hits come from the shared rule file.
. "${script_dir}/pure-rules.sh"
# CMake: an SDK, hardware, board or USB library, or the core or the link,
# named on a non-comment line.
cmake_comment_re='^[0-9]+:[[:space:]]*#'
cmake_banned_re="${nb}(pico_|hardware_|ocb_hw|tinyusb|tusb|ocb_core|ocb_link)"

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
    echo "check-p2-pure: no source files under ${root}" >&2
    exit 1
fi
if [ "${found}" -ne 0 ]; then
    echo "check-p2-pure: ocb_p2 must stay pure C99 (AD-2): allowed includes are <stdint.h> <stddef.h> <stdbool.h> <string.h> <limits.h>, \"ocb_proto/...\" and \"ocb_p2/...\"; no heap, I/O, clock, sleep, multicore, process-exit, assembly, section or register-address identifiers; no SDK, hardware, board or USB library in CMakeLists.txt" >&2
    exit 1
fi
echo "check-p2-pure: ok (${count} files)"
