# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Generate the Story 1.3 trial files (stick threshold U6 and the 45 fingerings).

SPDX-License-Identifier: MIT (the project licence, see LICENSE at the repo root).

THROWAWAY SPIKE CODE. Not shipped, not part of app/ or firmware/. It writes
input files for spikes/send_2wicc.py into spikes/samples/story-1-3/. The output
is deterministic and committed, so the owner can run the trials without running
this script; it exists so a second, finer round is one command.

Hardware safety: the spike sender has no AD-9 mask, so this generator is the
only guard. It can only emit the buttons L R A X Y, the D-pad up/down/neutral
and the left stick (see ALLOWED_* below); anything else raises GuardError.

Usage: uv run spikes/gen_trials_1_3.py                 # (re)write all files
       uv run spikes/gen_trials_1_3.py --key           # also print the key
       uv run spikes/gen_trials_1_3.py --ly-up 38,30 --sweep-step 0x08 --out-dir DIR
"""

from __future__ import annotations

import argparse
import pathlib
import sys
from collections.abc import Sequence
from dataclasses import dataclass

HERE = pathlib.Path(__file__).resolve().parent
DEFAULT_OUT = HERE / "samples" / "story-1-3"

# ---------------------------------------------------------------------------
# Allow-list (AD-9 spirit). Everything else would leave or disturb free play:
# B exits, D-pad left (also inside hats 5, 6, 7) closes the ocarina, ZL opens
# the song menu. The right stick cannot even be written in the input format.
# ---------------------------------------------------------------------------

ALLOWED_BUTTONS = ("L", "R", "A", "X", "Y")
HAT_UP, HAT_DOWN, HAT_NEUTRAL = 0, 4, 8
ALLOWED_HATS = {HAT_NEUTRAL: "-", HAT_UP: "up", HAT_DOWN: "down"}
STICK_CENTRE_8 = 0x80
STICK_CENTRE_12 = 0x800
STICK_SWING_12 = 0x600
RAW_MIN, RAW_MAX = STICK_CENTRE_12 - STICK_SWING_12, STICK_CENTRE_12 + STICK_SWING_12  # 0x200..0xE00

AUTO_STOP_MS = 6000  # U1: a held note stops by itself after about 6 s


class GuardError(ValueError):
    """The generator was asked for an input outside the allow-list or the safe timing."""


@dataclass(frozen=True)
class Stick:
    """Left stick. raw=False: canonical 8-bit (0x80 centre, ly 0x00 = up). raw=True: 12-bit wire."""

    x: int = STICK_CENTRE_8
    y: int = STICK_CENTRE_8
    raw: bool = False

    def token(self) -> str:
        if self.raw:
            return f"{self.x:03X}:{self.y:03X}"
        if (self.x, self.y) == (STICK_CENTRE_8, STICK_CENTRE_8):
            return "-"
        return f"{self.x:02X}:{self.y:02X}"


CENTRE = Stick()


def check_allowed(button: str, hat: int, stick: Stick) -> None:
    """Raise GuardError unless (button, hat, stick) is inside the allow-list."""
    if button not in ALLOWED_BUTTONS:
        raise GuardError(f"button {button!r} is not allowed (only {' '.join(ALLOWED_BUTTONS)})")
    if hat not in ALLOWED_HATS:
        raise GuardError(f"hat {hat!r} is not allowed (only neutral 8, up 0, down 4)")
    if not isinstance(stick, Stick):
        raise GuardError("only the left stick can be set")
    lo, hi = (RAW_MIN, RAW_MAX) if stick.raw else (0x00, 0xFF)
    for axis in (stick.x, stick.y):
        if not (isinstance(axis, int) and lo <= axis <= hi):
            raise GuardError(f"stick value {axis!r} outside {lo:#x}..{hi:#x}")


# ---------------------------------------------------------------------------
# Pitch model (reading aid only, never a result). Story 1.1: L D5, R F5, A A5,
# Y B5, X D6; D-pad up/down +-1, stick up/down +-2, they add.
# ---------------------------------------------------------------------------

BUTTON_MIDI = {"L": 74, "R": 77, "A": 81, "Y": 83, "X": 86}
NOTE_NAMES = ("C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B")
HAT_OFFSET = {HAT_NEUTRAL: 0, HAT_UP: 1, HAT_DOWN: -1}


def midi_name(m: int) -> str:
    return f"{NOTE_NAMES[m % 12]}{m // 12 - 1}"


def midi_hz(m: int) -> float:
    return 440.0 * 2 ** ((m - 69) / 12)


def pitch_text(m: int) -> str:
    return f"{midi_name(m)} {midi_hz(m):.1f} Hz"


# The nine modifier states, in the order of the fingering files: (name, hat, stick direction).
MOD_STATES = (
    ("none", HAT_NEUTRAL, 0),
    ("D-up", HAT_UP, 0),
    ("D-down", HAT_DOWN, 0),
    ("stick-up", HAT_NEUTRAL, +1),
    ("stick-down", HAT_NEUTRAL, -1),
    ("D-up+stick-up", HAT_UP, +1),
    ("D-up+stick-down", HAT_UP, -1),
    ("D-down+stick-up", HAT_DOWN, +1),
    ("D-down+stick-down", HAT_DOWN, -1),
)


def model_midi(button: str, hat: int, stick_dir: int) -> int:
    return BUTTON_MIDI[button] + HAT_OFFSET[hat] + 2 * stick_dir


# ---------------------------------------------------------------------------
# Wire values (same scaling as send_2wicc.stick_axis_to_12, written out here so
# the labels can show them; the tests pin both against the sender's encoder).
# ---------------------------------------------------------------------------


def wire_y(ly: int) -> int:
    """Canonical ly -> 12-bit wire Y (higher Y = up, clamped like the sender)."""
    v = STICK_CENTRE_12 + round(-(ly - STICK_CENTRE_8) * STICK_SWING_12 / 0x80)
    return max(RAW_MIN, min(RAW_MAX, v))


def wire_x(lx: int) -> int:
    v = STICK_CENTRE_12 + round((lx - STICK_CENTRE_8) * STICK_SWING_12 / 0x80)
    return max(RAW_MIN, min(RAW_MAX, v))


def swing_pct(wire: int) -> float:
    return abs(wire - STICK_CENTRE_12) * 100.0 / STICK_SWING_12


def direction(wire_y12: int) -> str:
    if wire_y12 > STICK_CENTRE_12:
        return "up"
    if wire_y12 < STICK_CENTRE_12:
        return "down"
    return "rest"


# ---------------------------------------------------------------------------
# Trial notes and rendering
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Note:
    button: str
    hat: int
    stick: Stick
    hold_ms: int
    label: str   # what is played, e.g. "A ly=0x40 wire=0xB00 up 50% try 3/5"
    expect: str  # model reading aid for the key, never a measured value

    def __post_init__(self):
        check_allowed(self.button, self.hat, self.stick)


@dataclass(frozen=True)
class Timing:
    lead_in: int = 1000
    mod_lead: int = 500
    tail: int = 200
    gap: int = 1500

    def validate(self) -> None:
        if self.lead_in < 0 or self.tail < 0:
            raise GuardError("--lead-in and --tail must be >= 0")
        if self.mod_lead <= 0 or self.gap <= 0:
            raise GuardError("--mod-lead and --gap must be > 0 (modifiers before the button; note stops before the next)")


@dataclass(frozen=True)
class Options:
    timing: Timing = Timing()
    hold: int = 1500             # U6 files
    fingering_hold: int = 2500   # 45 combinations
    sweep_hold: int = 3000
    repeats: int = 5             # U6 up/down
    diag_repeats: int = 3        # diagonals and raw sweep
    ly_up: tuple[int, ...] = (0x40, 0x20, 0x00)
    ly_down: tuple[int, ...] = (0xC0, 0xE0, 0xFF)
    sweep_step: int = 0x10
    stick_up: int = 0x00
    stick_down: int = 0xFF

    def validate(self) -> None:
        self.timing.validate()
        for name, v in (("--hold", self.hold), ("--fingering-hold", self.fingering_hold),
                        ("--sweep-hold", self.sweep_hold)):
            if not 0 < v < AUTO_STOP_MS:
                raise GuardError(f"{name} must be 1..{AUTO_STOP_MS - 1} ms (U1 auto-stop at about 6 s)")
        if self.repeats < 1 or self.diag_repeats < 1:
            raise GuardError("--repeats and --diag-repeats must be >= 1")
        if not 1 <= self.sweep_step <= 0x80:
            raise GuardError("--sweep-step must be 0x01..0x80")
        for v in self.ly_up + (self.stick_up,):
            if not 0x00 <= v < STICK_CENTRE_8:
                raise GuardError(f"up value {v:#04x} must be 0x00..0x7F")
        for v in self.ly_down + (self.stick_down,):
            if not STICK_CENTRE_8 < v <= 0xFF:
                raise GuardError(f"down value {v:#04x} must be 0x81..0xFF")
        if not self.ly_up or not self.ly_down:
            raise GuardError("--ly-up and --ly-down need at least one value")


def ly_desc(ly: int) -> str:
    w = wire_y(ly)
    return f"ly=0x{ly:02X} wire=0x{w:03X} {direction(w)} {swing_pct(w):.0f}%"


def stick_expect(base: str, sign: int) -> str:
    """Key text for a stick trial whose effect is what is being measured."""
    rest = BUTTON_MIDI[base]
    if sign == 0:
        return f"{pitch_text(rest)} (no change)"
    moved = rest + 2 * sign
    return f"{pitch_text(moved)} if counted, {pitch_text(rest)} if not"


def u6_straight(values: Sequence[int], opts: Options) -> list[Note]:
    notes = []
    for ly in values:
        sign = 1 if ly < STICK_CENTRE_8 else -1
        for i in range(1, opts.repeats + 1):
            notes.append(Note("A", HAT_NEUTRAL, Stick(0x80, ly), opts.hold,
                              f"A {ly_desc(ly)} try {i}/{opts.repeats}", stick_expect("A", sign)))
    return notes


def sweep_values(step: int) -> tuple[list[int], list[int]]:
    up = [0x80, *range(0x80 - step, -1, -step)]
    if up[-1] != 0x00:
        up.append(0x00)
    down = [0x80, *range(0x80 + step, 0x100, step)]
    if down[-1] != 0xFF:
        down.append(0xFF)
    return up, down


def u6_sweep(opts: Options) -> list[Note]:
    up, down = sweep_values(opts.sweep_step)
    notes = []
    for side, values in (("up", up), ("down", down)):
        for k, ly in enumerate(values):
            w = wire_y(ly)
            sign = 0 if w == STICK_CENTRE_12 else (1 if w > STICK_CENTRE_12 else -1)
            tag = "rest control" if sign == 0 else f"step {k}/{len(values) - 1}"
            notes.append(Note("A", HAT_NEUTRAL, Stick(0x80, ly), opts.sweep_hold,
                              f"A sweep {side} {ly_desc(ly)} {tag}", stick_expect("A", sign)))
    return notes


DIAGONALS = (
    # (lx, ly, name, vertical sign for the expectation)
    (0x00, 0x80, "lx-only left control", 0),
    (0xFF, 0x80, "lx-only right control", 0),
    (0x26, 0x26, "circle up-left", 1),
    (0xDA, 0x26, "circle up-right", 1),
    (0x26, 0xDA, "circle down-left", -1),
    (0xDA, 0xDA, "circle down-right", -1),
    (0x00, 0x00, "corner up-left", 1),
    (0xFF, 0x00, "corner up-right", 1),
    (0x00, 0xFF, "corner down-left", -1),
    (0xFF, 0xFF, "corner down-right", -1),
)


def u6_diagonals(opts: Options) -> list[Note]:
    notes = []
    for lx, ly, name, sign in DIAGONALS:
        for i in range(1, opts.diag_repeats + 1):
            label = (f"A {name} stick={lx:02X}:{ly:02X} wire={wire_x(lx):03X}:{wire_y(ly):03X}"
                     f" try {i}/{opts.diag_repeats}")
            notes.append(Note("A", HAT_NEUTRAL, Stick(lx, ly), opts.hold, label, stick_expect("A", sign)))
    return notes


RAW_LY = (0x800, 0xA00, 0xB00, 0xC00, 0xD00, 0xE00, 0x600, 0x500, 0x400, 0x300, 0x200)


def u6_raw(opts: Options) -> list[Note]:
    notes = []
    for y in RAW_LY:
        sign = 0 if y == STICK_CENTRE_12 else (1 if y > STICK_CENTRE_12 else -1)
        tag = "rest control" if sign == 0 else f"{direction(y)} {swing_pct(y):.0f}%"
        for i in range(1, opts.diag_repeats + 1):
            notes.append(Note("A", HAT_NEUTRAL, Stick(STICK_CENTRE_12, y, raw=True), opts.hold,
                              f"A raw ly=0x{y:03X} {tag} try {i}/{opts.diag_repeats}", stick_expect("A", sign)))
    return notes


def fingerings(button: str, opts: Options) -> list[Note]:
    notes = []
    for k, (name, hat, sdir) in enumerate(MOD_STATES, start=1):
        ly = {0: STICK_CENTRE_8, 1: opts.stick_up, -1: opts.stick_down}[sdir]
        m = model_midi(button, hat, sdir)
        notes.append(Note(button, hat, Stick(0x80, ly), opts.fingering_hold,
                          f"{button} {name} (note {k}/{len(MOD_STATES)}, model {midi_name(m)})",
                          f"{pitch_text(m)} (model)"))
    return notes


@dataclass(frozen=True)
class TrialFile:
    name: str
    title: str
    notes: list[Note]


def build_trials(opts: Options) -> list[TrialFile]:
    """All trial files, in the field run order (calibration first, u6-raw-ly last, optional)."""
    opts.validate()
    calibration = Note("A", HAT_NEUTRAL, CENTRE, opts.fingering_hold,
                       f"A calibration, no modifiers (note 1/1, model {midi_name(BUTTON_MIDI['A'])})",
                       f"{pitch_text(BUTTON_MIDI['A'])} (model)")
    files = [
        TrialFile("calibration-A.txt", "calibration: one A press, no modifiers (mic and alignment check)",
                  [calibration]),
        TrialFile("u6-ly-up.txt", "U6 stick threshold, up: base A, ly 0x40 0x20 0x00", u6_straight(opts.ly_up, opts)),
        TrialFile("u6-ly-down.txt", "U6 stick threshold, down: base A, ly 0xC0 0xE0 0xFF (mirrored)",
                  u6_straight(opts.ly_down, opts)),
        TrialFile("u6-sweep.txt", "U6 deflection sweep: base A, one press per step (pitch vs deflection curve)",
                  u6_sweep(opts)),
        TrialFile("u6-diagonals.txt", "U6 diagonals and lx-only controls: base A", u6_diagonals(opts)),
    ]
    for b in ("L", "R", "A", "Y", "X"):
        files.append(TrialFile(f"fingerings-{b}.txt", f"45 combinations: button {b} x 9 modifier states",
                               fingerings(b, opts)))
    files.append(TrialFile("u6-raw-ly.txt", "U6 optional raw 12-bit sweep (finer second round only if needed)",
                           u6_raw(opts)))
    return files


@dataclass(frozen=True)
class Line:
    t: int
    button: str  # "-" or one allowed name
    hat: int
    stick: Stick
    label: str


def note_lines(notes: Sequence[Note], timing: Timing) -> list[Line]:
    """The common row pattern: neutral lead-in, modifiers, press, release, neutral, gap."""
    lines = [Line(0, "-", HAT_NEUTRAL, CENTRE, "neutral lead-in")]
    t = timing.lead_in
    for i, n in enumerate(notes):
        hat = "hat neutral" if n.hat == HAT_NEUTRAL else f"D-{ALLOWED_HATS[n.hat]}"
        stick = "centre" if n.stick.token() == "-" else n.stick.token()
        mods = f"{hat}, stick {stick}"
        lines.append(Line(t, "-", n.hat, n.stick, f"mods for next: {mods}"))
        press = t + timing.mod_lead
        lines.append(Line(press, n.button, n.hat, n.stick, f"PRESS {n.label}"))
        release = press + n.hold_ms
        lines.append(Line(release, "-", n.hat, n.stick, "release, modifiers kept"))
        lines.append(Line(release + timing.tail, "-", HAT_NEUTRAL, CENTRE, "neutral"))
        t = release + timing.tail + timing.gap
    return lines


def press_times(notes: Sequence[Note], timing: Timing) -> list[int]:
    return [ln.t for ln in note_lines(notes, timing) if ln.button != "-"]


def duration_ms(notes: Sequence[Note], timing: Timing) -> int:
    return note_lines(notes, timing)[-1].t


def render(tf: TrialFile, opts: Options) -> str:
    lines = note_lines(tf.notes, opts.timing)
    out = [
        f"# Story 1.3 trial: {tf.title}.",
        "# GENERATED by spikes/gen_trials_1_3.py; do not edit by hand, regenerate instead.",
        "# Run in ocarina free play only (open it with D-pad left on the real controller, wait 2 s).",
        "# Allow-list: buttons L R A X Y, D-pad up/down, left stick. Never add B, ZL, ZR, D-pad left",
        "# or a diagonal D-pad, or the right stick. Stop with Ctrl+C in a real terminal.",
        f"# {len(tf.notes)} notes, about {duration_ms(tf.notes, opts.timing) / 1000:.1f} s. PRESS rows are the notes;",
        "# the D-pad/stick are set before the button and kept until just after the release.",
        "# time(ms)  buttons  hat      stick    label",
    ]
    for ln in lines:
        hat = ALLOWED_HATS[ln.hat]
        out.append(f"{ln.t:<11} {ln.button:<8} {hat:<8} {ln.stick.token():<8} # {ln.label}")
    return "\n".join(out) + "\n"


def key_lines(tf: TrialFile, opts: Options) -> list[str]:
    out = [f"{tf.name}: {len(tf.notes)} notes, {duration_ms(tf.notes, opts.timing) / 1000:.1f} s"]
    for k, (n, t) in enumerate(zip(tf.notes, press_times(tf.notes, opts.timing)), start=1):
        out.append(f"  {k:>3}  t={t / 1000:6.1f}s  {n.label:<58}  model: {n.expect}")
    return out


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def hex8(tok: str) -> int:
    t = tok.strip()
    body = t[2:] if t.lower().startswith("0x") else t
    if not body or len(body) > 2 or any(c not in "0123456789abcdefABCDEF" for c in body):
        raise argparse.ArgumentTypeError(f"bad 8-bit hex value {tok!r} (like 40 or 0x40)")
    return int(body, 16)


def hex_list(tok: str) -> tuple[int, ...]:
    return tuple(hex8(p) for p in tok.split(","))


def build_parser() -> argparse.ArgumentParser:
    d = Options()
    p = argparse.ArgumentParser(prog="gen_trials_1_3.py",
                                description="Throwaway spike: write the Story 1.3 trial files for send_2wicc.py.")
    p.add_argument("--out-dir", default=str(DEFAULT_OUT), help="output folder (default spikes/samples/story-1-3)")
    p.add_argument("--key", action="store_true", help="also print the key (notes, press times, model pitch)")
    p.add_argument("--lead-in", type=int, default=d.timing.lead_in, help="neutral at the start, ms (1000)")
    p.add_argument("--mod-lead", type=int, default=d.timing.mod_lead, help="modifiers before the button, ms (500)")
    p.add_argument("--tail", type=int, default=d.timing.tail, help="modifiers kept after the release, ms (200)")
    p.add_argument("--gap", type=int, default=d.timing.gap, help="neutral between notes, ms (1500)")
    p.add_argument("--hold", type=int, default=d.hold, help="note hold in the U6 files, ms (1500)")
    p.add_argument("--fingering-hold", type=int, default=d.fingering_hold, help="note hold, 45 combinations, ms (2500)")
    p.add_argument("--sweep-hold", type=int, default=d.sweep_hold, help="note hold in the sweep, ms (3000)")
    p.add_argument("--repeats", type=int, default=d.repeats, help="presses per value, U6 up/down (5)")
    p.add_argument("--diag-repeats", type=int, default=d.diag_repeats, help="presses per value, diagonals/raw (3)")
    p.add_argument("--ly-up", type=hex_list, default=d.ly_up, help="canonical up values (40,20,00)")
    p.add_argument("--ly-down", type=hex_list, default=d.ly_down, help="canonical down values (C0,E0,FF)")
    p.add_argument("--sweep-step", type=hex8, default=d.sweep_step, help="sweep step, hex (0x10)")
    p.add_argument("--stick-up", type=hex8, default=d.stick_up, help="ly for stick-up in the fingerings (0x00)")
    p.add_argument("--stick-down", type=hex8, default=d.stick_down, help="ly for stick-down in the fingerings (0xFF)")
    return p


def options_from_args(a: argparse.Namespace) -> Options:
    return Options(Timing(a.lead_in, a.mod_lead, a.tail, a.gap), a.hold, a.fingering_hold, a.sweep_hold,
                   a.repeats, a.diag_repeats, a.ly_up, a.ly_down, a.sweep_step, a.stick_up, a.stick_down)


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    opts = options_from_args(args)
    try:
        files = build_trials(opts)
    except GuardError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    out_dir = pathlib.Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    for tf in files:
        (out_dir / tf.name).write_text(render(tf, opts), encoding="utf-8", newline="\n")
        secs = duration_ms(tf.notes, opts.timing) / 1000
        print(f"{tf.name:<20} {len(tf.notes):>3} notes  {secs:6.1f} s")
    if args.key:
        print("\nKey (model pitches are a reading aid, not results):")
        for tf in files:
            print()
            print("\n".join(key_lines(tf, opts)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
