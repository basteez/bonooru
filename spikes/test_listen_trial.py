# SPDX-License-Identifier: MIT
# Throwaway spike tests for spikes/listen_trial.py. Synthetic audio only: no microphone, no port, no ffmpeg.
# Run: uv run --with pytest --with pyserial --with numpy pytest spikes/test_listen_trial.py
from __future__ import annotations

import importlib.util
import math
import pathlib
import sys

import pytest

np = pytest.importorskip("numpy")

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("listen_trial", HERE / "listen_trial.py")
lt = importlib.util.module_from_spec(_spec)
sys.modules["listen_trial"] = lt
_spec.loader.exec_module(lt)

SR = 48000
CHART = range(71, 90)  # B4..F6, the 19 ocarina pitches
RNG = np.random.default_rng(1234)


def midi_hz(m: float) -> float:
    return 440.0 * 2 ** ((m - 69) / 12)


def tone(f: float, dur: float, amps=(1.0,), level=0.3, sr=SR) -> np.ndarray:
    t = np.arange(int(dur * sr)) / sr
    x = sum(a * np.sin(2 * np.pi * f * (k + 1) * t + k) for k, a in enumerate(amps))
    return level * x / np.max(np.abs(x))


HARMONIC = (1.0, 0.6, 0.45, 0.3, 0.2, 0.1)


# ------------------------------------------------------------------ pitch


@pytest.mark.parametrize("m", CHART)
@pytest.mark.parametrize("amps", [(1.0,), HARMONIC], ids=["sine", "harmonic"])
def test_chart_pitches(m, amps):
    p = lt.estimate_pitch(tone(midi_hz(m), 0.6, amps), SR)
    assert p.hz is not None and p.voiced > 0.9
    n, cents = lt.note_and_cents(p.hz)
    assert n == m and abs(cents) < 3


@pytest.mark.parametrize("m", [71, 81, 89])
@pytest.mark.parametrize("detune", [-40, -15, 15, 40])
def test_detuning_reported_in_cents(m, detune):
    p = lt.estimate_pitch(tone(midi_hz(m + detune / 100), 0.6, HARMONIC), SR)
    n, cents = lt.note_and_cents(p.hz)
    assert n == m and abs(cents - detune) < 3


def test_strong_second_harmonic_no_octave_error():
    # second harmonic louder than the fundamental: still the fundamental
    p = lt.estimate_pitch(tone(midi_hz(76), 0.6, (0.6, 1.0, 0.3)), SR)
    assert lt.note_and_cents(p.hz)[0] == 76


@pytest.mark.parametrize("m", [71, 81, 89])
def test_noise_robustness(m):
    x = tone(midi_hz(m), 0.6, HARMONIC, level=0.3)
    noise = RNG.normal(0, 0.3 / math.sqrt(2) / math.sqrt(10), x.size)  # about 10 dB SNR
    p = lt.estimate_pitch(x + noise, SR)
    assert p.hz is not None and lt.note_and_cents(p.hz)[0] == m


def test_silence_and_noise_are_not_voiced():
    p = lt.estimate_pitch(np.zeros(SR // 2), SR)
    assert p.hz is None and p.voiced == 0
    p = lt.estimate_pitch(RNG.normal(0, 0.01, SR // 2), SR)
    assert p.voiced < 0.5


def test_note_names_and_cents():
    assert lt.note_and_cents(440.0) == (69, 0.0)
    n, c = lt.note_and_cents(987.77)
    assert lt.g.midi_name(n) == "B5" and abs(c) < 0.1


# ------------------------------------------------------------------ schedule and verdicts


def rows(text: str):
    return lt.s.parse_lines(text.splitlines())


def test_presses_from_schedule():
    pr = lt.presses_from_rows(rows(
        "0 -\n1000 - - 80:40 # mods\n1500 A - 80:40 # PRESS A ly=0x40 try 1/5\n3000 - - 80:40\n3200 -\n"
        "4700 - down 80:FF\n5200 X down 80:FF # PRESS X D-down+stick-down\n7700 - down 80:FF\n7900 -\n"))
    assert [(p.button, p.t_press, p.t_release) for p in pr] == [("A", 1.5, 3.0), ("X", 5.2, 7.7)]
    assert pr[0].label == "A ly=0x40 try 1/5" and pr[0].stick_sign == 1
    assert (pr[0].base_midi, pr[0].model_midi) == (81, 83)
    assert (pr[1].base_midi, pr[1].model_midi, pr[1].stick_sign) == (85, 83, -1)


def test_presses_match_generated_files():
    for name in ("u6-ly-up.txt", "fingerings-Y.txt", "u6-raw-ly.txt", "calibration-A.txt"):
        _, pr = lt.load_presses(HERE / "samples" / "story-1-3" / name)
        tf = {t.name: t for t in lt.g.build_trials(lt.g.Options())}[name]
        assert len(pr) == len(tf.notes)
        for p, note in zip(pr, tf.notes):
            assert abs(p.t_release - p.t_press - note.hold_ms / 1000) < 1 / 60
    _, pr = lt.load_presses(HERE / "samples" / "story-1-3" / "fingerings-Y.txt")
    # the state model agrees with the generator's label model on all nine
    for p in pr:
        assert f"model {lt.g.midi_name(p.model_midi)})" in p.label


def test_verdicts():
    up = lt.Press(1, 1, 1.5, 3.0, "", "A", 83, 81, 1)
    down = lt.Press(1, 1, 1.5, 3.0, "", "A", 79, 81, -1)
    plain = lt.Press(1, 1, 1.5, 3.0, "", "A", 81, 81, 0)
    assert [lt.verdict_for(up, m) for m in (83, 81, 82, None)] == ["counted", "not counted", "OTHER", "NO SOUND"]
    assert lt.verdict_for(down, 79) == "counted"
    assert [lt.verdict_for(plain, m) for m in (81, 80)] == ["match", "OTHER"]


# ------------------------------------------------------------------ whole recording: alignment + analysis


def synth_recording(presses, offset, latency=0.03, pitch_of=lambda p: p.model_midi, dur_pad=1.0, noise=0.002):
    end = max(p.t_release for p in presses) + offset + dur_pad
    x = RNG.normal(0, noise, int(end * SR))
    for p in presses:
        m = pitch_of(p)
        if m is None:
            continue
        a = int((p.t_press + offset + latency) * SR)
        seg = tone(midi_hz(m), p.t_release - p.t_press, HARMONIC, level=0.2)
        x[a: a + seg.size] += seg
    return x


def test_fingering_file_end_to_end():
    _, pr = lt.load_presses(HERE / "samples" / "story-1-3" / "fingerings-L.txt")
    x = synth_recording(pr, offset=0.734)
    res = lt.analyse(x, SR, pr, 0.734)
    assert len(res) == 9
    assert [r.verdict for r in res] == ["match", "match", "match", "counted", "counted",
                                        "counted", "counted", "counted", "counted"]
    assert [r.note for r in res] == ["D5", "D#5", "C#5", "E5", "C5", "F5", "C#5", "D#5", "B4"]
    for r in res:
        assert 0 <= r.onset_ms <= 60  # synthetic latency 30 ms, 10 ms blocks


def test_u6_not_counted_and_no_sound():
    _, pr = lt.load_presses(HERE / "samples" / "story-1-3" / "u6-ly-up.txt")

    def game(p):  # pretend: 0x40 not counted, 0x20 silent, 0x00 counted
        return {0x40: 81, 0x20: None}.get(int(p.label.split("ly=0x")[1][:2], 16), 83)

    x = synth_recording(pr, offset=0.5, pitch_of=game)
    v = [r.verdict for r in lt.analyse(x, SR, pr, 0.5)]
    assert v == ["not counted"] * 5 + ["NO SOUND"] * 5 + ["counted"] * 5


def test_wrong_offset_is_visible():
    _, pr = lt.load_presses(HERE / "samples" / "story-1-3" / "calibration-A.txt")
    x = synth_recording(pr, offset=0.5)
    assert lt.analyse(x, SR, pr, 0.5)[0].verdict == "match"
    # 2.5 s too early: the window sits in silence
    assert lt.analyse(x, SR, pr, -2.0)[0].verdict == "NO SOUND"


def test_presses_past_the_end_are_skipped():
    _, pr = lt.load_presses(HERE / "samples" / "story-1-3" / "fingerings-A.txt")
    x = synth_recording(pr[:3], offset=0.2, dur_pad=0.3)
    assert len(lt.analyse(x, SR, pr, 0.2)) == 3


def test_recording_start_and_sender_start():
    # chunks of 480 samples (10 ms) arriving with jitter; true start at host time 100.0
    chunks, total = [], 0
    for k in range(200):
        total += 480
        chunks.append((100.0 + total / SR + 0.004 + (0.02 if k % 7 == 0 else 0.0), total))
    assert abs(lt.recording_start(chunks, SR) - 100.004) < 1e-9
    trial = rows("0 - # neutral lead-in\n1000 - - - # mods\n1500 A # PRESS A\n4000 - # release\n")
    ann = lt.s.announcements(trial, 60)
    lines = [(102.0 + f / 60 + d, f"t={f / 60:.1f}s  {label}\n") for (f, label), d in zip(ann, (0.006, 0.002, 0.009, 0.004))]
    lines.insert(0, (101.9, "something else\n"))
    assert abs(lt.sender_start(lines, ann) - 102.002) < 1e-9
    # an out-of-step label stops the matching instead of guessing
    bad = [(102.0, "t=0.0s  neutral lead-in\n"), (102.5, "t=1.0s  WRONG\n"), (102.0 + 1.5, "t=1.5s  PRESS A\n")]
    assert abs(lt.sender_start(bad, ann) - 102.0) < 1e-9
    assert lt.sender_start([], ann) is None


def test_analyse_mode_cli_writes_reports(tmp_path, capsys):
    trial = HERE / "samples" / "story-1-3" / "fingerings-A.txt"
    _, pr = lt.load_presses(trial)
    wav = tmp_path / "rec.wav"
    lt.write_wav(wav, synth_recording(pr, offset=0.4), SR)
    assert lt.main(["--analyse", str(wav), str(trial), "--offset", "0.4"]) == 0
    out = capsys.readouterr().out
    assert "verdict" in out and "counted" in out
    csv_text = (tmp_path / "rec.csv").read_text()
    assert csv_text.splitlines()[0].startswith("n,line,t_press_s") and len(csv_text.splitlines()) == 10
    md = (tmp_path / "rec.md").read_text()
    assert "offset_s: 0.400" in md and md.count("| match |") == 3


def test_cli_errors(tmp_path, capsys):
    trial = HERE / "samples" / "story-1-3" / "calibration-A.txt"
    assert lt.main([str(trial)]) == 1  # neither --port nor --analyse
    assert lt.main(["--analyse", str(tmp_path / "x.wav"), str(trial)]) == 1  # no --offset
    assert lt.main(["--analyse", str(tmp_path / "missing.wav"), str(trial), "--offset", "0"]) == 1
    bad = tmp_path / "bad.txt"
    bad.write_text("0 Q\n")
    assert lt.main(["--analyse", "x.wav", str(bad), "--offset", "0"]) == 1


def test_wav_roundtrip(tmp_path):
    x = tone(880.0, 0.2)
    lt.write_wav(tmp_path / "a.wav", x, SR)
    y, sr = lt.read_wav(tmp_path / "a.wav")
    assert sr == SR and np.max(np.abs(x - y)) < 1e-4


# ------------------------------------------------------------------ process plumbing (fake ffmpeg / fake sender)


FAKE_FFMPEG = """
import sys, time, math, struct
sr = 48000
out = sys.stdout.buffer
for k in range(20):  # 0.2 s of a 880 Hz tone in 10 ms chunks
    block = b"".join(struct.pack("<h", int(8000 * math.sin(2 * math.pi * 880 * (k * 480 + i) / sr))) for i in range(480))
    out.write(block)
    out.flush()
    time.sleep(0.01)
time.sleep(30)  # like a live recording: runs until terminated
"""


def test_recorder_with_fake_ffmpeg(tmp_path):
    script = tmp_path / "fake_ffmpeg.py"
    script.write_text(FAKE_FFMPEG)
    rec = lt.Recorder("0", SR, tmp_path / "r.wav")
    rec.cmd = [sys.executable, str(script)]
    rec.start(timeout=10)
    import time

    deadline = time.monotonic() + 10
    while rec.total < 20 * 480 and time.monotonic() < deadline:
        time.sleep(0.01)
    rec.stop()
    assert rec.proc.poll() is not None  # terminated
    x, sr = lt.read_wav(tmp_path / "r.wav")
    assert sr == SR and x.size == 20 * 480
    assert lt.recording_start(rec.chunks, SR) <= rec.chunks[0][0]


def test_recorder_reports_missing_ffmpeg(tmp_path):
    rec = lt.Recorder("0", SR, tmp_path / "r.wav", ffmpeg=str(tmp_path / "no-such-ffmpeg"))
    with pytest.raises(OSError):
        rec.start()


FAKE_SENDER = """
import sys
print("t=0.0s  neutral lead-in", flush=True)
print("t=1.5s  PRESS A", flush=True)
print("summary: fake", flush=True)
sys.exit(3)
"""


def test_run_sender_timestamps_lines_and_passes_flags(tmp_path, monkeypatch, capsys):
    script = tmp_path / "fake_sender.py"
    script.write_text(FAKE_SENDER)
    monkeypatch.setattr(lt, "SENDER", script)
    code, lines, interrupted = lt.run_sender("/dev/null-port", tmp_path / "t.txt", [])
    assert code == 3 and not interrupted
    assert [t for _, t in lines] == ["t=0.0s  neutral lead-in\n", "t=1.5s  PRESS A\n", "summary: fake\n"]
    assert lines[0][0] <= lines[1][0]
    assert "PRESS A" in capsys.readouterr().out


# ------------------------------------------------------------------ regression: first live calibration (2026-11-20)
#
# What the three real recordings showed (they stay git-ignored; these fixtures copy their shape):
# - the game's A5 read about 894 Hz (+28 cents), clean, about -40 dBFS, while the room/console noise before
#   it was mostly below 200 Hz (broadband -57 dBFS, so SNR on the raw signal was only 8 dB);
# - the capture lost about 9 % of the samples in 1024-sample packets, so the WAV ran short of real time:
#   the note sat about 0.1 s earlier than the windows assumed and the "noise before the press" window
#   already contained the note; with SNR < 10 dB every note became NO SOUND.


def capture_with_drops(x: np.ndarray, keep_every: int = 11, packet: int = 1024) -> np.ndarray:
    """Drop one 1024-sample packet in every `keep_every` (about 9 %), like the live capture did."""
    parts = [x[k: k + packet] for i, k in enumerate(range(0, x.size, packet)) if i % keep_every != keep_every - 1]
    return np.concatenate(parts)


def rumble(n: int, level: float = 0.003) -> np.ndarray:
    """Low-frequency room/console noise (below 200 Hz) plus a little hiss."""
    w = RNG.normal(0, 1, n)
    f = np.fft.rfftfreq(n, 1 / SR)
    lowpassed = np.fft.irfft(np.fft.rfft(w) * (f < 200), n=n)
    return level * lowpassed / np.std(lowpassed) + RNG.normal(0, level / 30, n)


def live_like(note: bool, wall_offset=0.363, latency=0.09, dur=5.7) -> np.ndarray:
    x = rumble(int(dur * SR))
    if note:
        a = int((wall_offset + 1.5 + latency) * SR)
        x[a: a + int(2.5 * SR)] += tone(894.0, 2.5, (1.0, 0.15, 0.05), level=0.014)
    return capture_with_drops(x)


CAL = HERE / "samples" / "story-1-3" / "calibration-A.txt"
SCALE = 10 / 11


def test_live_like_note_detected_with_scale():
    _, pr = lt.load_presses(CAL)
    x = live_like(True)
    r = lt.analyse(x, SR, pr, 0.363 * SCALE, lt.Params(scale=SCALE))[0]
    assert (r.note, r.verdict) == ("A5", "match") and abs(r.cents - 28) < 5
    assert r.level_dbfs - r.noise_dbfs > 20
    assert 40 < r.onset_ms < 140  # the synthetic 90 ms latency, in schedule ms


def test_live_like_note_detected_even_without_scale():
    # old alignment (no scale): the note starts before the window assumes; the file's quiet floor
    # keeps the contaminated pre-press noise window from hiding it
    _, pr = lt.load_presses(CAL)
    r = lt.analyse(live_like(True), SR, pr, 0.363)[0]
    assert (r.note, r.verdict) == ("A5", "match")


def test_live_like_silence_stays_no_sound():
    _, pr = lt.load_presses(CAL)
    for scale in (1.0, SCALE):
        r = lt.analyse(live_like(False), SR, pr, 0.363 * scale, lt.Params(scale=scale))[0]
        assert r.verdict == "NO SOUND"


def test_raw_snr_would_have_failed():
    # documents the root cause: on the unfiltered signal the noise before the note is within 10 dB
    x = live_like(True)
    t_note = (0.363 + 1.5 + 0.09) * SCALE
    note = lt.dbfs(x[int((t_note + 0.3) * SR): int((t_note + 2.0) * SR)])
    pre = lt.dbfs(x[int((t_note - 0.5) * SR): int((t_note - 0.1) * SR)])
    assert note - pre < 15
    xb = lt.bandpass(x, SR, 250, 4000)
    assert lt.dbfs(xb[int((t_note + 0.3) * SR): int((t_note + 2.0) * SR)]) - \
        lt.dbfs(xb[int((t_note - 0.5) * SR): int((t_note - 0.1) * SR)]) > 25


def test_bandpass_keeps_ocarina_band_only():
    t = np.arange(SR) / SR
    low, mid = np.sin(2 * np.pi * 100 * t), np.sin(2 * np.pi * 880 * t)
    y = lt.bandpass(low + mid, SR, 250, 4000)
    assert np.max(np.abs(y[1000:-1000] - mid[1000:-1000])) < 0.02


def test_alignment_measures_lost_samples():
    # 10 ms of audio arrives every 10 ms of host time, but only 10 of 11 packets survive
    chunks, total = [], 0
    for k in range(1, 600):
        if k % 11:
            total += 480
        chunks.append((50.0 + k * 0.01 + 0.003, total))
    offset, scale = lt.alignment(chunks, SR, t_send=50.4)
    assert abs(scale - 10 / 11) < 0.01
    # schedule t=0 happened 0.4 s after the first sample (host time), i.e. 0.4 * scale into the WAV
    assert abs(offset - 0.4 * scale) < 0.02
    # a clean capture gives scale 1 and the plain offset
    clean = [(50.0 + k * 0.01 + 0.003, k * 480) for k in range(1, 600)]
    offset, scale = lt.alignment(clean, SR, t_send=50.4)
    assert abs(scale - 1) < 1e-6 and abs(offset - 0.4) < 0.005


def test_alignment_short_capture_falls_back_to_nominal_rate():
    chunks = [(10.0 + k * 0.01, k * 480) for k in range(1, 20)]
    offset, scale = lt.alignment(chunks, SR, t_send=10.1)
    assert scale == 1.0 and abs(offset - 0.1) < 0.005


def test_ffmpeg_command_has_large_thread_queue():
    cmd = lt.Recorder("MacBook Pro Microphone", SR, pathlib.Path("unused.wav")).cmd
    i = cmd.index("-thread_queue_size")
    assert int(cmd[i + 1]) >= 1024 and i < cmd.index("-f") and cmd[cmd.index("-i") + 1] == ":MacBook Pro Microphone"


def test_analyse_cli_scale_option(tmp_path, capsys):
    wav = tmp_path / "live.wav"
    lt.write_wav(wav, live_like(True), SR)
    assert lt.main(["--analyse", str(wav), str(CAL), "--offset", f"{0.363 * SCALE:.3f}",
                    "--scale", f"{SCALE:.4f}"]) == 0
    assert "match" in capsys.readouterr().out
    assert "scale: 0.9091" in (tmp_path / "live.md").read_text()


# ================================================================== Story 1.4: timing analysers


S14 = HERE / "samples" / "story-1-4"
F = 1 / 60  # one sender frame, s
G4 = lt._load("gen_trials_1_4")


def attempts(name: str):
    rows_, _ = lt.load_presses(S14 / name)
    return lt.attempts_from_rows(rows_)


def note_wave(f: float, dur: float, decay: float = 0.0, level: float = 0.2, attack: float = 0.003) -> np.ndarray:
    """A harmonic note of `dur` s at full level, then an exponential decay (time constant `decay` s)."""
    tail = 6 * decay
    t = np.arange(int((dur + tail) * SR)) / SR
    x = sum(a * np.sin(2 * np.pi * f * (k + 1) * t) for k, a in enumerate((1.0, 0.15, 0.05)))
    env = np.minimum(1.0, t / attack) if attack > 0 else np.ones_like(t)
    if decay > 0:
        env = np.where(t > dur, env * np.exp(-(t - dur) / decay), env)
    else:
        env = np.where(t > dur, 0.0, env)
    return level * env * x / 1.2


def place(x: np.ndarray, seg: np.ndarray, t: float) -> None:
    a = int(t * SR)
    b = min(x.size, a + seg.size)
    if b > a:
        x[a:b] += seg[: b - a]


def background(dur: float) -> np.ndarray:
    return RNG.normal(0, 0.001, int(dur * SR))


A5 = 894.0  # the game's A5 (+28 cents)


def test_attempts_from_generated_files():
    u7 = attempts("u7-minpress-part1.txt")
    assert len(u7) == 55 and sum(a.blank for a in u7) == 5
    assert {a.step for a in u7} == {"blank", *(f"{d}f" for d in range(1, 11))}
    assert all(a.kind == "u7" and a.button == "A" and a.model_midi == 81 for a in u7)
    assert all(a.round in range(1, 6) and a.t_end > a.t for a in u7)
    u2 = attempts("u2-gap-part2.txt")
    assert sum(a.control for a in u2) == 10 and {a.fields["hold"] for a in u2} == {12}
    assert {a.round for a in u2} == set(range(6, 11))
    u5 = attempts("u5-lead-sdown-part1.txt")
    assert {a.family for a in u5} == {"u5 sdown"} and {a.model_midi for a in u5} == {79}
    assert all(a.fields["note"] == 36 for a in u5)


def u7_recording(atts, game, offset=0.4, latency=0.08):
    x = background(atts[-1].t_end + offset + 1.5)
    for a in atts:
        f = game(a)
        if f is not None:
            place(x, note_wave(f, a.fields["press"] * F, decay=0.05), offset + latency + a.t)
    return x


def test_detect_short_bursts_and_blanks():
    atts = attempts("u7-minpress-part1.txt")
    x = u7_recording(atts, lambda a: None if a.blank else A5)
    res, _ = lt.analyse_attempts(x, SR, atts, 0.4, "detect")
    assert len(res) == 55
    for r in res:
        a = r.attempt
        if a.blank:
            assert (r.sound, r.verdict, r.passed) == (False, "silent (ok)", None)
            continue
        assert r.verdict == "PASS" and r.passed
        assert 70 <= r.onset_ms <= 95  # synthetic latency 80 ms
        # audible length: the press plus the decay down to -20 dB (50 ms x ln 10 = 115 ms)
        assert abs(r.audible_ms - (a.fields["press"] * 1000 * F + 115)) < 15
        assert r.hz is not None and lt.note_and_cents(r.hz)[0] == 81


def test_detect_misses_wrong_pitch_and_threshold():
    atts = attempts("u7-minpress-part1.txt")

    def game(a):  # pretend: 1 and 2-frame presses are ignored, a 5-frame press plays the wrong note
        if a.blank or a.fields["press"] <= 2:
            return None
        return 700.0 if a.fields["press"] == 5 and a.round == 3 else A5

    res, _ = lt.analyse_attempts(u7_recording(atts, game), SR, atts, 0.4, "detect")
    rows_ = [lt.attempt_row(r) for r in res]
    stats = {st.step: st for st in lt.step_table(rows_)}
    assert (stats["1f"].passes, stats["2f"].passes, stats["3f"].passes) == (0, 0, 5)
    assert stats["5f"].passes == 4 and stats["5f"].fail_rounds == (3,)
    assert [r.verdict for r in res if r.attempt.fields.get("press") == 5 and r.attempt.round == 3] == ["FAIL pitch"]
    assert lt.smallest_passing(list(stats.values())) == {"u7": "6f = 100.0 ms"}
    assert "INVALID" not in "\n".join(lt.attempt_summary_lines(rows_, {}))


def test_detect_blank_that_sounds_invalidates():
    atts = attempts("u7-minpress-part1.txt")
    x = u7_recording([a for a in atts if not a.blank], lambda a: A5)
    blank = next(a for a in atts if a.blank)
    place(x, note_wave(A5, 0.1, 0.05), 0.4 + 0.08 + blank.t)
    rows_ = [lt.attempt_row(r) for r in lt.analyse_attempts(x, SR, atts, 0.4, "detect")[0]]
    assert sum(r["verdict"] == "BLANK SOUNDED" for r in rows_) == 1
    assert any(line.startswith("INVALID") for line in lt.attempt_summary_lines(rows_, {}))


def test_retrigger_one_frame_dip_threshold():
    a = lt.Attempt(1, 1, 0.5, 3.0, "u2", "A", {"gap": 1, "hold": 12}, 1, False, False, 81, "TRY u2 A gap=1f hold=12f")
    for depth, want in ((3.0, 1), (12.0, 2)):
        x = background(3.0)
        n1 = note_wave(A5, 0.2)
        place(x, n1, 0.58)
        dip = note_wave(A5, F) * 10 ** (-depth / 20)
        place(x, dip, 0.78)
        place(x, note_wave(A5, 0.2), 0.78 + F)
        res, meta = lt.retrigger_attempts(x, SR, [a], 0.0, lt.Params(), lt.TimingParams(dip_db=6.0, rise_db=6.0))
        assert res[0].onsets == want and abs(res[0].dip_db - depth) < 2.5
        assert meta["threshold_source"] == "options"


def u2_recording(atts, offset=0.3, latency=0.08):
    """Pretend game: a release shorter than 3 frames is not seen (one long note); otherwise the note
    stops (30 ms decay) and restarts on the second press."""
    x = background(atts[-1].t_end + offset + 2.0)
    for a in atts:
        gap, hold, t0 = a.fields["gap"], a.fields["hold"] * F, offset + latency + a.t
        if gap < 3:
            place(x, note_wave(A5, 2 * hold + gap * F, decay=0.03), t0)
        else:
            place(x, note_wave(A5, hold, decay=0.03), t0)
            place(x, note_wave(A5, hold, decay=0.03), t0 + hold + gap * F)
    return x


def test_retrigger_thresholds_from_controls():
    atts = attempts("u2-gap-part1.txt")
    res, meta = lt.analyse_attempts(u2_recording(atts), SR, atts, 0.3, "retrigger")
    assert meta["threshold_source"] == "controls" and meta["controls_separated"] == "True"
    assert max(map(float, meta["controls_g0_valley_top3_db"].split(", "))) < 3
    assert min(map(float, meta["controls_g30_valley_low3_db"].split(", "))) > 20
    for r in res:
        gap = r.attempt.fields["gap"]
        assert r.onsets == (1 if gap < 3 else 2), (gap, r.dip_db, meta)
        if r.attempt.control:
            assert r.verdict == "control ok" and r.passed is None
    rows_ = [lt.attempt_row(r) for r in res]
    assert lt.smallest_passing(lt.step_table(rows_)) == {"u2": "3f = 50.0 ms"}
    # the dip depth per gap grows with the gap (Story 1.5 picks the margin from it)
    depth = {g_: np.median([r.dip_db for r in res if r.attempt.fields["gap"] == g_]) for g_ in (3, 5, 8)}
    assert depth[3] < depth[5] < depth[8]


def test_retrigger_without_controls_reports_no_threshold():
    atts = [a for a in attempts("u2-gap-part1.txt") if not a.control]
    res, meta = lt.analyse_attempts(u2_recording(atts), SR, atts, 0.3, "retrigger")
    assert {r.verdict for r in res} == {"NO THRESHOLD"} and meta["dip_db"] == "-"
    assert any(line.startswith("INVALID") for line in lt.attempt_summary_lines([lt.attempt_row(r) for r in res], {}))


def lead_note(f: float, off_cents: float, off_dur: float, dur: float) -> np.ndarray:
    """A note whose first `off_dur` s sound `off_cents` away from `f` (phase-continuous)."""
    t = np.arange(int((dur + 0.05) * SR)) / SR
    inst = np.where(t < off_dur, f * 2 ** (off_cents / 1200), f)
    ph = 2 * np.pi * np.cumsum(inst) / SR
    x = sum(a * np.sin((k + 1) * ph) for k, a in enumerate((1.0, 0.15, 0.05)))
    env = np.minimum(1.0, t / 0.003) * np.where(t > dur, np.exp(-(t - dur) / 0.01), 1.0)
    return 0.2 * env * x / 1.2


@pytest.mark.parametrize("off,ok", [(0, True), (20, True), (60, False), (-100, False), (100, False)])
def test_lead_pitch_at_note_start(off, ok):
    a = lt.Attempt(1, 1, 0.5, 2.5, "u5", "A", {"lead": 0, "note": 36}, 1, False, False, 82, "TRY u5 A mod=dup")
    x = background(2.5)
    place(x, lead_note(A5 * 2 ** (1 / 12), off, 0.06, 0.6), 0.58)
    r = lt.lead_attempt(lt.bandpass(x, SR, 250, 4000), SR, a, 0.0, lt.Params(), lt.TimingParams(), -60.0)
    assert r.passed is ok and r.sound
    assert abs(r.start_cents - off) < 15
    assert abs(lt.cents(r.hz, A5 * 2 ** (1 / 12))) < 3


def test_lead_file_end_to_end_and_threshold():
    atts = attempts("u5-lead-dup-part1.txt")
    x = background(atts[-1].t_end + 2.5)
    f = A5 * 2 ** (1 / 12)
    for a in atts:  # pretend: a lead under 2 frames starts a semitone low (the D-pad not seen yet) for 50 ms
        place(x, lead_note(f, -100 if a.fields["lead"] < 2 else 0, 0.05, 36 * F), 0.3 + 0.08 + a.t)
    res, meta = lt.analyse_attempts(x, SR, atts, 0.3, "lead")
    assert meta == {"lead_cents": "50", "band_snr_db": "8"}
    rows_ = [lt.attempt_row(r) for r in res]
    assert lt.smallest_passing(lt.step_table(rows_)) == {"u5 dup": "2f = 33.3 ms"}


def test_lead_no_sound_fails():
    atts = attempts("u5-lead-sup-part1.txt")[:3]
    res, _ = lt.analyse_attempts(background(atts[-1].t_end + 2), SR, atts, 0.3, "lead")
    assert {r.verdict for r in res} == {"FAIL no sound"}


# ------------------------------------------------------------------ jitter


def jitter_atts(tmp_path):
    (tf,) = G4.build(G4.Options(), "jitter")
    path = tmp_path / tf.name
    path.write_text(G4.render(tf))
    rows_, _ = lt.load_presses(path)
    return lt.attempts_from_rows(rows_)


def jitter_recording(atts, true_on, gaps: bool):
    x = background(true_on[-1] + 1.0)
    for i, a in enumerate(atts):
        f = lt.g.midi_hz(a.model_midi) * 2 ** (28 / 1200)
        nxt = true_on[i + 1] if i + 1 < len(atts) else true_on[i] + 0.125
        dur = a.fields["press"] * F if gaps else nxt - true_on[i]
        place(x, note_wave(f, dur, decay=0.01 if gaps else 0.0, level=0.15), true_on[i])
    return x


@pytest.mark.parametrize("gaps", [False, True], ids=["legato", "released"])
def test_jitter_recovers_known_sigma(tmp_path, gaps):
    atts = jitter_atts(tmp_path)
    assert len(atts) == 240
    sched = np.array([a.t for a in atts])
    rng = np.random.default_rng(7)
    true_on = 0.6 + 0.08 + sched + rng.normal(0, 0.003, sched.size)  # 3 ms onset jitter
    want = np.std(np.diff(true_on) - np.diff(sched), ddof=1) * 1000
    j = lt.analyse_jitter(jitter_recording(atts, true_on, gaps), SR, atts, 0.6)
    assert j.matched == 240
    assert abs(j.stats()["sigma_ms"] - want) < 1.0
    assert abs(j.latency_ms - 80) < 15
    assert abs(j.quant_sigma_ms - 1000 / 120) < 0.1  # the 7/8 pattern: +-half a frame
    assert any("not our scheduler" in line for line in lt.jitter_lines(j))


def test_jitter_shift_and_scale_do_not_change_sigma(tmp_path):
    atts = jitter_atts(tmp_path)
    sched = np.array([a.t for a in atts])
    rng = np.random.default_rng(11)
    noise = rng.normal(0, 0.002, sched.size)
    want = np.std(np.diff(noise), ddof=1) * 1000
    base = lt.analyse_jitter(jitter_recording(atts, 0.5 + 0.08 + sched + noise, False), SR, atts, 0.5)
    shifted = lt.analyse_jitter(jitter_recording(atts, 0.5 + 0.21 + sched + noise, False), SR, atts, 0.5)  # constant extra latency
    scaled = lt.analyse_jitter(jitter_recording(atts, (0.5 + 0.08 + sched + noise) * 1.005, False), SR, atts, 0.5)  # capture clock 0.5 % off, not corrected
    for j in (base, shifted, scaled):
        assert j.matched == 240 and abs(j.stats()["sigma_ms"] - want) < 1.0
    assert abs(shifted.latency_ms - base.latency_ms - 130) < 3
    assert abs(scaled.drift_ms_per_s - 5.0) < 0.5  # the scale error shows as drift, not as sigma


def test_jitter_mode_cli(tmp_path, capsys):
    atts = jitter_atts(tmp_path)
    sched = np.array([a.t for a in atts])
    wav = tmp_path / "jit.wav"
    lt.write_wav(wav, jitter_recording(atts, 0.4 + 0.08 + sched, False), SR)
    assert lt.main(["--analyse", str(wav), str(tmp_path / "jitter-120bpm-16ths.txt"), "--offset", "0.4",
                    "--mode", "jitter"]) == 0
    out = capsys.readouterr().out
    assert "matched 240" in out and "IOI error" in out and "quantisation" in out
    assert "sigma_ms" in (tmp_path / "jit.md").read_text()


# ------------------------------------------------------------------ CLI, reports, combine


def test_detect_mode_cli_and_combine(tmp_path, capsys):
    paths = []
    for part in ("u7-minpress-part1.txt", "u7-minpress-part2.txt"):
        atts = attempts(part)
        wav = tmp_path / f"{pathlib.Path(part).stem}.wav"
        lt.write_wav(wav, u7_recording(atts, lambda a: None if a.blank or a.fields["press"] < 4 else A5), SR)
        assert lt.main(["--analyse", str(wav), str(S14 / part), "--offset", "0.4", "--mode", "detect"]) == 0
        paths.append(str(wav.with_suffix(".csv")))
    out = capsys.readouterr().out
    assert "u7" in out and " 0/5" in out and "smallest step" in out
    md = (tmp_path / "u7-minpress-part1.md").read_text()
    assert "mode: detect" in md and "min_snr_db: 10" in md
    assert lt.main(["--combine", *paths]) == 0
    out = capsys.readouterr().out
    assert "3f" in out and " 0/10" in out and "10/10" in out and "u7: 4f = 66.7 ms" in out


def test_timing_mode_needs_try_rows(capsys):
    assert lt.main(["--analyse", "x.wav", str(CAL), "--offset", "0", "--mode", "detect"]) == 1
    assert "TRY rows" in capsys.readouterr().err


def test_combine_rejects_pitch_reports(tmp_path, capsys):
    bad = tmp_path / "x.csv"
    bad.write_text("n,line\n1,2\n")
    assert lt.main(["--combine", str(bad)]) == 1
    assert lt.main(["--combine", str(tmp_path / "missing.csv")]) == 1


# ------------------------------------------------------------------ capture self-test and the sounddevice backend


def steady_tone(dur: float, f: float = 1000.0, noise: float = 0.002) -> np.ndarray:
    t = np.arange(int(dur * SR)) / SR
    return 0.2 * np.sin(2 * np.pi * f * t + 0.3) + RNG.normal(0, noise, t.size)


def drop(x: np.ndarray, at: tuple, n: int = 1024) -> np.ndarray:
    keep = np.ones(x.size, bool)
    for a in at:
        keep[int(a * SR): int(a * SR) + n] = False
    return x[keep]


def test_tone_selftest_counts_discontinuities():
    assert lt.tone_discontinuities(steady_tone(10.0), SR).count == 0
    assert lt.tone_discontinuities(steady_tone(10.0, 1000.4), SR).count == 0  # other device slightly off 1 kHz
    tc = lt.tone_discontinuities(drop(steady_tone(10.0), (2.0, 6.5)), SR)
    assert tc.count == 2 and abs(tc.amp - 0.2) < 0.01
    assert lt.tone_discontinuities(drop(steady_tone(10.0), (1.0, 3.0, 5.0, 7.0, 9.0), n=500), SR).count == 5


def with_startup(x: np.ndarray, lead: float = 0.15, tail: float = 0.1) -> np.ndarray:
    """Like the live capture (2026-11-21): the input starts with near-silence (-75 to -134 dBFS) before the
    tone arrives, and the tone may stop before the recording does."""
    def quiet(d: float) -> np.ndarray:
        return RNG.normal(0, 1e-5, int(d * SR))

    return np.concatenate([np.zeros(int(0.01 * SR)), quiet(lead - 0.01), x, quiet(tail)])


def test_tone_selftest_ignores_startup_and_tail_silence():
    tc = lt.tone_discontinuities(with_startup(steady_tone(10.0)), SR)
    assert tc.count == 0 and tc.dropouts == 0
    assert abs(tc.lead_s - lt.STARTUP_GRACE_S) < 0.011 and abs(tc.tail_s - 0.1) < 0.011
    # leading silence longer than the grace window is skipped too, not a dropout
    tc = lt.tone_discontinuities(with_startup(steady_tone(10.0), lead=0.8), SR)
    assert tc.count == 0 and tc.dropouts == 0 and abs(tc.lead_s - 0.8) < 0.011
    # a real packet drop in the tone is still found
    assert lt.tone_discontinuities(with_startup(drop(steady_tone(10.0), (4.0,))), SR).count == 1
    chunks = [(100.0 + k * 0.01, k * 480) for k in range(1, 1000)]
    lines, ok = lt.selftest_lines(with_startup(steady_tone(10.0)), SR, chunks, 1000.0, "sounddevice", 0)
    assert ok and "first 0.5 s" in lines[2]


def test_tone_selftest_macos_startup_burst():
    # live shape (2026-11-21): 10 ms of tone, then about 150 ms near-silence while the input starts, then steady
    x = steady_tone(10.0)
    x[int(0.010 * SR): int(0.160 * SR)] = RNG.normal(0, 1e-5, int(0.150 * SR))
    tc = lt.tone_discontinuities(x, SR)
    assert tc.count == 0 and tc.dropouts == 0
    chunks = [(100.0 + k * 0.01, k * 480) for k in range(1, 1000)]
    assert lt.selftest_lines(x, SR, chunks, 1000.0, "sounddevice", 0)[1]


def test_tone_selftest_reports_mid_recording_dropout():
    x = steady_tone(10.0)
    x[int(5.0 * SR): int(5.2 * SR)] = RNG.normal(0, 1e-5, int(0.2 * SR))  # the tone stops for 0.2 s
    tc = lt.tone_discontinuities(x, SR)
    assert tc.dropouts == 1 and tc.count == 0
    chunks = [(100.0 + k * 0.01, k * 480) for k in range(1, 1000)]
    lines, ok = lt.selftest_lines(x, SR, chunks, 1000.0, "sounddevice", 0)
    assert not ok and any("dropped out" in line for line in lines)


def test_selftest_verdict():
    chunks = [(100.0 + k * 0.01, k * 480) for k in range(1, 1000)]
    lines, ok = lt.selftest_lines(steady_tone(10.0), SR, chunks, 1000.0, "sounddevice", 0)
    assert ok and lines[-1].startswith("self-test (sounddevice): PASS")
    lines, ok = lt.selftest_lines(drop(steady_tone(10.0), (2.0, 6.5)), SR, chunks, 1000.0, "sounddevice", 0)
    assert not ok and "discontinuities (> 0.2 ms): 2" in lines[0]
    lossy = [(100.0 + k * 0.01, k * 480 * 10 // 11) for k in range(1, 1000)]
    assert not lt.selftest_lines(steady_tone(10.0), SR, lossy, 1000.0, "ffmpeg", None)[1]
    assert not lt.selftest_lines(steady_tone(10.0), SR, chunks, 1000.0, "sounddevice", 1)[1]  # overflow
    lines, ok = lt.selftest_lines(RNG.normal(0, 0.001, 10 * SR), SR, chunks, 1000.0, "sounddevice", 0)
    assert not ok and any("no tone" in line for line in lines)


class FakeFlags:
    def __init__(self, overflow: bool):
        self.input_overflow = overflow

    def __bool__(self):
        return self.input_overflow

    def __str__(self):
        return "input overflow" if self.input_overflow else ""


class FakeSd:
    """Stands in for the sounddevice module: a stream whose start() delivers 20 blocks of 480 samples."""

    def __init__(self, overflow_at=None):
        self.overflow_at = overflow_at
        self.opened = {}

    def RawInputStream(self, **kw):  # mirrors the sounddevice API name
        self.opened = kw
        fake = self

        class Stream:
            def start(self):
                for k in range(20):
                    block = (np.sin(2 * np.pi * 880 * (k * 480 + np.arange(480)) / SR) * 8000).astype("<i2")
                    kw["callback"](memoryview(block.tobytes()), 480, None, FakeFlags(k == fake.overflow_at))

            def stop(self):
                pass

            def close(self):
                pass

        return Stream()


@pytest.mark.parametrize("overflow_at", [None, 7])
def test_sounddevice_recorder_with_fake_module(tmp_path, overflow_at):
    sd = FakeSd(overflow_at)
    rec = lt.SdRecorder("MacBook Pro Microphone", SR, tmp_path / "r.wav", sd=sd)
    rec.start(timeout=5)
    err = rec.stop()
    assert sd.opened["dtype"] == "int16" and sd.opened["channels"] == 1 and sd.opened["samplerate"] == SR
    assert sd.opened["device"] == "MacBook Pro Microphone"
    x, sr = lt.read_wav(tmp_path / "r.wav")
    assert sr == SR and x.size == 20 * 480 == rec.total and len(rec.chunks) == 20
    if overflow_at is None:
        assert rec.overflows == 0 and err == ""
    else:
        assert rec.overflows == 1 and "1 input overflow" in err


def test_sounddevice_device_index_and_factory(tmp_path):
    assert lt.SdRecorder("2", SR, tmp_path / "a.wav").device == 2
    assert isinstance(lt.make_recorder("sounddevice", "0", SR, tmp_path / "a.wav"), lt.SdRecorder)
    assert isinstance(lt.make_recorder("ffmpeg", "0", SR, tmp_path / "a.wav"), lt.Recorder)


# ------------------------------------------------------------------ live-recording traps (2026-11-21 field trials)


def loud_background(dur: float, level: float = 0.01) -> np.ndarray:
    """Broadband room/console noise strong enough that a quiet X note is only ~10 dB above it (live shape)."""
    return RNG.normal(0, level, int(dur * SR))


def test_detect_low_snr_x_notes_and_blanks():
    # X (D6) at about 10 dB above the broadband noise: the old peak-vs-RMS detector failed most of these
    atts = attempts("confirm-u7-X-2f.txt")
    f = lt.expected_hz(86)
    x = loud_background(atts[-1].t_end + 2.0)
    for a in atts:
        if not a.blank:
            place(x, note_wave(f, 2 * F, decay=0.08, level=0.012, attack=0.003), 0.4 + 0.13 + a.t)
    res, meta = lt.analyse_attempts(x, SR, atts, 0.4, "detect")
    assert meta["band_snr_db"] == "8"
    assert all(r.verdict == "silent (ok)" for r in res if r.attempt.blank)
    scored = [r for r in res if not r.attempt.blank]
    assert len(scored) == 20 and all(r.passed for r in scored)
    assert all(abs(lt.cents(r.hz, f)) < 15 for r in scored)  # no f/2 (D5) or f/3 (G4) readings


def harmonic_note(f: float, dur: float, amps=(0.5, 1.0, 0.7), level: float = 0.1) -> np.ndarray:
    t = np.arange(int(dur * SR)) / SR
    x = sum(a * np.sin(2 * np.pi * f * (k + 1) * t + 0.4 * k) for k, a in enumerate(amps))
    return level * np.minimum(1.0, t / 0.003) * x / sum(amps)


@pytest.mark.parametrize("off,ok", [(0, True), (-100, False)])
def test_lead_no_octave_error_with_strong_harmonics_and_noise(off, ok):
    # second harmonic louder than the fundamental plus noise: YIN on 30 ms windows read 2f or f/2 live
    f = lt.expected_hz(85)  # X + D-down, C#6
    a = lt.Attempt(1, 1, 0.5, 2.5, "u5", "X", {"lead": 0, "note": 36}, 1, False, False, 85, "TRY u5 X mod=ddown")
    x = loud_background(2.5, 0.004)
    note = harmonic_note(f, 0.6)
    if off:
        head = harmonic_note(f * 2 ** (off / 1200), 0.05)
        note[: head.size] = head
    place(x, note, 0.63)
    r = lt.lead_attempt(lt.bandpass(x, SR, 250, 4000), SR, a, 0.0, lt.Params(), lt.TimingParams())
    assert r.sound and r.passed is ok
    assert abs(lt.cents(r.hz, f)) < 10  # the steady pitch is the fundamental, not 2f
    assert abs(r.start_cents - off) < 30


def _u2_row(label: str, step: str, dip: float, control: bool) -> dict[str, str]:
    row = dict.fromkeys(lt.ATTEMPT_COLUMNS, "-")
    row.update({"label": label, "family": "u2", "step": step, "round": "1", "control": "1" if control else "0",
                "dip_db": f"{dip:.1f}", "onsets": "1", "verdict": "", "pass": "" if control else "0"})
    return row


def test_pooled_rescore_reports_outlier_held_note():
    rows = [_u2_row("g0 a", "0f", 2.0, True), _u2_row("g0 b", "0f", 3.0, True),
            _u2_row("g0 OUTLIER", "0f", 12.0, True), _u2_row("g30 a", "30f", 25.0, True),
            _u2_row("g30 b", "30f", 27.0, True)]
    rows += [_u2_row(f"gap4 {k}", "4f", d, False) for k, d in enumerate((14.0, 19.0, 20.0))]
    rows.append({**_u2_row("silent", "4f", 0.0, False), "onsets": "0"})
    out, lines = lt.pooled_rescore(rows)
    assert "threshold 18.0 dB" in lines[0] and "separated True" in lines[0]
    assert lines[1].startswith("deepest held-note dips") and "12.0 dB g0 OUTLIER" in lines[1]
    assert [r["pass"] for r in out[5:]] == ["0", "1", "1", "0"]
    assert all(r["verdict"] == "control ok" for r in out[:5])
    assert lt.pooled_rescore([_u2_row("x", "4f", 3.0, False)])[1][0].startswith("pooled controls: none")


def test_combine_pool_controls_cli(tmp_path, capsys):
    paths = []
    for part in ("u2-gap-part1.txt", "u2-gap-part2.txt"):
        atts = attempts(part)
        wav = tmp_path / f"{pathlib.Path(part).stem}.wav"
        lt.write_wav(wav, u2_recording(atts), SR)
        assert lt.main(["--analyse", str(wav), str(S14 / part), "--offset", "0.3", "--mode", "retrigger",
                        "--report-tag", "rescored"]) == 0
        paths.append(str(tmp_path / f"{pathlib.Path(part).stem}-rescored.csv"))
    capsys.readouterr()
    assert lt.main(["--combine", *paths, "--pool-controls"]) == 0
    out = capsys.readouterr().out
    assert "pooled controls: 10 held (gap 0), 10 retrigger (gap 30)" in out and "u2: 3f = 50.0 ms" in out


def test_jitter_close_pitches_and_quiet_x(tmp_path):
    # A5 and B5 are 110 Hz apart and X is much quieter than the others (live shape): every note still found
    atts = jitter_atts(tmp_path)
    sched = np.array([a.t for a in atts])
    true_on = 0.5 + 0.13 + sched + np.random.default_rng(3).normal(0, 0.003, sched.size)
    x = loud_background(true_on[-1] + 1.0, 0.003)
    for i, a in enumerate(atts):
        nxt = true_on[i + 1] if i + 1 < len(atts) else true_on[i] + 0.125
        lvl = 0.03 if a.button == "X" else 0.15
        place(x, note_wave(lt.expected_hz(a.model_midi), nxt - true_on[i], level=lvl), true_on[i])
    want = np.std(np.diff(true_on) - np.diff(sched), ddof=1) * 1000
    j = lt.analyse_jitter(x, SR, atts, 0.5)
    assert j.matched >= 238 and abs(j.stats()["sigma_ms"] - want) < 1.0
    assert abs(j.latency_ms - 130) < 15 and abs(j.drift_ms_per_s) < 0.2
    assert any(line.startswith("per-note latency minus median") for line in lt.jitter_lines(j))


def test_jitter_frame_quantised_latency_shows_two_clusters(tmp_path):
    # the live runs showed per-note latency in two clusters about one frame apart
    atts = jitter_atts(tmp_path)
    sched = np.array([a.t for a in atts])
    flip = np.random.default_rng(5).random(sched.size) < 0.4
    true_on = 0.5 + 0.13 + sched + np.where(flip, F, 0.0)
    j = lt.analyse_jitter(jitter_recording(atts, true_on, False), SR, atts, 0.5)
    counts = dict(j.lag_hist)
    big = sorted(counts, key=counts.get)[-2:]
    assert j.matched == 240 and 12 <= abs(big[0] - big[1]) <= 20


@pytest.mark.parametrize("off", [-200, -100, 100])
def test_lead_resolution_about_two_frames(off):
    # the verdict is the median of 9 overlapping 30 ms windows: a wrong start of about 2 frames is not
    # resolvable, a longer one fails; a missing stick modifier (-200 c) must not hide before the onset
    a = lt.Attempt(1, 1, 0.5, 2.5, "u5", "A", {"lead": 0, "note": 36}, 1, False, False, 82, "TRY u5 A")
    f = lt.expected_hz(82)

    def verdict(ms):
        x = background(2.5)
        place(x, lead_note(f, off, ms / 1000, 0.6), 0.58)
        return lt.lead_attempt(lt.bandpass(x, SR, 250, 4000), SR, a, 0.0, lt.Params(), lt.TimingParams()).passed

    assert verdict(30) is True       # under 2 frames: not seen
    assert verdict(33) is True
    assert verdict(36) is False      # the measured limit: about 35 ms (2 frames)
    assert verdict(45) is False


def test_combine_rejects_two_reports_of_one_recording(tmp_path, capsys):
    atts = attempts("u7-minpress-part1.txt")
    wav = tmp_path / "u7-minpress-part1-20261121-112505.wav"
    lt.write_wav(wav, u7_recording(atts, lambda a: None if a.blank else A5), SR)
    trial = str(S14 / "u7-minpress-part1.txt")
    assert lt.main(["--analyse", str(wav), trial, "--offset", "0.4", "--mode", "detect"]) == 0
    assert lt.main(["--analyse", str(wav), trial, "--offset", "0.4", "--mode", "detect", "--report-tag", "rescored"]) == 0
    live, tagged = str(wav.with_suffix(".csv")), str(tmp_path / "u7-minpress-part1-20261121-112505-rescored.csv")
    capsys.readouterr()
    assert lt.main(["--combine", live, tagged]) == 1
    assert "same recording" in capsys.readouterr().err
    assert lt.recording_id(tagged) == lt.recording_id(live) == "u7-minpress-part1-20261121-112505"
    # a repeat run of the same trial file (another recording) is allowed and reported
    other = tmp_path / "u7-minpress-part1-20261121-120000-rescored.csv"
    other.write_text(pathlib.Path(tagged).read_text())
    assert lt.main(["--combine", tagged, str(other)]) == 0
    assert "appear in more than one recording" in capsys.readouterr().out


# ------------------------------------------------------------------ Story 1.6: --mode melody


S16 = HERE / "samples" / "story-1-6"
MEL90 = S16 / "ode-to-joy-90bpm.txt"  # committed: staccato, presses capped at 400 ms (Story 1.6 workaround)
_UNCAPPED: dict[int, pathlib.Path] = {}


def uncapped(bpm: int = 90) -> pathlib.Path:
    """The melody file without the 400 ms press cap (long notes, hand-overs): the analyser rules for cuts,
    repeats and extra onsets need held notes. Generated on demand (Python 3.11+, tomllib)."""
    if bpm not in _UNCAPPED:
        pytest.importorskip("tomllib")
        import atexit
        import shutil
        import tempfile
        from fractions import Fraction

        g6 = lt._load("gen_trials_1_6")
        tf = g6.melody_file(g6.DEFAULT_MELODY, g6.load_profile(), Fraction(bpm), max_press_ms=None)
        d = pathlib.Path(tempfile.mkdtemp(prefix="uncapped-"))
        atexit.register(shutil.rmtree, d, True)
        _UNCAPPED[bpm] = d / tf.name
        _UNCAPPED[bpm].write_text(g6.render(tf), encoding="utf-8")
    return _UNCAPPED[bpm]
LAT = lt.STORY_1_6_LATENCY  # the default heard latency of melody/sustain


def melody_rows_and_presses(path=None):
    rows_, presses = lt.load_presses(path or uncapped())
    return rows_, presses, lt.melody_notes_from_rows(rows_)


def melody_recording(presses, offset=0.4, change=None, decay=0.03, level=0.2):
    """Pretend game: every press sounds its model pitch (+27 c) from the press to the release, then decays.
    `change(k, press)` may return None (silent), a list of (t0, dur, cents) pieces (relative to the press),
    or a pitch offset in semitones (float) for press k (1-based)."""
    x = background(presses[-1].t_release + offset + LAT + 1.5)
    for k, p in enumerate(presses, start=1):
        f = lt.expected_hz(p.model_midi)
        dur = p.t_release - p.t_press
        pieces = [(0.0, dur, 0.0)]
        c = change(k, p) if change else 0.0
        if c is None:
            continue
        if isinstance(c, list):
            pieces = c
        elif c:
            pieces = [(0.0, dur, 100.0 * c)]
        for t0, d, cents_ in pieces:
            place(x, note_wave(f * 2 ** (cents_ / 1200.0), d, decay=decay, level=level), offset + LAT + p.t_press + t0)
    return x


def test_melody_notes_from_the_committed_file():
    _, presses, notes = melody_rows_and_presses()
    assert len(notes) == 62 and len(presses) == 62
    assert [n.index for n in notes] == list(range(1, 63))
    assert notes[0].name == "B5" and notes[0].midi == 83 and notes[0].fingering == "Y"
    assert notes[8].fingering == "R+stick-up" and notes[46].midi == 74
    for n, p in zip(notes, presses):  # the label pitch agrees with the state's model pitch
        assert n.midi == p.model_midi and abs(n.t - p.t_press) < 1e-9


def test_melody_all_notes_ok_relative_to_the_game_tuning():
    _, presses, notes = melody_rows_and_presses()
    res = lt.melody_analyse(melody_recording(presses), SR, notes, 0.4, lt.Params(latency=LAT))
    assert len(res) == 62
    assert [r.verdict for r in res] == ["ok"] * 62, [(r.note.index, r.verdict, r.onsets) for r in res if r.verdict != "ok"]
    assert all(r.audible and r.onsets == 1 for r in res)
    # the game is +27 c of A4 = 440 Hz; that is not a fault, the reference is the run's own median
    assert all(abs(r.cents - lt.GAME_CENTS) < 6 and abs(r.dev_cents) < 6 for r in res)
    lines = lt.melody_summary_lines(res, lt.Params(latency=LAT), lt.MelodyParams())
    assert "right pitch 62/62" in lines[-1] and "missed 0" in lines[-1] and "extra onsets 0" in lines[-1]
    assert "+-50 c" in lines[0] and "10 dB" in lines[0]


def test_melody_missing_note_is_no_sound():
    _, presses, notes = melody_rows_and_presses()
    x = melody_recording(presses, change=lambda k, p: None if k == 20 else 0.0)  # D6 after D6 (X repeat)
    res = lt.melody_analyse(x, SR, notes, 0.4, lt.Params(latency=LAT))
    assert res[19].verdict == "NO SOUND" and not res[19].audible and res[19].onsets == 0
    assert [r.note.index for r in res if r.verdict != "ok"] == [20]
    assert "missed 1" in lt.melody_summary_lines(res, lt.Params(latency=LAT), lt.MelodyParams())[2]


def test_melody_wrong_neighbour_is_found_not_forced():
    _, presses, notes = melody_rows_and_presses()
    x = melody_recording(presses, change=lambda k, p: 1.0 if k == 8 else 0.0)  # A5 sounds A#5
    res = lt.melody_analyse(x, SR, notes, 0.4, lt.Params(latency=LAT))
    assert res[7].verdict == "WRONG PITCH" and lt.g.midi_name(res[7].heard_midi) == "A#5"
    assert [r.note.index for r in res if r.verdict != "ok"] == [8]


def test_melody_nearest_note_right_but_out_of_tune():
    # -70 c from the game's B5 is still nearest B5 (+27 - 70 = -43 c of 440 Hz), but off the run's median
    _, presses, notes = melody_rows_and_presses()
    x = melody_recording(presses, change=lambda k, p: -0.7 if k == 7 else 0.0)
    res = lt.melody_analyse(x, SR, notes, 0.4, lt.Params(latency=LAT))
    assert res[6].heard_midi == 83 and res[6].verdict == "WRONG PITCH" and res[6].dev_cents < -60
    assert sum(r.verdict == "ok" for r in res) == 61


def test_melody_extra_onset_is_reported():
    _, presses, notes = melody_rows_and_presses()

    def split(k, p):  # note 15 (A5, 80 frames) restarts in its middle: a second note
        if k != 15:
            return 0.0
        d = p.t_release - p.t_press
        return [(0.0, 0.5 * d - 0.08, 0.0), (0.5 * d, 0.5 * d, 0.0)]

    res = lt.melody_analyse(melody_recording(presses, change=split), SR, notes, 0.4, lt.Params(latency=LAT))
    assert res[14].onsets == 2 and res[14].verdict == "ok"
    assert all(r.onsets == 1 for r in res if r.note.index != 15)
    assert "extra onsets 1" in lt.melody_summary_lines(res, lt.Params(latency=LAT), lt.MelodyParams())[2]


def test_melody_repeated_pitch_needs_a_dip():
    # B5 B5 at the start (Y, Y): with the release gap there is a dip and a new attack; merged, note 2 has none
    _, presses, notes = melody_rows_and_presses()
    p1, p2 = presses[0], presses[1]

    def merge(k, p):
        if k == 1:
            return [(0.0, p2.t_release - p1.t_press, 0.0)]
        return None if k == 2 else 0.0

    res = lt.melody_analyse(melody_recording(presses, change=merge), SR, notes, 0.4, lt.Params(latency=LAT))
    assert res[1].onsets == 0 and res[1].verdict == "ok"  # it sounds the right pitch, but it was never attacked
    assert "02 B5 Y (no attack)" in lt.melody_summary_lines(res, lt.Params(latency=LAT), lt.MelodyParams())[3]
    res = lt.melody_analyse(melody_recording(presses), SR, notes, 0.4, lt.Params(latency=LAT))
    assert res[1].onsets == 1


def test_melody_window_skips_the_stick_glide():
    # G5 (R + stick up) starts 30 ms about -29 c low (row 2.3): the window starts 40 ms after the onset
    _, presses, notes = melody_rows_and_presses()

    def glide(k, p):
        if p.model_midi != 79:
            return 0.0
        d = p.t_release - p.t_press
        return [(0.0, 0.03, -29.0 * 3), (0.03, d - 0.03, 0.0)]

    res = lt.melody_analyse(melody_recording(presses, change=glide), SR, notes, 0.4, lt.Params(latency=LAT))
    assert all(r.verdict == "ok" for r in res if r.note.midi == 79)


def test_melody_quiet_x_notes_still_audible_at_the_pitch():
    # D6 (X) is the quiet button at the microphone: 14 dB below the others, louder room noise
    _, presses, notes = melody_rows_and_presses()
    x = background(presses[-1].t_release + 0.4 + LAT + 1.5) + loud_background(presses[-1].t_release + 0.4 + LAT + 1.5)
    for p in presses:
        lv = 0.2 if p.model_midi != 86 else 0.04
        place(x, note_wave(lt.expected_hz(p.model_midi), p.t_release - p.t_press, decay=0.03, level=lv),
              0.4 + LAT + p.t_press)
    res = lt.melody_analyse(x, SR, notes, 0.4, lt.Params(latency=LAT))
    assert all(r.verdict == "ok" for r in res), [(r.note.index, r.verdict) for r in res if r.verdict != "ok"]


def test_melody_mode_cli(tmp_path, capsys):
    _, presses, _ = melody_rows_and_presses()
    wav = tmp_path / "mel.wav"
    lt.write_wav(wav, melody_recording(presses, change=lambda k, p: None if k == 33 else 0.0), SR)
    assert lt.main(["--analyse", str(wav), str(uncapped()), "--offset", "0.4", "--mode", "melody"]) == 0
    out = capsys.readouterr().out
    assert "right pitch 61/62" in out and "missed 1" in out and "latency 0.130" in out
    md = (tmp_path / "mel.md").read_text()
    assert "mode: melody" in md and "latency_s: 0.130" in md and "NOTE 33" in md
    # a wrong --latency moves the windows: with 0 s the attacks are found late and notes go wrong
    assert lt.main(["--analyse", str(wav), str(uncapped()), "--offset", "0.4", "--mode", "melody", "--latency", "0.13",
                    "--report-tag", "t"]) == 0
    assert (tmp_path / "mel-t.csv").exists()


def test_melody_mode_needs_note_rows(capsys):
    assert lt.main(["--analyse", "x.wav", str(CAL), "--offset", "0", "--mode", "melody"]) == 1
    assert "NOTE rows" in capsys.readouterr().err


def test_melody_invalid_run_is_flagged():
    _, presses, notes = melody_rows_and_presses()
    res = lt.melody_analyse(melody_recording(presses), SR, notes, 0.4, lt.Params(latency=LAT))
    lines = lt.melody_summary_lines(res, lt.Params(latency=LAT), lt.MelodyParams(), {"valid": "no (input overflow)"})
    assert lines[-1].startswith("INVALID RUN")


def test_count_reattacks_and_max_rise():
    flat = np.zeros(50)
    assert lt.count_reattacks(flat, 8, 8) == 0 and lt.max_rise(flat) == 0
    two = np.concatenate([np.zeros(10), np.full(5, -20.0), np.zeros(10), np.full(5, -20.0), np.zeros(10)])
    assert lt.count_reattacks(two, 8, 8) == 2
    assert lt.count_reattacks(two, 25, 8) == 0  # dip not deep enough
    shallow = np.concatenate([np.zeros(10), np.full(5, -6.0), np.zeros(10)])
    assert lt.count_reattacks(shallow, 8, 8) == 0  # tremolo is not a new note
    assert lt.max_rise(np.concatenate([np.full(5, -40.0), np.zeros(5)])) == 40


# ------------------------------------------------------------------ Story 1.6: --mode sustain


U1P1 = S16 / "u1-sustain-A-part1.txt"


def u1_attempts(path=U1P1):
    rows_, _ = lt.load_presses(path)
    return lt.attempts_from_rows(rows_)


def u1_recording(atts, offset=0.4, auto_stop=6.0, waver_db_per_s=-1.0, blank_sounds=False, shape=None):
    """Pretend game: a held A5 sounds until the release or `auto_stop` s after the press, whichever is first;
    its level falls `waver_db_per_s` dB every second. `shape(a, t)` may return extra gain (dB) per sample."""
    end = max(a.t + a.fields.get("hold_ms", 0) / 1000 for a in atts) + offset + LAT + 2.0
    x = background(end)
    for a in atts:
        if a.blank and not blank_sounds:
            continue
        hold = a.fields.get("hold_ms", 1000) / 1000
        d = min(hold, auto_stop)
        with np.errstate(over="ignore", invalid="ignore"):  # exp() of the unused branch before the stop
            seg = note_wave(A5, d, decay=0.005)
        t = np.arange(seg.size) / SR
        gain_db = waver_db_per_s * np.minimum(t, d)
        if shape is not None:
            gain_db = gain_db + shape(a, t)
        place(x, seg * 10 ** (gain_db / 20), offset + LAT + a.t)
    return x


def test_u1_attempts_from_the_committed_file():
    atts = u1_attempts()
    assert len(atts) == 9 and {a.kind for a in atts} == {"u1"} and {a.button for a in atts} == {"A"}
    assert sorted(a.fields.get("hold_ms", 0) for a in atts) == [0, 1000, 4000, 5000, 5500, 6000, 6500, 7000, 10000]
    assert sum(a.blank for a in atts) == 1 and sum(a.control for a in atts) == 3
    assert all(a.model_midi == 81 and a.round == 1 for a in atts)


def test_sustain_auto_stop_time_and_held_notes():
    atts = u1_attempts()
    res = lt.sustain_analyse(u1_recording(atts), SR, atts, 0.4, lt.Params(latency=LAT))
    by = {r.hold_ms: r for r in res}
    for h in (1000, 4000, 5000, 5500, 6000):
        assert by[h].sound and by[h].interrupted is False, (h, by[h].verdict)
    for h in (6500, 7000, 10000):
        assert by[h].interrupted is True and abs(by[h].stop_ms - 6000) <= 20, (h, by[h].stop_ms)
    assert by[None].verdict == "silent (ok)"


def test_sustain_wavering_in_numbers():
    atts = u1_attempts()
    res = lt.sustain_analyse(u1_recording(atts, waver_db_per_s=-1.0), SR, atts, 0.4, lt.Params(latency=LAT))
    r = next(r for r in res if r.hold_ms == 5000)
    assert len(r.level_per_s) == 4  # full seconds before the release (the heard onset is a little early)
    assert r.level_per_s[0] == 0.0
    for k, v in enumerate(r.level_per_s):
        assert abs(v - (-1.0 * k)) < 0.6, r.level_per_s
    assert all(c is not None and abs(c) < 3 for c in r.cents_per_s)
    long = next(r for r in res if r.hold_ms == 10000)
    assert long.cents_per_s[-1] is None  # after the auto-stop there is no tone to read a pitch from


def test_sustain_traps_not_interrupted():
    atts = u1_attempts()
    four = next(a for a in atts if a.fields.get("hold_ms") == 4000)

    def dip(db, at, dur):
        def shape(a, t):
            if a is not four:
                return np.zeros_like(t)
            return np.where((t >= at) & (t < at + dur), db, 0.0)
        return shape

    for shape, why in ((dip(-12.0, 2.0, 0.5), "a 12 dB sag is wavering, not a stop"),
                       (dip(-60.0, 2.0, 0.10), "a 100 ms dropout is shorter than 150 ms"),
                       (dip(-60.0, 3.95, 0.5), "a drop 50 ms before the release is the release")):
        res = lt.sustain_analyse(u1_recording(atts, shape=shape), SR, [four], 0.4, lt.Params(latency=LAT))
        assert res[0].interrupted is False, why
    res = lt.sustain_analyse(u1_recording(atts, shape=dip(-60.0, 2.0, 0.3)), SR, [four], 0.4, lt.Params(latency=LAT))
    assert res[0].interrupted is True and abs(res[0].stop_ms - 2000) <= 20


def test_sustain_summary_rule_and_cli_combine(tmp_path, capsys):
    paths = []
    for k in (1, 2):
        path = S16 / f"u1-sustain-A-part{k}.txt"
        atts = u1_attempts(path)
        wav = tmp_path / f"u1-sustain-A-part{k}-20261122-10000{k}.wav"
        lt.write_wav(wav, u1_recording(atts), SR)
        assert lt.main(["--analyse", str(wav), str(path), "--offset", "0.4", "--mode", "sustain"]) == 0
        paths.append(str(wav.with_suffix(".csv")))
    out = capsys.readouterr().out
    assert "interrupted" in out and "latency 0.130" in out and "INVALID" not in out
    assert lt.main(["--combine", *paths]) == 0
    out = capsys.readouterr().out
    assert "A         6500     2/2" in out and "A         5500     0/2" in out
    assert "D_ok (largest hold with every attempt held, none silent) = 6000 ms" in out
    assert "candidate floor100(min(D_ok, T_stop) - 500) = 5500 ms" in out


def test_sustain_blank_that_sounds_invalidates(tmp_path, capsys):
    atts = u1_attempts()
    wav = tmp_path / "u1.wav"
    lt.write_wav(wav, u1_recording(atts, blank_sounds=True), SR)
    assert lt.main(["--analyse", str(wav), str(U1P1), "--offset", "0.4", "--mode", "sustain"]) == 0
    out = capsys.readouterr().out
    assert "BLANK SOUNDED" in out and "INVALID" in out


def test_sustain_mode_needs_try_rows(capsys):
    assert lt.main(["--analyse", "x.wav", str(MEL90), "--offset", "0", "--mode", "sustain"]) == 1
    assert "TRY rows" in capsys.readouterr().err


def test_latency_default_only_for_story_1_6_modes():
    p = lt.build_parser()
    assert lt.params_from(p.parse_args(["t.txt"])).latency == 0.0
    assert lt.params_from(p.parse_args(["t.txt", "--mode", "detect"])).latency == 0.0
    assert lt.params_from(p.parse_args(["t.txt", "--mode", "melody"])).latency == LAT
    assert lt.params_from(p.parse_args(["t.txt", "--mode", "sustain", "--latency", "0.12"])).latency == 0.12


def test_melody_missed_note_after_a_hand_over_is_not_the_previous_tail():
    # note 33 (B5 on Y) follows A5 on A without a release gap; if Y is ignored, A5 only decays: NO SOUND,
    # not "WRONG PITCH A5" (the tail is gone by the second half of the window)
    _, presses, notes = melody_rows_and_presses()
    x = melody_recording(presses, change=lambda k, p: None if k == 33 else 0.0)
    res = lt.melody_analyse(x, SR, notes, 0.4, lt.Params(latency=LAT))
    assert res[32].verdict == "NO SOUND"
    assert [r.note.index for r in res if r.verdict != "ok"] == [33]


@pytest.mark.parametrize("bpm", [60, 120])
def test_melody_other_tempos_all_ok(bpm):
    _, presses, notes = melody_rows_and_presses(S16 / f"ode-to-joy-{bpm}bpm.txt")
    res = lt.melody_analyse(melody_recording(presses), SR, notes, 0.4, lt.Params(latency=LAT))
    assert len(res) == 62 and all(r.verdict == "ok" and r.onsets == 1 for r in res), \
        [(r.note.index, r.verdict, r.onsets) for r in res if r.verdict != "ok" or r.onsets != 1]


def test_count_reattacks_needs_a_long_dip_and_a_real_re_rise():
    click = np.concatenate([np.zeros(20), np.full(2, -20.0), np.zeros(20)])  # 10 ms dip at 5 ms hops
    assert lt.count_reattacks(click, 8, 8) == 1 and lt.count_reattacks(click, 8, 8, min_dip=6) == 0
    regap = np.concatenate([np.zeros(20), np.full(12, -30.0), np.zeros(20)])  # a 60 ms re-press gap
    assert lt.count_reattacks(regap, 8, 8, min_dip=6, reach_db=10) == 1
    after_cut = np.concatenate([np.zeros(20), np.full(10, -40.0), np.full(3, -28.0), np.full(10, -40.0)])
    assert lt.count_reattacks(after_cut, 8, 8, min_dip=6) == 1  # noise bump in the silence ...
    assert lt.count_reattacks(after_cut, 8, 8, min_dip=6, reach_db=10) == 0  # ... is not a new note


def test_melody_cut_note_and_clicks_are_not_extra_onsets():
    # Seen live (2026-11-21, 60 BPM run): a held note stops early and the room noise / clicks in the
    # silence or over a held note were counted as extra attacks; a silent note had "3 onsets".
    _, presses, notes = melody_rows_and_presses()

    def cut(k, p):
        if k == 15:  # A5 half note stops 0.55 s after its start
            return [(0.0, 0.55, 0.0)]
        return None if k == 16 else 0.0

    x = melody_recording(presses, change=cut)
    h15 = 0.4 + LAT + presses[14].t_press
    for dt in np.arange(0.65, 1.2, 0.12):  # short noise bursts in the silence after the cut (note 15 lasts 1.33 s)
        place(x, RNG.normal(0, 0.01, int(0.02 * SR)), h15 + dt)
    h1 = 0.4 + LAT + presses[0].t_press
    place(x, -0.19 * note_wave(lt.expected_hz(83), 0.01, level=1.0), h1 + 0.5)  # a 10 ms notch over note 1
    res = lt.melody_analyse(x, SR, notes, 0.4, lt.Params(latency=LAT))
    assert res[14].onsets == 1 and res[0].onsets == 1
    assert res[15].verdict == "NO SOUND" and res[15].onsets == 0


def test_melody_notes_know_their_release():
    _, presses, notes = melody_rows_and_presses()
    for n, p in zip(notes, presses):
        assert abs(n.t_release - p.t_release) < 1e-9


def test_melody_cut_short_verdict_and_sounded_length():
    # 90 BPM: note 15 is an A5 half note (80 frames = 1.333 s); it stops 0.55 s after its start
    _, presses, notes = melody_rows_and_presses()
    x = melody_recording(presses, change=lambda k, p: [(0.0, 0.55, 0.0)] if k == 15 else 0.0, decay=0.005)
    res = lt.melody_analyse(x, SR, notes, 0.4, lt.Params(latency=LAT))
    r = res[14]
    assert r.verdict == "CUT SHORT" and abs(r.sounded_ms - 550) <= 20, (r.verdict, r.sounded_ms)
    assert [q.note.index for q in res if q.verdict != "ok"] == [15]
    lines = lt.melody_summary_lines(res, lt.Params(latency=LAT), lt.MelodyParams())
    assert "right pitch 62/62, missed 0, cut 1," in lines[2]
    assert lines[3].startswith("cut short: 15 A5 A sounded 5") and "of 1333 ms" in lines[3]
    assert "cut short: the tone falls >= 20 dB" in lines[0]


@pytest.mark.parametrize("why, pieces", [
    ("a 100 ms dropout is shorter than 150 ms", [(0.0, 0.4, 0.0), (0.5, 0.833, 0.0)]),
    ("a stop 60 ms before the release is the release", [(0.0, 1.273, 0.0)]),
    ("a 12 dB sag is not a stop", None),
])
def test_melody_cut_short_traps(why, pieces):
    _, presses, notes = melody_rows_and_presses()
    x = melody_recording(presses, change=lambda k, p: pieces if (k == 15 and pieces) else (None if k == 15 else 0.0),
                         decay=0.005)
    if pieces is None:  # note 15 at full level for 0.4 s, then 12 dB lower to its release
        p15 = presses[14]
        t0 = 0.4 + LAT + p15.t_press
        place(x, note_wave(lt.expected_hz(81), 0.4, decay=0.005), t0)
        place(x, 0.25 * note_wave(lt.expected_hz(81), p15.t_release - p15.t_press - 0.4, decay=0.005, attack=0.0),
              t0 + 0.4)
    res = lt.melody_analyse(x, SR, notes, 0.4, lt.Params(latency=LAT))
    assert res[14].verdict == "ok" and res[14].sounded_ms is None, why


def test_melody_cli_reports_cut(tmp_path, capsys):
    _, presses, _ = melody_rows_and_presses()
    wav = tmp_path / "cut.wav"
    lt.write_wav(wav, melody_recording(presses, change=lambda k, p: [(0.0, 0.3, 0.0)] if k == 13 else 0.0,
                                       decay=0.005), SR)
    assert lt.main(["--analyse", str(wav), str(uncapped()), "--offset", "0.4", "--mode", "melody"]) == 0
    out = capsys.readouterr().out
    assert "CUT SHORT" in out and "cut 1" in out and "cut short: 13 B5 Y sounded 3" in out
    assert "sounded_ms" in (tmp_path / "cut.csv").read_text().splitlines()[0]


A5X5 = "\n".join(["0f - # neutral lead-in"] + [
    f"{120 + 60 * k}f A # NOTE {k + 1:02d} A5 81 fingering=A\n{175 + 60 * k}f - # release" for k in range(5)
] + ["480f - # end"]) + "\n"


def test_melody_early_stop_close_to_the_release():
    # Live shape (A5 x 17, 60 BPM, 2026-11-21): presses of 55 frames, 5-frame re-press gaps; notes 07 and 14
    # stopped about 110 ms before their heard release (a 180 ms silence instead of about 30 ms). The 150 ms
    # rule cannot see it because the next note starts again 180 ms later.
    rows_ = lt.s.parse_lines(A5X5.splitlines())
    presses = lt.presses_from_rows(rows_)
    notes = lt.melody_notes_from_rows(rows_)
    x = melody_recording(presses, change=lambda k, p: [(0.0, 55 / 60 - 0.11, 0.0)] if k == 3 else 0.0,
                         decay=0.01)  # the live stop fell 10 dB within about 10 ms
    res = lt.melody_analyse(x, SR, notes, 0.4, lt.Params(latency=LAT))
    assert [r.verdict for r in res] == ["ok", "ok", "CUT SHORT", "ok", "ok"], [(r.verdict, r.sounded_ms) for r in res]
    assert abs(res[2].sounded_ms - (55 / 60 - 0.11) * 1000) <= 25
    lines = lt.melody_summary_lines(res, lt.Params(latency=LAT), lt.MelodyParams())
    assert "cut 1" in lines[2] and "stays down from 75 to 300 ms before the heard release" in lines[0]


def test_melody_early_stop_margin_absorbs_latency_jitter():
    # the heard release is up to about a frame off the assumed latency: a note that ends 40 ms "early" is fine
    rows_ = lt.s.parse_lines(A5X5.splitlines())
    presses = lt.presses_from_rows(rows_)
    x = melody_recording(presses, change=lambda k, p: [(0.0, 55 / 60 - 0.04, 0.0)] if k == 3 else 0.0, decay=0.01)
    res = lt.melody_analyse(x, SR, lt.melody_notes_from_rows(rows_), 0.4, lt.Params(latency=LAT))
    assert all(r.verdict == "ok" for r in res)


STACCATO = "\n".join(["0f - # neutral lead-in"] + [
    f"{120 + 120 * k}f R - 80:00 # NOTE {k + 1:02d} G5 79 fingering=R+stick-up\n{144 + 120 * k}f - - 80:00 # release"
    for k in range(4)
] + ["600f - # end"]) + "\n"


def test_melody_staccato_note_in_a_long_slot_is_audible_and_release_bounce_is_no_onset():
    # Live (2026-11-21, --max-press-ms 400 at 60 BPM): a 0.4 s G5 in a 2 s slot read NO SOUND (the silence after
    # the release diluted the level), and a G5 released with the stick still up dipped and bounced back ~5 dB
    # within 50 ms, counted as a second onset.
    rows_ = lt.s.parse_lines(STACCATO.splitlines())
    presses = lt.presses_from_rows(rows_)
    notes = lt.melody_notes_from_rows(rows_)
    x = background(presses[-1].t_release + 0.4 + LAT + 2.0)
    for p in presses:
        h = 0.4 + LAT + p.t_press
        place(x, note_wave(lt.expected_hz(79), 0.4, decay=0.005, level=0.006), h)
        place(x, note_wave(lt.expected_hz(79), 0.05, decay=0.03, level=0.006 * 10 ** (-5 / 20)), h + 0.43)
    x += loud_background(x.size / SR, level=0.004)
    res = lt.melody_analyse(x, SR, notes, 0.4, lt.Params(latency=LAT))
    assert [r.verdict for r in res] == ["ok"] * 4, [(r.verdict, r.level_db - r.noise_db) for r in res]
    assert all(r.onsets == 1 for r in res), [r.onsets for r in res]


def test_melody_committed_staccato_file_all_ok():
    # the committed 90 BPM file: every long note is a 400 ms press then silence (Story 1.6 workaround)
    _, presses, notes = melody_rows_and_presses(MEL90)
    assert max(p.t_release - p.t_press for p in presses) <= 0.4 + 1e-9
    res = lt.melody_analyse(melody_recording(presses), SR, notes, 0.4, lt.Params(latency=LAT))
    assert all(r.verdict == "ok" and r.onsets == 1 for r in res), [(r.note.index, r.verdict, r.onsets) for r in res
                                                                  if r.verdict != "ok" or r.onsets != 1]


def test_sustain_early_stop_is_the_7th_note_event_not_the_auto_stop(tmp_path, capsys):
    # an attempt stopped 0.8 s after its onset (the Story 1.6 event) must not become T_stop or break D_ok
    atts = u1_attempts()
    five = next(a for a in atts if a.fields.get("hold_ms") == 5000)

    def shape(a, t):
        return np.where(t >= 0.8, -80.0, 0.0) if a is five else np.zeros_like(t)

    wav = tmp_path / "u1-sustain-A-part1-20261122-100001.wav"
    lt.write_wav(wav, u1_recording(atts, shape=shape), SR)
    assert lt.main(["--analyse", str(wav), str(U1P1), "--offset", "0.4", "--mode", "sustain"]) == 0
    out = capsys.readouterr().out
    n = five.fields["attempt"]
    assert "EARLY STOPS" in out and f"A 5000 ms attempt {n} (note {n + 1} of its file, warm-up = 1) stopped at 8" in out
    assert "A         5000     0/0" in out  # left out of the counts
    assert "D_ok (largest hold with every attempt held, none silent) = 6000 ms" in out
    assert "candidate floor100(min(D_ok, T_stop) - 500) = 5500 ms" in out and "INVALID" not in out


# ------------------------------------------------------------------ Story 2.5: IOI statistics, --driver, explain_ioi


def test_ioi_stats_hand_computed():
    p = lt.Params(latency=0.0)
    sched = [0.0, 1.0, 2.0, 3.0]
    st = lt.ioi_stats([0.5, 1.51, 2.49, 3.5], sched, 0.5, p)
    # gaps 1.01, 0.98, 1.01 s against 1 s: errors +10, -20, +10 ms
    assert st.pairs == 3 and abs(st.mean_ms) < 1e-9
    assert abs(st.sigma_ms - math.sqrt(300.0)) < 1e-6
    assert abs(st.p95_ms - 19.0) < 1e-6 and abs(st.max_ms - 20.0) < 1e-6  # numpy linear percentile of 10, 10, 20
    assert st.missing == ()
    # a constant shift of every onset (another latency) and another offset give the same IOI error
    st2 = lt.ioi_stats([0.9, 1.91, 2.89, 3.9], sched, 0.3, p)
    assert (st2.pairs, round(st2.p95_ms, 6), round(st2.max_ms, 6)) == (3, 19.0, 20.0)
    # a note with no onset is listed and left out of the pairs
    st3 = lt.ioi_stats([0.5, None, 2.49, 3.5], sched, 0.5, p, indices=[1, 2, 3, 4])
    assert st3.pairs == 1 and st3.missing == (2,) and abs(st3.max_ms - 10.0) < 1e-6
    assert lt.ioi_stats([None, None], [0.0, 1.0], 0.0, p).pairs == 0
    assert "no onset (excluded from the pairs): 02" in lt.ioi_lines(st3)[1]


def test_ioi_on_a_clean_melody_is_within_the_analyser_allowance():
    """D26: the onset method on the committed melody played perfectly: the p95 IOI error is the analyser's
    own noise and must stay inside ANALYSER_NOISE_MS."""
    _, presses, notes = melody_rows_and_presses(MEL90)
    p = lt.Params(latency=LAT)
    x = melody_recording(presses)
    res = lt.melody_analyse(x, SR, notes, 0.4, p)
    on = lt.melody_onsets(x, SR, notes, 0.4, p, [r.audible for r in res])
    st = lt.ioi_stats(on, [n.t for n in notes], 0.4, p, [n.index for n in notes])
    assert st.pairs == 61 and st.missing == ()
    assert st.p95_ms < lt.ANALYSER_NOISE_MS, st
    assert abs(st.drift_ms_per_s) < 0.2


def test_ioi_sees_a_late_note_and_skips_a_missing_one():
    _, presses, notes = melody_rows_and_presses(MEL90)
    p = lt.Params(latency=LAT)

    def change(k, pr):
        if k == 5:
            return None  # silent: no onset, excluded
        if k == 20:
            return [(0.010, pr.t_release - pr.t_press, 0.0)]  # sounds 10 ms late
        return 0.0

    x = melody_recording(presses, change=change)
    res = lt.melody_analyse(x, SR, notes, 0.4, p)
    on = lt.melody_onsets(x, SR, notes, 0.4, p, [r.audible for r in res])
    st = lt.ioi_stats(on, [n.t for n in notes], 0.4, p, [n.index for n in notes])
    assert st.missing == (5,) and st.pairs == 59
    assert abs(st.max_ms - 10.0) < 2.5, st


def test_explain_ioi_three_verdicts():
    floor = (1 - math.sqrt(0.05)) * 1000.0 / 60.0
    assert abs(floor - 12.940) < 0.01
    met = lt.explain_ioi(12.0, 40, 8000)
    assert met["verdict"] == "MET" and abs(met["firmware_bound_ms"] - 8.04) < 1e-9
    assert abs(met["game_floor_ms"] - floor) < 1e-9
    ex = lt.explain_ioi(18.0, 40, 8000)
    assert abs(ex["residual_ms"] - (18.0 - math.sqrt(8.04 ** 2 + floor ** 2))) < 1e-9  # 2.77 ms
    assert ex["verdict"] == "EXPLAINED"
    assert lt.explain_ioi(22.0, 40, 8000)["verdict"] == "UNEXPLAINED"   # residual 6.8 ms > 4 ms
    assert lt.explain_ioi(18.0, 500, 8000)["verdict"] == "UNEXPLAINED"  # firmware bound 8.5 ms > half a frame
    line = lt.explain_lines(ex, 18.0, 40, 8000)[0]
    assert "EXPLAINED" in line and "0.776 frame" in line


def test_driver_parsing_and_offset():
    lines = [(1.0, "   0.0  ok   INFO received\n"), (2.0, "T0_HOST_MONO=103.250000\n"),
             (3.0, "PASTE_LINE: run 1 | late max 37 us, mean 12.0 us, hist 158/0/0/0/0/0 | IN median 8000, min 7950, "
                   "p95 8050, max 8100 us, slow 0 | loop max 41 us\n"),
             (4.0, "T0_HOST_MONO=200.000000\n")]
    assert lt.driver_t0(lines) == 103.25  # the first run
    assert lt.driver_self_timing(lines) == (37.0, 8000.0)
    assert lt.driver_t0([(1.0, "FAIL console_ready\n")]) is None
    assert lt.driver_self_timing([]) is None
    # chunks of 480 samples (10 ms) from host time 100.0: the WAV time of t0 is 3.25 s
    chunks = [(100.0 + 0.01 * (i + 1), 480 * (i + 1)) for i in range(600)]
    offset, scale = lt.alignment(chunks, SR, lt.driver_t0(lines))
    assert abs(offset - 3.25) < 1e-6 and abs(scale - 1.0) < 1e-6


def test_driver_argv_expands_the_port(monkeypatch):
    monkeypatch.setenv("P", "/dev/cu.usbmodem1234")
    argv = lt.driver_argv('python3 firmware/tools/link_probe.py --port "$P" selfplay --lead-ms 3000')
    assert argv == ["python3", "firmware/tools/link_probe.py", "--port", "/dev/cu.usbmodem1234", "selfplay",
                    "--lead-ms", "3000"]


class FakeRecorder:
    """The capture of a live run, without a microphone: writes `x` as the WAV, chunks from host 100.0."""

    def __init__(self, x, wav_path):
        self.x = x
        self.wav_path = wav_path
        self.chunks = []
        self.total = 0
        self.overflows = 0

    def start(self, timeout=5.0):
        lt.write_wav(self.wav_path, self.x, SR)
        n = self.x.size
        self.chunks = [(100.0 + (k + 480) / SR, k + 480) for k in range(0, n - 480, 480)]
        self.total = n

    def stop(self):
        return ""


def _live_driver(monkeypatch, tmp_path, lines, x):
    monkeypatch.setattr(lt, "make_recorder", lambda backend, device, sr, wav_path, ffmpeg="ffmpeg":
                        FakeRecorder(x, wav_path))
    monkeypatch.setattr(lt, "run_driver", lambda cmd: (0, lines, False))
    return lt.main(["--backend", "sounddevice", "--mode", "melody", "--tail", "0", "--out-dir", str(tmp_path),
                    "--driver", "python3 link_probe.py --port \"$P\" selfplay", str(MEL90)])


def test_driver_live_run_end_to_end(monkeypatch, tmp_path, capsys):
    _, presses, _ = melody_rows_and_presses(MEL90)
    x = melody_recording(presses)  # schedule t=0 at WAV 0.4 s
    lines = [(1.0, "T0_HOST_MONO=100.400000\n"),
             (2.0, "PASTE_LINE: run 1 | late max 37 us, mean 12.0 us, hist 158/0/0/0/0/0 | IN median 8000, min 7950, "
                   "p95 8050, max 8100 us, slow 0 | loop max 41 us\n")]
    assert _live_driver(monkeypatch, tmp_path, lines, x) == 0
    out = capsys.readouterr().out
    assert "driver exit 0" in out and "offset 0.400 s" in out
    assert "right pitch 62/62" in out and "IOI error (heard onset gap minus scheduled gap, 61 pairs)" in out
    assert "AC3 (Story 2.5 D16)" in out and ": MET" in out
    md = next(tmp_path.glob("*.md")).read_text()
    assert "IOI error" in md and "driver:" in md


def test_driver_without_t0_fails_the_run(monkeypatch, tmp_path, capsys):
    _, presses, _ = melody_rows_and_presses(MEL90)
    lines = [(1.0, "FAIL console_ready 1 for 2 s in a row within 120 s of the prompt\n")]
    assert _live_driver(monkeypatch, tmp_path, lines, melody_recording(presses[:3])) == 1
    assert "printed no T0_HOST_MONO= line" in capsys.readouterr().err
