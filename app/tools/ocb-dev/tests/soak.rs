//! C.17: the soak dry run (AC4): the same code path as the 10-minute bench
//! soak, 20 s long here.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ocb_core::protocol::NakReason;
use ocb_core::protocol::consts::{MSG_QUEUE_EVENT, MSG_SET_STATE};
use ocb_dev::runner::{EXIT_FAIL, EXIT_PASS, Outcome, Piece};
use ocb_dev::soak::{SOAK_DURATION_US, SOAK_NUDGE_LX, SOAK_PERIOD_US, Soak};
use ocb_dev::timeline::{allowed, neutral_end};
use ocb_sim::ControllerConfig;

use common::{ScriptedOwner, SimIo};

const SHORT: Piece = Piece::Soak {
    duration_us: 20_000_000,
    period_us: 1_000,
    lead_us: 500_000,
};

fn soak(io: &mut SimIo) -> (Outcome, ScriptedOwner) {
    common::run(io, SHORT, None, Arc::new(AtomicBool::new(false)))
}

#[test]
fn the_bench_soak_is_ten_minutes_of_allowed_states() {
    let s = Soak::new(SOAK_DURATION_US, SOAK_PERIOD_US);
    assert_eq!(s.len(), 600_000);
    assert!(s.last_t_us() < 1 << 31);
    let mut n = 0u32;
    let mut prev = None;
    for (t, st) in s {
        assert!(allowed(&st));
        assert!(prev.is_none_or(|p| t > p));
        assert!(st.lx == 0x80 || st.lx == SOAK_NUDGE_LX);
        prev = Some(t);
        n += 1;
        if n == 600_000 {
            assert_eq!(st, neutral_end());
        } else {
            assert!(!st.has_end());
        }
    }
    assert_eq!(n, 600_000);
}

#[test]
fn soak_passes_on_clean_counters() {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.ctrl.set_debug_extra(Some(common::debug_tail(1)));
    let (out, owner) = soak(&mut io);
    assert_eq!(out.code, EXIT_PASS, "{}\n{}", out.verdict, owner.text());
    assert_eq!(io.ctrl.log().applied.len(), 20_000);
    assert_eq!(io.ctrl.log().starts, vec![500_000], "the soak's lead");
    assert!(
        io.written_of(MSG_SET_STATE).is_empty(),
        "no first input in soak mode"
    );
    assert!(owner.prompted_at.is_none(), "no Enter prompt in soak mode");
    let text = owner.text();
    assert!(text.contains("SOAK: "), "{text}");
    assert!(text.contains("transport NAKs 0"), "{text}");
    assert!(
        text.contains("debug first: max_app_gap_ms=0 watchdog_trips=0 dropped_bytes=1"),
        "{text}"
    );
    // Bounded backlog: the prefill (queue_cap) then at most `backlog`.
    assert!(out.max_unsent <= 512, "max unsent {}", out.max_unsent);
    assert_eq!(out.queued, 20_000);
}

#[test]
fn soak_fails_when_bytes_are_dropped() {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.ctrl.set_debug_extra(Some(common::debug_tail(1)));
    io.hooks.push((
        10_000_000,
        Box::new(|c| c.set_debug_extra(Some(common::debug_tail(2)))),
    ));
    let (out, _) = soak(&mut io);
    assert_eq!(out.code, EXIT_FAIL);
    assert!(
        out.verdict.contains("counters moved: dropped_bytes"),
        "{}",
        out.verdict
    );
    assert!(out.verdict.contains("rx_err_bytes"), "{}", out.verdict);
}

#[test]
fn soak_needs_debug_counters() {
    let mut io = SimIo::clean(ControllerConfig::default());
    let (out, _) = soak(&mut io);
    assert_eq!(out.code, EXIT_FAIL);
    assert!(
        out.verdict.contains("needs a debug controller build"),
        "{}",
        out.verdict
    );
    let mut io = SimIo::clean(ControllerConfig::default());
    io.ctrl.set_debug_extra(Some(vec![0; 7]));
    let (out, _) = soak(&mut io);
    assert!(out.verdict.contains("v2 debug counters"), "{}", out.verdict);
}

#[test]
fn soak_refuses_the_timeline_build() {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.ctrl.set_debug_extra(Some(common::debug_tail(1)));
    io.ctrl.inject_nak(MSG_QUEUE_EVENT, NakReason::BadState);
    let (out, _) = soak(&mut io);
    assert_eq!(out.code, EXIT_FAIL);
    assert!(out.verdict.contains("built-in timeline"), "{}", out.verdict);
}

#[test]
fn soak_fails_on_transport_naks() {
    let mut io = SimIo::lossy(ControllerConfig::default(), 0.01, 7);
    io.ctrl.set_debug_extra(Some(common::debug_tail(1)));
    let (out, owner) = soak(&mut io);
    assert_eq!(out.code, EXIT_FAIL, "{}", owner.text());
    assert!(
        out.verdict.contains("transport NAKs during the soak"),
        "{}",
        out.verdict
    );
    assert_eq!(
        io.ctrl.log().applied.len(),
        20_000,
        "the stream itself recovered"
    );
}
