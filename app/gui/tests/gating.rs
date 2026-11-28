//! G.17 (Story 4.6 AC 1, 3, 9, 10; Story 4.8 AC 9, 10, 12): the gate model
//! `ocb_app::gating` on hand-built snapshots and LEDs: the precedence of the
//! six reasons (the firmware block from `required_update` only), the
//! firmware dialog trigger fires once per connection (`connected_us`), one
//! `gate` line per change, no online wording, no egui in the model.
//! Deterministic, no threads but the log's.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use ocb_app::app::WorkerHealth;
use ocb_app::gating::{
    self, Block, FirmwareWatch, Gate, GateTracker, HINT_BRIDGE_NOT_READY, HINT_CONTROLLER_LINK,
    HINT_FIRMWARE, HINT_NO_BRIDGE, HINT_SWITCH_NOT_READY, HINT_WORKER, STRINGS, dialog_closed_line,
    dialog_shown_line, gate_line,
};
use ocb_app::leds::{self, LedState, Leds, Link3, Reason, banned_word};
use ocb_core::device::Class;
use ocb_core::link::{Phase, StatusView};
use ocb_core::protocol::consts::PROTO_VERSION;
use ocb_core::protocol::{EventCode, HidProfile, Info, Mode};
use ocb_services::log::{Logger, MemorySink};
use ocb_services::worker::{Chosen, Counters, LinkSnapshot};

const NOW: u64 = 10_000_000;

fn info(fw: [u8; 3], bridge: [u8; 3], proto: u8) -> Info {
    Info {
        proto_ver: proto,
        fw_ver: fw,
        bridge_ver: bridge,
        hid_profile: HidProfile::P2,
        queue_cap: 512,
        app_timeout_ms: 1000,
        report_period_us: 8000,
    }
}

fn status(console_ready: bool) -> StatusView {
    StatusView {
        free: 512,
        now_us: 0,
        last_seq: 0,
        mode: Mode::Idle,
        abort_count: 0,
        last_event: EventCode::None,
        link_ok: true,
        console_ready,
    }
}

fn empty() -> LinkSnapshot {
    LinkSnapshot {
        now_us: NOW,
        scans: 10,
        recent_scans: Vec::new(),
        ports: Vec::new(),
        chosen: None,
        phase: Phase::Closed,
        info: None,
        status: None,
        counters: Counters::default(),
        port_writes: Vec::new(),
        last_error: None,
        session_since_us: None,
        connected_us: None,
        required_update: None,
        last_nak: None,
    }
}

/// A mismatching controller (`proto_ver` = the app's + 1) answered at
/// `connected_us`.
fn mismatch(connected_us: u64) -> LinkSnapshot {
    let mut s = empty();
    s.chosen = Some(Chosen {
        name: "/dev/cu.usbmodem2101".to_owned(),
        serial: Some("53032847189C2E9C".to_owned()),
        class: Class::Ours,
        open: true,
    });
    s.phase = Phase::Incompatible;
    s.session_since_us = Some(connected_us - 5_000);
    s.connected_us = Some(connected_us);
    s.required_update = Some(info([0, 2, 0], [0, 2, 1], PROTO_VERSION.wrapping_add(1)));
    s
}

/// A matching controller answered at `connected_us`.
fn matching(connected_us: u64) -> LinkSnapshot {
    let mut s = empty();
    s.chosen = Some(Chosen {
        name: "/dev/cu.usbmodem2101".to_owned(),
        serial: Some("53032847189C2E9C".to_owned()),
        class: Class::Ours,
        open: true,
    });
    s.phase = Phase::Connected;
    s.connected_us = Some(connected_us);
    s.info = Some(info([0, 0, 0], [0, 0, 0], PROTO_VERSION));
    s.status = Some((status(true), NOW - 10_000));
    s
}

fn firmware_block() -> Block {
    Block::FirmwareUpdate {
        controller_proto: PROTO_VERSION.wrapping_add(1),
        app_proto: PROTO_VERSION,
    }
}

/// The LEDs of `snap` with a running worker.
fn leds_of(snap: Option<&LinkSnapshot>) -> Leds {
    leds::evaluate(snap, 0, WorkerHealth::Running)
}

/// The gate of `snap` and its LEDs.
fn gate_of(snap: Option<&LinkSnapshot>) -> Gate {
    gating::evaluate(snap, &leds_of(snap))
}

fn both(b: Block) -> Gate {
    Gate {
        gamepad: Some(b),
        play: Some(b),
    }
}

/// LEDs all `ok` (a matching, ready controller), then each override.
fn all_ok() -> Leds {
    let l = leds_of(Some(&matching(NOW - 1_000_000)));
    for (_, led) in l.all() {
        assert_eq!(led.state, LedState::Ok, "{led:?}");
    }
    l
}

fn set(l: &mut Leds, link: Link3, state: LedState, reason: Reason) {
    let led = match link {
        Link3::PcBridge => &mut l.pc_bridge,
        Link3::BridgeController => &mut l.bridge_controller,
        Link3::ControllerSwitch => &mut l.switch,
    };
    led.state = state;
    led.reason = reason;
}

#[test]
fn g17_table() {
    assert_eq!(PROTO_VERSION, 1, "the table's numbers assume protocol 1");
    let open = matching(NOW - 1_000_000);
    let mism = mismatch(NOW - 1_000_000);
    // All ok + no required update → open.
    let g = gate_of(Some(&open));
    assert_eq!(g, Gate::default());
    assert!(g.gamepad_enabled() && g.play_enabled());
    // Each reason, end to end from a snapshot (or the worker's health).
    assert_eq!(
        gating::evaluate(
            Some(&open),
            &leds::evaluate(Some(&open), 0, WorkerHealth::Stalled)
        ),
        both(Block::Worker)
    );
    assert_eq!(
        gating::evaluate(
            Some(&open),
            &leds::evaluate(Some(&open), 0, WorkerHealth::Ended)
        ),
        both(Block::Worker)
    );
    assert_eq!(
        gating::evaluate(None, &leds::evaluate(None, 0, WorkerHealth::NotStarted)),
        both(Block::Worker)
    );
    assert_eq!(gate_of(Some(&empty())), both(Block::NoBridge));
    assert_eq!(
        gate_of(None),
        both(Block::BridgeNotReady),
        "no snapshot yet"
    );
    let g = gate_of(Some(&mism));
    assert_eq!(g, both(firmware_block()));
    assert_eq!(
        g.gamepad,
        Some(Block::FirmwareUpdate {
            controller_proto: 2,
            app_proto: 1
        })
    );
    // A mismatch with a (untrusted) STATUS saying the console is ready:
    // still the firmware block.
    let mut s = mism.clone();
    s.status = Some((status(true), NOW - 10_000));
    assert_eq!(gate_of(Some(&s)), both(firmware_block()));
    // Matching INFO, STATUS link_ok 0 → controller_link; console_ready 0 →
    // switch_not_ready.
    let mut s = open.clone();
    let mut v = status(true);
    v.link_ok = false;
    s.status = Some((v, NOW - 10_000));
    assert_eq!(gate_of(Some(&s)), both(Block::ControllerLink));
    let mut s = open.clone();
    s.status = Some((status(false), NOW - 10_000));
    assert_eq!(gate_of(Some(&s)), both(Block::SwitchNotReady));
    // The order, pairwise on the LEDs.
    let worker_over_bridge = {
        let mut l = all_ok();
        set(&mut l, Link3::PcBridge, LedState::Error, Reason::NotFound);
        set(
            &mut l,
            Link3::ControllerSwitch,
            LedState::Unknown,
            Reason::WorkerStalled,
        );
        gating::evaluate(Some(&open), &l)
    };
    assert_eq!(worker_over_bridge, both(Block::Worker));
    let no_bridge_over_firmware = {
        let mut l = all_ok();
        set(&mut l, Link3::PcBridge, LedState::Error, Reason::NotFound);
        gating::evaluate(Some(&mism), &l)
    };
    assert_eq!(no_bridge_over_firmware, both(Block::NoBridge));
    let bridge_over_firmware = {
        let mut l = all_ok();
        set(&mut l, Link3::PcBridge, LedState::Warning, Reason::Busy);
        gating::evaluate(Some(&mism), &l)
    };
    assert_eq!(bridge_over_firmware, both(Block::BridgeNotReady));
    let firmware_over_links = {
        let mut l = all_ok();
        set(
            &mut l,
            Link3::BridgeController,
            LedState::Error,
            Reason::LinkDown,
        );
        set(
            &mut l,
            Link3::ControllerSwitch,
            LedState::Warning,
            Reason::ConsoleNotReady,
        );
        gating::evaluate(Some(&mism), &l)
    };
    assert_eq!(firmware_over_links, both(firmware_block()));
    let controller_over_switch = {
        let mut l = all_ok();
        set(
            &mut l,
            Link3::BridgeController,
            LedState::Error,
            Reason::Stale,
        );
        set(
            &mut l,
            Link3::ControllerSwitch,
            LedState::Warning,
            Reason::ConsoleNotReady,
        );
        gating::evaluate(Some(&open), &l)
    };
    assert_eq!(controller_over_switch, both(Block::ControllerLink));
    let switch_alone = {
        let mut l = all_ok();
        set(
            &mut l,
            Link3::ControllerSwitch,
            LedState::Unknown,
            Reason::Waiting,
        );
        gating::evaluate(Some(&open), &l)
    };
    assert_eq!(switch_alone, both(Block::SwitchNotReady));
    // The hints and log reasons of every block.
    let want = [
        (Block::Worker, HINT_WORKER, "worker"),
        (Block::NoBridge, HINT_NO_BRIDGE, "no_bridge"),
        (
            Block::BridgeNotReady,
            HINT_BRIDGE_NOT_READY,
            "bridge_not_ready",
        ),
        (firmware_block(), HINT_FIRMWARE, "firmware_update"),
        (
            Block::ControllerLink,
            HINT_CONTROLLER_LINK,
            "controller_link",
        ),
        (
            Block::SwitchNotReady,
            HINT_SWITCH_NOT_READY,
            "switch_not_ready",
        ),
    ];
    assert_eq!(
        Block::samples().to_vec(),
        want.iter().map(|w| w.0).collect::<Vec<_>>()
    );
    // Each sample's rank (the exhaustive match) is its place: the precedence
    // `evaluate` applies, and the last rank closes the list.
    for (i, b) in Block::samples().iter().enumerate() {
        assert_eq!(b.rank(), i, "{b:?}");
    }
    assert_eq!(Block::SwitchNotReady.rank() + 1, Block::samples().len());
    for (b, hint, name) in want {
        assert_eq!(b.gamepad_hint(), hint);
        assert_eq!(b.as_str(), name);
    }
}

#[test]
fn g17_watch_once_per_connection() {
    let mut w = FirmwareWatch::default();
    let s = mismatch(2_000_000);
    let shown = (0..1_000)
        .filter_map(|_| w.take_new(Some(&s)))
        .collect::<Vec<_>>();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0], (s.required_update.unwrap(), 2_000_000));
    // Later snapshots of the same connection (time moves on) and missing
    // snapshots never bring it back.
    let mut later = mismatch(2_000_000);
    later.now_us = NOW + 10_000_000;
    assert_eq!(w.take_new(Some(&later)), None);
    assert_eq!(w.take_new(None), None);
    assert_eq!(w.take_new(Some(&later)), None);
    // A matching snapshot or one without connected_us: nothing.
    let mut w = FirmwareWatch::default();
    assert_eq!(w.take_new(Some(&matching(1_000))), None);
    let mut no_at = mismatch(2_000_000);
    no_at.connected_us = None;
    assert_eq!(w.take_new(Some(&no_at)), None);
}

#[test]
fn g17_watch_new_connection() {
    let mut w = FirmwareWatch::default();
    assert!(w.take_new(Some(&mismatch(1_000_000))).is_some());
    // Unplugged: snapshots without a mismatch.
    assert_eq!(w.take_new(Some(&empty())), None);
    // Replugged, still mismatching: a new connection.
    let again = w.take_new(Some(&mismatch(3_000_000)));
    assert_eq!(again.map(|(_, at)| at), Some(3_000_000));
    assert_eq!(w.take_new(Some(&mismatch(3_000_000))), None);
}

#[test]
fn g17_watch_coalesced_reconnect() {
    // The UI never saw the snapshot of the unplugged bridge (the channel
    // keeps only the newest): two mismatches with different connected_us
    // back to back are two connections. Same port name, same serial.
    let mut w = FirmwareWatch::default();
    let a = mismatch(1_000_000);
    let b = mismatch(3_000_000);
    assert!(
        a.chosen.is_some() && a.chosen == b.chosen,
        "same port in both"
    );
    assert!(w.take_new(Some(&a)).is_some());
    assert!(w.take_new(Some(&b)).is_some());
}

#[test]
fn g17_gate_log_lines() {
    // The matcher itself.
    assert!(common::is_gate_line(
        "gamepad=off gamepad_reason=firmware_update play=off play_reason=firmware_update controller_proto=2 app_proto=1"
    ));
    assert!(common::is_gate_line(
        "gamepad=on gamepad_reason=none play=on play_reason=none controller_proto=- app_proto=-"
    ));
    assert!(common::is_gate_line(
        "gamepad=off gamepad_reason=no_bridge play=off play_reason=no_bridge controller_proto=- app_proto=-"
    ));
    for bad in [
        "gamepad=maybe gamepad_reason=firmware_update play=off play_reason=firmware_update controller_proto=2 app_proto=1",
        "gamepad=off gamepad_reason=Firmware play=off play_reason=firmware_update controller_proto=2 app_proto=1",
        "gamepad=off gamepad_reason=firmware_update play=off play_reason=firmware_update controller_proto=2.0 app_proto=1",
        "gamepad=off gamepad_reason=firmware_update play=off play_reason=firmware_update controller_proto=2",
        "gamepad=off gamepad_reason=firmware_update play=off play_reason=firmware_update controller_proto=2 app_proto=1 x=1",
        "play=off play_reason=firmware_update gamepad=off gamepad_reason=firmware_update controller_proto=2 app_proto=1",
        "gamepad=off play=off reason=firmware_update controller_proto=2 app_proto=1",
        "gamepad=off gamepad_reason=asleep play=off play_reason=none controller_proto=- app_proto=-",
    ] {
        assert!(!common::is_gate_line(bad), "{bad}");
    }
    assert!(common::is_dialog_shown_line(
        "shown id=firmware_update_needed connected_us=1005000 controller_proto=2 app_proto=1 fw=0.2.0 bridge=0.2.1"
    ));
    assert!(!common::is_dialog_shown_line(
        "shown id=firmware_update_needed connected_us=1005000 controller_proto=2 app_proto=1 fw=v0.2.0 bridge=0.2.1"
    ));
    assert!(common::is_dialog_closed_line(
        "closed id=firmware_update_needed outcome=dismissed"
    ));
    assert!(!common::is_dialog_closed_line(
        "closed id=firmware_update_needed outcome=secondary"
    ));
    // The exact texts.
    let blocked = gate_of(Some(&mismatch(1_000_000)));
    assert_eq!(
        gate_line(&blocked),
        "gamepad=off gamepad_reason=firmware_update play=off play_reason=firmware_update controller_proto=2 app_proto=1"
    );
    assert_eq!(
        gate_line(&Gate::default()),
        "gamepad=on gamepad_reason=none play=on play_reason=none controller_proto=- app_proto=-"
    );
    // Each gate with its own reason (finding 14).
    let split = Gate {
        gamepad: Some(Block::SwitchNotReady),
        play: Some(firmware_block()),
    };
    assert_eq!(
        gate_line(&split),
        "gamepad=off gamepad_reason=switch_not_ready play=off play_reason=firmware_update controller_proto=2 app_proto=1"
    );
    let half = Gate {
        gamepad: Some(Block::Worker),
        play: None,
    };
    assert_eq!(
        gate_line(&half),
        "gamepad=off gamepad_reason=worker play=on play_reason=none controller_proto=- app_proto=-"
    );
    for b in Block::samples() {
        assert!(common::is_gate_line(&gate_line(&both(b))), "{b:?}");
    }
    let s = mismatch(1_005_000);
    let line = dialog_shown_line(&s.required_update.unwrap(), 1_005_000);
    assert_eq!(
        line,
        "shown id=firmware_update_needed connected_us=1005000 controller_proto=2 app_proto=1 fw=0.2.0 bridge=0.2.1"
    );
    assert!(common::is_dialog_shown_line(&line));
    for o in ["primary", "dismissed"] {
        assert!(common::is_dialog_closed_line(&dialog_closed_line(o)));
    }
    // One line per change, none while nothing changes; the first frame
    // without a mismatch logs nothing.
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let mut t = GateTracker::new(logger.handle());
    let open = gate_of(Some(&matching(NOW - 1_000)));
    for _ in 0..100 {
        t.observe(&open);
    }
    for _ in 0..100 {
        t.observe(&blocked);
    }
    t.observe(&gate_of(Some(&matching(NOW - 1_000))));
    for _ in 0..100 {
        t.observe(&open);
    }
    assert!(logger.stop());
    let texts = common::kind_texts(&sink.lines(), "gate");
    for l in &texts {
        assert!(common::is_gate_line(l), "{l}");
    }
    assert_eq!(
        texts,
        vec![
            "gamepad=off gamepad_reason=firmware_update play=off play_reason=firmware_update controller_proto=2 app_proto=1",
            "gamepad=on gamepad_reason=none play=on play_reason=none controller_proto=- app_proto=-",
        ]
    );
}

#[test]
fn g17_no_online_wording() {
    assert_eq!(banned_word("Sync now"), Some("sync"));
    assert_eq!(STRINGS.len(), 9);
    // Every block's hint comes from `samples()` (exhaustive: a new variant
    // does not compile until it is listed there), and every one is in
    // STRINGS.
    for b in Block::samples() {
        assert!(STRINGS.contains(&b.gamepad_hint()), "{b:?}");
    }
    for text in STRINGS
        .into_iter()
        .chain(Block::samples().iter().map(Block::gamepad_hint))
    {
        assert_eq!(banned_word(text), None, "{text}");
    }
}

#[test]
fn g17_model_has_no_egui() {
    fn hits(src: &str) -> Vec<String> {
        let code = common::code_only(src);
        let mut out = Vec::new();
        for word in code.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            if matches!(word, "egui" | "eframe" | "epaint" | "egui_kittest") {
                out.push(word.to_owned());
            }
        }
        out
    }
    // Self-test: comments and strings do not count, code does.
    assert!(hits("// egui is not used here\nlet s = \"egui\";\nfn f() {}").is_empty());
    assert_eq!(hits("use egui::Ui;"), vec!["egui".to_owned()]);
    assert_eq!(hits("let c: eframe::Frame;"), vec!["eframe".to_owned()]);
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/gating.rs"),
    )
    .unwrap();
    assert!(src.contains("pub fn evaluate("), "the right file");
    assert_eq!(hits(&src), Vec::<String>::new());
}
