#!/usr/bin/env bash
# The controller's hot path runs from RAM (story 2.5 D2, AD-11): with the
# default copy_to_ram build every function of the hot set below must have
# its address (arm-none-eabi-nm, the run address) in the RP2040 SRAM range
# 0x20000000..0x20041FFF (striped banks, SCRATCH_X, SCRATCH_Y), never in
# the XIP flash window (0x10000000..), where a cache miss stalls the core.
# The hot set: the scheduler and the core's frame handler, the link poll,
# RX drain and TX, the core 1 step, the published-state handoff, the core 0
# USB task and report builder, the liveness steps, the 64-bit timer read,
# TinyUSB's task, and the CRC, COBS and frame codec of ocb_proto.
# Every symbol must exist, so a rename cannot make the gate pass vacuously.
#
# Usage: firmware/tools/check-ram-hot-path.sh [--expect-flash] ELF
#   --expect-flash  self-test on the XIP variant (OCB_CONTROLLER_RUN_FROM_RAM
#                   =OFF, preset fw-release-xip): passes only when at least one
#                   hot function is in flash, i.e. when the RAM check would
#                   fail, so the check can tell the two builds apart.
#   nm: $OCB_ARM_TOOLCHAIN/bin/arm-none-eabi-nm when set, else PATH.
# Exit: 0 ok, 1 gate failed, 2 usage error.
set -euo pipefail

usage() {
    echo "usage: $0 [--expect-flash] ELF" >&2
    exit 2
}

HOT="ocb_core_tick ocb_core_frame ocb_link_poll ocb_link_rx_drain ocb_link_rx_entry ocb_link_tx_next
ocb_ctrl_link_step ocb_pubstate_read ocb_pubstate_publish ocb_ctrl_published_read usb_p2_task ocb_p2_next_in
liveness_core0_step liveness_core1_step time_us_64 tud_task_ext ocb_crc16 ocb_crc16_update ocb_cobs_decode
ocb_cobs_encode ocb_rx_push ocb_frame_decode ocb_frame_encode"
RAM_LO=$((0x20000000))
RAM_HI=$((0x20041FFF))

expect_flash=0
if [ "${1:-}" = "--expect-flash" ]; then
    expect_flash=1
    shift
fi
[ "$#" -eq 1 ] || usage
elf="$1"
[ -f "${elf}" ] || { echo "check-ram-hot-path: no such ELF: ${elf}" >&2; exit 2; }

if [ -n "${OCB_ARM_TOOLCHAIN:-}" ] && [ -x "${OCB_ARM_TOOLCHAIN}/bin/arm-none-eabi-nm" ]; then
    nm="${OCB_ARM_TOOLCHAIN}/bin/arm-none-eabi-nm"
elif command -v arm-none-eabi-nm >/dev/null 2>&1; then
    nm="arm-none-eabi-nm"
else
    echo "check-ram-hot-path: arm-none-eabi-nm not found (set OCB_ARM_TOOLCHAIN)" >&2
    exit 2
fi

table="$("${nm}" -n "${elf}")"
missing=0
in_flash=0
in_ram=0
for fn in ${HOT}; do
    addr="$(printf '%s\n' "${table}" | awk -v s="${fn}" '$3 == s && ($2 == "T" || $2 == "t") { print $1; exit }')"
    if [ -z "${addr}" ]; then
        echo "check-ram-hot-path: FAIL: ${fn} not found in ${elf} (renamed? update the hot set)" >&2
        missing=$((missing + 1))
        continue
    fi
    a=$((16#${addr}))
    if [ "${a}" -ge "${RAM_LO}" ] && [ "${a}" -le "${RAM_HI}" ]; then
        in_ram=$((in_ram + 1))
    else
        in_flash=$((in_flash + 1))
        [ "${expect_flash}" -eq 1 ] || echo "check-ram-hot-path: ${fn} at 0x${addr} is not in RAM" >&2
    fi
done

if [ "${missing}" -gt 0 ]; then
    exit 1
fi
if [ "${expect_flash}" -eq 1 ]; then
    if [ "${in_flash}" -eq 0 ]; then
        echo "check-ram-hot-path: self-test FAIL: every hot function is in RAM in ${elf} (not an XIP build?)" >&2
        exit 1
    fi
    echo "check-ram-hot-path: self-test ok, ${in_flash} hot functions in flash in the XIP build ${elf}"
    exit 0
fi
if [ "${in_flash}" -gt 0 ]; then
    echo "check-ram-hot-path: FAIL: ${in_flash} hot functions outside RAM (story 2.5 D2: copy_to_ram)" >&2
    exit 1
fi
echo "check-ram-hot-path: ok, ${in_ram} hot functions in RAM: ${elf}"
