# SPDX-License-Identifier: MIT
# Throwaway spike tests for spikes/send_2wicc.py. No hardware needed.
# Run: uv run --with pytest --with pyserial pytest spikes/test_send_2wicc.py
from __future__ import annotations

import importlib.util
import os
import pathlib
import sys
import threading

import pytest

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("send_2wicc", HERE / "send_2wicc.py")
s = importlib.util.module_from_spec(_spec)
sys.modules["send_2wicc"] = s
_spec.loader.exec_module(s)


# --------------------------------------------------------------------- parser


def test_parse_valid_file_with_comments_and_blanks():
    rows = s.parse_lines([
        "# header comment",
        "",
        "0   -  -  -",
        "500 A        # press",
        "   ",
        "1000 L+A up 80:40",
    ])
    assert [r.line for r in rows] == [3, 4, 6]
    assert rows[0].state == s.NEUTRAL
    assert rows[1].state == s.State(buttons=s.CANON_BUTTONS["A"])
    assert rows[2].state.buttons == s.CANON_BUTTONS["L"] | s.CANON_BUTTONS["A"]
    assert rows[2].state.hat == 0
    assert rows[2].state.stick == s.Stick(0x80, 0x40)


def test_parse_button_names_and_hex():
    assert s.parse_buttons("-") == 0
    assert s.parse_buttons("y+x") == s.CANON_BUTTONS["Y"] | s.CANON_BUTTONS["X"]
    assert s.parse_buttons("0x0014") == s.CANON_BUTTONS["A"] | s.CANON_BUTTONS["L"]
    with pytest.raises(s.ParseError):
        s.parse_buttons("0x4000")
    with pytest.raises(s.ParseError):
        s.parse_buttons("0xZZ")


@pytest.mark.parametrize("tok,hat", [("-", 8), ("0", 0), ("8", 8), ("up", 0), ("down-left", 5), ("LEFT", 6)])
def test_parse_hat(tok, hat):
    assert s.parse_hat(tok) == hat


def test_parse_hat_errors():
    for bad in ("9", "north"):
        with pytest.raises(s.ParseError):
            s.parse_hat(bad)


def test_parse_stick_forms():
    assert s.parse_stick("-") == s.Stick()
    assert s.parse_stick("80:00") == s.Stick(0x80, 0x00)
    assert s.parse_stick("800:5A0") == s.Stick(0x800, 0x5A0, raw=True)
    for bad in ("80", "80:0", "8:00", "GG:00", "800:80"):
        with pytest.raises(s.ParseError):
            s.parse_stick(bad)


def test_parse_errors_have_line_numbers():
    with pytest.raises(s.ParseError, match=r"line 2: unknown button 'Q'"):
        s.parse_lines(["0 -", "10 Q"])
    with pytest.raises(s.ParseError, match=r"line 3: bad stick hex"):
        s.parse_lines(["0", "", "10 - - ZZ:00"])
    with pytest.raises(s.ParseError, match=r"line 1: bad time"):
        s.parse_lines(["abc A"])
    with pytest.raises(s.ParseError, match=r"line 1: too many columns"):
        s.parse_lines(["0 A - - extra"])


def test_out_of_order_time_rejected_with_line():
    rows = s.parse_lines(["0 -", "500 A", "400 -"])
    with pytest.raises(s.ParseError, match=r"line 3: time goes backwards"):
        s.schedule(rows, 60)


# -------------------------------------------------------------------- encoder


def st(buttons="-", hat="-", stick="-"):
    return s.State(s.parse_buttons(buttons), s.parse_hat(hat), s.parse_stick(stick))


def test_neutral_encoding():
    assert s.encode(s.NEUTRAL) == "+QD 000000"
    assert s.STOP_SEQUENCE == ("+SPM RT", "+QD 000000")


def test_a_matches_spike_0b():
    assert s.encode(st("A")) == "+QD 080000"


@pytest.mark.parametrize("button,cmd", [
    ("L", "+QD 000040"), ("R", "+QD 400000"), ("A", "+QD 080000"),
    ("Y", "+QD 010000"), ("X", "+QD 020000"), ("B", "+QD 040000"),
    ("ZL", "+QD 000080"), ("ZR", "+QD 800000"), ("PLUS", "+QD 000200"),
    ("MINUS", "+QD 000100"), ("HOME", "+QD 001000"), ("CAPTURE", "+QD 002000"),
    ("L3", "+QD 000800"), ("R3", "+QD 000400"),
])
def test_each_button(button, cmd):
    assert s.encode(st(button)) == cmd


@pytest.mark.parametrize("hat,byte3", [
    (0, 0x02), (1, 0x06), (2, 0x04), (3, 0x05), (4, 0x01), (5, 0x09), (6, 0x08), (7, 0x0A), (8, 0x00),
])
def test_each_hat_direction(hat, byte3):
    assert s.encode(s.State(hat=hat)) == f"+QD 0000{byte3:02X}"


def test_buttons_and_hat_combined():
    assert s.encode(st("L+A", "up")) == "+QD 080042"


def test_stick_centre_is_neutral():
    assert s.stick_axis_to_12(0x80, False) == 0x800
    assert s.stick_axis_to_12(0x80, True) == 0x800
    assert s.pack_stick(0x800, 0x800) == bytes.fromhex("000880")
    assert s.encode(st(stick="80:80")) == "+QD 000000"
    assert s.encode(st(stick="800:800")) == "+QD 000000"


def test_stick_8_to_12_bit_range():
    assert s.stick_axis_to_12(0x00, False) == 0x200
    assert s.stick_axis_to_12(0xFF, False) == 0xDF4
    assert s.stick_axis_to_12(0x40, False) == 0x500
    # canonical ly 0x00 = up -> high 12-bit Y under the HIGHER_Y_IS_UP assumption
    assert s.stick_axis_to_12(0x00, True) == 0xE00
    assert s.stick_axis_to_12(0xFF, True) == 0x20C


def test_pack_stick_layout():
    # X = b0 | (b1 & 0xF) << 8 ; Y = b1 >> 4 | b2 << 4
    b = s.pack_stick(0xABC, 0x123)
    assert b == bytes((0xBC, 0x3A, 0x12))
    x = b[0] | (b[1] & 0x0F) << 8
    y = (b[1] >> 4) | (b[2] << 4)
    assert (x, y) == (0xABC, 0x123)


def test_stick_uses_qf_with_right_stick_neutral():
    cmd = s.encode(st("A", "-", "800:E00"))
    assert cmd == "+QF 080000" + "0008E0" + "000880"
    assert len(cmd.split()[1]) == 18


# ------------------------------------------------------------------ scheduler


def rows(*lines):
    return s.parse_lines(lines)


def test_frame_count_and_hold():
    frames, dropped = s.schedule(rows("0 -", "500 A", "1000 -"), 60)
    assert len(frames) == 61  # 30 neutral + 30 A + last row held 1 frame
    assert frames[:30] == [s.NEUTRAL] * 30
    assert frames[30:60] == [st("A")] * 30
    assert frames[60] == s.NEUTRAL
    assert dropped == []


def test_leading_gap_is_neutral():
    frames, _ = s.schedule(rows("100 A"), 60)
    assert frames == [s.NEUTRAL] * 6 + [st("A")]


def test_one_frame_press_with_frame_suffix():
    frames, _ = s.schedule(rows("0f -", "10f A", "11f -"), 60)
    assert len(frames) == 12
    assert frames[10] == st("A")
    assert frames[9] == frames[11] == s.NEUTRAL


def test_rounding_and_dropped_rows():
    assert s.time_to_frame(s.Time(8.3, False), 60) == 0
    assert s.time_to_frame(s.Time(8.4, False), 60) == 1
    assert s.time_to_frame(s.Time(25.0, False), 60) == 2  # 1.5 rounds half up
    frames, dropped = s.schedule(rows("0 -", "1 A", "2 -"), 60)
    assert [r.line for r in dropped] == [1, 2]
    assert frames == [s.NEUTRAL]


def test_empty_file():
    assert s.schedule([], 60) == ([], [])


# ---------------------------------------------------------------------- pacer


class FakeClock:
    def __init__(self):
        self.t = 0.0

    def __call__(self):
        return self.t

    def sleep(self, d):
        self.t += d


class Recorder:
    def __init__(self, clock=None, rate=60.0):
        self.clock = clock
        self.rate = rate
        self.lines: list[str] = []
        self.max_ahead = 0
        self.queued = 0

    def __call__(self, data: bytes):
        new = data.decode().splitlines()
        self.lines += new
        self.queued += sum(1 for line in new if line.startswith("+Q"))
        if self.clock is not None:
            played = int(self.clock() * self.rate)
            self.max_ahead = max(self.max_ahead, self.queued - played)

    def queue_lines(self):
        out = []
        for line in self.lines:
            if line == "+SPM RT":
                break
            if line.startswith("+Q"):
                out.append(line)
        return out


def cmds(n):
    return [f"+QD {i % 256:02X}0000" for i in range(n)]


@pytest.mark.parametrize("n", [1, 29, 30, 31, 61, 5000])
def test_pacer_sends_every_frame_once_in_order(n):
    clock = FakeClock()
    rec = Recorder(clock)
    s.Pacer(rec, clock, clock.sleep, 60, 30).run(cmds(n))
    assert rec.lines[0] == "+SPM BUF"
    assert rec.queue_lines() == cmds(n)
    assert rec.max_ahead <= 30


def test_pacer_long_file_stays_far_below_queue_and_lead_positive():
    clock = FakeClock()
    rec = Recorder(clock)
    p = s.Pacer(rec, clock, clock.sleep, 60, 30)
    p.run(cmds(5000))
    assert p.max_seen_lead <= 30 < s.QUEUE_CAPACITY
    assert p.min_seen_lead > 0
    assert rec.max_ahead <= 30
    # it waited for playback to finish (5000 frames ~ 83.3 s)
    assert clock.t >= 5000 / 60


def test_pacer_bursts_fit_serial_command_buffer():
    clock = FakeClock()
    sizes = []
    s.Pacer(lambda d: sizes.append(d.count(b"\n")), clock, clock.sleep, 60, s.MAX_LEAD_LIMIT).run(cmds(500))
    assert max(sizes) <= s.MAX_LEAD_LIMIT + 1 < s.SERIAL_CMD_BUFFER


def test_pacer_rejects_bad_lead():
    for bad in (0, s.MAX_LEAD_LIMIT + 1):
        with pytest.raises(ValueError):
            s.Pacer(print, lambda: 0, lambda d: None, 60, bad)


def test_pacer_detects_underrun():
    clock = FakeClock()

    def slow_sleep(d):  # host stalls for one second once
        clock.t += 1.0

    with pytest.raises(s.Underrun):
        s.Pacer(lambda d: None, clock, slow_sleep, 60, 30).run(cmds(200))


# ------------------------------------------------------------------------ exit


def test_neutral_last_after_normal_end():
    clock = FakeClock()
    rec = Recorder()
    stats = {}
    code = s.play(cmds(100), rec, clock, clock.sleep, 60, 30, stats)
    assert code == 0
    assert stats["sent"] == 100
    assert rec.lines[-2:] == ["+SPM RT", "+QD 000000"]


def test_neutral_last_after_ctrl_c_mid_run():
    clock = FakeClock()
    rec = Recorder()
    calls = {"n": 0}

    def sleep(d):
        calls["n"] += 1
        if calls["n"] == 10:
            raise KeyboardInterrupt
        clock.sleep(d)

    stats = {}
    code = s.play(cmds(1000), rec, clock, sleep, 60, 30, stats)
    assert code == 130
    assert stats["sent"] == len(rec.queue_lines()) < 1000
    assert rec.lines[-2:] == ["+SPM RT", "+QD 000000"]
    assert len(rec.queue_lines()) < 1000


def test_neutral_last_after_writer_error():
    clock = FakeClock()
    rec = Recorder()
    state = {"n": 0}

    def write(data):
        state["n"] += 1
        if state["n"] == 3:
            raise OSError("serial gone")
        rec(data)

    code = s.play(cmds(1000), write, clock, clock.sleep, 60, 30)
    assert code == 1
    assert rec.lines[-2:] == ["+SPM RT", "+QD 000000"]


def test_second_ctrl_c_during_stop_does_not_skip_it():
    rec = Recorder()
    state = {"n": 0}

    def write(data):
        state["n"] += 1
        if state["n"] == 1:
            raise KeyboardInterrupt
        rec(data)

    assert s.send_stop(write) is True
    assert rec.lines == ["+SPM RT", "+QD 000000"]


def test_dry_run_prints_lines_and_opens_no_port(tmp_path, capsys, monkeypatch):
    def boom(*a, **k):
        raise AssertionError("port opened in dry run")

    monkeypatch.setattr(s, "open_serial", boom)
    f = tmp_path / "in.txt"
    f.write_text("0 -\n500 A\n1000 -\n")
    assert s.main(["--dry-run", str(f)]) == 0
    out = capsys.readouterr().out.splitlines()
    assert out[0].endswith("+SPM BUF")
    assert out[31].split()[:3] == ["30", "500.0ms", "+QD"] and out[31].endswith("+QD 080000")
    assert out[-3].endswith("+SPM RT") and out[-2].endswith("+QD 000000")
    assert out[-1] == "summary: 3 rows, 61 frames, 1.017 s, port (dry run)"


def test_cli_errors(tmp_path, capsys):
    f = tmp_path / "in.txt"
    f.write_text("0 Q\n")
    assert s.main([str(f)]) == 1  # no --port
    assert s.main(["--dry-run", str(f)]) == 1  # parse error
    assert "line 1" in capsys.readouterr().err
    assert s.main(["--dry-run", "--max-lead-frames", "500", str(f)]) == 1


def test_samples_parse():
    for name in ("a-press.txt", "scale-lrayx.txt"):
        frames, dropped = s.schedule(s.parse_lines((HERE / "samples" / name).read_text().splitlines()), 60)
        assert frames and not dropped
        assert frames[-1] == s.NEUTRAL
    frames, _ = s.schedule(s.parse_lines((HERE / "samples" / "scale-lrayx.txt").read_text().splitlines()), 60)
    order = []
    for f in frames:
        if f.buttons and (not order or order[-1] != f.buttons):
            order.append(f.buttons)
    assert order == [s.CANON_BUTTONS[b] for b in ["L", "R", "A", "Y", "X"]]


# ------------------------------------------------------- review fixes (2026-11-20)


@pytest.mark.parametrize("tok", ["-1:80", "80:-1", "+8:80", " 8:80", "0x8:80", "8_:80", "８０:80", "²²:80"])
def test_stick_rejects_non_pure_hex(tok):
    with pytest.raises(s.ParseError):
        s.parse_stick(tok)


@pytest.mark.parametrize("tok", ["0x-1", "0x+4", "0x 4", "0x", "0x0_4", "0x４"])
def test_buttons_reject_non_pure_hex(tok):
    with pytest.raises(s.ParseError):
        s.parse_buttons(tok)


@pytest.mark.parametrize("tok", ["²", "٣", "+1", "-1", "1.0", "09x"])
def test_hat_rejects_odd_digits(tok):
    with pytest.raises(s.ParseError):
        s.parse_hat(tok)


@pytest.mark.parametrize("tok", ["-5", "+5", "1e3", "nan", "inf", "1_000", "٣", "²f", "-1f", " 5", "5ms"])
def test_time_rejects_odd_forms(tok):
    with pytest.raises(s.ParseError):
        s.parse_time(tok)


def test_time_accepts_plain_forms():
    assert s.parse_time("12.5") == s.Time(12.5, False)
    assert s.parse_time("3F") == s.Time(3, True)


def test_schedule_length_capped():
    ok = s.parse_lines([f"{s.MAX_FRAMES - 1}f A"])
    assert len(s.schedule(ok, 60)[0]) == s.MAX_FRAMES
    with pytest.raises(s.ParseError, match=r"line 2: time is beyond"):
        s.schedule(s.parse_lines(["0 -", "99999999999 A"]), 60)


def test_main_handles_bom_and_bad_encoding(tmp_path, capsys):
    f = tmp_path / "bom.txt"
    f.write_bytes(b"\xef\xbb\xbf0 A\n")
    assert s.main(["--dry-run", str(f)]) == 0
    g = tmp_path / "bad.txt"
    g.write_bytes(b"0 A\n\xff\xfe\n")
    assert s.main(["--dry-run", str(g)]) == 1
    assert "not UTF-8" in capsys.readouterr().err


real_signals = pytest.mark.skipif(
    sys.platform == "win32" or threading.current_thread() is not threading.main_thread(),
    reason="real signals: POSIX main thread only (os.kill SIGTERM terminates the process on Windows)",
)


def _all_handlers():
    return {sig: s.signal.getsignal(sig) for sig in s.STOP_SIGNALS}


@real_signals
@pytest.mark.parametrize("signame", ["SIGTERM", "SIGHUP"])
def test_sigterm_and_sighup_send_stop(signame):
    if not hasattr(s.signal, signame):
        pytest.skip(f"no {signame}")
    signum = getattr(s.signal, signame)
    before = _all_handlers()
    clock = FakeClock()
    rec = Recorder()
    during_stop = {}
    calls = {"n": 0}

    def write(data):
        if data.startswith(b"+SPM RT"):
            during_stop.update(_all_handlers())
        rec(data)

    def sleep(d):
        calls["n"] += 1
        if calls["n"] == 10:
            os.kill(os.getpid(), signum)
        clock.sleep(d)

    code = s.play(cmds(1000), write, clock, sleep, 60, 30)
    assert code == 128 + signum
    assert rec.lines[-2:] == ["+SPM RT", "+QD 000000"]
    assert len(rec.queue_lines()) < 1000
    assert all(h == s.signal.SIG_IGN for h in during_stop.values())  # ignored during the stop
    assert _all_handlers() == before  # SIGINT, SIGTERM, SIGHUP all restored


@real_signals
def test_second_signal_at_start_of_finally_does_not_skip_stop():
    """A SIGTERM arriving while the finally block starts (here: when stats is
    written) must not escape play(): signals are ignored first."""
    before = _all_handlers()

    class KillOnSet(dict):
        def __setitem__(self, k, v):
            os.kill(os.getpid(), s.signal.SIGTERM)
            super().__setitem__(k, v)

    clock = FakeClock()
    rec = Recorder()
    calls = {"n": 0}

    def sleep(d):
        calls["n"] += 1
        if calls["n"] == 10:
            raise KeyboardInterrupt
        clock.sleep(d)

    stats = KillOnSet()
    code = s.play(cmds(1000), rec, clock, sleep, 60, 30, stats)
    assert code == 130
    assert stats["sent"] > 0
    assert rec.lines[-2:] == ["+SPM RT", "+QD 000000"]
    assert _all_handlers() == before


@pytest.mark.parametrize("tok", ["1" * 400, "1" * 5000 + "f", "9" * 33])
def test_time_rejects_huge_tokens(tok):
    with pytest.raises(s.ParseError):
        s.parse_time(tok)


def test_huge_time_in_file_is_a_clean_error(tmp_path, capsys):
    f = tmp_path / "big.txt"
    f.write_text("0 -\n" + "9" * 31 + " A\n")
    assert s.main(["--dry-run", str(f)]) == 1
    assert "beyond" in capsys.readouterr().err


@pytest.mark.parametrize("rate", ["nan", "inf", "-inf", "0", "-60"])
def test_frame_rate_must_be_finite_positive(tmp_path, capsys, rate):
    f = tmp_path / "in.txt"
    f.write_text("0 A\n")
    assert s.main(["--dry-run", f"--frame-rate={rate}", str(f)]) == 1
    assert "--frame-rate" in capsys.readouterr().err


# -------------------------------------------------------- labels / --announce (Story 1.3)


def test_label_extracted_from_comment():
    rows = s.parse_lines([
        "# header only, no row",
        "0 - - -",
        "500 A - 80:40   #  A ly=0x40 up try 3/5 ",
        "1000 - - -  # x=0x40 # second hash kept",
        "1500 L",
    ])
    assert [r.label for r in rows] == ["", "A ly=0x40 up try 3/5", "x=0x40 # second hash kept", ""]
    # the states are what they were without labels
    assert rows[1].state == s.State(s.CANON_BUTTONS["A"], s.HAT_NEUTRAL, s.Stick(0x80, 0x40))
    assert rows[3].state == s.State(s.CANON_BUTTONS["L"])


def test_announcements_skip_unlabelled_rows_and_use_first_frame():
    rows = s.parse_lines(["0 - # start", "500 A", "1000 - # release"])
    assert s.announcements(rows, 60) == [(0, "start"), (60, "release")]


def _labelled_commands():
    rows = s.parse_lines(["0 - # start", "500 A # press A", "1000 - # release", "1500 R", "2000 - # end"])
    frames, _ = s.schedule(rows, 60)
    return [s.encode(f) for f in frames], s.announcements(rows, 60)


def test_announce_prints_when_played_in_order():
    commands, ann = _labelled_commands()
    clock = FakeClock()
    printed = []

    def emit(text):
        printed.append((clock(), text))

    p = s.Pacer(lambda d: None, clock, clock.sleep, 60, 30, ann, emit)
    p.run(commands)
    assert [t for _, t in printed] == ["t=0.0s  start", "t=0.5s  press A", "t=1.0s  release", "t=2.0s  end"]
    for (when, _), (frame, _) in zip(printed, ann):
        # printed once the frame is played (not when queued), and within one frame of it
        assert frame / 60 <= when + 1e-9 < frame / 60 + 1 / 60 + 1e-9


def test_announce_does_not_change_bytes():
    commands, ann = _labelled_commands()
    runs = []
    for a in ((), ann):
        clock = FakeClock()
        rec = Recorder(clock)
        assert s.play(commands, rec, clock, clock.sleep, 60, 30, None, a, lambda t: None) == 0
        runs.append(rec.lines)
    assert runs[0] == runs[1]


def test_no_announce_prints_nothing(capsys):
    commands, _ = _labelled_commands()
    clock = FakeClock()
    s.play(commands, Recorder(), clock, clock.sleep, 60, 30)
    assert capsys.readouterr().out == ""


def test_dry_run_announce_shows_labels_and_plain_output_unchanged(tmp_path, capsys):
    f = tmp_path / "in.txt"
    f.write_text("0 -  # lead-in\n500 A  # PRESS A ly=0x40\n1000 -\n")
    assert s.main(["--dry-run", str(f)]) == 0
    plain = capsys.readouterr().out.splitlines()
    assert not any("#" in line for line in plain)
    assert s.main(["--dry-run", "--announce", str(f)]) == 0
    ann = capsys.readouterr().out.splitlines()
    assert len(ann) == len(plain)
    assert ann[1] == plain[1] + "  # lead-in"
    assert ann[31] == plain[31] + "  # PRESS A ly=0x40"
    assert [a for a, b in zip(ann, plain) if a != b] == [ann[1], ann[31]]


def test_main_announce_flag_bytes_identical(tmp_path, monkeypatch):
    f = tmp_path / "in.txt"
    f.write_text("0 - # a\n100 A # b\n200 -\n")
    sent = {}

    class FakeSerial:
        def __init__(self):
            self.data = b""

        def write(self, d):
            self.data += d

        def flush(self):
            pass

        def close(self):
            pass

    clock = FakeClock()
    monkeypatch.setattr(s, "time", type("T", (), {"monotonic": clock, "sleep": clock.sleep}))
    for flag in ([], ["--announce"]):
        ser = FakeSerial()
        monkeypatch.setattr(s, "open_serial", lambda port, baud, ser=ser: ser)
        assert s.main(["--port", "fake", *flag, str(f)]) == 0
        sent[bool(flag)] = ser.data
    assert sent[False] == sent[True] and b"+QD 080000" in sent[True]
