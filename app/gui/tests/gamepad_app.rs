//! G.23 (Story 4.8 AC 4–10, 14–17, 19): the virtual gamepad in the real app
//! on a fake worker thread (an `ocb-sim` controller behind a fake bridge):
//! press = press, a click is a tap of at least 50 ms, a hold lasts, drag-out
//! keeps the hold, sticks follow and spring back, one `SetState` per change
//! and a resend after a failed `try_send`, the release rules (blur, STOP,
//! the gate turning blocked, the controller leaving DIRECT), clicks ignored
//! while gated, AccessKit taps, every control's bit, Esc winning over a
//! press in the same frame, the `pad` lines, and the H.35 dry run. The UI
//! clock is a `TestClock` and the worker's a `FakeClock`, moved together;
//! waits on the worker thread are bounded polls (≤ 2 s). Never the real
//! backend or the real log (G.13).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};

use common::{Pat, TestClock, Wake};
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::app::{App, Mode};
use ocb_app::gamepad::{Control, Stick, TAP_MIN_US};
use ocb_app::gating::{HINT_NO_BRIDGE, HINT_SWITCH_NOT_READY};
use ocb_app::ui::bottom_bar;
use ocb_core::link::Phase;
use ocb_core::protocol::consts::{
    BTN_A, BTN_B, BTN_CAPTURE, BTN_HOME, BTN_L, BTN_MINUS, BTN_PLUS, BTN_R, BTN_X, BTN_Y, BTN_ZL,
    BTN_ZR, HAT_DOWN, HAT_LEFT, HAT_NEUTRAL, HAT_RIGHT, HAT_UP, PROTO_VERSION,
};
use ocb_core::protocol::{Mode as CtlMode, NEUTRAL, State};
use ocb_services::log::{LogHandle, Logger, MemorySink};
use ocb_services::sim::{Device, FakeBackend, FakePort};
use ocb_services::worker::{Clock as _, FakeClock};
use ocb_sim::{ControllerConfig, FakeController};

const CU: &str = "/dev/cu.usbmodem2101";
const SERIAL: &str = "53032847189C2E9C";
const LIMIT: Duration = Duration::from_secs(2);
/// UI and worker time per frame in the tests (about one 60 Hz frame).
const FRAME_US: u64 = 16_000;

fn bridge() -> FakePort {
    FakePort::bridge(CU, SERIAL, Some("Bonooru Bridge"))
}

const LOG_NAMES: [&str; 16] = [
    "a", "b", "x", "y", "dup", "ddown", "dleft", "dright", "l", "r", "zl", "zr", "minus", "plus",
    "home", "capture",
];
/// `pad ignored` reasons: the six gate reasons and `busy`.
const IGNORED_REASONS: [&str; 7] = [
    "worker",
    "no_bridge",
    "bridge_not_ready",
    "firmware_update",
    "controller_link",
    "switch_not_ready",
    "busy",
];

/// An app `pad` line (its text after `pad `) against its pattern (Dev Notes
/// › Log lines).
fn is_app_pad_line(text: &str) -> bool {
    let ignored_controls: Vec<&str> = LOG_NAMES
        .iter()
        .copied()
        .chain(["stick_left", "stick_right"])
        .collect();
    let ignored_controls: &'static [&'static str] = Box::leak(ignored_controls.into_boxed_slice());
    common::line_matches(
        text,
        "press",
        &[
            ("control", Pat::OneOf(&LOG_NAMES)),
            ("source", Pat::OneOf(&["mouse", "accesskit"])),
        ],
    ) || common::line_matches(
        text,
        "release",
        &[
            ("control", Pat::OneOf(&LOG_NAMES)),
            (
                "reason",
                Pat::OneOf(&[
                    "pointer_up",
                    "tap",
                    "window_blur",
                    "stop",
                    "gate",
                    "controller_idle",
                ]),
            ),
        ],
    ) || common::line_matches(
        text,
        "ignored",
        &[
            ("control", Pat::OneOf(ignored_controls)),
            ("reason", Pat::OneOf(&IGNORED_REASONS)),
        ],
    ) || matches!(text, "stick=left start" | "stick=right start")
        || ["left", "right"].iter().any(|s| {
            text.strip_prefix(&format!("stick={s} end "))
                .is_some_and(|r| {
                    common::line_matches(
                        r,
                        "",
                        &[(
                            "reason",
                            Pat::OneOf(&[
                                "pointer_up",
                                "window_blur",
                                "stop",
                                "gate",
                                "controller_idle",
                            ]),
                        )],
                    )
                })
        })
        || common::line_matches(
            text,
            "",
            &[(
                "command",
                Pat::OneOf(&["queue_full", "no_worker", "worker_gone"]),
            )],
        )
}

/// The state of a `pad sent` line.
fn sent_state(text: &str) -> Option<State> {
    let rest = text.strip_prefix("sent ")?;
    let get = |k: &str| {
        rest.split(' ')
            .find_map(|f| f.strip_prefix(&format!("{k}=")))
            .map(str::to_owned)
    };
    Some(State {
        buttons: u16::from_str_radix(get("buttons")?.strip_prefix("0x")?, 16).ok()?,
        hat: get("hat")?.parse().ok()?,
        lx: get("lx")?.parse().ok()?,
        ly: get("ly")?.parse().ok()?,
        rx: get("rx")?.parse().ok()?,
        ry: get("ry")?.parse().ok()?,
        flags: 0,
    })
}

struct Rig {
    h: Harness<'static, App>,
    clock: FakeClock,
    ui: TestClock,
    backend: FakeBackend,
    _published: Arc<AtomicU64>,
    _logger: Logger,
    sink: MemorySink,
    log: LogHandle,
}

fn rig_with(ports: Vec<FakePort>, link: bool) -> Rig {
    let clock = FakeClock::at(0);
    let mut backend = FakeBackend::new(clock.clone());
    for p in ports {
        backend = backend.with(p);
    }
    let ui = TestClock::default();
    let published = Arc::new(AtomicU64::new(0));
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let log = logger.handle();
    let app_log = log.clone();
    let (c, p, u) = (clock.clone(), Arc::clone(&published), ui.clone());
    let mut slot = Some(backend.clone());
    let h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            let app = if link {
                let handle = common::fake_worker_logged(
                    &cc.egui_ctx,
                    slot.take().unwrap(),
                    c.clone(),
                    Arc::clone(&p),
                    Wake::App,
                    log.clone(),
                );
                App::with_link(cc, Mode::Main, handle)
            } else {
                App::without_link(cc, Mode::Main)
            };
            app.ui_clock(u.ui_clock()).led_log(log.clone())
        });
    Rig {
        h,
        clock,
        ui,
        backend,
        _published: published,
        _logger: logger,
        sink,
        log: app_log,
    }
}

fn rig(ports: Vec<FakePort>) -> Rig {
    rig_with(ports, true)
}

impl Rig {
    /// Every line so far (`<kind> <text>`, no stamps): a marker is written
    /// and waited for (the log is FIFO).
    fn all_now(&self) -> Vec<String> {
        let marker = format!("marker-{:?}", Instant::now());
        self.log.log("test", marker.clone());
        let start = Instant::now();
        loop {
            let lines = self.sink.lines();
            if lines.iter().any(|l| l.ends_with(&marker)) {
                return lines
                    .iter()
                    .filter(|l| !l.contains(" test marker-"))
                    .map(|l| l.splitn(3, ' ').nth(2).unwrap_or("").to_owned())
                    .collect();
            }
            assert!(start.elapsed() < LIMIT, "log never flushed");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    /// The `pad` texts so far.
    fn pads(&self) -> Vec<String> {
        self.all_now()
            .iter()
            .filter_map(|l| l.strip_prefix("pad ").map(str::to_owned))
            .collect()
    }

    /// The bridge port is open (opened more often than closed).
    fn port_open(&self) -> bool {
        let st = self.backend.state();
        let closes = st.closes.iter().filter(|c| c.1 == CU).count();
        st.opens_of(CU).len() > closes
    }

    /// One frame `d_us` later on both clocks. With the port open the
    /// worker's clock moves first and the frame waits until the worker has
    /// run a whole pass at the new time (two read calls at or after it:
    /// the first pass read and published), then the UI clock follows. A
    /// worker that does not get there within `LIMIT` fails the test
    /// ("worker starved") instead of letting the LEDs age (review
    /// 2026-11-27: slow CI runners).
    fn frame(&mut self, d_us: u64) {
        if d_us > 0 {
            self.clock.advance(d_us);
            let now = self.clock.now_us();
            let start = Instant::now();
            while self.port_open()
                && self
                    .backend
                    .state()
                    .read_calls
                    .iter()
                    .rev()
                    .take_while(|t| **t >= now)
                    .count()
                    < 2
            {
                assert!(start.elapsed() < LIMIT, "worker starved at {now} µs");
                std::thread::sleep(Duration::from_millis(1));
            }
            self.ui.advance_us(d_us);
        }
        self.h.step();
    }

    /// Frames of `FRAME_US` until `f` holds on the lines (bounded).
    fn pump_until(&mut self, what: &str, f: impl Fn(&[String]) -> bool) {
        let start = Instant::now();
        loop {
            let lines = self.all_now();
            if f(&lines) {
                return;
            }
            assert!(
                start.elapsed() < LIMIT,
                "{what} not reached:\n{}",
                lines.join("\n")
            );
            self.frame(FRAME_US);
        }
    }

    /// Runs until the session is connected, the three LEDs ok and the gate
    /// open.
    fn until_open(&mut self) {
        let start = Instant::now();
        while self
            .h
            .state()
            .link_snapshot()
            .is_none_or(|s| s.phase != Phase::Connected || s.status.is_none())
            || !self.h.state().gate().gamepad_enabled()
        {
            assert!(start.elapsed() < LIMIT, "gate never opened");
            self.frame(FRAME_US);
        }
        // A few frames so the gate's `on` line and the layout settle.
        for _ in 0..3 {
            self.frame(FRAME_US);
        }
    }

    fn ctl<T>(&self, f: impl FnOnce(&mut FakeController) -> T) -> T {
        let mut st = self.backend.state();
        match &mut st.ports[0].device {
            Device::Controller(c) => f(c),
            Device::Silent => panic!("no controller"),
        }
    }

    fn direct(&self) -> Vec<State> {
        self.ctl(|c| c.log().direct.clone())
    }

    /// Frames until the controller's DIRECT log has `n` states (bounded).
    fn until_direct(&mut self, n: usize) {
        let start = Instant::now();
        while self.direct().len() < n {
            assert!(
                start.elapsed() < LIMIT,
                "controller got {:?}, want {n}",
                self.direct()
            );
            self.frame(FRAME_US);
        }
    }

    fn center(&self, name: &str) -> Pos2 {
        self.h
            .get_by_role_and_label(Role::Button, name)
            .rect()
            .center()
    }

    fn stick_center(&self, s: Stick) -> Pos2 {
        self.h
            .get_by_role_and_label(Role::Slider, s.a11y_name())
            .rect()
            .center()
    }

    fn push(&mut self, e: Event) {
        self.h.input_mut().events.push(e);
    }

    /// The pointer over `pos` (one frame, no time).
    fn hover(&mut self, pos: Pos2) {
        self.push(Event::PointerMoved(pos));
        self.frame(0);
    }

    /// Press at `pos` in a frame of its own (no time passes).
    fn press(&mut self, pos: Pos2) {
        self.hover(pos);
        self.push(button(pos, true));
        self.frame(0);
    }

    /// Release at `pos` (no time passes).
    fn release(&mut self, pos: Pos2) {
        self.push(button(pos, false));
        self.frame(0);
    }

    /// A quick click: press and release in one frame.
    fn click(&mut self, pos: Pos2) {
        self.hover(pos);
        self.push(button(pos, true));
        self.push(button(pos, false));
        self.frame(0);
    }

    /// Frames until the pad has nothing pending and the controller's DIRECT
    /// log has `n` states.
    fn settle(&mut self, n: usize) {
        let start = Instant::now();
        while self.h.state().pad().held().is_some() {
            assert!(start.elapsed() < LIMIT, "still held");
            self.frame(FRAME_US);
        }
        self.until_direct(n);
    }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn esc() -> Event {
    Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn with(buttons: u16) -> State {
    State { buttons, ..NEUTRAL }
}

fn hat(h: u8) -> State {
    State { hat: h, ..NEUTRAL }
}

/// AC 4: a click (press and release in one frame) is a tap: A goes out in
/// the press frame, NEUTRAL no earlier than 50 ms after the press and no
/// later than the first frame at or after that time; the app asks for that
/// frame (`request_repaint_after`), no input needed.
#[test]
fn g23_click_is_a_tap() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let a = r.center("A");
    let t0 = r.ui.now_us();
    r.click(a);
    assert_eq!(
        r.h.state().pad().last_sent(),
        with(BTN_A),
        "A in the press frame"
    );
    assert_eq!(
        r.h.state().last_pad_wake(),
        Some(Duration::from_micros(TAP_MIN_US)),
        "the release frame is requested"
    );
    let mut released_at = None;
    for _ in 0..10 {
        r.frame(FRAME_US / 2);
        if r.h.state().pad().last_sent() == NEUTRAL {
            released_at = Some(r.ui.now_us());
            break;
        }
    }
    let at = released_at.expect("NEUTRAL sent");
    assert!(at >= t0 + TAP_MIN_US, "released at {at}, press at {t0}");
    assert!(
        at < t0 + TAP_MIN_US + FRAME_US / 2,
        "not one frame late: {at}"
    );
    r.until_direct(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
    let pads = r.pads();
    assert_eq!(pads[0], "press control=a source=mouse");
    assert!(
        pads.contains(&"release control=a reason=tap".to_owned()),
        "{pads:?}"
    );
}

/// AC 4: a 3 s hold is one press and one release, nothing in between.
#[test]
fn g23_hold_is_a_hold() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let a = r.center("A");
    r.press(a);
    r.until_direct(1);
    let start = r.ui.now_us();
    while r.ui.now_us() < start + 3_000_000 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.direct(), vec![with(BTN_A)], "nothing sent while held");
    assert_eq!(r.ctl(|c| c.mode()), CtlMode::Direct);
    r.release(a);
    r.until_direct(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
    assert_eq!(r.ctl(|c| c.log().app_timeouts), 0);
    let pads = r.pads();
    assert!(
        pads.contains(&"release control=a reason=pointer_up".to_owned()),
        "{pads:?}"
    );
}

/// AC 4: a press dragged out of A (over B) keeps A held until the release;
/// B is never pressed.
#[test]
fn g23_drag_out_keeps_hold() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let (a, b) = (r.center("A"), r.center("B"));
    r.press(a);
    for p in [a + vec2(30.0, 0.0), b, b + vec2(0.0, 40.0)] {
        r.push(Event::PointerMoved(p));
        r.frame(FRAME_US);
        assert_eq!(r.h.state().pad().held(), Some(Control::A));
    }
    for _ in 0..12 {
        r.frame(FRAME_US);
    }
    r.release(b);
    r.settle(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
}

/// AC 5: the left stick follows the drag (17 pt = full deflection,
/// circular clamp) and springs back to 0x80/0x80 on release.
#[test]
fn g23_stick_drag_and_spring() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let c = r.stick_center(Stick::Left);
    r.press(c);
    assert_eq!(r.h.state().pad().dragging(), Some(Stick::Left));
    let mut want = Vec::new();
    for (d, lx, ly) in [
        (vec2(17.0, 0.0), 255, 128),
        (vec2(-17.0, 0.0), 0, 128),
        (vec2(0.0, -17.0), 128, 0),
    ] {
        r.push(Event::PointerMoved(c + d));
        r.frame(FRAME_US);
        let s = r.h.state().pad().last_sent();
        assert_eq!((s.lx, s.ly), (lx, ly), "offset {d:?}");
        want.push(State { lx, ly, ..NEUTRAL });
    }
    r.push(Event::PointerMoved(c + vec2(30.0, 30.0)));
    r.frame(FRAME_US);
    let s = r.h.state().pad().last_sent();
    let radius = (f32::from(s.lx) - 128.0).hypot(f32::from(s.ly) - 128.0);
    assert!((radius - 127.5).abs() <= 127.5 * 0.02, "{s:?}");
    r.release(c + vec2(30.0, 30.0));
    r.frame(FRAME_US);
    assert_eq!(r.h.state().pad().last_sent(), NEUTRAL, "spring back");
    r.until_direct(5);
    assert_eq!(r.direct().last(), Some(&NEUTRAL));
    assert_eq!(&r.direct()[..3], &want[..]);
    let pads = r.pads();
    assert_eq!(pads.first().map(String::as_str), Some("stick=left start"));
    assert!(pads.contains(&"stick=left end reason=pointer_up".to_owned()));
}

/// AC 6: holding A still for 100 frames sends one SetState.
#[test]
fn g23_one_send_per_change() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let a = r.center("A");
    r.press(a);
    for _ in 0..100 {
        r.frame(10_000);
    }
    let sent: Vec<String> = r
        .pads()
        .into_iter()
        .filter(|l| l.starts_with("sent "))
        .collect();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(r.direct(), vec![with(BTN_A)]);
}

/// AC 6 (review 2026-11-27: through the real app): the worker thread held
/// in its pass (a stuck worker), the left stick dragged until the command
/// queue (64) is full: one `pad command=queue_full`, a resend wake of
/// `RESEND_AFTER_US`; the worker let go, the queue drains and the pad
/// sends its current state: the controller ends with exactly the pad's
/// state.
#[test]
fn g23_queue_full_resends() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let c = r.stick_center(Stick::Left);
    r.press(c);
    r.backend.state().hold = true;
    // Room for the worker to enter its held read.
    std::thread::sleep(Duration::from_millis(20));
    // UI time does not move while the worker is held: the LEDs stay fresh.
    let mut k = 0;
    while !r.pads().iter().any(|l| l == "command=queue_full") {
        k += 1;
        assert!(k < 200, "queue never full: {:?}", r.pads());
        let x = -17.0 + 34.0 * (k as f32 % 100.0) / 100.0;
        r.push(Event::PointerMoved(c + vec2(x, 0.0)));
        r.frame(0);
    }
    assert!(k >= 60, "the queue holds 64 commands, full after {k} moves");
    assert_eq!(
        r.h.state().last_pad_wake(),
        Some(Duration::from_micros(ocb_app::gamepad::RESEND_AFTER_US))
    );
    // Moved on while full: the state the pad holds now.
    r.push(Event::PointerMoved(c + vec2(5.0, -9.0)));
    r.frame(0);
    let want = r.h.state().pad().state();
    r.backend.state().hold = false;
    let start = Instant::now();
    while r.ctl(|c| c.published()) != want || r.h.state().pad().last_sent() != want {
        assert!(start.elapsed() < LIMIT, "never caught up");
        r.frame(FRAME_US);
    }
    let pads = r.pads();
    assert_eq!(
        pads.iter().filter(|l| *l == "command=queue_full").count(),
        1,
        "{pads:?}"
    );
    r.release(c + vec2(5.0, -9.0));
    r.frame(FRAME_US);
    let start = Instant::now();
    while r.ctl(|c| c.published()) != NEUTRAL {
        assert!(start.elapsed() < LIMIT);
        r.frame(FRAME_US);
    }
}

/// Review 2026-11-27: a STOP for a held input that finds the worker's
/// queue full is retried until it is queued (the queued states could
/// leave a stick deflected): `stop requested source=esc … command=
/// queue_full`, later `stop requested source=gamepad … command=queued`,
/// and the controller ends IDLE publishing NEUTRAL.
#[test]
fn g23_stop_retried_when_queue_full() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let c = r.stick_center(Stick::Left);
    r.press(c);
    r.backend.state().hold = true;
    std::thread::sleep(Duration::from_millis(20));
    let mut k = 0;
    while !r.pads().iter().any(|l| l == "command=queue_full") {
        k += 1;
        assert!(k < 200);
        r.push(Event::PointerMoved(c + vec2(-17.0 + 0.3 * k as f32, 0.0)));
        r.frame(0);
    }
    r.push(esc());
    r.frame(0);
    r.release(c);
    r.frame(0);
    let all = r.all_now();
    assert!(
        all.iter()
            .any(|l| l.starts_with("stop requested source=esc ")
                && l.ends_with("command=queue_full")),
        "{all:?}"
    );
    r.backend.state().hold = false;
    r.pump_until("retried STOP", |l| {
        l.iter().any(|x| {
            x.starts_with("stop requested source=gamepad ") && x.ends_with("command=queued")
        }) && l.iter().any(|x| x.starts_with("stop after "))
    });
    let start = Instant::now();
    while r.ctl(|c| c.published()) != NEUTRAL {
        assert!(start.elapsed() < LIMIT, "{:?}", r.ctl(|c| c.published()));
        r.frame(FRAME_US);
    }
    assert_eq!(r.ctl(|c| c.mode()), CtlMode::Idle);
}

/// Review 2026-11-27: eframe calls only `logic` while the window is
/// minimized or occluded; the pad's blur, tap timer and send run there. A
/// held A is released by a logic-only frame that sees the window not
/// visible (or unfocused), and a click's 50 ms tail ends without `ui`.
#[test]
fn g23_hidden_window_releases() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let a = r.center("A");
    r.press(a);
    r.until_direct(1);
    // A frame in which `ui` does not run: hidden.
    let wake = r.h.state_mut().pad_background(true, false, false);
    assert_eq!(wake, None);
    assert_eq!(
        r.h.state().pad().last_sent(),
        NEUTRAL,
        "released while hidden"
    );
    r.until_direct(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
    let pads = r.pads();
    assert!(
        pads.contains(&"release control=a reason=window_blur".to_owned()),
        "{pads:?}"
    );
    // Shown again with the button up: a click works.
    r.release(a);
    r.frame(FRAME_US);
    // The tap tail: the click, then only logic-only frames.
    r.click(a);
    assert_eq!(r.h.state().pad().last_sent(), with(BTN_A));
    r.ui.advance_us(TAP_MIN_US);
    let _ = r.h.state_mut().pad_background(true, true, false);
    assert_eq!(
        r.h.state().pad().last_sent(),
        NEUTRAL,
        "the tap ended without ui"
    );
    r.until_direct(4);
    assert_eq!(r.direct()[2..], [with(BTN_A), NEUTRAL]);
}

/// Review 2026-11-27: a press in the frame the window loses focus is spent,
/// not applied (rule 8a would otherwise hold it in an unfocused window).
#[test]
fn g23_press_in_the_blur_frame() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let a = r.center("A");
    r.hover(a);
    r.push(Event::WindowFocused(false));
    r.push(button(a, true));
    r.frame(0);
    assert_eq!(r.h.state().pad().held(), None);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert!(r.direct().is_empty());
    assert!(
        !r.pads().iter().any(|l| l.starts_with("press ")),
        "{:?}",
        r.pads()
    );
}

/// AC 8a: the window losing focus releases A with one NEUTRAL; with the
/// button still down after focus returns nothing is pressed until a new
/// press.
#[test]
fn g23_blur_releases() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let a = r.center("A");
    r.press(a);
    r.until_direct(1);
    r.push(Event::WindowFocused(false));
    r.frame(FRAME_US);
    r.until_direct(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
    r.push(Event::WindowFocused(true));
    r.frame(FRAME_US);
    for _ in 0..10 {
        r.frame(FRAME_US);
    }
    assert_eq!(
        r.direct().len(),
        2,
        "no new press while the button stays down"
    );
    r.release(a);
    r.press(a);
    r.until_direct(3);
    assert_eq!(r.direct()[2], with(BTN_A));
    let pads = r.pads();
    assert!(
        pads.contains(&"release control=a reason=window_blur".to_owned()),
        "{pads:?}"
    );
}

/// AC 8b: Esc (and the STOP button by Enter) while A is held: one `stop
/// requested`, `pad release … reason=stop`, no SetState after; the release
/// sends nothing; the next press sends A again. STOP keeps its focus during
/// the hold.
#[test]
fn g23_stop_drops_held() {
    for by_button in [false, true] {
        let mut r = rig(vec![bridge()]);
        r.until_open();
        let a = r.center("A");
        if by_button {
            for _ in 0..4 {
                r.h.key_press(Key::Tab);
                r.frame(0);
            }
            assert_eq!(r.h.ctx.memory(|m| m.focused()), Some(bottom_bar::stop_id()));
        }
        r.press(a);
        r.until_direct(1);
        if by_button {
            assert_eq!(
                r.h.ctx.memory(|m| m.focused()),
                Some(bottom_bar::stop_id()),
                "a press keeps STOP's focus"
            );
            r.h.key_press(Key::Enter);
        } else {
            r.push(esc());
        }
        r.frame(0);
        r.pump_until("stop after", |l| {
            l.iter().any(|x| x.starts_with("stop after "))
        });
        r.release(a);
        for _ in 0..10 {
            r.frame(FRAME_US);
        }
        assert_eq!(r.direct(), vec![with(BTN_A)], "nothing sent after STOP");
        assert_eq!(r.ctl(|c| c.published()), NEUTRAL, "STOP released A");
        let all = r.all_now();
        let source = if by_button { "button" } else { "esc" };
        let req = all
            .iter()
            .position(|l| l.starts_with(&format!("stop requested source={source} ")))
            .unwrap_or_else(|| panic!("{all:?}"));
        let rel = all
            .iter()
            .position(|l| l == "pad release control=a reason=stop")
            .unwrap_or_else(|| panic!("{all:?}"));
        assert!(req < rel, "STOP logged before the pad line");
        assert!(
            !all[rel..].iter().any(|l| l.starts_with("pad sent")),
            "{all:?}"
        );
        // The next press goes out (after the session is back from STOP).
        r.press(a);
        r.until_direct(2);
        assert_eq!(r.direct()[1], with(BTN_A), "by_button={by_button}");
    }
}

/// AC 8c: the Switch turning not ready while A is held: `pad release …
/// reason=gate`, `stop requested source=gamepad`, the controller ends
/// IDLE/NEUTRAL.
#[test]
fn g23_block_while_held() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let a = r.center("A");
    r.press(a);
    r.until_direct(1);
    r.ctl(|c| c.set_console_ready(false));
    r.pump_until("stop after", |l| {
        l.iter().any(|x| x.starts_with("stop after "))
    });
    let all = r.all_now();
    assert!(
        all.contains(&"pad release control=a reason=gate".to_owned()),
        "{all:?}"
    );
    assert!(
        all.iter()
            .any(|l| l.starts_with("stop requested source=gamepad ")),
        "{all:?}"
    );
    assert_eq!(r.ctl(|c| c.mode()), CtlMode::Idle);
    assert_eq!(r.ctl(|c| c.published()), NEUTRAL);
    assert_eq!(r.direct(), vec![with(BTN_A)], "no NEUTRAL SET_STATE");
}

/// AC 8d: the controller aborting DIRECT by itself while A is held drops
/// it (`reason=controller_idle`), nothing sent for it; the next press sends
/// A again.
#[test]
fn g23_controller_idle_drops_held() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let a = r.center("A");
    r.press(a);
    r.until_direct(1);
    r.pump_until("direct", |l| l.iter().any(|x| x.contains("to=direct")));
    r.ctl(|c| c.force_abort(None));
    r.pump_until("released", |l| {
        l.contains(&"pad release control=a reason=controller_idle".to_owned())
    });
    r.release(a);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.direct(), vec![with(BTN_A)], "nothing sent for it");
    r.until_open();
    r.press(a);
    r.until_direct(2);
    assert_eq!(r.direct()[1], with(BTN_A));
}

/// AC 9: every gate reason produced end to end; a click on A writes `pad
/// ignored control=a reason=<r>`, the controller gets nothing, the hint is
/// one `Role::Label`.
#[test]
fn g23_blocked_clicks_send_nothing() {
    type Setup = fn() -> (Vec<FakePort>, bool);
    let cases: Vec<(&str, Setup)> = vec![
        ("worker", || (vec![], false)),
        ("no_bridge", || (vec![], true)),
        ("bridge_not_ready", || {
            (
                vec![FakePort::bridge(CU, SERIAL, Some("Other Board"))],
                true,
            )
        }),
        ("firmware_update", || {
            (
                vec![bridge().controller(ControllerConfig {
                    proto_ver: PROTO_VERSION.wrapping_add(1),
                    ..ControllerConfig::default()
                })],
                true,
            )
        }),
        ("controller_link", || (vec![bridge().mute_at(0)], true)),
        ("controller_link", || {
            (
                vec![bridge().hook(0, Box::new(|c| c.set_link_ok(false)))],
                true,
            )
        }),
        ("switch_not_ready", || {
            (
                vec![bridge().hook(0, Box::new(|c| c.set_console_ready(false)))],
                true,
            )
        }),
    ];
    for (reason, setup) in cases {
        let (ports, link) = setup();
        let has_ctl = !ports.is_empty();
        let mut r = rig_with(ports, link);
        let start = Instant::now();
        while r
            .h
            .state()
            .gate()
            .gamepad
            .is_none_or(|b| b.as_str() != reason)
        {
            assert!(
                start.elapsed() < LIMIT,
                "{reason}: gate {:?}",
                r.h.state().gate()
            );
            r.frame(FRAME_US);
        }
        for _ in 0..3 {
            r.frame(FRAME_US);
        }
        // The firmware dialog (Story 4.6) is modal: closed first, so the
        // click reaches the pad.
        if r.h.state().modals().is_open() {
            r.h.get_by_role_and_label(Role::Button, "OK").click();
            for _ in 0..3 {
                r.frame(FRAME_US);
            }
            assert!(!r.h.state().modals().is_open(), "{reason}");
        }
        let a = r.center("A");
        r.click(a);
        for _ in 0..5 {
            r.frame(FRAME_US);
        }
        let pads = r.pads();
        assert_eq!(
            pads,
            vec![format!("ignored control=a reason={reason}")],
            "{reason}"
        );
        if has_ctl && reason != "bridge_not_ready" {
            assert!(r.direct().is_empty(), "{reason}");
        }
        let hint = r.h.state().gate().gamepad.unwrap().gamepad_hint();
        let _ = r.h.get_by_role_and_label(Role::Label, hint);
        assert!(pads.iter().all(|l| is_app_pad_line(l)), "{pads:?}");
    }
}

/// AC 14: an AccessKit click on each button is a tap: its bit (or hat),
/// then NEUTRAL at least 50 ms later.
#[test]
fn g23_accesskit_click_is_a_tap() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let t0 = r.ui.now_us();
    r.h.get_by_role_and_label(Role::Button, "A")
        .click_accesskit();
    r.frame(0);
    assert_eq!(r.h.state().pad().last_sent(), with(BTN_A));
    while r.h.state().pad().held().is_some() {
        r.frame(FRAME_US / 4);
        assert!(r.ui.now_us() < t0 + 1_000_000);
    }
    assert!(r.ui.now_us() >= t0 + TAP_MIN_US);
    r.settle(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
    assert_eq!(r.pads()[0], "press control=a source=accesskit");
    // Sticks are sliders without a Click action.
    for s in Stick::ALL {
        let node = r.h.get_by_role_and_label(Role::Slider, s.a11y_name());
        assert!(
            !node
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Click)
        );
    }
}

/// The wire state of each control (Dev Notes › Bit table).
fn bit_table() -> Vec<(Control, State)> {
    vec![
        (Control::Y, with(BTN_Y)),
        (Control::B, with(BTN_B)),
        (Control::A, with(BTN_A)),
        (Control::X, with(BTN_X)),
        (Control::L, with(BTN_L)),
        (Control::R, with(BTN_R)),
        (Control::Zl, with(BTN_ZL)),
        (Control::Zr, with(BTN_ZR)),
        (Control::Minus, with(BTN_MINUS)),
        (Control::Plus, with(BTN_PLUS)),
        (Control::Home, with(BTN_HOME)),
        (Control::Capture, with(BTN_CAPTURE)),
        (Control::DUp, hat(HAT_UP)),
        (Control::DRight, hat(HAT_RIGHT)),
        (Control::DDown, hat(HAT_DOWN)),
        (Control::DLeft, hat(HAT_LEFT)),
    ]
}

/// AC 14 (H.35 step 9): `click_accesskit()` on every control sends exactly
/// its bit or hat, then NEUTRAL.
#[test]
fn g23_every_control_bits() {
    assert_eq!(HAT_NEUTRAL, NEUTRAL.hat);
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let mut want = Vec::new();
    for (c, s) in bit_table() {
        r.h.get_by_role_and_label(Role::Button, c.a11y_name())
            .click_accesskit();
        r.frame(0);
        want.push(s);
        want.push(NEUTRAL);
        r.settle(want.len());
    }
    assert_eq!(r.direct(), want);
    // The pointer gives the same bits (one per control, mouse clicks).
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let mut want = Vec::new();
    for (c, s) in bit_table() {
        let p = r.center(c.a11y_name());
        r.click(p);
        want.push(s);
        want.push(NEUTRAL);
        r.settle(want.len());
    }
    assert_eq!(r.direct(), want);
}

/// AC 16: Esc and a press on A in the same frame: STOP, no SetState(A), no
/// `pad press` line, and the press is spent until the release.
#[test]
fn g23_esc_and_press_same_frame() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let a = r.center("A");
    r.hover(a);
    r.push(esc());
    r.push(button(a, true));
    r.frame(0);
    assert!(r.h.state().pad().is_spent());
    for _ in 0..10 {
        r.frame(FRAME_US);
    }
    r.release(a);
    let all = r.all_now();
    assert!(
        all.iter()
            .any(|l| l.starts_with("stop requested source=esc ")),
        "{all:?}"
    );
    assert!(!all.iter().any(|l| l.starts_with("pad ")), "{all:?}");
    assert!(r.direct().is_empty());
}

/// AC 17: every app `pad` line matches its pattern and precedes the
/// worker's `pad sent` for it; 100 idle frames write no `pad` line; the
/// pattern rejects what it must.
#[test]
fn g23_pad_lines() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    for _ in 0..100 {
        r.frame(FRAME_US);
    }
    assert!(r.pads().is_empty(), "idle: no pad line");
    let (a, c) = (r.center("A"), r.stick_center(Stick::Right));
    r.click(a);
    r.settle(2);
    r.press(c);
    r.push(Event::PointerMoved(c + vec2(10.0, 0.0)));
    r.frame(FRAME_US);
    r.release(c + vec2(10.0, 0.0));
    r.frame(FRAME_US);
    r.until_direct(4);
    let pads = r.pads();
    for l in &pads {
        if !l.starts_with("sent ") {
            assert!(is_app_pad_line(l), "{l}");
        }
    }
    let press = pads
        .iter()
        .position(|l| l == "press control=a source=mouse")
        .unwrap();
    let first_sent = pads.iter().position(|l| l.starts_with("sent ")).unwrap();
    assert!(press < first_sent, "{pads:?}");
    let start = pads.iter().position(|l| l == "stick=right start").unwrap();
    let stick_sent = pads
        .iter()
        .position(|l| sent_state(l).is_some_and(|s| s.rx != 128))
        .unwrap();
    assert!(start < stick_sent, "{pads:?}");
    for bad in [
        "press control=q source=mouse",
        "press control=a source=keyboard",
        "release control=a reason=later",
        "ignored control=a reason=asleep",
        "stick=up start",
        "stick=left end reason=tap",
        "command=busy",
        "press control=a",
    ] {
        assert!(!is_app_pad_line(bad), "{bad}");
    }
    for good in [
        "ignored control=stick_left reason=switch_not_ready",
        "command=no_worker",
        "ignored control=b reason=busy",
        "release control=capture reason=controller_idle",
    ] {
        assert!(is_app_pad_line(good), "{good}");
    }
}

/// The H.35 dry run (Task 9, rig block item 5): the bench steps replayed on
/// the fake worker with an `ocb-sim` controller, the expected lines of each
/// step printed (`cargo test -p ocb-app -- --nocapture h35_dry_run`) and
/// checked. Step 2's walking, 3's ocarina and the owner's ears are the
/// bench's; step 11's Joy-Con setting is `set_console_ready(false)`; step
/// 12 (close) is the smoke run's and G.13's.
#[test]
fn g23_h35_dry_run() {
    const UNPLUG_AT: u64 = 30_000_000;
    let port = bridge().present(vec![(0, Some(UNPLUG_AT)), (UNPLUG_AT + 2_000_000, None)]);
    let mut r = rig(vec![port]);
    r.until_open();
    let mut seen = 0;
    let print = |r: &Rig, seen: &mut usize, step: &str| -> Vec<String> {
        let all = r.all_now();
        let new: Vec<String> = all[*seen..].to_vec();
        *seen = all.len();
        println!("H.35 {step}:");
        for l in &new {
            println!("  {l}");
        }
        new
    };
    let setup = print(&r, &mut seen, "setup (start, three LEDs ok, gate on)");
    assert!(
        setup
            .iter()
            .any(|l| l.starts_with("gate gamepad=on gamepad_reason=none ")),
        "{setup:?}"
    );
    let ms = |l: &str| l.to_owned();
    let _ = ms;
    // Step 1: quick click on A. The runbook's release window on the UI
    // clock: `reason=tap` → the NEUTRAL goes out 49–70 ms after A.
    let a = r.center("A");
    let n0 = r.direct().len();
    let t0 = r.ui.now_us();
    r.click(a);
    let mut t1 = t0;
    while r.h.state().pad().last_sent() != NEUTRAL {
        assert!(r.ui.now_us() < t0 + 1_000_000);
        r.frame(FRAME_US / 2);
        t1 = r.ui.now_us();
    }
    let ms = (t1 - t0) / 1000;
    assert!((49..=70).contains(&ms), "step 1 tap window: {ms} ms");
    r.settle(n0 + 2);
    r.pump_until("step 1 mode", |l| {
        l.iter().any(|x| x.contains("from=idle to=direct"))
    });
    let l = print(&r, &mut seen, "step 1 (quick click A)");
    let pos = |p: &str| {
        l.iter()
            .position(|x| x.starts_with(p))
            .unwrap_or_else(|| panic!("{p}: {l:?}"))
    };
    let press = pos("pad press control=a source=mouse");
    let sent_a = pos(&format!(
        "pad sent port={CU} buttons=0x0004 hat=8 lx=128 ly=128 rx=128 ry=128"
    ));
    let release = pos("pad release control=a reason=tap");
    let sent_n = pos(&format!(
        "pad sent port={CU} buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128"
    ));
    let mode = pos(&format!("mode port={CU} from=idle to=direct abort_count="));
    assert!(
        press < sent_a && sent_a < sent_n && release < sent_n && sent_a < mode,
        "{l:?}"
    );
    assert!(!l.iter().any(|x| x.starts_with("nak ")), "{l:?}");
    // Step 2: drag the left stick up, release.
    let c = r.stick_center(Stick::Left);
    let n = r.direct().len();
    r.press(c);
    for k in 1..=5 {
        r.push(Event::PointerMoved(c + vec2(0.0, -4.0 * k as f32)));
        r.frame(FRAME_US);
    }
    r.release(c + vec2(0.0, -20.0));
    r.frame(FRAME_US);
    r.until_direct(n + 6);
    let l = print(&r, &mut seen, "step 2 (drag left stick up, release)");
    assert_eq!(l.iter().filter(|x| *x == "pad stick=left start").count(), 1);
    assert!(
        l.iter()
            .any(|x| x.starts_with("pad sent ") && sent_state(&x[4..]).is_some_and(|s| s.ly < 128))
    );
    assert!(l.contains(&"pad stick=left end reason=pointer_up".to_owned()));
    assert_eq!(
        l.iter()
            .rev()
            .find(|x| x.starts_with("pad sent "))
            .map(String::as_str),
        Some(
            format!("pad sent port={CU} buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128").as_str()
        )
    );
    // Step 3: D-pad left.
    let n = r.direct().len();
    let p = r.center("D-pad left");
    r.click(p);
    r.settle(n + 2);
    let l = print(&r, &mut seen, "step 3 (quick click D-pad left)");
    assert!(l.iter().any(|x| x.contains(" hat=6 ")), "{l:?}");
    // Step 4: A, X, Y, L, R.
    for (name, bits) in [
        ("A", "0x0004"),
        ("X", "0x0008"),
        ("Y", "0x0001"),
        ("L", "0x0010"),
        ("R", "0x0020"),
    ] {
        let n = r.direct().len();
        let p = r.center(name);
        r.click(p);
        r.settle(n + 2);
        r.frame(FRAME_US);
        let l = print(&r, &mut seen, &format!("step 4 (quick click {name})"));
        assert!(
            l.iter().any(|x| x.contains(&format!("buttons={bits} "))),
            "{l:?}"
        );
        assert!(l.iter().any(|x| x.contains("buttons=0x0000 ")), "{l:?}");
    }
    // Step 5: hold A for 3 s.
    let n = r.direct().len();
    r.press(a);
    r.until_direct(n + 1);
    let t = r.ui.now_us();
    while r.ui.now_us() < t + 3_000_000 {
        r.frame(FRAME_US);
    }
    r.release(a);
    r.settle(n + 2);
    let l = print(&r, &mut seen, "step 5 (hold A 3 s)");
    assert!(
        !l.iter().any(|x| x.contains("to=idle")
            || x.starts_with("nak ")
            || x.starts_with("stop ")
            || x.starts_with("reboot ")),
        "{l:?}"
    );
    assert!(l.contains(&"pad release control=a reason=pointer_up".to_owned()));
    // Step 6: hold A, Esc, release; then a quick click.
    let n = r.direct().len();
    r.press(a);
    r.until_direct(n + 1);
    r.frame(FRAME_US);
    r.push(esc());
    r.frame(0);
    r.pump_until("step 6", |l| {
        l[seen..].iter().any(|x| x.starts_with("stop after "))
            && l[seen..].iter().any(|x| x.contains("from=direct to=idle"))
    });
    r.release(a);
    r.frame(FRAME_US);
    let l = print(&r, &mut seen, "step 6 (hold A, Esc)");
    let stop_lines = |l: &[String], source: &str| {
        assert!(
            l.contains(&format!(
                "stop requested source={source} dialog=none command=queued"
            )),
            "{l:?}"
        );
        assert!(l.contains(&format!("stop sent port={CU}")), "{l:?}");
        let acked = l
            .iter()
            .find(|x| x.starts_with(&format!("stop acked port={CU} after_us=")))
            .unwrap_or_else(|| panic!("{l:?}"));
        let after_us: u64 = acked
            .split(' ')
            .find_map(|w| w.strip_prefix("after_us="))
            .unwrap()
            .parse()
            .unwrap();
        assert!(after_us <= 100_000, "{after_us}");
        assert!(acked.ends_with(" by=command"), "{acked}");
        assert!(
            l.iter()
                .any(|x| x.starts_with(&format!("stop after port={CU} mode=idle "))),
            "{l:?}"
        );
        assert!(
            l.contains(&"pad release control=a reason=stop".to_owned()),
            "{l:?}"
        );
        assert!(l.iter().any(|x| x.contains("from=direct to=idle")), "{l:?}");
        let at = |p: &str| {
            l.iter()
                .position(|x| x.starts_with(p) || x.contains(p))
                .unwrap_or_else(|| panic!("{p}: {l:?}"))
        };
        // The runbook's order: the app's two lines in this order, the
        // worker's four in this order (the two threads may interleave).
        let req = at("stop requested");
        assert!(req < at("pad release control=a reason=stop"), "{l:?}");
        let (sent, acked, mode, after) = (
            at("stop sent "),
            at("stop acked "),
            at("from=direct to=idle"),
            at("stop after "),
        );
        assert!(sent < acked && acked < mode && mode < after, "{l:?}");
        assert!(!l[req..].iter().any(|x| x.starts_with("pad sent")), "{l:?}");
    };
    stop_lines(&l, "esc");
    let n = r.direct().len();
    r.click(a);
    r.settle(n + 2);
    r.pump_until("step 6 click", |l| {
        l[seen..].iter().any(|x| x.contains("from=idle to=direct"))
    });
    let _ = print(&r, &mut seen, "step 6 (then a quick click on A)");
    // Step 7: Tab ×4, hold A, Enter, release.
    for _ in 0..4 {
        r.h.key_press(Key::Tab);
        r.frame(0);
    }
    assert_eq!(r.h.ctx.memory(|m| m.focused()), Some(bottom_bar::stop_id()));
    let n = r.direct().len();
    r.press(a);
    r.until_direct(n + 1);
    r.frame(FRAME_US);
    r.h.key_press(Key::Enter);
    r.frame(0);
    r.pump_until("step 7", |l| {
        l[seen..].iter().any(|x| x.starts_with("stop after "))
            && l[seen..].iter().any(|x| x.contains("from=direct to=idle"))
    });
    r.release(a);
    r.frame(FRAME_US);
    let l = print(&r, &mut seen, "step 7 (Tab x4, hold A, Enter)");
    stop_lines(&l, "button");
    // Step 8: hold A, blur, release, focus back.
    r.until_open();
    let n = r.direct().len();
    r.press(a);
    r.until_direct(n + 1);
    r.push(Event::WindowFocused(false));
    r.frame(FRAME_US);
    r.release(a);
    r.until_direct(n + 2);
    r.push(Event::WindowFocused(true));
    r.frame(FRAME_US);
    let l = print(&r, &mut seen, "step 8 (hold A, Cmd+Tab away and back)");
    assert!(
        l.contains(&"pad release control=a reason=window_blur".to_owned()),
        "{l:?}"
    );
    assert_eq!(
        l.iter()
            .filter(|x| x.starts_with("pad sent ") && x.contains("buttons=0x0000"))
            .count(),
        1,
        "{l:?}"
    );
    let blur = l
        .iter()
        .position(|x| x == "pad release control=a reason=window_blur")
        .unwrap();
    assert!(
        !l[blur..].iter().any(|x| x.starts_with("pad press")),
        "{l:?}"
    );
    // Step 8b: hold A, Cmd+H (hidden: eframe runs `logic` only), release,
    // back: the same lines.
    let n = r.direct().len();
    r.press(a);
    r.until_direct(n + 1);
    let _ = r.h.state_mut().pad_background(true, false, false);
    r.release(a);
    r.until_direct(n + 2);
    r.frame(FRAME_US);
    let l = print(&r, &mut seen, "step 8b (hold A, Cmd+H, back)");
    assert!(
        l.contains(&"pad release control=a reason=window_blur".to_owned()),
        "{l:?}"
    );
    assert_eq!(
        l.iter()
            .filter(|x| x.starts_with("pad sent ") && x.contains("buttons=0x0000"))
            .count(),
        1,
        "{l:?}"
    );
    // Step 9: the button tour (each control's bit; the right stick).
    for (c, s) in bit_table() {
        let n = r.direct().len();
        let p = r.center(c.a11y_name());
        r.click(p);
        r.settle(n + 2);
        assert_eq!(r.direct()[n], s, "{c:?}");
    }
    let rc = r.stick_center(Stick::Right);
    let n = r.direct().len();
    r.press(rc);
    r.push(Event::PointerMoved(rc + vec2(12.0, 0.0)));
    r.frame(FRAME_US);
    r.release(rc + vec2(12.0, 0.0));
    r.frame(FRAME_US);
    r.until_direct(n + 2);
    let l = print(&r, &mut seen, "step 9 (button tour, right stick)");
    assert!(l.contains(&"pad stick=right start".to_owned()), "{l:?}");
    // Step 10: bridge out, a click ignored, bridge back, a click works.
    while r.clock.now_us() < UNPLUG_AT {
        r.frame(FRAME_US);
    }
    r.pump_until("step 10 out", |l| {
        l[seen..]
            .iter()
            .any(|x| x.starts_with("gate gamepad=off gamepad_reason=no_bridge "))
    });
    r.frame(FRAME_US);
    let _ = r.h.get_by_role_and_label(Role::Label, HINT_NO_BRIDGE);
    r.click(a);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    let l = print(&r, &mut seen, "step 10 (bridge out, click A)");
    assert!(
        l.iter()
            .any(|x| x.starts_with("port_lost ") || x.starts_with("port_vanished ")),
        "{l:?}"
    );
    assert!(
        l.iter().any(|x| x.starts_with("led pc_bridge=error ")),
        "{l:?}"
    );
    assert!(
        l.contains(&"pad ignored control=a reason=no_bridge".to_owned()),
        "{l:?}"
    );
    // Gate lines while out: `no_bridge`, possibly after a short
    // `bridge_not_ready` (runbook step 10).
    let gates: Vec<&String> = l.iter().filter(|x| x.starts_with("gate ")).collect();
    assert!(
        gates.iter().all(|x| x.contains("gamepad_reason=no_bridge ")
            || x.contains("gamepad_reason=bridge_not_ready ")),
        "{gates:?}"
    );
    assert!(
        gates
            .last()
            .is_some_and(|x| x.contains("gamepad_reason=no_bridge ")),
        "{gates:?}"
    );
    assert!(
        !l.iter()
            .any(|x| x.starts_with("pad sent") || x.starts_with("stop ")),
        "{l:?}"
    );
    r.until_open();
    let n = r.direct().len();
    r.click(a);
    r.settle(n + 2);
    let l = print(&r, &mut seen, "step 10 (bridge back, quick click A)");
    assert!(l.iter().any(|x| x.starts_with("port_opened ")), "{l:?}");
    assert!(l.iter().any(|x| x.starts_with("gate gamepad=on ")), "{l:?}");
    // Step 11: Wired Communication Off (console_ready 0), click, On again.
    r.ctl(|c| c.set_console_ready(false));
    r.pump_until("step 11 off", |l| {
        l[seen..]
            .iter()
            .any(|x| x.starts_with("gate gamepad=off gamepad_reason=switch_not_ready "))
    });
    r.frame(FRAME_US);
    let _ =
        r.h.get_by_role_and_label(Role::Label, HINT_SWITCH_NOT_READY);
    r.click(a);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    let l = print(&r, &mut seen, "step 11 (Wired Communication Off, click A)");
    assert!(
        l.iter().any(|x| x.starts_with("led switch=warning ")),
        "{l:?}"
    );
    assert!(
        l.contains(&"pad ignored control=a reason=switch_not_ready".to_owned()),
        "{l:?}"
    );
    assert!(!l.iter().any(|x| x.starts_with("pad sent")), "{l:?}");
    r.ctl(|c| c.set_console_ready(true));
    r.until_open();
    let n = r.direct().len();
    r.click(a);
    r.settle(n + 2);
    let l = print(
        &r,
        &mut seen,
        "step 11 (Wired Communication On, quick click A)",
    );
    assert!(l.iter().any(|x| x.starts_with("led switch=ok ")), "{l:?}");
    assert!(l.iter().any(|x| x.starts_with("gate gamepad=on ")), "{l:?}");
    // Every pad line of the run matches its pattern.
    for l in r.pads() {
        if !l.starts_with("sent ") && !l.starts_with("dropped ") {
            assert!(is_app_pad_line(&l), "{l}");
        }
    }
}
