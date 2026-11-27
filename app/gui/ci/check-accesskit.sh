#!/usr/bin/env bash
# G.8 (Story 4.3 AC 15, Story 4.2 forward note): eframe's `accesskit`
# feature must be enabled by ocb-app itself in the normal (non-dev)
# dependency graph, for each desktop target. egui_kittest's dev feature
# turns it back on in test builds, so `cargo test` cannot see it missing:
# this check reads `cargo tree -e normal,features` instead.
#
# The tree is read with `--color never --prefix depth` (CI forces colour
# with CARGO_TERM_COLOR=always, and ANSI codes broke the first version's
# grep; ANSI escapes are also stripped defensively). The check passes when
# a line `<d>eframe feature "accesskit"` has, anywhere in its subtree (the
# following lines deeper than d), a direct dependent `<d+1>ocb-app v...`.
#
# Usage: check-accesskit.sh [target...]   (run from app/)
#        check-accesskit.sh --self-test   (the parser on fixtures)
#   default targets: x86_64-unknown-linux-gnu aarch64-apple-darwin
#                    x86_64-pc-windows-msvc
# Exit 0 if every target has it, 1 otherwise (with ::error:: annotations).
set -u

# Reads a `--prefix depth` tree on stdin; exit 0 if ocb-app directly
# enables eframe's accesskit feature.
has_enabler() {
  local esc
  esc=$(printf '\033')
  sed "s/${esc}\[[0-9;]*m//g" | awk '
    /^[0-9]+/ {
      match($0, /^[0-9]+/)
      d = substr($0, 1, RLENGTH) + 0
      rest = substr($0, RLENGTH + 1)
      if (in_feature && d <= fd) in_feature = 0
      if (in_feature && d == fd + 1 && rest ~ /^ocb-app v/) found = 1
      if (rest ~ /^eframe feature "accesskit"/) { in_feature = 1; fd = d }
    }
    END { exit found ? 0 : 1 }'
}

self_test() {
  local fail=0
  ok() { if printf '%b' "$2" | has_enabler; then :; else echo "self-test: expected pass: $1"; fail=1; fi; }
  ko() { if printf '%b' "$2" | has_enabler; then echo "self-test: expected fail: $1"; fail=1; fi; }
  ok "direct" '0eframe v0.37.0\n1eframe feature "accesskit"\n2ocb-app v0.0.0 (/x)\n'
  ok "another enabler sorted first" '0eframe v0.37.0\n1eframe feature "accesskit"\n2eframe feature "default"\n3zzz v1\n2ocb-app v0.0.0 (/x)\n'
  ok "ANSI colour" '\x1b[2m0\x1b[0meframe v0.37.0\n1eframe feature "accesskit"\n\x1b[2m2\x1b[0mocb-app v0.0.0 (/x)\n'
  ko "missing" '0eframe v0.37.0\n1eframe feature "clipboard"\n2ocb-app v0.0.0 (/x)\n'
  ko "only through a dev crate" '0eframe v0.37.0\n1eframe feature "accesskit"\n2egui_kittest feature "eframe"\n'
  ko "ocb-app deeper, not a direct dependent" '0eframe v0.37.0\n1eframe feature "accesskit"\n2egui_kittest feature "eframe"\n3ocb-app v0.0.0 (/x)\n'
  ko "ocb-app under the next feature" '0eframe v0.37.0\n1eframe feature "accesskit"\n2zzz v1\n1eframe feature "x11"\n2ocb-app v0.0.0 (/x)\n'
  [ "$fail" -eq 0 ] && echo "self-test ok"
  return "$fail"
}

if [ "${1:-}" = "--self-test" ]; then
  self_test
  exit $?
fi

targets=("$@")
if [ "${#targets[@]}" -eq 0 ]; then
  targets=(x86_64-unknown-linux-gnu aarch64-apple-darwin x86_64-pc-windows-msvc)
fi
self_test >/dev/null || { echo "::error::check-accesskit.sh parser self-test failed"; exit 1; }
status=0
for t in "${targets[@]}"; do
  tree=$(cargo tree --locked --color never --prefix depth -p ocb-app -e normal,features -i eframe --target "$t") || {
    echo "::error::cargo tree failed for $t"
    status=1
    continue
  }
  echo "== $t"
  echo "$tree"
  if printf '%s\n' "$tree" | has_enabler; then
    echo "ok: ocb-app enables eframe/accesskit for $t"
  else
    echo "::error::eframe's accesskit feature is not enabled by ocb-app for $t (Story 4.2 basic screen-reader level)"
    status=1
  fi
done
exit "$status"
