//! C.18: the `failsafe` dry run (Story 3.5 AC1, A.3): rounds ended by a
//! bridge unplug (the port fails) or a wire pull (LINK_LOST, port up).
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
use ocb_core::protocol::consts::MSG_STOP;
use ocb_dev::runner::{EXIT_FAIL, EXIT_INTERRUPTED, EXIT_PASS, Piece};
use ocb_sim::ControllerConfig;

use common::{Loss, SimIo};

fn rig(plan: Vec<Loss>) -> SimIo {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.set_debug(Some(common::debug_tail(1)));
    io.loss_plan = plan;
    io
}

#[test]
fn unplug_and_wire_rounds_pass() {
    let mut io = rig(vec![
        Loss::Unplug,
        Loss::Wire,
        Loss::Unplug,
        Loss::Wire,
        Loss::Unplug,
        Loss::Wire,
    ]);
    let (out, owner) = common::run(
        &mut io,
        Piece::Failsafe { rounds: 6 },
        Some(1_000_000),
        Arc::new(AtomicBool::new(false)),
    );
    let text = owner.text();
    assert_eq!(out.code, EXIT_PASS, "{}\n{text}", out.verdict);
    for (n, kind) in [
        (1, "unplug"),
        (2, "wire"),
        (3, "unplug"),
        (4, "wire"),
        (5, "unplug"),
        (6, "wire"),
    ] {
        assert!(
            text.contains(&format!("ROUND {n}: {kind} PASS")),
            "round {n}:\n{text}"
        );
    }
    assert!(
        text.contains(
            "FAILSAFE: 6 of 6 rounds done, 3 unplug, 3 wire, 0 failed, longest loss to neutral 5.00 ms"
        ),
        "{text}"
    );
    assert!(
        text.contains(
            "loss break → neutral within 5.00 ms (limit 100); watchdog_trips +1, rx_be +1"
        ),
        "{text}"
    );
    assert!(
        text.contains("last_event LinkLost, abort_count +1"),
        "{text}"
    );
    assert!(text.contains("watchdog_trips +1, rx_be +1"), "{text}");
    assert_eq!(
        owner
            .lines
            .iter()
            .filter(|l| l.starts_with("PROMPT"))
            .count(),
        1,
        "one prompt"
    );
    assert_eq!(io.ctrl.mode(), Mode::Idle);
}

#[test]
fn a_loss_the_controller_does_not_notice_fails_the_round() {
    let mut io = rig(vec![Loss::UnplugUnnoticed]);
    let (out, owner) = common::run(
        &mut io,
        Piece::Failsafe { rounds: 1 },
        Some(1_000_000),
        Arc::new(AtomicBool::new(false)),
    );
    let text = owner.text();
    assert_eq!(out.code, EXIT_FAIL, "{text}");
    assert!(text.contains("ROUND 1: unplug FAIL"), "{text}");
    assert!(
        out.verdict.contains("1 of 1 rounds failed"),
        "{}",
        out.verdict
    );
}

#[test]
fn a_piece_that_ends_before_the_loss_fails_that_round_only() {
    let mut io = rig(vec![Loss::Wire]);
    io.loss_skip = 1;
    let (out, owner) = common::run(
        &mut io,
        Piece::Failsafe { rounds: 2 },
        Some(1_000_000),
        Arc::new(AtomicBool::new(false)),
    );
    let text = owner.text();
    assert_eq!(out.code, EXIT_FAIL, "{text}");
    assert!(
        text.contains("ROUND 1: FAIL the piece ended before"),
        "{text}"
    );
    assert!(
        text.contains("ROUND 2: wire PASS"),
        "the run goes on: {text}"
    );
    assert!(
        out.verdict.contains("1 of 2 rounds failed"),
        "{}",
        out.verdict
    );
}

#[test]
fn a_neutral_later_than_100_ms_fails_the_round() {
    let mut io = rig(vec![Loss::Wire]);
    io.ctrl.set_loss_age_10us(10_001);
    let (out, owner) = common::run(
        &mut io,
        Piece::Failsafe { rounds: 1 },
        Some(1_000_000),
        Arc::new(AtomicBool::new(false)),
    );
    let text = owner.text();
    assert_eq!(out.code, EXIT_FAIL, "{text}");
    assert!(
        text.contains("ROUND 1: wire FAIL loss break → neutral within 100.01 ms"),
        "{text}"
    );
}

#[test]
fn failsafe_needs_debug_counters() {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.loss_plan = vec![Loss::Wire];
    let (out, _) = common::run(
        &mut io,
        Piece::Failsafe { rounds: 1 },
        Some(1_000_000),
        Arc::new(AtomicBool::new(false)),
    );
    assert_eq!(out.code, EXIT_FAIL);
    assert!(
        out.verdict
            .contains("needs the debug counters with the v5 block"),
        "{}",
        out.verdict
    );
}

#[test]
fn ctrl_c_mid_run_stops() {
    let mut io = rig(vec![Loss::Wire, Loss::Wire, Loss::Wire]);
    let flag = Arc::new(AtomicBool::new(false));
    io.interrupt_at = Some((16_000_000, Arc::clone(&flag)));
    let (out, owner) = common::run(
        &mut io,
        Piece::Failsafe { rounds: 3 },
        Some(1_000_000),
        flag,
    );
    assert_eq!(
        out.code,
        EXIT_INTERRUPTED,
        "{}\n{}",
        out.verdict,
        owner.text()
    );
    assert!(io.written_of(MSG_STOP).iter().any(|&t| t >= 16_000_000));
    assert!(
        owner.text().contains("FAILSAFE: "),
        "the tally on Ctrl+C: {}",
        owner.text()
    );
    assert_eq!(io.ctrl.mode(), Mode::Idle);
}
