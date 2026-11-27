//! The rotating file log (Story 4.4 AC 8, the "Logging" convention of the
//! architecture spine): `bonooru.log` in the OS log directory, rotated at
//! 1 MiB with three old files kept, written by its own thread so the worker
//! never waits on the disk.
//!
//! One line per entry: `<UTC time to the ms> +<ms since start>ms <kind> <text>`.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TrySendError, bounded};

/// The log file's name.
pub const LOG_FILE: &str = "bonooru.log";
/// Rotate when a line would take the file past this size.
pub const MAX_BYTES: u64 = 1024 * 1024;
/// Old files kept (`bonooru.1.log` to `bonooru.3.log`).
pub const KEEP: u32 = 3;
/// Entries waiting for the log thread; more are dropped and counted.
pub const QUEUE: usize = 1024;

/// The OS log directory (Story 4.4 D8): macOS `~/Library/Logs/Bonooru`,
/// Windows `%LOCALAPPDATA%\Bonooru\logs`, Linux `$XDG_STATE_HOME/bonooru/logs`
/// (default `~/.local/state/bonooru/logs`). `None` when the platform gives
/// no home directory.
#[must_use]
pub fn log_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        let base = directories::BaseDirs::new()?;
        return Some(base.home_dir().join("Library").join("Logs").join("Bonooru"));
    }
    let dirs = directories::ProjectDirs::from("", "", "Bonooru")?;
    if cfg!(windows) {
        // `data_local_dir` is `%LOCALAPPDATA%\Bonooru\data`.
        return Some(dirs.data_local_dir().parent()?.join("logs"));
    }
    Some(dirs.state_dir()?.join("logs"))
}

/// Where the log lines go.
pub trait LogSink: Send {
    /// Writes one line (without its newline).
    ///
    /// # Errors
    /// The I/O error.
    fn write_line(&mut self, line: &str) -> io::Result<()>;

    /// Something the log should record about itself (a failed rotation),
    /// once.
    fn take_notice(&mut self) -> Option<String> {
        None
    }
}

/// `bonooru.log` with size-based rotation.
#[derive(Debug)]
pub struct RotatingFile {
    dir: PathBuf,
    max_bytes: u64,
    keep: u32,
    file: File,
    size: u64,
    /// No rotation attempt before the file reaches this size (after a
    /// failed rotation).
    hold_until: u64,
    notice: Option<String>,
}

impl RotatingFile {
    /// Creates `dir` if needed and opens `bonooru.log` for appending.
    ///
    /// # Errors
    /// The I/O error (directory or file).
    pub fn open(dir: &Path, max_bytes: u64, keep: u32) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join(LOG_FILE);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let size = file.metadata()?.len();
        Ok(Self {
            dir: dir.to_path_buf(),
            max_bytes,
            keep,
            file,
            size,
            hold_until: 0,
            notice: None,
        })
    }

    /// The current log file's path.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.dir.join(LOG_FILE)
    }

    fn old(&self, n: u32) -> PathBuf {
        self.dir.join(format!("bonooru.{n}.log"))
    }

    fn reopen(&mut self) -> io::Result<()> {
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path())?;
        self.size = self.file.metadata().map_or(0, |m| m.len());
        Ok(())
    }

    /// Rotates. The current file is first moved aside to `bonooru.0.log`;
    /// if that fails (Windows: a viewer holds it), nothing else is touched,
    /// so a failed rotation never eats the old files.
    fn rotate(&mut self) -> io::Result<()> {
        let cur = self.path();
        if !cur.exists() {
            // Deleted while the app runs: start a new file.
            return self.reopen();
        }
        let aside = self.dir.join("bonooru.0.log");
        let _ = fs::remove_file(&aside);
        fs::rename(&cur, &aside)?;
        // Oldest first, so no rename lands on an existing file; the target
        // is removed anyway (Windows `rename` fails onto an existing file).
        let _ = fs::remove_file(self.old(self.keep));
        for n in (1..self.keep).rev() {
            let from = self.old(n);
            if from.exists() {
                let to = self.old(n + 1);
                let _ = fs::remove_file(&to);
                let _ = fs::rename(&from, &to);
            }
        }
        if self.keep > 0 {
            let to = self.old(1);
            let _ = fs::remove_file(&to);
            let _ = fs::rename(&aside, &to);
        } else {
            let _ = fs::remove_file(&aside);
        }
        self.reopen()
    }
}

impl LogSink for RotatingFile {
    fn write_line(&mut self, line: &str) -> io::Result<()> {
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\n');
        let len = bytes.len() as u64;
        if self.size > 0 && self.size + len > self.max_bytes && self.size >= self.hold_until {
            if let Err(e) = self.rotate() {
                // Keep writing to the current file (reopened if need be);
                // try again only after another 1/16 of the limit, so a
                // rotation that keeps failing costs neither the old files
                // nor every later line. Reported once through `notice`.
                let _ = self.reopen();
                self.hold_until = self.size + self.max_bytes / 16;
                self.notice = Some(format!("rotation failed: {e}"));
            } else {
                self.hold_until = 0;
            }
        }
        // One write call per line, then a flush: a line is never split
        // across files, and the log survives a crash.
        self.file.write_all(&bytes)?;
        self.file.flush()?;
        self.size += len;
        Ok(())
    }

    fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }
}

/// Lines kept in memory (tests and dry runs).
#[derive(Debug, Clone, Default)]
pub struct MemorySink(Arc<Mutex<Vec<String>>>);

impl MemorySink {
    /// An empty sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every line written so far.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl LogSink for MemorySink {
    fn write_line(&mut self, line: &str) -> io::Result<()> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(line.to_owned());
        Ok(())
    }
}

/// A sink that always fails (tests of `LogUnavailable`).
#[derive(Debug, Clone, Default)]
pub struct FailingSink;

impl LogSink for FailingSink {
    fn write_line(&mut self, _line: &str) -> io::Result<()> {
        Err(io::Error::other("disk full"))
    }
}

/// Drops every line (the log directory could not be opened).
struct DiscardSink;

impl LogSink for DiscardSink {
    fn write_line(&mut self, _line: &str) -> io::Result<()> {
        Ok(())
    }
}

/// One entry on its way to the log thread.
#[derive(Debug, Clone)]
pub struct Entry {
    at: SystemTime,
    since_start_ms: u64,
    kind: &'static str,
    text: String,
}

/// Civil date from days since 1970-01-01 (H. Hinnant's `civil_from_days`).
#[must_use]
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` of a `SystemTime` (UTC; before 1970 is
/// printed as 1970).
#[must_use]
pub fn utc_timestamp(t: SystemTime) -> String {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = i64::try_from(d.as_secs()).unwrap_or(i64::MAX);
    let ms = d.subsec_millis();
    let (y, mo, da) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{mo:02}-{da:02}T{:02}:{:02}:{:02}.{ms:03}Z",
        s / 3600,
        (s / 60) % 60,
        s % 60
    )
}

/// The text of one log line.
#[must_use]
pub fn format_line(at: SystemTime, since_start_ms: u64, kind: &str, text: &str) -> String {
    // One entry, one line: line breaks inside the text become spaces.
    let text: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    format!("{} +{since_start_ms}ms {kind} {text}", utc_timestamp(at))
}

impl Entry {
    fn line(&self) -> String {
        format_line(self.at, self.since_start_ms, self.kind, &self.text)
    }
}

/// The sending side of the log, cloned into every thread that logs.
#[derive(Clone)]
pub struct LogHandle {
    tx: Option<Sender<Entry>>,
    start: Instant,
    dropped: Arc<AtomicU64>,
    unavailable: Arc<Mutex<Option<String>>>,
    reported: Arc<AtomicU64>,
}

impl fmt::Debug for LogHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LogHandle")
            .field("enabled", &self.tx.is_some())
            .field("dropped", &self.dropped.load(Ordering::Relaxed))
            .finish()
    }
}

impl LogHandle {
    /// A handle that logs nothing (tests that do not look at the log).
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            tx: None,
            start: Instant::now(),
            dropped: Arc::new(AtomicU64::new(0)),
            unavailable: Arc::new(Mutex::new(None)),
            reported: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Queues one entry; never blocks. When the queue is full the entry is
    /// dropped and counted; the log thread writes the count when it next
    /// has room.
    pub fn log(&self, kind: &'static str, text: impl Into<String>) {
        let Some(tx) = &self.tx else {
            return;
        };
        let entry = Entry {
            at: SystemTime::now(),
            since_start_ms: u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX),
            kind,
            text: text.into(),
        };
        if let Err(TrySendError::Full(_)) = tx.try_send(entry) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Entries dropped since start (written or not yet written).
    #[must_use]
    pub fn dropped_total(&self) -> u64 {
        self.reported.load(Ordering::Relaxed) + self.dropped.load(Ordering::Relaxed)
    }

    /// The reason the log cannot be written, once (later calls: `None`).
    #[must_use]
    pub fn take_unavailable(&self) -> Option<String> {
        self.unavailable
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

/// The log thread and its handle.
#[derive(Debug)]
pub struct Logger {
    handle: LogHandle,
    join: Option<JoinHandle<()>>,
    path: Option<PathBuf>,
    stopping: Arc<AtomicBool>,
}

/// How long [`Logger::stop`] waits for the log thread.
pub const STOP_LIMIT: Duration = Duration::from_millis(500);

/// The `start` line (AC 8): app version, OS, arch, PID.
#[must_use]
pub fn start_text() -> String {
    format!(
        "app={} host={} arch={} pid={}",
        ocb_core::version::APP_VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::process::id()
    )
}

impl Logger {
    /// The real log: `bonooru.log` in [`log_dir`]. When the directory
    /// cannot be created or the file opened, the app keeps running: the
    /// entries are dropped and the reason is reported once through
    /// [`LogHandle::take_unavailable`].
    #[must_use]
    pub fn start() -> Self {
        let opened = log_dir()
            .ok_or_else(|| "no home directory for the log".to_owned())
            .and_then(|dir| {
                RotatingFile::open(&dir, MAX_BYTES, KEEP)
                    .map_err(|e| format!("{}: {e}", dir.display()))
            });
        match opened {
            Ok(file) => {
                let path = file.path();
                let mut l = Self::spawn(Box::new(file), true, None);
                l.path = Some(path);
                l
            }
            Err(reason) => {
                // Entries are discarded silently: the reason is reported once.
                let l = Self::spawn(Box::new(DiscardSink), false, None);
                *l.handle
                    .unavailable
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(reason);
                l
            }
        }
    }

    /// The log thread (`bonooru-log`) on `sink`. With `start_line`, the
    /// thread first writes the `start` entry. With `gate`, it waits for one
    /// message (or a disconnect) on it before reading entries (tests of the
    /// full queue).
    #[must_use]
    pub fn spawn(sink: Box<dyn LogSink>, start_line: bool, gate: Option<Receiver<()>>) -> Self {
        let (tx, rx) = bounded::<Entry>(QUEUE);
        let mut handle = LogHandle::disabled();
        handle.tx = Some(tx);
        let start_entry = start_line.then(|| Entry {
            at: SystemTime::now(),
            since_start_ms: 0,
            kind: "start",
            text: start_text(),
        });
        let stopping = Arc::new(AtomicBool::new(false));
        let thread = LogThread {
            stopping: Arc::clone(&stopping),
            sink,
            rx,
            dropped: Arc::clone(&handle.dropped),
            reported: Arc::clone(&handle.reported),
            unavailable: Arc::clone(&handle.unavailable),
            failed: false,
            start: handle.start,
        };
        let join = std::thread::Builder::new()
            .name("bonooru-log".to_owned())
            .spawn(move || thread.run(start_entry, gate))
            .ok();
        Self {
            handle,
            join,
            path: None,
            stopping,
        }
    }

    /// A handle to log with.
    #[must_use]
    pub fn handle(&self) -> LogHandle {
        self.handle.clone()
    }

    /// The file written, when this is the real log.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Ends the log thread after it has written everything already
    /// queued, waiting at most [`STOP_LIMIT`]. Other handles may still
    /// exist (a worker that did not end in time): their later entries are
    /// dropped, the queued ones are written. Returns whether the thread
    /// ended (it is joined then); never hangs on a slow disk.
    pub fn stop(self) -> bool {
        self.stop_within(STOP_LIMIT)
    }

    /// [`Logger::stop`] with a given limit.
    pub fn stop_within(mut self, limit: Duration) -> bool {
        self.stopping.store(true, Ordering::SeqCst);
        self.handle.tx = None;
        let Some(j) = self.join.take() else {
            return true;
        };
        let start = Instant::now();
        while !j.is_finished() {
            if start.elapsed() >= limit {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let _ = j.join();
        true
    }
}

struct LogThread {
    stopping: Arc<AtomicBool>,
    sink: Box<dyn LogSink>,
    rx: Receiver<Entry>,
    dropped: Arc<AtomicU64>,
    reported: Arc<AtomicU64>,
    unavailable: Arc<Mutex<Option<String>>>,
    failed: bool,
    start: Instant,
}

impl LogThread {
    fn write(&mut self, line: &str) {
        if let Err(e) = self.sink.write_line(line)
            && !self.failed
        {
            self.failed = true;
            *self
                .unavailable
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(e.to_string());
        }
    }

    fn meta(&mut self, kind: &str, text: &str) {
        let line = format_line(
            SystemTime::now(),
            u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX),
            kind,
            text,
        );
        self.write(&line);
    }

    /// Writes one entry, then what the log has to say about itself: a
    /// failed rotation, and how many entries were dropped while the queue
    /// was full (there is room again).
    fn entry(&mut self, entry: &Entry) {
        self.write(&entry.line());
        if let Some(n) = self.sink.take_notice() {
            self.meta("log_rotate_failed", &n);
        }
        let n = self.dropped.swap(0, Ordering::Relaxed);
        if n > 0 {
            self.reported.fetch_add(n, Ordering::Relaxed);
            self.meta("log_dropped", &n.to_string());
        }
    }

    fn run(mut self, start_entry: Option<Entry>, gate: Option<Receiver<()>>) {
        if let Some(e) = start_entry {
            self.write(&e.line());
        }
        if let Some(g) = gate {
            let _ = g.recv();
        }
        loop {
            if self.stopping.load(Ordering::SeqCst) {
                break;
            }
            match self.rx.recv_timeout(Duration::from_millis(20)) {
                Ok(e) => self.entry(&e),
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
        // Stopping with other senders still alive: what is queued is
        // written, nothing more is waited for.
        while let Ok(e) = self.rx.try_recv() {
            self.entry(&e);
        }
    }
}
