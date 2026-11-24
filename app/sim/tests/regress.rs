//! Regression tests for the Story 3.3 code review (C.7 to C.11): each test
//! is the scenario a reviewer reported, on the strict driver.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use ocb_core::link::{AbandonReason, LinkEvent, Phase, SessionConfig};
use ocb_core::protocol::consts::{MSG_HELLO, MSG_PING, MSG_QUEUE_EVENT, MSG_STOP};
use ocb_core::protocol::{EventCode, NakReason};
use ocb_sim::{Channel, ControllerConfig, Impair, Sim};

fn piece(s: &mut Sim, n: u32, step_us: u32) {
    for i in 0..n {
        s.session
            .queue_event(10_000 + i * step_us, common::note(i as usize))
            .unwrap();
    }
    s.session
        .queue_event(10_000 + n * step_us, common::end())
        .unwrap();
}

#[test]
fn stop_never_takes_the_seq_of_a_frame_in_flight() {
    // Slow replies (30 ms) so old ACKs arrive while STOP waits; 300 events
    // pushed (more than 256), 45 of them in flight (capacity 45), so the
    // controller's last ACK is ACK(45) = 1 + 300 mod 256, the seq the old
    // code gave STOP. The first STOP is lost.
    let mut s = Sim::with_channels(
        SessionConfig::default(),
        ControllerConfig {
            queue_cap: 45,
            ..ControllerConfig::default()
        },
        Channel::clean(100),
        Channel::clean(30_000),
    );
    assert!(s.connect());
    for i in 0..300u32 {
        s.session
            .queue_event(10_000 + i * 1000, common::note(i as usize))
            .unwrap();
    }
    s.run_for(5_000);
    s.up.set_impair(Impair {
        drop: 1.0,
        ..Impair::default()
    });
    s.session.stop().unwrap();
    s.run_for(45_000);
    assert_eq!(
        s.session.phase(),
        Phase::Stopping,
        "an old ACK is not STOP's ACK"
    );
    assert!(!s.events().contains(&LinkEvent::Stopped));
    s.up.set_impair(Impair::default());
    assert!(common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::Stopped));
    assert_eq!(s.ctrl.log().stops, 1);
    let before = s.ctrl.log().enqueued.len();
    s.run_for(100_000);
    for i in 0..20u32 {
        s.session
            .queue_event(10_000 + i * 1000, common::note(i as usize))
            .unwrap();
    }
    assert!(s.run_until(1_000_000, |s| s.ctrl.log().enqueued.len() == before + 20));
    common::assert_clean(&s);
}

#[test]
fn a_piece_longer_than_the_queue_plays_to_the_end() {
    for imp in [Impair::default(), Impair::all(0.05)] {
        for seed in common::seeds(&[1, 2, 3]) {
            let ctrl = ControllerConfig {
                queue_cap: 64,
                ..ControllerConfig::default()
            };
            let mut s = common::connected_lossy(ctrl, imp, seed);
            piece(&mut s, 600, 10_000);
            s.session.start(50_000).unwrap();
            let tag = format!("seed {seed}, {imp:?}");
            assert!(
                common::wait_event(&mut s, 20_000_000, |e| *e == LinkEvent::PlaybackDone),
                "{tag}: {} applied, events {:?}",
                s.ctrl.log().applied.len(),
                s.events()
                    .into_iter()
                    .filter(|e| !matches!(e, LinkEvent::Status(_)))
                    .collect::<Vec<_>>()
            );
            assert_eq!(s.ctrl.log().applied.len(), 601, "{tag}");
            assert!(
                common::wait_event(&mut s, 1_000_000, |e| *e == LinkEvent::Stopped),
                "{tag}"
            );
            common::assert_clean(&s);
        }
    }
}

#[test]
fn direct_input_never_waits_behind_events_without_credit() {
    let mut s = common::connected(ControllerConfig::default());
    s.session.set_state(common::note(0)).unwrap();
    for i in 0..600u32 {
        s.session
            .queue_event(10_000 + i * 1000, common::note(i as usize))
            .unwrap();
    }
    for k in 1..=20 {
        s.run_for(100_000);
        s.session.set_state(common::note(k)).unwrap();
    }
    s.run_for(100_000);
    assert_eq!(
        s.ctrl.log().direct.len(),
        21,
        "every SET_STATE reached the controller"
    );
    assert_eq!(s.ctrl.log().app_timeouts, 0);
    assert_eq!(s.ctrl.queue_len(), 512);
    common::assert_clean(&s);
}

#[test]
fn an_event_before_the_first_status_does_not_hide_a_later_abort() {
    let mut s = Sim::new(SessionConfig::default(), ControllerConfig::default());
    s.session.open(s.now);
    assert!(s.run_until(1_000_000, |s| s.session.phase() == Phase::Connected));
    // Before the first STATUS of the session: an abort with an EVENT.
    s.ctrl.force_abort(Some(EventCode::UsbSuspended));
    assert!(common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::Stopped));
    s.run_for(100_000);
    s.session.set_state(common::note(0)).unwrap();
    s.run_for(50_000);
    s.ctrl.force_abort(None); // app timeout: no EVENT
    assert!(common::wait_event(&mut s, 500_000, |e| *e
        == LinkEvent::PieceAbandoned {
            reason: AbandonReason::ControllerAborted
        }));
    assert!(common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::Stopped));
    assert!(!s.ctrl.aborted());
    common::assert_clean(&s);
}

#[test]
fn a_lost_refusal_of_start_is_seen_from_status() {
    let mut s = common::connected(ControllerConfig::default());
    piece(&mut s, 5, 20_000);
    s.run_for(50_000);
    s.ctrl.set_console_ready(false);
    s.ctrl.lose_next_nak(); // the NAK NOT_READY on START never arrives
    s.session.start(20_000).unwrap();
    assert!(common::wait_event(&mut s, 1_000_000, |e| *e
        == LinkEvent::PieceAbandoned {
            reason: AbandonReason::NotPlaying
        }));
    assert!(common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::Stopped));
    common::assert_clean(&s);
}

#[test]
fn a_reboot_while_idle_is_seen_from_status() {
    let mut s = common::connected(ControllerConfig::default());
    piece(&mut s, 3, 20_000);
    s.run_for(50_000);
    s.session.start(10_000).unwrap();
    assert!(common::wait_event(&mut s, 1_000_000, |e| *e == LinkEvent::Stopped));
    s.run_for(100_000);
    s.ctrl.reboot();
    assert!(common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::ControllerRebooted));
    assert!(common::wait_event(&mut s, 500_000, |e| matches!(
        e,
        LinkEvent::Connected(_)
    )));
    assert!(!s.events().contains(&LinkEvent::PieceAbandoned {
        reason: AbandonReason::ControllerAborted
    }));
    common::assert_clean(&s);
}

#[test]
fn a_link_loss_seen_only_from_status_abandons_with_a_lone_zero() {
    let mut s = common::connected(ControllerConfig::default());
    piece(&mut s, 20, 50_000);
    s.run_for(50_000);
    s.session.start(20_000).unwrap();
    s.run_for(100_000);
    let t = s.now;
    s.ctrl.lose_next_event();
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
        "lone 00 before STOP"
    );
    common::assert_clean(&s);
}

#[test]
fn an_abort_after_a_finished_piece_is_not_reported_as_done() {
    let mut s = common::connected(ControllerConfig::default());
    piece(&mut s, 2, 20_000);
    s.run_for(50_000);
    s.session.start(10_000).unwrap();
    assert!(common::wait_event(&mut s, 1_000_000, |e| *e == LinkEvent::Stopped));
    s.run_for(100_000);
    // Piece 2: aborted with no EVENT; last_event still says PLAYBACK_DONE.
    piece(&mut s, 20, 50_000);
    s.run_for(50_000);
    s.session.start(10_000).unwrap();
    s.run_for(100_000);
    let done_before = s
        .events()
        .iter()
        .filter(|e| **e == LinkEvent::PlaybackDone)
        .count();
    s.ctrl.force_abort(None);
    assert!(common::wait_event(&mut s, 500_000, |e| *e
        == LinkEvent::PieceAbandoned {
            reason: AbandonReason::ControllerAborted
        }));
    let done_after = s
        .events()
        .iter()
        .filter(|e| **e == LinkEvent::PlaybackDone)
        .count();
    assert_eq!(done_before, done_after);
    common::assert_clean(&s);
}

#[test]
fn a_refused_ping_is_reported_without_stop() {
    let mut s = common::connected(ControllerConfig::default());
    s.session.set_state(common::note(0)).unwrap();
    s.run_for(10_000);
    s.ctrl.inject_nak(MSG_PING, NakReason::BadState);
    assert!(common::wait_event(&mut s, 1_000_000, |e| *e
        == LinkEvent::Refused {
            reason: NakReason::BadState,
            ty: MSG_PING
        }));
    s.run_for(100_000);
    assert!(!s.tx_frames().iter().any(|f| f.1 == MSG_STOP));
    common::assert_clean(&s);
}

#[test]
fn a_lossy_stop_still_completes() {
    for seed in common::seeds(&[1, 2, 3]) {
        let mut s = common::connected_lossy(ControllerConfig::default(), Impair::all(0.10), seed);
        piece(&mut s, 200, 10_000);
        s.run_for(100_000);
        s.session.start(50_000).unwrap();
        s.run_for(300_000);
        s.session.stop().unwrap();
        assert!(
            common::wait_event(&mut s, 2_000_000, |e| *e == LinkEvent::Stopped),
            "seed {seed}"
        );
        s.run_for(100_000);
        assert!(!s.ctrl.aborted() && s.ctrl.queue_len() == 0, "seed {seed}");
        common::assert_clean(&s);
    }
}

#[test]
fn a_tiny_app_timeout_still_pings() {
    let mut s = common::connected(ControllerConfig {
        app_timeout_ms: 3,
        ..ControllerConfig::default()
    });
    s.session.set_state(common::note(0)).unwrap();
    s.run_for(20_000);
    let pings = s.tx_frames().iter().filter(|f| f.1 == MSG_PING).count();
    assert!(pings >= 10, "{pings}");
    common::assert_clean(&s);
}

#[test]
fn a_resend_after_silence_starts_with_a_lone_zero() {
    let mut s = common::connected(ControllerConfig::default());
    s.up.set_impair(Impair {
        drop: 1.0,
        ..Impair::default()
    });
    let t = s.now;
    s.session.queue_event(1000, common::note(0)).unwrap();
    s.run_for(60_000);
    let after: Vec<&Vec<u8>> = s
        .seen
        .tx
        .iter()
        .filter(|(tt, _)| *tt >= t)
        .map(|(_, c)| c)
        .collect();
    // QUEUE_EVENT, then at 50 ms: 00, QUEUE_EVENT again.
    assert_eq!(after.len(), 3, "{after:?}");
    assert_eq!(after[1].as_slice(), &[0u8][..]);
    let _ = MSG_QUEUE_EVENT;
    common::assert_clean(&s);
}

#[test]
fn hello_after_a_reboot_follows_a_lone_zero() {
    let mut s = common::connected(ControllerConfig::default());
    s.ctrl.reboot();
    s.session.queue_event(1000, common::note(0)).unwrap();
    assert!(common::wait_event(&mut s, 500_000, |e| *e == LinkEvent::ControllerRebooted));
    s.run_for(10_000);
    let tx = &s.seen.tx;
    let hello_at = tx
        .iter()
        .rposition(|(_, c)| {
            c.len() > 1
                && ocb_core::protocol::decode_frame(
                    ocb_core::protocol::RxSide::Controller,
                    &c[..c.len() - 1],
                )
                .is_ok_and(|f| f.ty == MSG_HELLO)
        })
        .unwrap();
    assert_eq!(tx[hello_at - 1].1, vec![0], "00 right before HELLO");
    common::assert_clean(&s);
}
