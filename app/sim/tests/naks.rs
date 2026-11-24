//! C.8: semantic NAKs (AC3).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use ocb_core::link::{AbandonReason, LinkEvent, Phase};
use ocb_core::protocol::NakReason;
use ocb_core::protocol::consts::{MSG_QUEUE_EVENT, MSG_SET_STATE, MSG_START, MSG_STOP};
use ocb_sim::ControllerConfig;

const SEMANTIC: [NakReason; 6] = [
    NakReason::Order,
    NakReason::BadState,
    NakReason::Aborted,
    NakReason::NotReady,
    NakReason::Busy,
    NakReason::NoLink,
];

/// Plays a small piece to its end: the session must work again.
fn piece_plays(s: &mut ocb_sim::Sim) {
    for i in 0..3u32 {
        s.session
            .queue_event(10_000 + i * 20_000, common::note(i as usize))
            .unwrap();
    }
    s.session.queue_event(80_000, common::end()).unwrap();
    assert!(s.run_until(1_000_000, |s| {
        s.session.status().is_some_and(|st| st.free < 512)
    }));
    s.session.start(20_000).unwrap();
    assert!(common::wait_event(s, 2_000_000, |e| *e == LinkEvent::PlaybackDone));
    assert!(common::wait_event(s, 500_000, |e| *e == LinkEvent::Stopped));
}

fn abandon_on(ty: u8, reason: NakReason) {
    let mut s = common::connected(ControllerConfig::default());
    for i in 0..3u32 {
        s.session
            .queue_event(10_000 + i * 20_000, common::note(i as usize))
            .unwrap();
    }
    if ty == MSG_START {
        s.run_for(100_000);
        s.ctrl.inject_nak(MSG_START, reason);
        s.session.start(20_000).unwrap();
    } else {
        s.ctrl.inject_nak(MSG_QUEUE_EVENT, reason);
    }
    let tag = format!("{reason:?} on {ty:#04x}");
    assert!(
        common::wait_event(&mut s, 1_000_000, |e| *e
            == LinkEvent::PieceAbandoned {
                reason: AbandonReason::Nak(reason)
            }),
        "{tag}: {:?}",
        s.events()
    );
    assert!(
        common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::Stopped),
        "{tag}"
    );
    assert!(
        s.tx_frames().iter().any(|f| f.1 == MSG_STOP),
        "{tag}: STOP on the wire"
    );
    assert_eq!(s.session.phase(), Phase::Connected);
    piece_plays(&mut s);
    common::assert_clean(&s);
}

#[test]
fn c8_semantic_nak_on_queue_event_abandons_and_stops() {
    for r in SEMANTIC {
        abandon_on(MSG_QUEUE_EVENT, r);
    }
}

#[test]
fn c8_semantic_nak_on_start_abandons_and_stops() {
    for r in SEMANTIC {
        abandon_on(MSG_START, r);
    }
}

#[test]
fn c8_not_ready_start_from_the_controller_itself() {
    let mut s = common::connected(ControllerConfig::default());
    s.ctrl.set_console_ready(false);
    s.session.queue_event(10_000, common::note(0)).unwrap();
    s.run_for(100_000);
    s.session.start(20_000).unwrap();
    assert!(common::wait_event(&mut s, 1_000_000, |e| *e
        == LinkEvent::PieceAbandoned {
            reason: AbandonReason::Nak(NakReason::NotReady)
        }));
    common::assert_clean(&s);
}

#[test]
fn c8_semantic_nak_on_set_state_is_refused_without_stop() {
    let mut s = common::connected(ControllerConfig::default());
    s.ctrl.set_console_ready(false);
    s.session.set_state(common::note(0)).unwrap();
    assert!(common::wait_event(&mut s, 500_000, |e| *e
        == LinkEvent::Refused {
            reason: NakReason::NotReady,
            ty: MSG_SET_STATE
        }));
    s.run_for(200_000);
    assert!(
        !s.tx_frames().iter().any(|f| f.1 == MSG_STOP),
        "no STOP for a refused SET_STATE"
    );
    assert!(
        !s.events()
            .iter()
            .any(|e| matches!(e, LinkEvent::PieceAbandoned { .. }))
    );
    common::assert_clean(&s);
}
