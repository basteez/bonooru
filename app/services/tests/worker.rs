//! C.23 (fake clock): the serial worker's rules (Story 4.4 AC 4–8), each
//! number of the AC → measurement table measured on `FakeBackend` with
//! `WorkerCore` stepped 1 ms at a time. No real port, no sleeps.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use ocb_core::device::{Class, Rules};
use ocb_core::link::{LinkEvent, Phase};
use ocb_core::protocol::consts::{BTN_A, MSG_HELLO, MSG_SET_STATE, MSG_STOP, NAK_VERSION};
use ocb_core::protocol::{Mode, NEUTRAL, NakReason, RxSide, State, decode_frame};
use ocb_services::log::{LogHandle, Logger, MemorySink};
use ocb_services::port::OpenError;
use ocb_services::sim::{FakeBackend, FakePort, SimRig};
use ocb_services::worker::{
    Clock as _, FakeClock, OPEN_RETRY_US, PROBE_WINDOW_US, SCAN_PERIOD_US, WorkerCommand,
    WorkerEvent,
};
use ocb_sim::ControllerConfig;

const S: &str = "53032847189C2E9C";
const CU: &str = "/dev/cu.usbmodem101";

fn rig(ports: Vec<FakePort>, rules: Rules) -> SimRig {
    let mut b = FakeBackend::new(FakeClock::at(0));
    for p in ports {
        b = b.with(p);
    }
    SimRig::new(b, rules, LogHandle::disabled())
}

fn diffs(v: &[u64]) -> Vec<u64> {
    v.windows(2).map(|w| w[1] - w[0]).collect()
}

fn connected(r: &SimRig) -> Vec<u64> {
    r.find(|e| matches!(e, WorkerEvent::Link(LinkEvent::Connected(_))))
        .iter()
        .map(|e| e.at_us)
        .collect()
}

#[test]
fn c23_rescan_period() {
    assert_eq!(SCAN_PERIOD_US, 1_000_000);
    // Idle: nothing listed.
    let mut r = rig(vec![], Rules::LINUX);
    r.run_until(5_500_000);
    let calls = r.backend.state().list_calls.clone();
    assert_eq!(calls.len(), 6);
    assert!(diffs(&calls).iter().all(|&d| d == 1_000_000), "{calls:?}");
    // Connected.
    let mut r = rig(
        vec![FakePort::bridge(CU, S, Some("Bonooru Bridge"))],
        Rules::MACOS,
    );
    r.run_until(5_500_000);
    assert_eq!(connected(&r).len(), 1);
    let calls = r.backend.state().list_calls.clone();
    assert_eq!(calls.len(), 6);
    assert!(diffs(&calls).iter().all(|&d| d == 1_000_000), "{calls:?}");
    let snap = r.snapshot.clone().unwrap();
    assert_eq!(snap.scans, 6);
    let starts: Vec<u64> = snap.recent_scans.iter().map(|s| s.0).collect();
    assert_eq!(starts, calls, "scan timing in the snapshot");
}

#[test]
fn c23_open_sequence() {
    // A bridge alone (nothing answers): HELLO every 100 ms after a lone 00.
    let mut r = rig(
        vec![FakePort::bridge(CU, S, Some("Bonooru Bridge")).silent()],
        Rules::MACOS,
    );
    r.run_until(1_050_000);
    let st = r.backend.state();
    let ops: Vec<&str> = st.ops.iter().take(4).map(|o| o.2).collect();
    assert_eq!(ops, ["open", "dtr_on", "clear", "write"]);
    // The first write is the lone 00 and the first HELLO (one flush), then
    // one HELLO per write, 100 ms apart.
    let first = &st.writes[0].2;
    assert_eq!(first[0], 0, "a lone 00 first");
    let mut times = vec![st.writes[0].0];
    let mut frames = vec![first[1..].to_vec()];
    for w in &st.writes[1..] {
        times.push(w.0);
        frames.push(w.2.clone());
    }
    for f in &frames {
        let (&z, enc) = f.split_last().unwrap();
        assert_eq!(z, 0);
        assert_eq!(decode_frame(RxSide::Controller, enc).unwrap().ty, MSG_HELLO);
    }
    assert_eq!(frames.len(), 11);
    assert!(diffs(&times).iter().all(|&d| d == 100_000), "{times:?}");
    drop(st);
    let snap = r.snapshot.clone().unwrap();
    assert_eq!(snap.counters.hello_sent, 11);
    assert_eq!(snap.counters.hello_dropped, 0);
    assert_eq!(snap.phase, Phase::Opening);
    assert_eq!(snap.port_writes, vec![(CU.to_owned(), 12)]);
    // With a controller: INFO, connected, the snapshot carries it.
    let mut r = rig(
        vec![FakePort::bridge(CU, S, Some("Bonooru Bridge"))],
        Rules::MACOS,
    );
    r.run_until(300_000);
    assert_eq!(connected(&r).len(), 1);
    let snap = r.snapshot.clone().unwrap();
    assert_eq!(snap.phase, Phase::Connected);
    assert!(snap.info.is_some());
    let chosen = snap.chosen.clone().unwrap();
    assert_eq!((chosen.name.as_str(), chosen.open), (CU, true));
    assert_eq!(chosen.serial.as_deref(), Some(S));
    let (_, at) = snap.status.unwrap();
    assert!(at <= snap.now_us);
}

#[test]
fn c23_probe_round_robin() {
    assert_eq!(PROBE_WINDOW_US, 2_000_000);
    // Windows: three unconfirmed ports, only COM10 has a controller.
    let mut r = rig(
        vec![
            FakePort::bridge("COM3", "A", None).silent(),
            FakePort::bridge("COM10", "C", None),
            FakePort::bridge("COM9", "B", None).silent(),
        ],
        Rules::WINDOWS,
    );
    r.run_until(10_000_000);
    let st = r.backend.state();
    let o3 = st.opens_of("COM3");
    let o9 = st.opens_of("COM9");
    let o10 = st.opens_of("COM10");
    assert_eq!((o3.len(), o9.len(), o10.len()), (1, 1, 1), "{:?}", st.opens);
    assert_eq!(o9[0] - o3[0], 2_000_000);
    assert_eq!(o10[0] - o9[0], 2_000_000);
    assert!(st.writes_of("COM3").iter().all(|&t| t < o9[0]));
    assert!(st.writes_of("COM9").iter().all(|&t| t < o10[0]));
    drop(st);
    let c = connected(&r);
    assert_eq!(c.len(), 1);
    let snap = r.snapshot.clone().unwrap();
    assert_eq!(snap.chosen.clone().unwrap().name, "COM10");
    assert_eq!(
        r.find(|e| matches!(e, WorkerEvent::PortClosed { .. }))
            .len(),
        2
    );
    // A single unconfirmed candidate stays open.
    let mut r = rig(
        vec![FakePort::bridge("COM3", "A", None).silent()],
        Rules::WINDOWS,
    );
    r.run_until(10_000_000);
    assert_eq!(r.backend.state().opens_of("COM3").len(), 1);
}

#[test]
fn c23_ours_before_unconfirmed() {
    let mut r = rig(
        vec![
            FakePort::bridge("/dev/ttyACM0", "A", None),
            FakePort::bridge("/dev/ttyACM1", "B", Some("Bonooru Bridge")),
        ],
        Rules::LINUX,
    );
    r.run_until(1_000_000);
    let st = r.backend.state();
    assert_eq!(st.opens.len(), 1);
    assert_eq!(st.opens[0].1, "/dev/ttyACM1");
}

#[test]
fn c23_foreign_never_touched() {
    let mut r = rig(
        vec![
            FakePort::bridge("/dev/ttyACM0", "F", Some("2wiCC COM"))
                .present(vec![(0, Some(10_000_000)), (20_000_000, None)]),
            FakePort::usb("/dev/ttyUSB0", 0x0403, 0x6001, Some("X"), Some("FT232R")),
            FakePort::plain("/dev/ttyS0"),
            FakePort::bridge("/dev/ttyACM1", "B", Some("RP2 Boot")).silent(),
        ],
        Rules::LINUX,
    );
    r.run_until(30_000_000);
    let st = r.backend.state();
    assert!(st.opens.is_empty(), "{:?}", st.opens);
    assert!(st.dtr.is_empty() && st.writes.is_empty());
    drop(st);
    let foreign =
        r.find(|e| matches!(e, WorkerEvent::ForeignBoard { port, .. } if port == "/dev/ttyACM0"));
    assert_eq!(foreign.len(), 2, "one per appearance");
    assert_eq!(foreign[0].at_us, 0);
    assert_eq!(foreign[1].at_us, 20_000_000);
    let snap = r.snapshot.clone().unwrap();
    assert!(
        snap.ports
            .iter()
            .any(|p| matches!(&p.class, Class::Foreign { product } if product == "2wiCC COM"))
    );
    assert!(snap.chosen.is_none());
}

#[test]
fn c23_open_retry() {
    assert_eq!(OPEN_RETRY_US, 250_000);
    let mut r = rig(
        vec![
            FakePort::bridge(CU, S, Some("Bonooru Bridge")).fail_opens(vec![
                OpenError::Gone,
                OpenError::Other("x".to_owned()),
                OpenError::Gone,
            ]),
        ],
        Rules::MACOS,
    );
    r.run_until(2_000_000);
    let opens = r.backend.state().opens_of(CU);
    assert_eq!(opens, [0, 250_000, 500_000, 750_000]);
    assert_eq!(
        r.find(|e| matches!(e, WorkerEvent::PortOpened { .. }))
            .len(),
        1
    );
    assert_eq!(connected(&r).len(), 1);
}

#[test]
fn c23_unplug_detected() {
    // Read error at t: lost in the same pass.
    let mut r = rig(
        vec![FakePort::bridge(CU, S, Some("Bonooru Bridge")).present(vec![(0, Some(3_000_000))])],
        Rules::MACOS,
    );
    r.run_until(5_000_000);
    let lost = r.find(|e| matches!(e, WorkerEvent::PortLost { .. }));
    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0].at_us, 3_000_000);
    let WorkerEvent::PortLost { last_rx_us, .. } = &lost[0].event else {
        unreachable!()
    };
    let last = last_rx_us.unwrap();
    assert!(last < 3_000_000 && 3_000_000 - last < 100_000, "{last}");
    assert!(r.backend.state().closes.iter().any(|c| c.0 == 3_000_000));
    // The handle is dropped at once (a lost port takes no DTR call).
    assert_eq!(r.backend.state().closes.len(), 1);
    // Silent port: it only vanishes from the list; lost at the next scan.
    let mut r = rig(
        vec![
            FakePort::bridge(CU, S, Some("Bonooru Bridge"))
                .present(vec![(0, Some(3_000_500))])
                .silent_unplug(),
        ],
        Rules::MACOS,
    );
    r.run_until(5_000_000);
    let lost = r.find(|e| matches!(e, WorkerEvent::PortLost { .. }));
    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0].at_us, 4_000_000);
    assert!(lost[0].at_us - 3_000_500 <= 1_000_000);
}

#[test]
fn c23_rename_same_serial() {
    let mut r = rig(
        vec![
            FakePort::bridge(CU, S, Some("Bonooru Bridge")).present(vec![(0, Some(3_000_000))]),
            FakePort::bridge("/dev/cu.usbmodem1101", S, Some("Bonooru Bridge"))
                .present(vec![(3_500_000, None)]),
        ],
        Rules::MACOS,
    );
    r.run_until(6_000_000);
    let renamed = r.find(|e| matches!(e, WorkerEvent::PortRenamed { .. }));
    assert_eq!(renamed.len(), 1);
    assert_eq!(
        renamed[0].event,
        WorkerEvent::PortRenamed {
            from: CU.to_owned(),
            to: "/dev/cu.usbmodem1101".to_owned()
        }
    );
    assert_eq!(renamed[0].at_us, 4_000_000, "at the scan that lists it");
    let c = connected(&r);
    assert_eq!(c.len(), 2);
    assert!(c[1] - 4_000_000 <= 1_500_000);
    assert_eq!(
        r.snapshot.clone().unwrap().chosen.clone().unwrap().name,
        "/dev/cu.usbmodem1101"
    );
}

#[test]
fn c23_last_serial_first() {
    // Two bridges; the one connected before comes back first after both
    // vanish and reappear.
    let mut r = rig(
        vec![
            FakePort::bridge("/dev/ttyACM1", "B", Some("Bonooru Bridge"))
                .present(vec![(0, Some(2_000_000)), (3_000_000, None)]),
            FakePort::bridge("/dev/ttyACM0", "A", Some("Bonooru Bridge"))
                .present(vec![(1_500_000, Some(2_000_000)), (3_000_000, None)]),
        ],
        Rules::LINUX,
    );
    r.run_until(4_000_000);
    let st = r.backend.state();
    let names: Vec<&str> = st.opens.iter().map(|o| o.1.as_str()).collect();
    assert_eq!(names, ["/dev/ttyACM1", "/dev/ttyACM1"], "{:?}", st.opens);
}

#[test]
fn c23_permission_denied() {
    let mut r = rig(
        vec![
            FakePort::bridge("/dev/ttyACM0", S, Some("Bonooru Bridge"))
                .always_fail(OpenError::PermissionDenied)
                .group("uucp")
                .present(vec![(0, Some(10_000_000)), (12_000_000, None)]),
        ],
        Rules::LINUX,
    );
    r.run_until(10_000_000);
    let perm = r.find(|e| matches!(e, WorkerEvent::PermissionDenied { .. }));
    assert_eq!(perm.len(), 1, "one for 10 scans");
    assert_eq!(perm[0].at_us, 0, "in the scan that lists the port");
    assert_eq!(
        perm[0].event,
        WorkerEvent::PermissionDenied {
            port: "/dev/ttyACM0".to_owned(),
            group: "uucp".to_owned(),
            message: "Bonooru can't open /dev/ttyACM0: your user is not in the uucp group. \
                      Run: sudo usermod -aG uucp $USER — then log out and back in."
                .to_owned()
        }
    );
    // Retried only at scans, never in a loop.
    let opens = r.backend.state().opens_of("/dev/ttyACM0");
    assert_eq!(opens.len(), 10);
    assert!(opens.iter().all(|t| t % 1_000_000 == 0), "{opens:?}");
    r.run_until(13_000_000);
    let perm = r.find(|e| matches!(e, WorkerEvent::PermissionDenied { .. }));
    assert_eq!(perm.len(), 2, "vanish + reappear: a second one");
    assert_eq!(perm[1].at_us, 12_000_000);
    // Fallback group.
    let mut r = rig(
        vec![FakePort::bridge("/dev/ttyACM0", S, None).always_fail(OpenError::PermissionDenied)],
        Rules::LINUX,
    );
    r.run_until(100_000);
    let perm =
        r.find(|e| matches!(e, WorkerEvent::PermissionDenied { group, .. } if group == "dialout"));
    assert_eq!(perm.len(), 1);
}

#[test]
fn c23_port_busy_deduplicated() {
    let mut r = rig(
        vec![FakePort::bridge(CU, S, Some("Bonooru Bridge")).always_fail(OpenError::Busy)],
        Rules::MACOS,
    );
    r.run_until(5_000_000);
    assert_eq!(
        r.find(|e| matches!(e, WorkerEvent::PortBusy { .. })).len(),
        1
    );
    assert_eq!(
        r.backend.state().opens_of(CU).len(),
        20,
        "retried every 250 ms"
    );
}

#[test]
fn c23_shutdown_closes() {
    let mut r = rig(
        vec![FakePort::bridge(CU, S, Some("Bonooru Bridge"))],
        Rules::MACOS,
    );
    r.run_until(500_000);
    r.ends.commands.send(WorkerCommand::Shutdown).unwrap();
    r.step();
    assert!(r.core.is_shut_down());
    assert_eq!(
        r.find(|e| matches!(e, WorkerEvent::PortClosed { .. }))
            .len(),
        1
    );
    let st = r.backend.state();
    assert_eq!(st.closes.len(), 1);
    assert!(!st.dtr_of(CU));
    let writes = st.writes.len();
    drop(st);
    r.run_until(2_000_000);
    assert_eq!(
        r.backend.state().writes.len(),
        writes,
        "nothing after shutdown"
    );
}

#[test]
fn c23_snapshot_channel_holds_one() {
    let mut b = FakeBackend::new(FakeClock::at(0));
    b = b.with(FakePort::bridge(CU, S, Some("Bonooru Bridge")));
    let clock = b.state().clock.clone();
    let (mut core, ends) = ocb_services::worker::WorkerCore::new(
        b,
        clock.clone(),
        Rules::MACOS,
        LogHandle::disabled(),
        Box::new(|| {}),
    );
    for _ in 0..3000 {
        core.step();
        clock.advance(1_000);
        assert!(ends.snapshots.len() <= 1);
    }
    let latest = ends.snapshots.try_recv().unwrap();
    // STATUS arrives every 50 ms, each one publishes.
    assert!(latest.now_us >= 2_940_000, "the newest: {}", latest.now_us);
}

#[test]
fn c23_everything_logged() {
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), true, None);
    let cfg = ControllerConfig {
        fw_ver: [9, 9, 9],
        ..ControllerConfig::default()
    };
    let b = FakeBackend::new(FakeClock::at(0))
        .with(
            FakePort::bridge(CU, S, Some("Bonooru Bridge"))
                .controller(cfg)
                .fail_opens(vec![OpenError::Busy, OpenError::PermissionDenied])
                .present(vec![(0, Some(6_000_000))])
                .hook(2_000_000, Box::new(|c| c.inject_event(0xEE)))
                .hook(2_100_000, Box::new(|c| c.inject_event(0x01)))
                .hook(
                    2_200_000,
                    Box::new(|c| c.inject_nak(MSG_SET_STATE, NakReason::NotReady)),
                )
                .hook(4_000_000, Box::new(|c| c.inject_raw_nak(0x7E, 0, 0)))
                .hook(4_500_000, Box::new(|c| c.inject_raw_nak(NAK_VERSION, 0, 0))),
        )
        .with(FakePort::bridge(
            "/dev/cu.usbmodem201",
            "F",
            Some("2wiCC COM"),
        ))
        .with(
            FakePort::bridge("/dev/cu.usbmodem1101", S, Some("Bonooru Bridge"))
                .present(vec![(7_000_000, None)]),
        );
    let mut r = SimRig::new(b, Rules::MACOS, logger.handle());
    r.run_until(2_300_000);
    // SET_STATE refused with NOT_READY (the command of a later story,
    // driven through the session here).
    r.core.session_mut().unwrap().set_state(NEUTRAL).unwrap();
    r.run_until(3_000_000);
    // A transport NAK: the next frame is damaged on the wire.
    {
        let mut st = r.backend.state();
        st.ports[0].corrupt_next_write = true;
    }
    r.core.session_mut().unwrap().set_state(NEUTRAL).unwrap();
    r.run_until(9_000_000);
    r.ends.commands.send(WorkerCommand::Shutdown).unwrap();
    r.step();
    drop(r);
    logger.stop();
    let lines = sink.lines();
    let kinds: Vec<&str> = lines.iter().map(|l| l.split(' ').nth(2).unwrap()).collect();
    let has = |k: &str, text: &str| {
        lines
            .iter()
            .any(|l| l.split(' ').nth(2) == Some(k) && l.contains(text))
    };
    let all = lines.join("\n");
    assert_eq!(kinds[0], "start", "{all}");
    assert!(has("port_listed", "class=ours"), "{all}");
    assert!(has("port_busy", CU), "{all}");
    assert!(has("permission_denied", "group=dialout"), "{all}");
    assert!(has("foreign", "2wiCC COM"), "{all}");
    assert!(has("port_opened", CU), "{all}");
    assert!(has("version", "connected"), "{all}");
    assert!(has("version", "recommended update fw=9.9.9"), "{all}");
    assert!(has("event", "code=0xee"), "{all}");
    assert!(has("event", "code=0x01"), "{all}");
    assert!(has("nak", "Refused reason=NotReady ty=0x02"), "{all}");
    assert!(has("nak", "transport"), "{all}");
    assert!(has("nak", "unknown reason=0x7e"), "{all}");
    assert!(has("reboot", "controller rebooted"), "{all}");
    assert!(has("port_lost", CU), "{all}");
    assert!(has("port_vanished", CU), "{all}");
    assert!(has("port_renamed", "to=/dev/cu.usbmodem1101"), "{all}");
    assert!(has("port_opened", "/dev/cu.usbmodem1101"), "{all}");
    assert!(has("port_closed", "/dev/cu.usbmodem1101"), "{all}");
    // Two INFOs (the reboot reconnects) + one after the rename.
    assert!(
        kinds.iter().filter(|k| **k == "version").count() >= 4,
        "{all}"
    );
}

#[test]
fn c23_log_unavailable_once() {
    let logger = Logger::spawn(Box::new(ocb_services::log::FailingSink), false, None);
    let b = FakeBackend::new(FakeClock::at(0));
    let mut r = SimRig::new(b, Rules::LINUX, logger.handle());
    logger.handle().log("event", "first write fails");
    // The log thread is real: give it real time to fail.
    let start = std::time::Instant::now();
    while r
        .find(|e| matches!(e, WorkerEvent::LogUnavailable { .. }))
        .is_empty()
        && start.elapsed().as_secs() < 5
    {
        r.step();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    r.run_until(r.clock.now_us() + 3_000_000);
    let un = r.find(|e| matches!(e, WorkerEvent::LogUnavailable { .. }));
    assert_eq!(un.len(), 1);
    drop(r);
    logger.stop();
}

#[test]
fn c23_no_scan_events() {
    // One `ScanDone` per second forever would crowd the bounded event
    // queue and the UI history: scans live in the snapshot only.
    let mut r = rig(vec![], Rules::LINUX);
    r.run_until(30_000_000);
    assert!(r.events.is_empty(), "{:?}", r.events.first());
    let snap = r.snapshot.clone().unwrap();
    assert_eq!(snap.scans, 30);
    assert_eq!(snap.recent_scans.len(), 30);
}

#[test]
fn c23_lost_port_retried_after_backoff() {
    // A read glitch on a port that stays listed: reopened 250 ms later,
    // not in the next pass.
    let mut r = rig(
        vec![FakePort::bridge(CU, S, Some("Bonooru Bridge"))],
        Rules::MACOS,
    );
    r.run_until(1_000_000);
    r.backend.state().ports[0].fail_next_read = true;
    r.run_until(2_000_000);
    let lost = r.find(|e| matches!(e, WorkerEvent::PortLost { .. }));
    assert_eq!(lost.len(), 1);
    let opens = r.backend.state().opens_of(CU);
    assert_eq!(opens.len(), 2, "{opens:?}");
    assert_eq!(opens[1] - lost[0].at_us, OPEN_RETRY_US);
}

#[test]
fn c23_round_robin_keeps_a_bridge_that_answered() {
    // Windows, two unconfirmed ports; the first answers with another
    // protocol version (RequiredUpdate): it stays open.
    let cfg = ControllerConfig {
        proto_ver: 2,
        ..ControllerConfig::default()
    };
    let mut r = rig(
        vec![
            FakePort::bridge("COM3", "A", None).controller(cfg),
            FakePort::bridge("COM9", "B", None).silent(),
        ],
        Rules::WINDOWS,
    );
    r.run_until(10_000_000);
    let st = r.backend.state();
    assert_eq!(st.opens_of("COM3").len(), 1, "{:?}", st.opens);
    assert!(st.opens_of("COM9").is_empty());
    drop(st);
    let req = r.find(|e| matches!(e, WorkerEvent::Link(LinkEvent::RequiredUpdate { .. })));
    assert_eq!(req.len(), 1);
    assert_eq!(r.snapshot.clone().unwrap().phase, Phase::Incompatible);
}

#[test]
fn c23_waiting_candidate_does_not_block_others() {
    // Two bridges; the first in rank is held by another program.
    let mut r = rig(
        vec![
            FakePort::bridge("/dev/ttyACM0", "A", Some("Bonooru Bridge"))
                .always_fail(OpenError::Busy),
            FakePort::bridge("/dev/ttyACM1", "B", Some("Bonooru Bridge")),
        ],
        Rules::LINUX,
    );
    r.run_until(1_000_000);
    assert_eq!(connected(&r).len(), 1);
    assert_eq!(
        r.snapshot.clone().unwrap().chosen.clone().unwrap().name,
        "/dev/ttyACM1"
    );
}

#[test]
fn c23_flags_reset_on_a_new_state() {
    // Busy, then opened, then lost by a glitch, then busy again: a second
    // PortBusy (the port never left the listing).
    let mut r = rig(
        vec![FakePort::bridge(CU, S, Some("Bonooru Bridge")).fail_opens(vec![OpenError::Busy])],
        Rules::MACOS,
    );
    r.run_until(1_000_000);
    assert_eq!(connected(&r).len(), 1);
    {
        let mut st = r.backend.state();
        st.ports[0].fail_next_read = true;
        st.ports[0].open_failures.push_back(OpenError::Busy);
    }
    r.run_until(3_000_000);
    assert_eq!(
        r.find(|e| matches!(e, WorkerEvent::PortBusy { .. })).len(),
        2
    );
    // Busy then permission then busy on one listing: each change reported.
    let mut r = rig(
        vec![
            FakePort::bridge(CU, S, Some("Bonooru Bridge")).fail_opens(vec![
                OpenError::Busy,
                OpenError::PermissionDenied,
                OpenError::Busy,
            ]),
        ],
        Rules::MACOS,
    );
    r.run_until(3_000_000);
    assert_eq!(
        r.find(|e| matches!(e, WorkerEvent::PortBusy { .. })).len(),
        2
    );
    assert_eq!(
        r.find(|e| matches!(e, WorkerEvent::PermissionDenied { .. }))
            .len(),
        1
    );
}

#[test]
fn c23_port_that_never_drains_keeps_the_worker_on_time() {
    // A bridge whose writes never progress (no controller link, the
    // device does not read): scans stay exactly 1 s apart, the HELLO
    // cadence goes on, the own queue fills and drops are counted.
    let mut p = FakePort::bridge(CU, S, Some("Bonooru Bridge")).silent();
    p.stall_writes = true;
    let mut r = rig(vec![p], Rules::MACOS);
    r.run_until(10_500_000);
    let calls = r.backend.state().list_calls.clone();
    assert_eq!(calls.len(), 11);
    assert!(diffs(&calls).iter().all(|&d| d == 1_000_000), "{calls:?}");
    let snap = r.snapshot.clone().unwrap();
    assert_eq!(snap.counters.hello_sent, 105);
    // About 7 B per HELLO: the own queue never fills in 10 s; what does
    // not move for 1 s is abandoned, once per second.
    assert!(snap.counters.dropped_chunks >= 9, "{:?}", snap.counters);
    // With the OS reporting a full queue, every HELLO is dropped and
    // counted, and the cadence is still the session's.
    let mut p = FakePort::bridge(CU, S, Some("Bonooru Bridge")).silent();
    p.queued = 8192;
    let mut r = rig(vec![p], Rules::MACOS);
    r.run_until(10_050_000);
    let c = r.snapshot.clone().unwrap().counters;
    assert_eq!((c.hello_sent, c.hello_dropped), (101, 101));
}

// ------------------------------------------------------------ C.26

/// The snapshot after the step that emitted the first event matching `f`
/// (the rig is stepped until then), with the event's time.
fn step_to(
    r: &mut SimRig,
    limit_us: u64,
    f: impl Fn(&WorkerEvent) -> bool,
) -> (u64, std::sync::Arc<ocb_services::worker::LinkSnapshot>) {
    let seen = r.events.len();
    while r.clock.now_us() < limit_us {
        r.step();
        if let Some(e) = r.events[seen..].iter().find(|e| f(&e.event)) {
            return (e.at_us, r.snapshot.clone().unwrap());
        }
    }
    panic!("no matching event before {limit_us} µs");
}

/// C.26 (Story 4.5 AC 4, 6, 7): `session_since_us`, `connected_us`,
/// `required_update` and `last_nak` are set and cleared at the right step.
#[test]
fn c26_snapshot_link_timing_fields() {
    let b = FakeBackend::new(FakeClock::at(0)).with(
        FakePort::bridge(CU, S, Some("Bonooru Bridge"))
            .present(vec![(0, Some(3_000_000))])
            .hook(
                400_000,
                Box::new(|c| c.inject_nak(MSG_SET_STATE, NakReason::NotReady)),
            )
            .hook(
                700_000,
                Box::new(|c| c.inject_nak(MSG_SET_STATE, NakReason::NoLink)),
            )
            .hook(1_020_000, Box::new(|c| c.inject_raw_nak(NAK_VERSION, 0, 0))),
    );
    let mut r = SimRig::new(b, Rules::MACOS, LogHandle::disabled());
    // Open at 0: the session's grace starts there, nothing else yet.
    let (t_open, snap) = step_to(&mut r, 100_000, |e| {
        matches!(e, WorkerEvent::PortOpened { .. })
    });
    assert_eq!(t_open, 0);
    assert_eq!(snap.session_since_us, Some(0));
    assert_eq!(
        (snap.connected_us, snap.required_update, snap.last_nak),
        (None, None, None)
    );
    // INFO: connected at its time.
    let (t_info, snap) = step_to(&mut r, 300_000, |e| {
        matches!(e, WorkerEvent::Link(LinkEvent::Connected(_)))
    });
    assert_eq!(snap.connected_us, Some(t_info));
    assert_eq!(snap.session_since_us, Some(0), "INFO does not move it");
    assert_eq!(snap.required_update, None);
    // SET_STATE refused with NOT_READY, then NO_LINK: the latest is kept.
    r.run_until(450_000);
    r.core.session_mut().unwrap().set_state(NEUTRAL).unwrap();
    let (t_nr, snap) = step_to(&mut r, 600_000, |e| {
        matches!(
            e,
            WorkerEvent::Link(LinkEvent::Refused {
                reason: NakReason::NotReady,
                ..
            })
        )
    });
    assert_eq!(snap.last_nak, Some((NakReason::NotReady, t_nr)));
    r.run_until(750_000);
    r.core.session_mut().unwrap().set_state(NEUTRAL).unwrap();
    let (t_nl, snap) = step_to(&mut r, 900_000, |e| {
        matches!(
            e,
            WorkerEvent::Link(LinkEvent::Refused {
                reason: NakReason::NoLink,
                ..
            })
        )
    });
    assert_eq!(snap.last_nak, Some((NakReason::NoLink, t_nl)));
    assert!(snap.status.is_some());
    // Mid-session NAK VERSION: a new grace from the re-open, nothing of the
    // previous boot kept.
    let (t_reboot, snap) = step_to(&mut r, 1_100_000, |e| {
        matches!(e, WorkerEvent::Link(LinkEvent::ControllerRebooted))
    });
    assert_eq!(snap.session_since_us, Some(t_reboot));
    assert_eq!(snap.phase, Phase::Opening);
    assert_eq!(
        (snap.status, snap.connected_us, snap.last_nak),
        (None, None, None),
        "reboot clears the previous boot's data"
    );
    let (t_info2, snap) = step_to(&mut r, 1_400_000, |e| {
        matches!(e, WorkerEvent::Link(LinkEvent::Connected(_)))
    });
    assert_eq!(snap.connected_us, Some(t_info2));
    assert_eq!(snap.session_since_us, Some(t_reboot));
    // Lost (unplugged at 3 s): everything cleared.
    let (_, snap) = step_to(&mut r, 3_100_000, |e| {
        matches!(e, WorkerEvent::PortLost { .. })
    });
    assert_eq!(
        (
            snap.session_since_us,
            snap.connected_us,
            snap.required_update,
            snap.last_nak,
            snap.status
        ),
        (None, None, None, None, None)
    );
}

/// C.26: a protocol mismatch keeps the INFO for the LEDs; a loss clears it.
#[test]
fn c26_required_update_kept_until_lost() {
    let cfg = ControllerConfig {
        proto_ver: 2,
        fw_ver: [0, 2, 0],
        bridge_ver: [0, 2, 1],
        ..ControllerConfig::default()
    };
    let b = FakeBackend::new(FakeClock::at(0)).with(
        FakePort::bridge(CU, S, Some("Bonooru Bridge"))
            .controller(cfg)
            .present(vec![(0, Some(1_000_000))]),
    );
    let mut r = SimRig::new(b, Rules::MACOS, LogHandle::disabled());
    let (t, snap) = step_to(&mut r, 300_000, |e| {
        matches!(e, WorkerEvent::Link(LinkEvent::RequiredUpdate { .. }))
    });
    assert_eq!(snap.phase, Phase::Incompatible);
    assert_eq!(snap.info, None, "the session keeps no INFO in Incompatible");
    let info = snap.required_update.unwrap();
    assert_eq!(
        (info.proto_ver, info.fw_ver, info.bridge_ver),
        (2, [0, 2, 0], [0, 2, 1])
    );
    assert_eq!(snap.connected_us, Some(t));
    let (_, snap) = step_to(&mut r, 1_100_000, |e| {
        matches!(e, WorkerEvent::PortLost { .. })
    });
    assert_eq!((snap.required_update, snap.connected_us), (None, None));
}

/// C.26: `FakePort::mute_at` cuts the controller off (no STATUS, port
/// still listed and open); `unmute_at` brings it back.
#[test]
fn c26_mute_cuts_the_controller_off() {
    let b = FakeBackend::new(FakeClock::at(0)).with(
        FakePort::bridge(CU, S, Some("Bonooru Bridge"))
            .mute_at(500_000)
            .unmute_at(1_500_000),
    );
    let mut r = SimRig::new(b, Rules::MACOS, LogHandle::disabled());
    r.run_until(1_400_000);
    let snap = r.snapshot.clone().unwrap();
    let (_, rx) = snap.status.unwrap();
    assert!((450_000..=500_000).contains(&rx), "last STATUS at {rx}");
    assert_eq!(snap.phase, Phase::Connected);
    assert!(snap.chosen.as_ref().unwrap().open, "the port stays open");
    assert!(
        r.find(|e| matches!(e, WorkerEvent::PortLost { .. }))
            .is_empty()
    );
    r.run_until(1_700_000);
    let (_, rx) = r.snapshot.clone().unwrap().status.unwrap();
    assert!(rx >= 1_500_000, "STATUS again after the unmute: {rx}");
}

/// C.26 (review): the STATUS-path reboot. A STATUS of a freshly booted
/// controller after a non-fresh one: the core pushes `Status` then
/// `ControllerRebooted`, and the worker must end with no STATUS kept and
/// the grace restarted.
#[test]
fn c26_status_path_reboot_clears_the_status() {
    let b = FakeBackend::new(FakeClock::at(0)).with(
        FakePort::bridge(CU, S, Some("Bonooru Bridge"))
            // A non-fresh STATUS: an abort with USB_SUSPENDED.
            .hook(
                300_000,
                Box::new(|c| c.force_abort(Some(ocb_core::protocol::EventCode::UsbSuspended))),
            )
            // Power cycle: the next STATUS is a fresh one (abort 0, NONE).
            .hook(1_010_000, Box::new(|c| c.reboot())),
    );
    let mut r = SimRig::new(b, Rules::MACOS, LogHandle::disabled());
    r.run_until(1_000_000);
    let (_, rx) = r.snapshot.clone().unwrap().status.unwrap();
    assert!(rx > 300_000, "a non-fresh STATUS seen");
    let (t, snap) = step_to(&mut r, 1_200_000, |e| {
        matches!(e, WorkerEvent::Link(LinkEvent::ControllerRebooted))
    });
    // The rebooted controller's STATUS came in the same pass, just before
    // the reboot event (it is what revealed the reboot): it must not
    // survive as "fresh" data.
    assert_eq!(snap.status, None, "the reboot clears the STATUS");
    assert_eq!(snap.session_since_us, Some(t));
    assert_eq!(snap.phase, Phase::Opening);
    assert_eq!((snap.connected_us, snap.last_nak), (None, None));
}

/// Review patch: an unavailable log never hides "port in use" (or
/// permission): Busy stays the problem worth showing.
#[test]
fn c26_log_unavailable_keeps_busy() {
    let logger = Logger::spawn(Box::new(ocb_services::log::FailingSink), false, None);
    let b = FakeBackend::new(FakeClock::at(0))
        .with(FakePort::bridge(CU, S, Some("Bonooru Bridge")).always_fail(OpenError::Busy));
    let mut r = SimRig::new(b, Rules::MACOS, logger.handle());
    r.run_until(10_000);
    assert!(matches!(
        r.snapshot.clone().unwrap().last_error,
        Some(ocb_services::worker::LastError::Busy { .. })
    ));
    logger.handle().log("event", "first write fails");
    let start = std::time::Instant::now();
    while r
        .find(|e| matches!(e, WorkerEvent::LogUnavailable { .. }))
        .is_empty()
    {
        assert!(start.elapsed().as_secs() < 5, "log never failed");
        r.step();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    r.step();
    assert!(
        matches!(
            r.snapshot.clone().unwrap().last_error,
            Some(ocb_services::worker::LastError::Busy { .. })
        ),
        "{:?}",
        r.snapshot.clone().unwrap().last_error
    );
    drop(r);
    logger.stop();
}

/// Review patch: `unmute_at` without an open `mute_at` is a script error.
#[test]
#[should_panic(expected = "without an open mute_at")]
fn c26_unmute_without_mute_panics() {
    let _ = FakePort::bridge(CU, S, Some("Bonooru Bridge")).unmute_at(1_000);
}

#[test]
#[should_panic(expected = "without an open mute_at")]
fn c26_unmute_twice_panics() {
    let _ = FakePort::bridge(CU, S, Some("Bonooru Bridge"))
        .mute_at(1_000)
        .unmute_at(2_000)
        .unmute_at(3_000);
}

/// C.27 (Story 4.6 AC 7): after a protocol mismatch the worker writes no
/// further byte to that port while the session stays `Incompatible`, a
/// command attempted on the session is refused and writes nothing, and the
/// port is still read (the fake controller's STATUS keeps arriving).
#[test]
fn c27_nothing_written_when_incompatible() {
    use ocb_core::link::CommandError;
    let cfg = ControllerConfig {
        proto_ver: 2,
        ..ControllerConfig::default()
    };
    let b = FakeBackend::new(FakeClock::at(0))
        .with(FakePort::bridge(CU, S, Some("Bonooru Bridge")).controller(cfg));
    let mut r = SimRig::new(b, Rules::MACOS, LogHandle::disabled());
    let (t, _) = step_to(&mut r, 1_500_000, |e| {
        matches!(e, WorkerEvent::Link(LinkEvent::RequiredUpdate { .. }))
    });
    // One more pass: whatever the INFO pass flushed is counted.
    r.step();
    let writes = |r: &SimRig| {
        let snap = r.snapshot.clone().unwrap();
        snap.port_writes
            .iter()
            .find(|(n, _)| n == CU)
            .map(|(_, w)| *w)
            .unwrap()
    };
    let at_info = writes(&r);
    assert!(at_info > 0, "the HELLO was written");
    let raw_at_info = r.backend.state().writes_of(CU).len();
    let first_status = r.snapshot.clone().unwrap().status.map(|(_, rx)| rx);
    r.run_until(t + 10_000_000);
    let snap = r.snapshot.clone().unwrap();
    assert_eq!(snap.phase, Phase::Incompatible);
    assert_eq!(writes(&r), at_info, "no byte written in Incompatible");
    assert_eq!(
        r.backend.state().writes_of(CU).len(),
        raw_at_info,
        "the fake port saw no write either"
    );
    // The port is still read: STATUS keeps arriving every 50 ms.
    let (_, rx) = snap.status.unwrap();
    assert!(rx > t + 9_000_000, "STATUS still read: last at {rx}");
    assert_ne!(Some(rx), first_status);
    // A command on the incompatible session is refused and writes nothing.
    assert_eq!(
        r.core.session_mut().unwrap().set_state(NEUTRAL),
        Err(CommandError::NotConnected)
    );
    let now = r.clock.now_us();
    r.run_until(now + 1_000_000);
    assert_eq!(writes(&r), at_info, "the refused command wrote nothing");
    assert_eq!(r.backend.state().writes_of(CU).len(), raw_at_info);
}

// ------------------------------------------------------------ C.28

/// The STOP frames among the chunks written to `CU` (a chunk may carry
/// several frames, each ended by its `00`), with the chunk's time.
fn stop_frames(r: &SimRig) -> Vec<u64> {
    let st = r.backend.state();
    let mut out = Vec::new();
    for (t, name, bytes) in &st.writes {
        if name != CU {
            continue;
        }
        for enc in bytes.split(|&b| b == 0).filter(|f| !f.is_empty()) {
            if decode_frame(RxSide::Controller, enc).is_ok_and(|f| f.ty == MSG_STOP) {
                out.push(*t);
            }
        }
    }
    out
}

fn writes_to(r: &SimRig, name: &str) -> u64 {
    r.snapshot
        .clone()
        .unwrap()
        .port_writes
        .iter()
        .find(|(n, _)| n == name)
        .map_or(0, |(_, w)| *w)
}

/// The fake controller behind the first port.
fn with_ctrl<T>(r: &SimRig, f: impl FnOnce(&ocb_sim::FakeController) -> T) -> T {
    let st = r.backend.state();
    match &st.ports[0].device {
        ocb_services::sim::Device::Controller(c) => f(c),
        ocb_services::sim::Device::Silent => panic!("no controller"),
    }
}

/// The `stop` lines (their text after the kind).
fn stop_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|l| l.split(' ').nth(2) == Some("stop"))
        .map(|l| l.splitn(4, ' ').nth(3).unwrap_or("").to_owned())
        .collect()
}

/// `0x<hh>`: a controller mode this app does not know (`Mode::Unknown`).
fn unknown_mode(v: Option<&str>) -> bool {
    v.and_then(|v| v.strip_prefix("0x"))
        .is_some_and(|h| h.len() == 2 && h.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// A `stop` worker line against its pattern (Story 4.7 Dev Notes › Log
/// lines): the verb, then exactly these `key=value` fields in order.
fn stop_line_ok(text: &str) -> bool {
    let mut it = text.split(' ');
    let verb = it.next().unwrap_or("");
    let fields: Vec<(&str, &str)> = it.filter_map(|f| f.split_once('=')).collect();
    let keys: Vec<&str> = fields.iter().map(|f| f.0).collect();
    let get = |k: &str| fields.iter().find(|f| f.0 == k).map(|f| f.1);
    let non_empty = |k: &str| get(k).is_some_and(|v| !v.is_empty());
    let one_of = |k: &str, set: &[&str]| get(k).is_some_and(|v| set.contains(&v));
    let digits =
        |k: &str| get(k).is_some_and(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()));
    let n = text.split(' ').count() - 1;
    if fields.len() != n {
        return false;
    }
    match verb {
        "sent" | "pending" => keys == ["port"] && non_empty("port"),
        "skipped" => {
            keys == ["reason", "port"]
                && one_of("reason", &["no_port", "opening", "incompatible", "closed"])
                && non_empty("port")
        }
        "acked" => {
            keys == ["port", "after_us", "by"]
                && non_empty("port")
                && (digits("after_us") || get("after_us") == Some("-"))
                && one_of("by", &["command", "session"])
        }
        "after" => {
            keys == ["port", "mode", "abort_count"]
                && non_empty("port")
                && (one_of("mode", &["idle", "direct", "playing"]) || unknown_mode(get("mode")))
                && digits("abort_count")
        }
        _ => false,
    }
}

fn logged_rig(ports: Vec<FakePort>) -> (SimRig, Logger, MemorySink) {
    let sink = MemorySink::new();
    let logger = Logger::spawn(Box::new(sink.clone()), false, None);
    let mut b = FakeBackend::new(FakeClock::at(0));
    for p in ports {
        b = b.with(p);
    }
    let r = SimRig::new(b, Rules::MACOS, logger.handle());
    (r, logger, sink)
}

fn finish(r: SimRig, logger: Logger, sink: &MemorySink) -> Vec<String> {
    drop(r);
    logger.stop();
    stop_lines(&sink.lines())
}

/// C.28 (Story 4.7 AC 7): the STOP frame is queued to the port in the very
/// worker pass that takes the `Stop` command, at the same fake time.
#[test]
fn c28_stop_frame_same_pass() {
    let mut r = rig(
        vec![FakePort::bridge(CU, S, Some("Bonooru Bridge"))],
        Rules::MACOS,
    );
    step_to(&mut r, 1_000_000, |e| {
        matches!(e, WorkerEvent::Link(LinkEvent::Connected(_)))
    });
    r.run_until(400_000);
    assert_eq!(r.snapshot.clone().unwrap().phase, Phase::Connected);
    let before = writes_to(&r, CU);
    assert!(stop_frames(&r).is_empty());
    let t = r.clock.now_us();
    r.ends.commands.send(WorkerCommand::Stop).unwrap();
    r.step();
    assert_eq!(
        writes_to(&r, CU),
        before + 1,
        "one frame queued in the pass"
    );
    assert_eq!(
        stop_frames(&r),
        vec![t],
        "the STOP frame, same pass, same time"
    );
}

/// C.28 (AC 7): an input held through the session is released: the
/// controller goes IDLE and publishes NEUTRAL, the ACK brings the session
/// back to `Connected`, and the three worker lines are written.
#[test]
fn c28_stop_releases_inputs() {
    let (mut r, logger, sink) = logged_rig(vec![FakePort::bridge(CU, S, Some("Bonooru Bridge"))]);
    step_to(&mut r, 1_000_000, |e| {
        matches!(e, WorkerEvent::Link(LinkEvent::Connected(_)))
    });
    let held = State {
        buttons: BTN_A,
        ..NEUTRAL
    };
    r.core.session_mut().unwrap().set_state(held).unwrap();
    let t = r.clock.now_us();
    r.run_until(t + 20_000);
    assert_eq!(with_ctrl(&r, |c| c.mode()), Mode::Direct);
    assert_eq!(with_ctrl(&r, |c| c.published().buttons), BTN_A, "A held");
    let seen = r.events.len();
    let t0 = r.clock.now_us();
    r.ends.commands.send(WorkerCommand::Stop).unwrap();
    let mut steps = 0;
    while !r.events[seen..]
        .iter()
        .any(|e| e.event == WorkerEvent::Link(LinkEvent::Stopped))
    {
        assert!(steps < 5, "no ACK within 5 steps");
        r.step();
        steps += 1;
    }
    assert!(r.clock.now_us() - t0 <= 5_000);
    assert_eq!(with_ctrl(&r, |c| c.mode()), Mode::Idle);
    assert_eq!(
        with_ctrl(&r, |c| c.published()),
        NEUTRAL,
        "every input released"
    );
    assert_eq!(with_ctrl(&r, |c| c.log().stops), 1);
    assert_eq!(r.snapshot.clone().unwrap().phase, Phase::Connected);
    // The first STATUS after the ACK.
    let t = r.clock.now_us();
    r.run_until(t + 100_000);
    let lines = finish(r, logger, &sink);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[0], format!("sent port={CU}"));
    assert!(
        lines[1].starts_with(&format!("acked port={CU} after_us=")),
        "{lines:?}"
    );
    assert!(lines[1].ends_with(" by=command"), "{lines:?}");
    let after: u64 = lines[1]
        .split(' ')
        .find_map(|f| f.strip_prefix("after_us="))
        .unwrap()
        .parse()
        .unwrap();
    assert!(after <= 100_000, "H.34 pass value: {after}");
    assert!(
        lines[2].starts_with(&format!("after port={CU} mode=idle abort_count=")),
        "{lines:?}"
    );
    assert!(lines.iter().all(|l| stop_line_ok(l)), "{lines:?}");
}

/// C.28 (AC 7): while a STOP waits for its ACK, more commands send no new
/// frame (the pending one keeps its 50 ms resend) and each logs `pending`.
#[test]
fn c28_stop_while_stopping() {
    let (mut r, logger, sink) = logged_rig(vec![
        FakePort::bridge(CU, S, Some("Bonooru Bridge")).mute_at(500_000),
    ]);
    r.run_until(600_000);
    assert_eq!(r.snapshot.clone().unwrap().phase, Phase::Connected);
    let t0 = r.clock.now_us();
    for _ in 0..10 {
        r.ends.commands.send(WorkerCommand::Stop).unwrap();
        r.step();
    }
    assert!(r.clock.now_us() - t0 <= 10_000);
    let frames: Vec<u64> = stop_frames(&r)
        .into_iter()
        .filter(|&t| t < t0 + 10_000)
        .collect();
    assert_eq!(frames, vec![t0], "exactly one STOP frame in 10 ms");
    assert_eq!(r.snapshot.clone().unwrap().phase, Phase::Stopping);
    let lines = finish(r, logger, &sink);
    assert_eq!(lines[0], format!("sent port={CU}"));
    assert_eq!(
        lines
            .iter()
            .filter(|l| **l == format!("pending port={CU}"))
            .count(),
        9,
        "{lines:?}"
    );
    assert_eq!(lines.len(), 10, "{lines:?}");
}

/// C.28 (AC 7, 8; issue #45 context): with the controller cut off the
/// STOP is resent every 50 ms (20 frames a second, the bound this story
/// adds to the port's write rate); once it is back the next STOP is ACKed
/// and the `after_us` shows the failing H.34 value (> 100 000).
#[test]
fn c28_stop_resend_unanswered() {
    let (mut r, logger, sink) = logged_rig(vec![
        FakePort::bridge(CU, S, Some("Bonooru Bridge"))
            .mute_at(500_000)
            .unmute_at(2_000_000),
    ]);
    r.run_until(600_000);
    assert_eq!(r.snapshot.clone().unwrap().phase, Phase::Connected);
    let t0 = r.clock.now_us();
    r.ends.commands.send(WorkerCommand::Stop).unwrap();
    r.run_until(t0 + 1_000_000);
    let frames = stop_frames(&r);
    let n = frames.iter().filter(|&&t| t < t0 + 1_000_000).count();
    assert!((19..=21).contains(&n), "{n} STOP frames in 1 s: {frames:?}");
    assert!(diffs(&frames).iter().all(|&d| d == 50_000), "{frames:?}");
    assert_eq!(with_ctrl(&r, |c| c.log().stops), 0, "nothing reached it");
    step_to(&mut r, 3_000_000, |e| {
        *e == WorkerEvent::Link(LinkEvent::Stopped)
    });
    assert_eq!(
        with_ctrl(&r, |c| c.log().stops),
        1,
        "the next STOP is ACKed"
    );
    r.run_until(r.clock.now_us() + 100_000);
    let lines = finish(r, logger, &sink);
    let acked = lines.iter().find(|l| l.starts_with("acked")).unwrap();
    let after: u64 = acked
        .split(' ')
        .find_map(|f| f.strip_prefix("after_us="))
        .unwrap()
        .parse()
        .unwrap();
    assert!(after > 100_000, "H.34 failing value shown: {after}");
    assert!(after >= 2_000_000 - t0, "{after}");
    assert!(acked.ends_with("by=command"), "{acked}");
}

/// C.28 (AC 8): without a port, with a bridge alone (`Opening`) and with an
/// incompatible controller a STOP writes nothing (same writes as the same
/// run without it) and logs why.
#[test]
fn c28_stop_harmless() {
    // No port.
    let (mut r, logger, sink) = logged_rig(vec![]);
    r.run_until(100_000);
    r.ends.commands.send(WorkerCommand::Stop).unwrap();
    r.run_until(200_000);
    assert!(r.backend.state().writes.is_empty());
    assert_eq!(
        finish(r, logger, &sink),
        vec!["skipped reason=no_port port=-"]
    );
    // Bridge alone and incompatible controller: with and without STOP.
    type Case = (&'static str, fn() -> FakePort, u64);
    let cases: Vec<Case> = vec![
        (
            "opening",
            || FakePort::bridge(CU, S, Some("Bonooru Bridge")).silent(),
            550_000,
        ),
        (
            "incompatible",
            || {
                FakePort::bridge(CU, S, Some("Bonooru Bridge")).controller(ControllerConfig {
                    proto_ver: 2,
                    ..ControllerConfig::default()
                })
            },
            1_500_000,
        ),
    ];
    for (reason, port, at) in cases {
        let run = |stop: bool| {
            let (mut r, logger, sink) = logged_rig(vec![port()]);
            r.run_until(at);
            if stop {
                r.ends.commands.send(WorkerCommand::Stop).unwrap();
            }
            r.run_until(at + 1_000_000);
            let writes: Vec<(u64, Vec<u8>)> = r
                .backend
                .state()
                .writes
                .iter()
                .map(|w| (w.0, w.2.clone()))
                .collect();
            let phase = r.snapshot.clone().unwrap().phase;
            let counted = writes_to(&r, CU);
            (writes, counted, phase, finish(r, logger, &sink))
        };
        let (w0, c0, p0, l0) = run(false);
        let (w1, c1, p1, l1) = run(true);
        assert_eq!(p0, p1);
        assert_eq!(w0, w1, "{reason}: identical writes");
        assert_eq!(c0, c1, "{reason}: identical port_writes");
        assert!(l0.is_empty());
        assert_eq!(l1, vec![format!("skipped reason={reason} port={CU}")]);
    }
}

/// C.28 (AC 11): every worker `stop` line matches its pattern, one per
/// event; the `after` line can show `direct` (the failing H.34 value);
/// nothing is logged while nothing happens.
#[test]
fn c28_stop_log_lines() {
    let (mut r, logger, sink) = logged_rig(vec![FakePort::bridge(CU, S, Some("Bonooru Bridge"))]);
    // Idle for 3 s: no stop line.
    r.run_until(3_000_000);
    r.ends.commands.send(WorkerCommand::Stop).unwrap();
    step_to(&mut r, 4_000_000, |e| {
        *e == WorkerEvent::Link(LinkEvent::Stopped)
    });
    // An input held right after the ACK, before the next STATUS.
    let held = State {
        buttons: BTN_A,
        ..NEUTRAL
    };
    r.core.session_mut().unwrap().set_state(held).unwrap();
    r.run_until(r.clock.now_us() + 200_000);
    // Several STATUS later: still one `after` line per ACK.
    r.run_until(r.clock.now_us() + 2_000_000);
    let lines = finish(r, logger, &sink);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines.iter().all(|l| stop_line_ok(l)), "{lines:?}");
    assert!(
        lines[2].starts_with(&format!("after port={CU} mode=direct ")),
        "{lines:?}"
    );
    // The pattern rejects what it must.
    for bad in [
        "sent",
        "sent port=",
        "skipped reason=asleep port=-",
        "acked port=x after_us=12a by=command",
        "acked port=x after_us=1 by=user",
        "after port=x mode=idle",
        "after port=x mode=idle abort_count=1 extra=1",
        "after port=x mode=0x7 abort_count=1",
        "after port=x mode=0xzz abort_count=1",
    ] {
        assert!(!stop_line_ok(bad), "{bad}");
    }
    for good in [
        "after port=x mode=0x7f abort_count=3",
        "acked port=x after_us=- by=session",
        "skipped reason=no_port port=-",
    ] {
        assert!(stop_line_ok(good), "{good}");
    }
}

/// C.28 (AC 7; review finding 1): the intake takes every queued command in
/// one pass: three Stops queued together give `sent`, `pending`, `pending`
/// and one STOP frame; a Stop queued with a Shutdown is not sent (the
/// close lowers DTR); a Stop queued before the last sender is dropped is
/// drained, then the drop is a shutdown.
#[test]
fn c28_intake_takes_every_command() {
    let connect = |r: &mut SimRig| {
        step_to(r, 1_000_000, |e| {
            matches!(e, WorkerEvent::Link(LinkEvent::Connected(_)))
        });
        r.run_until(r.clock.now_us() + 20_000);
    };
    // Three in one batch.
    let (mut r, logger, sink) = logged_rig(vec![
        FakePort::bridge(CU, S, Some("Bonooru Bridge")).mute_at(400_000),
    ]);
    connect(&mut r);
    r.run_until(500_000);
    for _ in 0..3 {
        r.ends.commands.send(WorkerCommand::Stop).unwrap();
    }
    r.step();
    assert_eq!(stop_frames(&r).len(), 1);
    let lines = finish(r, logger, &sink);
    assert_eq!(
        lines,
        vec![
            format!("sent port={CU}"),
            format!("pending port={CU}"),
            format!("pending port={CU}"),
        ]
    );
    // Stop with Shutdown: shut down, nothing sent for the Stop.
    let (mut r, logger, sink) = logged_rig(vec![FakePort::bridge(CU, S, Some("Bonooru Bridge"))]);
    connect(&mut r);
    r.ends.commands.send(WorkerCommand::Stop).unwrap();
    r.ends.commands.send(WorkerCommand::Shutdown).unwrap();
    r.step();
    assert!(r.core.is_shut_down());
    assert!(stop_frames(&r).is_empty());
    assert!(finish(r, logger, &sink).is_empty());
    // Stop queued, then every sender dropped: drained, then shut down.
    let (mut r, logger, sink) = logged_rig(vec![FakePort::bridge(CU, S, Some("Bonooru Bridge"))]);
    connect(&mut r);
    r.ends.commands.send(WorkerCommand::Stop).unwrap();
    let (gone, _) = crossbeam_channel::bounded(1);
    drop(std::mem::replace(&mut r.ends.commands, gone));
    r.step();
    assert!(r.core.is_shut_down(), "a dropped handle is a shutdown");
    assert!(stop_frames(&r).is_empty());
    assert!(finish(r, logger, &sink).is_empty());
}
