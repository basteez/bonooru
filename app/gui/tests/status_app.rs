//! G.16 (Story 4.5 AC 5, 6, 8, 12): the LEDs in the real app on a fake
//! worker thread: unknown before the first snapshot, the new LED one frame
//! after a snapshot, staleness drawn with no new snapshot (and the repaint
//! timer asked for), a dead worker shown as unknown, the `led` lines in the
//! log. Fake clocks for every number; waits on the worker thread are
//! bounded polls (≤ 2 s).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use common::{Scheduler, TestClock, Wake};
use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::app::{App, Mode, WorkerHealth};
use ocb_app::leds::{LedState, Reason};
use ocb_services::log::{LogHandle, Logger, MemorySink};
use ocb_services::sim::{BlockingBackend, FakeBackend, FakePort};
use ocb_services::worker::{FakeClock, RealClock, host_rules, spawn_with};

const CU: &str = "/dev/cu.usbmodem2101";
const LIMIT: Duration = Duration::from_secs(2);

struct Rig {
    h: Harness<'static, App>,
    clock: FakeClock,
    ui: TestClock,
    published: Arc<AtomicU64>,
}

fn rig(ports: Vec<FakePort>, log: Option<LogHandle>, wake: Wake) -> Rig {
    let clock = FakeClock::at(0);
    let mut backend = FakeBackend::new(clock.clone());
    for p in ports {
        backend = backend.with(p);
    }
    let ui = TestClock::default();
    let published = Arc::new(AtomicU64::new(0));
    let (c, u, p) = (clock.clone(), ui.clone(), Arc::clone(&published));
    let mut slot = Some(backend);
    let h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            let handle = common::fake_worker(
                &cc.egui_ctx,
                slot.take().unwrap(),
                c.clone(),
                Arc::clone(&p),
                wake,
            );
            let app = App::with_link(cc, Mode::Main, handle).ui_clock(u.ui_clock());
            match log.clone() {
                Some(l) => app.led_log(l),
                None => app,
            }
        });
    Rig {
        h,
        clock,
        ui,
        published,
    }
}

impl Rig {
    fn names(&self) -> [String; 3] {
        let l = self.h.state().leds();
        l.all().map(|(_, led)| led.name())
    }

    /// The AccessKit names of the three `Role::Status` nodes of the last
    /// frame.
    fn drawn(&self) -> Vec<String> {
        self.h
            .query_all_by_role(Role::Status)
            .map(|n| n.accesskit_node().label().unwrap_or_default())
            .collect()
    }

    /// Moves the fake clock 1 ms per frame until `f` holds (bounded).
    fn advance_until(&mut self, f: impl Fn(&App) -> bool) {
        let start = Instant::now();
        loop {
            self.clock.advance(1_000);
            std::thread::sleep(Duration::from_millis(1));
            self.h.step();
            if f(self.h.state()) {
                return;
            }
            assert!(start.elapsed() < LIMIT, "not reached: {:?}", self.names());
        }
    }

    /// Waits until the worker published nothing for 50 ms of real time
    /// (the fake clock is frozen, so it has nothing to say).
    fn quiet(&self) {
        let start = Instant::now();
        let mut last = self.published.load(Ordering::SeqCst);
        let mut since = Instant::now();
        while since.elapsed() < Duration::from_millis(50) {
            std::thread::sleep(Duration::from_millis(2));
            let now = self.published.load(Ordering::SeqCst);
            if now != last {
                last = now;
                since = Instant::now();
            }
            assert!(start.elapsed() < LIMIT, "worker never quiet");
        }
    }
}

fn all_ok(app: &App) -> bool {
    app.leds()
        .all()
        .iter()
        .all(|(_, l)| l.state == LedState::Ok)
}

#[test]
fn g16_first_frame_unknown() {
    // A worker whose first listing takes 2 s: no snapshot yet.
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(|cc| {
            let handle = spawn_with(
                BlockingBackend {
                    delay: Duration::from_secs(2),
                },
                RealClock::new(),
                host_rules(),
                LogHandle::disabled(),
                Box::new(|| {}),
            )
            .unwrap();
            App::with_link(cc, Mode::Main, handle)
        });
    h.step();
    let leds = h.state().leds();
    for (_, l) in leds.all() {
        assert_eq!((l.state, l.reason), (LedState::Unknown, Reason::Start));
    }
    for name in [
        "Bridge – unknown",
        "Controller – unknown",
        "Switch – unknown",
    ] {
        let _ = h.get_by_role_and_label(Role::Status, name);
    }
    // The status region is first: the first Tab reaches the PC↔Bridge LED.
    h.key_press(egui::Key::Tab);
    h.step();
    assert_eq!(
        h.ctx.memory(|m| m.focused()),
        Some(ocb_app::ui::status_strip::led_id("pc_bridge"))
    );
}

#[test]
fn g16_one_frame_after_snapshot() {
    // The bridge appears at 0.5 s and is listed by the scan at 1 s.
    let bridge = FakePort::bridge(CU, "S", Some("Bonooru Bridge"))
        .silent()
        .present(vec![(500_000, None)]);
    let mut r = rig(vec![bridge], None, Wake::CountOnly);
    // The first scan (nothing listed) arrives.
    let start = Instant::now();
    while r.published.load(Ordering::SeqCst) == 0 {
        assert!(start.elapsed() < LIMIT);
        std::thread::sleep(Duration::from_millis(1));
    }
    r.quiet();
    r.h.step();
    assert!(
        r.drawn().contains(&"Bridge – not found".to_owned()),
        "{:?}",
        r.drawn()
    );
    // The scan at 1 s lists and opens it: one publish later, one frame.
    let before = r.published.load(Ordering::SeqCst);
    r.clock.set(1_000_000);
    let start = Instant::now();
    while r.published.load(Ordering::SeqCst) == before {
        assert!(start.elapsed() < LIMIT, "no publish");
        std::thread::sleep(Duration::from_millis(1));
    }
    r.h.step();
    assert!(
        r.drawn().contains(&"Bridge – found".to_owned()),
        "{:?}",
        r.drawn()
    );
}

#[test]
fn g16_staleness_without_snapshot() {
    // The worker's waker only counts: a wake from the worker thread landing
    // before a frame makes egui report a repaint delay of 0 for that frame
    // (the macOS CI failure, Story 4.5 review), so the delay egui reports
    // here is the app's own request alone.
    let bridge = FakePort::bridge(CU, "S", Some("Bonooru Bridge"));
    let mut r = rig(vec![bridge], None, Wake::CountOnly);
    r.advance_until(all_ok);
    // Freeze the worker's clock: no STATUS, no publish. Two frames: the
    // first drains the last snapshot, the second runs with nothing pending
    // from before it.
    r.quiet();
    r.h.step();
    r.h.step();
    let leds = r.h.state().leds();
    assert_eq!(leds.bridge_controller.reason, Reason::Info);
    let next = leds.next_change_in_us.expect("a staleness timer");
    assert!((100_001..=150_001).contains(&next), "{next}");
    // The frame asked egui to repaint at the deadline: the app's record
    // and egui's own output agree (nothing else asks for a repaint while
    // the worker is quiet).
    assert_eq!(
        r.h.state().last_repaint_after(),
        Some(Duration::from_micros(next))
    );
    let delay = r.h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    assert_eq!(delay, Duration::from_micros(next));
    // One µs before: still fresh.
    r.ui.advance_us(next - 1);
    r.h.step();
    assert!(
        r.drawn().contains(&"Switch – ready".to_owned()),
        "{:?}",
        r.drawn()
    );
    // At the deadline, one frame, no new snapshot: stale.
    r.ui.advance_us(1);
    r.h.step();
    let drawn = r.drawn();
    assert!(
        drawn.contains(&"Controller – not answering".to_owned()),
        "{drawn:?}"
    );
    assert!(drawn.contains(&"Switch – unknown".to_owned()), "{drawn:?}");
    // Stale has no LED timer left; the worker's 3 s stall deadline remains
    // (the UI clock moved `next` since the last snapshot).
    assert_eq!(
        r.h.state().last_repaint_after(),
        Some(Duration::from_secs(3) - Duration::from_micros(next))
    );
}

/// A backend whose first listing waits for `go`, then panics (a bug in
/// the worker's thread at a moment the test chooses).
struct PanicBackend {
    go: Arc<std::sync::atomic::AtomicBool>,
}

impl ocb_services::port::Backend for PanicBackend {
    fn list(&mut self) -> Result<Vec<ocb_services::port::PortInfo>, String> {
        while !self.go.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("worker bug");
    }
    fn open(
        &mut self,
        _name: &str,
    ) -> Result<Box<dyn ocb_services::port::Port>, ocb_services::port::OpenError> {
        Err(ocb_services::port::OpenError::Gone)
    }
}

/// AC 8 + review: a worker that dies while the window is idle wakes the UI
/// itself (its end runs the app's waker after dropping the snapshot
/// sender). Frames run only when egui's own repaint scheduling asks for
/// one, so a missing wake leaves the strip unchanged and the test fails.
#[test]
fn g16_dead_worker_unknown() {
    let go = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let g = Arc::clone(&go);
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            let handle = spawn_with(
                PanicBackend { go: Arc::clone(&g) },
                RealClock::new(),
                host_rules(),
                LogHandle::disabled(),
                ocb_app::app::repaint_waker(&cc.egui_ctx),
            )
            .unwrap();
            App::with_link(cc, Mode::Main, handle)
        });
    let sched = Scheduler::install(&h.ctx);
    h.step();
    // Idle: the worker waits in its listing; nothing is due but the 3 s
    // stall deadline.
    let settle = Instant::now();
    while settle.elapsed() < Duration::from_millis(100) {
        if sched.due() {
            sched.clear();
            h.step();
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(h.state().worker_health(), WorkerHealth::Running);
    assert_eq!(h.state().leds().pc_bridge.reason, Reason::Start);
    go.store(true, Ordering::SeqCst);
    sched.drive_until(&mut h, LIMIT, |h| {
        h.state().worker_health() == WorkerHealth::Ended
            && h.state().leds().pc_bridge.reason == Reason::WorkerEnded
    });
    let leds = h.state().leds();
    for (_, l) in leds.all() {
        assert_eq!(
            (l.state, l.reason),
            (LedState::Unknown, Reason::WorkerEnded)
        );
    }
    let node = h.get_by_role_and_label(Role::Status, "Bridge – unknown");
    assert_eq!(
        node.accesskit_node().description().as_deref(),
        Some(
            "Bonooru stopped checking the USB ports.\nRestart Bonooru. If it happens again, send the file bonooru.log."
        )
    );
    // Ended has no timer: nothing more is asked for.
    assert_eq!(h.state().last_repaint_after(), None);
}

/// Review patch: a silent worker (no snapshot, no wake) is shown as stalled
/// at the 3 s deadline because the app asks for a frame then.
#[test]
fn g16_stall_repaint() {
    let ui = TestClock::default();
    let u = ui.clone();
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
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
            App::with_link(cc, Mode::Main, handle).ui_clock(u.ui_clock())
        });
    h.step();
    h.step();
    assert_eq!(h.state().last_repaint_after(), Some(Duration::from_secs(3)));
    let delay = h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
    assert_eq!(delay, Duration::from_secs(3));
    ui.advance_us(1_000_000);
    h.step();
    assert_eq!(h.state().last_repaint_after(), Some(Duration::from_secs(2)));
    ui.advance_us(2_000_000);
    h.step();
    assert_eq!(h.state().worker_health(), WorkerHealth::Stalled);
    assert_eq!(h.state().leds().pc_bridge.reason, Reason::WorkerStalled);
    let _ = h.get_by_role_and_label(Role::Status, "Bridge – unknown");
}

/// Review decision (D2): every new snapshot asks egui for a repaint, so an
/// idle window drains it at its next frame; the time it waits is not in
/// the LED ages.
#[test]
fn g16_new_snapshot_wakes_an_idle_ui() {
    let bridge = FakePort::bridge(CU, "S", Some("Bonooru Bridge"))
        .silent()
        .present(vec![(500_000, None)]);
    let mut r = rig(vec![bridge], None, Wake::App);
    let sched = Scheduler::install(&r.h.ctx);
    r.h.step();
    let start = Instant::now();
    while r.published.load(Ordering::SeqCst) == 0 {
        assert!(start.elapsed() < LIMIT);
        std::thread::sleep(Duration::from_millis(1));
    }
    r.quiet();
    r.h.step();
    r.h.step();
    assert!(r.drawn().contains(&"Bridge – not found".to_owned()));
    // Idle now: only the 3 s stall deadline is pending (UI clock frozen).
    sched.clear();
    std::thread::sleep(Duration::from_millis(20));
    assert!(!sched.due(), "an idle window has no frame due");
    r.clock.set(1_000_000);
    sched.drive_until(&mut r.h, LIMIT, |h| {
        h.query_by_label("Bridge – found").is_some()
    });
}

#[test]
fn g16_led_lines_logged() {
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let bridge = FakePort::bridge(CU, "S", Some("Bonooru Bridge"));
    let mut r = rig(vec![bridge], Some(logger.handle()), Wake::App);
    r.advance_until(all_ok);
    r.h.step();
    drop(r);
    assert!(logger.stop());
    let texts: Vec<String> = sink
        .lines()
        .iter()
        .filter_map(|l| l.split_once(" led ").map(|(_, t)| t.to_owned()))
        .collect();
    // The first frame logs all three (the worker may have published before
    // it, so not necessarily `start`).
    let first: Vec<&str> = texts
        .iter()
        .take(3)
        .map(|t| t.split('=').next().unwrap())
        .collect();
    assert_eq!(
        first,
        ["pc_bridge", "bridge_controller", "switch"],
        "{texts:#?}"
    );
    for want in [
        "pc_bridge=ok name=\"Bridge v0.0.0\" reason=info",
        "bridge_controller=ok name=\"Controller v0.0.0\" reason=info status_age_us=",
        "switch=ok name=\"Switch – ready\" reason=console_ready",
    ] {
        assert!(
            texts.iter().any(|t| t.starts_with(want)),
            "{want} missing in {texts:#?}"
        );
    }
    // One line per change: no LED is logged twice in a row with the same
    // state and name.
    for tag in ["pc_bridge=", "bridge_controller=", "switch="] {
        let mine: Vec<&String> = texts.iter().filter(|t| t.starts_with(tag)).collect();
        for w in mine.windows(2) {
            // State, name and reason (AC 12): never twice the same.
            let key = |t: &str| t.split(" status_age_us=").next().unwrap().to_owned();
            assert_ne!(key(w[0]), key(w[1]), "{tag} logged twice");
        }
    }
}
