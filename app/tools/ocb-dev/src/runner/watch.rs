//! H.3 (Story 3.5 Task 3): records `console_ready` and `link_ok` edges,
//! STATUS silences (the console cuts USB power in sleep) and controller
//! reboots, and probes `START` while the console is not ready (it must get
//! `NAK NOT_READY`). Runs until Ctrl+C or its limit, then judges AC3 and
//! prints the OQ-3 / OQ-4 data (STATUS intervals, the console's longest
//! polling gap).

use ocb_core::link::{AbandonReason, Phase, StatusView};
use ocb_core::protocol::NakReason;

use super::{Io, Owner, Runner, Step, Stop};
use crate::timeline::neutral_end;

/// No STATUS for this long: "STATUS stopped" (six periods).
const SILENCE_US: u64 = 300_000;
/// A ready 1 → 0 edge with STATUS flowing must show within this of the
/// last ready STATUS: 150 ms (the 100 ms stall rule plus one STATUS
/// period) plus one more STATUS period, as `p2watch` judged H.16.
const FALL_LIMIT_US: u64 = 200_000;
/// One NOT_READY probe every this long while the console is not ready.
const PROBE_EVERY_US: u64 = 2_000_000;
/// After a silence, probe once this long after STATUS resumed if the
/// session has not seen the reboot by itself.
const REBOOT_PROBE_AFTER_US: u64 = 1_000_000;

/// What `watch` saw.
#[derive(Debug, Default)]
pub(super) struct WatchLog {
    pub(super) active: bool,
    start: u64,
    info_at: Option<u64>,
    first_ready: Option<bool>,
    last_status_t: Option<u64>,
    last_ready: Option<bool>,
    last_link: Option<bool>,
    last_ready1_t: Option<u64>,
    silent_since: Option<u64>,
    resume_at: Option<u64>,
    /// Per silence: (length µs, reboot seen after it, ready 1 after it).
    silences: Vec<(u64, bool, bool)>,
    falls_flowing: Vec<u64>,
    rises: Vec<u64>,
    gaps: Vec<u64>,
    probes: u32,
    probe_pending: bool,
    probes_not_ready: u32,
    probe_other: Vec<String>,
    reboots: u32,
    next_probe: u64,
    in_max_us: Option<u16>,
    in_slow: Option<u16>,
}

fn ms(us: u64) -> f64 {
    us as f64 / 1000.0
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted.get(idx).copied().unwrap_or(0)
}

impl<I: Io, O: Owner> Runner<'_, I, O> {
    fn wsay(&mut self, now: u64, what: &str) {
        let t = ms(now.saturating_sub(self.watch.start));
        self.owner.say(&format!("{t:>10.1} ms  {what}"));
    }

    pub(super) fn watch(&mut self, max_us: u64) -> Step<String> {
        let now = self.io.now_us();
        self.watch.active = true;
        self.watch.start = now;
        self.s.open(now);
        self.owner.say(
            "WATCH (H.3): plug the controller into the console, then sleep, wake, Wired Communication \
             OFF and ON, one at a time; Ctrl+C ends and judges",
        );
        let end = now + max_us;
        loop {
            self.check_interrupt()?;
            let now = self.io.now_us();
            if now >= end {
                break;
            }
            if self.watch.info_at.is_none() && self.info.is_some() {
                self.watch.info_at = Some(now);
                let i = self.info;
                if let Some(i) = i {
                    self.wsay(
                        now,
                        &format!(
                            "INFO: fw_ver {:?} report_period_us {}",
                            i.fw_ver, i.report_period_us
                        ),
                    );
                }
            }
            // Silence: the controller lost power (sleep) or was unplugged.
            if let Some(t) = self.watch.last_status_t
                && self.watch.silent_since.is_none()
                && now > t + SILENCE_US
            {
                self.watch.silent_since = Some(t);
                self.wsay(now, "STATUS stopped (the controller lost power?)");
            }
            self.probe(now);
            if let Err(e) = self.pump(now + 20_000) {
                return Err(Stop::Fail(format!("watch: {e}")));
            }
        }
        self.watch_verdict()
    }

    /// The NOT_READY probe, and the probe that lets the session see a
    /// reboot after a silence.
    fn probe(&mut self, now: u64) {
        if self.watch.probe_pending || self.s.phase() != Phase::Connected || self.s.is_playing() {
            return;
        }
        let Some(st) = self.status else { return };
        if !st.link_ok || self.watch.silent_since.is_some() {
            return;
        }
        let reboot_unseen = self.watch.silences.last().is_some_and(|s| !s.1)
            && self
                .watch
                .resume_at
                .is_some_and(|t| now >= t + REBOOT_PROBE_AFTER_US);
        let not_ready = !st.console_ready && now >= self.watch.next_probe;
        if !not_ready && !reboot_unseen {
            return;
        }
        if self.s.queue_event(0, neutral_end()).is_err() || self.s.start(0).is_err() {
            return;
        }
        self.watch.next_probe = now + PROBE_EVERY_US;
        if not_ready {
            self.watch.probes += 1;
            self.watch.probe_pending = true;
            self.wsay(now, "probe: START while console_ready 0");
        } else {
            self.wsay(
                now,
                "probe: START after the silence (does the session see a reboot?)",
            );
        }
    }

    pub(super) fn watch_status(&mut self, st: &StatusView, now: u64) {
        let gap = self.watch.last_status_t.map(|t| now - t);
        if let Some(since) = self.watch.silent_since.take() {
            let len = now - since;
            self.watch.silences.push((len, false, false));
            self.watch.resume_at = Some(now);
            self.wsay(now, &format!("STATUS resumed after {:.0} ms", ms(len)));
        } else if let Some(g) = gap {
            self.watch.gaps.push(g);
        }
        if let Some(d) = self.tap.last {
            if let Some(m) = d.in_max_us {
                self.watch.in_max_us = Some(self.watch.in_max_us.map_or(m, |x| x.max(m)));
            }
            if d.in_slow.is_some() {
                self.watch.in_slow = d.in_slow;
            }
        }
        if self.watch.first_ready.is_none() && self.watch.info_at.is_some() {
            self.watch.first_ready = Some(st.console_ready);
            self.wsay(
                now,
                &format!(
                    "first STATUS after INFO: console_ready {}",
                    u8::from(st.console_ready)
                ),
            );
        }
        if self.watch.last_link != Some(st.link_ok) {
            self.wsay(now, &format!("link_ok {}", u8::from(st.link_ok)));
        }
        match (self.watch.last_ready, st.console_ready) {
            (Some(true), false) => {
                let flowing = gap.is_some_and(|g| g <= SILENCE_US);
                let since = self.watch.last_ready1_t.map_or(0, |t| now - t);
                if flowing {
                    self.watch.falls_flowing.push(since);
                }
                self.wsay(
                    now,
                    &format!(
                        "console_ready 1 -> 0, {:.0} ms after the last ready STATUS{}",
                        ms(since),
                        if flowing { "" } else { " (after a silence)" }
                    ),
                );
            }
            (prev, true) if prev != Some(true) => {
                let from = self.watch.resume_at.or(self.watch.info_at).unwrap_or(now);
                let since = now.saturating_sub(from);
                self.watch.rises.push(since);
                if let Some(s) = self.watch.silences.last_mut() {
                    s.2 = true;
                }
                self.wsay(
                    now,
                    &format!(
                        "console_ready 0 -> 1, {:.0} ms after {}",
                        ms(since),
                        if self.watch.resume_at.is_some() {
                            "STATUS resumed"
                        } else {
                            "INFO"
                        }
                    ),
                );
            }
            _ => {}
        }
        if st.console_ready {
            self.watch.last_ready1_t = Some(now);
        }
        self.watch.last_ready = Some(st.console_ready);
        self.watch.last_link = Some(st.link_ok);
        self.watch.last_status_t = Some(now);
    }

    pub(super) fn watch_abandon(&mut self, reason: AbandonReason, now: u64) {
        let pending = std::mem::take(&mut self.watch.probe_pending);
        match reason {
            AbandonReason::Nak(NakReason::NotReady) if pending => {
                self.watch.probes_not_ready += 1;
                self.wsay(now, "probe answered NAK NOT_READY");
            }
            AbandonReason::ControllerRebooted => {
                self.wsay(now, "probe abandoned: the controller rebooted");
            }
            other => {
                if pending {
                    self.watch.probe_other.push(format!("{other:?}"));
                }
                self.wsay(now, &format!("piece abandoned: {other:?}"));
            }
        }
    }

    pub(super) fn watch_reboot(&mut self, now: u64) {
        self.watch.reboots += 1;
        // A probe answered by the reboot (NAK VERSION) tested no NOT_READY.
        if std::mem::take(&mut self.watch.probe_pending) {
            self.watch.probes = self.watch.probes.saturating_sub(1);
        }
        if let Some(s) = self.watch.silences.last_mut() {
            s.1 = true;
        }
        self.wsay(
            now,
            "the session reports: controller rebooted (HELLO again)",
        );
    }

    /// The analysis and the AC3 verdict.
    pub(super) fn watch_verdict(&mut self) -> Step<String> {
        let w = &self.watch;
        let mut gaps = w.gaps.clone();
        gaps.sort_unstable();
        let mut lines = vec![
            format!(
                "first STATUS after INFO: console_ready {}",
                w.first_ready
                    .map_or("-".to_owned(), |r| u8::from(r).to_string())
            ),
            format!(
                "console_ready 0 -> 1 edges: {} ({})",
                w.rises.len(),
                w.rises
                    .iter()
                    .map(|r| format!("{:.0} ms", ms(*r)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            format!(
                "console_ready 1 -> 0 with STATUS flowing: {} ({}; limit {:.0} ms)",
                w.falls_flowing.len(),
                w.falls_flowing
                    .iter()
                    .map(|r| format!("{:.0} ms", ms(*r)))
                    .collect::<Vec<_>>()
                    .join(", "),
                ms(FALL_LIMIT_US)
            ),
            format!(
                "STATUS silences: {} ({})",
                w.silences.len(),
                w.silences
                    .iter()
                    .map(|s| format!(
                        "{:.0} ms, reboot {}, ready after {}",
                        ms(s.0),
                        if s.1 { "seen" } else { "NOT seen" },
                        if s.2 { "yes" } else { "no" }
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            format!(
                "NOT_READY probes: {} sent, {} NAK NOT_READY, other: {}",
                w.probes,
                w.probes_not_ready,
                if w.probe_other.is_empty() {
                    "none".to_owned()
                } else {
                    w.probe_other.join(", ")
                }
            ),
            format!("controller rebooted (session): {}", w.reboots),
            format!(
                "STATUS interval while flowing: n {}, median {:.1} ms, p99 {:.1} ms, max {:.1} ms (OQ-3)",
                gaps.len(),
                ms(percentile(&gaps, 0.5)),
                ms(percentile(&gaps, 0.99)),
                ms(gaps.last().copied().unwrap_or(0))
            ),
            format!(
                "console polling (debug v4): in_max_us {}, in_slow {} (OQ-4)",
                w.in_max_us.map_or("-".to_owned(), |v| v.to_string()),
                w.in_slow.map_or("-".to_owned(), |v| v.to_string())
            ),
        ];
        let mut fails = Vec::new();
        if w.rises.is_empty() {
            fails.push("console_ready never went 0 -> 1".to_owned());
        }
        if w.falls_flowing.is_empty() {
            fails.push(
                "no console_ready 1 -> 0 with STATUS flowing (Wired Communication OFF)".to_owned(),
            );
        }
        if let Some(bad) = w.falls_flowing.iter().find(|&&f| f > FALL_LIMIT_US) {
            fails.push(format!("a 1 -> 0 edge took {:.0} ms", ms(*bad)));
        }
        if w.silences.is_empty() {
            fails.push("no STATUS silence (sleep)".to_owned());
        }
        if w.silences.iter().any(|s| !s.1 || !s.2) {
            fails.push("a silence without a reported reboot or a ready 1 after it".to_owned());
        }
        if w.probes == 0 || w.probes_not_ready != w.probes || !w.probe_other.is_empty() {
            fails.push(format!(
                "probes: {} sent, {} NOT_READY, other {:?}",
                w.probes, w.probes_not_ready, w.probe_other
            ));
        }
        for l in lines.drain(..) {
            self.owner.say(&format!("H.3: {l}"));
        }
        if fails.is_empty() {
            Ok("PASS".to_owned())
        } else {
            Err(Stop::Fail(fails.join("; ")))
        }
    }
}
