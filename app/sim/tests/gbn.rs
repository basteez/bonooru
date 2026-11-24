//! C.7: go-back-N and credits through an impaired channel (AC2).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use ocb_core::protocol::State;
use ocb_core::protocol::consts::{MSG_QUEUE_EVENT, NAK_FULL};
use ocb_sim::{ControllerConfig, Impair};

const N: usize = 2000;

fn stream(seed: u64, impair: Impair, cap: u16) {
    let ctrl = ControllerConfig {
        queue_cap: cap,
        sink_every_us: Some(500),
        ..ControllerConfig::default()
    };
    let mut s = common::connected_lossy(ctrl, impair, seed);
    let want: Vec<(u32, State)> = (0..N)
        .map(|i| (1000 + i as u32 * 1000, common::note(i)))
        .collect();
    for &(t, st) in &want {
        s.session.queue_event(t, st).unwrap();
    }
    let ok = s.run_until(300_000_000, |s| {
        s.ctrl.log().enqueued.len() >= N && s.session.unacked() == 0
    });
    let tag = format!("seed {seed}, {impair:?}, cap {cap}");
    assert!(
        ok,
        "{tag}: only {} of {N} enqueued",
        s.ctrl.log().enqueued.len()
    );
    assert_eq!(
        s.ctrl.log().enqueued,
        want,
        "{tag}: every event exactly once, in order"
    );
    common::assert_clean(&s);
    assert!(
        !s.ctrl.log().naks.iter().any(|n| n.0 == NAK_FULL),
        "{tag}: credits must keep the queue from overflowing"
    );
    // The impairments really happened (no vacuous pass).
    let (u, d) = (s.up.stats(), s.down.stats());
    let hit = |p: f64, n: u64| p == 0.0 || n > 0;
    assert!(
        hit(impair.drop, u.dropped + d.dropped),
        "{tag}: nothing dropped"
    );
    assert!(
        hit(impair.flip, u.flipped + d.flipped),
        "{tag}: nothing flipped"
    );
    assert!(
        hit(impair.dup, u.duplicated + d.duplicated),
        "{tag}: nothing duplicated"
    );
    assert!(
        hit(impair.reorder, u.reordered + d.reordered),
        "{tag}: nothing reordered"
    );
    assert!(hit(impair.junk, u.junked + d.junked), "{tag}: no junk");
    if impair.drop >= 0.05 {
        assert!(st_check(&s), "{tag}: a lossy run must have needed recovery");
    }
    if std::env::var("OCB_SIM_VERBOSE").is_ok() {
        let st = s.session.send_stats();
        println!(
            "{tag}: t={} ms sent={} resends={} rewinds={} up={u:?} down={d:?}",
            s.now / 1000,
            st.sent,
            st.resends,
            st.rewinds
        );
    }
    // A rewind only on a transport NAK ahead of the last rewind point: never
    // more rewinds than distinct NAK points, plus the points the 50 ms
    // resend clears.
    let st = s.session.send_stats();
    let points = s.seen.transport_nak_points.len() as u64;
    assert!(
        st.rewinds <= points + st.resends,
        "{tag}: {} rewinds for {points} NAK points and {} resends",
        st.rewinds,
        st.resends
    );
}

fn st_check(s: &ocb_sim::Sim) -> bool {
    let st = s.session.send_stats();
    st.resends + st.rewinds > 0
}

fn matrix() -> Vec<Impair> {
    let mut v = vec![Impair::default()];
    for p in [0.01, 0.05, 0.10] {
        v.push(Impair {
            drop: p,
            ..Impair::default()
        });
    }
    v.push(Impair {
        flip: 0.05,
        ..Impair::default()
    });
    v.push(Impair {
        dup: 0.05,
        ..Impair::default()
    });
    v.push(Impair {
        reorder: 0.05,
        ..Impair::default()
    });
    v.push(Impair {
        junk: 0.05,
        ..Impair::default()
    });
    v.push(Impair::all(0.05));
    v.push(Impair::all(0.10));
    v
}

#[test]
fn c7_every_event_once_in_order_capacity_512() {
    for seed in common::seeds(&[1, 2, 3]) {
        for imp in matrix() {
            stream(seed, imp, 512);
        }
    }
}

#[test]
fn c7_every_event_once_in_order_capacity_5() {
    for seed in common::seeds(&[1, 2, 3]) {
        for imp in matrix() {
            stream(seed, imp, 5);
        }
    }
}

#[test]
fn c7_resend_after_exactly_50_ms() {
    let mut s = common::connected(ControllerConfig::default());
    s.up.set_impair(Impair {
        drop: 1.0,
        ..Impair::default()
    });
    s.session.queue_event(1000, common::note(0)).unwrap();
    s.run_for(260_000);
    let qe: Vec<u64> = s
        .tx_frames()
        .into_iter()
        .filter(|f| f.1 == MSG_QUEUE_EVENT)
        .map(|f| f.0)
        .collect();
    assert!(qe.len() >= 5, "{qe:?}");
    for w in qe.windows(2) {
        assert_eq!(w[1] - w[0], 50_000, "{qe:?}");
    }
    common::assert_clean(&s);
}

#[test]
fn c7_window_of_64_and_credit_limit() {
    // 100 free slots but nothing ever acknowledged: 64 frames, then nothing.
    let mut s = common::connected(ControllerConfig::default());
    s.up.set_impair(Impair {
        drop: 1.0,
        ..Impair::default()
    });
    for i in 0..100 {
        s.session
            .queue_event(1000 + i * 1000, common::note(i as usize))
            .unwrap();
    }
    s.run_for(10_000);
    assert_eq!(s.session.unacked(), 64);
    // A capacity of 3: 3 frames go out and wait for credit.
    let mut s = common::connected(ControllerConfig {
        queue_cap: 3,
        ..ControllerConfig::default()
    });
    for i in 0..10 {
        s.session
            .queue_event(1000 + i * 1000, common::note(i as usize))
            .unwrap();
    }
    s.run_for(200_000);
    assert_eq!(s.ctrl.log().enqueued.len(), 3);
    assert_eq!(s.session.credit(), Some(0));
    common::assert_clean(&s);
}

#[test]
fn c7_a_duplicated_nak_rewinds_once() {
    // Every controller frame arrives twice; app frames are lost now and then,
    // so every transport NAK is seen twice. Only the first may rewind.
    for seed in common::seeds(&[1, 2, 3]) {
        let ctrl = ControllerConfig { sink_every_us: Some(500), ..ControllerConfig::default() };
        let mut s = common::connected_lossy(ctrl, Impair::default(), seed);
        s.up.set_impair(Impair { drop: 0.05, ..Impair::default() });
        s.down.set_impair(Impair { dup: 1.0, ..Impair::default() });
        for i in 0..N {
            s.session.queue_event(1000 + i as u32 * 1000, common::note(i)).unwrap();
        }
        assert!(s.run_until(300_000_000, |s| s.ctrl.log().enqueued.len() >= N));
        let st = s.session.send_stats();
        let points = s.seen.transport_nak_points.len() as u64;
        assert!(s.seen.transport_naks >= 2 * points && points > 10, "seed {seed}: {points} points");
        assert!(
            st.rewinds <= points,
            "seed {seed}: {} rewinds for {points} NAK points ({} NAKs delivered)",
            st.rewinds,
            s.seen.transport_naks
        );
        common::assert_clean(&s);
    }
}
