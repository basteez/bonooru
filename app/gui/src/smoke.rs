//! The smoke run (`bonooru --smoke-test`, AC 8): open the real window,
//! render [`FRAMES`] frames and wait for the serial worker's first snapshot
//! (Story 4.4), judge what the window really is (theme, measured
//! inner size, window-system backend), print one `OCB_SMOKE ok ...` or
//! `OCB_SMOKE fail (...) ...` line and close. A watchdog ends the process
//! with [`EXIT_TIMEOUT`] if the event loop has not returned in time.

use std::io::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use raw_window_handle::RawWindowHandle;

/// Completed frames before the smoke run reports and closes.
pub const FRAMES: u64 = 3;
/// The watchdog's limit, from start until the event loop has returned.
pub const TIMEOUT: Duration = Duration::from_secs(60);
/// How long the smoke run waits for the worker's first snapshot after its
/// frames (then it reports `worker=0`, a fail line), well inside [`TIMEOUT`].
pub const WORKER_WAIT: Duration = Duration::from_secs(20);
/// Grace for `std::process::exit` after a watchdog timeout: if the exit
/// itself hangs (an `atexit` handler of a stuck driver), abort.
pub const EXIT_GRACE: Duration = Duration::from_secs(5);
/// Tolerance on the measured inner size: a fractional `pixels_per_point`
/// can turn 1024 pt into 1023.9 pt.
pub const SIZE_TOLERANCE: f32 = 0.5;

/// Exit code: eframe returned an error, or the window closed before the
/// smoke report.
pub const EXIT_ERROR: u8 = 1;
/// Exit code: unknown or non-UTF-8 argument.
pub const EXIT_USAGE: u8 = 2;
/// Exit code: the smoke watchdog fired.
pub const EXIT_TIMEOUT: u8 = 3;
/// Exit code: the smoke verdict failed (the `OCB_SMOKE fail` line).
pub const EXIT_SMOKE_FAIL: u8 = 4;

/// The window-system backends a smoke run may report on this OS: X11 on
/// Linux (AD-18), AppKit on macOS, Win32 on Windows.
#[must_use]
pub const fn expected_backends() -> &'static [&'static str] {
    if cfg!(target_os = "linux") {
        &["Xlib", "Xcb"]
    } else if cfg!(target_os = "macos") {
        &["AppKit"]
    } else if cfg!(target_os = "windows") {
        &["Win32"]
    } else {
        &["Xlib", "Xcb", "Wayland", "AppKit", "Win32", "WinRt"]
    }
}

/// What the smoke run measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// Completed frames (`Context::cumulative_frame_nr`), not egui passes.
    pub frames: u64,
    pub ppp: f32,
    /// Inner size in points, from the viewport (`None` if not reported).
    pub inner: Option<(f32, f32)>,
    /// The configured minimum inner size in points.
    pub min: (f32, f32),
    pub dark: bool,
    pub backend: &'static str,
    /// Scans the serial worker had done (its latest snapshot, Story 4.4).
    pub worker: u64,
}

impl Report {
    /// The verdict: `Err(reason)` naming the first failed condition.
    ///
    /// # Errors
    /// Fewer than [`FRAMES`] frames, a light theme, no or a too small
    /// measured inner size, a backend not in `expected`, or no worker scan.
    pub fn verdict(&self, expected: &[&str]) -> Result<(), String> {
        if self.frames < FRAMES {
            return Err(format!("frames={} < {FRAMES}", self.frames));
        }
        if !self.dark {
            return Err("theme=light".to_owned());
        }
        let Some((w, h)) = self.inner else {
            return Err("no inner size reported".to_owned());
        };
        if w + SIZE_TOLERANCE < self.min.0 || h + SIZE_TOLERANCE < self.min.1 {
            return Err(format!(
                "inner={w}x{h} below min={}x{}",
                self.min.0, self.min.1
            ));
        }
        if !expected.contains(&self.backend) {
            return Err(format!(
                "backend={} not one of {}",
                self.backend,
                expected.join("|")
            ));
        }
        if self.worker < 1 {
            return Err(format!("worker={} < 1", self.worker));
        }
        Ok(())
    }

    fn fields(&self) -> String {
        let inner = self.inner.map_or_else(
            || "none".to_owned(),
            |(w, h)| format!("{}x{}", w.round(), h.round()),
        );
        format!(
            "frames={} ppp={} inner={inner} min={}x{} theme={} backend={} worker={}",
            self.frames,
            self.ppp,
            self.min.0.round(),
            self.min.1.round(),
            if self.dark { "dark" } else { "light" },
            self.backend,
            self.worker,
        )
    }

    /// The single stdout line for a verdict, e.g.
    /// `OCB_SMOKE ok frames=3 ppp=2 inner=1280x800 min=1024x680 theme=dark backend=AppKit worker=1`
    /// or `OCB_SMOKE fail (theme=light) frames=3 ...`.
    #[must_use]
    pub fn line(&self, verdict: &Result<(), String>) -> String {
        match verdict {
            Ok(()) => format!("OCB_SMOKE ok {}", self.fields()),
            Err(reason) => format!("OCB_SMOKE fail ({reason}) {}", self.fields()),
        }
    }
}

/// The `raw-window-handle` variant name of a window handle.
#[must_use]
pub fn backend_name(handle: Option<RawWindowHandle>) -> &'static str {
    match handle {
        None => "none",
        Some(RawWindowHandle::Xlib(_)) => "Xlib",
        Some(RawWindowHandle::Xcb(_)) => "Xcb",
        Some(RawWindowHandle::Wayland(_)) => "Wayland",
        Some(RawWindowHandle::AppKit(_)) => "AppKit",
        Some(RawWindowHandle::Win32(_)) => "Win32",
        Some(RawWindowHandle::WinRt(_)) => "WinRt",
        Some(_) => "other",
    }
}

/// Outcome of a smoke run, shared between the app and `main`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pending,
    Ok,
    Fail,
}

/// Shared outcome cell (`main` reads it after the event loop returns).
#[derive(Clone, Debug, Default)]
pub struct OutcomeCell(Arc<AtomicU8>);

impl OutcomeCell {
    pub fn set(&self, outcome: Outcome) {
        let v = match outcome {
            Outcome::Pending => 0,
            Outcome::Ok => 1,
            Outcome::Fail => 2,
        };
        self.0.store(v, Ordering::SeqCst);
    }

    #[must_use]
    pub fn get(&self) -> Outcome {
        match self.0.load(Ordering::SeqCst) {
            1 => Outcome::Ok,
            2 => Outcome::Fail,
            _ => Outcome::Pending,
        }
    }
}

/// The smoke state of a running app.
#[derive(Debug)]
pub struct Smoke {
    outcome: OutcomeCell,
    report: Option<Report>,
    started: Instant,
}

impl Smoke {
    #[must_use]
    pub fn new(outcome: OutcomeCell) -> Self {
        Self {
            outcome,
            report: None,
            started: Instant::now(),
        }
    }

    /// Called once per egui pass with `ctx.cumulative_frame_nr()` and the
    /// scan count of the latest worker snapshot (`None` before the first).
    /// Returns `true` exactly once, in the first pass after [`FRAMES`]
    /// frames have completed and a worker snapshot has arrived: the caller
    /// then measures and calls [`Smoke::finish`]. Extra passes of one frame
    /// (an egui discard) do not count.
    /// Without a snapshot after [`WORKER_WAIT`] it reports anyway, so a
    /// worker that never started or never published gives an
    /// `OCB_SMOKE fail (worker=0 < 1)` line, not a watchdog timeout.
    #[must_use]
    pub fn due(&self, completed_frames: u64, worker_scans: Option<u64>) -> bool {
        self.due_after(completed_frames, worker_scans, self.started.elapsed())
    }

    /// [`Smoke::due`] with the time since the smoke run started given.
    #[must_use]
    pub fn due_after(
        &self,
        completed_frames: u64,
        worker_scans: Option<u64>,
        waited: Duration,
    ) -> bool {
        self.report.is_none()
            && completed_frames >= FRAMES
            && (worker_scans.is_some() || waited >= WORKER_WAIT)
    }

    /// Judges the report, stores the outcome and returns the line to print.
    pub fn finish(&mut self, report: Report) -> String {
        let verdict = report.verdict(expected_backends());
        let line = report.line(&verdict);
        self.outcome.set(if verdict.is_ok() {
            Outcome::Ok
        } else {
            Outcome::Fail
        });
        self.report = Some(report);
        line
    }

    /// The report, once made.
    #[must_use]
    pub fn report(&self) -> Option<&Report> {
        self.report.as_ref()
    }
}

/// The exit code `main` returns for a smoke run whose event loop returned.
#[must_use]
pub fn exit_code(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Ok => 0,
        Outcome::Fail => EXIT_SMOKE_FAIL,
        Outcome::Pending => EXIT_ERROR,
    }
}

/// Waits `timeout`, then calls `on_timeout(EXIT_TIMEOUT)` unless `exited`
/// is set. `exited` is set by `main` only after `run_native` has returned,
/// i.e. after the window has really closed. Split out so the timeout path
/// is testable with an injected duration.
pub fn watchdog(timeout: Duration, exited: &AtomicBool, on_timeout: impl FnOnce(u8)) {
    std::thread::sleep(timeout);
    if !exited.load(Ordering::SeqCst) {
        on_timeout(EXIT_TIMEOUT);
    }
}

/// Starts the watchdog thread. On a timeout it prints `OCB_SMOKE timeout`,
/// flushes stdout and calls `std::process::exit(3)`. `exit` runs the C
/// `atexit` handlers (Mesa, Xlib), which can block if the main thread is
/// stuck inside the driver; a second thread therefore aborts the process
/// after [`EXIT_GRACE`] if `exit` has not finished by then (exit code from
/// the abort, still non-zero). `unsafe_code` is forbidden, so `_exit` is not
/// available; see the Story 4.1 Debug Log.
pub fn spawn_watchdog(exited: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        watchdog(TIMEOUT, &exited, |code| {
            println!(
                "OCB_SMOKE timeout: the event loop did not return within {} s",
                TIMEOUT.as_secs()
            );
            let _ = std::io::stdout().flush();
            std::thread::spawn(|| {
                std::thread::sleep(EXIT_GRACE);
                eprintln!("OCB_SMOKE timeout: exit hung, aborting");
                std::process::abort();
            });
            std::process::exit(i32::from(code));
        });
    });
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::num::{NonZeroIsize, NonZeroU32};
    use std::ptr::NonNull;

    use raw_window_handle::{
        AppKitWindowHandle, WaylandWindowHandle, Win32WindowHandle, XcbWindowHandle,
        XlibWindowHandle,
    };

    use super::*;

    fn good() -> Report {
        Report {
            frames: 3,
            ppp: 1.5,
            inner: Some((1280.0, 800.0)),
            min: (1024.0, 680.0),
            dark: true,
            backend: "Xlib",
            worker: 1,
        }
    }

    const X11: &[&str] = &["Xlib", "Xcb"];

    #[test]
    fn ok_line_format() {
        let r = good();
        assert_eq!(
            r.line(&r.verdict(X11)),
            "OCB_SMOKE ok frames=3 ppp=1.5 inner=1280x800 min=1024x680 theme=dark backend=Xlib worker=1"
        );
    }

    #[test]
    fn every_failed_condition_is_a_fail_line() {
        let cases: [(Report, &str); 7] = [
            (
                Report {
                    frames: 2,
                    ..good()
                },
                "frames=2 < 3",
            ),
            (
                Report {
                    dark: false,
                    ..good()
                },
                "theme=light",
            ),
            (
                Report {
                    inner: None,
                    ..good()
                },
                "no inner size reported",
            ),
            (
                Report {
                    inner: Some((1000.0, 800.0)),
                    ..good()
                },
                "inner=1000x800 below min=1024x680",
            ),
            (
                Report {
                    inner: Some((1280.0, 600.0)),
                    ..good()
                },
                "inner=1280x600 below min=1024x680",
            ),
            (
                Report {
                    backend: "none",
                    ..good()
                },
                "backend=none not one of Xlib|Xcb",
            ),
            (
                Report {
                    worker: 0,
                    ..good()
                },
                "worker=0 < 1",
            ),
        ];
        for (r, reason) in cases {
            let v = r.verdict(X11);
            assert_eq!(v, Err(reason.to_owned()));
            assert!(
                r.line(&v)
                    .starts_with(&format!("OCB_SMOKE fail ({reason}) frames="))
            );
        }
        let wayland = Report {
            backend: "Wayland",
            ..good()
        };
        assert!(wayland.verdict(X11).is_err(), "Wayland is not X11 (AD-18)");
        let none_line = Report {
            inner: None,
            ..good()
        };
        assert!(
            none_line
                .line(&none_line.verdict(X11))
                .contains(" inner=none ")
        );
    }

    #[test]
    fn fractional_ppp_rounding_is_tolerated() {
        let r = Report {
            inner: Some((1023.6, 679.6)),
            ..good()
        };
        assert_eq!(r.verdict(X11), Ok(()));
        let r = Report {
            inner: Some((1023.4, 680.0)),
            ..good()
        };
        assert!(r.verdict(X11).is_err());
    }

    #[test]
    fn due_once_after_three_completed_frames_and_a_worker_snapshot() {
        let mut s = Smoke::new(OutcomeCell::default());
        // Frame numbers seen per pass; frame 1 has two passes (a discard).
        let passes = [0, 1, 1, 2];
        assert!(passes.iter().all(|&n| !s.due(n, Some(1))));
        assert!(!s.due(3, None), "waits for the first worker snapshot");
        assert!(!s.due(9, None));
        assert!(s.due(3, Some(1)));
        let line = s.finish(good());
        assert!(line.starts_with("OCB_SMOKE ok ") || line.starts_with("OCB_SMOKE fail "));
        assert!(
            !s.due(3, Some(1)) && !s.due(4, Some(2)),
            "reports only once"
        );
    }

    #[test]
    fn no_worker_snapshot_reports_worker_0_after_the_wait() {
        let mut s = Smoke::new(OutcomeCell::default());
        let almost = WORKER_WAIT - Duration::from_millis(1);
        assert!(!s.due_after(3, None, almost));
        assert!(
            s.due_after(3, None, WORKER_WAIT),
            "reports without a snapshot"
        );
        assert!(!s.due_after(2, None, WORKER_WAIT), "frames still first");
        assert!(WORKER_WAIT < TIMEOUT);
        let report = Report {
            worker: 0,
            backend: expected_backends().first().copied().unwrap(),
            ..good()
        };
        let line = s.finish(report);
        assert!(line.starts_with("OCB_SMOKE fail (worker=0 < 1) "), "{line}");
    }

    #[test]
    fn outcome_and_exit_codes() {
        let cell = OutcomeCell::default();
        assert_eq!(cell.get(), Outcome::Pending);
        assert_eq!(exit_code(cell.get()), 1);
        let mut s = Smoke::new(cell.clone());
        let bad = Report {
            dark: false,
            ..good()
        };
        let _ = s.finish(bad);
        assert_eq!(cell.get(), Outcome::Fail);
        assert_eq!(exit_code(cell.get()), 4);
        cell.set(Outcome::Ok);
        assert_eq!(exit_code(cell.get()), 0);
    }

    #[test]
    fn watchdog_fires_with_exit_code_3() {
        let exited = AtomicBool::new(false);
        let mut got = None;
        watchdog(Duration::ZERO, &exited, |code| got = Some(code));
        assert_eq!(got, Some(3));
    }

    #[test]
    fn watchdog_stays_quiet_once_the_loop_returned() {
        let exited = AtomicBool::new(true);
        let mut got = None;
        watchdog(Duration::ZERO, &exited, |code| got = Some(code));
        assert_eq!(got, None);
    }

    #[test]
    fn backend_names() {
        let p = NonNull::<std::ffi::c_void>::dangling();
        assert_eq!(backend_name(None), "none");
        let h = |r: RawWindowHandle| backend_name(Some(r));
        assert_eq!(h(XlibWindowHandle::new(1).into()), "Xlib");
        assert_eq!(
            h(XcbWindowHandle::new(NonZeroU32::new(1).unwrap()).into()),
            "Xcb"
        );
        assert_eq!(h(WaylandWindowHandle::new(p).into()), "Wayland");
        assert_eq!(h(AppKitWindowHandle::new(p).into()), "AppKit");
        assert_eq!(
            h(Win32WindowHandle::new(NonZeroIsize::new(1).unwrap()).into()),
            "Win32"
        );
    }
}
