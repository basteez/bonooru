#!/usr/bin/env bash
# The cross-core barrier is real on the target (story 2.5 D5, D24): the
# published-state handoff of the controller (ocb_link/pubstate.c) must hold
# at least two `dmb sy` instructions in the writer (ocb_pubstate_publish
# and the write helpers it calls: before and after the slot) and in the
# reader (ocb_pubstate_read and its try helpers: after the first and before
# the second generation load), and no libcall standing in for a fence
# (__atomic_*, __sync_*). OCB_LINK_BARRIER() comes from
# controller/platform/ocb_link_barrier.h, force-included in the firmware
# builds; without it the macro is empty and this gate fails.
#
# Usage: firmware/tools/check-barrier.sh ELF|OBJECT
#        firmware/tools/check-barrier.sh --self-test
#   --self-test compiles controller/link/pubstate.c with the pinned
#   toolchain twice, into a temporary directory: without the force-include
#   the check must FAIL, with it the check must pass (so the gate can
#   neither pass vacuously nor depend on anything but the definition).
#   objdump/gcc: $OCB_ARM_TOOLCHAIN/bin/arm-none-eabi-* when set, else PATH.
# Exit: 0 ok, 1 gate failed, 2 usage error.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fw="$(cd "${script_dir}/.." && pwd)"

tool() {
    if [ -n "${OCB_ARM_TOOLCHAIN:-}" ] && [ -x "${OCB_ARM_TOOLCHAIN}/bin/arm-none-eabi-$1" ]; then
        printf '%s' "${OCB_ARM_TOOLCHAIN}/bin/arm-none-eabi-$1"
    elif command -v "arm-none-eabi-$1" >/dev/null 2>&1; then
        printf '%s' "arm-none-eabi-$1"
    else
        echo "check-barrier: arm-none-eabi-$1 not found (set OCB_ARM_TOOLCHAIN)" >&2
        exit 2
    fi
}

# count FILE FN...: the `dmb sy` in the named functions that exist (the
# helpers are inlined at -O3 and separate at -Og); 255 when the first one
# (the entry point) is missing. Fails on a fence libcall in any of them.
count() {
    local file="$1" objdump fn dis total=0 n bad first=1
    shift
    objdump="$(tool objdump)"
    for fn in "$@"; do
        dis="$("${objdump}" -d --disassemble="${fn}" "${file}" 2>/dev/null || true)"
        if ! printf '%s\n' "${dis}" | grep -q "<${fn}>:"; then
            if [ "${first}" -eq 1 ]; then
                echo "check-barrier: FAIL: ${fn} not found in ${file} (renamed?)" >&2
                echo 255
                return 0
            fi
            continue
        fi
        first=0
        n="$(printf '%s\n' "${dis}" | grep -cE '[[:space:]]dmb[[:space:]]+sy' || true)"
        bad="$(printf '%s\n' "${dis}" | grep -E 'bl[[:space:]].*<__(atomic|sync)_' || true)"
        if [ -n "${bad}" ]; then
            echo "check-barrier: FAIL: ${fn} calls a fence libcall: ${bad}" >&2
            echo 254
            return 0
        fi
        total=$((total + n))
    done
    echo "${total}"
}

# check FILE: 0 when the writer and the reader each carry >= 2 `dmb sy`.
check() {
    local file="$1" rc=0 w r
    w="$(count "${file}" ocb_pubstate_publish ocb_pubstate_write ocb_pubstate_write_begin ocb_pubstate_write_end)"
    r="$(count "${file}" ocb_pubstate_read ocb_pubstate_try_begin ocb_pubstate_try_end)"
    for pair in "writer:${w}" "reader:${r}"; do
        local who="${pair%%:*}" n="${pair#*:}"
        if [ "${n}" -ge 254 ]; then
            rc=1
        elif [ "${n}" -lt 2 ]; then
            echo "check-barrier: FAIL: the ${who} has ${n} 'dmb sy' (wanted >= 2) in ${file}" >&2
            rc=1
        else
            echo "check-barrier: ${who}: ${n} x dmb sy"
        fi
    done
    return "${rc}"
}

if [ "${1:-}" = "--self-test" ]; then
    [ "$#" -eq 1 ] || { echo "usage: $0 --self-test" >&2; exit 2; }
    gcc="$(tool gcc)"
    tmp="$(mktemp -d)"
    trap 'rm -rf "${tmp}"' EXIT
    flags=(-mcpu=cortex-m0plus -mthumb -O3 -std=c99 -ffunction-sections -c
        -I"${fw}/controller/link/include" -I"${fw}/controller/core/include" -I"${fw}/common/proto/include")
    "${gcc}" "${flags[@]}" "${fw}/controller/link/pubstate.c" -o "${tmp}/without.o"
    "${gcc}" "${flags[@]}" -include "${fw}/controller/platform/ocb_link_barrier.h" \
        "${fw}/controller/link/pubstate.c" -o "${tmp}/with.o"
    if check "${tmp}/without.o" >/dev/null 2>&1; then
        echo "check-barrier: self-test FAIL: the check passed on a build without the barrier definition" >&2
        exit 1
    fi
    if ! check "${tmp}/with.o"; then
        echo "check-barrier: self-test FAIL: the check failed on a build with the barrier definition" >&2
        exit 1
    fi
    echo "check-barrier: self-test ok (fails without the definition, passes with it)"
    exit 0
fi

[ "$#" -eq 1 ] || { echo "usage: $0 ELF|OBJECT | --self-test" >&2; exit 2; }
[ -f "$1" ] || { echo "check-barrier: no such file: $1" >&2; exit 2; }
if check "$1"; then
    echo "check-barrier: ok, the published-state handoff carries real barriers: $1"
    exit 0
fi
echo "check-barrier: FAIL: the cross-core barrier is missing (story 2.5 D5: controller/platform/ocb_link_barrier.h force-included?)" >&2
exit 1
