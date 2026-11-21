# SPDX-License-Identifier: MIT
# Throwaway spike tests for spikes/serial_rx_log.py. No port, no hardware, no real sleeps.
# Run: uv run --with pytest pytest spikes/test_serial_rx_log.py
from __future__ import annotations

import datetime as dt
import importlib.util
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("serial_rx_log", HERE / "serial_rx_log.py")
rx = importlib.util.module_from_spec(_spec)
sys.modules["serial_rx_log"] = rx
_spec.loader.exec_module(rx)


def test_line_splitter_handles_partial_and_crlf():
    sp = rx.LineSplitter()
    assert sp.feed(b"+LGHT 0") == []
    assert sp.feed(b"1\r\n+WRN TIMEOUT 1\r\n\r\n+ERR") == ["+LGHT 01", "+WRN TIMEOUT 1"]
    assert sp.feed(b" WATCHDOG\n") == ["+ERR WATCHDOG"]


class FakeClock:
    def __init__(self):
        self.t = 100.0

    def __call__(self):
        return self.t


def test_run_timestamps_each_line_and_stops():
    clock = FakeClock()
    chunks = [b"", b"+LGHT 01\r\n", b"", b"+WRN TIM", b"EOUT 0\r\n", b"+RMBL 00"]

    def read():
        clock.t += 0.5
        return chunks.pop(0) if chunks else b""

    out = []
    wall = lambda: dt.datetime(2026, 11, 21, 19, 0, 0)
    n = rx.run(read, clock, wall, out.append, lambda el: el >= 4.0)
    assert n == 3
    assert out[0].startswith("start 2026-11-21T19:00:00.000")
    assert "t=    1.000s" in out[1] and out[1].endswith("+LGHT 01")
    assert "t=    2.500s" in out[2] and out[2].endswith("+WRN TIMEOUT 0")
    assert "(partial) +RMBL 00" in out[3]
    assert out[-1].endswith("3 line(s) received")


class FakeSerial:
    def __init__(self, data):
        self.data = list(data)
        self.writes = 0
        self.closed = False

    def read(self, n):
        if not self.data:
            raise KeyboardInterrupt  # ends the run like Ctrl+C
        return self.data.pop(0)

    def write(self, b):  # pragma: no cover - must never be called
        self.writes += 1

    def close(self):
        self.closed = True


def test_main_never_writes_and_logs_to_file(tmp_path, capsys):
    fake = FakeSerial([b"+ERR WATCHDOG\r\n"])
    assert rx.main(["--port", "X", "--out-dir", str(tmp_path)], open_port=lambda p, b: fake) == 0
    assert fake.writes == 0 and fake.closed
    (log,) = tmp_path.glob("rx-*.log")
    text = log.read_text()
    assert "+ERR WATCHDOG" in text and "1 line(s) received" in text
    assert "log:" in capsys.readouterr().out


def test_main_open_error(tmp_path, capsys):
    def bad(p, b):
        raise OSError("busy")

    assert rx.main(["--port", "X", "--out-dir", str(tmp_path)], open_port=bad) == 1
    assert "cannot open X" in capsys.readouterr().err


def test_main_stops_cleanly_on_sigint(tmp_path):
    import signal

    class SigSerial(FakeSerial):
        def read(self, n):
            signal.raise_signal(signal.SIGINT)  # Ctrl+C (twice) while reading
            signal.raise_signal(signal.SIGINT)
            return b""

    before = signal.getsignal(signal.SIGINT)
    fake = SigSerial([])
    assert rx.main(["--port", "X", "--out-dir", str(tmp_path)], open_port=lambda p, b: fake) == 0
    assert fake.closed and signal.getsignal(signal.SIGINT) is before


def test_port_error_is_reported_and_exits_non_zero(tmp_path, capsys):
    class Gone(FakeSerial):
        def read(self, n):
            if self.data:
                return self.data.pop(0)
            raise OSError("device disconnected")

    fake = Gone([b"+LGHT 01\r\n"])
    assert rx.main(["--port", "X", "--out-dir", str(tmp_path)], open_port=lambda p, b: fake) == 2
    assert fake.closed
    text = next(tmp_path.glob("rx-*.log")).read_text()
    assert "+LGHT 01" in text and "PORT ERROR device disconnected" in text and "1 line(s) received" in text


def test_log_file_error_closes_the_port(tmp_path, capsys):
    fake = FakeSerial([])
    blocker = tmp_path / "file"
    blocker.write_text("x")  # --out-dir is a file: the log cannot be created
    assert rx.main(["--port", "X", "--out-dir", str(blocker / "sub")], open_port=lambda p, b: fake) == 1
    assert fake.closed and "cannot write" in capsys.readouterr().err
