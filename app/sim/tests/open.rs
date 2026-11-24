//! C.6: session open and versions (AC1).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use ocb_core::link::{LinkEvent, Phase, SessionConfig};
use ocb_core::protocol::consts::{MSG_HELLO, PROTO_VERSION};
use ocb_core::protocol::{HidProfile, Info};
use ocb_sim::{Channel, ControllerConfig, Impair, Sim};

#[test]
fn c6_zero_then_only_hello_every_100_ms_until_info() {
    // The controller's answers are all lost: INFO never arrives.
    let mut down = Channel::clean(100);
    down.set_impair(Impair {
        drop: 1.0,
        ..Impair::default()
    });
    let cfg = SessionConfig {
        hello_seq: 77,
        ..SessionConfig::default()
    };
    let mut s = Sim::with_channels(cfg, ControllerConfig::default(), Channel::clean(100), down);
    let t0 = s.now;
    s.session.open(t0);
    s.run_for(1_050_000);
    assert_eq!(s.seen.tx[0], (t0, vec![0]), "a lone 0x00 first");
    let frames = s.tx_frames();
    assert_eq!(frames.len(), 11, "{frames:?}");
    for (i, (t, ty, seq)) in frames.iter().enumerate() {
        assert_eq!((*ty, *seq), (MSG_HELLO, 77));
        assert_eq!(*t, t0 + i as u64 * 100_000, "HELLO {i}");
    }
    assert_eq!(s.seen.tx.len(), 12, "nothing but 00 and HELLOs");
    assert_eq!(s.session.phase(), Phase::Opening);
    common::assert_clean(&s);
}

#[test]
fn c6_info_values_are_exposed_not_hard_coded() {
    let ctrl = ControllerConfig {
        fw_ver: [0, 0, 0],
        queue_cap: 7,
        app_timeout_ms: 400,
        report_period_us: 8000,
        ..ControllerConfig::default()
    };
    let s = common::connected(ctrl);
    let want = Info {
        proto_ver: PROTO_VERSION,
        fw_ver: [0, 0, 0],
        bridge_ver: [0, 0, 0],
        hid_profile: HidProfile::P2,
        queue_cap: 7,
        app_timeout_ms: 400,
        report_period_us: 8000,
    };
    assert_eq!(s.session.info(), Some(&want));
    let first = s
        .events()
        .into_iter()
        .find(|e| !matches!(e, LinkEvent::Status(_)));
    assert_eq!(first, Some(LinkEvent::Connected(want)));
    // Credit comes from STATUS free, i.e. the controller's capacity.
    assert_eq!(s.session.credit(), Some(7));
    common::assert_clean(&s);
}

#[test]
fn c6_protocol_mismatch_is_a_required_update_and_hello_stops() {
    let ctrl = ControllerConfig {
        proto_ver: 2,
        ..ControllerConfig::default()
    };
    let mut s = Sim::new(SessionConfig::default(), ctrl);
    s.session.open(s.now);
    s.run_for(1_000_000);
    let ev = s.events();
    assert!(
        ev.iter().any(|e| matches!(
            e,
            LinkEvent::RequiredUpdate {
                controller_proto: 2,
                app_proto: 1,
                ..
            }
        )),
        "{ev:?}"
    );
    assert!(!ev.iter().any(|e| matches!(e, LinkEvent::Connected(_))));
    assert_eq!(s.session.phase(), Phase::Incompatible);
    assert_eq!(s.tx_frames().len(), 1, "one HELLO, then silence");
    common::assert_clean(&s);
}

fn recommended(fw: [u8; 3], bridge: [u8; 3]) -> Vec<LinkEvent> {
    let cfg = SessionConfig {
        app_version: [0, 2, 0],
        ..SessionConfig::default()
    };
    let ctrl = ControllerConfig {
        fw_ver: fw,
        bridge_ver: bridge,
        ..ControllerConfig::default()
    };
    let mut s = Sim::new(cfg, ctrl);
    assert!(s.connect());
    s.events()
        .into_iter()
        .filter(|e| matches!(e, LinkEvent::RecommendedUpdate { .. }))
        .collect()
}

#[test]
fn c6_version_differences_are_a_recommended_update() {
    assert_eq!(recommended([0, 2, 0], [0, 2, 0]), vec![]);
    assert_eq!(
        recommended([0, 2, 0], [0, 0, 0]),
        vec![],
        "zero bridge = no heartbeat yet"
    );
    assert_eq!(
        recommended([0, 3, 0], [0, 0, 0]),
        vec![LinkEvent::RecommendedUpdate {
            fw_ver: [0, 3, 0],
            bridge_ver: [0, 0, 0],
            app_ver: [0, 2, 0],
            fw_differs: true,
            bridge_differs: false,
        }]
    );
    assert_eq!(
        recommended([0, 2, 0], [0, 1, 9]),
        vec![LinkEvent::RecommendedUpdate {
            fw_ver: [0, 2, 0],
            bridge_ver: [0, 1, 9],
            app_ver: [0, 2, 0],
            fw_differs: false,
            bridge_differs: true,
        }]
    );
}
