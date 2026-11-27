//! G.13 (Story 4.4 AC 1, 2, 10): the UI never waits on the serial worker,
//! `on_exit` ends it within its bound, and the tests never start the real
//! backend or the real log. Source scans are self-tested.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::vec2;
use egui_kittest::Harness;
use ocb_app::app::{App, Mode, WorkerHealth};
use ocb_services::log::LogHandle;
use ocb_services::sim::{BlockingBackend, FakeBackend, FakePort};
use ocb_services::worker::{FakeClock, RealClock, WorkerEvent, host_rules, spawn_with};

#[test]
fn g13_ui_never_waits() {
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(|cc| {
            let handle = spawn_with(
                BlockingBackend {
                    delay: Duration::from_secs(5),
                },
                RealClock::new(),
                host_rules(),
                LogHandle::disabled(),
                Box::new(|| {}),
            )
            .unwrap();
            App::with_link(cc, Mode::Main, handle)
        });
    let t0 = Instant::now();
    for _ in 0..10 {
        h.run_steps(1);
    }
    let took = t0.elapsed();
    assert!(took < Duration::from_secs(1), "10 frames took {took:?}");
    // The exit never hangs either: the worker sleeps in `list` for 5 s,
    // `on_exit` gives up after its 500 ms bound.
    let t0 = Instant::now();
    eframe::App::on_exit(h.state_mut());
    let took = t0.elapsed();
    assert!(took < Duration::from_millis(800), "on_exit took {took:?}");
}

#[test]
fn g13_on_exit_ends_the_worker_and_closes_the_port() {
    let cu = if cfg!(target_os = "macos") {
        "/dev/cu.usbmodem101"
    } else if cfg!(windows) {
        "COM3"
    } else {
        "/dev/ttyACM0"
    };
    let backend = FakeBackend::new(FakeClock::at(0))
        .with(FakePort::bridge(cu, "S", Some("Bonooru Bridge")).silent());
    backend.state().read_sleep = Some(Duration::from_millis(1));
    let b = backend.clone();
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            let ctx = cc.egui_ctx.clone();
            let handle = spawn_with(
                b,
                RealClock::new(),
                host_rules(),
                LogHandle::disabled(),
                Box::new(move || ctx.request_repaint()),
            )
            .unwrap();
            App::with_link(cc, Mode::Main, handle)
        });
    // Frames until the app has seen the port open.
    let start = Instant::now();
    loop {
        h.run_steps(1);
        let opened = h.state().link_events().is_some_and(|ev| {
            ev.iter()
                .any(|e| matches!(e.event, WorkerEvent::PortOpened { .. }))
        });
        if opened && h.state().link_snapshot().is_some() {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5), "never opened");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(h.state().link_snapshot().is_some());
    assert!(backend.state().dtr_of(cu));
    let t0 = Instant::now();
    eframe::App::on_exit(h.state_mut());
    let took = t0.elapsed();
    assert!(took <= Duration::from_millis(500), "{took:?}");
    let st = backend.state();
    assert_eq!(st.closes.len(), 1, "port closed");
    assert!(!st.dtr_of(cu), "DTR falls");
}

/// A backend whose listing panics (a bug in the worker's thread).
struct PanicBackend;

impl ocb_services::port::Backend for PanicBackend {
    fn list(&mut self) -> Result<Vec<ocb_services::port::PortInfo>, String> {
        panic!("worker bug");
    }
    fn open(
        &mut self,
        _name: &str,
    ) -> Result<Box<dyn ocb_services::port::Port>, ocb_services::port::OpenError> {
        Err(ocb_services::port::OpenError::Gone)
    }
}

fn app_on(backend: impl ocb_services::port::Backend + 'static) -> Harness<'static, App> {
    let mut slot = Some(backend);
    Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            let handle = spawn_with(
                slot.take().unwrap(),
                RealClock::new(),
                host_rules(),
                LogHandle::disabled(),
                Box::new(|| {}),
            )
            .unwrap();
            App::with_link(cc, Mode::Main, handle)
        })
}

#[test]
fn g13_dead_worker_is_visible() {
    let mut h = app_on(PanicBackend);
    let start = Instant::now();
    while h.state().worker_health() != WorkerHealth::Ended {
        assert!(start.elapsed() < Duration::from_secs(5), "never seen ended");
        std::thread::sleep(Duration::from_millis(5));
        h.run_steps(1);
    }
}

#[test]
fn g13_stalled_worker_is_visible() {
    // The listing blocks 5 s: no snapshot for 3 s means stalled.
    let mut h = app_on(BlockingBackend {
        delay: Duration::from_secs(5),
    });
    h.run_steps(1);
    assert_eq!(h.state().worker_health(), WorkerHealth::Running);
    let start = Instant::now();
    while h.state().worker_health() != WorkerHealth::Stalled {
        assert!(
            start.elapsed() < Duration::from_millis(3500),
            "never stalled"
        );
        std::thread::sleep(Duration::from_millis(20));
        h.run_steps(1);
    }
    assert!(start.elapsed() >= Duration::from_millis(2900));
    // A healthy idle worker never looks stalled.
    let mut h = app_on(FakeBackend::new(FakeClock::at(0)));
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(3500) {
        h.run_steps(1);
        assert_eq!(h.state().worker_health(), WorkerHealth::Running);
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn g13_events_kept_bounded() {
    assert_eq!(ocb_app::app::EVENTS_KEPT, 64);
}

// ------------------------------------------------------------ source scans

/// The code of a Rust source with comments removed and string/char
/// literals (raw strings included) replaced by one `§` token, so `f("x")`
/// still has an argument.
fn code_only(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            out.push(' ');
            continue;
        }
        let prev_ident = i > 0 && (b[i - 1].is_alphanumeric() || b[i - 1] == '_');
        if c == 'r' && !prev_ident && matches!(next, Some('"' | '#')) {
            let mut j = i + 1;
            let mut hashes = 0;
            while b.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if b.get(j) == Some(&'"') {
                j += 1;
                while j < b.len()
                    && !(b[j] == '"' && (0..hashes).all(|k| b.get(j + 1 + k) == Some(&'#')))
                {
                    j += 1;
                }
                i = j + 1 + hashes;
                out.push('§');
                continue;
            }
        }
        if c == '"' {
            i += 1;
            while i < b.len() && b[i] != '"' {
                if b[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            out.push('§');
            continue;
        }
        if c == '\'' {
            let close = if next == Some('\\') {
                b.iter()
                    .skip(i + 2)
                    .position(|&x| x == '\'')
                    .map(|p| i + 2 + p)
            } else if b.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            if let Some(end) = close {
                i = end + 1;
                out.push('§');
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn tokens(src: &str) -> Vec<String> {
    let code = code_only(src);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_alphanumeric() || c == '_' {
            cur.push(c);
            continue;
        }
        if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if c == ':' && chars.peek() == Some(&':') {
            chars.next();
            out.push("::".to_owned());
        } else if !c.is_whitespace() {
            out.push(c.to_string());
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn seq(t: &[String], i: usize, want: &[&str]) -> bool {
    want.iter()
        .enumerate()
        .all(|(k, w)| t.get(i + k).map(String::as_str) == Some(*w))
}

/// AC 1 in `app/gui/src`: no `serialport`, no log file opened, no blocking
/// receive or join (`recv()`, `recv_timeout`, `recv_deadline`, `.join()`).
fn ui_hits(src: &str) -> Vec<String> {
    let t = tokens(src);
    let mut out = Vec::new();
    for i in 0..t.len() {
        let tok = t[i].as_str();
        if matches!(
            tok,
            "serialport" | "recv_timeout" | "recv_deadline" | "OpenOptions" | "File" | "fs"
        ) {
            out.push(tok.to_owned());
        }
        if seq(&t, i, &[".", "recv", "(", ")"]) {
            out.push(".recv()".to_owned());
        }
        if seq(&t, i, &[".", "join", "(", ")"]) {
            out.push(".join()".to_owned());
        }
    }
    out
}

/// Test isolation in `app/gui/tests`: no real worker (`App::new(`,
/// `App::with_smoke(`), no real backend or log.
fn test_hits(src: &str) -> Vec<String> {
    let t = tokens(src);
    let mut out = Vec::new();
    for i in 0..t.len() {
        if seq(&t, i, &["App", "::", "new", "("]) {
            out.push("App::new(".to_owned());
        }
        if seq(&t, i, &["App", "::", "with_smoke", "("]) {
            out.push("App::with_smoke(".to_owned());
        }
        if t[i] == "SerialBackend" {
            out.push("SerialBackend".to_owned());
        }
        if seq(&t, i, &["Logger", "::", "start", "("]) {
            out.push("Logger::start(".to_owned());
        }
        if seq(&t, i, &["worker", "::", "spawn", "("]) {
            out.push("worker::spawn(".to_owned());
        }
    }
    out
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn scan_dir(dir: &str, f: fn(&str) -> Vec<String>) -> Vec<String> {
    let mut files = Vec::new();
    rs_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join(dir), &mut files);
    assert!(files.len() >= 5, "{dir}: {} files", files.len());
    let mut problems = Vec::new();
    for p in files {
        for h in f(&fs::read_to_string(&p).unwrap()) {
            problems.push(format!("{}: {h}", p.display()));
        }
    }
    problems
}

#[test]
fn g13_scanners_self_test() {
    let good_ui = r#"
        // let x = rx.recv(); serialport::new in a comment
        let s = "recv() File::open fs";
        while let Ok(e) = rx.try_recv() {}
        let j = names.join(", "); fn join_reach() {}
    "#;
    assert_eq!(ui_hits(good_ui), Vec::<String>::new());
    for (bad, want) in [
        ("let e = self.rx.recv();", ".recv()"),
        ("rx.recv_timeout(d)", "recv_timeout"),
        ("let _ = handle.join();", ".join()"),
        ("serialport::available_ports()", "serialport"),
        ("std::fs::OpenOptions::new()", "fs"),
        ("File::create(p)", "File"),
    ] {
        assert!(ui_hits(bad).contains(&want.to_owned()), "{bad}");
    }
    let good_tests = r#"
        // App::new(cc, Mode::Main) is banned here
        let s = "SerialBackend App::with_smoke(";
        App::with_link(cc, Mode::Main, h).smoke_mode(s);
    "#;
    assert_eq!(test_hits(good_tests), Vec::<String>::new());
    for (bad, want) in [
        ("App::new(cc, Mode::Main)", "App::new("),
        ("App :: new (cc, m)", "App::new("),
        ("App::with_smoke(cc, m, s)", "App::with_smoke("),
        ("let b = SerialBackend::new();", "SerialBackend"),
        ("let l = Logger::start();", "Logger::start("),
        ("worker::spawn(b, l, w)", "worker::spawn("),
    ] {
        assert_eq!(test_hits(bad), vec![want.to_owned()], "{bad}");
    }
}

#[test]
fn g13_ui_code_never_blocks() {
    let problems = scan_dir("src", ui_hits);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn g13_tests_never_start_the_real_worker() {
    let problems = scan_dir("tests", test_hits);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
