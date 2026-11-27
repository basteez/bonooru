//! C.24: dry runs of `ocb-probe` (Story 4.4 AC 9, E3-A2). The runner of
//! the bench, unchanged, drives the `ocb-services` worker stepped on a fake
//! clock with `FakeBackend` (fake controllers from `ocb-sim`); a scripted
//! owner reacts to the `PROMPT:` lines. One test per verdict branch, each
//! asserting the exact `OCB_PROBE` line and the exit code.
//!
//! `cargo test -p ocb-probe -- --nocapture run_pass` prints every line the
//! owner will see on the bench (Task 12).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;

use ocb_core::device::Rules;
use ocb_probe::runner::{
    self, Driver, EXIT_FAIL, EXIT_INTERRUPTED, EXIT_PASS, PROMPT_PLUG, PROMPT_UNPLUG, list_lines,
};
use ocb_services::log::{LOG_FILE, LogHandle, Logger, MemorySink};
use ocb_services::port::{OpenError, RawListing};
use ocb_services::sim::{Device, FakeBackend, FakePort, FakeState};
use ocb_services::worker::{
    Clock as _, Endpoints, FakeClock, LinkSnapshot, TimedEvent, WorkerCommand, WorkerCore,
};
use ocb_sim::ControllerConfig;

const S: &str = "53032847189C2E9C";
const CU: &str = "/dev/cu.usbmodem101";
const CU2: &str = "/dev/cu.usbmodem1101";
const ACM: &str = "/dev/ttyACM0";

/// A change to the fake world at a time after a prompt.
type Action = Box<dyn FnOnce(&mut FakeState, u64)>;
/// The owner: what to do (and when, µs after the prompt) on a prompt line.
type OnPrompt = Box<dyn FnMut(&str) -> Vec<(u64, Action)>>;

struct SimDriver {
    core: Option<WorkerCore<FakeBackend, FakeClock>>,
    ends: Endpoints,
    clock: FakeClock,
    backend: FakeBackend,
    logger: Option<Logger>,
    sink: Option<MemorySink>,
    snapshot: Option<Arc<LinkSnapshot>>,
    on_prompt: OnPrompt,
    pending: Vec<(u64, Action)>,
    interrupt_at: Option<u64>,
    interrupted: bool,
    host: &'static str,
    lines: Vec<String>,
    finished: Option<(Result<Vec<String>, String>, String)>,
}

impl SimDriver {
    fn new(backend: FakeBackend, rules: Rules, host: &'static str) -> Self {
        let sink = MemorySink::new();
        let logger = Logger::spawn(Box::new(sink.clone()), true, None);
        Self::with_log(backend, rules, host, Some((logger, sink)))
    }

    fn with_log(
        backend: FakeBackend,
        rules: Rules,
        host: &'static str,
        log: Option<(Logger, MemorySink)>,
    ) -> Self {
        let clock = backend.state().clock.clone();
        let handle = log
            .as_ref()
            .map_or_else(LogHandle::disabled, |(l, _)| l.handle());
        let (core, ends) = WorkerCore::new(
            backend.clone(),
            clock.clone(),
            rules,
            handle,
            Box::new(|| {}),
        );
        let (logger, sink) = log.map_or((None, None), |(l, s)| (Some(l), Some(s)));
        Self {
            core: Some(core),
            ends,
            clock,
            backend,
            logger,
            sink,
            snapshot: None,
            on_prompt: Box::new(|_| Vec::new()),
            pending: Vec::new(),
            interrupt_at: None,
            interrupted: false,
            host,
            lines: Vec::new(),
            finished: None,
        }
    }

    fn owner(mut self, f: OnPrompt) -> Self {
        self.on_prompt = f;
        self
    }

    fn step(&mut self) {
        let now = self.clock.now_us();
        if self.interrupt_at.is_some_and(|t| now >= t) {
            self.interrupted = true;
        }
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 <= now {
                let (_, a) = self.pending.remove(i);
                a(&mut self.backend.state(), now);
            } else {
                i += 1;
            }
        }
        if let Some(core) = self.core.as_mut() {
            core.step();
        }
        if let Ok(s) = self.ends.snapshots.try_recv() {
            self.snapshot = Some(s);
        }
        self.clock.advance(1_000);
    }

    fn probe_line(&self) -> &str {
        self.lines
            .iter()
            .find(|l| l.starts_with("OCB_PROBE "))
            .map_or("", String::as_str)
    }
}

impl Driver for SimDriver {
    fn next(&mut self, timeout_us: u64) -> Option<TimedEvent> {
        let deadline = self.clock.now_us() + timeout_us;
        loop {
            if let Ok(e) = self.ends.events.try_recv() {
                return Some(e);
            }
            if self.interrupted || self.clock.now_us() >= deadline {
                return None;
            }
            self.step();
        }
    }

    fn now_us(&self) -> u64 {
        self.clock.now_us()
    }

    fn snapshot(&mut self) -> Option<Arc<LinkSnapshot>> {
        self.snapshot.clone()
    }

    fn interrupted(&self) -> bool {
        self.interrupted
    }

    fn host(&self) -> &'static str {
        self.host
    }

    fn say(&mut self, line: &str) {
        println!("{line}");
        if line.starts_with("PROMPT:") {
            let now = self.clock.now_us();
            for (after, a) in (self.on_prompt)(line) {
                self.pending.push((now + after, a));
            }
        }
        self.lines.push(line.to_owned());
    }

    fn finish(&mut self) -> (Result<Vec<String>, String>, String) {
        if let Some(f) = &self.finished {
            return f.clone();
        }
        let _ = self.ends.commands.send(WorkerCommand::Shutdown);
        if let Some(mut core) = self.core.take() {
            core.step();
        }
        if let Some(l) = self.logger.take() {
            l.stop();
        }
        let lines = Ok(self
            .sink
            .as_ref()
            .map(MemorySink::lines)
            .unwrap_or_default());
        let path = format!("<sim>/{LOG_FILE}");
        self.finished = Some((lines, path));
        self.finished.clone().unwrap()
    }
}

fn backend(ports: Vec<FakePort>) -> FakeBackend {
    let mut b = FakeBackend::new(FakeClock::at(0));
    for p in ports {
        b = b.with(p);
    }
    b
}

fn ends_at(name: &'static str) -> Action {
    Box::new(move |st, now| {
        let p = st.ports.iter_mut().find(|p| p.info.name == name).unwrap();
        p.present = vec![(0, Some(now))];
    })
}

fn appears(name: &'static str) -> Action {
    Box::new(move |st, now| {
        let p = st.ports.iter_mut().find(|p| p.info.name == name).unwrap();
        p.present.push((now, None));
    })
}

/// The owner of H.29 on a Mac: unplugs 1.5 s after the prompt, plugs into
/// another socket (`CU2`) 2 s after the second prompt.
fn mac_owner() -> OnPrompt {
    Box::new(|line| {
        if line == PROMPT_UNPLUG {
            vec![(1_500_000, ends_at(CU))]
        } else if line == PROMPT_PLUG {
            vec![(2_000_000, appears(CU2))]
        } else {
            Vec::new()
        }
    })
}

fn mac_ports() -> Vec<FakePort> {
    vec![
        FakePort::bridge(CU, S, Some("Bonooru Bridge")),
        FakePort::bridge(CU2, S, Some("Bonooru Bridge")).present(vec![]),
    ]
}

// ------------------------------------------------------------------ run

#[test]
fn run_pass() {
    let mut d = SimDriver::new(backend(mac_ports()), Rules::MACOS, "macos").owner(mac_owner());
    let code = runner::run(&mut d, 120);
    assert_eq!(code, EXIT_PASS, "{}", d.lines.join("\n"));
    assert_eq!(
        d.probe_line(),
        "OCB_PROBE PASS host=macos port=/dev/cu.usbmodem1101 serial=53032847189C2E9C renamed=yes info_after_open_ms=2 lost_after_last_rx_ms=2 info_after_listed_ms=2 scan_p50_ms=1000 scan_max_ms=1000 enum_max_ms=0 fw=0.0.0 bridge=0.0.0 proto=1"
    );
    for want in [
        "STEP connect PASS port=/dev/cu.usbmodem101 serial=53032847189C2E9C info_after_open_ms=2",
        PROMPT_UNPLUG,
        "STEP unplug PASS lost_after_last_rx_ms=2",
        PROMPT_PLUG,
        "STEP replug PASS port=/dev/cu.usbmodem1101 renamed=yes info_after_listed_ms=2",
        "STEP log PASS",
    ] {
        assert!(d.lines.iter().any(|l| l == want), "missing {want}");
    }
    assert_eq!(d.lines.last().unwrap(), "LOG <sim>/bonooru.log");
    // The lines come in the documented order.
    let pos = |p: &str| d.lines.iter().position(|l| l.starts_with(p)).unwrap();
    assert!(pos("STEP connect") < pos("PROMPT: UNPLUG"));
    assert!(pos("PROMPT: UNPLUG") < pos("STEP unplug"));
    assert!(pos("STEP unplug") < pos("PROMPT: PLUG"));
    assert!(pos("PROMPT: PLUG") < pos("STEP replug"));
    assert!(pos("STEP replug") < pos("STEP log"));
    assert!(pos("STEP log") < pos("OCB_PROBE"));
    assert!(
        d.lines
            .iter()
            .any(|l| l.starts_with("EVT +") && l.contains(" renamed from="))
    );
}

#[test]
fn run_pass_windows_same_name() {
    // Windows keeps COM5 for the same device; the product is not compared.
    let mut d = SimDriver::new(
        backend(vec![FakePort::bridge("COM5", S, None)]),
        Rules::WINDOWS,
        "windows",
    )
    .owner(Box::new(|line| {
        if line == PROMPT_UNPLUG {
            vec![(1_000_000, ends_at("COM5"))]
        } else if line == PROMPT_PLUG {
            vec![(3_000_000, appears("COM5"))]
        } else {
            Vec::new()
        }
    }));
    assert_eq!(
        runner::run(&mut d, 120),
        EXIT_PASS,
        "{}",
        d.lines.join("\n")
    );
    assert!(
        d.probe_line().starts_with(
            "OCB_PROBE PASS host=windows port=COM5 serial=53032847189C2E9C renamed=no "
        ),
        "{}",
        d.probe_line()
    );
}

#[test]
fn run_fail_no_bridge() {
    let mut d = SimDriver::new(backend(vec![]), Rules::MACOS, "macos");
    assert_eq!(runner::run(&mut d, 3), EXIT_FAIL);
    assert!(
        d.clock.now_us() <= 3_001_000,
        "gave up at {}",
        d.clock.now_us()
    );
    assert_eq!(
        d.probe_line(),
        "OCB_PROBE FAIL (no bridge within 3 s) host=macos port=- serial=- renamed=no info_after_open_ms=- lost_after_last_rx_ms=- info_after_listed_ms=- scan_p50_ms=1000 scan_max_ms=1000 enum_max_ms=0 fw=- bridge=- proto=-"
    );
}

#[test]
fn run_fail_only_foreign() {
    let mut d = SimDriver::new(
        backend(vec![FakePort::bridge(ACM, "F", Some("2wiCC COM"))]),
        Rules::LINUX,
        "linux",
    );
    assert_eq!(runner::run(&mut d, 3), EXIT_FAIL);
    assert!(
        d.probe_line()
            .starts_with("OCB_PROBE FAIL (only foreign boards: 2wiCC COM) host=linux port=- "),
        "{}",
        d.probe_line()
    );
    assert!(d.backend.state().opens.is_empty());
}

#[test]
fn run_fail_permission_denied() {
    let mut d = SimDriver::new(
        backend(vec![
            FakePort::bridge(ACM, S, Some("Bonooru Bridge"))
                .always_fail(OpenError::PermissionDenied),
        ]),
        Rules::LINUX,
        "linux",
    );
    assert_eq!(runner::run(&mut d, 120), EXIT_FAIL);
    assert!(
        d.probe_line()
            .starts_with("OCB_PROBE FAIL (permission denied on /dev/ttyACM0) host=linux "),
        "{}",
        d.probe_line()
    );
}

#[test]
fn run_fail_port_busy() {
    let mut d = SimDriver::new(
        backend(vec![
            FakePort::bridge(CU, S, Some("Bonooru Bridge")).always_fail(OpenError::Busy),
        ]),
        Rules::MACOS,
        "macos",
    );
    assert_eq!(runner::run(&mut d, 120), EXIT_FAIL);
    assert!(
        d.probe_line()
            .starts_with("OCB_PROBE FAIL (port busy: /dev/cu.usbmodem101) host=macos "),
        "{}",
        d.probe_line()
    );
    assert!(d.clock.now_us() >= 5_000_000 && d.clock.now_us() < 6_000_000);
}

#[test]
fn run_fail_no_info() {
    let mut d = SimDriver::new(
        backend(vec![
            FakePort::bridge(CU, S, Some("Bonooru Bridge")).silent(),
        ]),
        Rules::MACOS,
        "macos",
    );
    assert_eq!(runner::run(&mut d, 120), EXIT_FAIL);
    // Opened at 0, given up 5 s later.
    assert!(
        d.clock.now_us() <= 5_001_000,
        "gave up at {}",
        d.clock.now_us()
    );
    assert!(
        d.probe_line().starts_with(
            "OCB_PROBE FAIL (no INFO within 5 s of opening /dev/cu.usbmodem101) host=macos port=/dev/cu.usbmodem101 serial=53032847189C2E9C "
        ),
        "{}",
        d.probe_line()
    );
}

#[test]
fn run_fail_protocol_mismatch() {
    let cfg = ControllerConfig {
        proto_ver: 2,
        ..ControllerConfig::default()
    };
    let mut d = SimDriver::new(
        backend(vec![
            FakePort::bridge(CU, S, Some("Bonooru Bridge")).controller(cfg),
        ]),
        Rules::MACOS,
        "macos",
    );
    assert_eq!(runner::run(&mut d, 120), EXIT_FAIL);
    assert!(
        d.probe_line()
            .starts_with("OCB_PROBE FAIL (protocol mismatch: controller 2, app 1) host=macos "),
        "{}",
        d.probe_line()
    );
    assert!(d.probe_line().ends_with(" proto=2"), "{}", d.probe_line());
}

#[test]
fn run_fail_unplug_not_seen() {
    let mut d = SimDriver::new(backend(mac_ports()), Rules::MACOS, "macos");
    assert_eq!(runner::run(&mut d, 3), EXIT_FAIL);
    // Connected within ms, then 3 s for the unplug.
    assert!(
        d.clock.now_us() <= 3_100_000,
        "gave up at {}",
        d.clock.now_us()
    );
    assert!(d.lines.iter().any(|l| l == "STEP unplug FAIL timeout_s=3"));
    assert!(
        d.probe_line()
            .starts_with("OCB_PROBE FAIL (unplug not seen within 3 s) host=macos "),
        "{}",
        d.probe_line()
    );
}

#[test]
fn run_fail_loss_late() {
    // The controller goes quiet at the prompt; the port only leaves the
    // list 1.6 s later (no read error): the loss comes > 1500 ms after
    // the last byte.
    let mut d = SimDriver::new(
        backend(vec![
            FakePort::bridge(CU, S, Some("Bonooru Bridge")).silent_unplug(),
        ]),
        Rules::MACOS,
        "macos",
    )
    .owner(Box::new(|line| {
        if line == PROMPT_UNPLUG {
            vec![
                (
                    0,
                    Box::new(|st: &mut FakeState, _| st.ports[0].device = Device::Silent) as Action,
                ),
                (1_600_000, ends_at(CU)),
            ]
        } else {
            Vec::new()
        }
    }));
    assert_eq!(runner::run(&mut d, 120), EXIT_FAIL);
    let line = d.probe_line().to_owned();
    assert!(
        line.starts_with("OCB_PROBE FAIL (loss detected ")
            && line.contains(" ms after the last byte (limit 1500)) host=macos "),
        "{line}"
    );
    assert!(
        d.lines
            .iter()
            .any(|l| l.starts_with("STEP unplug FAIL lost_after_last_rx_ms="))
    );
}

#[test]
fn run_fail_replug_not_seen() {
    let mut d =
        SimDriver::new(backend(mac_ports()), Rules::MACOS, "macos").owner(Box::new(|line| {
            if line == PROMPT_UNPLUG {
                vec![(1_000_000, ends_at(CU))]
            } else {
                Vec::new()
            }
        }));
    assert_eq!(runner::run(&mut d, 5), EXIT_FAIL);
    // Unplugged 1 s after the prompt, then 5 s for the replug.
    assert!(
        d.clock.now_us() <= 6_200_000,
        "gave up at {}",
        d.clock.now_us()
    );
    assert!(d.lines.iter().any(|l| l == "STEP replug FAIL timeout_s=5"));
    assert!(
        d.probe_line()
            .starts_with("OCB_PROBE FAIL (replug not seen within 5 s) host=macos "),
        "{}",
        d.probe_line()
    );
}

#[test]
fn run_fail_info_late_after_listed() {
    // Back in, but the controller answers only 2.5 s later (still booting).
    let mut d =
        SimDriver::new(backend(mac_ports()), Rules::MACOS, "macos").owner(Box::new(|line| {
            if line == PROMPT_UNPLUG {
                vec![(1_000_000, ends_at(CU))]
            } else if line == PROMPT_PLUG {
                vec![
                    (
                        0,
                        Box::new(|st: &mut FakeState, _| st.ports[1].device = Device::Silent)
                            as Action,
                    ),
                    (1_000_000, appears(CU2)),
                    (
                        3_500_000,
                        Box::new(|st: &mut FakeState, _| {
                            st.ports[1].device = Device::Controller(Box::new(
                                ocb_sim::FakeController::new(ControllerConfig::default()),
                            ));
                        }) as Action,
                    ),
                ]
            } else {
                Vec::new()
            }
        }));
    assert_eq!(runner::run(&mut d, 120), EXIT_FAIL);
    let line = d.probe_line().to_owned();
    assert!(
        line.starts_with("OCB_PROBE FAIL (INFO ")
            && line.contains(" ms after the port was listed (limit 1500)) host=macos "),
        "{line}"
    );
}

#[test]
fn run_fail_rescan_period() {
    // One enumeration stalls 1.6 s (right after the plug prompt): one scan
    // period over the 1500 ms limit.
    let b = backend(mac_ports());
    let mut d = SimDriver::new(b, Rules::MACOS, "macos").owner(Box::new(|line| {
        if line == PROMPT_UNPLUG {
            vec![(1_500_000, ends_at(CU))]
        } else if line == PROMPT_PLUG {
            vec![
                (
                    0,
                    Box::new(|st: &mut FakeState, _| st.list_stalls.push_back(1_600_000)) as Action,
                ),
                (2_000_000, appears(CU2)),
            ]
        } else {
            Vec::new()
        }
    }));
    assert_eq!(
        runner::run(&mut d, 120),
        EXIT_FAIL,
        "{}",
        d.lines.join("\n")
    );
    assert!(
        d.probe_line().starts_with(
            "OCB_PROBE FAIL (rescan period p50 1000 ms / max 1601 ms (limits 900–1100 / 1500)) host=macos "
        ),
        "{}",
        d.probe_line()
    );
    assert!(d.probe_line().contains(" enum_max_ms=1600 "));
    assert!(d.lines.iter().any(|l| l.starts_with("STEP replug PASS ")));
}

#[test]
fn run_fail_log_check() {
    // The worker runs without a log: nothing to find.
    let mut d =
        SimDriver::with_log(backend(mac_ports()), Rules::MACOS, "macos", None).owner(mac_owner());
    assert_eq!(runner::run(&mut d, 120), EXIT_FAIL);
    assert!(
        d.probe_line().starts_with(
            "OCB_PROBE FAIL (log check: version 0 < 1, port_opened 0 < 2, port_lost 0 < 1) host=macos "
        ),
        "{}",
        d.probe_line()
    );
    assert!(d.lines.iter().any(|l| l.starts_with("STEP log FAIL ")));
}

#[test]
fn run_interrupted() {
    let mut d = SimDriver::new(backend(mac_ports()), Rules::MACOS, "macos");
    d.interrupt_at = Some(2_000_000);
    assert_eq!(runner::run(&mut d, 120), EXIT_INTERRUPTED);
    assert_eq!(d.probe_line(), "OCB_PROBE INTERRUPTED step=unplug");
    assert_eq!(d.backend.state().closes.len(), 1, "the port is closed");
}

// ------------------------------------------------------------ permission

#[test]
fn permission_pass() {
    let mut d = SimDriver::new(
        backend(vec![
            FakePort::bridge(ACM, S, Some("Bonooru Bridge"))
                .always_fail(OpenError::PermissionDenied)
                .group("dialout"),
        ]),
        Rules::LINUX,
        "linux",
    );
    assert_eq!(runner::permission(&mut d, 10), EXIT_PASS);
    assert_eq!(
        d.probe_line(),
        "OCB_PROBE PASS permission port=/dev/ttyACM0 group=dialout after_listed_ms=0"
    );
    assert_eq!(
        d.lines.last().unwrap(),
        "MSG Bonooru can't open /dev/ttyACM0: your user is not in the dialout group. Run: sudo usermod -aG dialout $USER — then log out and back in."
    );
}

#[test]
fn permission_fail_port_opened() {
    let mut d = SimDriver::new(
        backend(vec![FakePort::bridge(ACM, S, Some("Bonooru Bridge"))]),
        Rules::LINUX,
        "linux",
    );
    assert_eq!(runner::permission(&mut d, 10), EXIT_FAIL);
    assert_eq!(
        d.probe_line(),
        "OCB_PROBE FAIL (port opened: the user can already open /dev/ttyACM0)"
    );
}

#[test]
fn permission_fail_no_event() {
    let mut d = SimDriver::new(backend(vec![]), Rules::LINUX, "linux");
    assert_eq!(runner::permission(&mut d, 2), EXIT_FAIL);
    assert!(
        d.clock.now_us() <= 2_001_000,
        "gave up at {}",
        d.clock.now_us()
    );
    assert_eq!(
        d.probe_line(),
        "OCB_PROBE FAIL (no permission event within 2 s)"
    );
}

#[test]
fn permission_fail_late() {
    // Five transient failures first (250 ms apart), then the denial.
    let mut d = SimDriver::new(
        backend(vec![
            FakePort::bridge(ACM, S, Some("Bonooru Bridge"))
                .fail_opens(vec![OpenError::Gone; 5])
                .always_fail(OpenError::PermissionDenied),
        ]),
        Rules::LINUX,
        "linux",
    );
    assert_eq!(runner::permission(&mut d, 10), EXIT_FAIL);
    assert_eq!(
        d.probe_line(),
        "OCB_PROBE FAIL (permission event 1250 ms after the port was listed (limit 1000))"
    );
}

// ------------------------------------------------------------------ alone

#[test]
fn alone_pass() {
    let mut d = SimDriver::new(
        backend(vec![
            FakePort::bridge(CU, S, Some("Bonooru Bridge")).silent(),
        ]),
        Rules::MACOS,
        "macos",
    );
    assert_eq!(runner::alone(&mut d, 10), EXIT_PASS);
    assert_eq!(
        d.probe_line(),
        "OCB_PROBE PASS alone port=/dev/cu.usbmodem101 class=ours hello_frames=100 seconds=10 other_ports_written=0"
    );
}

#[test]
fn alone_fail_info() {
    let mut d = SimDriver::new(
        backend(vec![FakePort::bridge(CU, S, Some("Bonooru Bridge"))]),
        Rules::MACOS,
        "macos",
    );
    assert_eq!(runner::alone(&mut d, 10), EXIT_FAIL);
    assert_eq!(
        d.probe_line(),
        "OCB_PROBE FAIL (INFO received: the controller is powered)"
    );
}

#[test]
fn alone_fail_no_bridge() {
    let mut d = SimDriver::new(backend(vec![]), Rules::MACOS, "macos");
    assert_eq!(runner::alone(&mut d, 10), EXIT_FAIL);
    assert!(
        d.clock.now_us() <= 10_001_000,
        "gave up at {}",
        d.clock.now_us()
    );
    assert_eq!(d.probe_line(), "OCB_PROBE FAIL (no bridge port)");
}

#[test]
fn alone_fail_other_port() {
    // Windows, two unconfirmed ports and no answer: after 2 s the worker
    // tries the other one.
    let mut d = SimDriver::new(
        backend(vec![
            FakePort::bridge("COM3", S, None).silent(),
            FakePort::bridge("COM9", "B", None).silent(),
        ]),
        Rules::WINDOWS,
        "windows",
    );
    assert_eq!(runner::alone(&mut d, 10), EXIT_FAIL);
    assert_eq!(
        d.probe_line(),
        "OCB_PROBE FAIL (wrote to another port: COM9)"
    );
}

#[test]
fn alone_fail_few_hellos() {
    // The OS queue stays full: every chunk is dropped, no HELLO goes out.
    let mut p = FakePort::bridge(CU, S, Some("Bonooru Bridge")).silent();
    p.queued = 8192;
    let mut d = SimDriver::new(backend(vec![p]), Rules::MACOS, "macos");
    assert_eq!(runner::alone(&mut d, 10), EXIT_FAIL);
    assert_eq!(
        d.probe_line(),
        "OCB_PROBE FAIL (only 0 HELLO frames in 10 s (expected ≥ 9 per s))"
    );
}

#[test]
fn alone_interrupted() {
    let mut d = SimDriver::new(
        backend(vec![
            FakePort::bridge(CU, S, Some("Bonooru Bridge")).silent(),
        ]),
        Rules::MACOS,
        "macos",
    );
    d.interrupt_at = Some(3_000_000);
    assert_eq!(runner::alone(&mut d, 10), EXIT_INTERRUPTED);
    assert_eq!(d.probe_line(), "OCB_PROBE INTERRUPTED step=alone");
}

// ------------------------------------------------------------------- list

fn listing(name: &str, vid: u16, pid: u16, product: Option<&str>, raw: Option<&str>) -> RawListing {
    RawListing {
        info: FakePort::usb(name, vid, pid, Some(S), product).info,
        product_raw: raw.map(str::to_owned),
    }
}

#[test]
fn list_ok() {
    let (lines, code) = list_lines(
        Ok(vec![
            listing(CU, 0x2E8A, 0x000A, Some("Bonooru Bridge"), None),
            listing(
                "/dev/tty.usbmodem101",
                0x2E8A,
                0x000A,
                Some("Bonooru Bridge"),
                None,
            ),
            listing(
                "/dev/cu.usbmodem201",
                0x2E8A,
                0x000A,
                Some("2wiCC COM"),
                None,
            ),
            RawListing {
                info: FakePort::plain("/dev/cu.Bluetooth-Incoming-Port").info,
                product_raw: None,
            },
        ]),
        Rules::MACOS,
        "macos",
    );
    assert_eq!(code, EXIT_PASS);
    assert_eq!(
        lines,
        [
            "PORT name=/dev/cu.usbmodem101 usb=2E8A:000A serial=53032847189C2E9C manufacturer=\"Bonooru\" product=\"Bonooru Bridge\" class=ours",
            "PORT name=/dev/tty.usbmodem101 usb=2E8A:000A serial=53032847189C2E9C manufacturer=\"Bonooru\" product=\"Bonooru Bridge\" class=ignored",
            "PORT name=/dev/cu.usbmodem201 usb=2E8A:000A serial=53032847189C2E9C manufacturer=\"Bonooru\" product=\"2wiCC COM\" class=foreign",
            "PORT name=/dev/cu.Bluetooth-Incoming-Port usb=- class=ignored",
            "OCB_PORTS n=4 ours=1 unconfirmed=0 foreign=1 host=macos",
        ]
    );
    // Windows: the friendly name is printed raw, never compared.
    let (lines, _) = list_lines(
        Ok(vec![listing(
            "COM5",
            0x2E8A,
            0x000A,
            None,
            Some("USB Serial Device (COM5)"),
        )]),
        Rules::WINDOWS,
        "windows",
    );
    assert_eq!(
        lines[0],
        "PORT name=COM5 usb=2E8A:000A serial=53032847189C2E9C manufacturer=\"Bonooru\" product=\"-\" product_raw=\"USB Serial Device (COM5)\" class=unconfirmed"
    );
    assert_eq!(
        lines[1],
        "OCB_PORTS n=1 ours=0 unconfirmed=1 foreign=0 host=windows"
    );
}

#[test]
fn list_error() {
    let (lines, code) = list_lines(Err("libudev: no context".to_owned()), Rules::LINUX, "linux");
    assert_eq!(code, EXIT_FAIL);
    assert_eq!(lines, ["OCB_PORTS error libudev: no context"]);
}
