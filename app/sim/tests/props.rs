//! C.11 (send buffer property): random ACKs, NAKs, STATUS and timeouts never
//! break the window, never panic, and never send a new QUEUE_EVENT without
//! credit.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use ocb_core::link::SendBuffer;
use ocb_core::protocol::consts::{MSG_PING, MSG_QUEUE_EVENT};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    PushQe,
    PushPing,
    Send,
    Ack(u8),
    Nak(u8),
    Status(u16, u8),
    Wait(u64),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => Just(Op::PushQe),
        1 => Just(Op::PushPing),
        4 => Just(Op::Send),
        2 => any::<u8>().prop_map(Op::Ack),
        1 => any::<u8>().prop_map(Op::Nak),
        2 => (0u16..20, any::<u8>()).prop_map(|(f, s)| Op::Status(f, s)),
        1 => (0u64..80_000).prop_map(Op::Wait),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]
    #[test]
    fn c11_send_buffer_invariants(sync in any::<u8>(), ops in proptest::collection::vec(op(), 0..400)) {
        let mut b = SendBuffer::new(sync);
        let mut now = 0u64;
        let mut new_qe_seen = 0u64;
        for o in ops {
            match o {
                Op::PushQe => { b.push(MSG_QUEUE_EVENT, vec![0; 12]); }
                Op::PushPing => { b.push(MSG_PING, vec![]); }
                Op::Send => {
                    let before = b.unacked();
                    let credit = b.credit();
                    if let Some((_, ty, _)) = b.next_to_send(now)
                        && b.unacked() > before
                        && ty == MSG_QUEUE_EVENT
                    {
                        // A new QUEUE_EVENT needs a positive credit.
                        prop_assert!(credit.is_some_and(|c| c > 0), "credit {credit:?}");
                        new_qe_seen += 1;
                    }
                }
                Op::Ack(s) => { b.ack(s, now); }
                Op::Nak(s) => { b.transport_nak(s, now); }
                Op::Status(f, s) => { b.status(f, s, now); }
                Op::Wait(d) => { now += d; let _ = b.check_timeout(now); }
            }
            prop_assert!(b.unacked() <= 64);
            prop_assert!(b.unacked() <= b.len());
        }
        // Not vacuous: count the runs that really sent a QUEUE_EVENT.
        if new_qe_seen > 0 {
            SENT_SOME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

static SENT_SOME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[test]
fn c11_send_buffer_property_sends_queue_events() {
    c11_send_buffer_invariants();
    assert!(
        SENT_SOME.load(std::sync::atomic::Ordering::Relaxed) > 50,
        "too few runs sent a QUEUE_EVENT"
    );
}
