#!/usr/bin/env bash
# ocb_bridge_core purity check (AD-2, Story 3.1): the bridge logic
# stays pure C99 that builds on the host.
#  (a) every #include must be one of <stdint.h> <stddef.h> <stdbool.h>
#      <string.h> <limits.h>, "ocb_proto/<name>.h" or
#      "ocb_bridge/<name>.h";
#      an include line naming pico, hardware/, tusb, class/ (TinyUSB),
#      ocb_hw, ocb_link, ocb_core or ocb_version.h is refused in any form;
#  (b) no line (code or comment) may name a heap, I/O, clock, sleep,
#      multicore, process-exit (exit( abort( ...), file (fopen), strdup,
#      alloca or inline-assembly identifier of libc or the SDK, a raw
#      register address (a hex literal cast to a pointer, or an RP2040
#      peripheral-window literal 0x40/0x50/0xd0/0xe0xxxxxx), or a section
#      attribute outside a file named ramfunc.h: the bridge core takes the time as
#      an argument, allocates nothing and does no I/O (USB, UART and alarm glue
#      live in bridge/platform);
#  (c) a CMakeLists.txt in the tree must not link or name an SDK, hardware
#      or board library (pico_*, hardware_*, tinyusb*, cmsis*, ocb_hw,
#      ocb_link, ocb_core) outside a # comment;
#      ocb_proto is allowed.
# The identifier rules come from tools/pure-rules.sh (story 4.0 D1), shared
# by all five purity gates; the self-test fails if this gate keeps a
# private banned_word_re.
# Scanned: *.c *.h *.cpp *.cc *.S *.inc (rules a and b) and CMakeLists.txt
# (rule c).
#
# Usage: firmware/tools/check-bridge-pure.sh [ROOT]
#   ROOT  directory or single file to scan (default: bridge/core)
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
root="${1:-${script_dir}/../bridge/core}"
[ -e "${root}" ] || { echo "check-bridge-pure: no such file or directory: ${root}" >&2; usage; }

if [ -d "${root}" ]; then
    files="$(find "${root}" -type f \( -name '*.c' -o -name '*.h' -o -name '*.cpp' -o -name '*.cc' \
        -o -name '*.S' -o -name '*.inc' -o -name 'CMakeLists.txt' \) -print | LC_ALL=C sort)"
else
    files="${root}"
fi

include_re='^[[:space:]]*#[[:space:]]*include'
allowed_re='^[[:space:]]*#[[:space:]]*include[[:space:]]*(<(stdint|stddef|stdbool|string|limits)\.h>|"ocb_(proto|bridge)/[A-Za-z0-9_]+\.h")[[:space:]]*(/[*/].*)?$'
banned_include_re='pico|hardware/|tusb|class/|ocb_hw|ocb_link|ocb_core|ocb_version\.h'
# Identifiers (rule b): nb, banned_word_re, section_re, raw_addr_re and
# pure_ident_hits come from the shared rule file.
. "${script_dir}/pure-rules.sh"
# CMake: an SDK, hardware or board library named on a non-comment line.
cmake_comment_re='^[0-9]+:[[:space:]]*#'
cmake_banned_re="${nb}(pico_|hardware_|ocb_hw|tinyusb|ocb_link|ocb_core|cmsis)"

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
    echo "check-bridge-pure: no source files under ${root}" >&2
    exit 1
fi
if [ "${found}" -ne 0 ]; then
    echo "check-bridge-pure: ocb_bridge_core must stay pure C99 (AD-2): allowed includes are <stdint.h> <stddef.h> <stdbool.h> <string.h> <limits.h>, \"ocb_proto/...\" and \"ocb_bridge/...\"; no heap, I/O, clock, sleep, multicore, process-exit, assembly, section or register-address identifiers; no SDK, hardware or board library in CMakeLists.txt" >&2
    exit 1
fi
echo "check-bridge-pure: ok (${count} files)"
