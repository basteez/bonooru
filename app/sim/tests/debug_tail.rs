//! C.20 (Story 4.0): the fake controller's debug STATUS carries the v6
//! block (`p2_queue_full`, offset 70 of a 72-byte tail with an empty P2
//! log window) when the tail is long enough, and leaves a Story 3.5 tail
//! (70 bytes) as it is.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use ocb_core::protocol::{Message, RxEvent, RxSide, Splitter};
use ocb_sim::{ControllerConfig, FakeController};

/// The `extra` tail of the first STATUS the controller sends.
fn first_status_extra(c: &mut FakeController) -> Vec<u8> {
    c.tick(1_000);
    let mut split = Splitter::new(RxSide::App);
    while let Some(frame) = c.poll_transmit() {
        for b in frame {
            if let RxEvent::Frame(f) = split.push(b)
                && let Ok(Message::Status(s)) = Message::decode(f.ty, &f.body)
            {
                return s.extra;
            }
        }
    }
    panic!("no STATUS");
}

#[test]
fn c20_v6_p2_queue_full_read_back_through_a_status() {
    let mut c = FakeController::new(ControllerConfig::default());
    c.set_debug_extra(Some(vec![0u8; 72]));
    assert_eq!(&first_status_extra(&mut c)[70..72], &[0, 0], "default 0");
    let mut c = FakeController::new(ControllerConfig::default());
    c.set_debug_extra(Some(vec![0u8; 72]));
    c.set_p2_queue_full(0x1234);
    let extra = first_status_extra(&mut c);
    assert_eq!(extra.len(), 72);
    assert_eq!(&extra[70..72], &[0x34, 0x12]);
}

#[test]
fn c20_a_story_3_5_tail_has_no_v6() {
    let mut c = FakeController::new(ControllerConfig::default());
    c.set_debug_extra(Some(vec![0u8; 70]));
    c.set_p2_queue_full(9);
    assert_eq!(first_status_extra(&mut c).len(), 70);
}
