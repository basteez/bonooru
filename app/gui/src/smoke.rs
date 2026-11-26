//! The smoke run (`bonooru --smoke-test`, AC 8): open the real window,
//! render a few frames, print one `OCB_SMOKE ok ...` line and close. A
//! watchdog ends the process with [`EXIT_TIMEOUT`] if the window has not
//! closed in time.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use raw_window_handle::RawWindowHandle;

/// Frames rendered before the smoke run reports and closes.
pub const FRAMES: u32 = 3;
/// The watchdog's limit.
pub const TIMEOUT: Duration = Duration::from_secs(60);

/// Exit code: eframe returned an error, or the window closed before the
/// smoke report.
pub const EXIT_ERROR: u8 = 1;
/// Exit code: unknown argument.
pub const EXIT_USAGE: u8 = 2;
/// Exit code: the smoke watchdog fired.
pub const EXIT_TIMEOUT: u8 = 3;

/// What the smoke line reports.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub frames: u32,
    pub ppp: f32,
    /// Inner size in points.
    pub inner: (f32, f32),
    /// Minimum inner size in points.
    pub min: (f32, f32),
    pub dark: bool,
    pub backend: &'static str,
}

impl Report {
    /// The single stdout line, e.g.
    /// `OCB_SMOKE ok frames=3 ppp=2 inner=1280x800 min=1024x680 theme=dark backend=AppKit`.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "OCB_SMOKE ok frames={} ppp={} inner={}x{} min={}x{} theme={} backend={}",
            self.frames,
            self.ppp,
            self.inner.0.round(),
            self.inner.1.round(),
            self.min.0.round(),
            self.min.1.round(),
            if self.dark { "dark" } else { "light" },
            self.backend,
        )
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

/// The smoke state of a running app: counts frames until it reports.
#[derive(Debug)]
pub struct Smoke {
    frames: u32,
    reported: Arc<AtomicBool>,
}

impl Smoke {
    /// `reported` is set once the line has been printed; `main` reads it to
    /// tell a clean smoke exit from a window closed early.
    #[must_use]
    pub fn new(reported: Arc<AtomicBool>) -> Self {
        Self {
            frames: 0,
            reported,
        }
    }

    /// Counts one frame. Returns `true` exactly once, on frame [`FRAMES`]:
    /// the caller then prints the report and closes the window.
    pub fn frame(&mut self) -> bool {
        self.frames = self.frames.saturating_add(1);
        self.frames == FRAMES
    }

    #[must_use]
    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// Marks the report as printed.
    pub fn mark_reported(&self) {
        self.reported.store(true, Ordering::SeqCst);
    }
}

/// Waits `timeout`, then calls `on_timeout(EXIT_TIMEOUT)` unless `done` is
/// set. Runs on the watchdog thread; split out so the timeout path is
/// testable with an injected duration.
pub fn watchdog(timeout: Duration, done: &AtomicBool, on_timeout: impl FnOnce(u8)) {
    std::thread::sleep(timeout);
    if !done.load(Ordering::SeqCst) {
        on_timeout(EXIT_TIMEOUT);
    }
}

/// Starts the watchdog thread: after [`TIMEOUT`] it prints
/// `OCB_SMOKE timeout` and exits the process with [`EXIT_TIMEOUT`].
/// (`std::process::exit` is fine in the app crate; the purity rules are
/// for `ocb-core`.)
pub fn spawn_watchdog(done: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        watchdog(TIMEOUT, &done, |code| {
            println!("OCB_SMOKE timeout after {} s", TIMEOUT.as_secs());
            std::process::exit(i32::from(code));
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_format() {
        let r = Report {
            frames: 3,
            ppp: 1.5,
            inner: (1280.0, 800.0),
            min: (1024.0, 680.0),
            dark: true,
            backend: "Xlib",
        };
        assert_eq!(
            r.line(),
            "OCB_SMOKE ok frames=3 ppp=1.5 inner=1280x800 min=1024x680 theme=dark backend=Xlib"
        );
    }

    #[test]
    fn frame_counter_reports_once_on_the_third_frame() {
        let mut s = Smoke::new(Arc::default());
        let hits: Vec<bool> = (0..6).map(|_| s.frame()).collect();
        assert_eq!(hits, [false, false, true, false, false, false]);
        assert_eq!(s.frames(), 6);
    }

    #[test]
    fn watchdog_fires_with_exit_code_3() {
        let done = AtomicBool::new(false);
        let mut got = None;
        watchdog(Duration::ZERO, &done, |code| got = Some(code));
        assert_eq!(got, Some(3));
    }

    #[test]
    fn watchdog_stays_quiet_once_done() {
        let done = AtomicBool::new(true);
        let mut got = None;
        watchdog(Duration::ZERO, &done, |code| got = Some(code));
        assert_eq!(got, None);
    }

    #[test]
    fn backend_names() {
        assert_eq!(backend_name(None), "none");
    }
}
