# Shared identifier rules of the five purity gates (AD-2, story 4.0 D1):
# check-proto-pure.sh, check-core-pure.sh, check-link-pure.sh,
# check-p2-pure.sh and check-bridge-pure.sh source this file, so the deny
# list exists once and cannot drift between gates. Each gate keeps its own
# include allow-list, banned includes and CMake rule.
#
# Sourced, not executed: it sets no shell option and prints nothing.
#
# The rules match code and comments alike, so a word that also appears in
# prose (exit, abort, section) is matched in call form only. POSIX ERE, no
# \b (story 2.4 D20): a word boundary is spelled out, so names like
# ocb_queue_free( or ocb_exit_code stay legal while a bare free( or exit(
# does not. Same behaviour with GNU, BSD and ugrep grep -E.

# A word boundary on the left.
nb='(^|[^A-Za-z0-9_])'

# Heap, I/O, clock, sleep, multicore, process exit and inline assembly.
banned_word_re="${nb}(malloc|calloc|realloc|free[[:space:]]*\\(|[a-z]*printf|time_us|get_absolute_time|clock_gettime|gettimeofday|sleep_|busy_wait|multicore_|mutex_|spin_lock|__not_in_flash_func|stdio_|(puts|putchar)([^A-Za-z0-9_]|\$)|(exit|_exit|_Exit|quick_exit|abort)[[:space:]]*\\(|(fopen|strdup|strndup|alloca|__builtin_alloca|asm|__asm|__asm__)([^A-Za-z0-9_]|\$))"

# A section attribute: legal only in a file named ramfunc.h, the hook of
# OCB_CORE_RAMFUNC (story 2.4 D12).
section_re="${nb}(section|__section__)[[:space:]]*\\("

# A raw register address: a hex literal right after a pointer cast, or an
# 8-digit literal in an RP2040 peripheral window (APB 0x40, AHB-Lite 0x50,
# SIO 0xd0, Cortex-M0+ PPB 0xe0; datasheet section 2.2). 0x80000000,
# 0xFFFFFFFFu and 0xEDB88320u stay legal.
raw_addr_re="\\*[[:space:]]*\\)[[:space:]]*\\(?[[:space:]]*0[xX][0-9A-Fa-f]+|${nb}0[xX][45dDeE]0[0-9A-Fa-f]{6}([uUlL]|[^0-9A-Za-z_]|\$)"

# pure_ident_hits FILE: print the grep -n hits (line:text) of every
# identifier rule for FILE, unsorted; the caller sorts and de-duplicates.
pure_ident_hits() {
    grep -n -E "${banned_word_re}" "$1" || true
    grep -n -E "${raw_addr_re}" "$1" || true
    if [ "$(basename "$1")" != "ramfunc.h" ]; then
        grep -n -E "${section_re}" "$1" || true
    fi
}
