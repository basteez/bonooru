#!/usr/bin/env bash
# Story 4.4 (probe-artifacts job): the Linux ocb-probe binary links only
# what every glibc + systemd distro has: libudev.so.1 (serialport's port
# enumeration), libc, libm, libgcc_s, the vDSO and the loader. Anything
# else, or a "not found", fails: the owner's machine might not have it.
#
# Usage: check-ldd.sh <file with the output of `ldd ocb-probe`>
# Exit 0 if every line is allowed, 1 otherwise (::error:: annotation),
# 2 on a usage error.
set -u
if [ "$#" -ne 1 ] || [ ! -r "$1" ]; then
  echo "usage: $0 <ldd-output-file>" >&2
  exit 2
fi
file=$1
if grep -q 'not found' "$file"; then
  echo "::error::ldd: missing library: $(grep 'not found' "$file" | tr -s ' \t' ' ')"
  exit 1
fi
allowed='^[[:space:]]*(linux-vdso\.so\.1|linux-gate\.so\.1|libudev\.so\.1|libc\.so\.6|libm\.so\.6|libgcc_s\.so\.1|/lib[^ ]*/ld-linux[^ ]*\.so\.[0-9]+|ld-linux[^ ]*\.so\.[0-9]+)([[:space:]]|$)'
extra=$(grep -vE "$allowed" "$file" | grep -v '^[[:space:]]*$')
if [ -n "$extra" ]; then
  echo "::error::ldd: library not on the allowed list: $(echo "$extra" | tr -s ' \t' ' ' | tr '\n' ';')"
  exit 1
fi
grep -q 'libudev\.so\.1' "$file" || { echo "::error::ldd: libudev.so.1 not linked (serialport without libudev?)"; exit 1; }
echo "ldd check ok: $(grep -c . "$file") lines, all allowed"
