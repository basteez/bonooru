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
use ocb_core::protocol::consts::{MSG_HELLO, MSG_SET_STATE, NAK_VERSION};
use ocb_core::protocol::{NEUTRAL, NakReason, RxSide, decode_frame};
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
