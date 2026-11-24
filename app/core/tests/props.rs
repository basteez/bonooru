//! C.2 (property round trips, proptest) and C.3 (codec API edges).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use ocb_core::protocol::consts::{
    BODY_LEN_STATUS, MAX_BODY, MAX_WIRE_FRAME, NAK_SEMANTIC_MIN, NAK_TRANSPORT_MAX,
};
use ocb_core::protocol::{
    EventCode, Frame, FrameError, HidProfile, Message, Mode, MsgError, NakReason, RxEvent, RxSide,
    Splitter, Status, cobs, decode_frame, encode_frame,
};
use proptest::prelude::*;

/// Bytes biased toward zeros and long non-zero runs (the COBS edge cases).
fn cobs_input() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        proptest::collection::vec(any::<u8>(), 0..=600),
        proptest::collection::vec(prop_oneof![3 => Just(0u8), 1 => 1u8..=255], 0..=600),
        proptest::collection::vec(prop_oneof![1 => Just(0u8), 200 => 1u8..=255], 0..=600),
    ]
}

fn side_and_type() -> impl Strategy<Value = (RxSide, u8)> {
    prop_oneof![
        (0x01u8..=0x7F).prop_map(|t| (RxSide::Controller, t)),
        (0x80u8..=0xFF).prop_map(|t| (RxSide::App, t)),
    ]
}

/// One chunk of a random line: a valid frame, garbage, or delimiters.
#[derive(Debug, Clone)]
enum Chunk {
    Frame(u8, u8, Vec<u8>),
    Garbage(Vec<u8>),
    Zeros(usize),
}

fn chunk() -> impl Strategy<Value = Chunk> {
    prop_oneof![
        4 => (0x80u8..=0xFF, any::<u8>(), proptest::collection::vec(any::<u8>(), 0..=40))
            .prop_map(|(t, s, b)| Chunk::Frame(t, s, b)),
        1 => proptest::collection::vec(any::<u8>(), 0..=400).prop_map(Chunk::Garbage),
        1 => (1usize..=3).prop_map(Chunk::Zeros),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn c2_cobs_round_trip(x in cobs_input()) {
        let enc = cobs::encode(&x);
        prop_assert!(!enc.contains(&0));
        prop_assert!(enc.len() <= x.len() + x.len().div_ceil(254) + 1);
        prop_assert_eq!(cobs::decode(&enc).unwrap(), x);
    }

    #[test]
    fn c2_frame_round_trip((side, ty) in side_and_type(), seq in any::<u8>(),
                           body in proptest::collection::vec(any::<u8>(), 0..=MAX_BODY)) {
        let wire = encode_frame(ty, seq, &body).unwrap();
        prop_assert!(wire.len() <= MAX_WIRE_FRAME);
        prop_assert_eq!(*wire.last().unwrap(), 0);
        prop_assert!(!wire[..wire.len() - 1].contains(&0));
        let f = decode_frame(side, &wire[..wire.len() - 1]).unwrap();
        prop_assert_eq!(f, Frame { ty, seq, body });
    }

    #[test]
    fn c2_splitter_chunking(chunks in proptest::collection::vec(chunk(), 0..=30),
                            cuts in proptest::collection::vec(1usize..=50, 1..=40)) {
        let mut line = Vec::new();
        let mut want = Vec::new();
        for c in &chunks {
            match c {
                Chunk::Frame(t, s, b) => {
                    // A frame after a delimiter is always delivered.
                    line.push(0);
                    line.extend(encode_frame(*t, *s, b).unwrap());
                    want.push(Frame { ty: *t, seq: *s, body: b.clone() });
                }
                Chunk::Garbage(g) => line.extend(g),
                Chunk::Zeros(n) => line.extend(std::iter::repeat_n(0, *n)),
            }
        }
        // Byte by byte.
        let mut a = Splitter::new(RxSide::App);
        let mut got_a = Vec::new();
        for &b in &line {
            if let RxEvent::Frame(f) = a.push(b) {
                got_a.push(f);
            }
        }
        // Random chunks through push_slice.
        let mut b = Splitter::new(RxSide::App);
        let mut got_b = Vec::new();
        let mut rest = line.as_slice();
        for &n in cuts.iter().cycle() {
            if rest.is_empty() {
                break;
            }
            let (head, tail) = rest.split_at(n.min(rest.len()));
            got_b.extend(b.push_slice(head));
            rest = tail;
        }
        prop_assert_eq!(&got_a, &got_b);
        prop_assert_eq!(a.stats(), b.stats());
        // Every planted frame is delivered, in order (garbage may add frames
        // only if it happens to be a valid frame, so check as a subsequence).
        let mut it = got_a.iter();
        for w in &want {
            prop_assert!(it.any(|g| g == w), "frame {:?} not delivered", w);
        }
    }

    #[test]
    fn c2_no_panic_on_arbitrary_input(x in proptest::collection::vec(any::<u8>(), 0..=600),
                                      ty in any::<u8>()) {
        let _ = decode_frame(RxSide::App, &x);
        let _ = decode_frame(RxSide::Controller, &x);
        let _ = cobs::decode(&x);
        let _ = Message::decode(ty, &x);
        let mut s = Splitter::new(RxSide::App);
        let _ = s.push_slice(&x);
    }

    #[test]
    fn c2_message_round_trip(ty in prop::sample::select(vec![
        0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x40, 0x81, 0x83, 0x84, 0x85, 0x86]),
        body in proptest::collection::vec(any::<u8>(), 0..=40)) {
        // Whatever decodes re-encodes to a prefix of the body (trailing bytes
        // dropped by HELLO / INFO / HEARTBEAT) or to the whole body.
        if let Ok(m) = Message::decode(ty, &body) {
            let enc = m.encode_body().unwrap();
            prop_assert!(body.starts_with(&enc));
            prop_assert_eq!(Message::decode(ty, &enc).unwrap(), m);
        }
    }
}

#[test]
fn c3_body_too_long_refused() {
    assert_eq!(
        encode_frame(0x83, 0, &[0u8; 256]),
        Err(FrameError::BodyTooLong)
    );
    assert!(encode_frame(0x83, 0, &[0u8; 255]).is_ok());
}

#[test]
fn c3_status_extra_limit() {
    let mut s = Status {
        free: 0,
        now_us: 0,
        last_seq: 0,
        mode: Mode::Idle,
        abort_count: 0,
        last_event: EventCode::None,
        flags: 0,
        extra: vec![0xAA; MAX_BODY - BODY_LEN_STATUS],
    };
    assert_eq!(s.encode().unwrap().len(), MAX_BODY);
    assert!(Message::Status(s.clone()).encode_frame(7).is_ok());
    s.extra.push(0);
    assert_eq!(s.encode(), Err(MsgError::TooLong));
    assert_eq!(Message::Status(s).encode_frame(7), Err(MsgError::TooLong));
}

#[test]
fn c3_every_prefix_of_a_frame_is_an_error() {
    let wire = encode_frame(
        0x81,
        3,
        &[1, 0, 2, 3, 0, 2, 1, 2, 0, 2, 0xE8, 3, 0x40, 0x1F],
    )
    .unwrap();
    let enc = &wire[..wire.len() - 1];
    assert!(decode_frame(RxSide::App, enc).is_ok());
    for n in 0..enc.len() {
        assert!(decode_frame(RxSide::App, &enc[..n]).is_err(), "prefix {n}");
    }
}

#[test]
fn c3_unknown_codes_round_trip() {
    for v in 0..=255u8 {
        assert_eq!(NakReason::from_u8(v).to_u8(), v);
        assert_eq!(EventCode::from_u8(v).to_u8(), v);
        assert_eq!(Mode::from_u8(v).to_u8(), v);
        assert_eq!(HidProfile::from_u8(v).to_u8(), v);
    }
    assert_eq!(NakReason::from_u8(0x7E), NakReason::Unknown(0x7E));
    assert_eq!(EventCode::from_u8(0xEE), EventCode::Unknown(0xEE));
    assert!(NakReason::Crc.is_transport() && !NakReason::Crc.is_semantic());
    assert!(NakReason::Aborted.is_semantic() && !NakReason::Aborted.is_transport());
    assert!(NakReason::from_u8(NAK_TRANSPORT_MAX).is_transport());
    assert!(NakReason::from_u8(NAK_SEMANTIC_MIN).is_semantic());
    let zero = NakReason::from_u8(0);
    assert!(!zero.is_transport() && !zero.is_semantic());
    assert!(!NakReason::from_u8(0x80).is_semantic());
}

#[test]
fn c3_unassigned_types() {
    for ty in [0x00u8, 0x07, 0x3F, 0x41, 0x7F, 0x80, 0x82, 0x87, 0xFF] {
        assert_eq!(Message::decode(ty, &[]), Err(MsgError::UnknownType(ty)));
    }
}

#[test]
fn c3_overlong_counted_once() {
    let mut s = Splitter::new(RxSide::App);
    for _ in 0..1000 {
        assert_eq!(s.push(0x11), RxEvent::None);
    }
    assert_eq!(s.stats().overlong, 1);
    assert_eq!(s.push(0), RxEvent::Dropped(FrameError::Overlong));
    assert_eq!(s.stats().overlong, 1);
    let wire = encode_frame(0x84, 1, &[5]).unwrap();
    assert_eq!(
        s.push_slice(&wire),
        vec![Frame {
            ty: 0x84,
            seq: 1,
            body: vec![5]
        }]
    );
}
