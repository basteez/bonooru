#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Generate the built-in test timeline of the controller (story 2.5 D8, Task 9.1).

One source of truth: the Story 1.6 trial file, itself generated from the Story
1.5 profile (spikes/samples/story-1-6/ode-to-joy-90bpm.txt, 62 notes at 90 BPM,
400 ms press cap). This script converts it, with its own small parser (not
send_2wicc.py), into:

  firmware/controller/core/testtl_data.c   the const event table the debug
                                            timeline flavour loads (committed,
                                            generated: do not edit)
  firmware/tests/testtl_expected.txt       the expected notes (index, sounding
                                            pitch, frame, t_us) for the tests
                                            and the audio analysis

Rows are `<frames>f buttons hat stick  # label`. Each row becomes one event:
  t_us   = (frames * 1000000 + 30) // 60      (integer, rounded, the 60 Hz grid)
  state  = the canonical AD-10 state: buttons (L R A X Y only, AD-9), hat
           (up, down or neutral), left stick `lx:ly` (2 hex digits each; ly 00
           is up), right stick centred, flags 0
and the LAST row, which must be neutral, gets NEUTRAL|END.
Checks: allow-list, strictly increasing t_us, t_us < 2^31, at most 400 events,
the source header names the Story 1.5 profile. Stdlib only, Python 3.9+.

Usage: gen_test_timeline.py [--src FILE] [--out-c FILE] [--out-expected FILE]
Exit: 0 written, 1 a rule broken (message on stderr), 2 usage error.
"""

import argparse
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
FW = os.path.dirname(HERE)
REPO = os.path.dirname(FW)
SRC_DEFAULT = os.path.join(REPO, "spikes", "samples", "story-1-6", "ode-to-joy-90bpm.txt")
OUT_C_DEFAULT = os.path.join(FW, "controller", "core", "testtl_data.c")
OUT_EXPECTED_DEFAULT = os.path.join(FW, "tests", "testtl_expected.txt")

PROFILE_MARK = "profile: app/profiles/oot-switch2-1.1.1.toml"
MAX_EVENTS = 400
T_US_LIMIT = 1 << 31
FRAME_RATE = 60

# Canonical buttons (AD-10, ocb_proto/consts.h) allowed in PLAYING (AD-9).
BUTTONS = {"Y": 0x0001, "A": 0x0004, "X": 0x0008, "L": 0x0010, "R": 0x0020}
FORBIDDEN_BUTTONS = ("B", "ZL", "ZR", "MINUS", "PLUS", "L3", "R3", "HOME", "CAPTURE")
HATS = {"-": 8, "up": 0, "down": 4, "0": 0, "4": 4, "8": 8}
HAT_NEUTRAL = 8
STICK_CENTRE = 0x80
END_FLAG = 0x01

_ROW_RE = re.compile(r"^(\d+)f$")
_NOTE_RE = re.compile(r"NOTE\s+(\d+)\s+([A-G]#?\d)\b")


class GenError(Exception):
    pass


def t_us_of(frames):
    """Frames of the 60 Hz grid to microseconds, integer, rounded."""
    return (frames * 1000000 + FRAME_RATE // 2) // FRAME_RATE


def parse_buttons(tok, line):
    if tok == "-":
        return 0
    mask = 0
    for name in tok.split("+"):
        key = name.strip().upper()
        if key in FORBIDDEN_BUTTONS:
            raise GenError("line %d: button %s is outside the AD-9 allow-list (L R A X Y)" % (line, name))
        if key not in BUTTONS:
            raise GenError("line %d: unknown button %r" % (line, name))
        mask |= BUTTONS[key]
    return mask


def parse_hat(tok, line):
    key = tok.lower()
    if key not in HATS:
        raise GenError("line %d: hat %r is outside the allow-list (up, down, neutral)" % (line, tok))
    return HATS[key]


def parse_stick(tok, line):
    if tok == "-":
        return STICK_CENTRE, STICK_CENTRE
    m = re.fullmatch(r"([0-9A-Fa-f]{2}):([0-9A-Fa-f]{2})", tok)
    if m is None:
        raise GenError("line %d: bad stick %r (lx:ly, 2 hex digits each)" % (line, tok))
    return int(m.group(1), 16), int(m.group(2), 16)


def parse(text):
    """Returns (events, notes). events: [(t_us, (buttons, hat, lx, ly, rx, ry, flags), frames, label)]."""
    if PROFILE_MARK not in text:
        raise GenError("the source header does not say it is generated from the Story 1.5 profile (%r)"
                       % PROFILE_MARK)
    events = []
    notes = []
    for n, raw in enumerate(text.splitlines(), start=1):
        body, _, label = raw.partition("#")
        toks = body.split()
        if not toks:
            continue
        if len(toks) > 4:
            raise GenError("line %d: too many columns (time buttons hat stick)" % n)
        toks += ["-"] * (4 - len(toks))
        m = _ROW_RE.match(toks[0])
        if m is None:
            raise GenError("line %d: time %r is not in frames (like 30f)" % (n, toks[0]))
        frames = int(m.group(1))
        t = t_us_of(frames)
        buttons = parse_buttons(toks[1], n)
        hat = parse_hat(toks[2], n)
        lx, ly = parse_stick(toks[3], n)
        if events and t <= events[-1][0]:
            raise GenError("line %d: time %d us is not after the previous row (%d us)" % (n, t, events[-1][0]))
        if t >= T_US_LIMIT:
            raise GenError("line %d: t_us %d is not below 2^31" % (n, t))
        label = label.strip()
        events.append((t, (buttons, hat, lx, ly, STICK_CENTRE, STICK_CENTRE, 0), frames, label))
        nm = _NOTE_RE.search(label)
        if nm is not None:
            if buttons == 0:
                raise GenError("line %d: a NOTE row without a note button" % n)
            notes.append((int(nm.group(1)), nm.group(2), frames, t))
    if not events:
        raise GenError("no rows")
    if len(events) > MAX_EVENTS:
        raise GenError("%d events, more than %d" % (len(events), MAX_EVENTS))
    t, st, frames, label = events[-1]
    if st != (0, HAT_NEUTRAL, STICK_CENTRE, STICK_CENTRE, STICK_CENTRE, STICK_CENTRE, 0):
        raise GenError("the last row (frame %d) is not neutral: no final neutral to mark NEUTRAL|END" % frames)
    events[-1] = (t, st[:6] + (END_FLAG,), frames, label)
    return events, notes


def render_c(events, src_rel):
    out = []
    out.append("/*")
    out.append(" * SPDX-License-Identifier: MIT")
    out.append(" *")
    out.append(" * GENERATED by firmware/tools/gen_test_timeline.py from")
    out.append(" * %s; do not edit." % src_rel)
    out.append(" * Regenerate with: python3 firmware/tools/gen_test_timeline.py")
    out.append(" * The built-in test timeline of the debug timeline flavour (story 2.5")
    out.append(" * D8): %d events, the last one NEUTRAL|END; t_us on the 60 Hz frame" % len(events))
    out.append(" * grid; buttons, hat and left stick inside the AD-9 allow-list.")
    out.append(" */")
    out.append("#include <stddef.h>")
    out.append("#include <stdint.h>")
    out.append("")
    out.append('#include "ocb_core/testtl.h"')
    out.append("")
    out.append("const ocb_testtl_event ocb_testtl_table[] = {")
    for t, (b, hat, lx, ly, rx, ry, fl), frames, label in events:
        comment = "frame %d" % frames
        if label:
            comment += ": " + label.replace("*/", "* /")
        out.append("    {%du, {0x%04Xu, %du, 0x%02Xu, 0x%02Xu, 0x%02Xu, 0x%02Xu, 0x%02Xu}}, /* %s */"
                   % (t, b, hat, lx, ly, rx, ry, fl, comment))
    out.append("};")
    out.append("")
    out.append("const size_t ocb_testtl_table_len = sizeof ocb_testtl_table / sizeof ocb_testtl_table[0];")
    out.append("")
    return "\n".join(out)


def render_expected(notes, n_events, end_us, src_rel):
    out = ["# GENERATED by firmware/tools/gen_test_timeline.py from %s; do not edit." % src_rel,
           "# Expected notes of the built-in test timeline (story 2.5 D8, D16): %d notes in %d events, "
           "the last event at %d us." % (len(notes), n_events, end_us),
           "# index pitch frame t_us"]
    for idx, pitch, frames, t in notes:
        out.append("%02d %s %d %d" % (idx, pitch, frames, t))
    out.append("")
    return "\n".join(out)


def generate(src, out_c, out_expected):
    with open(src, "r", encoding="utf-8") as f:
        text = f.read()
    events, notes = parse(text)
    rel = os.path.relpath(os.path.abspath(src), REPO).replace(os.sep, "/")
    if rel.startswith(".."):
        rel = os.path.basename(src)
    with open(out_c, "w", encoding="utf-8", newline="\n") as f:
        f.write(render_c(events, rel))
    with open(out_expected, "w", encoding="utf-8", newline="\n") as f:
        f.write(render_expected(notes, len(events), events[-1][0], rel))
    return events, notes


def main(argv=None):
    ap = argparse.ArgumentParser(description="Generate the controller's built-in test timeline (story 2.5).")
    ap.add_argument("--src", default=SRC_DEFAULT)
    ap.add_argument("--out-c", default=OUT_C_DEFAULT)
    ap.add_argument("--out-expected", default=OUT_EXPECTED_DEFAULT)
    args = ap.parse_args(argv)
    try:
        events, notes = generate(args.src, args.out_c, args.out_expected)
    except (GenError, OSError) as e:
        print("gen_test_timeline: FAIL %s" % e, file=sys.stderr)
        return 1
    print("gen_test_timeline: %d events, %d notes, last at %d us -> %s, %s"
          % (len(events), len(notes), events[-1][0], args.out_c, args.out_expected))
    return 0


if __name__ == "__main__":
    sys.exit(main())
