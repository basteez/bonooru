//! C.10: PING in DIRECT after `app_timeout_ms / 4` of silence (AC5).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use ocb_core::protocol::consts::{MSG_PING, MSG_SET_STATE};
use ocb_sim::ControllerConfig;

fn times(s: &ocb_sim::Sim, ty: u8) -> Vec<u64> {
    s.tx_frames()
        .into_iter()
        .filter(|f| f.1 == ty)
        .map(|f| f.0)
        .collect()
}

fn cadence(app_timeout_ms: u16, interval_us: u64) {
    let mut s = common::connected(ControllerConfig {
        app_timeout_ms,
        ..ControllerConfig::default()
    });
    let idle_start = s.now;
    s.run_for(2_000_000);
    assert!(times(&s, MSG_PING).is_empty(), "no PING in IDLE");
    let _ = idle_start;
    s.session.set_state(common::note(0)).unwrap();
    s.run_for(1_000_000);
    let set = times(&s, MSG_SET_STATE);
    let pings = times(&s, MSG_PING);
    assert_eq!(set.len(), 1);
    assert!(pings.len() >= 3, "{pings:?}");
    assert_eq!(pings[0], set[0] + interval_us);
    for w in pings.windows(2) {
        assert_eq!(w[1] - w[0], interval_us, "{pings:?}");
    }
    // A new SET_STATE resets the timer.
    s.session.set_state(common::note(1)).unwrap();
    let before = times(&s, MSG_PING).len();
    s.run_for(interval_us + 1_000);
    let set2 = *times(&s, MSG_SET_STATE).last().unwrap();
    let pings2 = times(&s, MSG_PING);
    assert_eq!(pings2[before], set2 + interval_us);
    // STOP ends DIRECT: no more PING.
    s.session.stop().unwrap();
    s.run_for(100_000);
    let n = times(&s, MSG_PING).len();
    s.run_for(2_000_000);
    assert_eq!(times(&s, MSG_PING).len(), n, "no PING after STOP");
    common::assert_clean(&s);
}

#[test]
fn c10_ping_every_quarter_of_the_app_timeout() {
    cadence(1000, 250_000);
    cadence(400, 100_000);
}

#[test]
fn c10_idle_direct_never_trips_the_app_timeout() {
    let mut s = common::connected(ControllerConfig::default());
    s.session.set_state(common::note(0)).unwrap();
    s.run_for(10_000_000);
    assert_eq!(s.ctrl.log().app_timeouts, 0);
    assert!(!s.ctrl.aborted());
    assert!(s.ctrl.log().pings >= 39);
    common::assert_clean(&s);
}

#[test]
fn c10_no_ping_while_playing() {
    let mut s = common::connected(ControllerConfig::default());
    for i in 0..20u32 {
        s.session
            .queue_event(10_000 + i * 100_000, common::note(i as usize))
            .unwrap();
    }
    s.session.queue_event(2_100_000, common::end()).unwrap();
    s.run_for(100_000);
    s.session.start(10_000).unwrap();
    s.run_for(2_500_000);
    assert!(times(&s, MSG_PING).is_empty());
    common::assert_clean(&s);
}
