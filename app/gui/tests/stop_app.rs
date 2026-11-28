//! G.20 (Story 4.7 AC 4–8, 10, 11, 13): STOP in the real app on a fake
//! worker thread (an `ocb-sim` controller behind a fake bridge): Esc read
//! at the top of every frame in every mode and every state without being
//! consumed, key repeats ignored and one STOP per frame, STOP by click,
//! Enter and AccessKit, Space never clicking, harmless without a link or a
//! worker, the Tab order, the `stop` lines of the app and the worker in one
//! log, and the H.34 dry run. Waits on the worker thread are bounded polls
//! (≤ 2 s); the worker's clock is fake. Never the real backend or the real
//! log (G.13).

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

use common::{Pat, TestClock, Wake};
use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use ocb_app::app::{App, Mode};
use ocb_app::leds::LedState;
use ocb_app::stop::{self, StopSource};
use ocb_app::ui::bottom_bar;
use ocb_app::ui::keys::{self, FrameKeys};
use ocb_app::ui::modal::ModalOutcome;
use ocb_app::ui::status_strip::led_id;
use ocb_app::ui::{firmware, gallery};
use ocb_core::link::Phase;
use ocb_core::protocol::consts::PROTO_VERSION;
use ocb_services::log::{LogHandle, Logger, MemorySink};
use ocb_services::sim::{BlockingBackend, FakeBackend, FakePort};
use ocb_services::worker::{Clock as _, FakeClock, RealClock, spawn_with};
use ocb_sim::ControllerConfig;

const CU: &str = "/dev/cu.usbmodem2101";
const SERIAL: &str = "53032847189C2E9C";
const LIMIT: Duration = Duration::from_secs(2);

fn bridge() -> FakePort {
    FakePort::bridge(CU, SERIAL, Some("Bonooru Bridge"))
}

fn mismatched() -> FakePort {
    bridge().controller(ControllerConfig {
        proto_ver: PROTO_VERSION.wrapping_add(1),
        ..ControllerConfig::default()
    })
}

/// The app's `stop requested` line pattern (Dev Notes › Log lines).
fn is_requested(line: &str) -> bool {
    common::line_matches(
        line,
        "requested",
        &[
            ("source", Pat::OneOf(&["esc", "button", "gamepad"])),
            ("dialog", Pat::OneOf(&["open", "none"])),
            (
                "command",
                Pat::OneOf(&["queued", "no_worker", "worker_gone", "queue_full"]),
            ),
        ],
    )
}

struct Rig {
    h: Harness<'static, App>,
    clock: FakeClock,
    published: Arc<AtomicU64>,
    logger: Option<Logger>,
    sink: MemorySink,
    log: LogHandle,
}

fn rig_with(ports: Vec<FakePort>, mode: Mode, wake: Wake) -> Rig {
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
    let (c, p) = (clock.clone(), Arc::clone(&published));
    let mut slot = Some(backend);
    let h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            let handle = common::fake_worker_logged(
                &cc.egui_ctx,
                slot.take().unwrap(),
                c.clone(),
                Arc::clone(&p),
                wake,
                log.clone(),
            );
            App::with_link(cc, mode, handle)
                .ui_clock(ui.ui_clock())
                .led_log(log.clone())
        });
    Rig {
        h,
        clock,
        published,
        logger: Some(logger),
        sink,
        log: app_log,
    }
}

fn rig(ports: Vec<FakePort>) -> Rig {
    rig_with(ports, Mode::Main, Wake::App)
}

impl Rig {
    /// Every line so far (`<kind> <text>`, no stamps), without stopping the
    /// log: a marker is written and waited for (the log is FIFO).
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

    /// The `stop` texts so far.
    fn stops(&self) -> Vec<String> {
        self.all_now()
            .iter()
            .filter_map(|l| l.strip_prefix("stop ").map(str::to_owned))
            .collect()
    }

    /// Moves the worker clock 1 ms per frame until `f` holds on the lines
    /// (bounded, 2 s of real time).
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
            self.clock.advance(1_000);
            std::thread::sleep(Duration::from_millis(1));
            self.h.step();
        }
    }

    /// Runs until the session is connected and the LEDs are logged ok.
    fn until_connected(&mut self) {
        self.pump_until("connected", |_| true);
        let start = Instant::now();
        while self
            .h
            .state()
            .link_snapshot()
            .is_none_or(|s| s.phase != Phase::Connected || s.status.is_none())
        {
            assert!(start.elapsed() < LIMIT, "never connected");
            self.clock.advance(1_000);
            std::thread::sleep(Duration::from_millis(1));
            self.h.step();
        }
        self.pump_until("leds ok", |l| leds_ok(l) >= 1);
    }

    /// Waits until the worker published nothing for 50 ms of real time.
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

    fn led_states(&self) -> Vec<(LedState, Option<String>)> {
        led_states_of(self.h.state())
    }

    fn stop_center(&self) -> egui::Pos2 {
        self.h
            .get_by_role_and_label(Role::Button, stop::STOP_A11Y_NAME)
            .rect()
            .center()
    }

    /// A mouse click on STOP over three frames (move, press, release).
    fn click_stop(&mut self) {
        let pos = self.stop_center();
        self.h.input_mut().events.push(Event::PointerMoved(pos));
        self.h.step();
        self.h.input_mut().events.push(press(pos, true));
        self.h.step();
        self.h.input_mut().events.push(press(pos, false));
        self.h.step();
    }

    /// Shift+Tab once from nothing focused: the ring on STOP (Story 4.9:
    /// Tab × 4 now reaches the gamepad panel, which keeps Tab).
    fn focus_stop(&mut self) {
        self.h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
        self.h.step();
        assert_eq!(
            self.h.ctx.memory(|m| m.focused()),
            Some(bottom_bar::stop_id())
        );
    }
}

/// How many times all three LEDs have turned ok (the `switch` LED's ok
/// line, after the other two).
fn leds_ok(lines: &[String]) -> usize {
    let mut ok = [false; 3];
    let mut n = 0;
    for l in lines {
        for (i, tag) in ["pc_bridge", "bridge_controller", "switch"]
            .iter()
            .enumerate()
        {
            if let Some(rest) = l.strip_prefix(&format!("led {tag}=")) {
                ok[i] = rest.starts_with("ok ");
                if ok == [true; 3] && i == 2 {
                    n += 1;
                }
            }
        }
    }
    n
}

/// The three LEDs' state and detail (no ages).
fn led_states_of(app: &App) -> Vec<(LedState, Option<String>)> {
    let l = app.leds();
    [l.pc_bridge, l.bridge_controller, l.switch]
        .into_iter()
        .map(|x| (x.state, x.detail))
        .collect()
}

fn press(pos: egui::Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn esc(repeat: bool) -> Event {
    Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers: Modifiers::NONE,
    }
}

fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

fn count(lines: &[String], prefix: &str) -> usize {
    lines.iter().filter(|l| l.starts_with(prefix)).count()
}

/// AC 4: with a dialog open one Esc does both: the STOP is requested
/// before the dialog consumes the Esc (`dialog=open`), the dialog closes
/// `Dismissed`, and the worker sends STOP. The gallery's readiness sample
/// is used (its own id): the firmware dialog's id would make the main
/// screen log a `dialog closed` with no `shown`.
#[test]
fn g20_esc_with_dialog_open() {
    let mut r = rig(vec![bridge()]);
    r.until_connected();
    let spec = gallery::readiness_dialog();
    let id = spec.id;
    r.h.state_mut().modals_mut().request(spec);
    for _ in 0..3 {
        r.h.step();
    }
    assert!(r.h.state().modals().is_open());
    // One frame: Esc down.
    r.h.input_mut().events.push(esc(false));
    r.h.step();
    assert!(!r.h.state().modals().is_open(), "the dialog took the Esc");
    // Written in that frame (the worker may already have answered).
    let stops = r.stops();
    assert!(!stops.is_empty(), "no stop line: the Esc was not read");
    assert_eq!(
        stops[0], "requested source=esc dialog=open command=queued",
        "{stops:?}"
    );
    assert_eq!(count(&stops, "requested"), 1, "{stops:?}");
    assert_eq!(
        r.h.state_mut().modals_mut().take_outcome(id),
        Some(ModalOutcome::Dismissed)
    );
    assert_eq!(count(&r.all_now(), "dialog "), 0, "no dialog line");
    r.pump_until("stop sent", |l| l.contains(&format!("stop sent port={CU}")));
}

/// AC 4 with Story 4.6: Esc in the frame the firmware dialog arrives: the
/// STOP is requested before `Modals::show` (`dialog=open`); the dialog
/// ignores that Esc and stays (4.6 behaviour unchanged); the incompatible
/// session writes nothing (`skipped reason=incompatible`).
#[test]
fn g20_esc_in_the_firmware_arrival_frame() {
    let mut r = rig_with(vec![mismatched()], Mode::Main, Wake::CountOnly);
    r.h.step();
    for _ in 0..200 {
        r.clock.advance(1_000);
        std::thread::sleep(Duration::from_millis(1));
    }
    r.quiet();
    assert!(!r.h.state().modals().is_open());
    // Esc down in the draining frame, up in the next.
    r.h.key_press(Key::Escape);
    r.h.step();
    r.h.step();
    assert!(r.h.state().modals().is_open(), "dismissed by an unseen Esc");
    assert_eq!(r.h.state().modals().open_id(), Some(firmware::dialog_id()));
    let lines = r.all_now();
    let dialog: Vec<&String> = lines.iter().filter(|l| l.starts_with("dialog ")).collect();
    assert_eq!(dialog.len(), 1, "{dialog:?}");
    assert!(dialog[0].starts_with("dialog shown"), "{dialog:?}");
    r.pump_until("skipped", |l| {
        l.iter().any(|x| x.starts_with("stop skipped"))
    });
    assert_eq!(
        r.stops(),
        vec![
            "requested source=esc dialog=open command=queued".to_owned(),
            format!("skipped reason=incompatible port={CU}"),
        ]
    );
}

/// The harness of `g20_frame_start_keys`: `frame_start`, then a text edit.
#[derive(Default)]
struct KeysState {
    keys: FrameKeys,
    esc_left: bool,
    space_left: bool,
    cmd_space_left: bool,
    text: String,
    focus: bool,
}

/// AC 4, 6: `frame_start` reads Esc without consuming it (also when a text
/// field had focus: egui's `Memory::begin_pass` clears that focus on Esc
/// before the read, and Esc is read anyway), consumes Space (Cmd/Ctrl+Space
/// only while a widget has focus), and leaves Space to a focused text edit.
#[test]
fn g20_frame_start_keys() {
    let edit_id = egui::Id::unique("g20-edit");
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 200.0))
        .build_ui_state(
            move |ui, s: &mut KeysState| {
                // First pass only, as the app does (a discarded pass reruns
                // the frame with no input events).
                let first = ui.ctx().current_pass_index() == 0;
                if first {
                    s.keys = keys::frame_start(ui.ctx());
                }
                let left = |k: Key, cmd: bool| {
                    ui.input(|i| {
                        i.events.iter().any(|e| {
                            matches!(e, Event::Key { key, pressed: true, modifiers, .. }
                                if *key == k && modifiers.command == cmd)
                        })
                    })
                };
                if first {
                    s.esc_left = left(Key::Escape, false);
                    s.space_left = left(Key::Space, false);
                    s.cmd_space_left = left(Key::Space, true);
                }
                let label = ui.label("Edit");
                let resp = ui
                    .add(egui::TextEdit::singleline(&mut s.text).id(edit_id))
                    .labelled_by(label.id);
                if s.focus {
                    resp.request_focus();
                }
            },
            KeysState::default(),
        );
    h.run();
    // No text edit focused: Esc read, kept; Space consumed; Cmd+Space kept.
    h.input_mut().events.push(esc(false));
    h.input_mut()
        .events
        .push(key_event(Key::Space, true, Modifiers::NONE));
    h.input_mut()
        .events
        .push(key_event(Key::Space, true, Modifiers::COMMAND));
    release(&mut h, &[Key::Escape, Key::Space]);
    h.step();
    assert!(h.state().keys.stop);
    assert!(h.state().esc_left, "Esc consumed");
    assert!(!h.state().space_left, "Space not consumed");
    assert!(h.state().cmd_space_left, "Cmd+Space consumed");
    // A focused text edit: Esc still read and kept, Space reaches it.
    h.state_mut().focus = true;
    h.run();
    assert!(h.ctx.text_edit_focused());
    h.state_mut().focus = false;
    h.input_mut()
        .events
        .push(key_event(Key::Space, true, Modifiers::NONE));
    h.input_mut().events.push(Event::Text(" ".to_owned()));
    release(&mut h, &[Key::Space]);
    h.step();
    assert!(!h.state().keys.stop);
    assert!(h.state().space_left, "Space taken from the text edit");
    assert_eq!(h.state().text, " ", "the text edit got its space");
    assert!(
        h.ctx.text_edit_focused(),
        "the text edit has focus before the Esc"
    );
    h.input_mut().events.push(esc(false));
    h.step();
    assert!(h.state().keys.stop, "Esc with a focused text edit");
    assert!(h.state().esc_left);
    // Esc still held: the next press is a repeat (egui marks a press of a
    // key that is down as a repeat), no STOP.
    h.input_mut().events.push(esc(true));
    h.step();
    assert!(!h.state().keys.stop, "a repeat is not a fresh press");
    release(&mut h, &[Key::Escape]);
    h.step();
}

/// Key releases in the next frame's input.
fn release<S>(h: &mut Harness<'_, S>, keys: &[Key]) {
    for k in keys {
        h.input_mut()
            .events
            .push(key_event(*k, false, Modifiers::NONE));
    }
}

/// AC 4: a frame with a fresh Esc, three repeats and a STOP click asks
/// once; a frame of repeats alone asks nothing.
#[test]
fn g20_esc_repeat_and_coalesce() {
    let mut r = rig(vec![]);
    r.h.step();
    let pos = r.stop_center();
    r.h.input_mut().events.push(Event::PointerMoved(pos));
    r.h.step();
    r.h.input_mut().events.push(press(pos, true));
    r.h.step();
    r.h.input_mut().events.push(esc(false));
    for _ in 0..3 {
        r.h.input_mut().events.push(esc(true));
    }
    r.h.input_mut().events.push(press(pos, false));
    r.h.step();
    let requested: Vec<String> = r
        .stops()
        .into_iter()
        .filter(|l| l.starts_with("requested"))
        .collect();
    assert_eq!(
        requested,
        vec!["requested source=esc dialog=none command=queued"]
    );
    for _ in 0..3 {
        r.h.input_mut().events.push(esc(true));
    }
    r.h.step();
    let requested = count(&r.stops(), "requested");
    assert_eq!(requested, 1, "repeats alone requested STOP");
}

/// AC 4 (D3): Esc is STOP in the gallery too.
#[test]
fn g20_esc_in_gallery() {
    let mut r = rig_with(vec![], Mode::Gallery, Wake::App);
    r.h.step();
    r.h.key_press(Key::Escape);
    r.h.step();
    r.pump_until("skipped", |l| {
        l.iter().any(|x| x.starts_with("stop skipped"))
    });
    assert_eq!(
        r.stops(),
        vec![
            "requested source=esc dialog=none command=queued",
            "skipped reason=no_port port=-"
        ]
    );
}

/// AC 5: a click, Enter with focus and an AccessKit click each request STOP
/// once, and the worker handles each command.
#[test]
fn g20_click_enter_accesskit() {
    let mut r = rig(vec![bridge()]);
    r.until_connected();
    r.click_stop();
    r.pump_until("click handled", |l| count_worker(l) == 1);
    r.focus_stop();
    r.h.key_press(Key::Enter);
    r.h.step();
    r.pump_until("enter handled", |l| count_worker(l) == 2);
    r.h.get_by_role_and_label(Role::Button, stop::STOP_A11Y_NAME)
        .click_accesskit();
    r.h.step();
    r.pump_until("accesskit handled", |l| count_worker(l) == 3);
    let stops = r.stops();
    let requested: Vec<&String> = stops
        .iter()
        .filter(|l| l.starts_with("requested"))
        .collect();
    assert_eq!(requested.len(), 3, "{stops:?}");
    assert!(
        requested
            .iter()
            .all(|l| *l == "requested source=button dialog=none command=queued"),
        "{stops:?}"
    );
    assert!(
        stops
            .iter()
            .all(|l| !l.starts_with("requested") || is_requested(l))
    );
}

/// Worker lines that handle a command (`sent` or `pending`).
fn count_worker(lines: &[String]) -> usize {
    lines
        .iter()
        .filter(|l| l.starts_with("stop sent ") || l.starts_with("stop pending "))
        .count()
}

/// AC 5, 6: with STOP focused, Space with any modifiers (none, Shift,
/// Ctrl, Cmd) clicks nothing; Enter does, once per fresh press even when
/// held (its key repeats are dropped).
#[test]
fn g20_space_never_clicks() {
    let mut r = rig(vec![]);
    r.h.step();
    r.focus_stop();
    for m in [
        Modifiers::NONE,
        Modifiers::SHIFT,
        Modifiers::CTRL,
        Modifiers::COMMAND,
        Modifiers::MAC_CMD,
    ] {
        r.h.key_press_modifiers(m, Key::Space);
        r.h.step();
    }
    assert_eq!(r.h.ctx.memory(|m| m.focused()), Some(bottom_bar::stop_id()));
    assert_eq!(count(&r.stops(), "requested"), 0, "Space clicked STOP");
    r.h.key_press(Key::Enter);
    r.h.step();
    assert_eq!(
        r.stops()
            .into_iter()
            .filter(|l| l.starts_with("requested"))
            .collect::<Vec<_>>(),
        vec!["requested source=button dialog=none command=queued"]
    );
    // Enter held: one fresh press, then presses egui marks as repeats.
    assert_eq!(r.h.ctx.memory(|m| m.focused()), Some(bottom_bar::stop_id()));
    for _ in 0..10 {
        r.h.input_mut()
            .events
            .push(key_event(Key::Enter, true, Modifiers::NONE));
        r.h.step();
    }
    release(&mut r.h, &[Key::Enter]);
    r.h.step();
    assert_eq!(
        count(&r.stops(), "requested"),
        2,
        "a held Enter requested STOP more than once"
    );
}

/// AC 8: no worker, a dead worker and a full command queue: logged, no
/// panic, each frame well under 100 ms, LEDs unchanged.
#[test]
fn g20_stop_without_worker() {
    fn frame_ms(h: &mut Harness<'static, App>) -> u128 {
        let t = Instant::now();
        h.step();
        t.elapsed().as_millis()
    }
    // No worker.
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let log = logger.handle();
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| App::without_link(cc, Mode::Main).led_log(log.clone()));
    h.step();
    let before = h.state().leds();
    h.key_press(Key::Escape);
    assert!(frame_ms(&mut h) < 100);
    h.step();
    assert_eq!(h.state().leds(), before, "LEDs unchanged");
    assert!(!h.state().modals().is_open(), "no dialog");
    drop(h);
    assert!(logger.stop());
    assert_eq!(
        common::kind_texts(&sink.lines(), "stop"),
        vec!["requested source=esc dialog=none command=no_worker"]
    );
    // A dead worker.
    let mut r = rig(vec![]);
    r.h.step();
    assert!(r.h.state_mut().shut_down_link());
    r.h.step();
    let before = r.led_states();
    r.h.key_press(Key::Escape);
    assert!(frame_ms(&mut r.h) < 100);
    r.h.step();
    assert_eq!(r.led_states(), before);
    assert!(!r.h.state().modals().is_open(), "no dialog");
    assert_eq!(
        r.stops(),
        vec!["requested source=esc dialog=none command=worker_gone"]
    );
    // A full queue: the worker is stuck in a 2 s listing.
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let log = logger.handle();
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            let handle = spawn_with(
                BlockingBackend {
                    delay: Duration::from_secs(2),
                },
                RealClock::new(),
                ocb_core::device::Rules::MACOS,
                LogHandle::disabled(),
                Box::new(|| {}),
            )
            .unwrap();
            App::with_link(cc, Mode::Main, handle).led_log(log.clone())
        });
    h.step();
    std::thread::sleep(Duration::from_millis(50));
    let before = led_states_of(h.state());
    for _ in 0..64 {
        h.state_mut().request_stop(StopSource::Esc);
    }
    h.key_press(Key::Escape);
    assert!(frame_ms(&mut h) < 100, "the UI waited on the worker");
    h.step();
    assert_eq!(led_states_of(h.state()), before, "LEDs unchanged");
    assert!(!h.state().modals().is_open(), "no dialog");
    drop(h);
    assert!(logger.stop());
    let stops = common::kind_texts(&sink.lines(), "stop");
    assert_eq!(stops.len(), 65);
    assert!(
        stops[..64]
            .iter()
            .all(|l| l == "requested source=esc dialog=none command=queued")
    );
    assert_eq!(
        stops[64],
        "requested source=esc dialog=none command=queue_full"
    );
}

/// AC 8: no bridge, end to end: queued, skipped, nothing changes.
#[test]
fn g20_stop_no_bridge() {
    let mut r = rig(vec![]);
    for _ in 0..5 {
        r.h.step();
    }
    let before = r.led_states();
    r.h.key_press(Key::Escape);
    r.h.step();
    r.pump_until("skipped", |l| {
        l.iter().any(|x| x.starts_with("stop skipped"))
    });
    assert_eq!(
        r.stops(),
        vec![
            "requested source=esc dialog=none command=queued",
            "skipped reason=no_port port=-"
        ]
    );
    r.h.step();
    assert_eq!(r.led_states(), before, "LEDs unchanged");
    assert!(!r.h.state().modals().is_open(), "no dialog");
}

/// AC 10 (second clause): with a dialog open over the main screen, Tab and
/// Shift+Tab stay inside it: focus never reaches STOP or a LED.
#[test]
fn g20_tab_stays_in_dialog() {
    let mut r = rig(vec![]);
    r.h.step();
    r.h.state_mut()
        .modals_mut()
        .request(gallery::readiness_dialog());
    for _ in 0..3 {
        r.h.step();
    }
    let outside = [
        led_id("pc_bridge"),
        led_id("bridge_controller"),
        led_id("switch"),
        bottom_bar::stop_id(),
    ];
    let mut inside = 0;
    for i in 0..12 {
        if i < 6 {
            r.h.key_press(Key::Tab);
        } else {
            r.h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
        }
        r.h.step();
        let f = r.h.ctx.memory(|m| m.focused());
        assert!(
            f.is_none_or(|f| !outside.contains(&f)),
            "Tab {i} left the dialog"
        );
        inside += usize::from(f.is_some());
        assert!(r.h.state().modals().is_open());
    }
    assert!(inside > 0, "Tab focused nothing in the dialog");
}

/// AC 10 (Story 4.7), updated by Story 4.9 AC 11: Tab runs LED, LED, LED,
/// then the gamepad panel, which keeps Tab and Shift+Tab (the focus lock);
/// Esc leaves it (and is STOP); Shift+Tab from nothing focused reaches
/// STOP.
#[test]
fn g20_tab_order() {
    let mut r = rig(vec![]);
    r.h.step();
    let want = [
        led_id("pc_bridge"),
        led_id("bridge_controller"),
        led_id("switch"),
        ocb_app::ui::gamepad::panel_id(),
        ocb_app::ui::gamepad::panel_id(),
        ocb_app::ui::gamepad::panel_id(),
    ];
    for (i, id) in want.iter().enumerate() {
        r.h.key_press(Key::Tab);
        r.h.step();
        assert_eq!(r.h.ctx.memory(|m| m.focused()), Some(*id), "Tab {}", i + 1);
    }
    r.h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    r.h.step();
    r.h.step();
    assert_eq!(
        r.h.ctx.memory(|m| m.focused()),
        Some(ocb_app::ui::gamepad::panel_id()),
        "Shift+Tab stays on the panel"
    );
    r.h.key_press(Key::Escape);
    r.h.step();
    assert_eq!(
        r.h.ctx.memory(|m| m.focused()),
        None,
        "Esc leaves the panel"
    );
    assert_eq!(
        r.stops()
            .iter()
            .filter(|l| l.starts_with("requested source=esc "))
            .count(),
        1,
        "Esc is STOP"
    );
    r.h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    r.h.step();
    assert_eq!(r.h.ctx.memory(|m| m.focused()), Some(bottom_bar::stop_id()));
    // 100 idle frames: no new STOP request (the worker's own line for the
    // Esc above may still arrive: only the app's requests are counted).
    let requested = |r: &Rig| {
        r.stops()
            .iter()
            .filter(|l| l.starts_with("requested "))
            .count()
    };
    let stops = requested(&r);
    for _ in 0..100 {
        r.h.step();
    }
    assert_eq!(requested(&r), stops);
}

/// AC 11, 13 (E3-A3): the H.34 steps replayed on a fake bridge and an
/// `ocb-sim` controller; prints the expected lines per step
/// (`cargo test -p ocb-app -- --nocapture h34_dry_run`).
#[test]
fn g20_h34_dry_run() {
    const UNPLUG: u64 = 6_000_000;
    const REPLUG: u64 = 8_000_000;
    let port = bridge().present(vec![(0, Some(UNPLUG)), (REPLUG, None)]);
    let mut r = rig(vec![port]);
    r.until_connected();
    let mut seen = r.all_now().len();
    let print = |r: &Rig, seen: &mut usize, step: &str| -> Vec<String> {
        let all = r.all_now();
        let new: Vec<String> = all[*seen..].to_vec();
        *seen = all.len();
        println!("H.34 {step}:");
        for l in &new {
            println!("  {l}");
        }
        new
    };
    println!("H.34 setup (start, three LEDs ok):");
    for l in r.all_now() {
        println!("  {l}");
    }
    // The runbook's rule: within a request, the app's `requested` and the
    // worker's `sent` may come in either order (two threads, one log);
    // `acked` comes after `sent` and `after` after `acked` (worker order).
    let four = |lines: &[String], source: &str| {
        let s: Vec<&String> = lines.iter().filter(|l| l.starts_with("stop ")).collect();
        assert_eq!(s.len(), 4, "{lines:?}");
        let at = |p: &str| {
            let v: Vec<usize> = (0..s.len()).filter(|&k| s[k].starts_with(p)).collect();
            assert_eq!(v.len(), 1, "one `{p}`: {s:?}");
            v[0]
        };
        let req = at("stop requested ");
        assert_eq!(
            *s[req],
            format!("stop requested source={source} dialog=none command=queued")
        );
        let sent = at("stop sent ");
        assert_eq!(*s[sent], format!("stop sent port={CU}"));
        let acked = at("stop acked ");
        let after = at("stop after ");
        assert!(req < acked && sent < acked && acked < after, "{s:?}");
        assert!(
            s[acked].starts_with(&format!("stop acked port={CU} after_us=")),
            "{s:?}"
        );
        let after_us: u64 = s[acked]
            .split(' ')
            .find_map(|w| w.strip_prefix("after_us="))
            .unwrap()
            .parse()
            .unwrap();
        assert!(after_us <= 100_000, "{after_us}");
        assert!(s[acked].ends_with(" by=command"), "{s:?}");
        assert!(
            s[after].starts_with(&format!("stop after port={CU} mode=idle abort_count=")),
            "{s:?}"
        );
        assert!(
            !lines.iter().any(|l| l.starts_with("led ")
                || l.starts_with("nak ")
                || l.starts_with("reboot ")),
            "{lines:?}"
        );
    };
    let done =
        |l: &[String], n: usize| l.iter().filter(|x| x.starts_with("stop after ")).count() >= n;
    // Step 1: Esc.
    let base = count(&r.all_now(), "stop after ");
    r.h.key_press(Key::Escape);
    r.h.step();
    r.pump_until("step 1", |l| done(l, base + 1));
    let l = print(&r, &mut seen, "step 1 (Esc)");
    four(&l, "esc");
    // Step 2: click.
    r.click_stop();
    r.pump_until("step 2", |l| done(l, base + 2));
    let l = print(&r, &mut seen, "step 2 (click STOP)");
    four(&l, "button");
    // Step 3: Shift+Tab (STOP; Story 4.9: Tab × 4 is the gamepad panel
    // now), Space (nothing), Enter.
    r.focus_stop();
    r.h.key_press(Key::Space);
    r.h.step();
    let l = print(&r, &mut seen, "step 3a (Shift+Tab: STOP; Space)");
    assert_eq!(count(&l, "stop "), 0, "{l:?}");
    r.h.key_press(Key::Enter);
    r.h.step();
    r.pump_until("step 3", |l| done(l, base + 3));
    let l = print(&r, &mut seen, "step 3b (Enter)");
    four(&l, "button");
    // Step 3c: Enter held on the focused STOP (one fresh press, repeats).
    for _ in 0..20 {
        r.h.input_mut()
            .events
            .push(key_event(Key::Enter, true, Modifiers::NONE));
        r.h.step();
    }
    release(&mut r.h, &[Key::Enter]);
    r.h.step();
    r.pump_until("step 3c", |l| done(l, base + 4));
    let l = print(&r, &mut seen, "step 3c (Enter held)");
    four(&l, "button");
    // Step 4: Esc ×5 quickly (the worker clock does not move between them).
    for _ in 0..5 {
        r.h.key_press(Key::Escape);
        r.h.step();
    }
    r.pump_until("step 4 commands", |l| count_worker(&l[seen..]) == 5);
    r.pump_until("step 4", |l| done(l, base + 5));
    let l = print(&r, &mut seen, "step 4 (Esc x5 quickly)");
    assert_eq!(
        count(&l, "stop requested source=esc dialog=none command=queued"),
        5,
        "{l:?}"
    );
    assert_eq!(count_worker(&l), 5, "{l:?}");
    // Counts, not order; then an ACK by command and an idle `after` after
    // the last `sent`/`pending` (worker order).
    let last_cmd = (0..l.len())
        .rev()
        .find(|&k| l[k].starts_with("stop sent ") || l[k].starts_with("stop pending "))
        .unwrap();
    let acked = (last_cmd..l.len())
        .find(|&k| l[k].starts_with("stop acked ") && l[k].ends_with(" by=command"))
        .unwrap_or_else(|| panic!("no acked after the last command: {l:?}"));
    assert!(
        (acked..l.len()).any(|k| l[k].starts_with("stop after ") && l[k].contains(" mode=idle ")),
        "{l:?}"
    );
    assert_eq!(count(&l, "nak "), 0);
    assert_eq!(count(&l, "led "), 0);
    // Step 5: Esc held (one press, then repeats over frames).
    r.h.input_mut().events.push(esc(false));
    r.h.step();
    for _ in 0..20 {
        r.h.input_mut().events.push(esc(true));
        r.h.step();
    }
    r.h.input_mut()
        .events
        .push(key_event(Key::Escape, false, Modifiers::NONE));
    r.h.step();
    r.pump_until("step 5", |l| done(l, base + 6));
    let l = print(&r, &mut seen, "step 5 (Esc held)");
    assert_eq!(count(&l, "stop requested "), 1, "{l:?}");
    // Step 6: bridge out, Esc, bridge back, Esc.
    while r.clock.now_us() < UNPLUG {
        r.clock.advance(10_000);
        std::thread::sleep(Duration::from_millis(1));
        r.h.step();
    }
    r.pump_until("vanished", |l| {
        l.iter().any(|x| x.starts_with("port_vanished "))
    });
    r.pump_until("led error", |l| {
        l.iter().any(|x| x.starts_with("led pc_bridge=error"))
    });
    r.h.key_press(Key::Escape);
    r.h.step();
    r.pump_until("step 6a", |l| {
        l.iter().any(|x| x.starts_with("stop skipped"))
    });
    let l = print(&r, &mut seen, "step 6a (bridge out, Esc)");
    let mut s: Vec<&String> = l.iter().filter(|x| x.starts_with("stop ")).collect();
    s.sort();
    assert_eq!(
        s,
        vec![
            "stop requested source=esc dialog=none command=queued",
            "stop skipped reason=no_port port=-"
        ],
        "either order"
    );
    while r.clock.now_us() < REPLUG {
        r.clock.advance(10_000);
        std::thread::sleep(Duration::from_millis(1));
        r.h.step();
    }
    r.pump_until("back", |l| leds_ok(l) >= 2);
    let l = print(&r, &mut seen, "step 6b (bridge back)");
    assert!(
        l.iter()
            .any(|x| x.starts_with(&format!("port_opened port={CU}")))
    );
    assert!(l.iter().any(|x| x.starts_with("version connected")));
    let base6 = count(&r.all_now(), "stop after ");
    r.h.key_press(Key::Escape);
    r.h.step();
    r.pump_until("step 6c", |l| done(l, base6 + 1));
    let l = print(&r, &mut seen, "step 6c (Esc)");
    four(&l, "esc");
    // Step 7: close the window (what `on_exit` does).
    let t = Instant::now();
    assert!(r.h.state_mut().shut_down_link());
    assert!(t.elapsed() < Duration::from_secs(1));
    let l = print(&r, &mut seen, "step 7 (close the window)");
    assert!(
        l.iter().any(|x| *x == format!("port_closed port={CU}")),
        "{l:?}"
    );
    // Every stop line of the run matches its pattern.
    if let Some(lg) = r.logger.take() {
        assert!(lg.stop());
    }
    for l in common::kind_texts(&r.sink.lines(), "stop") {
        if l.starts_with("requested") {
            assert!(is_requested(&l), "{l}");
        }
    }
}
