//! The three status LEDs as data (Story 4.5; FR-23, UX-DR9–UX-DR11,
//! UX-DR65): a pure function of the latest worker snapshot, the time since
//! the UI received it and the worker's health. No drawing here (G.14 checks
//! this file never names the GUI crate): `ui::status_strip` draws the
//! result, the app logs every change through [`LedTracker`].
//!
//! The contract is the LED decision table of the Story 4.5 file (Dev Notes
//! › LED decision table): each LED is decided top to bottom, the first
//! matching row wins. Every string an LED can show lives in this module.

use ocb_core::device::Class;
use ocb_core::link::Phase;
use ocb_core::protocol::{EventCode, NakReason};
use ocb_services::log::LogHandle;
use ocb_services::worker::{LastError, LinkSnapshot};

use crate::app::WorkerHealth;

/// A STATUS older than this is stale, µs (OQ-3, resolved 2026-11-26 in
/// Story 3.5: 3 × the 50 ms STATUS period). Exactly this age is still
/// fresh.
pub const STATUS_STALE_US: u64 = 150_000;

/// How long an opening session may wait for INFO before the controller is
/// "not answering", µs (Story 4.5 D5: INFO came 4–5 ms after the open on
/// the macOS bench, H.29; five HELLOs at 100 ms). Exactly this age is still
/// within the grace.
pub const INFO_GRACE_US: u64 = 500_000;

/// Words no LED string may contain, case-insensitive (NFR-06, AD-21:
/// nothing may imply online features). `g14_no_online_wording` checks
/// every string a [`Reason`] can produce against this list.
pub const BANNED_WORDS: [&str; 10] = [
    "online",
    "offline",
    "internet",
    "cloud",
    "network",
    "account",
    "sync",
    "update check",
    "linked to game",
    "free play",
];

/// The first banned word in `text`, if any (case-insensitive).
#[must_use]
pub fn banned_word(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    BANNED_WORDS.into_iter().find(|w| lower.contains(w))
}

/// One of the three links, left to right in the strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link3 {
    /// PC ↔ bridge (label "Bridge").
    PcBridge,
    /// Bridge ↔ controller (label "Controller").
    BridgeController,
    /// Controller ↔ Switch (label "Switch").
    ControllerSwitch,
}

impl Link3 {
    /// The three, in strip order.
    pub const ALL: [Self; 3] = [
        Self::PcBridge,
        Self::BridgeController,
        Self::ControllerSwitch,
    ];

    /// The tag of the `led` log line.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::PcBridge => "pc_bridge",
            Self::BridgeController => "bridge_controller",
            Self::ControllerSwitch => "switch",
        }
    }

    /// The visible label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::PcBridge => "Bridge",
            Self::BridgeController => "Controller",
            Self::ControllerSwitch => "Switch",
        }
    }
}

/// The four LED states (UX-DR9; symbol + colour + label, never colour
/// alone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedState {
    /// Filled dot with glow, `status-ok`.
    Ok,
    /// Filled triangle, `status-warning`.
    Warning,
    /// Cross, `status-error`.
    Error,
    /// Hollow ring, `status-unknown`; label only.
    Unknown,
}

impl LedState {
    /// The word of the `led` log line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Unknown => "unknown",
        }
    }
}

/// Which row of the decision table decided an LED (the `reason=` of the
/// `led` log line).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// P1: no snapshot for 3 s.
    WorkerStalled,
    /// P2: the worker thread ended or never started.
    WorkerEnded,
    /// P3: no snapshot yet.
    Start,
    /// P4 / C9: INFO of a compatible session.
    Info,
    /// P5 / C2: INFO with another protocol version.
    RequiredUpdate,
    /// P6: the OS refused the port (Linux serial group).
    Permission,
    /// P7: another program holds the port.
    Busy,
    /// P8: our bridge is open, no INFO.
    OursOpen,
    /// P9: an unconfirmed board is open, no INFO (OQ-5).
    Unconfirmed,
    /// P10 / S1: waiting (a candidate not open yet; the controller not
    /// ready yet).
    Waiting,
    /// P11: only boards with another product string.
    Foreign,
    /// P12: no `2E8A:000A` port.
    NotFound,
    /// C3: no open port.
    NoPort,
    /// C4: opening, within the INFO grace.
    Opening,
    /// C5: opening, INFO grace over.
    NoAnswer,
    /// C6: no fresh STATUS.
    Stale,
    /// C7: `NAK NO_LINK` newer than the last STATUS.
    NakNoLink,
    /// C8: STATUS `link_ok` 0.
    LinkDown,
    /// S2: `NAK NOT_READY` newer than the last STATUS.
    NakNotReady,
    /// S3: STATUS `console_ready` 0.
    ConsoleNotReady,
    /// S4: STATUS `console_ready` 1.
    ConsoleReady,
}

impl Reason {
    /// Every reason (G.14 checks each one's strings for banned words).
    pub const ALL: [Self; 21] = [
        Self::WorkerStalled,
        Self::WorkerEnded,
        Self::Start,
        Self::Info,
        Self::RequiredUpdate,
        Self::Permission,
        Self::Busy,
        Self::OursOpen,
        Self::Unconfirmed,
        Self::Waiting,
        Self::Foreign,
        Self::NotFound,
        Self::NoPort,
        Self::Opening,
        Self::NoAnswer,
        Self::Stale,
        Self::NakNoLink,
        Self::LinkDown,
        Self::NakNotReady,
        Self::ConsoleNotReady,
        Self::ConsoleReady,
    ];

    /// The code of the `led` log line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkerStalled => "worker_stalled",
            Self::WorkerEnded => "worker_ended",
            Self::Start => "start",
            Self::Info => "info",
            Self::RequiredUpdate => "required_update",
            Self::Permission => "permission",
            Self::Busy => "busy",
            Self::OursOpen => "ours_open",
            Self::Unconfirmed => "unconfirmed",
            Self::Waiting => "waiting",
            Self::Foreign => "foreign",
            Self::NotFound => "not_found",
            Self::NoPort => "no_port",
            Self::Opening => "opening",
            Self::NoAnswer => "no_answer",
            Self::Stale => "stale",
            Self::NakNoLink => "nak_no_link",
            Self::LinkDown => "link_down",
            Self::NakNotReady => "nak_not_ready",
            Self::ConsoleNotReady => "console_not_ready",
            Self::ConsoleReady => "console_ready",
        }
    }
}

/// One LED: what the strip draws, the screen reader reads and the log
/// records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Led {
    /// The state.
    pub state: LedState,
    /// "Bridge", "Controller" or "Switch".
    pub label: &'static str,
    /// The muted text after the label (`None` for unknown: label only).
    pub detail: Option<String>,
    /// The tooltip: the detail line, then each fix on its own line.
    pub tooltip: Vec<String>,
    /// The decision-table row.
    pub reason: Reason,
    /// The STATUS age the row used, µs.
    pub status_age_us: Option<u64>,
    /// The session age the row used, µs.
    pub session_age_us: Option<u64>,
}

impl Led {
    fn new(state: LedState, link: Link3, detail: Option<String>, reason: Reason) -> Self {
        Self {
            state,
            label: link.label(),
            detail,
            tooltip: Vec::new(),
            reason,
            status_age_us: None,
            session_age_us: None,
        }
    }

    fn tip(mut self, lines: impl IntoIterator<Item = String>) -> Self {
        self.tooltip.extend(lines);
        self
    }

    fn status_age(mut self, us: Option<u64>) -> Self {
        self.status_age_us = us;
        self
    }

    fn session_age(mut self, us: Option<u64>) -> Self {
        self.session_age_us = us;
        self
    }

    /// The visible text (and the `name=` of the `led` line): label + " " +
    /// detail (the label alone when there is no detail).
    #[must_use]
    pub fn name(&self) -> String {
        match &self.detail {
            Some(d) => format!("{} {d}", self.label),
            None => self.label.to_owned(),
        }
    }

    /// The AccessKit name (AC 11): the visible text, with the state said in
    /// words where the text alone does not say it: an unknown LED is
    /// "<label> – unknown", a required update "<label> v<x> – update
    /// required" (the warning is otherwise only the triangle).
    #[must_use]
    pub fn accessible_name(&self) -> String {
        if self.state == LedState::Unknown {
            format!("{} {UNKNOWN_SUFFIX}", self.label)
        } else if self.reason == Reason::RequiredUpdate {
            format!("{} {UPDATE_SUFFIX}", self.name())
        } else {
            self.name()
        }
    }

    /// The tooltip as one text (the AccessKit description): its lines
    /// joined by newlines.
    #[must_use]
    pub fn tooltip_text(&self) -> String {
        self.tooltip.join("\n")
    }
}

/// The three LEDs and when a timed rule would next change one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leds {
    /// PC ↔ bridge.
    pub pc_bridge: Led,
    /// Bridge ↔ controller.
    pub bridge_controller: Led,
    /// Controller ↔ Switch.
    pub switch: Led,
    /// The smallest time until a staleness or grace boundary would change
    /// an LED with no new snapshot, µs (`None`: no timed rule active).
    pub next_change_in_us: Option<u64>,
}

impl Leds {
    /// The three, in strip order.
    #[must_use]
    pub fn all(&self) -> [(Link3, &Led); 3] {
        [
            (Link3::PcBridge, &self.pc_bridge),
            (Link3::BridgeController, &self.bridge_controller),
            (Link3::ControllerSwitch, &self.switch),
        ]
    }

    /// The LED of `link`.
    #[must_use]
    pub fn get(&self, link: Link3) -> &Led {
        match link {
            Link3::PcBridge => &self.pc_bridge,
            Link3::BridgeController => &self.bridge_controller,
            Link3::ControllerSwitch => &self.switch,
        }
    }
}

// ------------------------------------------------------------ copy
//
// Every LED string (Story 4.5 Dev Notes › Copy). "draft" strings follow the
// Voice rules of EXPERIENCE.md and wait for the OQ-13 microcopy pass.

const CHECKING: &str = "Checking…";
const UNKNOWN_SUFFIX: &str = "– unknown";
const UPDATE_SUFFIX: &str = "– update required";
const STALLED_TIP: &str = "Bonooru's USB check is not responding.";
const STALLED_FIX: &str = "Wait a moment. If it stays like this, restart Bonooru.";
const ENDED_TIP: &str = "Bonooru stopped checking the USB ports.";
const ENDED_FIX: &str = "Restart Bonooru. If it happens again, send the file bonooru.log.";
const DETAIL_NO_ACCESS: &str = "– no access";
const DETAIL_IN_USE: &str = "– in use";
const DETAIL_FOUND: &str = "– found";
const DETAIL_OTHER_BOARD: &str = "– other board";
const DETAIL_NOT_FOUND: &str = "– not found";
const DETAIL_NOT_ANSWERING: &str = "– not answering";
const DETAIL_NO_LINK: &str = "– no link";
const DETAIL_NOT_READY: &str = "– not ready";
const DETAIL_READY: &str = "– ready";
const BUSY_FIX: &str =
    "Close it (a serial monitor or another Bonooru window); Bonooru retries by itself.";
const UNCONFIRMED_FIX: &str = "Wire the controller to the bridge and plug it into the Switch.";
const FOREIGN_FIX: &str = "Flash it with the bridge firmware: hold BOOT, plug it in and drag the bridge .uf2 onto the RPI-RP2 drive.";
const NOT_FOUND_TIP: &str = "No bridge found.";
const NOT_FOUND_FIX: &str = "Plug the bridge into your PC.";
const REQUIRED_TIP: &str = "This firmware doesn't match this app.";
const REQUIRED_FIX: &str =
    "Update both boards: hold BOOT, plug in, drag the matching .uf2 onto the RPI-RP2 drive.";
const WAITING_BRIDGE: &str = "Waiting for the bridge.";
const ASKING: &str = "Asking the controller…";
const NO_ANSWER_TIP: &str = "No answer from the controller.";
const NO_ANSWER_FIX: &str =
    "Check the three wires between the boards and that the controller is plugged into the Switch.";
const NO_LINK_TIP: &str = "The controller doesn't hear the bridge.";
const NO_LINK_FIX: &str = "Check the wire from the bridge's GP0 to the controller's GP1, and GND.";
const WAITING_CONTROLLER: &str = "Waiting for the controller.";
const NOT_READY_FIX: &str =
    "Turn on Pro Controller Wired Communication, wake the Switch, check the cable.";
const UNDOCK_FIX: &str = "Undock the Switch.";
const READY_TIP: &str = "The Switch is reading the controller.";

/// A firmware version as shown in the labels: `v<X>.<Y>.<Z>` (D4).
#[must_use]
pub fn version(v: [u8; 3]) -> String {
    let [a, b, c] = v;
    format!("v{a}.{b}.{c}")
}

fn s(text: &str) -> String {
    text.to_owned()
}

// ------------------------------------------------------------ model

fn status_age(snap: &LinkSnapshot, since_received_us: u64) -> Option<u64> {
    let rx = snap.status.map(|(_, rx)| rx).or(snap.connected_us)?;
    Some(
        snap.now_us
            .saturating_sub(rx)
            .saturating_add(since_received_us),
    )
}

fn session_age(snap: &LinkSnapshot, since_received_us: u64) -> Option<u64> {
    snap.session_since_us.map(|t| {
        snap.now_us
            .saturating_sub(t)
            .saturating_add(since_received_us)
    })
}

/// The NAK `reason` is newer than the last STATUS (or no STATUS came).
fn nak_latched(snap: &LinkSnapshot, reason: NakReason) -> bool {
    snap.last_nak
        .is_some_and(|(r, t)| r == reason && snap.status.is_none_or(|(_, rx)| rx <= t))
}

fn is_candidate(class: &Class) -> bool {
    matches!(class, Class::Ours | Class::Unconfirmed)
}

/// The three LEDs before or without a usable snapshot (P1–P3, C1, S1).
fn all_unknown(reason: Reason) -> Leds {
    let led = |link: Link3| {
        let lines = match reason {
            Reason::WorkerStalled => vec![s(STALLED_TIP), s(STALLED_FIX)],
            Reason::WorkerEnded => vec![s(ENDED_TIP), s(ENDED_FIX)],
            _ => vec![s(CHECKING)],
        };
        Led::new(LedState::Unknown, link, None, reason).tip(lines)
    };
    Leds {
        pc_bridge: led(Link3::PcBridge),
        bridge_controller: led(Link3::BridgeController),
        switch: led(Link3::ControllerSwitch),
        next_change_in_us: None,
    }
}

fn pc_bridge(snap: &LinkSnapshot) -> Led {
    let link = Link3::PcBridge;
    let chosen_name = snap.chosen.as_ref().map_or("", |c| c.name.as_str());
    // P4, P5: versions from INFO.
    if let Some(info) = snap.info.as_ref().or(snap.required_update.as_ref()) {
        let reason = if snap.info.is_some() {
            Reason::Info
        } else {
            Reason::RequiredUpdate
        };
        let v = version(info.bridge_ver);
        let tip = format!("Bridge {v} on {chosen_name}.");
        return Led::new(LedState::Ok, link, Some(v), reason).tip([tip]);
    }
    let listed = |port: &str| {
        snap.ports
            .iter()
            .any(|p| p.info.name == port && is_candidate(&p.class))
    };
    // P6, P7: what the worker reported for a listed candidate.
    match &snap.last_error {
        Some(LastError::PermissionDenied { port, message }) if listed(port) => {
            return Led::new(
                LedState::Error,
                link,
                Some(s(DETAIL_NO_ACCESS)),
                Reason::Permission,
            )
            .tip([format!("Bridge {DETAIL_NO_ACCESS}"), message.clone()]);
        }
        Some(LastError::Busy { port }) if listed(port) => {
            return Led::new(
                LedState::Warning,
                link,
                Some(s(DETAIL_IN_USE)),
                Reason::Busy,
            )
            .tip([format!("Another program is using {port}."), s(BUSY_FIX)]);
        }
        _ => {}
    }
    // P8, P9: a candidate is open, no INFO yet.
    if let Some(c) = snap.chosen.as_ref().filter(|c| c.open) {
        match c.class {
            Class::Ours => {
                return Led::new(LedState::Ok, link, Some(s(DETAIL_FOUND)), Reason::OursOpen)
                    .tip([format!("Bridge found on {}.", c.name)]);
            }
            Class::Unconfirmed => {
                return Led::new(LedState::Unknown, link, None, Reason::Unconfirmed).tip([
                    format!(
                        "A board that may be the bridge is on {}. Bonooru can confirm it once the controller answers.",
                        c.name
                    ),
                    s(UNCONFIRMED_FIX),
                ]);
            }
            _ => {}
        }
    }
    // P10: listed, not open yet (or retrying).
    if let Some(p) = snap.ports.iter().find(|p| is_candidate(&p.class)) {
        let port = if listed(chosen_name) {
            chosen_name
        } else {
            p.info.name.as_str()
        };
        return Led::new(LedState::Unknown, link, None, Reason::Waiting)
            .tip([format!("Opening {port}…")]);
    }
    // P11: only boards with another product string.
    let foreign: Vec<&str> = snap
        .ports
        .iter()
        .filter_map(|p| match &p.class {
            Class::Foreign { product } => Some(product.as_str()),
            _ => None,
        })
        .collect();
    if let Some(first) = foreign.first() {
        let more = foreign.len() - 1;
        let tip = if more == 0 {
            format!("Found a “{first}” board – not a Bonooru bridge.")
        } else {
            format!("Found a “{first}” board – not a Bonooru bridge – and {more} more.")
        };
        return Led::new(
            LedState::Warning,
            link,
            Some(s(DETAIL_OTHER_BOARD)),
            Reason::Foreign,
        )
        .tip([tip, s(FOREIGN_FIX)]);
    }
    // P12.
    Led::new(
        LedState::Error,
        link,
        Some(s(DETAIL_NOT_FOUND)),
        Reason::NotFound,
    )
    .tip([s(NOT_FOUND_TIP), s(NOT_FOUND_FIX)])
}

/// The Controller LED and its next timed change.
fn bridge_controller(snap: &LinkSnapshot, since_received_us: u64) -> (Led, Option<u64>) {
    let link = Link3::BridgeController;
    // C2.
    if let Some(info) = &snap.required_update {
        let led = Led::new(
            LedState::Warning,
            link,
            Some(version(info.fw_ver)),
            Reason::RequiredUpdate,
        )
        .tip([s(REQUIRED_TIP), s(REQUIRED_FIX)]);
        return (led, None);
    }
    let no_port =
        || Led::new(LedState::Unknown, link, None, Reason::NoPort).tip([s(WAITING_BRIDGE)]);
    // C3.
    if !snap.chosen.as_ref().is_some_and(|c| c.open) {
        return (no_port(), None);
    }
    match snap.phase {
        Phase::Opening => {
            let age = session_age(snap, since_received_us);
            match age {
                // C5.
                Some(a) if a > INFO_GRACE_US => {
                    let led = Led::new(
                        LedState::Error,
                        link,
                        Some(s(DETAIL_NOT_ANSWERING)),
                        Reason::NoAnswer,
                    )
                    .tip([s(NO_ANSWER_TIP), s(NO_ANSWER_FIX)])
                    .session_age(age);
                    (led, None)
                }
                // C4.
                _ => {
                    let led = Led::new(LedState::Unknown, link, None, Reason::Opening)
                        .tip([s(ASKING)])
                        .session_age(age);
                    (led, age.map(|a| INFO_GRACE_US + 1 - a))
                }
            }
        }
        Phase::Connected | Phase::Stopping => {
            let age = status_age(snap, since_received_us);
            let Some(a) = age.filter(|&a| a <= STATUS_STALE_US) else {
                // C6.
                let led = Led::new(
                    LedState::Error,
                    link,
                    Some(s(DETAIL_NOT_ANSWERING)),
                    Reason::Stale,
                )
                .tip([s(NO_ANSWER_TIP), s(NO_ANSWER_FIX)])
                .status_age(age);
                return (led, None);
            };
            let next = Some(STATUS_STALE_US + 1 - a);
            let no_link = |reason| {
                Led::new(LedState::Error, link, Some(s(DETAIL_NO_LINK)), reason)
                    .tip([s(NO_LINK_TIP), s(NO_LINK_FIX)])
                    .status_age(age)
            };
            // C7.
            if nak_latched(snap, NakReason::NoLink) {
                return (no_link(Reason::NakNoLink), next);
            }
            // C8.
            if snap.status.is_some_and(|(v, _)| !v.link_ok) {
                return (no_link(Reason::LinkDown), next);
            }
            // C9. A connected session always has INFO; should it not, the
            // port is open and the controller unconfirmed: "opening", not
            // "no port".
            let Some(info) = &snap.info else {
                let led = Led::new(LedState::Unknown, link, None, Reason::Opening)
                    .tip([s(ASKING)])
                    .status_age(age);
                return (led, None);
            };
            let v = version(info.fw_ver);
            let led = Led::new(LedState::Ok, link, Some(v.clone()), Reason::Info)
                .tip([format!("Controller {v}, talking to the bridge.")])
                .status_age(age);
            (led, next)
        }
        // `Incompatible` always comes with `required_update` (C2); `Closed`
        // has no port.
        Phase::Closed | Phase::Incompatible => (no_port(), None),
    }
}

fn controller_switch(snap: &LinkSnapshot, controller: &Led) -> Led {
    let link = Link3::ControllerSwitch;
    let waiting =
        || Led::new(LedState::Unknown, link, None, Reason::Waiting).tip([s(WAITING_CONTROLLER)]);
    // S1.
    if controller.state != LedState::Ok {
        return waiting();
    }
    let not_ready = |reason| {
        let mut lines = vec![format!("Switch {DETAIL_NOT_READY}"), s(NOT_READY_FIX)];
        if snap
            .status
            .is_some_and(|(v, _)| v.last_event == EventCode::UsbSuspended)
        {
            lines.push(s(UNDOCK_FIX));
        }
        Led::new(LedState::Warning, link, Some(s(DETAIL_NOT_READY)), reason).tip(lines)
    };
    // S2.
    if nak_latched(snap, NakReason::NotReady) {
        return not_ready(Reason::NakNotReady);
    }
    match snap.status {
        // S3.
        Some((v, _)) if !v.console_ready => not_ready(Reason::ConsoleNotReady),
        // S4.
        Some(_) => Led::new(
            LedState::Ok,
            link,
            Some(s(DETAIL_READY)),
            Reason::ConsoleReady,
        )
        .tip([s(READY_TIP)]),
        // INFO came but no STATUS yet: nothing is known of the console.
        None => waiting(),
    }
}

/// The three LEDs (Story 4.5 AC 4–8): `snap` is the latest snapshot the UI
/// holds, `since_received_us` the UI clock's time since it arrived (the
/// snapshot's `now_us` freezes between publishes, D2), `health` the
/// worker's.
#[must_use]
pub fn evaluate(snap: Option<&LinkSnapshot>, since_received_us: u64, health: WorkerHealth) -> Leds {
    match health {
        WorkerHealth::Stalled => return all_unknown(Reason::WorkerStalled),
        WorkerHealth::Ended | WorkerHealth::NotStarted => {
            return all_unknown(Reason::WorkerEnded);
        }
        WorkerHealth::Running => {}
    }
    let Some(snap) = snap else {
        return all_unknown(Reason::Start);
    };
    let pc = pc_bridge(snap);
    let (controller, next) = bridge_controller(snap, since_received_us);
    let switch = controller_switch(snap, &controller);
    Leds {
        pc_bridge: pc,
        bridge_controller: controller,
        switch,
        next_change_in_us: next,
    }
}

// ------------------------------------------------------------ log

/// The text of one `led` log line (Story 4.5 AC 12; the bench reads
/// exactly this): `<link>=<state> name="<name>" reason=<reason>
/// status_age_us=<n|-> session_age_us=<n|->`. Ages in whole µs, as the
/// rules judge them (`> 150000`, `> 500000`): rounded to ms, a correct
/// change at the boundary would read 150 / 500 (instrument review).
#[must_use]
pub fn led_line(link: Link3, led: &Led) -> String {
    let us = |v: Option<u64>| v.map_or_else(|| "-".to_owned(), |u| u.to_string());
    format!(
        "{}={} name=\"{}\" reason={} status_age_us={} session_age_us={}",
        link.tag(),
        led.state.as_str(),
        led.name(),
        led.reason.as_str(),
        us(led.status_age_us),
        us(led.session_age_us)
    )
}

/// Writes one `led` line per LED whose state, name or reason changed
/// since the last [`LedTracker::observe`], nothing otherwise (AC 12): a
/// worker death while an LED is already unknown, Stale ↔ NoAnswer and the
/// NAK latches are all logged. Logging never blocks (`LogHandle::log` is a
/// `try_send`, AD-16).
#[derive(Debug)]
pub struct LedTracker {
    last: Option<[(LedState, String, Reason); 3]>,
    log: LogHandle,
}

impl LedTracker {
    /// A tracker that has seen nothing yet: its first `observe` logs all
    /// three LEDs.
    #[must_use]
    pub fn new(log: LogHandle) -> Self {
        Self { last: None, log }
    }

    /// Logs the LEDs that changed.
    pub fn observe(&mut self, leds: &Leds) {
        let now = leds
            .all()
            .map(|(_, led)| (led.state, led.name(), led.reason));
        for (k, (link, led)) in leds.all().into_iter().enumerate() {
            let same = self
                .last
                .as_ref()
                .and_then(|l| l.get(k))
                .is_some_and(|prev| now.get(k) == Some(prev));
            if !same {
                self.log.log("led", led_line(link, led));
            }
        }
        self.last = Some(now);
    }
}
