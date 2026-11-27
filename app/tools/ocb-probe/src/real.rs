//! The bench driver: the real worker thread on the real backend, the real
//! log, the terminal.

use std::io::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ocb_services::log::{LOG_FILE, Logger, log_dir};
use ocb_services::port::SerialBackend;
use ocb_services::worker::{
    Clock as _, LinkSnapshot, RealClock, TimedEvent, WorkerHandle, host_rules, spawn_with,
};

use crate::runner::{Driver, host_name};

/// How long `finish` waits for the worker thread.
const SHUTDOWN_LIMIT: Duration = Duration::from_secs(2);

/// The real worker and log.
#[derive(Debug)]
pub struct RealDriver {
    handle: WorkerHandle,
    clock: RealClock,
    logger: Option<Logger>,
    snapshot: Option<Arc<LinkSnapshot>>,
    interrupt: Arc<AtomicBool>,
    finished: Option<(Result<Vec<String>, String>, String)>,
}

impl RealDriver {
    /// Starts the log and the worker on the real backend.
    ///
    /// # Errors
    /// The OS error if the worker thread cannot start.
    pub fn start(interrupt: Arc<AtomicBool>) -> std::io::Result<Self> {
        let logger = Logger::start();
        let clock = RealClock::new();
        let handle = spawn_with(
            SerialBackend::new(),
            clock,
            host_rules(),
            logger.handle(),
            Box::new(|| {}),
        )?;
        Ok(Self {
            handle,
            clock,
            logger: Some(logger),
            snapshot: None,
            interrupt,
            finished: None,
        })
    }
}

/// This process's lines of a log text: from the last `start` line with
/// this PID on.
#[must_use]
pub fn lines_of_pid(text: &str, pid: u32) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let tag = format!(" pid={pid}");
    let from = lines
        .iter()
        .rposition(|l| l.split(' ').nth(2) == Some("start") && l.ends_with(&tag));
    from.map_or_else(Vec::new, |i| {
        lines
            .get(i..)
            .unwrap_or(&[])
            .iter()
            .map(|l| (*l).to_owned())
            .collect()
    })
}

/// This process's lines across a rotation: `bonooru.1.log` (if any) then
/// `bonooru.log`, from the last `start` line with this PID on.
#[must_use]
pub fn run_lines(older: Option<&str>, current: &str, pid: u32) -> Vec<String> {
    let mut text = older.unwrap_or("").to_owned();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(current);
    lines_of_pid(&text, pid)
}

impl Driver for RealDriver {
    fn next(&mut self, timeout_us: u64) -> Option<TimedEvent> {
        // Short slices, so Ctrl+C is seen within 50 ms.
        let slice = Duration::from_micros(timeout_us.min(50_000));
        while let Ok(s) = self.handle.snapshots.try_recv() {
            self.snapshot = Some(s);
        }
        self.handle.events.recv_timeout(slice).ok()
    }

    fn now_us(&self) -> u64 {
        self.clock.now_us()
    }

    fn snapshot(&mut self) -> Option<Arc<LinkSnapshot>> {
        while let Ok(s) = self.handle.snapshots.try_recv() {
            self.snapshot = Some(s);
        }
        self.snapshot.clone()
    }

    fn interrupted(&self) -> bool {
        self.interrupt.load(Ordering::SeqCst)
    }

    fn host(&self) -> &'static str {
        host_name()
    }

    fn say(&mut self, line: &str) {
        println!("{line}");
        let _ = std::io::stdout().flush();
    }

    fn finish(&mut self) -> (Result<Vec<String>, String>, String) {
        if let Some(f) = &self.finished {
            return f.clone();
        }
        let _ = self.snapshot();
        let _ = self.handle.shutdown(SHUTDOWN_LIMIT);
        let path = self
            .logger
            .as_ref()
            .and_then(|l| l.path().map(std::path::Path::to_path_buf))
            .or_else(|| log_dir().map(|d| d.join(LOG_FILE)));
        if let Some(l) = self.logger.take() {
            // Everything queued is written before the file is read (bounded
            // wait, also when the worker did not end in time).
            let _ = l.stop();
        }
        let shown = path
            .as_ref()
            .map_or_else(|| "-".to_owned(), |p| p.display().to_string());
        let lines = match &path {
            None => Err("no log directory".to_owned()),
            Some(p) => std::fs::read_to_string(p)
                .map(|t| {
                    // The file may have rotated during the run: the start
                    // line can be in `bonooru.1.log`.
                    let older = p
                        .parent()
                        .and_then(|d| std::fs::read_to_string(d.join("bonooru.1.log")).ok());
                    run_lines(older.as_deref(), &t, std::process::id())
                })
                .map_err(|e| format!("{}: {e}", p.display())),
        };
        let out = (lines, shown);
        self.finished = Some(out.clone());
        out
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]

    use super::*;

    #[test]
    fn lines_from_the_last_start_of_this_pid() {
        let text = "\
t +0ms start app=1 host=macos arch=aarch64 pid=7
t +1ms port_opened port=a
t +0ms start app=1 host=macos arch=aarch64 pid=9
t +2ms port_opened port=b
t +0ms start app=1 host=macos arch=aarch64 pid=7
t +3ms port_opened port=c
t +4ms version x";
        let l = lines_of_pid(text, 7);
        assert_eq!(l.len(), 3);
        assert!(l[1].ends_with("port=c"));
        assert!(lines_of_pid(text, 8).is_empty());
    }

    #[test]
    fn lines_across_a_rotation() {
        let older = "t +0ms start app=1 host=macos arch=aarch64 pid=7\nt +1ms port_opened port=a\n";
        let current = "t +2ms port_lost port=a\nt +3ms version x";
        let l = run_lines(Some(older), current, 7);
        assert_eq!(l.len(), 4, "{l:?}");
        assert!(l[3].ends_with("version x"));
        assert_eq!(run_lines(None, current, 7).len(), 0);
    }
}
