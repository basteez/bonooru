//! C.22: the rotating file log (Story 4.4 AC 8): rotation at 1 MiB with
//! three old files, the line format, the bounded queue with drop counting,
//! and the start line. Temp directories only, never the real log dir.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use ocb_services::log::{
    KEEP, LOG_FILE, LogSink, Logger, MAX_BYTES, MemorySink, QUEUE, RotatingFile, civil_from_days,
    format_line, log_dir, utc_timestamp,
};

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ocb-log-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    d
}

#[test]
fn c22_rotation_keeps_three_old_files() {
    assert_eq!(MAX_BYTES, 1_048_576);
    assert_eq!(KEEP, 3);
    let dir = temp_dir("rotate");
    let mut f = RotatingFile::open(&dir, MAX_BYTES, KEEP).unwrap();
    // 4.5 MiB of 200-byte lines (199 characters + newline), numbered.
    let total = (4.5 * 1_048_576.0 / 200.0) as usize + 1;
    for i in 0..total {
        let head = format!("line {i:08} ");
        let line = format!("{head}{}", "x".repeat(199 - head.len()));
        f.write_line(&line).unwrap();
    }
    let names = [
        "bonooru.log",
        "bonooru.1.log",
        "bonooru.2.log",
        "bonooru.3.log",
    ];
    for n in names {
        let len = fs::metadata(dir.join(n)).unwrap().len();
        assert!(len <= MAX_BYTES, "{n} is {len} B");
        assert!(len > 0, "{n} is empty");
        // Every line whole: 200 bytes each.
        assert_eq!(len % 200, 0, "{n}: a line was split");
    }
    assert!(!dir.join("bonooru.4.log").exists(), "at most 3 old files");
    let current = fs::read_to_string(dir.join(LOG_FILE)).unwrap();
    let last = current.lines().last().unwrap();
    assert!(
        last.starts_with(&format!("line {:08} ", total - 1)),
        "newest line last in bonooru.log: {last}"
    );
    // Order across files: .1 ends right before bonooru.log begins.
    let one = fs::read_to_string(dir.join("bonooru.1.log")).unwrap();
    let n_of = |l: &str| l[5..13].parse::<usize>().unwrap();
    assert_eq!(
        n_of(one.lines().last().unwrap()) + 1,
        n_of(current.lines().next().unwrap())
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn c22_reopen_appends() {
    let dir = temp_dir("append");
    RotatingFile::open(&dir, MAX_BYTES, KEEP)
        .unwrap()
        .write_line("a")
        .unwrap();
    RotatingFile::open(&dir, MAX_BYTES, KEEP)
        .unwrap()
        .write_line("b")
        .unwrap();
    assert_eq!(fs::read_to_string(dir.join(LOG_FILE)).unwrap(), "a\nb\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn c22_line_format() {
    let t = UNIX_EPOCH + Duration::from_millis(1_795_774_502_123);
    assert_eq!(utc_timestamp(t), "2026-11-27T10:15:02.123Z");
    let line = format_line(t, 4321, "port_opened", "port=/dev/cu.x\nsecond");
    assert_eq!(
        line,
        "2026-11-27T10:15:02.123Z +4321ms port_opened port=/dev/cu.x second"
    );
    // The AC 8 shape, checked by hand (no regex crate).
    let (ts, rest) = line.split_at(24);
    let b = ts.as_bytes();
    for (i, c) in b.iter().enumerate() {
        let want_digit = !matches!(i, 4 | 7 | 10 | 13 | 16 | 19 | 23);
        assert_eq!(c.is_ascii_digit(), want_digit, "byte {i} of {ts}");
    }
    assert_eq!(&ts[19..20], ".");
    assert_eq!(&ts[23..24], "Z");
    let mut f = rest.trim_start().splitn(3, ' ');
    let ms = f.next().unwrap();
    assert!(ms.starts_with('+') && ms.ends_with("ms"));
    assert!(ms[1..ms.len() - 2].bytes().all(|c| c.is_ascii_digit()));
    let kind = f.next().unwrap();
    assert!(kind.bytes().all(|c| c.is_ascii_lowercase() || c == b'_'));
    assert!(!f.next().unwrap().is_empty());
    // Calendar edge cases.
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    assert_eq!(civil_from_days(-1), (1969, 12, 31));
    assert_eq!(utc_timestamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
}

#[test]
fn c22_full_channel_drops_and_counts() {
    assert_eq!(QUEUE, 1024);
    let sink = MemorySink::new();
    let (gate_tx, gate_rx) = crossbeam_channel::bounded::<()>(0);
    let logger = Logger::spawn(Box::new(sink.clone()), false, Some(gate_rx));
    let h = logger.handle();
    for i in 0..1030 {
        h.log("event", format!("n={i}"));
    }
    assert_eq!(h.dropped_total(), 6);
    drop(gate_tx); // resume the log thread
    drop(h);
    logger.stop();
    let lines = sink.lines();
    let written = lines.iter().filter(|l| l.contains(" event n=")).count();
    assert_eq!(written, 1024);
    let dropped: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains(" log_dropped "))
        .collect();
    assert_eq!(dropped.len(), 1, "{dropped:?}");
    assert!(dropped[0].ends_with(" log_dropped 6"), "{}", dropped[0]);
}

#[test]
fn c22_start_line() {
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), true, None);
    logger.handle().log("port_listed", "port=x");
    logger.stop();
    let lines = sink.lines();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains(" +0ms start app="), "{}", lines[0]);
    assert!(lines[0].contains(&format!(" host={} ", std::env::consts::OS)));
    assert!(lines[0].contains(&format!(" arch={} ", std::env::consts::ARCH)));
    assert!(lines[0].ends_with(&format!(" pid={}", std::process::id())));
    assert!(lines[1].contains(" port_listed port=x"));
}

#[test]
fn c22_unwritable_log_reported_once() {
    let logger = Logger::spawn(Box::new(ocb_services::log::FailingSink), false, None);
    let h = logger.handle();
    h.log("event", "a");
    h.log("event", "b");
    let start = std::time::Instant::now();
    let mut got = None;
    while got.is_none() && start.elapsed() < Duration::from_secs(5) {
        got = h.take_unavailable();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(got.as_deref(), Some("disk full"));
    h.log("event", "c");
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(h.take_unavailable(), None, "reported once");
    drop(h);
    logger.stop();
}

#[test]
fn c22_log_dir_per_platform() {
    let d = log_dir().expect("a home directory on the test machine");
    let s = d.to_string_lossy().replace('\\', "/");
    if cfg!(target_os = "macos") {
        assert!(s.ends_with("/Library/Logs/Bonooru"), "{s}");
    } else if cfg!(windows) {
        assert!(s.ends_with("/Bonooru/logs"), "{s}");
        assert!(s.contains("AppData/Local"), "{s}");
    } else {
        assert!(s.ends_with("/bonooru/logs"), "{s}");
    }
}
