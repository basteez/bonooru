//! C.19: the `watch` dry run (Story 3.5 AC3, H.3): a scripted console —
//! handshake, Wired Communication OFF and ON, sleep (power cut, reboot),
//! wake — and the verdicts.
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
use ocb_core::protocol::consts::MSG_START;
use ocb_dev::runner::{EXIT_FAIL, EXIT_PASS, Outcome, Piece};
use ocb_sim::ControllerConfig;

use common::{ScriptedOwner, SimIo};

const WATCH: Piece = Piece::Watch { max_us: 60_000_000 };

/// Not ready at first; handshake at 1 s; OFF at 5 s, ON at 9 s; sleep from
/// 11 s to 14 s (the controller reboots, ready again 1.5 s later); Ctrl+C
/// at 19 s.
fn h3() -> SimIo {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.set_debug(Some(common::debug_tail_v4(9_000, 3)));
    io.ctrl.set_console_ready(false);
    io.hooks
        .push((1_000_000, Box::new(|c| c.set_console_ready(true))));
    io.hooks
        .push((5_000_000, Box::new(|c| c.set_console_ready(false))));
    io.hooks
        .push((9_000_000, Box::new(|c| c.set_console_ready(true))));
    io.sleep = Some((11_000_000, 14_000_000));
    io
}

fn run(io: &mut SimIo, ctrl_c_at: u64) -> (Outcome, ScriptedOwner) {
    let flag = Arc::new(AtomicBool::new(false));
    io.interrupt_at = Some((ctrl_c_at, Arc::clone(&flag)));
    common::run(io, WATCH, None, flag)
}

#[test]
fn a_scripted_h3_passes() {
    let mut io = h3();
    let (out, owner) = run(&mut io, 19_000_000);
    let text = owner.text();
    assert_eq!(out.code, EXIT_PASS, "{}\n{text}", out.verdict);
    assert!(
        text.contains("H.3: first STATUS after INFO: console_ready 0"),
        "{text}"
    );
    assert!(text.contains("console_ready 0 -> 1 edges: 3"), "{text}");
    assert!(
        text.contains("console_ready 1 -> 0 with STATUS flowing: 1"),
        "{text}"
    );
    assert!(text.contains("STATUS silences: 1 ("), "{text}");
    assert!(text.contains("reboot seen, ready after yes"), "{text}");
    assert!(text.contains("controller rebooted (session): 1"), "{text}");
    assert!(text.contains("in_max_us 9000, in_slow 3"), "{text}");
    let probes: Vec<&str> = text
        .lines()
        .filter(|l| l.contains("probe answered NAK NOT_READY"))
        .collect();
    assert!(probes.len() >= 2, "{text}");
    assert!(text.contains("NOT_READY probes: "), "{text}");
    assert!(text.contains("other: none"), "{text}");
}

#[test]
fn a_slow_fall_fails() {
    let mut io = h3();
    // The STATUS frames around OFF are lost for about 200 ms: the fall shows late (but STATUS is not silent).
    io.down_gap = Some((4_995_000, 5_200_000));
    let (out, owner) = run(&mut io, 19_000_000);
    assert_eq!(out.code, EXIT_FAIL, "{}", owner.text());
    assert!(out.verdict.contains("1 -> 0 edge took"), "{}", out.verdict);
}

#[test]
fn a_probe_answered_otherwise_fails() {
    let mut io = h3();
    io.hooks.push((
        5_500_000,
        Box::new(|c| c.inject_nak(MSG_START, NakReason::Busy)),
    ));
    let (out, owner) = run(&mut io, 19_000_000);
    assert_eq!(out.code, EXIT_FAIL, "{}", owner.text());
    assert!(out.verdict.contains("probes:"), "{}", out.verdict);
}

#[test]
fn no_sleep_fails() {
    let mut io = h3();
    io.sleep = None;
    let (out, _) = run(&mut io, 19_000_000);
    assert_eq!(out.code, EXIT_FAIL);
    assert!(out.verdict.contains("no STATUS silence"), "{}", out.verdict);
}

#[test]
fn a_reboot_the_session_never_sees_fails() {
    // After the sleep the controller stays not ready and the link down, so
    // no probe can run and the session cannot learn of the reboot.
    let mut io = h3();
    io.hooks
        .push((14_000_500, Box::new(|c| c.set_link_ok(false))));
    let (out, owner) = run(&mut io, 19_000_000);
    assert_eq!(out.code, EXIT_FAIL, "{}", owner.text());
    assert!(
        out.verdict.contains("without a reported reboot"),
        "{}",
        out.verdict
    );
}
