//! G.18 (Story 4.6 AC 2–6, 8, 9): the firmware gate in the real app on a
//! fake worker thread whose fake controller reports another `proto_ver`:
//! the "Firmware update needed" dialog (OK only, 440 pt) drawn in the frame
//! the snapshot is drained, OK and Esc close it without lifting the gate,
//! once per connection (no nagging), again on reconnect and relaunch, gone
//! with a matching controller, the gamepad hint, the LEDs behind the dialog,
//! a recommended update blocking nothing, the `gate` and `dialog` lines.
//! Fake clocks for every number; waits on the worker thread are bounded
//! polls (≤ 2 s). Never the real backend or the real log (G.13).

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

use common::{TestClock, Wake};
use egui::accesskit::Role;
use egui::epaint::Shape;
use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ocb_app::app::{App, Mode};
use ocb_app::gating::{Block, Gate, HINT_FIRMWARE, MODAL_BODY, MODAL_TITLE};
use ocb_app::ui::firmware;
use ocb_app::ui::layout::regions;
use ocb_app::ui::theme::{self, components::modal_dialog as md};
use ocb_core::link::Phase;
use ocb_core::protocol::consts::PROTO_VERSION;
use ocb_core::version::APP_VERSION_TRIPLE;
use ocb_services::log::{LogHandle, Logger, MemorySink};
use ocb_services::sim::{FakeBackend, FakePort};
use ocb_services::worker::FakeClock;
use ocb_sim::ControllerConfig;

const CU: &str = "/dev/cu.usbmodem2101";
const CU2: &str = "/dev/cu.usbmodem3101";
const SERIAL: &str = "53032847189C2E9C";
const SERIAL2: &str = "E66138935F4A2B21";
const LIMIT: Duration = Duration::from_secs(2);

fn mismatched() -> ControllerConfig {
    ControllerConfig {
        proto_ver: PROTO_VERSION.wrapping_add(1),
        fw_ver: [0, 2, 0],
        bridge_ver: [0, 2, 1],
        ..ControllerConfig::default()
    }
}

fn bridge(cfg: ControllerConfig) -> FakePort {
    FakePort::bridge(CU, SERIAL, Some("Bonooru Bridge")).controller(cfg)
}

struct Rig {
    h: Harness<'static, App>,
    clock: FakeClock,
    published: Arc<AtomicU64>,
    logger: Option<Logger>,
    sink: MemorySink,
    /// The app's log handle (a test writes markers through it: the log is
    /// FIFO, so a marker seen in the sink means every earlier line is too).
    log: LogHandle,
}

fn rig_sized(ports: Vec<FakePort>, wake: Wake, size: (f32, f32)) -> Rig {
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
        .with_size(vec2(size.0, size.1))
        .build_eframe(move |cc| {
            let handle = common::fake_worker(
                &cc.egui_ctx,
                slot.take().unwrap(),
                c.clone(),
                Arc::clone(&p),
                wake,
            );
            App::with_link(cc, Mode::Main, handle)
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
    rig_sized(ports, Wake::App, (1280.0, 800.0))
}

impl Rig {
    /// Moves the worker's fake clock `step_us` per frame until `f` holds
    /// after a frame (bounded, 2 s of real time).
    fn advance_until(&mut self, step_us: u64, f: impl Fn(&Harness<'static, App>) -> bool) {
        let start = Instant::now();
        loop {
            self.clock.advance(step_us);
            std::thread::sleep(Duration::from_millis(1));
            self.h.step();
            if f(&self.h) {
                return;
            }
            assert!(
                start.elapsed() < LIMIT,
                "not reached; gate {:?}, open {}",
                self.h.state().gate(),
                self.h.state().modals().is_open()
            );
        }
    }

    /// Runs until the firmware dialog is open.
    fn until_dialog(&mut self) {
        self.advance_until(1_000, |h| h.state().modals().is_open());
    }

    fn dialog_drawn(&self) -> bool {
        self.h
            .query_by_role_and_label(Role::Dialog, MODAL_TITLE)
            .is_some()
    }

    fn hint_drawn(&self) -> bool {
        self.h
            .query_by_role_and_label(Role::Label, HINT_FIRMWARE)
            .is_some()
    }

    fn click_ok(&mut self) {
        self.h.get_by_role_and_label(Role::Button, "OK").click();
        for _ in 0..3 {
            self.h.step();
        }
    }

    /// The AccessKit names of the three LEDs drawn in the last frame.
    fn leds_drawn(&self) -> Vec<String> {
        self.h
            .query_all_by_role(Role::Status)
            .map(|n| n.accesskit_node().label().unwrap_or_default())
            .collect()
    }

    /// Stops the log (flushing it) and returns the `kind` lines.
    fn lines(&mut self, kind: &str) -> Vec<String> {
        if let Some(l) = self.logger.take() {
            assert!(l.stop());
        }
        common::kind_texts(&self.sink.lines(), kind)
    }

    /// The `kind` lines written so far, without stopping the log: writes a
    /// marker and waits (bounded) until the log thread has written it.
    fn lines_now(&self, kind: &str) -> Vec<String> {
        let marker = format!("marker-{}", Instant::now().elapsed().as_nanos());
        self.log.log("test", marker.clone());
        let start = Instant::now();
        loop {
            let lines = self.sink.lines();
            if lines.iter().any(|l| l.ends_with(&marker)) {
                return common::kind_texts(&lines, kind);
            }
            assert!(start.elapsed() < LIMIT, "log never flushed");
            std::thread::sleep(Duration::from_millis(1));
        }
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
}

fn firmware_block() -> Option<Block> {
    Some(Block::FirmwareUpdate {
        controller_proto: PROTO_VERSION.wrapping_add(1),
        app_proto: PROTO_VERSION,
    })
}

fn blocked() -> Gate {
    Gate {
        gamepad: firmware_block(),
        play: firmware_block(),
    }
}

fn shown_at(line: &str) -> u64 {
    line.split(' ')
        .find_map(|w| w.strip_prefix("connected_us="))
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn g18_modal_on_mismatch() {
    let mut r = rig(vec![bridge(mismatched())]);
    r.until_dialog();
    assert!(r.dialog_drawn());
    assert_eq!(r.h.state().modals().open_id(), Some(firmware::dialog_id()));
    let _ = r.h.get_by_role_and_label(Role::Label, MODAL_BODY);
    // Exactly one dialog button, "OK" (the LEDs are Status nodes); the
    // bottom bar's STOP (Story 4.7) is the only other button, under the
    // scrim, and comes first in the tree.
    let buttons: Vec<String> =
        r.h.query_all_by_role(Role::Button)
            .map(|n| n.accesskit_node().label().unwrap_or_default())
            .collect();
    assert_eq!(buttons, vec!["Stop".to_owned(), "OK".to_owned()]);
    let spec = firmware::required_update_dialog();
    assert_eq!(spec.secondary, None);
    assert!((spec.outer_width() - 440.0).abs() < f32::EPSILON);
    // G.12's frame geometry at 1280×800: 440 ± 0.5 wide, centred, the title
    // inside the padding, OK inside the frame's padding.
    let flat = common::shapes_flat(&r.h.output().shapes);
    let frame = common::dialog_frame_rect(&flat);
    assert!((frame.width() - 440.0).abs() <= 0.5, "{}", frame.width());
    assert!((frame.center() - r.h.ctx.content_rect().center()).length() <= 1.0);
    let title = flat
        .iter()
        .find_map(|f| match &f.shape {
            Shape::Text(t) if t.galley.text() == MODAL_TITLE => {
                Some(t.galley.rect.translate(t.pos.to_vec2()))
            }
            _ => None,
        })
        .unwrap();
    assert!(title.top() >= frame.top() + md::PADDING_Y - 0.01);
    let ok = r.h.get_by_role_and_label(Role::Button, "OK").rect();
    assert!(ok.right() <= frame.right() - md::PADDING_X + 0.5);
    assert!(ok.bottom() <= frame.bottom() - md::PADDING_Y + 0.5);
    // Logged once, in the exact format.
    let shown: Vec<String> = r
        .lines("dialog")
        .into_iter()
        .filter(|l| l.starts_with("shown"))
        .collect();
    assert_eq!(shown.len(), 1);
    assert!(common::is_dialog_shown_line(&shown[0]), "{}", shown[0]);
    let want = format!(
        "controller_proto={} app_proto={PROTO_VERSION} fw=0.2.0 bridge=0.2.1",
        PROTO_VERSION.wrapping_add(1)
    );
    assert!(shown[0].ends_with(&want), "{}", shown[0]);
}

#[test]
fn g18_modal_same_frame() {
    // The worker's waker only counts: no frame runs unless the test steps.
    let mut r = rig_sized(vec![bridge(mismatched())], Wake::CountOnly, (1280.0, 800.0));
    r.h.step();
    assert!(!r.dialog_drawn());
    // Let the worker open the port and get the mismatched INFO with no
    // frame in between, then freeze its clock.
    for _ in 0..200 {
        r.clock.advance(1_000);
        std::thread::sleep(Duration::from_millis(1));
    }
    r.quiet();
    // Exactly one frame drains the snapshot and draws the dialog.
    r.h.step();
    // Painted (scrim and title) and in the AccessKit tree in that frame:
    // egui's sizing pass of the new area is discarded within the frame.
    let flat = common::shapes_flat(&r.h.output().shapes);
    assert!(
        flat.iter()
            .any(|f| matches!(&f.shape, Shape::Rect(x) if x.fill == theme::SCRIM)),
        "scrim drawn in the draining frame"
    );
    assert!(
        flat.iter()
            .any(|f| matches!(&f.shape, Shape::Text(t) if t.galley.text() == MODAL_TITLE)),
        "title drawn in the draining frame"
    );
    assert_eq!(r.h.state().gate(), blocked());
    assert!(
        r.dialog_drawn(),
        "the dialog is drawn in the draining frame"
    );
}

#[test]
fn g18_ok_closes_gate_stays() {
    let mut r = rig(vec![bridge(mismatched())]);
    r.until_dialog();
    r.click_ok();
    assert!(!r.dialog_drawn());
    assert!(!r.h.state().modals().is_open());
    assert_eq!(r.h.state().gate(), blocked());
    assert!(r.hint_drawn());
    let closed: Vec<String> = r
        .lines("dialog")
        .into_iter()
        .filter(|l| l.starts_with("closed"))
        .collect();
    assert_eq!(
        closed,
        vec!["closed id=firmware_update_needed outcome=primary".to_owned()]
    );
}

#[test]
fn g18_esc_closes_gate_stays() {
    let mut r = rig(vec![bridge(mismatched())]);
    r.until_dialog();
    r.h.key_press(Key::Escape);
    for _ in 0..3 {
        r.h.step();
    }
    assert!(!r.dialog_drawn());
    assert!(!r.h.state().modals().is_open());
    assert_eq!(r.h.state().gate(), blocked());
    assert!(r.hint_drawn());
    let closed: Vec<String> = r
        .lines("dialog")
        .into_iter()
        .filter(|l| l.starts_with("closed"))
        .collect();
    assert_eq!(
        closed,
        vec!["closed id=firmware_update_needed outcome=dismissed".to_owned()]
    );
}

#[test]
fn g18_no_nagging() {
    let mut r = rig(vec![bridge(mismatched())]);
    r.until_dialog();
    r.click_ok();
    // 10 s of fake time in 50 ms steps, a frame each.
    for _ in 0..200 {
        r.clock.advance(50_000);
        std::thread::sleep(Duration::from_millis(1));
        r.h.step();
        assert!(!r.h.state().modals().is_open(), "the dialog came back");
        assert!(!r.dialog_drawn());
    }
    // The worker thread follows the fake clock at its own pace: on a slow
    // CI runner its last snapshot can lag the 200 frames. Let it catch up
    // (real-time bound), still checking that the dialog never comes back.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while r
        .h
        .state()
        .link_snapshot()
        .is_none_or(|s| s.now_us < 10_000_000)
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(5));
        r.h.step();
        assert!(!r.h.state().modals().is_open(), "the dialog came back");
    }
    let snap = r.h.state().link_snapshot().cloned().unwrap();
    assert!(snap.now_us >= 10_000_000, "the worker kept running");
    assert_eq!(snap.phase, Phase::Incompatible, "the same connection");
    assert_eq!(r.h.state().gate(), blocked());
    assert!(r.hint_drawn());
    let dialog = r.lines("dialog");
    assert_eq!(
        dialog.iter().filter(|l| l.starts_with("shown")).count(),
        1,
        "{dialog:?}"
    );
}

#[test]
fn g18_reshown_on_reconnect() {
    let port = bridge(mismatched()).present(vec![(0, Some(1_000_000)), (2_000_000, None)]);
    let mut r = rig(vec![port]);
    r.until_dialog();
    r.click_ok();
    // Unplugged at 1 s: the gate lifts with the loss and the hint goes
    // (Open question 7).
    r.advance_until(50_000, |h| {
        h.state()
            .link_snapshot()
            .is_some_and(|s| s.required_update.is_none() && s.now_us >= 1_000_000)
    });
    assert_eq!(r.h.state().gate(), Gate::default(), "unplugged");
    assert!(!r.hint_drawn(), "no hint while unplugged");
    assert!(!r.dialog_drawn());
    // Back at 2 s, still mismatching: the dialog again.
    r.advance_until(50_000, |h| h.state().modals().is_open());
    assert!(r.dialog_drawn(), "the dialog after the replug");
    assert!(r.h.state().link_snapshot().unwrap().now_us >= 2_000_000);
    let shown: Vec<u64> = r
        .lines("dialog")
        .iter()
        .filter(|l| l.starts_with("shown"))
        .map(|l| shown_at(l))
        .collect();
    assert_eq!(shown.len(), 2, "{shown:?}");
    assert!(shown[1] > shown[0] && shown[1] >= 2_000_000, "{shown:?}");
}

#[test]
fn g18_reshown_on_relaunch() {
    for run in 0..2 {
        let mut r = rig(vec![bridge(mismatched())]);
        r.until_dialog();
        assert!(r.dialog_drawn(), "run {run}");
        r.click_ok();
        let shown = r.lines("dialog");
        assert_eq!(
            shown.iter().filter(|l| l.starts_with("shown")).count(),
            1,
            "run {run}"
        );
    }
}

#[test]
fn g18_resolved_by_reconnect() {
    let old = bridge(mismatched()).present(vec![(0, Some(1_000_000))]);
    let new =
        FakePort::bridge(CU2, SERIAL2, Some("Bonooru Bridge")).present(vec![(2_000_000, None)]);
    let mut r = rig(vec![old, new]);
    r.until_dialog();
    r.click_ok();
    r.advance_until(50_000, |h| {
        h.state()
            .link_snapshot()
            .is_some_and(|s| s.phase == Phase::Connected && s.info.is_some())
    });
    assert_eq!(r.h.state().gate(), Gate::default());
    assert!(!r.hint_drawn());
    assert!(!r.dialog_drawn());
    // Exactly off (mismatch), on (unplug at 1 s): the matching INFO at 2 s
    // brings no new block, and the matching snapshot stays unblocked for a
    // while (a block from the matching INFO would add a third line).
    for _ in 0..20 {
        r.clock.advance(50_000);
        std::thread::sleep(Duration::from_millis(1));
        r.h.step();
        assert_eq!(r.h.state().gate(), Gate::default());
    }
    assert_eq!(r.h.state().link_snapshot().unwrap().phase, Phase::Connected);
    let gate = r.lines("gate");
    let blocked_line = format!(
        "gamepad=off play=off reason=firmware_update controller_proto={} app_proto={PROTO_VERSION}",
        PROTO_VERSION.wrapping_add(1)
    );
    assert_eq!(
        gate,
        vec![
            blocked_line,
            "gamepad=on play=on reason=none controller_proto=- app_proto=-".to_owned()
        ],
        "{gate:?}"
    );
    let dialog = common::kind_texts(&r.sink.lines(), "dialog");
    assert_eq!(
        dialog.iter().filter(|l| l.starts_with("shown")).count(),
        1,
        "{dialog:?}"
    );
}

/// The hint's text shape in the last frame: (ink rect, size, colour).
fn hint_shape(h: &Harness<'_, App>) -> Option<(egui::Rect, f32, egui::Color32)> {
    common::shapes_flat(&h.output().shapes)
        .into_iter()
        .find_map(|f| match &f.shape {
            Shape::Text(t) if t.galley.text() == HINT_FIRMWARE => Some((
                t.galley.rect.translate(t.pos.to_vec2()),
                t.galley.job.sections[0].format.font_id.size,
                t.galley.job.sections[0].format.color,
            )),
            _ => None,
        })
}

#[test]
fn g18_hint() {
    let mut r = rig(vec![bridge(mismatched())]);
    r.until_dialog();
    // Visible behind the dialog: painted before the scrim.
    assert!(r.hint_drawn());
    let flat = common::shapes_flat(&r.h.output().shapes);
    let index_of =
        |f: &dyn Fn(&Shape) -> bool| flat.iter().find(|x| f(&x.shape)).map(|x| x.index).unwrap();
    let hint_at = index_of(&|s| matches!(s, Shape::Text(t) if t.galley.text() == HINT_FIRMWARE));
    let scrim_at = index_of(&|s| matches!(s, Shape::Rect(x) if x.fill == theme::SCRIM));
    assert!(hint_at < scrim_at, "hint {hint_at} after scrim {scrim_at}");
    let (ink, size, color) = hint_shape(&r.h).expect("hint behind the dialog");
    assert_eq!(size, 12.5);
    assert_eq!(color, theme::TEXT_MUTED);
    let pad = regions(r.h.ctx.content_rect()).gamepad;
    assert!(
        common::inside(ink, pad.shrink(10.0), 0.5),
        "{ink:?} in {pad:?}"
    );
    assert!((ink.top() - (pad.top() + 10.0)).abs() <= 0.5);
    r.click_ok();
    assert!(hint_shape(&r.h).is_some(), "after OK");
    // Not focusable, and never in the Tab ring while Tab does move focus.
    let node = r.h.get_by_role_and_label(Role::Label, HINT_FIRMWARE);
    assert!(
        !node
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Focus),
        "the hint advertises focus"
    );
    let mut focused = std::collections::BTreeSet::new();
    for _ in 0..12 {
        r.h.key_press(Key::Tab);
        r.h.step();
        assert!(
            !r.h.get_by_role_and_label(Role::Label, HINT_FIRMWARE)
                .is_focused(),
            "Tab focused the hint"
        );
        if let Some(id) = r.h.ctx.memory(|m| m.focused()) {
            focused.insert(format!("{id:?}"));
        }
    }
    assert!(focused.len() >= 3, "Tab moved focus: {focused:?}");
    // Without the block: no hint text, no hint node.
    let mut ok = rig(vec![bridge(ControllerConfig::default())]);
    ok.advance_until(1_000, |h| {
        h.state()
            .link_snapshot()
            .is_some_and(|s| s.phase == Phase::Connected)
    });
    ok.h.step();
    assert!(!ok.hint_drawn());
    assert!(hint_shape(&ok.h).is_none());
}

#[test]
fn g18_leds_keep_versions() {
    let mut r = rig(vec![bridge(mismatched())]);
    r.until_dialog();
    let want = [
        // Story 4.5's accessible names (review decision 2026-11-27).
        "Bridge v0.2.1 – update required",
        "Controller v0.2.0 – update required",
        "Switch – unknown",
    ];
    use ocb_app::leds::LedState;
    let states = |r: &Rig| {
        let l = r.h.state().leds();
        l.all().map(|(_, led)| (led.state, led.name()))
    };
    let want_states = [
        (LedState::Ok, "Bridge v0.2.1".to_owned()),
        (LedState::Warning, "Controller v0.2.0".to_owned()),
        (LedState::Unknown, "Switch".to_owned()),
    ];
    assert_eq!(r.leds_drawn(), want, "with the dialog open");
    assert_eq!(states(&r), want_states, "with the dialog open");
    r.click_ok();
    assert_eq!(r.leds_drawn(), want, "after OK");
    assert_eq!(states(&r), want_states, "after OK");
    let leds = r.h.state().leds();
    assert_eq!(
        leds.bridge_controller.tooltip.first().map(String::as_str),
        Some("This firmware doesn't match this app.")
    );
}

#[test]
fn g18_recommended_blocks_nothing() {
    let [a, b, c] = APP_VERSION_TRIPLE;
    let cfg = ControllerConfig {
        fw_ver: [a, b, c.wrapping_add(1)],
        ..ControllerConfig::default()
    };
    let mut r = rig(vec![bridge(cfg)]);
    r.advance_until(1_000, |h| {
        h.state()
            .link_snapshot()
            .is_some_and(|s| s.phase == Phase::Connected && s.status.is_some())
    });
    for _ in 0..20 {
        r.clock.advance(1_000);
        std::thread::sleep(Duration::from_millis(1));
        r.h.step();
        assert!(!r.h.state().modals().is_open());
    }
    assert!(!r.dialog_drawn());
    assert!(!r.hint_drawn());
    assert_eq!(r.h.state().gate(), Gate::default());
    let leds = r.h.state().leds();
    assert_eq!(
        leds.pc_bridge.state,
        ocb_app::leds::LedState::Ok,
        "{:?}",
        leds.pc_bridge
    );
    assert_eq!(leds.bridge_controller.state, ocb_app::leds::LedState::Ok);
    assert!(leds.bridge_controller.name().starts_with("Controller v"));
    assert!(r.lines("dialog").is_empty());
    assert!(common::kind_texts(&r.sink.lines(), "gate").is_empty());
}

/// Review finding 1: an Esc pressed in the frame the mismatch arrives (the
/// dialog's first pass is discarded, so the user never saw it) does not
/// dismiss the dialog; it stays open, with no `closed` line.
#[test]
fn g18_esc_in_the_arrival_frame() {
    let mut r = rig_sized(vec![bridge(mismatched())], Wake::CountOnly, (1280.0, 800.0));
    r.h.step();
    for _ in 0..200 {
        r.clock.advance(1_000);
        std::thread::sleep(Duration::from_millis(1));
    }
    r.quiet();
    assert!(!r.h.state().modals().is_open());
    // Esc down in the draining frame, up in the next (one frame per event).
    r.h.key_press(Key::Escape);
    r.h.step();
    r.h.step();
    assert!(r.h.state().modals().is_open(), "dismissed by an unseen Esc");
    assert!(r.dialog_drawn());
    let dialog = r.lines("dialog");
    assert_eq!(dialog.len(), 1, "{dialog:?}");
    assert!(dialog[0].starts_with("shown"), "{dialog:?}");
}

/// Review finding 2: a replug while the dialog is still open (owner
/// decision 4) logs no second `shown`; the dialog stays.
#[test]
fn g18_replug_while_open() {
    let port = bridge(mismatched()).present(vec![(0, Some(1_000_000)), (2_000_000, None)]);
    let mut r = rig(vec![port]);
    r.until_dialog();
    let first = r.h.state().link_snapshot().unwrap().connected_us;
    r.advance_until(50_000, |h| {
        h.state()
            .link_snapshot()
            .is_some_and(|s| s.required_update.is_some() && s.connected_us != first)
    });
    r.h.step();
    assert!(r.dialog_drawn(), "still open");
    let dialog = r.lines("dialog");
    assert_eq!(dialog.len(), 1, "one shown, no close: {dialog:?}");
    assert_eq!(shown_at(&dialog[0]), first.unwrap());
}

/// Review finding 2: requested while another dialog is open, the firmware
/// dialog waits; `dialog shown` is written when it opens (not before), and
/// it is drawn in that frame.
#[test]
fn g18_queued_behind_another_dialog() {
    let mut r = rig(vec![bridge(mismatched())]);
    r.h.step();
    r.h.state_mut()
        .modals_mut()
        .request(ocb_app::ui::gallery::readiness_dialog());
    r.advance_until(1_000, |h| h.state().gate() == blocked());
    for _ in 0..3 {
        r.h.step();
    }
    assert_eq!(r.h.state().modals().queued(), 1, "firmware dialog queued");
    assert!(!r.dialog_drawn());
    assert!(
        r.lines_now("dialog").is_empty(),
        "logged while not on screen"
    );
    // Close the other dialog: the firmware dialog opens and is drawn in
    // the next frame, its `shown` line written then.
    r.h.get_by_role_and_label(Role::Button, "Cancel").click();
    r.h.step();
    r.h.step();
    assert!(r.dialog_drawn(), "drawn in the frame it opens");
    let dialog = r.lines("dialog");
    assert_eq!(dialog.len(), 1, "{dialog:?}");
    assert!(common::is_dialog_shown_line(&dialog[0]), "{dialog:?}");
}

/// Review finding 4: in the gallery the update sample is the real firmware
/// dialog, but its outcome is never read there: OK on it writes no `dialog`
/// line.
#[test]
fn g18_gallery_logs_nothing() {
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let log = logger.handle();
    let mut h = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .build_eframe(move |cc| {
            App::with_link(cc, Mode::Gallery, common::fake_link(&cc.egui_ctx)).led_log(log.clone())
        });
    h.step();
    // What the gallery's "Open update dialog" button does.
    h.state_mut()
        .modals_mut()
        .request(ocb_app::ui::gallery::update_dialog());
    for _ in 0..3 {
        h.step();
    }
    assert!(
        h.query_by_role_and_label(Role::Dialog, MODAL_TITLE)
            .is_some()
    );
    h.get_by_role_and_label(Role::Button, "OK").click();
    for _ in 0..3 {
        h.step();
    }
    assert!(!h.state().modals().is_open());
    drop(h);
    assert!(logger.stop());
    assert_eq!(
        common::kind_texts(&sink.lines(), "dialog"),
        Vec::<String>::new()
    );
}
