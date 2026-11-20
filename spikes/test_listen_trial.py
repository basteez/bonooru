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
