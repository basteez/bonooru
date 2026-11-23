#!/usr/bin/env bash
# Core 1 stack, static lower bound (story 2.5 D15, Task 12.2; issue #21).
# The controller builds compile our sources and the SDK sources of the
# controller with -fstack-usage -fcallgraph-info=su (controller/
# CMakeLists.txt), so every object has a .ci call graph with the static
# frame of each function. This gate walks the graph from core 1's entry
# (main.c core1_entry) and from the UART RX interrupt handler that runs on
# core 1's stack (common/hw/uart_link.c on_uart_rx, which calls the
# link runner's rx_sink through a pointer: that edge is added by name),
# takes the deepest chain of each, adds the 32-byte exception frame the hardware stacks for
# the interrupt, and fails when the sum is above the limit (1536 bytes,
# issue #21's threshold, D15). It prints both chains.
#
# A lower bound, never the truth: calls through a function pointer, library
# code compiled without the flag (newlib's memcpy/memset) and the SDK's
# launch words are not counted (they are listed); a dynamic frame or a
# recursion fails the gate. The hardware high-water mark of the debug
# STATUS v4 (stack_used_max) is the measurement; this gate keeps a later
# change from silently deepening the chain.
#
# Usage: firmware/tools/check-stack.sh [--limit BYTES] BUILD_DIR
#   BUILD_DIR  a configured and built controller preset, e.g.
#              firmware/build/fw-debug-timeline
# Exit: 0 ok, 1 over the limit or an unsound graph, 2 usage error.
set -euo pipefail

limit=1536
if [ "${1:-}" = "--limit" ]; then
    [ "$#" -ge 2 ] || { echo "usage: $0 [--limit BYTES] BUILD_DIR" >&2; exit 2; }
    limit="$2"
    shift 2
fi
[ "$#" -eq 1 ] || { echo "usage: $0 [--limit BYTES] BUILD_DIR" >&2; exit 2; }
build="$1"
[ -d "${build}/controller" ] || { echo "check-stack: no controller build in ${build}" >&2; exit 2; }
command -v python3 >/dev/null 2>&1 || { echo "check-stack: python3 not found" >&2; exit 2; }

python3 -I - "${build}" "${limit}" <<'PY'
import os
import re
import sys

build, limit = sys.argv[1], int(sys.argv[2])
EXC_FRAME = 32  # r0-r3, r12, lr, pc, xpsr stacked by the Cortex-M0+ on interrupt entry

node_re = re.compile(r'^node: \{ title: "([^"]+)" label: "([^"]*)"')
edge_re = re.compile(r'^edge: \{ sourcename: "([^"]+)" targetname: "([^"]+)"')
frame_re = re.compile(r'\\n(\d+) bytes \((static|dynamic[^)]*)\)')

frames = {}   # title -> bytes
dynamic = set()
edges = {}
files = 0
for root, _, names in os.walk(build):
    for n in names:
        if not n.endswith(".ci"):
            continue
        files += 1
        with open(os.path.join(root, n), "r", encoding="utf-8", errors="replace") as f:
            for line in f:
                m = node_re.match(line)
                if m:
                    fm = frame_re.search(m.group(2))
                    if fm:
                        frames[m.group(1)] = int(fm.group(1))
                        if fm.group(2) != "static":
                            dynamic.add(m.group(1))
                    continue
                m = edge_re.match(line)
                if m:
                    edges.setdefault(m.group(1), set()).add(m.group(2))
if files == 0:
    print("check-stack: FAIL: no .ci file under %s (built without -fcallgraph-info=su?)" % build)
    sys.exit(1)

# Calls through a function pointer whose target is known: the UART RX
# interrupt calls the sink installed by ocb_ctrl_link_start_rx (rx_sink).
KNOWN_INDIRECT = (("hw/uart_link.c:on_uart_rx", "platform/link_runner.c:rx_sink"),)


def find(suffix):
    hits = [t for t in frames if t == suffix or t.endswith("/" + suffix) or t.endswith(":" + suffix.split(":")[-1])
            and t.split(":")[0].endswith(suffix.split(":")[0])]
    return hits[0] if len(hits) == 1 else None


unknown = set()
memo = {}
errors = []


def deepest(t, stack):
    if t in memo:
        return memo[t]
    if t in stack:
        errors.append("recursion through %s" % t)
        return (0, [])
    if t not in frames:
        unknown.add(t)
        return (0, [])
    if t in dynamic:
        errors.append("dynamic stack frame in %s" % t)
    best = (0, [])
    for c in sorted(edges.get(t, ())):
        d = deepest(c, stack | {t})
        if d[0] > best[0]:
            best = d
    r = (frames[t] + best[0], [t] + best[1])
    memo[t] = r
    return r


def short(t):
    return t.split("/")[-1]


for src, dst in KNOWN_INDIRECT:
    a, b = find(src), find(dst)
    if a is None or b is None:
        print("check-stack: FAIL: known indirect call %s -> %s not in the call graph (renamed?)" % (src, dst))
        sys.exit(1)
    edges.setdefault(a, set()).add(b)

roots = {}
for name, suffix in (("core 1 loop", "controller/main.c:core1_entry"), ("UART RX interrupt", "hw/uart_link.c:on_uart_rx")):
    t = find(suffix)
    if t is None:
        print("check-stack: FAIL: %s root %s not found in the call graph (renamed?)" % (name, suffix))
        sys.exit(1)
    roots[name] = deepest(t, set())

total = roots["core 1 loop"][0] + roots["UART RX interrupt"][0] + EXC_FRAME
for name, (b, chain) in roots.items():
    print("check-stack: %s: %d bytes: %s" % (name, b, " -> ".join("%s(%d)" % (short(c), frames[c]) for c in chain)))
print("check-stack: + %d bytes of exception frame = %d bytes (limit %d)" % (EXC_FRAME, total, limit))
skipped = sorted(u for u in unknown if not u.startswith("__builtin"))
if skipped:
    print("check-stack: not counted (no frame information: library, indirect or inlined): %s" % ", ".join(
        short(u) for u in skipped))
for e in errors:
    print("check-stack: FAIL: %s" % e)
if errors:
    sys.exit(1)
if total > limit:
    print("check-stack: FAIL: the static core 1 chain (%d bytes) is above %d bytes (story 2.5 D15: raise "
          "PICO_CORE1_STACK_SIZE or make the chain shallower)" % (total, limit))
    sys.exit(1)
print("check-stack: ok")
PY
