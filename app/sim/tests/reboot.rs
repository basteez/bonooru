//! C.9: a controller reboot is seen as a mid-session `NAK VERSION` (AC4).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use ocb_core::link::{AbandonReason, LinkEvent, Phase, SessionConfig};
use ocb_core::protocol::consts::{MSG_HELLO, NAK_VERSION};
use ocb_sim::{ControllerConfig, Sim};

#[test]
fn c9_nak_version_mid_session_rehellos_and_reports_reboot() {
    let mut s = common::connected(ControllerConfig::default());
    s.session.queue_event(10_000, common::note(0)).unwrap();
    s.run_for(50_000);
    s.ctrl.reboot();
    let t_reboot = s.now;
    s.session.queue_event(30_000, common::note(1)).unwrap();
    assert!(common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::ControllerRebooted));
    let ev = s.events();
    assert!(
        ev.contains(&LinkEvent::PieceAbandoned {
            reason: AbandonReason::ControllerRebooted
        }),
        "{ev:?}"
    );
    // A lone 0x00 then HELLO again, and a new session.
    let after: Vec<&(u64, Vec<u8>)> = s.seen.tx.iter().filter(|(t, _)| *t >= t_reboot).collect();
    let zero = after
        .iter()
        .position(|(_, c)| c == &vec![0])
        .expect("a lone 00");
    let hello = s
        .tx_frames()
        .into_iter()
        .filter(|f| f.0 >= t_reboot && f.1 == MSG_HELLO)
        .count();
    assert!(hello >= 1);
    assert!(after.len() > zero + 1);
    assert!(common::wait_event(&mut s, 500_000, |e| matches!(
        e,
        LinkEvent::Connected(_)
    )));
    assert_eq!(s.session.phase(), Phase::Connected);
    assert_eq!(
        s.ctrl
            .log()
            .naks
            .iter()
            .filter(|n| n.0 == NAK_VERSION)
            .count(),
        1
    );
    common::assert_clean(&s);
}

#[test]
fn c9_nak_version_before_info_is_not_a_reboot() {
    let mut s = Sim::new(SessionConfig::default(), ControllerConfig::default());
    s.ctrl.inject_raw_nak(NAK_VERSION, 1, 0);
    s.session.open(s.now);
    s.run_for(500_000);
    let ev = s.events();
    assert!(!ev.contains(&LinkEvent::ControllerRebooted), "{ev:?}");
    assert!(ev.iter().any(|e| matches!(e, LinkEvent::Connected(_))));
    common::assert_clean(&s);
}
