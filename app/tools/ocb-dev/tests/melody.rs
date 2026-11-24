//! C.15: the melody dry run (AC1, AC2) and the refusals before START (8.6).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ocb_core::protocol::consts::{
    HAT_DOWN, MSG_PING, MSG_QUEUE_EVENT, MSG_SET_STATE, MSG_START, MSG_STOP,
};
use ocb_core::protocol::{Mode, NEUTRAL};
use ocb_dev::runner::{EXIT_FAIL, EXIT_PASS, Piece};
use ocb_dev::timeline::parse;
use ocb_sim::ControllerConfig;

use common::SimIo;

fn melody() -> Piece {
    Piece::Melody(parse(&common::melody_text()).unwrap())
}

fn plays(io: &mut SimIo, clean: bool) {
    let tl = parse(&common::melody_text()).unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let (out, owner) = common::run(io, melody(), Some(5_000_000), flag);
    assert_eq!(out.code, EXIT_PASS, "{}\n{}", out.verdict, owner.text());
    assert_eq!(out.verdict, "PASS");
    // AC2: three D-pad-down presses in DIRECT, each released, never A.
    let direct = &io.ctrl.log().direct;
    let down = NEUTRAL_WITH_HAT_DOWN;
    assert_eq!(
        direct.as_slice(),
        &[down, NEUTRAL, down, NEUTRAL, down, NEUTRAL],
        "first input"
    );
    assert!(
        direct.iter().all(|s| s.buttons == 0),
        "never a button (never A)"
    );
    // The prompt, then 5 s of waiting kept alive by PINGs, no app timeout.
    let prompt = owner.prompted_at.expect("the Enter prompt");
    let pings: Vec<u64> = io
        .written_of(MSG_PING)
        .into_iter()
        .filter(|&t| t > prompt && t < prompt + 5_000_000)
        .collect();
    assert!(pings.len() >= 15, "PINGs while waiting: {}", pings.len());
    assert_eq!(io.ctrl.log().app_timeouts, 0);
    // START after Enter, with the profile's lead; every event applied in order.
    let start = io.written_of(MSG_START);
    // A lossy channel resends START (go-back-N); a clean one sends it once.
    assert!(!start.is_empty());
    if clean {
        assert_eq!(start.len(), 1, "one START");
    }
    assert!(start[0] >= prompt + 5_000_000, "START only after Enter");
    assert!(io.written_of(MSG_SET_STATE).iter().all(|&t| t < start[0]));
    assert_eq!(
        io.ctrl.log().starts,
        vec![tl.lead_us],
        "START with the profile's lead"
    );
    assert_eq!(
        io.ctrl.log().applied,
        tl.events,
        "every event applied, in order"
    );
    assert_eq!(io.ctrl.mode(), Mode::Idle, "STOP after PLAYBACK_DONE");
    assert!(!io.written_of(MSG_STOP).is_empty());
    assert!(owner.text().contains("PLAYBACK_DONE: 158 events"));
    assert!(owner.text().contains("report_period_us 8000 (the console)"));
}

const NEUTRAL_WITH_HAT_DOWN: ocb_core::protocol::State = ocb_core::protocol::State {
    hat: HAT_DOWN,
    ..NEUTRAL
};

#[test]
fn melody_plays_clean() {
    plays(&mut SimIo::clean(ControllerConfig::default()), true);
}

#[test]
fn melody_plays_through_an_impaired_channel() {
    for seed in [1, 2, 3] {
        let mut io = SimIo::lossy(ControllerConfig::default(), 0.02, seed);
        plays(&mut io, false);
    }
}

#[test]
fn prefill_respects_queue_cap_and_streams_on_credit() {
    // A queue of 16: the prefill is 16 events, the rest streams after START.
    let mut io = SimIo::clean(ControllerConfig {
        queue_cap: 16,
        ..ControllerConfig::default()
    });
    let flag = Arc::new(AtomicBool::new(false));
    let (out, owner) = common::run(&mut io, melody(), Some(1_000_000), flag);
    assert_eq!(out.code, EXIT_PASS, "{}\n{}", out.verdict, owner.text());
    let start = io.written_of(MSG_START)[0];
    let before = io
        .written_of(MSG_QUEUE_EVENT)
        .iter()
        .filter(|&&t| t <= start)
        .count();
    assert_eq!(
        before, 16,
        "the prefill fills the queue of 16, START follows"
    );
    assert_eq!(io.ctrl.log().applied.len(), 158);
}

#[test]
fn an_enter_before_the_prompt_does_not_start_the_piece() {
    let mut io = SimIo::clean(ControllerConfig::default());
    // Enter pressed while the D-pad presses run (before the prompt), then
    // the real one 2 s after the prompt.
    let (out, owner) = common::run_with(
        &mut io,
        melody(),
        Some(2_000_000),
        vec![2_500_000],
        Arc::new(AtomicBool::new(false)),
    );
    assert_eq!(out.code, EXIT_PASS, "{}\n{}", out.verdict, owner.text());
    let prompt = owner.prompted_at.unwrap();
    assert!(prompt > 2_500_000, "the early Enter came before the prompt");
    let start = io.written_of(MSG_START)[0];
    assert!(
        start >= prompt + 2_000_000,
        "START at {start}, prompt at {prompt}"
    );
}

#[test]
fn a_controller_that_goes_silent_fails_the_ready_wait() {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.silence_at = Some(500_000);
    let (out, _) = common::run(&mut io, melody(), Some(0), Arc::new(AtomicBool::new(false)));
    assert_eq!(out.code, EXIT_FAIL);
    assert!(out.verdict.contains("stopped answering"), "{}", out.verdict);
    assert!(
        io.written_of(MSG_SET_STATE).is_empty(),
        "no input to a silent controller"
    );
}

#[test]
fn a_controller_that_goes_silent_mid_piece_fails_fast() {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.silence_at = Some(10_000_000);
    let (out, _) = common::run(
        &mut io,
        melody(),
        Some(1_000_000),
        Arc::new(AtomicBool::new(false)),
    );
    assert_eq!(out.code, EXIT_FAIL);
    assert!(out.verdict.contains("stopped answering"), "{}", out.verdict);
    assert!(
        io.now.get() < 13_100_000,
        "1 s of silence plus the 2 s STOP wait, not the whole piece"
    );
}

#[test]
fn the_timeline_build_is_refused_before_any_event() {
    let mut io = SimIo::clean(ControllerConfig::default());
    let tl = parse(&common::melody_text()).unwrap();
    io.ctrl.set_table_on_sync(Some(tl.events));
    let (out, _) = common::run(&mut io, melody(), Some(0), Arc::new(AtomicBool::new(false)));
    assert_eq!(out.code, EXIT_FAIL);
    assert!(out.verdict.contains("built-in timeline"), "{}", out.verdict);
    assert!(io.written_of(MSG_QUEUE_EVENT).is_empty());
    assert!(io.written_of(MSG_SET_STATE).is_empty());
}

#[test]
fn a_refusal_mid_piece_stops_and_clears_the_latch() {
    let mut io = SimIo::clean(ControllerConfig {
        queue_cap: 16,
        ..ControllerConfig::default()
    });
    io.hooks.push((
        12_000_000,
        Box::new(|c| c.inject_nak(MSG_QUEUE_EVENT, ocb_core::protocol::NakReason::Order)),
    ));
    let (out, owner) = common::run(
        &mut io,
        melody(),
        Some(1_000_000),
        Arc::new(AtomicBool::new(false)),
    );
    assert_eq!(out.code, EXIT_FAIL, "{}", owner.text());
    assert!(out.verdict.contains("Nak(Order)"), "{}", out.verdict);
    assert_eq!(io.ctrl.mode(), Mode::Idle);
    assert!(
        !io.ctrl.aborted(),
        "the STOP after the abandon was ACKed (latch clear)"
    );
    assert_eq!(io.ctrl.published(), NEUTRAL);
    assert!(
        owner.text().contains("SUMMARY: "),
        "a summary on failure too"
    );
}

#[test]
fn no_info_fails_within_5_s() {
    let mut io = SimIo::clean(ControllerConfig {
        proto_ver: 1,
        ..ControllerConfig::default()
    });
    // Drop everything from the controller: no INFO ever.
    io.down.set_impair(ocb_sim::Impair {
        drop: 1.0,
        ..ocb_sim::Impair::default()
    });
    let t0 = io.now.get();
    let (out, _) = common::run(&mut io, melody(), Some(0), Arc::new(AtomicBool::new(false)));
    assert_eq!(out.code, EXIT_FAIL);
    assert!(
        out.verdict.contains("no INFO within 5 s"),
        "{}",
        out.verdict
    );
    assert!(io.now.get() - t0 <= 5_100_000);
}

#[test]
fn console_never_ready_fails_after_120_s() {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.ctrl.set_console_ready(false);
    let (out, owner) = common::run(&mut io, melody(), Some(0), Arc::new(AtomicBool::new(false)));
    assert_eq!(out.code, EXIT_FAIL);
    assert!(
        out.verdict.contains("console_ready never held 1"),
        "{}",
        out.verdict
    );
    assert!(owner.text().contains("WAITING for console_ready"));
    assert!(
        io.written_of(MSG_SET_STATE).is_empty(),
        "no input before ready"
    );
}

#[test]
fn no_link_fails_with_a_hint() {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.ctrl.set_link_ok(false);
    let (out, _) = common::run(&mut io, melody(), Some(0), Arc::new(AtomicBool::new(false)));
    assert_eq!(out.code, EXIT_FAIL);
    assert!(out.verdict.contains("link_ok 0"), "{}", out.verdict);
}

#[test]
fn a_bounce_restarts_the_2_s_ready_window() {
    let mut io = SimIo::clean(ControllerConfig::default());
    io.hooks
        .push((1_500_000, Box::new(|c| c.set_console_ready(false))));
    io.hooks
        .push((1_600_000, Box::new(|c| c.set_console_ready(true))));
    let (out, owner) = common::run(&mut io, melody(), Some(0), Arc::new(AtomicBool::new(false)));
    assert_eq!(out.code, EXIT_PASS, "{}\n{}", out.verdict, owner.text());
    let first_set = io.written_of(MSG_SET_STATE)[0];
    assert!(
        first_set >= 3_600_000,
        "first input at {first_set}, before the 2 s after the bounce"
    );
}
