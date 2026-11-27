#!/usr/bin/env bash
# G.8 (Story 4.3 AC 15, Story 4.2 forward note): eframe's `accesskit`
# feature must be enabled by ocb-app itself in the normal (non-dev)
# dependency graph, for each desktop target. egui_kittest's dev feature
# turns it back on in test builds, so `cargo test` cannot see it missing:
# this check reads `cargo tree -e normal,features` instead, where a line
# `eframe feature "accesskit"` must be directly followed by the line of the
# crate that enables it, `ocb-app v...`.
#
# Usage: check-accesskit.sh [target...]   (run from app/)
#   default targets: x86_64-unknown-linux-gnu aarch64-apple-darwin
#                    x86_64-pc-windows-msvc
# Exit 0 if every target has it, 1 otherwise (with ::error:: annotations).
set -u
targets=("$@")
if [ "${#targets[@]}" -eq 0 ]; then
  targets=(x86_64-unknown-linux-gnu aarch64-apple-darwin x86_64-pc-windows-msvc)
fi
status=0
for t in "${targets[@]}"; do
  tree=$(cargo tree --locked -p ocb-app -e normal,features -i eframe --target "$t") || {
    echo "::error::cargo tree failed for $t"
    status=1
    continue
  }
  echo "== $t"
  echo "$tree"
  if echo "$tree" | grep -A1 -F 'eframe feature "accesskit"' | grep -qE '[└├]── ocb-app v'; then
    echo "ok: ocb-app enables eframe/accesskit for $t"
  else
    echo "::error::eframe's accesskit feature is not enabled by ocb-app for $t (Story 4.2 basic screen-reader level)"
    status=1
  fi
done
exit "$status"
