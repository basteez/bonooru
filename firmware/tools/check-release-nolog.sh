#!/usr/bin/env bash
# Release has no debug log (story 2.3 AC5a, D14): the P2 debug log (the SPSC
# ring, its push and drain, the firmware's ring accessor) exists only in
# debug builds. This gate lists the symbols of a controller ELF with
# arm-none-eabi-nm and fails if any debug-log symbol is present.
# The second mode is the gate's own self-test, run in CI on the debug ELF:
# --expect-present fails if the symbols are NOT found, so a rename of the
# debug log cannot make the release check pass vacuously.
#
# Usage: firmware/tools/check-release-nolog.sh [--expect-present] ELF
#   nm:  $OCB_ARM_TOOLCHAIN/bin/arm-none-eabi-nm when OCB_ARM_TOOLCHAIN is
#        set, else arm-none-eabi-nm from PATH.
# Exit: 0 ok, 1 gate failed, 2 usage error (missing ELF or nm).
set -euo pipefail

usage() {
    echo "usage: $0 [--expect-present] ELF" >&2
    exit 2
}

expect_present=0
if [ "${1:-}" = "--expect-present" ]; then
    expect_present=1
    shift
fi
[ "$#" -eq 1 ] || usage
elf="$1"
[ -f "${elf}" ] || { echo "check-release-nolog: no such ELF: ${elf}" >&2; exit 2; }

if [ -n "${OCB_ARM_TOOLCHAIN:-}" ] && [ -x "${OCB_ARM_TOOLCHAIN}/bin/arm-none-eabi-nm" ]; then
    nm="${OCB_ARM_TOOLCHAIN}/bin/arm-none-eabi-nm"
elif command -v arm-none-eabi-nm >/dev/null 2>&1; then
    nm="arm-none-eabi-nm"
else
    echo "check-release-nolog: arm-none-eabi-nm not found (set OCB_ARM_TOOLCHAIN)" >&2
    exit 2
fi

# The debug log's symbols: every ocb_dbglog_* function and the firmware's
# ring accessor ocb_ctrl_dbglog.
pattern='(^|[^A-Za-z0-9_])(ocb_dbglog_[A-Za-z0-9_]*|ocb_ctrl_dbglog)$'
all="$("${nm}" "${elf}")"
hits="$(printf '%s\n' "${all}" | grep -E "${pattern}" || true)"

if [ "${expect_present}" -eq 1 ]; then
    if [ -z "${hits}" ]; then
        echo "check-release-nolog: FAIL: no debug-log symbol in ${elf} (expected in a debug build; renamed?)" >&2
        exit 1
    fi
    echo "check-release-nolog: ok, debug build carries the debug log ($(printf '%s\n' "${hits}" | wc -l | tr -d ' ') symbols): ${elf}"
    exit 0
fi
if [ -n "${hits}" ]; then
    printf '%s\n' "${hits}"
    echo "check-release-nolog: FAIL: debug-log symbols in ${elf} (AC5a: release builds have no debug log)" >&2
    exit 1
fi
echo "check-release-nolog: ok, no debug-log symbol in ${elf}"
