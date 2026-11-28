//! G.25 (Story 4.9 AC 1, 2, 4–9, 11, 12): the gamepad panel's keyboard
//! focus and keys in the real app on a fake worker thread (an `ocb-sim`
//! controller behind a fake bridge): focus gained by a press on the panel,
//! by Tab and by AccessKit, lost by a click elsewhere, Esc, a dialog and a
//! window blur; the focus lock from the first frame; the keys by physical
//! key (fresh press, 50 ms minimum, OS repeats ignored, chords, duplicates,
//! opposites, the union with the mouse, Cmd/Ctrl left alone, consumption);
//! the release rules with keys held; gated keys; the global keys; the
//! layout switch; the `pad` lines; and the H.36 dry run. Key events are
//! pushed as winit sends them (`physical_key: Some(..)`, `repeat: false`:
//! egui itself flags a press of a held key as a repeat). The UI clock is a
//! `TestClock` and the worker's a `FakeClock`, moved together; waits on the
//! worker thread are bounded polls (≤ 2 s); no wall-clock upper bound is
//! asserted. Never the real backend or the real log (G.13).

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

use common::{TestClock, Wake, is_app_pad_line};
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::app::{App, Mode};
use ocb_app::gamepad::{Control, PadKey, Stick, TAP_MIN_US};
use ocb_app::ui::bottom_bar;
use ocb_app::ui::gamepad::{GROUP_KEYS_ACTIVE, GROUP_KEYS_OFF, panel_id};
use ocb_app::ui::modal::{ModalAction, ModalBody, ModalSpec};
use ocb_core::link::Phase;
use ocb_core::protocol::consts::{
    BTN_A, BTN_B, BTN_L, BTN_MINUS, BTN_PLUS, BTN_R, BTN_X, BTN_Y, BTN_ZL, BTN_ZR, HAT_DOWN,
    HAT_LEFT, HAT_NEUTRAL, HAT_RIGHT, HAT_UP, PROTO_VERSION,
};
use ocb_core::protocol::{NEUTRAL, State};
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

/// The state of a `pad sent` line's text (after `pad `).
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

fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
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

fn with(buttons: u16) -> State {
    State { buttons, ..NEUTRAL }
}

fn hat(h: u8) -> State {
    State { hat: h, ..NEUTRAL }
}

const CMD: Modifiers = Modifiers {
    alt: false,
    ctrl: false,
    shift: false,
    mac_cmd: true,
    command: true,
};

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

    /// The app's `pad` texts so far (not the worker's `sent`/`dropped`).
    fn app_pads(&self) -> Vec<String> {
        self.pads()
            .into_iter()
            .filter(|l| !l.starts_with("sent ") && !l.starts_with("dropped "))
            .collect()
    }

    fn port_open(&self) -> bool {
        let st = self.backend.state();
        let closes = st.closes.iter().filter(|c| c.1 == CU).count();
        st.opens_of(CU).len() > closes
    }

    /// One frame `d_us` later on both clocks (the starvation-safe step of
    /// G.23: with the port open the frame waits for a whole worker pass at
    /// the new time).
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

    /// Frames until nothing is held (mouse or keys) and the controller's
    /// DIRECT log has `n` states.
    fn settle(&mut self, n: usize) {
        let start = Instant::now();
        while self.h.state().pad().held().is_some() || self.h.state().pad().keys_held().count() > 0
        {
            assert!(start.elapsed() < LIMIT, "still held");
            self.frame(FRAME_US);
        }
        self.until_direct(n);
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

    /// The gamepad region (the group node's bounds).
    fn region(&self) -> Rect {
        self.h
            .query_all_by_role(Role::Group)
            .find(|n| {
                n.accesskit_node()
                    .label()
                    .is_some_and(|l| l == GROUP_KEYS_OFF || l == GROUP_KEYS_ACTIVE)
            })
            .expect("the gamepad group")
            .rect()
    }

    /// Empty panel space: right of the tab, above the pad (a painted chip
    /// at most, no widget).
    fn panel_spot(&self) -> Pos2 {
        let r = self.region();
        Pos2::new(r.right() - 15.0, r.top() + 15.0)
    }

    /// The note view's centre (no widget there yet).
    fn note_view_spot(&self) -> Pos2 {
        let r = self.region();
        Pos2::new(r.left() - 300.0, r.center().y)
    }

    /// Focus rings painted around anything but the gamepad region (2 pt
    /// `amber` rect strokes): another widget had the focus in this frame.
    fn foreign_rings(&self) -> usize {
        let region = self.region();
        common::shapes_flat(&self.h.output().shapes)
            .iter()
            .filter(|f| {
                matches!(&f.shape, egui::epaint::Shape::Rect(r)
                    if r.stroke.color == ocb_app::ui::theme::AMBER
                        && (r.stroke.width - 2.0).abs() < 1e-3
                        && !common::rect_eq(r.rect, region, 0.5))
            })
            .count()
    }

    fn focused(&self) -> Option<egui::Id> {
        self.h.ctx.memory(|m| m.focused())
    }

    fn panel_has_focus(&self) -> bool {
        self.focused() == Some(panel_id())
    }

    fn push(&mut self, e: Event) {
        self.h.input_mut().events.push(e);
    }

    /// Esc pressed and released in this frame's input (with `m` held), so
    /// egui never counts the next Esc as a key repeat.
    fn esc(&mut self, m: Modifiers) {
        self.push(esc(m));
        self.push(Event::Key {
            key: Key::Escape,
            physical_key: Some(Key::Escape),
            pressed: false,
            repeat: false,
            modifiers: m,
        });
    }

    fn hover(&mut self, pos: Pos2) {
        self.push(Event::PointerMoved(pos));
        self.frame(0);
    }

    fn press(&mut self, pos: Pos2) {
        self.hover(pos);
        self.push(button(pos, true));
        self.frame(0);
    }

    fn release(&mut self, pos: Pos2) {
        self.push(button(pos, false));
        self.frame(0);
    }

    fn click(&mut self, pos: Pos2) {
        self.hover(pos);
        self.push(button(pos, true));
        self.push(button(pos, false));
        self.frame(0);
    }

    /// A click on empty panel space, then one frame (the lock is set).
    fn focus_panel(&mut self) {
        let p = self.panel_spot();
        self.click(p);
        assert!(self.panel_has_focus(), "the click gave the panel focus");
        self.frame(0);
    }

    fn key(&mut self, k: Key, pressed: bool, m: Modifiers) {
        self.push(key_event(k, pressed, m));
    }

    /// A key pressed in a frame of its own (no time passes).
    fn key_down(&mut self, k: Key) {
        self.key(k, true, Modifiers::NONE);
        self.frame(0);
    }

    /// A key released in a frame of its own (no time passes).
    fn key_up(&mut self, k: Key) {
        self.key(k, false, Modifiers::NONE);
        self.frame(0);
    }

    /// A quick key tap: press and release in one frame.
    fn tap_key(&mut self, k: Key) {
        self.key(k, true, Modifiers::NONE);
        self.key(k, false, Modifiers::NONE);
        self.frame(0);
    }

    /// Frames of `FRAME_US` for at least `us`.
    fn wait_us(&mut self, us: u64) {
        let t = self.ui.now_us();
        while self.ui.now_us() < t + us {
            self.frame(FRAME_US);
        }
    }
}

fn esc(m: Modifiers) -> Event {
    Event::Key {
        key: Key::Escape,
        physical_key: Some(Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: m,
    }
}

fn sample_dialog() -> ModalSpec {
    ModalSpec::new(
        "g25-dialog",
        "A dialog",
        ModalBody::Text("Something to read.".to_owned()),
        ModalAction::new("OK"),
    )
}

/// AC 1: a press on empty panel space, on a control (pressed in the same
/// frame) and on a stick gives the panel focus, kept through the press,
/// the hold and the release; a click elsewhere, an unmodified Esc, a dialog
/// and a window blur (an event, and a hidden window in a `logic`-only
/// frame) take it, one line each; Tab × 4 and an AccessKit `Focus` give it
/// with `source=tab` and `source=other`.
#[test]
fn g25_focus_gain_and_loss() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    // Empty panel space.
    let spot = r.panel_spot();
    r.click(spot);
    assert!(r.panel_has_focus());
    assert!(r.h.state().panel_focused());
    assert_eq!(r.app_pads(), vec!["keys=on source=click"]);
    // Esc.
    r.esc(Modifiers::NONE);
    r.frame(0);
    assert_eq!(r.focused(), None);
    assert_eq!(r.app_pads()[1..], ["keys=off reason=esc"]);
    // A control: pressed in the same frame, focus through hold and release.
    let a = r.center("A");
    r.press(a);
    assert!(r.panel_has_focus(), "focus in the press frame");
    assert_eq!(
        r.h.state().pad().last_sent(),
        with(BTN_A),
        "same-frame press"
    );
    for _ in 0..10 {
        r.frame(FRAME_US);
        assert!(r.panel_has_focus(), "focus through the hold");
    }
    r.release(a);
    r.frame(FRAME_US);
    assert!(r.panel_has_focus(), "focus kept at the release");
    let pads = r.app_pads();
    assert_eq!(
        pads[2..4],
        ["keys=on source=click", "press control=a source=mouse"]
    );
    // A quick click on a control keeps it too.
    r.click(a);
    r.frame(FRAME_US);
    assert!(r.panel_has_focus(), "a quick click keeps the focus");
    // Elsewhere.
    let nv = r.note_view_spot();
    r.click(nv);
    assert_eq!(r.focused(), None);
    assert!(
        r.app_pads()
            .contains(&"keys=off reason=elsewhere".to_owned())
    );
    // A stick.
    let s = r.stick_center(Stick::Left);
    r.press(s);
    assert!(r.panel_has_focus(), "a stick press focuses");
    r.release(s);
    r.frame(FRAME_US);
    assert!(r.panel_has_focus());
    // A dialog takes it.
    r.h.state_mut().modals_mut().request(sample_dialog());
    r.frame(0);
    r.frame(0);
    assert!(!r.panel_has_focus());
    assert!(r.app_pads().contains(&"keys=off reason=dialog".to_owned()));
    r.h.get_by_role_and_label(Role::Button, "OK").click();
    for _ in 0..3 {
        r.frame(FRAME_US);
    }
    assert!(!r.h.state().modals().is_open());
    // Window blur by event.
    r.focus_panel();
    r.push(Event::WindowFocused(false));
    r.frame(FRAME_US);
    assert_eq!(r.focused(), None);
    assert!(
        r.app_pads()
            .contains(&"keys=off reason=window_blur".to_owned())
    );
    r.push(Event::WindowFocused(true));
    r.frame(FRAME_US);
    assert_eq!(r.focused(), None, "focus does not come back by itself");
    // A hidden window, `logic` only.
    r.focus_panel();
    let n = r.app_pads().len();
    let _ = r.h.state_mut().pad_background(true, false, false);
    assert!(!r.h.state().panel_focused());
    r.frame(FRAME_US);
    assert_eq!(r.focused(), None);
    assert_eq!(r.app_pads()[n..], ["keys=off reason=window_blur"]);
    // Tab × 4 from nothing focused.
    for _ in 0..4 {
        r.h.key_press(Key::Tab);
        r.frame(0);
    }
    assert!(r.panel_has_focus());
    assert_eq!(
        r.app_pads().last().map(String::as_str),
        Some("keys=on source=tab")
    );
    r.esc(Modifiers::NONE);
    r.frame(0);
    // AccessKit focus on the group.
    r.h.get_by_role_and_label(Role::Group, GROUP_KEYS_OFF)
        .focus();
    r.frame(0);
    r.frame(0);
    assert!(r.panel_has_focus());
    assert_eq!(
        r.app_pads().last().map(String::as_str),
        Some("keys=on source=other")
    );
    let _ = r.h.get_by_role_and_label(Role::Group, GROUP_KEYS_ACTIVE);
    for l in r.app_pads() {
        assert!(is_app_pad_line(&l), "{l}");
    }
}

/// AC 1: with a dialog open a press at A's centre neither focuses the panel
/// nor presses anything.
#[test]
fn g25_dialog_blocks_focus() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let a = r.center("A");
    r.h.state_mut().modals_mut().request(sample_dialog());
    r.frame(0);
    r.frame(0);
    let in_dialog = r.focused();
    assert!(in_dialog.is_some() && in_dialog != Some(panel_id()));
    r.click(a);
    r.frame(FRAME_US);
    assert_eq!(r.focused(), in_dialog, "the dialog keeps the focus");
    assert!(r.pads().is_empty(), "{:?}", r.pads());
    assert!(r.direct().is_empty());
}

/// AC 2: Tab, Shift+Tab and the arrows keep the focus on the panel, also in
/// the very first frame after a Tab-in or a click, and 10 frames later; no
/// `keys=off`; the arrows press the D-pad; Esc leaves.
#[test]
fn g25_focus_lock() {
    for by_click in [false, true] {
        for (k, m) in [
            (Key::Tab, Modifiers::NONE),
            (Key::Tab, Modifiers::SHIFT),
            (Key::ArrowUp, Modifiers::NONE),
            (Key::ArrowDown, Modifiers::NONE),
            (Key::ArrowLeft, Modifiers::NONE),
            (Key::ArrowRight, Modifiers::NONE),
        ] {
            let ctx = format!("{k:?} {m:?} by_click={by_click}");
            let mut r = rig(vec![bridge()]);
            r.until_open();
            if by_click {
                let p = r.panel_spot();
                r.click(p);
            } else {
                for _ in 0..4 {
                    r.h.key_press(Key::Tab);
                    r.frame(0);
                }
            }
            assert!(r.panel_has_focus(), "{ctx}");
            // The very next frame, then two more frames for a move egui
            // applies one frame late (Shift+Tab, arrows).
            r.key(k, true, m);
            r.key(k, false, m);
            r.frame(0);
            r.frame(0);
            r.frame(0);
            assert!(r.panel_has_focus(), "{ctx}: first frame");
            for _ in 0..10 {
                r.frame(FRAME_US);
            }
            // Later the filter holds the key: focus never leaves the panel,
            // not even for a frame (no other widget paints its focus ring).
            r.key(k, true, m);
            r.key(k, false, m);
            for i in 0..3 {
                r.frame(0);
                assert_eq!(r.foreign_rings(), 0, "{ctx}: later, frame {i}");
            }
            assert!(r.panel_has_focus(), "{ctx}: later");
            if k != Key::Tab {
                r.until_direct(4);
                let hats: Vec<u8> = r.direct().iter().map(|s| s.hat).collect();
                let want = match k {
                    Key::ArrowUp => HAT_UP,
                    Key::ArrowDown => HAT_DOWN,
                    Key::ArrowLeft => HAT_LEFT,
                    _ => HAT_RIGHT,
                };
                assert_eq!(hats, vec![want, HAT_NEUTRAL, want, HAT_NEUTRAL], "{ctx}");
            }
            // A click on the focused panel re-requests its focus (D11); the
            // filter is set after that request, so the key in the very
            // next frame is still held by it.
            let p = r.panel_spot();
            r.click(p);
            r.key(k, true, m);
            r.key(k, false, m);
            for i in 0..3 {
                r.frame(0);
                assert_eq!(r.foreign_rings(), 0, "{ctx}: after a click, frame {i}");
            }
            assert!(r.panel_has_focus(), "{ctx}: after a click");
            let pads = r.app_pads();
            assert!(
                !pads.iter().any(|l| l.starts_with("keys=off")),
                "{ctx}: {pads:?}"
            );
            r.esc(Modifiers::NONE);
            r.frame(0);
            assert_eq!(r.focused(), None, "{ctx}");
            assert_eq!(
                r.app_pads().last().map(String::as_str),
                Some("keys=off reason=esc"),
                "{ctx}"
            );
        }
    }
}

/// The wire state of each key (Dev Notes › Key table).
fn key_table() -> Vec<(Key, PadKey, State)> {
    vec![
        (Key::ArrowUp, PadKey::ArrowUp, hat(HAT_UP)),
        (Key::W, PadKey::W, hat(HAT_UP)),
        (Key::ArrowDown, PadKey::ArrowDown, hat(HAT_DOWN)),
        (Key::S, PadKey::S, hat(HAT_DOWN)),
        (Key::ArrowLeft, PadKey::ArrowLeft, hat(HAT_LEFT)),
        (Key::A, PadKey::A, hat(HAT_LEFT)),
        (Key::ArrowRight, PadKey::ArrowRight, hat(HAT_RIGHT)),
        (Key::D, PadKey::D, hat(HAT_RIGHT)),
        (Key::H, PadKey::H, with(BTN_Y)),
        (Key::J, PadKey::J, with(BTN_B)),
        (Key::K, PadKey::K, with(BTN_X)),
        (Key::L, PadKey::L, with(BTN_A)),
        (Key::Q, PadKey::Q, with(BTN_L)),
        (Key::E, PadKey::E, with(BTN_R)),
        (Key::Z, PadKey::Z, with(BTN_ZL)),
        (Key::C, PadKey::C, with(BTN_ZR)),
        (Key::Enter, PadKey::Enter, with(BTN_PLUS)),
        (Key::Backspace, PadKey::Backspace, with(BTN_MINUS)),
    ]
}

/// AC 4: each of the 18 keys (physical key set) sends exactly its bit or
/// hat, then NEUTRAL at least 50 ms later; the same events without a
/// physical key do nothing; Home, Capture and the sticks have no key.
#[test]
fn g25_every_key_bit() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    let mut want = Vec::new();
    for (k, p, s) in key_table() {
        let t0 = r.ui.now_us();
        r.tap_key(k);
        assert_eq!(r.h.state().pad().last_sent(), s, "{k:?}");
        while r.h.state().pad().keys_held().count() > 0 {
            r.frame(FRAME_US / 4);
        }
        assert!(r.ui.now_us() >= t0 + TAP_MIN_US, "{k:?}");
        want.push(s);
        want.push(NEUTRAL);
        r.settle(want.len());
        assert!(
            r.app_pads().contains(&format!(
                "press control={} source=keyboard key={}",
                p.control().log_name(),
                p.log_name()
            )),
            "{k:?}"
        );
    }
    assert_eq!(r.direct(), want);
    assert_eq!(key_table().len(), 18);
    for c in [Control::Home, Control::Capture] {
        assert!(PadKey::ALL.iter().all(|k| k.control() != c));
    }
    // No physical key: nothing (no fallback to the logical key).
    let n = r.app_pads().len();
    for (k, _, _) in key_table() {
        r.push(Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        r.push(Event::Key {
            key: k,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        r.frame(FRAME_US);
    }
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.app_pads().len(), n, "{:?}", &r.app_pads()[n..]);
    assert_eq!(r.direct(), want);
}

/// AC 5: a key pressed and released in one frame is a 50 ms tap: A in the
/// press frame, NEUTRAL no earlier than 50 ms after it (UI clock), and the
/// app asks for that frame; a long hold ends at the key's release.
#[test]
fn g25_key_tap_and_hold() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    let t0 = r.ui.now_us();
    r.tap_key(Key::L);
    assert_eq!(r.h.state().pad().last_sent(), with(BTN_A));
    assert_eq!(
        r.h.state().last_pad_wake(),
        Some(Duration::from_micros(TAP_MIN_US)),
        "the release frame is requested"
    );
    let mut released_at = None;
    for _ in 0..20 {
        r.frame(FRAME_US / 2);
        if r.h.state().pad().last_sent() == NEUTRAL {
            released_at = Some(r.ui.now_us());
            break;
        }
    }
    let at = released_at.expect("NEUTRAL sent");
    assert!(at >= t0 + TAP_MIN_US, "released at {at}, press at {t0}");
    r.until_direct(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
    let pads = r.app_pads();
    assert!(pads.contains(&"press control=a source=keyboard key=l".to_owned()));
    assert!(pads.contains(&"release control=a reason=tap key=l".to_owned()));
    // A hold.
    r.key_down(Key::L);
    r.wait_us(500_000);
    assert_eq!(r.direct().len(), 3);
    r.key_up(Key::L);
    r.settle(4);
    assert_eq!(r.direct()[2..], [with(BTN_A), NEUTRAL]);
    assert_eq!(
        r.app_pads().last().map(String::as_str),
        Some("release control=a reason=key_up key=l")
    );
}

/// AC 5: a 3 s hold with an OS key repeat every 33 ms is one press: [A,
/// NEUTRAL] only, one `pad press`, no other app `pad` line between press
/// and release.
#[test]
fn g25_key_repeat_ignored() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    let n = r.app_pads().len();
    r.key_down(Key::L);
    let t = r.ui.now_us();
    let mut last = t;
    while r.ui.now_us() < t + 3_000_000 {
        if r.ui.now_us() >= last + 33_000 {
            // A repeat as winit sends it: egui flags it (L is down).
            r.key(Key::L, true, Modifiers::NONE);
            last = r.ui.now_us();
        }
        r.frame(FRAME_US);
    }
    r.key_up(Key::L);
    r.settle(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
    let pads = r.app_pads();
    assert_eq!(
        pads[n..],
        [
            "press control=a source=keyboard key=l",
            "release control=a reason=key_up key=l"
        ]
    );
}

/// AC 5: chords (the union), duplicate keys, opposite keys, and key holds
/// apart from the mouse hold.
#[test]
fn g25_chords_and_union() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    // L + K → 0x000c; K up → 0x0004; L up → NEUTRAL.
    r.key_down(Key::L);
    r.frame(FRAME_US * 4);
    r.key_down(Key::K);
    assert_eq!(r.h.state().pad().last_sent().buttons, 0x000c);
    r.frame(FRAME_US * 4);
    r.key_up(Key::K);
    assert_eq!(r.h.state().pad().last_sent().buttons, 0x0004);
    r.frame(FRAME_US * 4);
    r.key_up(Key::L);
    r.settle(4);
    assert_eq!(
        r.direct(),
        vec![with(0x0004), with(0x000c), with(0x0004), NEUTRAL]
    );
    // W + ArrowUp: one hat=0; W up → nothing sent; ArrowUp up → hat=8.
    r.key_down(Key::W);
    r.frame(FRAME_US * 4);
    r.key_down(Key::ArrowUp);
    r.frame(FRAME_US * 4);
    r.key_up(Key::W);
    r.frame(FRAME_US * 4);
    assert_eq!(r.h.state().pad().last_sent(), hat(HAT_UP), "W up: still up");
    r.key_up(Key::ArrowUp);
    r.settle(6);
    assert_eq!(r.direct()[4..], [hat(HAT_UP), NEUTRAL]);
    // ArrowLeft + D → hat=8; ArrowLeft up → hat=2; D up → hat=8.
    r.key_down(Key::ArrowLeft);
    r.frame(FRAME_US * 4);
    r.key_down(Key::D);
    r.frame(FRAME_US * 4);
    r.key_up(Key::ArrowLeft);
    r.frame(FRAME_US * 4);
    r.key_up(Key::D);
    r.settle(10);
    assert_eq!(
        r.direct()[6..],
        [hat(HAT_LEFT), NEUTRAL, hat(HAT_RIGHT), NEUTRAL]
    );
    // Mouse holds A, key L pressed and released: A stays until the pointer
    // is up.
    let a = r.center("A");
    r.press(a);
    r.frame(FRAME_US);
    r.key_down(Key::L);
    r.frame(FRAME_US * 4);
    r.key_up(Key::L);
    r.frame(FRAME_US * 4);
    assert_eq!(r.h.state().pad().held(), Some(Control::A));
    assert_eq!(r.h.state().pad().last_sent(), with(BTN_A));
    r.release(a);
    r.settle(12);
    assert_eq!(r.direct()[10..], [with(BTN_A), NEUTRAL]);
    // Key L held, mouse clicks B: 0x0006, then 0x0004.
    r.key_down(Key::L);
    r.frame(FRAME_US * 4);
    let b = r.center("B");
    r.press(b);
    assert_eq!(r.h.state().pad().last_sent().buttons, 0x0006);
    r.frame(FRAME_US * 4);
    r.release(b);
    r.frame(FRAME_US);
    assert_eq!(r.h.state().pad().last_sent().buttons, 0x0004);
    r.key_up(Key::L);
    r.settle(16);
    assert_eq!(
        r.direct()[12..],
        [with(0x0004), with(0x0006), with(0x0004), NEUTRAL]
    );
    assert!(r.panel_has_focus(), "the clicks kept the focus");
    assert!(!r.app_pads().iter().any(|l| l.contains("busy")));
}

fn has_key_event(r: &Rig, k: Key) -> bool {
    r.h.ctx.input(|i| {
        i.events
            .iter()
            .any(|e| matches!(e, Event::Key { physical_key: Some(p), .. } if *p == k))
    })
}

/// AC 5, 8: Cmd+L, Ctrl+L and Cmd+Enter press nothing and stay in the
/// input; a release is honoured whatever the modifiers; a release of a key
/// never pressed does nothing; a plain mapped key is consumed while the
/// panel has focus.
#[test]
fn g25_modifiers_and_consumption() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    let n = r.app_pads().len();
    let ctrl = Modifiers {
        ctrl: true,
        ..Modifiers::NONE
    };
    for (k, m) in [(Key::L, CMD), (Key::L, ctrl), (Key::Enter, CMD)] {
        r.key(k, true, m);
        r.frame(0);
        assert!(has_key_event(&r, k), "{k:?} {m:?} stays in the input");
        r.key(k, false, m);
        r.frame(FRAME_US);
    }
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.app_pads().len(), n, "{:?}", &r.app_pads()[n..]);
    assert!(r.direct().is_empty());
    // L held, then Cmd down, L released with Cmd: the release counts.
    r.key_down(Key::L);
    assert!(!has_key_event(&r, Key::L), "a plain L is consumed");
    r.frame(FRAME_US * 5);
    r.key(Key::L, false, CMD);
    r.frame(0);
    r.settle(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
    // A release of K never pressed: nothing.
    let n = r.app_pads().len();
    r.key_up(Key::K);
    r.frame(FRAME_US);
    assert_eq!(r.app_pads().len(), n);
    // Shift and Alt do not block a press.
    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    r.key(Key::H, true, shift);
    r.key(Key::H, false, shift);
    r.frame(0);
    r.settle(4);
    assert_eq!(r.direct()[2..], [with(BTN_Y), NEUTRAL]);
}

/// AC 5 (D6): a key in the frame of the click that gives focus, a key in
/// the Esc frame, and keys with the panel unfocused do nothing.
#[test]
fn g25_focus_frames() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    // Unfocused: nothing.
    r.tap_key(Key::L);
    r.tap_key(Key::Enter);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert!(r.pads().is_empty(), "{:?}", r.pads());
    // In the click's frame.
    let p = r.panel_spot();
    r.hover(p);
    r.push(button(p, true));
    r.push(button(p, false));
    r.key(Key::L, true, Modifiers::NONE);
    r.key(Key::L, false, Modifiers::NONE);
    r.frame(0);
    assert!(r.panel_has_focus());
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.app_pads(), vec!["keys=on source=click"]);
    // In the Esc frame.
    r.esc(Modifiers::NONE);
    r.key(Key::L, true, Modifiers::NONE);
    r.key(Key::L, false, Modifiers::NONE);
    r.frame(0);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(
        r.app_pads(),
        vec!["keys=on source=click", "keys=off reason=esc"]
    );
    assert!(r.direct().is_empty());
}

/// AC 6a: blur and a hidden window end a key hold (`window_blur`, one
/// NEUTRAL) and drop the panel's focus; back in focus an L repeat does
/// nothing.
#[test]
fn g25_blur_releases_keys() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    r.key_down(Key::L);
    r.until_direct(1);
    r.push(Event::WindowFocused(false));
    r.frame(FRAME_US);
    r.until_direct(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
    let pads = r.app_pads();
    let rel = pads
        .iter()
        .position(|l| l == "release control=a reason=window_blur key=l")
        .unwrap_or_else(|| panic!("{pads:?}"));
    assert_eq!(pads[rel + 1], "keys=off reason=window_blur");
    assert_eq!(r.focused(), None);
    r.push(Event::WindowFocused(true));
    r.frame(FRAME_US);
    // The OS repeat after refocus (egui cleared its keys: not flagged).
    r.key_down(Key::L);
    r.key_up(Key::L);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.direct().len(), 2, "keys off after the blur");
    // Hidden, `logic` only.
    r.focus_panel();
    r.key_down(Key::L);
    r.until_direct(3);
    let _ = r.h.state_mut().pad_background(true, false, false);
    assert_eq!(r.h.state().pad().last_sent(), NEUTRAL, "released hidden");
    r.until_direct(4);
    r.frame(FRAME_US);
    assert_eq!(r.focused(), None);
    let pads = r.app_pads();
    assert_eq!(
        pads[pads.len() - 2..],
        [
            "release control=a reason=window_blur key=l",
            "keys=off reason=window_blur"
        ]
    );
}

/// AC 6a′: a click elsewhere and a dialog take the focus and release a
/// key hold with one NEUTRAL (`keys=off` first, then `release …
/// reason=focus_lost`). The firmware dialog opens only on a new
/// connection, when the gate was already blocked (the gate's release and
/// STOP come first; nothing is held when the dialog's `keys=off` comes).
#[test]
fn g25_focus_loss_releases() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    r.key_down(Key::L);
    r.until_direct(1);
    let nv = r.note_view_spot();
    r.click(nv);
    r.until_direct(2);
    assert_eq!(r.direct(), vec![with(BTN_A), NEUTRAL]);
    let pads = r.app_pads();
    let off = pads
        .iter()
        .position(|l| l == "keys=off reason=elsewhere")
        .unwrap_or_else(|| panic!("{pads:?}"));
    assert_eq!(pads[off + 1], "release control=a reason=focus_lost key=l");
    r.key_up(Key::L);
    for _ in 0..3 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.direct().len(), 2, "the L release sends nothing");
    // A dialog.
    r.focus_panel();
    r.key_down(Key::L);
    r.until_direct(3);
    r.h.state_mut().modals_mut().request(sample_dialog());
    r.frame(0);
    r.frame(0);
    r.until_direct(4);
    assert_eq!(r.direct()[2..], [with(BTN_A), NEUTRAL]);
    let pads = r.app_pads();
    let off = pads
        .iter()
        .position(|l| l == "keys=off reason=dialog")
        .unwrap_or_else(|| panic!("{pads:?}"));
    assert_eq!(pads[off + 1], "release control=a reason=focus_lost key=l");

    // The firmware dialog: the panel focused while gated (no bridge yet),
    // then a board whose firmware needs an update connects.
    let late = bridge()
        .controller(ControllerConfig {
            proto_ver: PROTO_VERSION.wrapping_add(1),
            ..ControllerConfig::default()
        })
        .present(vec![(500_000, None)]);
    let mut r = rig(vec![late]);
    for _ in 0..3 {
        r.frame(FRAME_US);
    }
    r.focus_panel();
    r.pump_until("firmware dialog", |l| {
        l.iter().any(|x| x.starts_with("dialog shown "))
    });
    r.frame(0);
    r.frame(0);
    assert!(!r.panel_has_focus());
    let pads = r.app_pads();
    assert!(
        pads.contains(&"keys=off reason=dialog".to_owned()),
        "{pads:?}"
    );
    assert!(
        !pads.iter().any(|l| l.starts_with("release ")),
        "nothing held: {pads:?}"
    );
}

/// AC 6b, 8: STOP while a key is held: Esc (`stop requested`, `release …
/// reason=stop key=l`, `keys=off reason=esc`, in that order, no SetState
/// after), the STOP button by AccessKit (focus as egui leaves it), Shift+Esc
/// (STOP, the panel keeps focus; the next L repeat does nothing, a fresh L
/// presses A).
#[test]
fn g25_stop_releases_keys() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    r.key_down(Key::L);
    r.until_direct(1);
    r.esc(Modifiers::NONE);
    r.frame(0);
    r.pump_until("stop after", |l| {
        l.iter().any(|x| x.starts_with("stop after "))
    });
    r.key_up(Key::L);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.direct(), vec![with(BTN_A)], "nothing sent after STOP");
    let all = r.all_now();
    let at = |p: &str| {
        all.iter()
            .position(|l| l.starts_with(p))
            .unwrap_or_else(|| panic!("{p}: {all:?}"))
    };
    let req = at("stop requested source=esc ");
    let rel = at("pad release control=a reason=stop key=l");
    let off = at("pad keys=off reason=esc");
    assert!(req < rel && rel < off, "{all:?}");
    assert!(
        !all[req..].iter().any(|l| l.starts_with("pad sent")),
        "{all:?}"
    );
    assert_eq!(r.focused(), None);
    // STOP by AccessKit click.
    r.until_open();
    r.focus_panel();
    let n = r.direct().len();
    r.key_down(Key::L);
    r.until_direct(n + 1);
    r.h.get_by_role_and_label(Role::Button, ocb_app::stop::STOP_A11Y_NAME)
        .click_accesskit();
    r.frame(0);
    r.pump_until("button stop", |l| {
        l.iter()
            .any(|x| x.starts_with("stop requested source=button "))
    });
    let pads = r.app_pads();
    assert_eq!(
        pads.iter()
            .filter(|l| *l == "release control=a reason=stop key=l")
            .count(),
        2
    );
    r.key_up(Key::L);
    // Shift+Esc: STOP, the focus stays.
    r.until_open();
    r.focus_panel();
    let n = r.direct().len();
    r.key_down(Key::L);
    r.until_direct(n + 1);
    r.esc(Modifiers::SHIFT);
    r.frame(0);
    assert!(r.panel_has_focus(), "Shift+Esc keeps the focus");
    r.pump_until("shift stop", |l| {
        l.iter()
            .filter(|x| x.starts_with("stop requested source=esc "))
            .count()
            == 2
    });
    assert_eq!(
        r.app_pads()
            .iter()
            .filter(|l| *l == "release control=a reason=stop key=l")
            .count(),
        3
    );
    // The next OS repeat of the still-held L: nothing.
    r.key(Key::L, true, Modifiers::NONE);
    r.frame(FRAME_US);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.direct().len(), n + 1, "a repeat presses nothing");
    // A fresh L.
    r.key_up(Key::L);
    r.until_open();
    r.key_down(Key::L);
    r.until_direct(n + 2);
    assert_eq!(r.direct()[n + 1], with(BTN_A));
}

/// AC 6c, 6d: the gate turning blocked with a key held (`release …
/// reason=gate key=l`, `stop requested source=gamepad`); the controller
/// leaving DIRECT by itself (`reason=controller_idle key=l`, nothing sent).
#[test]
fn g25_block_and_idle_with_keys() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    r.key_down(Key::L);
    r.until_direct(1);
    r.ctl(|c| c.set_console_ready(false));
    r.pump_until("stop after", |l| {
        l.iter().any(|x| x.starts_with("stop after "))
    });
    let all = r.all_now();
    assert!(
        all.contains(&"pad release control=a reason=gate key=l".to_owned()),
        "{all:?}"
    );
    assert!(
        all.iter()
            .any(|l| l.starts_with("stop requested source=gamepad ")),
        "{all:?}"
    );
    assert_eq!(r.direct(), vec![with(BTN_A)]);
    r.key_up(Key::L);
    r.ctl(|c| c.set_console_ready(true));
    r.until_open();
    // Controller idle.
    r.key_down(Key::L);
    r.until_direct(2);
    r.pump_until("direct", |l| {
        l.iter().filter(|x| x.contains("to=direct")).count() >= 2
    });
    r.ctl(|c| c.force_abort(None));
    r.pump_until("released", |l| {
        l.contains(&"pad release control=a reason=controller_idle key=l".to_owned())
    });
    r.key_up(Key::L);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.direct().len(), 2, "nothing sent for it");
}

/// AC 7: with the gate blocked for each reason (as G.23) and the panel
/// focused, a fresh L writes `pad ignored control=a reason=<r> key=l` and
/// sends nothing; its repeats and its release write nothing.
#[test]
fn g25_gated_keys() {
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
        ("switch_not_ready", || {
            (
                vec![bridge().hook(0, Box::new(|c| c.set_console_ready(false)))],
                true,
            )
        }),
    ];
    for (reason, setup) in cases {
        let (ports, link) = setup();
        let has_ctl = !ports.is_empty() && reason != "bridge_not_ready";
        let mut r = rig_with(ports, link);
        let start = Instant::now();
        while r
            .h
            .state()
            .gate()
            .gamepad
            .is_none_or(|b| b.as_str() != reason)
        {
            assert!(start.elapsed() < LIMIT, "{reason}");
            r.frame(FRAME_US);
        }
        for _ in 0..3 {
            r.frame(FRAME_US);
        }
        if r.h.state().modals().is_open() {
            r.h.get_by_role_and_label(Role::Button, "OK").click();
            for _ in 0..3 {
                r.frame(FRAME_US);
            }
        }
        r.focus_panel();
        let n = r.app_pads().len();
        r.key_down(Key::L);
        for _ in 0..3 {
            r.key(Key::L, true, Modifiers::NONE);
            r.frame(FRAME_US);
        }
        r.key_up(Key::L);
        for _ in 0..5 {
            r.frame(FRAME_US);
        }
        let pads = r.app_pads();
        assert_eq!(
            pads[n..],
            [format!("ignored control=a reason={reason} key=l")],
            "{reason}"
        );
        assert!(pads.iter().all(|l| is_app_pad_line(l)), "{pads:?}");
        if has_ctl {
            assert!(r.direct().is_empty(), "{reason}");
        }
        assert!(r.panel_has_focus(), "{reason}: the chip stays (D14)");
    }
}

/// AC 8: with the panel focused Space does nothing, Enter presses +,
/// Backspace −, Cmd+Enter nothing; with STOP focused (Shift+Tab from
/// nothing) Enter is one STOP and no `pad` line.
#[test]
fn g25_global_keys() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    let n = r.all_now().len();
    r.tap_key(Key::Space);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    let all = r.all_now();
    assert!(
        !all[n..]
            .iter()
            .any(|l| l.starts_with("pad ") || l.starts_with("stop ")),
        "{:?}",
        &all[n..]
    );
    r.tap_key(Key::Enter);
    r.settle(2);
    r.tap_key(Key::Backspace);
    r.settle(4);
    assert_eq!(
        r.direct(),
        vec![with(BTN_PLUS), NEUTRAL, with(BTN_MINUS), NEUTRAL]
    );
    r.key(Key::Enter, true, CMD);
    r.key(Key::Enter, false, CMD);
    r.frame(0);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.direct().len(), 4, "Cmd+Enter presses nothing");
    // STOP focused: Enter is STOP, never the pad.
    r.esc(Modifiers::NONE);
    r.frame(0);
    r.pump_until("esc stop", |l| {
        l.iter()
            .any(|x| x.starts_with("stop requested source=esc "))
    });
    r.h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    r.frame(0);
    assert_eq!(r.focused(), Some(bottom_bar::stop_id()));
    let pads = r.app_pads().len();
    r.tap_key(Key::Enter);
    r.pump_until("enter stop", |l| {
        l.iter()
            .any(|x| x.starts_with("stop requested source=button "))
    });
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.app_pads().len(), pads, "no pad line");
    assert_eq!(r.direct().len(), 4);
}

/// AC 9: a stick dragged at 1280 pt ends when the window turns compact
/// (`stick=left end reason=layout`, one SetState with the axes centred);
/// a key hold of a button survives the switch.
#[test]
fn g25_layout_switch() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    let c = r.stick_center(Stick::Left);
    r.press(c);
    r.push(Event::PointerMoved(c + vec2(17.0, 0.0)));
    r.frame(FRAME_US);
    assert_eq!(r.h.state().pad().last_sent().lx, 255);
    r.h.set_size(vec2(1100.0, 800.0));
    r.frame(FRAME_US);
    r.frame(FRAME_US);
    assert_eq!(r.h.state().pad().dragging(), None);
    assert_eq!(r.h.state().pad().last_sent(), NEUTRAL);
    assert_eq!(r.h.query_all_by_role(Role::Slider).count(), 0, "compact");
    let pads = r.app_pads();
    assert!(
        pads.contains(&"stick=left end reason=layout".to_owned()),
        "{pads:?}"
    );
    r.release(c);
    r.until_direct(2);
    for _ in 0..3 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.direct().len(), 2, "the release sends nothing more");
    assert_eq!(r.direct().last(), Some(&NEUTRAL));
    assert_eq!(
        r.direct()
            .iter()
            .filter(|s| s.lx == 128 && s.ly == 128)
            .count(),
        1,
        "{:?}",
        r.direct()
    );
    // A key hold survives the switch back and forth.
    r.h.set_size(vec2(1280.0, 800.0));
    r.frame(FRAME_US);
    r.frame(FRAME_US);
    r.focus_panel();
    let n = r.direct().len();
    r.key_down(Key::L);
    r.until_direct(n + 1);
    let lines = r.app_pads().len();
    r.h.set_size(vec2(1100.0, 800.0));
    r.frame(FRAME_US);
    r.frame(FRAME_US);
    assert!(r.h.state().pad().is_pressed(Control::A), "still held");
    assert_eq!(r.app_pads().len(), lines, "no line");
    assert_eq!(r.direct().len(), n + 1);
    r.key_up(Key::L);
    r.settle(n + 2);
}

/// AC 12: every app `pad` line of a session of keys matches its pattern;
/// the `press … source=keyboard` line precedes the worker's `pad sent`;
/// 100 idle frames with the panel focused write no `pad` line; the
/// patterns reject what they must.
#[test]
fn g25_pad_lines() {
    let mut r = rig(vec![bridge()]);
    r.until_open();
    r.focus_panel();
    let n = r.pads().len();
    for _ in 0..100 {
        r.frame(FRAME_US);
    }
    assert_eq!(r.pads().len(), n, "idle with focus: no pad line");
    r.tap_key(Key::L);
    r.settle(2);
    let pads = r.pads();
    let press = pads
        .iter()
        .position(|l| l == "press control=a source=keyboard key=l")
        .unwrap();
    let sent = pads.iter().position(|l| l.starts_with("sent ")).unwrap();
    assert!(press < sent, "{pads:?}");
    assert!(sent_state(&pads[sent]).is_some_and(|s| s == with(BTN_A)));
    for l in r.app_pads() {
        assert!(is_app_pad_line(&l), "{l}");
    }
    for good in [
        "press control=dup source=keyboard key=arrow_up",
        "release control=minus reason=key_up key=backspace",
        "release control=a reason=focus_lost key=l",
        "release control=a reason=focus_lost",
        "stick=right end reason=layout",
        "stick=left end reason=focus_lost",
        "ignored control=plus reason=no_bridge key=enter",
        "keys=on source=click",
        "keys=on source=tab",
        "keys=on source=other",
        "keys=off reason=esc",
        "keys=off reason=dialog",
        "keys=off reason=window_blur",
        "keys=off reason=elsewhere",
        "press control=a source=mouse",
    ] {
        assert!(is_app_pad_line(good), "{good}");
    }
    for bad in [
        "press control=a source=keyboard",
        "press control=a source=mouse key=l",
        "press control=a source=keyboard key=space",
        "ignored control=a reason=busy key=l",
        "ignored control=stick_left reason=no_bridge key=l",
        "keys=on source=mouse",
        "keys=off reason=tab",
        "keys=on",
        "release control=a reason=key_down key=l",
        "stick=left end reason=key_up",
    ] {
        assert!(!is_app_pad_line(bad), "{bad}");
    }
}

/// The H.36 dry run (Task 9, rig block item 5): the bench steps replayed
/// on the fake worker with an `ocb-sim` controller, the expected lines of
/// each step printed (`cargo test -p ocb-app -- --nocapture h36_dry_run`)
/// and checked. The game's reactions and the owner's ears are the bench's;
/// step 13's tooltip is G.24's; step 14 (close) is the smoke run's and
/// G.13's.
#[test]
fn g25_h36_dry_run() {
    const UNPLUG_AT: u64 = 30_000_000;
    let port = bridge().present(vec![(0, Some(UNPLUG_AT)), (UNPLUG_AT + 2_000_000, None)]);
    let mut r = rig(vec![port]);
    r.until_open();
    let mut seen = 0;
    let print = |r: &Rig, seen: &mut usize, step: &str| -> Vec<String> {
        let all = r.all_now();
        let new: Vec<String> = all[*seen..].to_vec();
        *seen = all.len();
        println!("H.36 {step}:");
        for l in &new {
            println!("  {l}");
        }
        new
    };
    let neutral = format!("pad sent port={CU} buttons=0x0000 hat=8 lx=128 ly=128 rx=128 ry=128");
    let sent_bits =
        |bits: &str| format!("pad sent port={CU} buttons={bits} hat=8 lx=128 ly=128 rx=128 ry=128");
    let setup = print(&r, &mut seen, "setup (start, three LEDs ok, gate on)");
    assert!(
        setup
            .iter()
            .any(|l| l.starts_with("gate gamepad=on gamepad_reason=none ")),
        "{setup:?}"
    );
    // Step 1: click empty panel space; Esc; Tab × 4.
    let spot = r.panel_spot();
    r.click(spot);
    r.frame(FRAME_US);
    let l = print(&r, &mut seen, "step 1a (click an empty spot of the panel)");
    assert_eq!(
        l.iter()
            .filter(|x| x.starts_with("pad "))
            .cloned()
            .collect::<Vec<_>>(),
        ["pad keys=on source=click"],
        "{l:?}"
    );
    r.esc(Modifiers::NONE);
    r.frame(0);
    r.pump_until("step 1b", |l| {
        l[seen..].iter().any(|x| x.starts_with("stop after "))
    });
    let l = print(&r, &mut seen, "step 1b (Esc)");
    let req = l
        .iter()
        .position(|x| x == "stop requested source=esc dialog=none command=queued")
        .unwrap_or_else(|| panic!("{l:?}"));
    let off = l
        .iter()
        .position(|x| x == "pad keys=off reason=esc")
        .unwrap_or_else(|| panic!("{l:?}"));
    assert!(req < off, "{l:?}");
    for _ in 0..4 {
        r.h.key_press(Key::Tab);
        r.frame(0);
    }
    r.frame(FRAME_US);
    let l = print(&r, &mut seen, "step 1c (Tab four times)");
    assert!(l.contains(&"pad keys=on source=tab".to_owned()), "{l:?}");
    assert!(r.panel_has_focus());
    // Step 2: quick L.
    let n0 = r.direct().len();
    let t0 = r.ui.now_us();
    r.tap_key(Key::L);
    let mut t1 = t0;
    while r.h.state().pad().last_sent() != NEUTRAL {
        assert!(r.ui.now_us() < t0 + 1_000_000);
        r.frame(FRAME_US / 2);
        t1 = r.ui.now_us();
    }
    let ms = (t1 - t0) / 1000;
    assert!((49..=70).contains(&ms), "step 2 tap window: {ms} ms");
    r.settle(n0 + 2);
    r.pump_until("step 2 mode", |l| {
        l[seen..].iter().any(|x| x.contains("from=idle to=direct"))
    });
    let l = print(&r, &mut seen, "step 2 (quick L)");
    let pos = |l: &[String], p: &str| {
        l.iter()
            .position(|x| x.starts_with(p))
            .unwrap_or_else(|| panic!("{p}: {l:?}"))
    };
    let press = pos(&l, "pad press control=a source=keyboard key=l");
    let sent_a = pos(&l, &sent_bits("0x0004"));
    let release = pos(&l, "pad release control=a reason=tap key=l");
    let sent_n = pos(&l, &neutral);
    let mode = pos(
        &l,
        &format!("mode port={CU} from=idle to=direct abort_count="),
    );
    assert!(
        press < sent_a && sent_a < sent_n && release < sent_n && sent_a < mode,
        "{l:?}"
    );
    assert!(!l.iter().any(|x| x.starts_with("nak ")), "{l:?}");
    // Step 3: quick ArrowLeft.
    let n = r.direct().len();
    r.tap_key(Key::ArrowLeft);
    r.settle(n + 2);
    let l = print(&r, &mut seen, "step 3 (quick ArrowLeft)");
    let h6 = l
        .iter()
        .position(|x| x.starts_with("pad sent ") && x.contains(" hat=6 "));
    let h8 = l.iter().rposition(|x| *x == neutral);
    assert!(h6.is_some() && h8 > h6, "{l:?}");
    // Step 4: L, K, H, Q, E.
    for (k, name, bits) in [
        (Key::L, "l", "0x0004"),
        (Key::K, "k", "0x0008"),
        (Key::H, "h", "0x0001"),
        (Key::Q, "q", "0x0010"),
        (Key::E, "e", "0x0020"),
    ] {
        let n = r.direct().len();
        r.tap_key(k);
        r.settle(n + 2);
        r.wait_us(200_000);
        let l = print(&r, &mut seen, &format!("step 4 (quick {name})"));
        assert!(
            l.iter().any(|x| x.starts_with("pad press ")
                && x.ends_with(&format!(" source=keyboard key={name}"))),
            "{l:?}"
        );
        let a = pos(&l, &sent_bits(bits));
        assert!(l[a..].contains(&neutral), "{l:?}");
    }
    // Step 5: hold L about 3 s with key repeat.
    let n = r.direct().len();
    r.key_down(Key::L);
    let t = r.ui.now_us();
    let mut last = t;
    while r.ui.now_us() < t + 3_000_000 {
        if r.ui.now_us() >= last + 33_000 && r.ui.now_us() >= t + 500_000 {
            r.key(Key::L, true, Modifiers::NONE);
            last = r.ui.now_us();
        }
        r.frame(FRAME_US);
    }
    r.key_up(Key::L);
    r.settle(n + 2);
    let l = print(&r, &mut seen, "step 5 (hold L about 3 s, key repeat on)");
    let p = pos(&l, "pad press control=a source=keyboard key=l");
    let q = pos(&l, "pad release control=a reason=key_up key=l");
    let between = &l[p + 1..q];
    assert_eq!(
        between
            .iter()
            .filter(|x| x.starts_with("pad sent "))
            .count(),
        1,
        "{l:?}"
    );
    assert!(
        !between.iter().any(|x| x.starts_with("pad press")
            || x.starts_with("pad ignored")
            || x.contains("to=idle")
            || x.starts_with("nak ")
            || x.starts_with("stop ")
            || x.starts_with("reboot ")),
        "{l:?}"
    );
    assert_eq!(l.iter().filter(|x| x.starts_with("pad press")).count(), 1);
    // Step 6: chord L, K; K up; L up.
    let n = r.direct().len();
    r.key_down(Key::L);
    r.frame(FRAME_US * 6);
    r.key_down(Key::K);
    r.frame(FRAME_US * 6);
    r.key_up(Key::K);
    r.frame(FRAME_US * 6);
    r.key_up(Key::L);
    r.settle(n + 4);
    let l = print(
        &r,
        &mut seen,
        "step 6 (chord: hold L, then K; release K, then L)",
    );
    let sents: Vec<&String> = l.iter().filter(|x| x.starts_with("pad sent ")).collect();
    assert_eq!(
        sents
            .iter()
            .map(|x| sent_state(&x[4..]).unwrap().buttons)
            .collect::<Vec<_>>(),
        vec![0x0004, 0x000c, 0x0004, 0x0000],
        "{l:?}"
    );
    // Step 7: Enter; W + ArrowUp; ArrowLeft + D; Enter.
    let n = r.direct().len();
    r.tap_key(Key::Enter);
    r.settle(n + 2);
    r.wait_us(200_000);
    for (first, second) in [(Key::W, Key::ArrowUp), (Key::ArrowLeft, Key::D)] {
        r.key_down(first);
        r.frame(FRAME_US * 6);
        r.key_down(second);
        r.frame(FRAME_US * 6);
        r.key_up(first);
        r.frame(FRAME_US * 6);
        r.key_up(second);
        r.frame(FRAME_US * 6);
    }
    let n = r.direct().len();
    r.tap_key(Key::Enter);
    r.settle(n + 2);
    let l = print(
        &r,
        &mut seen,
        "step 7 (Enter; hold W, then ArrowUp, release W, then ArrowUp; hold ArrowLeft, then D, release ArrowLeft, then D; Enter)",
    );
    let sents: Vec<State> = l
        .iter()
        .filter(|x| x.starts_with("pad sent "))
        .map(|x| sent_state(&x[4..]).unwrap())
        .collect();
    assert_eq!(
        sents,
        vec![
            with(BTN_PLUS),
            NEUTRAL,
            hat(HAT_UP),
            NEUTRAL,
            hat(HAT_LEFT),
            NEUTRAL,
            hat(HAT_RIGHT),
            NEUTRAL,
            with(BTN_PLUS),
            NEUTRAL
        ],
        "{l:?}"
    );
    // Step 8: hold L, Esc, release L, a new L.
    let n = r.direct().len();
    r.key_down(Key::L);
    r.until_direct(n + 1);
    r.wait_us(1_000_000);
    r.esc(Modifiers::NONE);
    r.frame(0);
    r.pump_until("step 8", |l| {
        l[seen..].iter().any(|x| x.starts_with("stop after "))
            && l[seen..].iter().any(|x| x.contains("from=direct to=idle"))
    });
    r.key_up(Key::L);
    r.frame(FRAME_US);
    r.tap_key(Key::L);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    let l = print(
        &r,
        &mut seen,
        "step 8 (hold L, Esc, release L; a new L press)",
    );
    let req = pos(&l, "stop requested source=esc dialog=none command=queued");
    let rel = pos(&l, "pad release control=a reason=stop key=l");
    let off = pos(&l, "pad keys=off reason=esc");
    assert!(req < rel && rel < off, "{l:?}");
    let (sent, acked, mode, after) = (
        pos(&l, &format!("stop sent port={CU}")),
        pos(&l, &format!("stop acked port={CU} after_us=")),
        l.iter()
            .position(|x| x.contains("from=direct to=idle"))
            .unwrap(),
        pos(&l, &format!("stop after port={CU} mode=idle ")),
    );
    assert!(sent < acked && acked < mode && mode < after, "{l:?}");
    let after_us: u64 = l[acked]
        .split(' ')
        .find_map(|w| w.strip_prefix("after_us="))
        .unwrap()
        .parse()
        .unwrap();
    assert!(after_us <= 100_000, "{after_us}");
    assert!(!l[req..].iter().any(|x| x.starts_with("pad sent")), "{l:?}");
    assert!(
        !l[off + 1..].iter().any(|x| x.starts_with("pad ")),
        "the L release and the new L write nothing: {l:?}"
    );
    // Step 9: click the panel, hold L, click the note view, release L.
    r.until_open();
    let n = r.direct().len();
    let spot = r.panel_spot();
    r.click(spot);
    r.frame(0);
    r.key_down(Key::L);
    r.until_direct(n + 1);
    r.frame(FRAME_US * 6);
    let nv = r.note_view_spot();
    r.click(nv);
    r.until_direct(n + 2);
    r.key_up(Key::L);
    r.frame(FRAME_US);
    let l = print(
        &r,
        &mut seen,
        "step 9 (click the panel, hold L, click the note view, release L)",
    );
    let order = [
        "pad keys=on source=click".to_owned(),
        "pad press control=a source=keyboard key=l".to_owned(),
        sent_bits("0x0004"),
        "pad keys=off reason=elsewhere".to_owned(),
        "pad release control=a reason=focus_lost key=l".to_owned(),
        neutral.clone(),
    ];
    let at: Vec<usize> = order.iter().map(|p| pos(&l, p)).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{l:?}");
    assert!(
        !l[at[5] + 1..].iter().any(|x| x.starts_with("pad ")),
        "{l:?}"
    );
    // Step 10: click the panel, hold L, Cmd+Tab away, release L, back.
    let n = r.direct().len();
    let spot = r.panel_spot();
    r.click(spot);
    r.frame(0);
    r.key_down(Key::L);
    r.until_direct(n + 1);
    r.push(Event::WindowFocused(false));
    r.frame(FRAME_US);
    r.key_up(Key::L);
    r.until_direct(n + 2);
    r.push(Event::WindowFocused(true));
    r.frame(FRAME_US);
    r.tap_key(Key::L);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    let l = print(
        &r,
        &mut seen,
        "step 10 (click the panel, hold L, Cmd+Tab away, release L, back; an L press)",
    );
    let rel = pos(&l, "pad release control=a reason=window_blur key=l");
    let off = pos(&l, "pad keys=off reason=window_blur");
    assert!(rel < off, "{l:?}");
    assert_eq!(l.iter().filter(|x| **x == neutral).count(), 1, "{l:?}");
    // After `keys=off`: the worker's NEUTRAL at most, no app line (the L
    // release and the new L press write nothing).
    assert!(
        !l[off + 1..]
            .iter()
            .any(|x| x.starts_with("pad ") && !x.starts_with("pad sent ")),
        "{l:?}"
    );
    assert!(!r.panel_has_focus(), "the ring is gone");
    // Step 10b: the same with Cmd+H (hidden: `logic` only).
    let n = r.direct().len();
    let spot = r.panel_spot();
    r.click(spot);
    r.frame(0);
    r.key_down(Key::L);
    r.until_direct(n + 1);
    let _ = r.h.state_mut().pad_background(true, false, false);
    r.key_up(Key::L);
    r.until_direct(n + 2);
    r.frame(FRAME_US);
    r.tap_key(Key::L);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    let l = print(
        &r,
        &mut seen,
        "step 10b (click the panel, hold L, Cmd+H, release L, back; an L press)",
    );
    let rel = pos(&l, "pad release control=a reason=window_blur key=l");
    let off = pos(&l, "pad keys=off reason=window_blur");
    assert!(rel < off, "{l:?}");
    assert_eq!(l.iter().filter(|x| **x == neutral).count(), 1, "{l:?}");
    // After `keys=off`: the worker's NEUTRAL at most, no app line (the L
    // release and the new L press write nothing).
    assert!(
        !l[off + 1..]
            .iter()
            .any(|x| x.starts_with("pad ") && !x.starts_with("pad sent ")),
        "{l:?}"
    );
    // Step 11: click the panel, Tab × 3, Shift+Tab; Space; Cmd+Enter;
    // Backspace.
    let spot = r.panel_spot();
    r.click(spot);
    r.frame(0);
    for _ in 0..3 {
        r.h.key_press(Key::Tab);
        r.frame(0);
    }
    r.h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    r.frame(0);
    r.frame(0);
    assert!(r.panel_has_focus(), "the ring stays on the panel");
    r.tap_key(Key::Space);
    r.key(Key::Enter, true, CMD);
    r.key(Key::Enter, false, CMD);
    r.frame(0);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    let l = print(
        &r,
        &mut seen,
        "step 11a (click the panel, Tab x3, Shift+Tab, Space, Cmd+Enter)",
    );
    assert_eq!(
        l.iter()
            .filter(|x| x.starts_with("pad ") || x.starts_with("stop "))
            .cloned()
            .collect::<Vec<_>>(),
        ["pad keys=on source=click"],
        "{l:?}"
    );
    let n = r.direct().len();
    r.tap_key(Key::Backspace);
    r.settle(n + 2);
    let l = print(&r, &mut seen, "step 11b (Backspace)");
    let p = pos(&l, "pad press control=minus source=keyboard key=backspace");
    let s = pos(&l, &sent_bits("0x0100"));
    assert!(p < s && l[s..].contains(&neutral), "{l:?}");
    // Step 12: bridge out, L ignored, bridge back, quick L.
    while r.clock.now_us() < UNPLUG_AT {
        r.frame(FRAME_US);
    }
    r.pump_until("step 12 out", |l| {
        l[seen..]
            .iter()
            .any(|x| x.starts_with("gate gamepad=off gamepad_reason=no_bridge "))
    });
    r.frame(FRAME_US);
    assert!(r.panel_has_focus(), "the panel keeps its focus");
    r.tap_key(Key::L);
    for _ in 0..5 {
        r.frame(FRAME_US);
    }
    let l = print(&r, &mut seen, "step 12a (bridge out, L)");
    assert!(
        l.iter()
            .any(|x| x.starts_with("port_lost ") || x.starts_with("port_vanished ")),
        "{l:?}"
    );
    assert!(
        l.contains(&"pad ignored control=a reason=no_bridge key=l".to_owned()),
        "{l:?}"
    );
    let gates: Vec<&String> = l.iter().filter(|x| x.starts_with("gate ")).collect();
    assert!(
        gates.iter().all(|x| x.contains("gamepad_reason=no_bridge ")
            || x.contains("gamepad_reason=bridge_not_ready ")),
        "{gates:?}"
    );
    assert!(
        !l.iter()
            .any(|x| x.starts_with("pad sent") || x.starts_with("stop ")),
        "{l:?}"
    );
    r.until_open();
    let n = r.direct().len();
    r.tap_key(Key::L);
    r.settle(n + 2);
    let l = print(&r, &mut seen, "step 12b (bridge back, quick L)");
    assert!(l.iter().any(|x| x.starts_with("gate gamepad=on ")), "{l:?}");
    let p = pos(&l, "pad press control=a source=keyboard key=l");
    assert!(l[p..].contains(&sent_bits("0x0004")), "{l:?}");
    // Step 13: compact (about 1100 pt): click A, quick L; back to 1280.
    r.h.set_size(vec2(1100.0, 800.0));
    r.frame(FRAME_US);
    r.frame(FRAME_US);
    assert_eq!(r.h.query_all_by_role(Role::Slider).count(), 0, "compact");
    let n = r.direct().len();
    let a = r.center("A");
    r.click(a);
    r.settle(n + 2);
    r.frame(0);
    if !r.panel_has_focus() {
        let spot = r.panel_spot();
        r.click(spot);
        r.frame(0);
    }
    let n = r.direct().len();
    r.tap_key(Key::L);
    r.settle(n + 2);
    r.h.set_size(vec2(1280.0, 800.0));
    r.frame(FRAME_US);
    r.frame(FRAME_US);
    assert_eq!(r.h.query_all_by_role(Role::Slider).count(), 2, "full again");
    let l = print(
        &r,
        &mut seen,
        "step 13 (about 1100 pt: click A, quick L; back to 1280)",
    );
    assert_eq!(
        l.iter().filter(|x| **x == sent_bits("0x0004")).count(),
        2,
        "{l:?}"
    );
    assert!(
        l.contains(&"pad press control=a source=mouse".to_owned()),
        "{l:?}"
    );
    assert!(
        l.contains(&"pad press control=a source=keyboard key=l".to_owned()),
        "{l:?}"
    );
    // Every app pad line of the run matches its pattern.
    for l in r.app_pads() {
        assert!(is_app_pad_line(&l), "{l}");
    }
}
