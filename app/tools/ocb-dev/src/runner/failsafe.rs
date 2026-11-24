//! A.3 (Story 3.5 Task 2): the melody round after round; the owner ends
//! each round by unplugging the bridge or pulling the bridge-TX wire. Each
//! round is judged on the controller's own data:
//! - the debug v5 block: the loss's kind (timeout or break) and its age,
//!   from the last heartbeat to the abort and NEUTRAL (≤ 100 ms, AC1);
//! - the watchdog fired: `watchdog_trips` and `abort_count` rose,
//!   `last_event` LINK_LOST;
//! - the piece did not go on: the first STATUS after the loss is IDLE (for
//!   an unplug it follows the new HELLO, so there the v5 numbers carry it).

use ocb_core::link::{AbandonReason, Phase, Session};
use ocb_core::protocol::{EventCode, Mode, State};

use super::{Io, Owner, Runner, Step, Stop};
use crate::tap::DebugCounters;
use crate::timeline::{Timeline, parse};

/// The melody every round plays (the Story 3.4 file, embedded).
const MELODY: &str = include_str!("../../timelines/ode-to-joy-90bpm.csv");

/// The bridge or the wire must come back within this (the owner replugs).
const RECOVER_US: u64 = 120_000_000;
/// A new round starts this long after the controller is ready again.
const ROUND_GAP_US: u64 = 3_000_000;
/// Port reopen attempts while the bridge is unplugged.
const REOPEN_EVERY_US: u64 = 100_000;
/// AC1: the console is neutral within this of the link loss.
const NEUTRAL_LIMIT_10US: u16 = 10_000;

/// How a round ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Loss {
    /// The port failed: the bridge was unplugged.
    Unplug,
    /// The link was lost with the port up: a UART wire was pulled.
    Wire,
}

/// The rounds so far (printed on every ending, Ctrl+C included).
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Tally {
    pub(super) total: u32,
    pub(super) done: u32,
    unplugs: u32,
    wires: u32,
    failed: u32,
    max_age_10us: u16,
}

impl Tally {
    pub(super) fn line(&self) -> String {
        format!(
            "FAILSAFE: {} of {} rounds done, {} unplug, {} wire, {} failed, longest loss to neutral {:.2} ms",
            self.done,
            self.total,
            self.unplugs,
            self.wires,
            self.failed,
            f64::from(self.max_age_10us) / 100.0
        )
    }
}

impl<I: Io, O: Owner> Runner<'_, I, O> {
    pub(super) fn failsafe(&mut self, rounds: u32) -> Step<String> {
        let tl = parse(MELODY).map_err(|e| Stop::Fail(format!("embedded melody: {e}")))?;
        self.rounds.total = rounds;
        if self.tap.last.and_then(|d| d.loss_kind).is_none() {
            return Err(Stop::Fail(
                "failsafe needs the debug counters with the v5 block (fw-debug or fw-debug-o3 from Story 3.5)"
                    .to_owned(),
            ));
        }
        self.first_input()?;
        self.wait_enter()?;
        for n in 1..=rounds {
            if n > 1 {
                self.hold(ROUND_GAP_US)?;
            }
            let (Some(before), Some(st0)) = (self.tap.last, self.status) else {
                return Err(Stop::Fail("no debug STATUS before the round".to_owned()));
            };
            self.owner.say(&format!(
                "ROUND {n}/{rounds}: playing — unplug the bridge or pull the bridge-TX wire now"
            ));
            let line = match self.round_until_loss(&tl)? {
                Ok(loss) => {
                    self.recover(loss)?;
                    let after = self.tap.last;
                    let first = self.first_after_loss;
                    let (text, ok, age) =
                        round_verdict(loss, st0.abort_count, &before, first, after);
                    self.rounds.max_age_10us = self.rounds.max_age_10us.max(age);
                    match loss {
                        Loss::Unplug => self.rounds.unplugs += 1,
                        Loss::Wire => self.rounds.wires += 1,
                    }
                    self.rounds.failed += u32::from(!ok);
                    format!(
                        "ROUND {n}: {} {} {text}",
                        if loss == Loss::Unplug {
                            "unplug"
                        } else {
                            "wire"
                        },
                        if ok { "PASS" } else { "FAIL" }
                    )
                }
                Err(why) => {
                    // The round failed without a loss (the piece ended first):
                    // the run goes on with the next round.
                    self.rounds.failed += 1;
                    self.stop_quietly();
                    self.trouble = None;
                    self.wait_ready()?;
                    format!("ROUND {n}: FAIL {why}")
                }
            };
            self.rounds.done = n;
            self.owner.say(&line);
        }
        if self.rounds.failed > 0 {
            return Err(Stop::Fail(format!(
                "{} of {rounds} rounds failed",
                self.rounds.failed
            )));
        }
        Ok("PASS".to_owned())
    }

    /// Plays the melody until the owner breaks the link. `Ok(Err(why))`: the
    /// round ended without a loss.
    fn round_until_loss(&mut self, tl: &Timeline) -> Step<Result<Loss, String>> {
        self.playback_done = false;
        self.stopped = false;
        self.started = false;
        self.trouble = None;
        self.abandoned = None;
        let mut src = tl.events.clone().into_iter();
        let cap = self.info.map_or(1, |i| usize::from(i.queue_cap).max(1));
        self.feed_round(&mut src, cap)?;
        self.s
            .start(tl.lead_us)
            .map_err(|e| Stop::Fail(format!("START refused locally: {e:?}")))?;
        self.started = true;
        let last = tl.events.last().map_or(0, |e| u64::from(e.0));
        let deadline = self.io.now_us() + u64::from(tl.lead_us) + last + self.lim.tail_us;
        loop {
            self.check_interrupt()?;
            if self.port_lost {
                self.mark_loss();
                return Ok(Ok(Loss::Unplug));
            }
            if self.abandoned == Some(AbandonReason::LinkLost) {
                self.mark_loss();
                return Ok(Ok(Loss::Wire));
            }
            if let Some(t) = self.trouble.take() {
                return Ok(Err(format!("{t} (not a link loss)")));
            }
            if self.playback_done {
                return Ok(Err(
                    "the piece ended before the bridge or the wire was pulled".to_owned(),
                ));
            }
            if self.io.now_us() >= deadline {
                return Ok(Err("no loss and no PLAYBACK_DONE".to_owned()));
            }
            self.feed_round(&mut src, self.lim.backlog)?;
            // A port failure ends the round (it sets `port_lost`); any other
            // error ends the run.
            if let Err(e) = self.pump(deadline)
                && !self.port_lost
            {
                return Err(Stop::Fail(e));
            }
        }
    }

    fn feed_round(
        &mut self,
        src: &mut impl Iterator<Item = (u32, State)>,
        upto: usize,
    ) -> Step<()> {
        if self.port_lost {
            return Ok(());
        }
        self.feed(src, upto)
    }

    fn mark_loss(&mut self) {
        self.first_after_loss = None;
    }

    /// Waits for the bridge (unplug) or the link (wire) to come back, makes
    /// sure the latch is cleared and the console is ready again.
    fn recover(&mut self, loss: Loss) -> Step<()> {
        // The controller is silent until the link is back: no liveness
        // check meanwhile.
        self.liveness = false;
        let r = self.recover_inner(loss);
        self.liveness = true;
        r?;
        self.trouble = None;
        self.wait_ready()
    }

    fn recover_inner(&mut self, loss: Loss) -> Step<()> {
        let deadline = self.io.now_us() + RECOVER_US;
        match loss {
            Loss::Unplug => {
                self.owner.say("bridge unplugged: plug it back in");
                // A replug can bounce: the port may vanish again during the
                // new session's open; go back to waiting for it then.
                loop {
                    self.reopen_until(deadline)?;
                    self.port_lost = false;
                    // A new bridge boot: a new session (HELLO also clears the latch).
                    self.s = Session::new(self.cfg);
                    self.tap.reset_splitter();
                    self.info = None;
                    self.status = None;
                    self.status_at = None;
                    self.ready_since = None;
                    self.first_after_loss = None;
                    match self.connect() {
                        Ok(()) => break,
                        Err(Stop::Fail(_)) if self.port_lost => {}
                        Err(e) => return Err(e),
                    }
                }
            }
            Loss::Wire => {
                self.owner.say("link lost: plug the wire back in");
                let ok = self.wait(deadline, |r| {
                    // A STATUS after the loss, and the latest one with the link up.
                    r.first_after_loss.is_some()
                        && r.status.is_some_and(|s| s.link_ok)
                        && r.s.phase() != Phase::Stopping
                })?;
                if !ok {
                    return Err(Stop::Fail(
                        "the link did not come back within 120 s".to_owned(),
                    ));
                }
                // The session stopped after LINK_LOST; STOP again in case the
                // latch was set by a replug bounce after that STOP.
                self.stopped = false;
                if self.s.phase() == Phase::Connected && self.s.stop().is_ok() {
                    let end = self.io.now_us() + self.lim.stop_wait_us;
                    if !self.wait(end, |r| r.stopped)? {
                        return Err(Stop::Fail(
                            "STOP not acknowledged after the replug".to_owned(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn reopen_until(&mut self, deadline: u64) -> Step<()> {
        loop {
            self.check_interrupt()?;
            if self.io.reopen().map_err(Stop::Fail)? {
                return Ok(());
            }
            let now = self.io.now_us();
            if now >= deadline {
                return Err(Stop::Fail(
                    "the bridge did not come back within 120 s".to_owned(),
                ));
            }
            self.io.idle_until(now + REOPEN_EVERY_US);
        }
    }
}

/// One round's verdict from the controller's own data: (line, PASS, the
/// loss-to-neutral age in 10 µs units).
fn round_verdict(
    loss: Loss,
    abort0: u8,
    before: &DebugCounters,
    first: Option<ocb_core::link::StatusView>,
    after: Option<DebugCounters>,
) -> (String, bool, u16) {
    let mut ok = true;
    let mut parts = Vec::new();
    let mut age = 0u16;
    match after {
        Some(d) => {
            let trips = d.watchdog_trips.wrapping_sub(before.watchdog_trips);
            let be = d.rx_be.unwrap_or(0).wrapping_sub(before.rx_be.unwrap_or(0));
            age = d.loss_age_10us.unwrap_or(u16::MAX);
            let kind = d.loss_kind.unwrap_or(0);
            ok &= trips >= 1 && kind != 0 && age <= NEUTRAL_LIMIT_10US;
            if loss == Loss::Wire {
                ok &= be >= 1;
            }
            parts.push(format!(
                "loss {} → neutral within {:.2} ms (limit 100); watchdog_trips +{trips}, rx_be +{be}",
                match kind {
                    1 => "heartbeat timeout",
                    2 => "break",
                    _ => "none recorded",
                },
                f64::from(age) / 100.0
            ));
        }
        None => {
            ok = false;
            parts.push("no debug counters after the loss".to_owned());
        }
    }
    match first {
        Some(st) => {
            let lost = st.last_event == EventCode::LinkLost;
            let aborts = st.abort_count.wrapping_sub(abort0);
            ok &= lost && aborts >= 1;
            if loss == Loss::Wire {
                ok &= st.mode == Mode::Idle;
            }
            parts.push(format!(
                "first STATUS after: mode {:?}, last_event {:?}, abort_count +{aborts}",
                st.mode, st.last_event
            ));
        }
        None => {
            ok = false;
            parts.push("no STATUS after the loss".to_owned());
        }
    }
    (parts.join("; "), ok, age)
}
