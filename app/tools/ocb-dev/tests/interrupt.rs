//! C.16: Ctrl+C (AC3): STOP on the wire, the controller IDLE and neutral,
//! exit 130 — mid-piece and at the Enter prompt.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ocb_core::protocol::Mode;
use ocb_core::protocol::consts::{MSG_START, MSG_STOP};
use ocb_dev::runner::{EXIT_INTERRUPTED, Piece};
use ocb_dev::timeline::parse;
use ocb_sim::ControllerConfig;

use common::SimIo;

fn melody() -> Piece {
    Piece::Melody(parse(&common::melody_text()).unwrap())
}

#[test]
fn ctrl_c_mid_piece_stops_and_goes_neutral() {
    let mut io = SimIo::clean(ControllerConfig::default());
    let flag = Arc::new(AtomicBool::new(false));
    // Enter 1 s after the prompt (about t = 5 s); Ctrl+C at 12 s: the
    // melody (44 s) is playing.
    io.interrupt_at = Some((12_000_000, Arc::clone(&flag)));
    let (out, owner) = common::run(&mut io, melody(), Some(1_000_000), flag);
    assert_eq!(
        out.code,
        EXIT_INTERRUPTED,
        "{}\n{}",
        out.verdict,
        owner.text()
    );
    let start = io.written_of(MSG_START)[0];
    assert!(start < 12_000_000, "playing when interrupted");
    let stops = io.written_of(MSG_STOP);
    assert!(
        stops.iter().any(|&t| t >= 12_000_000),
        "STOP after Ctrl+C: {stops:?}"
    );
    assert_eq!(io.ctrl.mode(), Mode::Idle);
    assert_eq!(io.ctrl.published(), ocb_core::protocol::NEUTRAL, "neutral");
    let applied = io.ctrl.log().applied.len();
    assert!(applied > 0 && applied < 158, "cut mid-piece: {applied}");
    assert!(
        out.verdict
            .contains("STOP ACKed, controller mode Idle (neutral)"),
        "{}",
        out.verdict
    );
}

#[test]
fn ctrl_c_at_the_prompt_stops_too() {
    let mut io = SimIo::clean(ControllerConfig::default());
    let flag = Arc::new(AtomicBool::new(false));
    io.interrupt_at = Some((8_000_000, Arc::clone(&flag)));
    let (out, owner) = common::run(&mut io, melody(), None, flag);
    assert_eq!(
        out.code,
        EXIT_INTERRUPTED,
        "{}\n{}",
        out.verdict,
        owner.text()
    );
    assert!(owner.prompted_at.is_some_and(|p| p < 8_000_000));
    assert!(io.written_of(MSG_START).is_empty(), "never started");
    assert!(io.written_of(MSG_STOP).iter().any(|&t| t >= 8_000_000));
    assert_eq!(io.ctrl.mode(), Mode::Idle, "DIRECT left for IDLE");
    assert_eq!(io.ctrl.published(), ocb_core::protocol::NEUTRAL, "neutral");
    assert!(out.verdict.contains("(neutral)"), "{}", out.verdict);
}

#[test]
fn ctrl_c_before_the_session_opens() {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.down.set_impair(ocb_sim::Impair {
        drop: 1.0,
        ..ocb_sim::Impair::default()
    });
    let flag = Arc::new(AtomicBool::new(false));
    io.interrupt_at = Some((500_000, Arc::clone(&flag)));
    let (out, _) = common::run(&mut io, melody(), None, flag);
    assert_eq!(out.code, EXIT_INTERRUPTED);
    assert!(
        out.verdict.contains("the session was not open"),
        "{}",
        out.verdict
    );
    assert!(
        io.written_of(MSG_STOP).is_empty(),
        "no STOP without a session"
    );
}
