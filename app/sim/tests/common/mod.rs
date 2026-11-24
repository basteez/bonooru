//! Helpers shared by the session tests.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use ocb_core::link::{LinkEvent, Phase, SessionConfig};
use ocb_core::protocol::consts::{BTN_A, BTN_L, BTN_R, BTN_X, BTN_Y, STATE_FLAG_END};
use ocb_core::protocol::{NEUTRAL, State};
use ocb_sim::{Channel, ControllerConfig, Impair, Sim};

/// A note state from a small index (buttons the ocarina uses).
pub fn note(i: usize) -> State {
    let b = [BTN_A, BTN_L, BTN_R, BTN_X, BTN_Y];
    State {
        buttons: b[i % b.len()],
        ..NEUTRAL
    }
}

/// `NEUTRAL | END`.
pub fn end() -> State {
    State {
        flags: STATE_FLAG_END,
        ..NEUTRAL
    }
}

/// A connected simulation (clean channels) with this controller.
pub fn connected(ctrl: ControllerConfig) -> Sim {
    let mut s = Sim::new(SessionConfig::default(), ctrl);
    assert!(s.connect(), "no connection: {:?}", s.events());
    s
}

/// A connected simulation whose channels get these impairments after the
/// connection.
pub fn connected_lossy(ctrl: ControllerConfig, impair: Impair, seed: u64) -> Sim {
    let mut s = Sim::with_channels(
        SessionConfig::default(),
        ctrl,
        Channel::new(Impair::default(), 100, seed),
        Channel::new(
            Impair::default(),
            100,
            seed.wrapping_mul(31).wrapping_add(7),
        ),
    );
    assert!(s.connect(), "no connection");
    s.up.set_impair(impair);
    s.down.set_impair(impair);
    s
}

/// Runs until the session reports an event matching `pred`.
pub fn wait_event(s: &mut Sim, max_us: u64, pred: impl Fn(&LinkEvent) -> bool) -> bool {
    let start = s.seen.events.len();
    s.run_until(max_us, |s| {
        s.seen.events[start..].iter().any(|(_, e)| pred(e))
    })
}

/// Runs until the session is connected and idle (after a STOP).
pub fn wait_idle(s: &mut Sim, max_us: u64) -> bool {
    s.run_until(max_us, |s| s.session.phase() == Phase::Connected)
}

/// No invariant was violated.
pub fn assert_clean(s: &Sim) {
    assert!(
        s.seen.violations.is_empty(),
        "violations: {:#?}",
        s.seen.violations
    );
}

/// Replay one seed with OCB_SIM_SEED, else the given seeds.
pub fn seeds(default: &[u64]) -> Vec<u64> {
    match std::env::var("OCB_SIM_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        Some(s) => vec![s],
        None => default.to_vec(),
    }
}
