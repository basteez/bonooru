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
        continue
    fi
    # Story 4.0 review: the fixture must be flagged on a code line, not only
    # on its comment (a comment word can satisfy another rule).
    code_hits="$(printf '%s\n' "${out}" | sed -E 's/^[^:]*:[0-9]+://' | grep -v -E '^[[:space:]]*(/\*|//|\*([[:space:]]|/|$))' || true)"
    if [ -z "${code_hits}" ]; then
        echo "FAIL: bad fixture flagged only on a comment line: ${f}"
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

# Shared-list guard (story 4.0 D1, tightened by the review): the gate
# sources tools/pure-rules.sh, applies it to every scanned file through
# pure_ident_hits, and neither reassigns a shared rule (nb, banned_word_re,
# section_re, raw_addr_re, with = or +=, also behind readonly / declare /
# export / local) nor defines its own pure_ident_hits. Comment lines are
# ignored.
gate_code="$(grep -v -E '^[[:space:]]*#' "${check}" || true)"
if ! printf '%s\n' "${gate_code}" | grep -q -E '^[[:space:]]*\.[[:space:]]+"\$\{script_dir\}/pure-rules\.sh"'; then
    echo "FAIL: $(basename "${check}") does not source tools/pure-rules.sh"
    fail=1
fi
if ! printf '%s\n' "${gate_code}" | grep -q -E '(^|[^A-Za-z0-9_])pure_ident_hits[[:space:]]+"\$\{f\}"'; then
    echo "FAIL: $(basename "${check}") does not call pure_ident_hits \"\${f}\""
    fail=1
fi
if printf '%s\n' "${gate_code}" | grep -q -E '(^|[^A-Za-z0-9_])(nb|banned_word_re|section_re|raw_addr_re)\+?='; then
    echo "FAIL: $(basename "${check}") assigns a shared rule (nb, banned_word_re, section_re or raw_addr_re)"
    fail=1
fi
if printf '%s\n' "${gate_code}" | grep -q -E '(^|[^A-Za-z0-9_])(function[[:space:]]+pure_ident_hits|pure_ident_hits[[:space:]]*\(\))'; then
    echo "FAIL: $(basename "${check}") defines its own pure_ident_hits"
    fail=1
fi

[ "${fail}" -eq 0 ] && echo "bridge_pure_selftest: ok (${count} bad fixtures, rules from tools/pure-rules.sh)"
exit "${fail}"
