# /// script
# requires-python = ">=3.10"
# dependencies = ["pyserial>=3.5"]
# ///
"""Log every line the 2wiCC firmware sends back over the bridge port, with timestamps (read only).

SPDX-License-Identifier: MIT (the project licence, see LICENSE at the repo root).

THROWAWAY SPIKE CODE (Story 1.6 diagnosis). send_2wicc.py never reads the port,
so unsolicited firmware messages (`+ERR WATCHDOG` = the board rebooted,
`+WRN TIMEOUT 0/1` = the console changed USB timeout 4, `+LGHT xx` = the console
set the player lights, typically when it (re)initialises the controller,
`+RMBL ...` if rumble reporting is on) are never seen. Run this in a second
terminal during a trial: it opens the same port, NEVER writes to it, and
prints/logs each line with the time since its start and the wall-clock time.

Order matters: start this logger BEFORE the trial and stop it (Ctrl+C) AFTER
the trial has ended. The bridge only forwards host bytes while the port is
open with DTR asserted (spikes/README.md, OQ-9); on macOS DTR drops on the last
close, so this logger must not be the first to close while the sender runs.

Usage: uv run spikes/serial_rx_log.py --port /dev/cu.usbmodem101
       uv run spikes/serial_rx_log.py --port /dev/cu.usbmodem101 --seconds 120
"""

from __future__ import annotations

import argparse
import datetime as dt
import pathlib
import signal
import sys
import time
from collections.abc import Callable, Sequence

HERE = pathlib.Path(__file__).resolve().parent
DEFAULT_OUT = HERE / "recordings"  # git-ignored


class LineSplitter:
    """Bytes in, complete lines out (CR/LF stripped); a partial line waits for the rest."""

    def __init__(self) -> None:
        self.buf = b""

    def feed(self, data: bytes) -> list[str]:
        self.buf += data
        *lines, self.buf = self.buf.split(b"\n")
        return [ln.rstrip(b"\r").decode("ascii", "replace") for ln in lines if ln.strip(b"\r")]


def run(read: Callable[[], bytes], clock: Callable[[], float], wall: Callable[[], dt.datetime],
        emit: Callable[[str], object], stop: Callable[[float], bool], errors: list | None = None) -> int:
    """Read until `stop(elapsed)` is true, Ctrl+C or a port error; emit one line per received line. Returns the
    count; a port error (OSError, which includes pyserial's SerialException) is emitted and added to `errors`."""
    t0 = clock()
    split = LineSplitter()
    n = 0
    emit(f"start {wall().isoformat(timespec='milliseconds')}")
    try:
        while not stop(clock() - t0):
            data = read()
            if not data:
                continue
            for line in split.feed(data):
                n += 1
                emit(f"t={clock() - t0:9.3f}s  {wall().isoformat(timespec='milliseconds')}  {line}")
    except KeyboardInterrupt:
        pass
    except OSError as e:  # the port went away (unplugged bridge) or another read error
        emit(f"t={clock() - t0:9.3f}s  PORT ERROR {e}")
        if errors is not None:
            errors.append(e)
    if split.buf.strip():
        n += 1
        emit(f"t={clock() - t0:9.3f}s  (partial) {split.buf.decode('ascii', 'replace')}")
    emit(f"end {wall().isoformat(timespec='milliseconds')}, {n} line(s) received")
    return n


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="serial_rx_log.py",
                                description="Throwaway spike: log what the 2wiCC firmware sends back (read only).")
    p.add_argument("--port", required=True, help="bridge serial port (the same as the sender's)")
    p.add_argument("--baud", type=int, default=460800, help="baud rate (default 460800, as the sender)")
    p.add_argument("--seconds", type=float, help="stop after this many seconds (default: until Ctrl+C)")
    p.add_argument("--out-dir", default=str(DEFAULT_OUT), help="log folder (default spikes/recordings, git-ignored)")
    return p


def main(argv: Sequence[str] | None = None, open_port=None) -> int:
    a = build_parser().parse_args(argv)
    if open_port is None:
        import serial  # lazily, so the tests need no pyserial

        def open_port(port, baud):
            # read only: no write ever happens; DTR asserted like the sender (pyserial default)
            return serial.Serial(port, baudrate=baud, timeout=0.05)
    try:
        ser = open_port(a.port, a.baud)
    except OSError as e:
        print(f"error: cannot open {a.port}: {e}", file=sys.stderr)
        return 1
    out = pathlib.Path(a.out_dir)
    log = out / f"rx-{dt.datetime.now().strftime('%Y%m%d-%H%M%S')}.log"
    errors: list = []
    try:
        out.mkdir(parents=True, exist_ok=True)
        fh = open(log, "w", encoding="utf-8")  # noqa: SIM115 - closed by the `with fh` block below
    except OSError as e:
        ser.close()
        print(f"error: cannot write {log}: {e}", file=sys.stderr)
        return 1
    with fh:
        def emit(text: str) -> None:
            print(text, flush=True)
            fh.write(text + "\n")
            fh.flush()

        limit = a.seconds
        stopping = []
        # Ctrl+C only sets a flag: the loop ends cleanly and exits 0 (uv and the terminal may both deliver
        # SIGINT; a second KeyboardInterrupt during the shutdown used to end with a traceback and exit 1).
        old = signal.signal(signal.SIGINT, lambda *_: stopping.append(True))
        try:
            run(lambda: ser.read(256), time.monotonic, dt.datetime.now, emit,
                lambda el: bool(stopping) or (limit is not None and el >= limit), errors)
        finally:
            ser.close()
            signal.signal(signal.SIGINT, old)
    print(f"log: {log}")
    return 2 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
