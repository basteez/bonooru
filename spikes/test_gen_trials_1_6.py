# SPDX-License-Identifier: MIT
# Throwaway spike tests for spikes/gen_trials_1_6.py (Story 1.6). No hardware, no real sleeps.
# Needs Python 3.11+ (tomllib, the generator reads the real profile). Run:
#   uv run --python 3.12 --with pytest --with pyserial pytest spikes/test_gen_trials_1_6.py
from __future__ import annotations

import importlib.util
import pathlib
import re
import sys
from fractions import Fraction

import pytest

pytest.importorskip("tomllib")

HERE = pathlib.Path(__file__).resolve().parent
SAMPLES = HERE / "samples" / "story-1-6"
MELODY = SAMPLES / "ode-to-joy.melody"


def _load(name: str):
    if name in sys.modules:
        return sys.modules[name]
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


s = _load("send_2wicc")
g6 = _load("gen_trials_1_6")

PROF = g6.load_profile()  # the real profile: a Part C change is followed, not copied
FILES = {tf.name: tf for tf in g6.build(g6.Options())}
ALLOWED_MASK = sum(s.CANON_BUTTONS[b] for b in "LRAXY")
NOTE_RE = re.compile(r"NOTE (\d+) (\S+) (\d+) fingering=(\S+)$")

# Documented melody facts (the .melody file and the story's "Melody and range check"), not profile values.
MELODY_NOTES = 62
MELODY_BEATS = 64
MELODY_PITCHES = {74, 79, 81, 83, 84, 86}  # D5 G5 A5 B5 C6 D6


def parsed(name: str):
    return s.parse_lines((SAMPLES / name).read_text(encoding="utf-8").splitlines())


def frames_of(rows):
    return [s.time_to_frame(r.time, PROF.frame_rate) for r in rows]


def write_melody(tmp_path, text: str) -> pathlib.Path:
    p = tmp_path / "m.melody"
    p.write_text(text, encoding="utf-8")
    return p


# ------------------------------------------------------------- committed files (3.1, 3.2)


LATER = sorted(p.name for p in SAMPLES.glob("u1-confirm-*.txt"))  # generated in the session from round 1


def test_file_set():
    assert {p.name for p in SAMPLES.glob("*.txt")} - set(LATER) == set(FILES)
    assert {"ode-to-joy-60bpm.txt", "ode-to-joy-90bpm.txt", "ode-to-joy-120bpm.txt"} <= set(FILES)
    assert sorted(n for n in FILES if n.startswith("u1-sustain-A-part")) == \
        [f"u1-sustain-A-part{k}.txt" for k in range(1, 6)]


@pytest.mark.parametrize("name", sorted(FILES))
def test_committed_file_matches_fresh_run(name):
    assert (SAMPLES / name).read_bytes() == g6.render(FILES[name]).encode("utf-8"), \
        "regenerate: uv run --python 3.12 spikes/gen_trials_1_6.py"


@pytest.mark.parametrize("name", LATER)
def test_committed_confirm_file_matches_its_command(name):
    text = (SAMPLES / name).read_text(encoding="utf-8")
    ms = int(re.search(r"# u1_confirm_ms: (\d+)", text).group(1))
    button = name.removeprefix("u1-confirm-").removesuffix(".txt")
    (tf,) = g6.u1_confirm(g6.U1Options(confirm_ms=ms, buttons=(button,)), PROF)
    assert text == g6.render(tf)


@pytest.mark.parametrize("name", sorted({*FILES, *LATER}))
def test_file_parses_encodes_and_stays_in_the_allow_list(name):
    rows = parsed(name)
    frames, dropped = s.schedule(rows, PROF.frame_rate)
    assert frames and dropped == []
    assert rows[0].state == s.NEUTRAL and rows[-1].state == s.NEUTRAL
    assert all(r.time.frames for r in rows)
    for st in set(frames):
        s.encode(st)
        assert st.buttons & ~ALLOWED_MASK == 0
        assert st.buttons.bit_count() <= 1  # U3: never two note buttons
        assert st.hat in (0, 4, 8)
        assert not st.stick.raw


def test_generator_cli_writes_identical_files(tmp_path, capsys):
    assert g6.main(["--out-dir", str(tmp_path), "--key"]) == 0
    for name in FILES:
        assert (tmp_path / name).read_bytes() == (SAMPLES / name).read_bytes()
    out = capsys.readouterr().out
    assert "62 notes" in out and "9 attempts" in out and "R+stick-up" in out and "TRY u1 A" in out


def test_cli_errors(tmp_path, capsys):
    assert g6.main(["--set", "confirm", "--out-dir", str(tmp_path)]) == 1
    assert "--u1-confirm-ms" in capsys.readouterr().err
    assert g6.main(["--set", "melody", "--bpm", "400", "--out-dir", str(tmp_path)]) == 1
    assert "min_press_frames" in capsys.readouterr().err
    assert not list(tmp_path.iterdir())


# ------------------------------------------------------------- melody facts and pitch check (3.4)


def test_melody_file_facts():
    items = g6.parse_melody(MELODY.read_text(encoding="utf-8"))
    notes = [i for i in items if i.midi is not None]
    assert len(notes) == MELODY_NOTES and sum(i.beats for i in items) == MELODY_BEATS
    assert {i.midi for i in notes} == MELODY_PITCHES
    assert "public domain" in MELODY.read_text(encoding="utf-8").lower()


@pytest.mark.parametrize("bpm", g6.COMMITTED_BPMS)
def test_every_note_label_matches_the_profile_pitch(bpm):
    rows = parsed(f"ode-to-joy-{bpm}bpm.txt")
    items = [i for i in g6.parse_melody(MELODY.read_text(encoding="utf-8")) if i.midi is not None]
    labelled = [(r, NOTE_RE.match(r.label)) for r in rows if r.label.startswith("NOTE ")]
    assert len(labelled) == len(items)
    used_buttons = set()
    for k, ((r, m), item) in enumerate(zip(labelled, items), start=1):
        assert m and int(m.group(1)) == k and int(m.group(3)) == item.midi
        button = next(n for n in "LRAXY" if r.state.buttons == s.CANON_BUTTONS[n])
        dpad = {8: "none", 0: "up", 4: "down"}[r.state.hat]
        ly = r.state.stick.y
        stick = "up" if ly == PROF.modifiers["stick_up_ly"] and ly != 0x80 else \
            "down" if ly == PROF.modifiers["stick_down_ly"] and ly != 0x80 else "none"
        assert PROF.model_pitch(button, dpad, stick) == item.midi  # melody and fingering agree
        chart = PROF.chart_fingering(item.midi, "test")
        assert (chart.button, chart.dpad, chart.stick) == (button, dpad, stick)
        assert m.group(4) == chart.text() and m.group(2) == g6.g.midi_name(item.midi)
        used_buttons.add(button)
    assert used_buttons == set("LRAXY")  # G5 tonic uses all five buttons (owner decision 2)


# ------------------------------------------------------------- timing guards (3.3)


def press_runs_from_file(name):
    rows = parsed(name)
    fr = frames_of(rows)
    runs = []  # (button mask, start, end, hat, stick)
    for i, r in enumerate(rows[:-1]):
        if not r.state.buttons:
            continue
        if runs and runs[-1][0] == r.state.buttons and runs[-1][2] == fr[i]:
            runs[-1] = (*runs[-1][:2], fr[i + 1], *runs[-1][3:])
            assert (runs[-1][3], runs[-1][4]) == (r.state.hat, r.state.stick), "modifier change while held (U4)"
        else:
            runs.append((r.state.buttons, fr[i], fr[i + 1], r.state.hat, r.state.stick))
    return rows, fr, runs


@pytest.mark.parametrize("bpm", g6.COMMITTED_BPMS)
def test_melody_timing_rules_from_the_profile(bpm):
    rows, fr, runs = press_runs_from_file(f"ode-to-joy-{bpm}bpm.txt")
    state_at = {}
    for i, r in enumerate(rows[:-1]):
        for f in range(fr[i], fr[i + 1]):
            state_at[f] = r.state
    for k, (b, a, e, hat, stick) in enumerate(runs):
        assert e - a >= PROF.min_press
        assert (e - a) * 1000 <= PROF.max_sustain_ms * PROF.frame_rate
        if k + 1 < len(runs) and runs[k + 1][0] == b:
            assert runs[k + 1][1] - e >= PROF.min_gap
        for f in range(a - PROF.lead, a):  # modifiers in place modifier_lead_frames before the press
            assert (state_at[f].hat, state_at[f].stick) == (hat, stick)
    g6.check_rows(list(FILES[f"ode-to-joy-{bpm}bpm.txt"].rows), PROF)


@pytest.mark.parametrize("bpm", g6.COMMITTED_BPMS)
def test_onsets_on_the_cumulative_grid(bpm):
    rows = parsed(f"ode-to-joy-{bpm}bpm.txt")
    fr = frames_of(rows)
    onsets = [f for f, r in zip(fr, rows) if r.label.startswith("NOTE ")]
    items = g6.parse_melody(MELODY.read_text(encoding="utf-8"))
    beat = Fraction(60 * PROF.frame_rate, bpm)
    cum, want = Fraction(0), []
    for i in items:
        if i.midi is not None:
            want.append(PROF.start_lead_frames + int(cum * beat + Fraction(1, 2)))
        cum += i.beats
    assert onsets == want


def test_hand_computed_small_example(tmp_path):
    # 100 BPM at 60 Hz: beat = 36 frames, a third of a beat = 12 frames exactly; 0.5 beat = 18.
    # Beats B5 0.5, C6 0.5, B5 1/3 (as 0.3333 -> 11.9988 -> rounded), D6 1: cumulative rounding.
    text = "B5 0.5\nC6 0.5\nB5 0.3333\nD6 1\n"
    tf = g6.melody_file(write_melody(tmp_path, text), PROF, Fraction(100), max_press_ms=None)  # the rules, uncapped
    lead_in = PROF.start_lead_frames
    assert [p.onset for p in tf.notes] == [lead_in, lead_in + 18, lead_in + 36, lead_in + 48]
    b1, c6, b2, d6 = tf.notes
    assert b1.release == c6.onset - PROF.min_gap        # same button Y (C6 = Y + D-up): min gap wins
    assert c6.mods_at == c6.onset - PROF.lead           # D-up set modifier_lead_frames early
    assert c6.release == b2.onset - PROF.min_gap        # Y again
    assert b2.release == d6.onset                       # Y -> X, same (neutral) modifiers: hand-over
    assert d6.release == lead_in + 48 + 36              # last note ends on the grid
    assert tf.rows[-1].frame == d6.release + g6.END_TAIL_FRAMES


def test_cumulative_rounding_does_not_drift(tmp_path):
    # 7 notes of 1/3 beat at 70 BPM (beat 51.43 frames): per-note rounding would drift, cumulative does not.
    text = "".join("A5 0.3333333333\nB5 0.3333333334\nA5 0.3333333333\n" for _ in range(3))
    tf = g6.melody_file(write_melody(tmp_path, text), PROF, Fraction(70))
    last_end = tf.notes[-1].release
    assert last_end == PROF.start_lead_frames + round(3 * 60 * PROF.frame_rate / 70)


def test_rest_is_neutral_and_resets_modifiers(tmp_path):
    tf = g6.melody_file(write_melody(tmp_path, "G5 1\nR 1\nG5 1\n"), PROF, Fraction(60), max_press_ms=None)
    g1, g2 = tf.notes
    assert g1.release == g1.onset + 60 and not g1.contiguous
    rows = tf.rows
    rest_row = next(r for r in rows if r.frame == g1.release)
    assert rest_row.button == "-" and rest_row.hat == 8 and rest_row.stick == g6.g.CENTRE
    assert g2.mods_at == g2.onset - PROF.lead  # stick set again before the next press


# ------------------------------------------------------------- errors (3.5)


def test_pitch_out_of_range(tmp_path):
    with pytest.raises(g6.GuardError, match=r"note 02 .*outside the profile range"):
        g6.melody_file(write_melody(tmp_path, "A5 1\nA4 1\n"), PROF, Fraction(90))
    with pytest.raises(g6.GuardError, match="outside the profile range"):
        g6.melody_file(write_melody(tmp_path, "G6 1\n"), PROF, Fraction(90))


def test_pitch_without_a_chart_row(tmp_path):
    prof = g6.Profile(PROF.path, PROF.frame_rate, PROF.buttons, PROF.modifiers, PROF.low, PROF.high, PROF.timing,
                      tuple(f for f in PROF.fingerings if not (f.pitch == 81 and f.chart)))
    with pytest.raises(g6.GuardError, match=r"note 01 .*A5 \(81\) has 0 chart fingerings"):
        g6.melody_file(write_melody(tmp_path, "A5 1\n"), prof, Fraction(90))


def test_absurd_tempo_press_too_short():
    with pytest.raises(g6.GuardError, match=r"note \d\d .*shorter than min_press_frames"):
        g6.melody_file(MELODY, PROF, Fraction(400))


@pytest.mark.parametrize("bpm", [Fraction(29), Fraction(1)])
def test_tempo_below_the_sane_bound(bpm):
    with pytest.raises(g6.GuardError, match="sane bound"):
        g6.melody_file(MELODY, PROF, bpm)


def test_bad_melody_lines(tmp_path):
    for text in ("H5 1\n", "A5\n", "A5 0\n", "A5 x\n", "# only a comment\n"):
        with pytest.raises(g6.GuardError):
            g6.melody_file(write_melody(tmp_path, text), PROF, Fraction(90))
    assert g6.parse_pitch("Bb5") == 82 and g6.parse_pitch("C#6") == 85 and g6.parse_pitch("A5") == 81


def test_long_note_is_re_pressed_after_the_min_gap(tmp_path):
    # A 4-beat note at 30 BPM = 8 s, longer than max_sustain_ms: re-pressed, never held past the limit.
    tf = g6.melody_file(write_melody(tmp_path, "A5 4\n"), PROF, Fraction(30), max_press_ms=None)
    (p,) = tf.notes
    assert len(p.segments) >= 2
    for (a, b), (c, _) in zip(p.segments, p.segments[1:]):
        assert c - b == PROF.min_gap
    assert all((b - a) <= PROF.max_sustain_frames for a, b in p.segments)
    assert p.segments[0][0] == p.onset and p.segments[-1][1] == p.release
    assert sum(1 for r in tf.rows if r.label.startswith("REPRESS 01")) == len(p.segments) - 1
    g6.check_rows(list(tf.rows), PROF)


def test_split_sustain():
    assert g6.split_sustain(0, 300, 300, 5) == ((0, 300),)
    segs = g6.split_sustain(0, 301, 300, 5)
    assert len(segs) == 2 and segs[-1][1] == 301 and segs[1][0] - segs[0][1] == 5
    segs = g6.split_sustain(10, 10 + 700, 300, 5)
    assert len(segs) == 3 and all(b - a <= 300 for a, b in segs)


def test_check_rows_catches_each_rule():
    R = g6.Row
    up = g6.g.HAT_UP
    ok = [R(0, "-"), R(30, "A"), R(40, "-"), R(100, "-")]
    g6.check_rows(ok, PROF)
    cases = {
        "min_press_frames": [R(0, "-"), R(30, "A"), R(30 + PROF.min_press - 1, "-"), R(100, "-")],
        "min_gap_same_button": [R(0, "-"), R(30, "A"), R(40, "-"), R(40 + PROF.min_gap - 1, "A"), R(60, "-"),
                                R(100, "-")],
        "modifier_lead_frames": [R(0, "-"), R(30, "A", up), R(40, "-"), R(100, "-")],
        "U4": [R(0, "-"), R(20, "-", up), R(30, "A", up), R(35, "A"), R(45, "-"), R(100, "-")],
        "max_sustain_ms": [R(0, "-"), R(30, "A"), R(31 + PROF.max_sustain_frames, "-"), R(400, "-")],
    }
    for what, rows in cases.items():
        with pytest.raises(g6.GuardError, match=what):
            g6.check_rows(rows, PROF)


def test_allow_list_rejects_forbidden_inputs():
    for bad in ("B", "ZL", "ZR", "L+R"):
        with pytest.raises(g6.GuardError):
            g6.Row(0, bad)
    with pytest.raises(g6.GuardError):
        g6.Row(0, "A", 6)  # D-pad left


# ------------------------------------------------------------- U1 files (2.7)


TRY_RE = re.compile(r"TRY u1 (\w) (?:hold_ms=(\d+)|blank) attempt=(\d+) round (\d+)/(\d+)( control)?$")


def u1_tries(rows):
    return [(r, TRY_RE.match(r.label)) for r in rows if r.label.startswith("TRY")]


def test_u1_round1_design():
    names = [f"u1-sustain-A-part{k}.txt" for k in range(1, 6)]
    holds = []
    for k, name in enumerate(names, start=1):
        rows = parsed(name)
        assert rows[1].label.startswith("WARMUP")
        tries = u1_tries(rows)
        assert all(m for _, m in tries)
        assert {int(m.group(4)) for _, m in tries} == {k}
        holds += [m.group(2) for _, m in tries]
    want = [str(h) for h in g6.U1Options().holds_ms]
    for h in want:
        assert holds.count(h) == 5
    assert holds.count("10000") == 5 and holds.count("1000") == 5 and holds.count(None) == 5
    orders = [[m.group(2) for _, m in u1_tries(parsed(n))] for n in names]
    assert len({tuple(o) for o in orders}) > 1  # shuffled, not blocks


@pytest.mark.parametrize("name", [f"u1-sustain-A-part{k}.txt" for k in range(1, 6)])
def test_u1_hold_lengths_and_spacing(name):
    rows = parsed(name)
    fr = frames_of(rows)
    for i, r in enumerate(rows):
        m = TRY_RE.match(r.label)
        if not m:
            continue
        before = fr[i] - fr[i - 1]
        assert before >= 30 and rows[i - 1].state == s.NEUTRAL
        if m.group(2) is None:
            assert r.state == s.NEUTRAL
            continue
        assert r.state.buttons == s.CANON_BUTTONS["A"] and r.state.hat == 8 and r.state.stick == s.Stick()
        assert fr[i + 1] - fr[i] == int(m.group(2)) * PROF.frame_rate // 1000
        assert rows[i + 1].state == s.NEUTRAL
        assert fr[i + 2] - fr[i + 1] >= 3 * PROF.frame_rate  # 3 s of neutral after the release


def test_u1_guards():
    with pytest.raises(g6.GuardError, match="12000"):
        g6.u1_round1(g6.U1Options(holds_ms=(12500,)), PROF)
    with pytest.raises(g6.GuardError, match="frame grid"):
        g6.u1_round1(g6.U1Options(holds_ms=(4001,)), PROF)
    with pytest.raises(g6.GuardError, match="not allowed"):
        g6.u1_round1(g6.U1Options(button="B"), PROF)
    with pytest.raises(g6.GuardError, match="not allowed"):
        g6.u1_confirm(g6.U1Options(confirm_ms=5000, buttons=("ZL",)), PROF)


def test_u1_confirm_files():
    files = g6.u1_confirm(g6.U1Options(confirm_ms=5500), PROF)
    assert [tf.name for tf in files] == [f"u1-confirm-{b}.txt" for b in "LRYX"]
    for tf, b in zip(files, "LRYX"):
        rows = s.parse_lines(g6.render(tf).splitlines())
        tries = u1_tries(rows)
        holds = [m.group(2) for _, m in tries]
        assert holds.count("5500") == 5 and holds.count("6000") == 5  # the candidate and the next tested
        assert all(m.group(1) == b for _, m in tries)
        assert all(r.state.buttons == s.CANON_BUTTONS[b] for r, _ in tries)
    (tf,) = g6.u1_confirm(g6.U1Options(confirm_ms=7000, buttons=("X",)), PROF)
    assert "hold_ms=7500" in g6.render(tf)  # beyond the tested list: +500 ms


# ------------------------------------------------------------- diagnostic variants (Story 1.6 Part B diagnosis)


def test_diagnostic_subset_and_extra_lead():
    full = g6.melody_file(MELODY, PROF, Fraction(60))
    tf = g6.melody_file(MELODY, PROF, Fraction(60), (1, 16), 4000)
    assert tf.name == "ode-to-joy-60bpm-n01-16-lead4000ms.txt"
    assert [p.index for p in tf.notes] == list(range(1, 17))
    shift = 4 * PROF.frame_rate
    for a, b in zip(full.notes[:16], tf.notes):  # same states, only later (the last release ends on its beat)
        assert b.onset - a.onset == shift and (b.index == 16 or b.release - a.release == shift)
        assert (a.fingering, a.mods) == (b.fingering, b.mods)
    assert "DIAGNOSTIC variant" in g6.render(tf)
    g6.check_rows(list(tf.rows), PROF)
    mid = g6.melody_file(MELODY, PROF, Fraction(60), (9, 16))
    assert [p.index for p in mid.notes] == list(range(9, 17)) and mid.notes[0].onset == PROF.start_lead_frames
    assert "DIAGNOSTIC" not in g6.render(full)


def test_diagnostic_cli(tmp_path, capsys):
    assert g6.main(["--set", "melody", "--bpm", "60", "--notes", "1-16", "--extra-lead-ms", "4000",
                    "--out-dir", str(tmp_path)]) == 0
    assert (tmp_path / "ode-to-joy-60bpm-n01-16-lead4000ms.txt").exists()
    assert g6.main(["--set", "melody", "--notes", "5-70", "--out-dir", str(tmp_path)]) == 1
    assert "notes 1..62" in capsys.readouterr().err
    with pytest.raises(SystemExit):
        g6.main(["--set", "melody", "--notes", "x"])


def test_diagnostic_max_press_staccato():
    tf = g6.melody_file(MELODY, PROF, Fraction(60), max_press_ms=300)
    assert tf.name == "ode-to-joy-60bpm-maxpress300ms.txt" and "DIAGNOSTIC" in g6.render(tf)
    full = g6.melody_file(MELODY, PROF, Fraction(60), max_press_ms=None)
    assert full.name == "ode-to-joy-60bpm-uncapped.txt"
    for a, b in zip(full.notes, tf.notes):
        assert b.onset == a.onset and b.pressed_frames() <= round(0.3 * PROF.frame_rate)
    g6.check_rows(list(tf.rows), PROF)
    runs = g6.press_runs(list(tf.rows))
    assert all(runs[k + 1][1] > runs[k][2] for k in range(len(runs) - 1))  # no hand-over left: neutral between
    with pytest.raises(g6.GuardError, match="below min_press_frames"):
        g6.melody_file(MELODY, PROF, Fraction(60), max_press_ms=10)


def test_committed_melody_files_use_the_press_cap_and_u1_files_do_not():
    cap = round(g6.DEFAULT_MAX_PRESS_MS * PROF.frame_rate / 1000)
    for bpm in g6.COMMITTED_BPMS:
        name = f"ode-to-joy-{bpm}bpm.txt"
        runs = g6.press_runs(list(FILES[name].rows))
        assert max(e - a for _, a, e, _ in runs) == cap  # long notes are capped, staccato
        assert all(runs[k + 1][1] > runs[k][2] for k in range(len(runs) - 1) if runs[k][2] - runs[k][1] == cap)
        text = (SAMPLES / name).read_text(encoding="utf-8")
        assert f"# Press cap: {g6.DEFAULT_MAX_PRESS_MS} ms (Story 1.6 workaround" in text and "DIAGNOSTIC" not in text
    for k in range(1, 6):
        runs = g6.press_runs(list(FILES[f"u1-sustain-A-part{k}.txt"].rows))
        assert max(e - a for _, a, e, _ in runs) == 10 * PROF.frame_rate  # the 10 s control is not capped


def test_uncapped_variant_from_the_cli(tmp_path):
    assert g6.main(["--set", "melody", "--bpm", "90", "--max-press-ms", "0", "--out-dir", str(tmp_path)]) == 0
    text = (tmp_path / "ode-to-joy-90bpm-uncapped.txt").read_text()
    assert "# Press cap: none" in text and "DIAGNOSTIC variant" in text


# ------------------------------------------------------------- review fixes


def test_melody_comments_and_sharps(tmp_path):
    items = g6.parse_melody("C#6 1  # a sharp, then a comment\n# whole-line comment\nBb5 0.5 #tight comment\n")
    assert [(i.midi, i.beats) for i in items] == [(85, 1), (82, Fraction(1, 2))]
    tf = g6.melody_file(write_melody(tmp_path, "C#6 1  # chart C#6 = X + D-down\nD6 1\n"), PROF, Fraction(90))
    assert [p.midi for p in tf.notes] == [85, 86] and tf.notes[0].fingering.text() == "X+D-down"


def test_modifier_before_the_file_start_is_refused(tmp_path):
    t = dict(PROF.timing, start_lead_ms=0)
    prof = g6.Profile(PROF.path, PROF.frame_rate, PROF.buttons, PROF.modifiers, PROF.low, PROF.high, t,
                      PROF.fingerings)
    with pytest.raises(g6.GuardError, match="note 01: its modifiers would be set before the start"):
        g6.melody_file(write_melody(tmp_path, "C6 1\n"), prof, Fraction(90))
    assert g6.melody_file(write_melody(tmp_path, "C6 1\n"), prof, Fraction(90), extra_lead_ms=100).notes


def test_split_sustain_guards():
    with pytest.raises(g6.GuardError, match="at least one frame"):
        g6.split_sustain(0, 100, 0, 5)
    t = dict(PROF.timing, max_sustain_ms=20)  # 1 frame, below min_press_frames
    prof = g6.Profile(PROF.path, PROF.frame_rate, PROF.buttons, PROF.modifiers, PROF.low, PROF.high, t,
                      PROF.fingerings)
    with pytest.raises(g6.GuardError, match="shorter than min_press_frames"):
        g6.melody_file(MELODY, prof, Fraction(90))
