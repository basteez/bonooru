//! G.14 (Story 4.5 AC 4–10, 12): the LED model `ocb_app::leds` on
//! hand-built snapshots (one case per row of the LED decision table) and on
//! the real worker stepped on the fake clock (`SimRig`), the 150 ms
//! staleness and 500 ms grace boundaries, the worker health rows, the
//! banned words, the `led` log line and the H.33 dry run. No egui here.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;

use ocb_app::app::WorkerHealth;
use ocb_app::leds::{
    self, INFO_GRACE_US, Led, LedState, LedTracker, Leds, Link3, Reason, STATUS_STALE_US,
    banned_word, led_line,
};
use ocb_core::device::{BRIDGE_PID, BRIDGE_VID, Class, PortInfo, Rules, UsbIds};
use ocb_core::link::{Phase, StatusView};
use ocb_core::protocol::consts::{MSG_SET_STATE, NAK_VERSION};
use ocb_core::protocol::{EventCode, HidProfile, Info, Mode, NEUTRAL, NakReason};
use ocb_services::log::{LogHandle, Logger, MemorySink};
use ocb_services::sim::{FakeBackend, FakePort, SimRig};
use ocb_services::worker::{
    Chosen, Clock as _, Counters, FakeClock, LastError, LinkSnapshot, SeenPort,
};

const NOW: u64 = 10_000_000;
const CU: &str = "/dev/cu.usbmodem2101";
const SERIAL: &str = "53032847189C2E9C";

// ------------------------------------------------------------ builders

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

fn status(link_ok: bool, console_ready: bool, last_event: EventCode) -> StatusView {
    StatusView {
        free: 512,
        now_us: 0,
        last_seq: 0,
        mode: Mode::Idle,
        abort_count: 0,
        last_event,
        link_ok,
        console_ready,
    }
}

fn port(name: &str, product: Option<&str>, class: Class) -> SeenPort {
    SeenPort {
        info: PortInfo {
            name: name.to_owned(),
            usb: Some(UsbIds {
                vid: BRIDGE_VID,
                pid: BRIDGE_PID,
                serial: Some(SERIAL.to_owned()),
                manufacturer: None,
                product: product.map(str::to_owned),
            }),
        },
        class,
    }
}

/// Nothing listed, no port, at `NOW`.
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

/// Our bridge listed (not open).
fn listed() -> LinkSnapshot {
    let mut s = empty();
    s.ports = vec![port(CU, Some("Bonooru Bridge"), Class::Ours)];
    s
}

/// Our bridge open, session opened `age` µs before `NOW`.
fn opening(age: u64) -> LinkSnapshot {
    let mut s = listed();
    s.chosen = Some(Chosen {
        name: CU.to_owned(),
        serial: Some(SERIAL.to_owned()),
        class: Class::Ours,
        open: true,
    });
    s.phase = Phase::Opening;
    s.session_since_us = Some(NOW - age);
    s
}

/// Connected (INFO fw 0.3.0, bridge 0.3.1), the last STATUS `age` µs old.
fn connected(age: u64, st: StatusView) -> LinkSnapshot {
    let mut s = opening(2_000_000);
    s.phase = Phase::Connected;
    s.info = Some(info([0, 3, 0], [0, 3, 1], 1));
    s.connected_us = Some(NOW - 1_000_000);
    s.status = Some((st, NOW - age));
    s
}

fn ready() -> StatusView {
    status(true, true, EventCode::None)
}

fn run(s: &LinkSnapshot) -> Leds {
    leds::evaluate(Some(s), 0, WorkerHealth::Running)
}

fn sig(led: &Led) -> Sig {
    (led.state, led.name(), led.reason)
}

fn want(state: LedState, name: &str, reason: Reason) -> Sig {
    (state, name.to_owned(), reason)
}

// ------------------------------------------------------------ the table

/// (state, name, reason) of one LED.
type Sig = (LedState, String, Reason);

/// Every row of the decision table: (row, LEDs, link, expected).
#[allow(clippy::too_many_lines)]
fn table() -> Vec<(&'static str, Leds, Link3, Sig)> {
    use LedState::{Error, Ok, Unknown, Warning};
    use Link3::{BridgeController as C, ControllerSwitch as S, PcBridge as P};
    let fresh = connected(20_000, ready());
    let mut rows = Vec::new();
    let mut add = |row, leds: Leds, link, w| rows.push((row, leds, link, w));
    // PC ↔ bridge.
    add(
        "P1",
        leds::evaluate(Some(&fresh), 0, WorkerHealth::Stalled),
        P,
        want(Unknown, "Bridge", Reason::WorkerStalled),
    );
    add(
        "P2 ended",
        leds::evaluate(Some(&fresh), 0, WorkerHealth::Ended),
        P,
        want(Unknown, "Bridge", Reason::WorkerEnded),
    );
    add(
        "P2 not started",
        leds::evaluate(None, 0, WorkerHealth::NotStarted),
        P,
        want(Unknown, "Bridge", Reason::WorkerEnded),
    );
    add(
        "P3",
        leds::evaluate(None, 0, WorkerHealth::Running),
        P,
        want(Unknown, "Bridge", Reason::Start),
    );
    add(
        "P4",
        run(&fresh),
        P,
        want(Ok, "Bridge v0.3.1", Reason::Info),
    );
    let mut req = opening(100_000);
    req.phase = Phase::Incompatible;
    req.required_update = Some(info([0, 2, 0], [0, 2, 1], 2));
    req.connected_us = Some(NOW - 50_000);
    add(
        "P5",
        run(&req),
        P,
        want(Ok, "Bridge v0.2.1", Reason::RequiredUpdate),
    );
    let mut perm = listed();
    perm.last_error = Some(LastError::PermissionDenied {
        port: CU.to_owned(),
        message: "MSG".to_owned(),
    });
    add(
        "P6",
        run(&perm),
        P,
        want(Error, "Bridge – no access", Reason::Permission),
    );
    let mut busy = listed();
    busy.last_error = Some(LastError::Busy {
        port: CU.to_owned(),
    });
    add(
        "P7",
        run(&busy),
        P,
        want(Warning, "Bridge – in use", Reason::Busy),
    );
    add(
        "P8",
        run(&opening(100_000)),
        P,
        want(Ok, "Bridge – found", Reason::OursOpen),
    );
    let mut unc = opening(100_000);
    unc.ports = vec![port("COM5", None, Class::Unconfirmed)];
    unc.chosen.as_mut().unwrap().name = "COM5".to_owned();
    unc.chosen.as_mut().unwrap().class = Class::Unconfirmed;
    add(
        "P9",
        run(&unc),
        P,
        want(Unknown, "Bridge", Reason::Unconfirmed),
    );
    add(
        "P10",
        run(&listed()),
        P,
        want(Unknown, "Bridge", Reason::Waiting),
    );
    let mut foreign = empty();
    foreign.ports = vec![port(
        "/dev/cu.usbmodem201",
        Some("RiiDraw"),
        Class::Foreign {
            product: "RiiDraw".to_owned(),
        },
    )];
    add(
        "P11",
        run(&foreign),
        P,
        want(Warning, "Bridge – other board", Reason::Foreign),
    );
    add(
        "P12",
        run(&empty()),
        P,
        want(Error, "Bridge – not found", Reason::NotFound),
    );
    // Bridge ↔ controller.
    add(
        "C1",
        leds::evaluate(Some(&fresh), 0, WorkerHealth::Stalled),
        C,
        want(Unknown, "Controller", Reason::WorkerStalled),
    );
    add(
        "C2",
        run(&req),
        C,
        want(Warning, "Controller v0.2.0", Reason::RequiredUpdate),
    );
    add(
        "C3",
        run(&listed()),
        C,
        want(Unknown, "Controller", Reason::NoPort),
    );
    add(
        "C4",
        run(&opening(INFO_GRACE_US)),
        C,
        want(Unknown, "Controller", Reason::Opening),
    );
    add(
        "C5",
        run(&opening(INFO_GRACE_US + 1)),
        C,
        want(Error, "Controller – not answering", Reason::NoAnswer),
    );
    add(
        "C6",
        run(&connected(STATUS_STALE_US + 1, ready())),
        C,
        want(Error, "Controller – not answering", Reason::Stale),
    );
    let mut nak_nl = connected(20_000, ready());
    nak_nl.last_nak = Some((NakReason::NoLink, NOW - 10_000));
    add(
        "C7",
        run(&nak_nl),
        C,
        want(Error, "Controller – no link", Reason::NakNoLink),
    );
    add(
        "C8",
        run(&connected(20_000, status(false, true, EventCode::None))),
        C,
        want(Error, "Controller – no link", Reason::LinkDown),
    );
    add(
        "C9",
        run(&fresh),
        C,
        want(Ok, "Controller v0.3.0", Reason::Info),
    );
    // C7 is a latch only until a newer STATUS.
    let mut nak_old = connected(20_000, ready());
    nak_old.last_nak = Some((NakReason::NoLink, NOW - 30_000));
    add(
        "C7 cleared by a newer STATUS",
        run(&nak_old),
        C,
        want(Ok, "Controller v0.3.0", Reason::Info),
    );
    // Controller ↔ Switch.
    add(
        "S1",
        run(&opening(100_000)),
        S,
        want(Unknown, "Switch", Reason::Waiting),
    );
    add(
        "S1 worker",
        leds::evaluate(Some(&fresh), 0, WorkerHealth::Ended),
        S,
        want(Unknown, "Switch", Reason::WorkerEnded),
    );
    let mut nak_nr = connected(20_000, ready());
    nak_nr.last_nak = Some((NakReason::NotReady, NOW - 10_000));
    add(
        "S2",
        run(&nak_nr),
        S,
        want(Warning, "Switch – not ready", Reason::NakNotReady),
    );
    let mut nak_nr_old = connected(20_000, ready());
    nak_nr_old.last_nak = Some((NakReason::NotReady, NOW - 30_000));
    add(
        "S2 cleared by a newer STATUS",
        run(&nak_nr_old),
        S,
        want(Ok, "Switch – ready", Reason::ConsoleReady),
    );
    add(
        "S3",
        run(&connected(20_000, status(true, false, EventCode::None))),
        S,
        want(Warning, "Switch – not ready", Reason::ConsoleNotReady),
    );
    add(
        "S4",
        run(&fresh),
        S,
        want(Ok, "Switch – ready", Reason::ConsoleReady),
    );
    let mut no_status = connected(20_000, ready());
    no_status.status = None;
    no_status.connected_us = Some(NOW - 20_000);
    add(
        "S4 needs a STATUS",
        run(&no_status),
        S,
        want(Unknown, "Switch", Reason::Waiting),
    );
    rows
}

#[test]
fn g14_table() {
    for (row, leds, link, w) in table() {
        assert_eq!(sig(leds.get(link)), w, "row {row}");
    }
}

#[test]
fn g14_tooltips_name_the_fix() {
    // P6: the exact Story 4.4 message is the fix line.
    let mut perm = listed();
    let msg = "Bonooru can't open /dev/ttyACM0: your user is not in the dialout group. Run: sudo usermod -aG dialout $USER — then log out and back in.";
    perm.ports = vec![port("/dev/ttyACM0", Some("Bonooru Bridge"), Class::Ours)];
    perm.last_error = Some(LastError::PermissionDenied {
        port: "/dev/ttyACM0".to_owned(),
        message: msg.to_owned(),
    });
    assert_eq!(
        run(&perm).pc_bridge.tooltip,
        vec!["Bridge – no access".to_owned(), msg.to_owned()]
    );
    // A problem of a port no longer listed is not shown.
    perm.ports.clear();
    assert_eq!(run(&perm).pc_bridge.reason, Reason::NotFound);
    // P9 (OQ-5): the port and the way to confirm it.
    let mut unc = opening(100_000);
    unc.ports = vec![port("COM5", None, Class::Unconfirmed)];
    unc.chosen.as_mut().unwrap().name = "COM5".to_owned();
    unc.chosen.as_mut().unwrap().class = Class::Unconfirmed;
    let l = run(&unc).pc_bridge;
    assert_eq!(
        l.tooltip,
        vec![
            "A board that may be the bridge is on COM5. Bonooru can confirm it once the controller answers.".to_owned(),
            "Wire the controller to the bridge and plug it into the Switch.".to_owned()
        ]
    );
    assert_eq!(l.detail, None, "unknown: label only");
    // P11 names the board.
    let mut foreign = empty();
    let f = |name: &str, product: &str| {
        port(
            name,
            Some(product),
            Class::Foreign {
                product: product.to_owned(),
            },
        )
    };
    foreign.ports = vec![f("/dev/cu.usbmodem201", "RiiDraw")];
    assert_eq!(
        run(&foreign).pc_bridge.tooltip[0],
        "Found a “RiiDraw” board – not a Bonooru bridge."
    );
    foreign.ports.push(f("/dev/cu.usbmodem301", "2wiCC COM"));
    assert!(run(&foreign).pc_bridge.tooltip[0].ends_with("– and 1 more."));
    // S3: "Undock the Switch." only after USB_SUSPENDED.
    let plain = run(&connected(20_000, status(true, false, EventCode::None))).switch;
    assert_eq!(
        plain.tooltip,
        vec![
            "Switch – not ready".to_owned(),
            "Turn on Pro Controller Wired Communication, wake the Switch, check the cable."
                .to_owned()
        ]
    );
    let suspended = run(&connected(
        20_000,
        status(true, false, EventCode::UsbSuspended),
    ))
    .switch;
    assert_eq!(suspended.name(), "Switch – not ready");
    assert_eq!(suspended.tooltip.last().unwrap(), "Undock the Switch.");
    assert_eq!(suspended.tooltip.len(), 3);
    // C2 (Story 4.6 D6): neutral, a board can be newer than the app.
    let mut req = opening(100_000);
    req.phase = Phase::Incompatible;
    req.required_update = Some(info([0, 2, 0], [0, 2, 1], 2));
    assert_eq!(
        run(&req).bridge_controller.tooltip,
        vec![
            "This firmware doesn't match this app.".to_owned(),
            "Update both boards: hold BOOT, plug in, drag the matching .uf2 onto the RPI-RP2 drive."
                .to_owned()
        ]
    );
    // Versions in the tooltip with the port (D4).
    let ok = run(&connected(20_000, ready()));
    assert_eq!(
        ok.pc_bridge.tooltip,
        vec![format!("Bridge v0.3.1 on {CU}.")]
    );
    assert_eq!(
        ok.bridge_controller.tooltip,
        vec!["Controller v0.3.0, talking to the bridge.".to_owned()]
    );
}

#[test]
fn g14_no_snapshot() {
    let l = leds::evaluate(None, 0, WorkerHealth::Running);
    for (link, led) in l.all() {
        assert_eq!(
            (led.state, led.reason, led.name()),
            (LedState::Unknown, Reason::Start, link.label().to_owned())
        );
        assert_eq!(led.tooltip, vec!["Checking…".to_owned()]);
    }
    assert_eq!(l.next_change_in_us, None);
}

#[test]
fn g14_staleness_boundary() {
    assert_eq!(STATUS_STALE_US, 150_000);
    // age = (now_us − rx_us) + since_received_us, both parts non-zero.
    let s = connected(100_000, ready());
    let at = |since| leds::evaluate(Some(&s), since, WorkerHealth::Running);
    let fresh = at(50_000);
    assert_eq!(fresh.bridge_controller.reason, Reason::Info);
    assert_eq!(fresh.bridge_controller.status_age_us, Some(150_000));
    assert_eq!(fresh.switch.state, LedState::Ok);
    let stale = at(50_001);
    assert_eq!(
        sig(&stale.bridge_controller),
        want(LedState::Error, "Controller – not answering", Reason::Stale)
    );
    assert_eq!(stale.bridge_controller.status_age_us, Some(150_001));
    assert_eq!(
        sig(&stale.switch),
        want(LedState::Unknown, "Switch", Reason::Waiting)
    );
    // The UI's own time alone makes it stale (the snapshot froze).
    let frozen = connected(0, ready());
    let l = leds::evaluate(Some(&frozen), 150_001, WorkerHealth::Running);
    assert_eq!(l.bridge_controller.reason, Reason::Stale);
    // Before the first STATUS the INFO time counts.
    let mut s = connected(0, ready());
    s.status = None;
    s.connected_us = Some(NOW - 100_000);
    assert_eq!(
        leds::evaluate(Some(&s), 50_000, WorkerHealth::Running)
            .bridge_controller
            .reason,
        Reason::Info
    );
    assert_eq!(
        leds::evaluate(Some(&s), 50_001, WorkerHealth::Running)
            .bridge_controller
            .reason,
        Reason::Stale
    );
    // A snapshot whose clock is behind its STATUS never underflows.
    let mut s = connected(0, ready());
    s.status = Some((ready(), NOW + 5));
    assert_eq!(run(&s).bridge_controller.status_age_us, Some(0));
}

#[test]
fn g14_next_deadline() {
    // Fresh STATUS: 150 000 − age + 1.
    let s = connected(100_000, ready());
    let l = leds::evaluate(Some(&s), 20_000, WorkerHealth::Running);
    assert_eq!(l.next_change_in_us, Some(150_000 - 120_000 + 1));
    // That much later the LED has changed, one µs earlier it has not.
    let next = l.next_change_in_us.unwrap();
    let at = |since| leds::evaluate(Some(&s), since, WorkerHealth::Running);
    assert_eq!(at(20_000 + next - 1).bridge_controller.reason, Reason::Info);
    assert_eq!(at(20_000 + next).bridge_controller.reason, Reason::Stale);
    // Grace: 500 000 − age + 1.
    let s = opening(300_000);
    let l = leds::evaluate(Some(&s), 10_000, WorkerHealth::Running);
    assert_eq!(l.next_change_in_us, Some(500_000 - 310_000 + 1));
    // The latched rows keep the staleness timer.
    let mut nak = connected(20_000, status(false, true, EventCode::None));
    assert_eq!(run(&nak).next_change_in_us, Some(130_001));
    nak.last_nak = Some((NakReason::NoLink, NOW));
    assert_eq!(run(&nak).next_change_in_us, Some(130_001));
    // No timed rule: stale, grace over, no port, not found, worker dead.
    for (what, l) in [
        ("stale", run(&connected(200_000, ready()))),
        ("no answer", run(&opening(600_000))),
        ("no port", run(&listed())),
        ("not found", run(&empty())),
        (
            "stalled",
            leds::evaluate(Some(&connected(0, ready())), 0, WorkerHealth::Stalled),
        ),
    ] {
        assert_eq!(l.next_change_in_us, None, "{what}");
    }
}

#[test]
fn g14_info_grace() {
    assert_eq!(INFO_GRACE_US, 500_000);
    let l = run(&opening(500_000));
    assert_eq!(
        sig(&l.bridge_controller),
        want(LedState::Unknown, "Controller", Reason::Opening)
    );
    assert_eq!(l.bridge_controller.session_age_us, Some(500_000));
    assert_eq!(l.switch.state, LedState::Unknown);
    // The UI's time counts too.
    let s = opening(400_000);
    let l = leds::evaluate(Some(&s), 100_001, WorkerHealth::Running);
    assert_eq!(
        sig(&l.bridge_controller),
        want(
            LedState::Error,
            "Controller – not answering",
            Reason::NoAnswer
        )
    );
    assert_eq!(l.bridge_controller.session_age_us, Some(500_001));
    assert_eq!(
        sig(&l.switch),
        want(LedState::Unknown, "Switch", Reason::Waiting)
    );
    // A reboot re-opens the session (the worker resets `session_since_us`):
    // unknown again, the grace from the re-open.
    let mut s = connected(20_000, ready());
    s.phase = Phase::Opening;
    s.info = None;
    s.status = None;
    s.connected_us = None;
    s.session_since_us = Some(NOW - 10_000);
    let l = run(&s);
    assert_eq!(l.bridge_controller.reason, Reason::Opening);
    assert_eq!(l.pc_bridge.reason, Reason::OursOpen);
    assert_eq!(l.switch.state, LedState::Unknown);
}

#[test]
fn g14_worker_health() {
    let fresh = connected(10_000, ready());
    for (health, reason, tip) in [
        (
            WorkerHealth::Stalled,
            Reason::WorkerStalled,
            vec![
                "Bonooru's USB check is not responding.",
                "Wait a moment. If it stays like this, restart Bonooru.",
            ],
        ),
        (
            WorkerHealth::Ended,
            Reason::WorkerEnded,
            vec![
                "Bonooru stopped checking the USB ports.",
                "Restart Bonooru. If it happens again, send the file bonooru.log.",
            ],
        ),
        (
            WorkerHealth::NotStarted,
            Reason::WorkerEnded,
            vec![
                "Bonooru stopped checking the USB ports.",
                "Restart Bonooru. If it happens again, send the file bonooru.log.",
            ],
        ),
    ] {
        let l = leds::evaluate(Some(&fresh), 0, health);
        for (link, led) in l.all() {
            assert_eq!(
                (led.state, led.reason, led.name()),
                (LedState::Unknown, reason, link.label().to_owned()),
                "{health:?} {link:?}"
            );
        }
        assert_eq!(l.pc_bridge.tooltip, tip, "{health:?}");
        assert_eq!(l.next_change_in_us, None);
    }
}

/// Every string an LED can show, from the table rows plus samples of the
/// rows whose text depends on the input.
fn every_led() -> Vec<Led> {
    let mut out: Vec<Led> = Vec::new();
    for (_, l, _, _) in table() {
        out.extend(l.all().map(|(_, led)| led.clone()));
    }
    out
}

#[test]
fn g14_no_online_wording() {
    assert_eq!(banned_word("Go ONLINE now"), Some("online"));
    assert_eq!(banned_word("Free Play"), Some("free play"));
    assert_eq!(banned_word("Bridge – found"), None);
    let all = every_led();
    // Every reason is produced, so a string added later is checked too.
    for r in Reason::ALL {
        assert!(all.iter().any(|l| l.reason == r), "no case for {r:?}");
    }
    let mut n = 0;
    for led in &all {
        for text in [led.name(), led.accessible_name()]
            .into_iter()
            .chain(led.tooltip.iter().cloned())
        {
            assert_eq!(banned_word(&text), None, "{text}");
            n += 1;
        }
    }
    assert!(n > 60, "{n} strings checked");
}

// ------------------------------------------------------------ led lines

/// The G.14 regex of the `led` line, by hand (no regex crate):
/// `^(pc_bridge|bridge_controller|switch)=(ok|warning|error|unknown)
/// name="[^"]+" reason=[a-z_]+ status_age_us=(\d+|-) session_age_us=(\d+|-)$`.
fn is_led_line(t: &str) -> bool {
    let num_or_dash =
        |v: &str| v == "-" || (!v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()));
    let Some((link, rest)) = t.split_once('=') else {
        return false;
    };
    if !["pc_bridge", "bridge_controller", "switch"].contains(&link) {
        return false;
    }
    let Some((state, rest)) = rest.split_once(" name=\"") else {
        return false;
    };
    if !["ok", "warning", "error", "unknown"].contains(&state) {
        return false;
    }
    let Some((name, rest)) = rest.split_once("\" reason=") else {
        return false;
    };
    if name.is_empty() || name.contains('"') {
        return false;
    }
    let Some((reason, rest)) = rest.split_once(" status_age_us=") else {
        return false;
    };
    if reason.is_empty() || !reason.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
        return false;
    }
    let Some((status, session)) = rest.split_once(" session_age_us=") else {
        return false;
    };
    num_or_dash(status) && num_or_dash(session)
}

/// The text of each `led` line of a memory log.
fn led_texts(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|l| l.split_once(" led ").map(|(_, t)| t.to_owned()))
        .collect()
}

#[test]
fn g14_led_log_lines() {
    // The matcher itself.
    assert!(is_led_line(
        "bridge_controller=error name=\"Controller – not answering\" reason=stale status_age_us=163400 session_age_us=-"
    ));
    for bad in [
        "bridge=error name=\"x\" reason=stale status_age_us=1 session_age_us=-",
        "switch=bad name=\"x\" reason=stale status_age_us=1 session_age_us=-",
        "switch=ok name=\"\" reason=stale status_age_us=1 session_age_us=-",
        "switch=ok name=\"x\" reason=Stale status_age_us=1 session_age_us=-",
        "switch=ok name=\"x\" reason=stale status_age_us=1.5 session_age_us=-",
        "switch=ok name=\"x\" reason=stale status_age_us=1",
        "switch=ok name=\"x\" reason=stale status_age_us=1 session_age_us=",
    ] {
        assert!(!is_led_line(bad), "{bad}");
    }
    // At the boundary the µs say it: rounded down to ms, a correct change
    // 1 µs past 150 ms would read 150 (the old format's flaw).
    let edge = run(&connected(150_001, ready()));
    assert_eq!(
        led_line(Link3::BridgeController, &edge.bridge_controller),
        "bridge_controller=error name=\"Controller – not answering\" reason=stale status_age_us=150001 session_age_us=-"
    );
    let edge = run(&opening(500_001));
    assert!(
        led_line(Link3::BridgeController, &edge.bridge_controller)
            .ends_with("session_age_us=500001")
    );
    // The exact text of one line.
    let stale = run(&connected(163_400, ready()));
    assert_eq!(
        led_line(Link3::BridgeController, &stale.bridge_controller),
        "bridge_controller=error name=\"Controller – not answering\" reason=stale status_age_us=163400 session_age_us=-"
    );
    let grace = run(&opening(501_999));
    assert_eq!(
        led_line(Link3::BridgeController, &grace.bridge_controller),
        "bridge_controller=error name=\"Controller – not answering\" reason=no_answer status_age_us=- session_age_us=501999"
    );
    // One line per change, none while nothing changes.
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let mut t = LedTracker::new(logger.handle());
    let start = leds::evaluate(None, 0, WorkerHealth::Running);
    for _ in 0..101 {
        t.observe(&start);
    }
    // Only the controller changes (the ages change on every call: no line).
    t.observe(&run(&opening(100_000)));
    t.observe(&run(&opening(200_000)));
    t.observe(&run(&opening(600_000)));
    for age in 0..100 {
        t.observe(&run(&opening(600_000 + age * 1000)));
    }
    assert!(logger.stop());
    let texts = led_texts(&sink.lines());
    for l in &texts {
        assert!(is_led_line(l), "{l}");
    }
    assert_eq!(
        texts,
        vec![
            "pc_bridge=unknown name=\"Bridge\" reason=start status_age_us=- session_age_us=-",
            "bridge_controller=unknown name=\"Controller\" reason=start status_age_us=- session_age_us=-",
            "switch=unknown name=\"Switch\" reason=start status_age_us=- session_age_us=-",
            // A new reason is a change too (Controller and Switch keep
            // state and name), the ages alone are not.
            "pc_bridge=ok name=\"Bridge – found\" reason=ours_open status_age_us=- session_age_us=-",
            "bridge_controller=unknown name=\"Controller\" reason=opening status_age_us=- session_age_us=100000",
            "switch=unknown name=\"Switch\" reason=waiting status_age_us=- session_age_us=-",
            "bridge_controller=error name=\"Controller – not answering\" reason=no_answer status_age_us=- session_age_us=600000",
        ]
    );
}

#[test]
fn g14_model_has_no_egui() {
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
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/leds.rs"),
    )
    .unwrap();
    assert!(src.contains("pub fn evaluate("), "the right file");
    assert_eq!(hits(&src), Vec::<String>::new());
}

// ------------------------------------------------------------ on the worker

/// The real worker on the fake clock with a UI-like evaluation once per
/// worker pass (1 ms): `since_received_us` is the time since the snapshot
/// was published, as the app measures it.
struct Rig {
    r: SimRig,
    snap: Option<Arc<LinkSnapshot>>,
    received_at: u64,
    tracker: LedTracker,
    leds: Leds,
}

impl Rig {
    fn new(ports: Vec<FakePort>, rules: Rules, log: LogHandle) -> Self {
        let mut b = FakeBackend::new(FakeClock::at(0));
        for p in ports {
            b = b.with(p);
        }
        let mut tracker = LedTracker::new(log);
        let leds = leds::evaluate(None, 0, WorkerHealth::Running);
        tracker.observe(&leds);
        Self {
            r: SimRig::new(b, rules, LogHandle::disabled()),
            snap: None,
            received_at: 0,
            tracker,
            leds,
        }
    }

    fn now(&self) -> u64 {
        self.r.clock.now_us()
    }

    fn step(&mut self) {
        let t = self.now();
        self.r.step();
        if let Some(s) = &self.r.snapshot
            && !self.snap.as_ref().is_some_and(|p| Arc::ptr_eq(p, s))
        {
            self.snap = Some(Arc::clone(s));
            self.received_at = t;
        }
        self.leds = leds::evaluate(
            self.snap.as_deref(),
            t - self.received_at,
            WorkerHealth::Running,
        );
        self.tracker.observe(&self.leds);
    }

    fn run_until(&mut self, t: u64) {
        while self.now() < t {
            self.step();
        }
    }

    /// Steps until `f` holds for the LEDs (at most until `limit`); returns
    /// the time of that pass.
    fn until(&mut self, limit: u64, f: impl Fn(&Leds) -> bool) -> u64 {
        while self.now() < limit {
            let t = self.now();
            self.step();
            if f(&self.leds) {
                return t;
            }
        }
        panic!("not reached before {limit}: {:#?}", self.leds);
    }
}

fn states(l: &Leds) -> [(LedState, Reason); 3] {
    l.all().map(|(_, led)| (led.state, led.reason))
}

#[test]
fn g14_on_sim_rig() {
    use LedState::{Error, Ok, Unknown, Warning};
    let bridge = FakePort::bridge(CU, SERIAL, Some("Bonooru Bridge"))
        .present(vec![(0, Some(6_000_000))])
        .hook(1_000_000, Box::new(|c| c.set_console_ready(false)))
        .hook(1_500_000, Box::new(|c| c.set_console_ready(true)))
        .hook(
            2_000_000,
            Box::new(|c| c.inject_nak(MSG_SET_STATE, NakReason::NotReady)),
        )
        .hook(3_000_000, Box::new(|c| c.set_link_ok(false)))
        .hook(3_500_000, Box::new(|c| c.set_link_ok(true)))
        .mute_at(4_000_000);
    let mut r = Rig::new(vec![bridge], Rules::MACOS, LogHandle::disabled());
    // Plugged → INFO → STATUS: all ok well within 1 s.
    let t = r.until(1_000_000, |l| states(l).iter().all(|s| s.0 == Ok));
    assert!(t < 100_000, "all ok at {t} µs");
    assert_eq!(r.leds.pc_bridge.name(), "Bridge v0.0.0");
    assert_eq!(r.leds.bridge_controller.name(), "Controller v0.0.0");
    // console_ready off → Switch warning; on → ok.
    r.until(1_200_000, |l| {
        l.switch.state == Warning && l.switch.reason == Reason::ConsoleNotReady
    });
    r.until(1_700_000, |l| l.switch.state == Ok);
    // NAK NOT_READY on SET_STATE (a later story's command, driven through
    // the session): latched until the next STATUS.
    r.run_until(2_010_000);
    r.r.core.session_mut().unwrap().set_state(NEUTRAL).unwrap();
    r.until(2_100_000, |l| l.switch.reason == Reason::NakNotReady);
    r.until(2_200_000, |l| l.switch.reason == Reason::ConsoleReady);
    // link_ok 0 → Controller error, Switch unknown.
    let t = r.until(3_200_000, |l| {
        l.bridge_controller.state == Error && l.bridge_controller.reason == Reason::LinkDown
    });
    assert!(t >= 3_000_000);
    assert_eq!(r.leds.switch.state, Unknown);
    r.until(3_900_000, |l| states(l).iter().all(|s| s.0 == Ok));
    // Controller muted: stale after 150 ms of no STATUS.
    let t = r.until(4_400_000, |l| l.bridge_controller.reason == Reason::Stale);
    let age = r.leds.bridge_controller.status_age_us.unwrap();
    assert!((150_001..=151_000).contains(&age), "{age}");
    assert!((4_100_000..=4_200_000).contains(&t), "{t}");
    assert_eq!(states(&r.leds)[2], (Unknown, Reason::Waiting));
    // Port vanished: not found, the other two unknown.
    r.until(7_100_000, |l| l.pc_bridge.reason == Reason::NotFound);
    assert_eq!(states(&r.leds)[1], (Unknown, Reason::NoPort));
    assert_eq!(states(&r.leds)[2], (Unknown, Reason::Waiting));

    // A foreign board alone.
    let foreign = FakePort::bridge("/dev/cu.usbmodem201", "F", Some("RiiDraw COM")).silent();
    let mut r = Rig::new(vec![foreign], Rules::MACOS, LogHandle::disabled());
    r.run_until(50_000);
    assert_eq!(states(&r.leds)[0], (Warning, Reason::Foreign));
    assert!(r.leds.pc_bridge.tooltip[0].contains("RiiDraw COM"));

    // Windows: unconfirmed until INFO (OQ-5), then ours with versions.
    let unc_silent = FakePort::bridge("COM5", SERIAL, None).silent();
    let mut r = Rig::new(vec![unc_silent], Rules::WINDOWS, LogHandle::disabled());
    r.run_until(100_000);
    assert_eq!(states(&r.leds)[0], (Unknown, Reason::Unconfirmed));
    assert_eq!(states(&r.leds)[1], (Unknown, Reason::Opening));
    r.run_until(600_000);
    assert_eq!(states(&r.leds)[1], (Error, Reason::NoAnswer));
    let unc = FakePort::bridge("COM5", SERIAL, None);
    let mut r = Rig::new(vec![unc], Rules::WINDOWS, LogHandle::disabled());
    r.until(200_000, |l| l.pc_bridge.reason == Reason::Info);
    assert_eq!(r.leds.pc_bridge.name(), "Bridge v0.0.0");
}

/// Task 12: the H.33 sequence on `ocb-sim`, asserting the exact `led`
/// lines the bench must show (`cargo test -p ocb-app -- --nocapture
/// h33_dry_run` prints them).
#[test]
fn g14_h33_dry_run() {
    // Step 1–5 on one run of the app.
    let bridge = FakePort::bridge(CU, SERIAL, Some("Bonooru Bridge"))
        .present(vec![(0, Some(6_500_000)), (7_200_000, None)])
        // 2: Wired Communication off, then on.
        .hook(1_000_000, Box::new(|c| c.set_console_ready(false)))
        .hook(2_000_000, Box::new(|c| c.set_console_ready(true)))
        // 3: the controller loses power (out of the Switch).
        .mute_at(3_000_000)
        // 4: back: a freshly booted controller; the app's next frame gets
        // NAK VERSION (modelled by the controller sending it).
        .unmute_at(4_000_000)
        .hook(
            4_000_000,
            Box::new(|c| {
                c.reboot();
                c.inject_raw_nak(NAK_VERSION, 0, 0);
            }),
        );
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let mut r = Rig::new(vec![bridge], Rules::MACOS, logger.handle());
    r.run_until(9_000_000);
    drop(r);
    assert!(logger.stop());
    let run1 = led_texts(&sink.lines());
    // Step 6: a new app run, the bridge alone.
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let alone = FakePort::bridge(CU, SERIAL, Some("Bonooru Bridge")).silent();
    let mut r = Rig::new(vec![alone], Rules::MACOS, logger.handle());
    r.run_until(2_000_000);
    drop(r);
    assert!(logger.stop());
    let run2 = led_texts(&sink.lines());
    println!("H.33 dry run, steps 1-5 (one app run):");
    for l in &run1 {
        println!("  led {l}");
    }
    println!("H.33 dry run, step 6 (bridge alone, new app run):");
    for l in &run2 {
        println!("  led {l}");
    }
    for l in run1.iter().chain(&run2) {
        assert!(is_led_line(l), "{l}");
    }
    let expect1: Vec<String> = H33_RUN1.iter().map(|s| (*s).to_owned()).collect();
    let expect2: Vec<String> = H33_RUN2.iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(run1, expect1);
    assert_eq!(run2, expect2);
}

/// H.33 steps 1–5: the expected `led` lines (also the bench's reading list).
const H33_RUN1: &[&str] = &[
    // Start.
    "pc_bridge=unknown name=\"Bridge\" reason=start status_age_us=- session_age_us=-",
    "bridge_controller=unknown name=\"Controller\" reason=start status_age_us=- session_age_us=-",
    "switch=unknown name=\"Switch\" reason=start status_age_us=- session_age_us=-",
    // Step 1: open, INFO, STATUS.
    "pc_bridge=ok name=\"Bridge – found\" reason=ours_open status_age_us=- session_age_us=-",
    "bridge_controller=unknown name=\"Controller\" reason=opening status_age_us=- session_age_us=0",
    "switch=unknown name=\"Switch\" reason=waiting status_age_us=- session_age_us=-",
    "pc_bridge=ok name=\"Bridge v0.0.0\" reason=info status_age_us=- session_age_us=-",
    "bridge_controller=ok name=\"Controller v0.0.0\" reason=info status_age_us=1000 session_age_us=-",
    "switch=ok name=\"Switch – ready\" reason=console_ready status_age_us=- session_age_us=-",
    // Step 2: Wired Communication off, on.
    "switch=warning name=\"Switch – not ready\" reason=console_not_ready status_age_us=- session_age_us=-",
    "switch=ok name=\"Switch – ready\" reason=console_ready status_age_us=- session_age_us=-",
    // Step 3: controller out of the Switch: stale at 151 000 µs (the dry run evaluates once per 1 ms worker pass).
    "bridge_controller=error name=\"Controller – not answering\" reason=stale status_age_us=151000 session_age_us=-",
    "switch=unknown name=\"Switch\" reason=waiting status_age_us=- session_age_us=-",
    // Step 4: back, rebooted (NAK VERSION): the session HELLOs again.
    "pc_bridge=ok name=\"Bridge – found\" reason=ours_open status_age_us=- session_age_us=-",
    "bridge_controller=unknown name=\"Controller\" reason=opening status_age_us=- session_age_us=0",
    "pc_bridge=ok name=\"Bridge v0.0.0\" reason=info status_age_us=- session_age_us=-",
    "bridge_controller=ok name=\"Controller v0.0.0\" reason=info status_age_us=2000 session_age_us=-",
    "switch=ok name=\"Switch – ready\" reason=console_ready status_age_us=- session_age_us=-",
    // Step 5: bridge out: the unknown lines at `port_lost`, not found at `port_vanished` (next scan); then back.
    "pc_bridge=unknown name=\"Bridge\" reason=waiting status_age_us=- session_age_us=-",
    "bridge_controller=unknown name=\"Controller\" reason=no_port status_age_us=- session_age_us=-",
    "switch=unknown name=\"Switch\" reason=waiting status_age_us=- session_age_us=-",
    "pc_bridge=error name=\"Bridge – not found\" reason=not_found status_age_us=- session_age_us=-",
    "pc_bridge=ok name=\"Bridge – found\" reason=ours_open status_age_us=- session_age_us=-",
    "bridge_controller=unknown name=\"Controller\" reason=opening status_age_us=- session_age_us=0",
    "pc_bridge=ok name=\"Bridge v0.0.0\" reason=info status_age_us=- session_age_us=-",
    "bridge_controller=ok name=\"Controller v0.0.0\" reason=info status_age_us=1000 session_age_us=-",
    "switch=ok name=\"Switch – ready\" reason=console_ready status_age_us=- session_age_us=-",
];
/// H.33 step 6 (bridge alone, OQ-6 on macOS).
const H33_RUN2: &[&str] = &[
    "pc_bridge=unknown name=\"Bridge\" reason=start status_age_us=- session_age_us=-",
    "bridge_controller=unknown name=\"Controller\" reason=start status_age_us=- session_age_us=-",
    "switch=unknown name=\"Switch\" reason=start status_age_us=- session_age_us=-",
    // The bridge found; the grace runs, then no answer after 500 000 µs.
    "pc_bridge=ok name=\"Bridge – found\" reason=ours_open status_age_us=- session_age_us=-",
    "bridge_controller=unknown name=\"Controller\" reason=opening status_age_us=- session_age_us=0",
    "switch=unknown name=\"Switch\" reason=waiting status_age_us=- session_age_us=-",
    "bridge_controller=error name=\"Controller – not answering\" reason=no_answer status_age_us=- session_age_us=501000",
];

/// AC 11 (review decision 2026-11-27): the screen-reader name says the
/// state where the visible text does not.
#[test]
fn g14_accessible_names_say_the_state() {
    let unknown = run(&opening(100_000));
    assert_eq!(unknown.bridge_controller.name(), "Controller");
    assert_eq!(
        unknown.bridge_controller.accessible_name(),
        "Controller – unknown"
    );
    assert_eq!(unknown.switch.accessible_name(), "Switch – unknown");
    let mut req = opening(100_000);
    req.phase = Phase::Incompatible;
    req.required_update = Some(info([0, 2, 0], [0, 2, 1], 2));
    let l = run(&req);
    assert_eq!(
        l.pc_bridge.accessible_name(),
        "Bridge v0.2.1 – update required"
    );
    assert_eq!(
        l.bridge_controller.accessible_name(),
        "Controller v0.2.0 – update required"
    );
    // Elsewhere the name is the visible text.
    let ok = run(&connected(20_000, ready()));
    for (_, led) in ok.all() {
        assert_eq!(led.accessible_name(), led.name());
    }
    let nf = run(&empty());
    assert_eq!(nf.pc_bridge.accessible_name(), "Bridge – not found");
}

/// Review patch: a connected session without INFO (never produced by the
/// worker) is "opening" on an open port, never "no port".
#[test]
fn g14_connected_without_info_is_not_no_port() {
    let mut s = connected(20_000, ready());
    s.info = None;
    let l = run(&s);
    assert_eq!(
        sig(&l.bridge_controller),
        want(LedState::Unknown, "Controller", Reason::Opening)
    );
    assert_eq!(
        l.bridge_controller.tooltip,
        vec!["Asking the controller…".to_owned()]
    );
}

/// AC 12 (review decision 2026-11-27): a reason change alone writes a line:
/// the worker dying while the LEDs are already unknown, Stale ↔ NoAnswer,
/// the NAK latch pairs.
#[test]
fn g14_reason_changes_are_logged() {
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let mut t = LedTracker::new(logger.handle());
    let unc = run(&opening(100_000));
    t.observe(&unc);
    t.observe(&leds::evaluate(
        Some(&opening(100_000)),
        0,
        WorkerHealth::Ended,
    ));
    let mut nak = connected(20_000, status(false, true, EventCode::None));
    t.observe(&run(&nak));
    nak.last_nak = Some((NakReason::NoLink, NOW));
    t.observe(&run(&nak));
    assert!(logger.stop());
    let texts = led_texts(&sink.lines());
    let reasons: Vec<&str> = texts
        .iter()
        .map(|t| {
            t.split(" reason=")
                .nth(1)
                .unwrap()
                .split(' ')
                .next()
                .unwrap()
        })
        .collect();
    // 3 (first) + 3 (worker_ended, all three unknown already) + 3 (to the
    // connected link-down state) + 1 (NakNoLink, same state and name).
    assert_eq!(texts.len(), 10, "{texts:#?}");
    assert_eq!(&reasons[3..6], ["worker_ended"; 3]);
    assert_eq!(reasons[9], "nak_no_link");
}
