# SPDX-License-Identifier: MIT
# Throwaway spike tests for spikes/gen_trials_1_3.py (Story 1.3). No hardware needed.
# Run: uv run --with pytest --with pyserial pytest spikes/test_gen_trials_1_3.py
from __future__ import annotations

import importlib.util
import pathlib
import sys

import pytest

HERE = pathlib.Path(__file__).resolve().parent
SAMPLES = HERE / "samples" / "story-1-3"


def _load(name: str):
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


s = _load("send_2wicc")
g = _load("gen_trials_1_3")

OPTS = g.Options()
FILES = {tf.name: tf for tf in g.build_trials(OPTS)}
EXPECTED_NAMES = {
    "calibration-A.txt",
    "u6-ly-up.txt", "u6-ly-down.txt", "u6-sweep.txt", "u6-diagonals.txt", "u6-raw-ly.txt",
    "fingerings-L.txt", "fingerings-R.txt", "fingerings-A.txt", "fingerings-Y.txt", "fingerings-X.txt",
}
ALLOWED_MASK = sum(s.CANON_BUTTONS[b] for b in "LRAXY")


def parsed(name: str) -> list:
    return s.parse_lines((SAMPLES / name).read_text(encoding="utf-8").splitlines())


def presses(rows):
    """(index, row) for every row that presses a button."""
    return [(i, r) for i, r in enumerate(rows) if r.state.buttons]


def stick_of(note) -> object:
    return s.Stick(note.stick.x, note.stick.y, raw=note.stick.raw)


# ------------------------------------------------------------- committed files


def test_file_set():
    assert set(FILES) == EXPECTED_NAMES
    assert {p.name for p in SAMPLES.glob("*.txt")} == EXPECTED_NAMES


@pytest.mark.parametrize("name", sorted(EXPECTED_NAMES))
def test_committed_file_matches_fresh_run_and_sends(name):
    text = (SAMPLES / name).read_bytes()
    assert text == g.render(FILES[name], OPTS).encode("utf-8"), "regenerate: uv run spikes/gen_trials_1_3.py"
    rows = parsed(name)
    frames, dropped = s.schedule(rows, 60)
    assert frames and not dropped
    assert rows[0].state == s.NEUTRAL and rows[-1].state == s.NEUTRAL
    for f in frames:
        s.encode(f)  # every frame encodes


def test_generator_cli_writes_identical_files(tmp_path, capsys):
    assert g.main(["--out-dir", str(tmp_path), "--key"]) == 0
    for name in EXPECTED_NAMES:
        assert (tmp_path / name).read_bytes() == (SAMPLES / name).read_bytes()
    out = capsys.readouterr().out
    assert "u6-ly-up.txt" in out and "15 notes" in out and "Key" in out and "model:" in out


def test_output_is_deterministic():
    again = {tf.name: g.render(tf, OPTS) for tf in g.build_trials(g.Options())}
    assert again == {n: g.render(tf, OPTS) for n, tf in FILES.items()}


# ------------------------------------------------------------- allow-list guard


@pytest.mark.parametrize("name", sorted(EXPECTED_NAMES))
def test_every_state_inside_allow_list(name):
    rows = parsed(name)
    frames, _ = s.schedule(rows, 60)
    for st in {*frames, *(r.state for r in rows)}:
        assert st.buttons & ~ALLOWED_MASK == 0
        assert st.buttons.bit_count() <= 1
        assert st.hat in (0, 4, 8)
        # left stick only: the encoder always sends the right stick neutral
        cmd = s.encode(st)
        if cmd.startswith("+QF"):
            assert cmd.endswith("000880")


@pytest.mark.parametrize("name", sorted(EXPECTED_NAMES))
def test_modifiers_set_before_and_at_every_press(name):
    rows = parsed(name)
    notes = FILES[name].notes
    ps = presses(rows)
    assert len(ps) == len(notes)
    for (i, row), note in zip(ps, notes):
        want = (note.hat, stick_of(note))
        assert (row.state.hat, row.state.stick) == want  # stick never centred by accident
        mods = rows[i - 1]
        assert mods.state.buttons == 0 and (mods.state.hat, mods.state.stick) == want
        assert s.time_to_frame(row.time, 60) - s.time_to_frame(mods.time, 60) >= 30  # mod-lead 500 ms
        rel = rows[i + 1]
        assert rel.state.buttons == 0 and (rel.state.hat, rel.state.stick) == want  # tail
        assert rows[i + 2].state == s.NEUTRAL


@pytest.mark.parametrize("button", ["B", "ZL", "ZR", "L3", "R3", "PLUS", "MINUS", "HOME", "CAPTURE", "", "A+L"])
def test_guard_rejects_buttons(button):
    with pytest.raises(g.GuardError):
        g.check_allowed(button, g.HAT_NEUTRAL, g.CENTRE)


@pytest.mark.parametrize("hat", [1, 2, 3, 5, 6, 7, 9, -1])
def test_guard_rejects_hats(hat):
    with pytest.raises(g.GuardError):
        g.check_allowed("A", hat, g.CENTRE)


@pytest.mark.parametrize("stick", [g.Stick(0x80, 0x100), g.Stick(-1, 0x80), g.Stick(0x800, 0x1FF, raw=True),
                                   g.Stick(0x800, 0xE01, raw=True), (0x80, 0x80)])
def test_guard_rejects_sticks(stick):
    with pytest.raises(g.GuardError):
        g.check_allowed("A", g.HAT_NEUTRAL, stick)


def test_guard_on_note_construction():
    with pytest.raises(g.GuardError):
        g.Note("B", g.HAT_NEUTRAL, g.CENTRE, 1000, "x", "y")
    with pytest.raises(g.GuardError):
        g.Note("A", 6, g.CENTRE, 1000, "x", "y")


@pytest.mark.parametrize("argv", [
    ["--hold", "6000"], ["--fingering-hold", "0"], ["--sweep-hold", "7000"], ["--mod-lead", "0"],
    ["--gap", "0"], ["--repeats", "0"], ["--sweep-step", "0"], ["--sweep-step", "81"],
    ["--ly-up", "80"], ["--ly-down", "40"], ["--stick-up", "C0"], ["--stick-down", "80"],
])
def test_cli_rejects_unsafe_options(tmp_path, argv, capsys):
    assert g.main(["--out-dir", str(tmp_path), *argv]) == 1
    assert "error" in capsys.readouterr().err
    assert not list(tmp_path.iterdir())


@pytest.mark.parametrize("bad", ["0x", "100", "4G", "-1", "0x40,", ""])
def test_cli_rejects_bad_hex(tmp_path, bad):
    with pytest.raises(SystemExit):
        g.main(["--out-dir", str(tmp_path), "--ly-up", bad])


# ------------------------------------------------------------- AC coverage


def _ly_counts(name):
    out = {}
    for _, r in presses(parsed(name)):
        assert r.state.stick.x == 0x80 and not r.state.stick.raw and r.state.hat == 8
        assert r.state.buttons == s.CANON_BUTTONS["A"]
        out[r.state.stick.y] = out.get(r.state.stick.y, 0) + 1
    return out


def test_u6_up_and_down_values_and_repeats():
    assert list(_ly_counts("u6-ly-up.txt").items()) == [(0x40, 5), (0x20, 5), (0x00, 5)]
    assert list(_ly_counts("u6-ly-down.txt").items()) == [(0xC0, 5), (0xE0, 5), (0xFF, 5)]


def test_u6_hold_timing():
    rows = parsed("u6-ly-up.txt")
    for i, r in presses(rows):
        assert rows[i + 1].time.value - r.time.value == 1500
    assert rows[1].time.value == 1000  # lead-in


def test_sweep_steps_and_timing():
    rows = parsed("u6-sweep.txt")
    ps = presses(rows)
    ys = [r.state.stick.y for _, r in ps]
    up = [0x80, 0x70, 0x60, 0x50, 0x40, 0x30, 0x20, 0x10, 0x00]
    down = [0x80, 0x90, 0xA0, 0xB0, 0xC0, 0xD0, 0xE0, 0xF0, 0xFF]
    assert ys == up + down
    assert {0x40, 0x20, 0x00, 0xC0, 0xE0, 0xFF} <= set(ys)
    for i, r in ps:
        assert rows[i + 1].time.value - r.time.value == 3000
        assert rows[i + 1].state.buttons == 0  # released between steps
        assert r.state.hat == 8 and r.state.stick.x == 0x80


def test_sweep_step_option_changes_step():
    up, down = g.sweep_values(0x08)
    assert up[:3] == [0x80, 0x78, 0x70] and up[-1] == 0x00 and len(up) == 17
    assert down[:3] == [0x80, 0x88, 0x90] and down[-2:] == [0xF8, 0xFF]
    up, down = g.sweep_values(0x30)
    assert up == [0x80, 0x50, 0x20, 0x00] and down == [0x80, 0xB0, 0xE0, 0xFF]
    tf = {t.name: t for t in g.build_trials(g.Options(sweep_step=0x08))}["u6-sweep.txt"]
    assert len(tf.notes) == 17 + 17


def test_diagonals_file():
    ps = presses(parsed("u6-diagonals.txt"))
    sticks = [(r.state.stick.x, r.state.stick.y) for _, r in ps]
    expected = [(0x00, 0x80), (0xFF, 0x80), (0x26, 0x26), (0xDA, 0x26), (0x26, 0xDA), (0xDA, 0xDA),
                (0x00, 0x00), (0xFF, 0x00), (0x00, 0xFF), (0xFF, 0xFF)]
    assert sticks == [xy for xy in expected for _ in range(3)]
    assert all(r.state.buttons == s.CANON_BUTTONS["A"] and r.state.hat == 8 for _, r in ps)


def test_raw_file():
    ps = presses(parsed("u6-raw-ly.txt"))
    ys = [r.state.stick.y for _, r in ps]
    raw = [0x800, 0xA00, 0xB00, 0xC00, 0xD00, 0xE00, 0x600, 0x500, 0x400, 0x300, 0x200]
    assert ys == [y for y in raw for _ in range(3)]
    for _, r in ps:
        assert r.state.stick.raw and r.state.stick.x == 0x800
        assert s.stick_to_12(r.state.stick) == (0x800, r.state.stick.y)  # sent unchanged


FINGER_ORDER = [(8, 0x80), (0, 0x80), (4, 0x80), (8, 0x00), (8, 0xFF), (0, 0x00), (0, 0xFF), (4, 0x00), (4, 0xFF)]


def test_45_combinations():
    triples = set()
    for b in "LRAYX":
        ps = presses(parsed(f"fingerings-{b}.txt"))
        assert len(ps) == 9
        assert [(r.state.hat, r.state.stick.y) for _, r in ps] == FINGER_ORDER
        rows = parsed(f"fingerings-{b}.txt")
        for i, r in ps:
            assert r.state.buttons == s.CANON_BUTTONS[b] and r.state.stick.x == 0x80
            assert rows[i + 1].time.value - r.time.value == 2500
            triples.add((b, r.state.hat, r.state.stick))
    assert len(triples) == 45


def test_model_covers_19_pitches():
    pitches = {}
    for b in "LRAYX":
        for _, hat, sdir in g.MOD_STATES:
            pitches.setdefault(g.model_midi(b, hat, sdir), []).append((b, hat, sdir))
    assert sorted(pitches) == list(range(71, 90))  # B4..F6, every pitch has a fingering
    assert sum(len(v) for v in pitches.values()) == 45
    assert len({g.HAT_OFFSET[h] + 2 * d for _, h, d in g.MOD_STATES}) == 7
    assert g.midi_name(71) == "B4" and g.midi_name(89) == "F6" and g.midi_name(84) == "C6"
    # C6 = Y+D-up = X+stick-down = A+D-up+stick-up = Y+D-down+stick-up
    assert sorted(pitches[84]) == [("A", 0, 1), ("X", 8, -1), ("Y", 0, 0), ("Y", 4, 1)]


def test_model_label_only_in_key_and_labels():
    # the model is a reading aid: fingering labels say "model", never "measured"
    for b in "LRAYX":
        text = (SAMPLES / f"fingerings-{b}.txt").read_text()
        assert "model " in text and "measured" not in text.lower()


# ------------------------------------------------------------- wire values


@pytest.mark.parametrize("ly,wire", [(0x40, 0xB00), (0x20, 0xC80), (0x00, 0xE00),
                                     (0xC0, 0x500), (0xE0, 0x380), (0xFF, 0x20C)])
def test_wire_values_through_encoder(ly, wire):
    cmd = s.encode(s.State(s.CANON_BUTTONS["A"], 8, s.Stick(0x80, ly)))
    assert cmd.startswith("+QF ")
    b = bytes.fromhex(cmd[4:])
    left = b[3:6]
    x12 = left[0] | ((left[1] & 0x0F) << 8)
    y12 = (left[1] >> 4) | (left[2] << 4)
    assert (x12, y12) == (0x800, wire)
    assert g.wire_y(ly) == wire  # the generator's labels agree with the sender
    assert s.HIGHER_Y_IS_UP is True


def test_label_wire_values_match_encoder_for_every_value():
    for v in range(256):
        assert g.wire_y(v) == s.stick_axis_to_12(v, True)
        assert g.wire_x(v) == s.stick_axis_to_12(v, False)


def test_dry_run_every_file(capsys):
    for name in sorted(EXPECTED_NAMES):
        assert s.main(["--dry-run", "--announce", str(SAMPLES / name)]) == 0
        out = capsys.readouterr().out
        assert "PRESS" in out and out.rstrip().splitlines()[-1].startswith("summary:")
