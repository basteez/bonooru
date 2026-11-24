//! C.11: unknown codes (AC6), recovery rules and local refusals.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use ocb_core::link::{AbandonReason, CommandError, LinkEvent, Phase, Session, SessionConfig};
use ocb_core::protocol::consts::{MSG_STOP, T_US_LIMIT};
use ocb_core::protocol::{EventCode, NEUTRAL, State};
use ocb_sim::ControllerConfig;

fn sink() -> ControllerConfig {
    ControllerConfig {
        sink_every_us: Some(1000),
        ..ControllerConfig::default()
    }
}

#[test]
fn c11_unknown_event_code_is_reported_and_the_session_goes_on() {
    let mut s = common::connected(sink());
    for i in 0..200u32 {
        s.session
            .queue_event(1000 + i * 1000, common::note(i as usize))
            .unwrap();
    }
    s.run_for(20_000);
    s.ctrl.inject_event(0xEE);
    let ok = s.run_until(5_000_000, |s| s.ctrl.log().applied.len() == 200);
    assert!(
        ok,
        "{} enqueued, phase {:?}, events {:?}",
        s.ctrl.log().enqueued.len(),
        s.session.phase(),
        s.events()
            .into_iter()
            .filter(|e| !matches!(e, LinkEvent::Status(_)))
            .collect::<Vec<_>>()
    );
    assert!(
        s.events()
            .contains(&LinkEvent::ControllerEvent(EventCode::Unknown(0xEE)))
    );
    assert_eq!(s.session.phase(), Phase::Connected);
    assert!(!s.tx_frames().iter().any(|f| f.1 == MSG_STOP));
    common::assert_clean(&s);
}

#[test]
fn c11_unknown_nak_reason_never_rewinds_or_stops() {
    let mut s = common::connected(sink());
    for i in 0..200u32 {
        s.session
            .queue_event(1000 + i * 1000, common::note(i as usize))
            .unwrap();
    }
    s.run_for(20_000);
    let rewinds = s.session.send_stats().rewinds;
    s.ctrl.inject_raw_nak(0x7E, 3, 9);
    s.ctrl.inject_raw_nak(0x05, 3, 9); // unassigned, inside the transport range
    assert!(s.run_until(5_000_000, |s| s.ctrl.log().applied.len() == 200));
    let ev = s.events();
    assert!(
        ev.contains(&LinkEvent::UnknownNak {
            reason: 0x7E,
            expected: 3,
            got: 9
        }),
        "{ev:?}"
    );
    assert!(ev.contains(&LinkEvent::UnknownNak {
        reason: 0x05,
        expected: 3,
        got: 9
    }));
    assert_eq!(s.session.send_stats().rewinds, rewinds);
    assert!(!s.tx_frames().iter().any(|f| f.1 == MSG_STOP));
    common::assert_clean(&s);
}

#[test]
fn c11_link_lost_abandons_with_a_lone_zero_before_stop() {
    let mut s = common::connected(ControllerConfig::default());
    for i in 0..10u32 {
        s.session
            .queue_event(10_000 + i * 50_000, common::note(i as usize))
            .unwrap();
    }
    s.run_for(60_000);
    s.session.start(20_000).unwrap();
    s.run_for(50_000);
    let t = s.now;
    s.ctrl.set_link_ok(false);
    assert!(common::wait_event(&mut s, 500_000, |e| *e
        == LinkEvent::PieceAbandoned {
            reason: AbandonReason::LinkLost
        }));
    let after: Vec<&Vec<u8>> = s
        .seen
        .tx
        .iter()
        .filter(|(tt, _)| *tt >= t)
        .map(|(_, c)| c)
        .collect();
    assert_eq!(
        after.first().map(|c| c.as_slice()),
        Some(&[0u8][..]),
        "lone 00 first"
    );
    assert!(common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::Stopped));
    s.run_for(100_000);
    assert_eq!(s.session.start(0), Err(CommandError::NoLink));
    assert_eq!(
        s.session.set_state(common::note(0)),
        Err(CommandError::NoLink)
    );
    common::assert_clean(&s);
}

#[test]
fn c11_abort_without_event_abandons() {
    let mut s = common::connected(ControllerConfig::default());
    s.session.set_state(common::note(0)).unwrap();
    s.run_for(50_000);
    s.ctrl.force_abort(None); // app timeout, DTR edge or core stall: no EVENT
    assert!(common::wait_event(&mut s, 500_000, |e| *e
        == LinkEvent::PieceAbandoned {
            reason: AbandonReason::ControllerAborted
        }));
    assert!(common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::Stopped));
    assert!(!s.ctrl.aborted(), "STOP cleared the latch");
    common::assert_clean(&s);
}

#[test]
fn c11_playback_done_sends_stop() {
    let mut s = common::connected(ControllerConfig::default());
    s.session.queue_event(10_000, common::note(0)).unwrap();
    s.session.queue_event(40_000, common::end()).unwrap();
    s.run_for(60_000);
    s.session.start(10_000).unwrap();
    assert!(common::wait_event(&mut s, 1_000_000, |e| *e == LinkEvent::PlaybackDone));
    assert!(common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::Stopped));
    assert!(!s.ctrl.aborted());
    assert!(
        !s.events()
            .iter()
            .any(|e| matches!(e, LinkEvent::PieceAbandoned { .. }))
    );
    common::assert_clean(&s);
}

#[test]
fn c11_local_refusals_mirror_the_controller() {
    let mut closed = Session::new(SessionConfig::default());
    assert_eq!(
        closed.queue_event(1, NEUTRAL),
        Err(CommandError::NotConnected)
    );
    assert_eq!(closed.stop(), Err(CommandError::NotConnected));

    let mut s = common::connected(ControllerConfig::default());
    let ses = &mut s.session;
    assert_eq!(ses.start(0), Err(CommandError::EmptyQueue));
    ses.queue_event(100, common::note(0)).unwrap();
    assert_eq!(
        ses.queue_event(100, common::note(0)),
        Err(CommandError::Order)
    );
    assert_eq!(
        ses.queue_event(50, common::note(0)),
        Err(CommandError::Order)
    );
    assert_eq!(
        ses.queue_event(T_US_LIMIT, common::note(0)),
        Err(CommandError::Order)
    );
    assert_eq!(
        ses.queue_event(200, State { hat: 9, ..NEUTRAL }),
        Err(CommandError::BadState)
    );
    assert_eq!(
        ses.queue_event(
            200,
            State {
                flags: 0x02,
                ..NEUTRAL
            }
        ),
        Err(CommandError::BadState)
    );
    let end_on_a = State {
        flags: 1,
        ..common::note(0)
    };
    assert_eq!(ses.queue_event(200, end_on_a), Err(CommandError::BadState));
    ses.queue_event(200, common::end()).unwrap();
    assert_eq!(
        ses.queue_event(300, common::note(1)),
        Err(CommandError::BadState),
        "after END"
    );
    assert_eq!(
        ses.set_state(State {
            flags: 1,
            ..NEUTRAL
        }),
        Err(CommandError::BadState)
    );
    ses.start(10_000).unwrap();
    assert_eq!(ses.start(10_000), Err(CommandError::Busy));
    assert_eq!(ses.set_state(common::note(0)), Err(CommandError::Busy));
    ses.stop().unwrap();
    assert_eq!(ses.queue_event(1, NEUTRAL), Err(CommandError::Stopping));
    common::assert_clean(&s);
}
