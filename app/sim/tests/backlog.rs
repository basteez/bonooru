//! `Session::events_unsent` (Story 3.4 Task 1): the count a streamer uses to
//! keep a bounded backlog (QUEUE_EVENTs pushed and not yet on the wire).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use ocb_core::link::LinkEvent;
use ocb_sim::ControllerConfig;

#[test]
fn events_unsent_follows_queue_and_wire() {
    let mut s = common::connected(ControllerConfig {
        queue_cap: 8,
        ..ControllerConfig::default()
    });
    assert_eq!(s.session.events_unsent(), 0);
    for i in 0..20u32 {
        s.session
            .queue_event(10_000 + i * 10_000, common::note(i as usize))
            .unwrap();
    }
    assert_eq!(s.session.events_unsent(), 20, "nothing sent yet");
    // The credit (8 free slots) lets 8 go out; the rest wait.
    s.run_for(200_000);
    assert_eq!(s.session.events_unsent(), 12, "8 sent on credit");
    s.session.start(100_000).unwrap();
    s.run_for(2_000_000);
    assert_eq!(s.session.events_unsent(), 0, "all streamed after START");
}

#[test]
fn events_unsent_is_zero_after_stop() {
    let mut s = common::connected(ControllerConfig {
        queue_cap: 4,
        ..ControllerConfig::default()
    });
    for i in 0..10u32 {
        s.session
            .queue_event(10_000 + i * 10_000, common::note(i as usize))
            .unwrap();
    }
    s.run_for(100_000);
    assert!(s.session.events_unsent() > 0);
    s.session.stop().unwrap();
    assert_eq!(s.session.events_unsent(), 0, "STOP abandons the backlog");
    assert!(common::wait_event(&mut s, 1_000_000, |e| matches!(
        e,
        LinkEvent::Stopped
    )));
}
