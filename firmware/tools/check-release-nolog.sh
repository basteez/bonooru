#!/usr/bin/env bash
# Release has no debug-only code (story 2.3 AC5a, D14; story 2.5 Task 8.3):
# lists the symbols of a controller ELF with arm-none-eabi-nm and fails if a
# symbol of any debug-only family is present:
#   dbglog  the P2 debug log (every ocb_dbglog_* function, the firmware's
#           ring accessor ocb_ctrl_dbglog), story 2.3;
#   testtl  the built-in test melody and its loader (ocb_testtl_*), story 2.5;
#   hang    the hang injectors (debug_hang_*), story 2.5.
# The second mode is the gate's own self-test, run in CI on the debug ELFs:
# --expect-present[=FAMILY,...] fails unless EVERY named family is found
# (default: dbglog), so a rename cannot make the release check pass
# vacuously.
#
# Usage: firmware/tools/check-release-nolog.sh [--expect-present[=dbglog,testtl,hang]] ELF
#   nm:  $OCB_ARM_TOOLCHAIN/bin/arm-none-eabi-nm when OCB_ARM_TOOLCHAIN is
#        set, else arm-none-eabi-nm from PATH.
# Exit: 0 ok, 1 gate failed, 2 usage error (missing ELF or nm, unknown family).
set -euo pipefail

usage() {
    echo "usage: $0 [--expect-present[=dbglog,testtl,hang]] ELF" >&2
    exit 2
}

nb='(^|[^A-Za-z0-9_])'
family_re() {
    case "$1" in
        dbglog) printf '%s' "${nb}(ocb_dbglog_[A-Za-z0-9_]*|ocb_ctrl_dbglog)\$" ;;
        testtl) printf '%s' "${nb}ocb_testtl_[A-Za-z0-9_]*\$" ;;
        hang) printf '%s' "${nb}debug_hang_[A-Za-z0-9_]*\$" ;;
        *) echo "check-release-nolog: unknown family '$1' (dbglog, testtl, hang)" >&2; exit 2 ;;
    esac
}

expect_present=0
families="dbglog"
case "${1:-}" in
    --expect-present) expect_present=1; shift ;;
    --expect-present=*) expect_present=1; families="${1#--expect-present=}"; shift ;;
esac
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

all="$("${nm}" "${elf}")"

if [ "${expect_present}" -eq 1 ]; then
    IFS=',' read -r -a fams <<<"${families}"
    total=0
    for fam in "${fams[@]}"; do
        re="$(family_re "${fam}")"
        n="$(printf '%s\n' "${all}" | grep -cE "${re}" || true)"
        if [ "${n}" -eq 0 ]; then
            echo "check-release-nolog: FAIL: no ${fam} symbol in ${elf} (expected in this debug build; renamed?)" >&2
            exit 1
        fi
        total=$((total + n))
    done
    echo "check-release-nolog: ok, debug build carries ${families} (${total} symbols): ${elf}"
    exit 0
fi

hits=""
for fam in dbglog testtl hang; do
    re="$(family_re "${fam}")"
    h="$(printf '%s\n' "${all}" | grep -E "${re}" || true)"
    if [ -n "${h}" ]; then
        hits="${hits}${h}"$'\n'
    fi
done
if [ -n "${hits}" ]; then
    printf '%s' "${hits}"
    echo "check-release-nolog: FAIL: debug-only symbols in ${elf} (release builds have no debug log, test melody or hang injector)" >&2
    exit 1
fi
echo "check-release-nolog: ok, no debug-only symbol (debug log, test melody, hang injector) in ${elf}"
