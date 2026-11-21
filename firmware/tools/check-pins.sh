#!/usr/bin/env bash
# Pin grep (AD-20, Story 2.1 Decision D6): fail if a GPIO number appears in
# firmware sources outside firmware/common/hw/include/ocb_hw/pins.h.
#
# Usage: firmware/tools/check-pins.sh [ROOT]
#   ROOT  directory or single file to scan (default: the firmware/ directory)
# Exit:   0 no violation, 1 violation(s) found (printed as file:line:text),
#         2 usage error.
#
# Portable POSIX `grep -E` only, same behaviour with GNU grep and BSD grep.
# Known limit: a pin number hidden behind an unrelated macro is not seen.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
    echo "usage: $0 [ROOT]" >&2
    exit 2
}

[ "$#" -le 1 ] || usage
root="${1:-${script_dir}/..}"
[ -e "${root}" ] || { echo "check-pins: no such file or directory: ${root}" >&2; usage; }

# Rule 1: GP<n>, GPIO<n>, GP_<n>, GPIO_<n>, "gpio <n>" (case-insensitive, code or comments).
rule1='(^|[^A-Za-z0-9])(gpio|gp)(_|[[:space:]]+)?[0-9]{1,2}([^0-9]|$)'
# Rule 2: a gpio_*( call whose first argument is a numeric literal.
rule2='(^|[^A-Za-z0-9_])gpio_[a-z0-9_]*[[:space:]]*\([[:space:]]*[0-9]'
# Rule 3: a numeric literal in the pin position of PIO pin-taking helpers.
a='[^,()]*,[[:space:]]*'
rule3="(pio_gpio_init[[:space:]]*\\(${a}[0-9])|(pio_sm_set_consecutive_pindirs[[:space:]]*\\(${a}${a}[0-9])|(sm_config_set_(in|out|set|sideset|jmp)_pins[[:space:]]*\\(${a}[0-9])|(ws2812_program_init[[:space:]]*\\(${a}${a}${a}[0-9])"
# Rule 4: SDK board default pin macros.
rule4='PICO_DEFAULT_[A-Z0-9_]*(PIN|UART|I2C|SPI|WS2812)|PICO_[A-Z0-9_]*_PINS?([^A-Za-z0-9_]|$)'

if [ -d "${root}" ]; then
    root="$(cd "${root}" && pwd)"
    files="$(find "${root}" \
        \( -path "${root}/external" -o -path "${root}/third_party" \
           -o -path "${root}/build" -o -path "${root}/build-*" -o -path "${root}/.tools" \
           -o -path "${root}/tests/pin_check_fixtures" -o -path "${root}/common/proto/vectors" \) -prune \
        -o -type f \( -name '*.c' -o -name '*.h' -o -name '*.cpp' -o -name '*.hpp' -o -name '*.S' \
           -o -name '*.pio' -o -name '*.cmake' -o -name '*.in' -o -name '*.json' -o -name 'CMakeLists.txt' \) \
        ! -path "${root}/common/hw/include/ocb_hw/pins.h" -print | LC_ALL=C sort)"
else
    files="${root}"
fi

found=0
while IFS= read -r f; do
    [ -n "${f}" ] || continue
    hits="$( {
        grep -n -i -E "${rule1}" "${f}" || true
        grep -n -E "${rule2}" "${f}" || true
        grep -n -E "${rule3}" "${f}" || true
        grep -n -E "${rule4}" "${f}" || true
    } | LC_ALL=C sort -t: -k1,1n -u)"
    if [ -n "${hits}" ]; then
        found=1
        while IFS= read -r h; do
            printf '%s:%s\n' "${f}" "${h}"
        done <<<"${hits}"
    fi
done <<<"${files}"

if [ "${found}" -ne 0 ]; then
    echo "check-pins: GPIO numbers found outside ocb_hw/pins.h (AD-20); use OCB_PIN_* instead." >&2
    exit 1
fi
echo "check-pins: ok"
