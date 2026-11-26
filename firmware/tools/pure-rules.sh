# Shared identifier rules of the five purity gates (AD-2, story 4.0 D1):
# check-proto-pure.sh, check-core-pure.sh, check-link-pure.sh,
# check-p2-pure.sh and check-bridge-pure.sh source this file, so the deny
# list exists once and cannot drift between gates. Each gate keeps its own
# include allow-list, banned includes and CMake rule; its self-test fails if
# the gate stops calling pure_ident_hits, reassigns a rule below or defines
# its own pure_ident_hits.
#
# Sourced, not executed: it sets no shell option and prints nothing.
#
# The rules match code and comments alike, so a word that also appears in
# prose (exit, abort, section) is matched in call form only. POSIX ERE, no
# \b (story 2.4 D20): a word boundary is spelled out, so names like
# ocb_queue_free( or ocb_exit_code stay legal while a bare free( or exit(
# does not. Same behaviour with GNU, BSD and ugrep grep -E.
#
# Known false positives, accepted on purpose (story 4.0 review: the gates
# stay simple greps; none hits the real trees today). Each has a workaround:
#  - "abort (", "exit (" or "section (" in a comment, e.g. "on abort (D7)"
#    or "see section (2.2)": the call form allows a space before "(".
#    Workaround: reword the comment ("on an abort, D7", "see section 2.2").
#  - The word "asm" anywhere, comments included ("no asm here"). Workaround:
#    write "assembly" in prose.
#  - An 8-digit hex literal in a register window used as a plain mask or
#    value, e.g. 0x40000000u (bit 30), 0xE0000000u, 0x50000000,
#    0x10000000u (bit 28), the test pattern 0x12345678: the rule cannot
#    tell an address from a mask or a value.
#    Workaround: spell the mask as a shift, (1u << 30), or as a sum of
#    named bits.
# Known gaps (not matched): a cast through a pointer typedef with no "*"
# in the cast, e.g. (reg_ptr_t)0x20041000 (an address outside the
# windows below), and an address built at run time.

# A word boundary on the left.
nb='(^|[^A-Za-z0-9_])'

# Heap, I/O, clock, sleep, multicore, process exit and inline assembly.
# __builtin_alloca matches every builtin of the family
# (__builtin_alloca_with_align, __builtin_alloca_with_align_and_max).
banned_word_re="${nb}(malloc|calloc|realloc|free[[:space:]]*\\(|[a-z]*printf|time_us|get_absolute_time|clock_gettime|gettimeofday|sleep_|busy_wait|multicore_|mutex_|spin_lock|__not_in_flash_func|stdio_|(puts|putchar)([^A-Za-z0-9_]|\$)|(exit|_exit|_Exit|quick_exit|abort)[[:space:]]*\\(|(fopen|strdup|strndup|alloca|__builtin_alloca[A-Za-z0-9_]*|asm|__asm|__asm__)([^A-Za-z0-9_]|\$))"

# A section attribute: legal only in a file named ramfunc.h, the hook of
# OCB_CORE_RAMFUNC (story 2.4 D12).
section_re="${nb}(section|__section__)[[:space:]]*\\("

# A raw register address, either of:
#  (a) a hex literal right after a pointer cast: "(T *)0x...", with const /
#      volatile / restrict after the "*" ("(T * const)0x..."), with the
#      literal in parentheses ("(T *)(0x...)"), or through one more cast
#      ("(T *)(uintptr_t)0x...");
#  (b) an 8-digit hex literal (u / l suffix allowed) in an RP2040 register
#      or XIP window (datasheet section 2.2): XIP flash and its aliases
#      0x10..0x13, XIP_CTRL 0x14, XIP SRAM 0x15, XIP_SSI 0x18, APB 0x40,
#      AHB-Lite 0x50, SIO 0xd0, Cortex-M0+ PPB 0xe0. 0x80000000,
#      0xFFFFFFFFu, 0x1FFFFFFFu and 0xEDB88320u stay legal.
raw_addr_re="\\*([[:space:]]*(const|volatile|restrict))*[[:space:]]*\\)[[:space:]]*(\\([[:space:]]*[A-Za-z_][A-Za-z0-9_[:space:]]*\\)[[:space:]]*)?\\(?[[:space:]]*0[xX][0-9A-Fa-f]+|${nb}0[xX]([45dDeE]0|1[0-58])[0-9A-Fa-f]{6}([uUlL]|[^0-9A-Za-z_]|\$)"

# pure_ident_hits FILE: print the grep -n hits (line:text) of every
# identifier rule for FILE, unsorted; the caller sorts and de-duplicates.
pure_ident_hits() {
    grep -n -E "${banned_word_re}" "$1" || true
    grep -n -E "${raw_addr_re}" "$1" || true
    if [ "$(basename "$1")" != "ramfunc.h" ]; then
        grep -n -E "${section_re}" "$1" || true
    fi
}
