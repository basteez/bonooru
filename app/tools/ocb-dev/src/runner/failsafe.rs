//! A.3 (Story 3.5 Task 2): the melody round after round; the owner ends
//! each round by unplugging the bridge or pulling the bridge-TX wire. Each
//! round is judged on the controller's own data: the link watchdog fired
//! (LINK_LOST, `watchdog_trips` + 1, `abort_count` + 1) and the piece was
//! not still playing when the link came back (mode IDLE).

use ocb_core::link::{Phase, Session};
use ocb_core::protocol::{EventCode, Mode, State};

use super::{Io, Owner, Runner, Step, Stop};
use crate::tap::DebugCounters;
use crate::timeline::{Timeline, parse};

/// The melody every round plays (the Story 3.4 file, embedded).
const MELODY: &str = include_str!("../../timelines/ode-to-joy-90bpm.csv");

/// The bridge must come back within this (the owner replugs).
const RECOVER_US: u64 = 120_000_000;
/// A new round starts this long after the controller is ready again.
const ROUND_GAP_US: u64 = 3_000_000;
/// Port reopen attempts while the bridge is unplugged.
const REOPEN_EVERY_US: u64 = 100_000;

/// How a round ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Loss {
    /// The port failed: the bridge was unplugged.
    Unplug,
    /// The link was lost with the port up: a UART wire was pulled.
    Wire,
}

impl<I: Io, O: Owner> Runner<'_, I, O> {
    pub(super) fn failsafe(&mut self, rounds: u32) -> Step<String> {
        let tl = parse(MELODY).map_err(|e| Stop::Fail(format!("embedded melody: {e}")))?;
        self.first_input()?;
        self.wait_enter()?;
        let (mut unplugs, mut wires, mut failed) = (0u32, 0u32, 0u32);
        for n in 1..=rounds {
            if n > 1 {
                self.hold(ROUND_GAP_US)?;
            }
            let Some(before) = self.tap.last.filter(DebugCounters::has_v2) else {
                return Err(Stop::Fail(
                    "failsafe needs the debug counters (fw-debug or fw-debug-o3)".to_owned(),
                ));
            };
            let Some(st0) = self.status else {
                return Err(Stop::Fail("no STATUS before the round".to_owned()));
            };
            self.owner.say(&format!(
                "ROUND {n}/{rounds}: playing — unplug the bridge or pull the bridge-TX wire now"
            ));
            let loss = self.round_until_loss(&tl)?;
            self.recover(loss)?;
            let after = self.tap.last.filter(DebugCounters::has_v2);
            let first = self.first_after_loss;
            let (line, ok) = round_verdict(loss, st0.abort_count, &before, first, after);
            failed += u32::from(!ok);
            match loss {
                Loss::Unplug => unplugs += 1,
                Loss::Wire => wires += 1,
            }
            self.owner.say(&format!(
                "ROUND {n}: {} {} {line}",
                match loss {
                    Loss::Unplug => "unplug",
                    Loss::Wire => "wire",
                },
                if ok { "PASS" } else { "FAIL" }
            ));
        }
        self.owner.say(&format!(
            "FAILSAFE: {rounds} rounds, {unplugs} unplug, {wires} wire, {failed} failed"
        ));
        if failed > 0 {
            return Err(Stop::Fail(format!("{failed} of {rounds} rounds failed")));
        }
        Ok("PASS".to_owned())
    }

    /// Plays the melody until the owner breaks the link.
    fn round_until_loss(&mut self, tl: &Timeline) -> Step<Loss> {
        self.playback_done = false;
        self.stopped = false;
        self.started = false;
        self.trouble = None;
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
                return Ok(Loss::Unplug);
            }
            if let Some(t) = self.trouble.take() {
                if t.contains("LinkLost") {
                    self.mark_loss();
                    return Ok(Loss::Wire);
                }
                return Err(Stop::Fail(format!("round: {t}")));
            }
            if self.playback_done {
                return Err(Stop::Fail(
                    "the piece ended before the bridge or the wire was pulled".to_owned(),
                ));
            }
            if self.io.now_us() >= deadline {
                return Err(Stop::Fail("no loss and no PLAYBACK_DONE".to_owned()));
            }
            self.feed_round(&mut src, self.lim.backlog)?;
            // A port failure is the round's end, not a tool failure.
            let _ = self.pump(deadline);
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
                loop {
                    self.check_interrupt()?;
                    if self.io.reopen().map_err(Stop::Fail)? {
                        break;
                    }
                    let now = self.io.now_us();
                    if now >= deadline {
                        return Err(Stop::Fail(
                            "the bridge did not come back within 120 s".to_owned(),
                        ));
                    }
                    self.io.idle_until(now + REOPEN_EVERY_US);
                }
                self.port_lost = false;
                // A new bridge boot: a new session (HELLO also clears the latch).
                self.s = Session::new(self.cfg);
                self.tap.reset_splitter();
                self.info = None;
                self.status = None;
                self.status_at = None;
                self.ready_since = None;
                self.first_after_loss = None;
                self.connect()?;
            }
            Loss::Wire => {
                self.owner.say("link lost: plug the wire back in");
                let ok = self.wait(deadline, |r| {
                    r.status.is_some_and(|s| s.link_ok) && r.s.phase() != Phase::Stopping
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
}

/// One round's verdict line from the controller's own data.
fn round_verdict(
    loss: Loss,
    abort0: u8,
    before: &DebugCounters,
    first: Option<ocb_core::link::StatusView>,
    after: Option<DebugCounters>,
) -> (String, bool) {
    let mut ok = true;
    let mut parts = Vec::new();
    match first {
        Some(st) => {
            let idle = st.mode == Mode::Idle;
            let lost = st.last_event == EventCode::LinkLost;
            let aborts = st.abort_count.wrapping_sub(abort0);
            ok &= idle && lost && aborts >= 1;
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
    match after {
        Some(d) => {
            let trips = d.watchdog_trips.wrapping_sub(before.watchdog_trips);
            let be = d.rx_be.unwrap_or(0).wrapping_sub(before.rx_be.unwrap_or(0));
            ok &= trips >= 1;
            if loss == Loss::Wire {
                ok &= be >= 1;
            }
            parts.push(format!("watchdog_trips +{trips}, rx_be +{be}"));
        }
        None => {
            ok = false;
            parts.push("no debug counters after the loss".to_owned());
        }
    }
    (parts.join("; "), ok)
}
