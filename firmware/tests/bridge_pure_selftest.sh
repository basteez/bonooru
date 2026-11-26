#!/usr/bin/env bash
# Self-test of check-bridge-pure.sh (Story 3.1, test F.40): every bad fixture
# must be reported, the good fixture and the real bridge/core tree must pass,
# a usage error exits 2, the gate must take its identifier rules from
# tools/pure-rules.sh (story 4.0 D1), and no source under firmware/bridge may define
# tud_cdc_line_coding_cb (AD-13: pico_usb_reset owns it).
# Usage: bridge_pure_selftest.sh CHECK_BRIDGE_PURE_SH FIRMWARE_DIR
set -euo pipefail

[ "$#" -eq 2 ] || { echo "usage: $0 CHECK_BRIDGE_PURE_SH FIRMWARE_DIR" >&2; exit 2; }
check="$1"
fw="$2"
fixtures="${fw}/tests/bridge_pure_fixtures"
fail=0
count=0

for f in "${fixtures}"/bad/*; do
    count=$((count + 1))
    rc=0
    out="$(bash "${check}" "${f}" 2>/dev/null)" || rc=$?
    if [ "${rc}" -ne 1 ] || [ -z "${out}" ]; then
        echo "FAIL: bad fixture not reported (rc=${rc}): ${f}"
        fail=1
    fi
done
[ "${count}" -gt 0 ] || { echo "FAIL: no bad fixtures found"; fail=1; }

if ! bash "${check}" "${fixtures}/good" >/dev/null; then
    echo "FAIL: good fixture reported"
    fail=1
fi

if ! bash "${check}" "${fw}/bridge/core"; then
    echo "FAIL: the real bridge/core tree is not pure"
    fail=1
fi

rc=0
bash "${check}" a b >/dev/null 2>&1 || rc=$?
[ "${rc}" -eq 2 ] || { echo "FAIL: usage error must exit 2 (got ${rc})"; fail=1; }

# AD-13: no own line-coding callback (a definition in C, not a comment).
if grep -rnE '^[^/*#]*tud_cdc_line_coding_cb[[:space:]]*\(' --include='*.c' --include='*.h' "${fw}/bridge"; then
    echo "FAIL: firmware/bridge defines tud_cdc_line_coding_cb (pico_usb_reset must own it)"
    fail=1
fi
# ... and the check itself catches one (red proof on a temporary copy).
tmp="$(mktemp -d)"
printf 'void tud_cdc_line_coding_cb(int itf, const void *c) { (void)itf; (void)c; }\n' > "${tmp}/x.c"
if ! grep -rqE '^[^/*#]*tud_cdc_line_coding_cb[[:space:]]*\(' --include='*.c' "${tmp}"; then
    echo "FAIL: the line-coding check does not see a definition"
    fail=1
fi
rm -rf "${tmp}"

# Shared-list guard (story 4.0 D1): the gate sources tools/pure-rules.sh
# and keeps no private banned_word_re that could drift from the others.
if ! grep -q -E '^[[:space:]]*\.[[:space:]]+"\$\{script_dir\}/pure-rules\.sh"' "${check}"; then
    echo "FAIL: $(basename "${check}") does not source tools/pure-rules.sh"
    fail=1
fi
if grep -q -E '^[[:space:]]*banned_word_re=' "${check}"; then
    echo "FAIL: $(basename "${check}") defines a private banned_word_re"
    fail=1
fi

[ "${fail}" -eq 0 ] && echo "bridge_pure_selftest: ok (${count} bad fixtures, rules from tools/pure-rules.sh)"
exit "${fail}"
