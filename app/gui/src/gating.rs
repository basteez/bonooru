//! The gate as data (Story 4.6, 4.8; FR-10, UX-DR13, UX-DR38, AD-7):
//! whether the gamepad and Play are blocked, and why, as a pure function of
//! the latest worker snapshot and the three LEDs. No drawing here (G.17
//! checks this file never names the GUI crate): `ui::gamepad` draws the
//! hint, `ui::firmware` builds the dialog, the app logs every change through
//! [`GateTracker`].
//!
//! The firmware reason reads `LinkSnapshot.required_update` only: never
//! `info` (it is `None` in `Incompatible`) and never `status` (a mismatching
//! controller's STATUS is decoded with this app's layout and is not trusted;
//! AD-7: only INFO byte 0 is frozen). The link reasons (Story 4.8, D4) read
//! the LEDs, which already judge staleness, the INFO grace, the worker's
//! health and the NAKs: the gate never disagrees with the strip. Every
//! string the gate can show lives in this module.

use ocb_core::protocol::Info;
use ocb_core::protocol::consts::PROTO_VERSION;
use ocb_services::log::LogHandle;
use ocb_services::worker::LinkSnapshot;

use crate::leds::{LedState, Leds, Reason};

/// The gamepad hint while a required update blocks it (`EXPERIENCE.md`
/// Voice, "Required update, gamepad hint", draft).
pub const HINT_FIRMWARE: &str = "The gamepad is off until both boards are updated.";
/// The firmware dialog's title (`EXPERIENCE.md` Voice).
pub const MODAL_TITLE: &str = "Firmware update needed";
/// The firmware dialog's body (Story 4.6 Copy, draft; neutral: a board can
/// be older or newer than the app).
pub const MODAL_BODY: &str = "The boards' firmware doesn't match this app. Reflash both with the .uf2 files of this app version: hold BOOT, plug the board in and drag its .uf2 onto the RPI-RP2 drive.";
/// The firmware dialog's only button (Epic 4: OK only, no wizard).
pub const MODAL_OK: &str = "OK";
/// The firmware dialog's id in the `dialog` log lines.
pub const DIALOG_LOG_ID: &str = "firmware_update_needed";

/// The gamepad hint while the serial worker is stuck or gone (Story 4.8
/// Copy, draft; the LED worker tooltip's wording).
pub const HINT_WORKER: &str = "Gamepad clicks are ignored: Bonooru's USB check is not responding.";
/// The gamepad hint while no bridge is found (`EXPERIENCE.md` State
/// Patterns › Bridge not found; "Set up / Reflash…" comes with Epic 8).
pub const HINT_NO_BRIDGE: &str = "Plug the bridge into your PC.";
/// The gamepad hint while the bridge is found but not ready (draft).
pub const HINT_BRIDGE_NOT_READY: &str = "Gamepad clicks are ignored until the bridge is ready.";
/// The gamepad hint while the controller does not answer (draft).
pub const HINT_CONTROLLER_LINK: &str = "Gamepad clicks are ignored until the controller answers.";
/// The gamepad hint while the Switch is not ready (`epics.md` Story 4.8,
/// mock `.ihint`).
pub const HINT_SWITCH_NOT_READY: &str = "Gamepad clicks are ignored until the Switch is ready.";

/// Every string the gate can show, for the banned-words check (G.17).
pub const STRINGS: [&str; 9] = [
    HINT_WORKER,
    HINT_NO_BRIDGE,
    HINT_BRIDGE_NOT_READY,
    HINT_FIRMWARE,
    HINT_CONTROLLER_LINK,
    HINT_SWITCH_NOT_READY,
    MODAL_TITLE,
    MODAL_BODY,
    MODAL_OK,
];

/// Why the gamepad or Play is off, in precedence order (first match wins,
/// Story 4.8 AC 9). Epic 6 adds the song reasons.
///
/// A required update outranks the Controller and Switch link reasons
/// because they are its consequence: no STATUS is trusted in
/// `Incompatible`. The bridge reasons outrank it: without a bridge no INFO
/// can be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Block {
    /// The serial worker is stuck or ended (the LEDs show it).
    Worker,
    /// No bridge port (PC↔Bridge `NotFound`).
    NoBridge,
    /// PC↔Bridge not `ok` for any other reason.
    BridgeNotReady,
    /// The controller speaks another protocol version (FR-10).
    FirmwareUpdate {
        /// INFO byte 0 of the controller.
        controller_proto: u8,
        /// This app's `PROTO_VERSION`.
        app_proto: u8,
    },
    /// Bridge↔Controller not `ok`.
    ControllerLink,
    /// Controller↔Switch not `ok`.
    SwitchNotReady,
}

impl Block {
    /// One block of every variant (G.17, finding 16).
    #[must_use]
    pub fn samples() -> [Self; 6] {
        let all = [
            Self::Worker,
            Self::NoBridge,
            Self::BridgeNotReady,
            Self::FirmwareUpdate {
                controller_proto: PROTO_VERSION.wrapping_add(1),
                app_proto: PROTO_VERSION,
            },
            Self::ControllerLink,
            Self::SwitchNotReady,
        ];
        // Exhaustive on purpose: a new variant fails to compile here until
        // it is listed above (its position is its precedence).
        for b in &all {
            let _listed: usize = match b {
                Self::Worker => 0,
                Self::NoBridge => 1,
                Self::BridgeNotReady => 2,
                Self::FirmwareUpdate { .. } => 3,
                Self::ControllerLink => 4,
                Self::SwitchNotReady => 5,
            };
        }
        all
    }

    /// The inline hint the gamepad panel shows for this block.
    #[must_use]
    pub fn gamepad_hint(&self) -> &'static str {
        match self {
            Self::Worker => HINT_WORKER,
            Self::NoBridge => HINT_NO_BRIDGE,
            Self::BridgeNotReady => HINT_BRIDGE_NOT_READY,
            Self::FirmwareUpdate { .. } => HINT_FIRMWARE,
            Self::ControllerLink => HINT_CONTROLLER_LINK,
            Self::SwitchNotReady => HINT_SWITCH_NOT_READY,
        }
    }

    /// The reason in the `gate` and `pad ignored` log lines.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Worker => "worker",
            Self::NoBridge => "no_bridge",
            Self::BridgeNotReady => "bridge_not_ready",
            Self::FirmwareUpdate { .. } => "firmware_update",
            Self::ControllerLink => "controller_link",
            Self::SwitchNotReady => "switch_not_ready",
        }
    }
}

/// What is blocked now. `None` = enabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gate {
    /// Why the gamepad is off.
    pub gamepad: Option<Block>,
    /// Why Play is off.
    pub play: Option<Block>,
}

impl Gate {
    /// Whether the gamepad may drive the controller.
    #[must_use]
    pub fn gamepad_enabled(&self) -> bool {
        self.gamepad.is_none()
    }

    /// Whether Play may start a song.
    #[must_use]
    pub fn play_enabled(&self) -> bool {
        self.play.is_none()
    }
}

/// The gate for the latest snapshot and the LEDs drawn from it (Story 4.8
/// AC 9, D4): first match wins: `worker` › `no_bridge` ›
/// `bridge_not_ready` › `firmware_update` › `controller_link` ›
/// `switch_not_ready`; open only with the three LEDs `ok` and no required
/// update. Play gets the same block (Epic 6 adds the song reasons).
#[must_use]
pub fn evaluate(snap: Option<&LinkSnapshot>, leds: &Leds) -> Gate {
    let worker = leds
        .all()
        .iter()
        .any(|(_, l)| matches!(l.reason, Reason::WorkerStalled | Reason::WorkerEnded));
    let block = if worker {
        Some(Block::Worker)
    } else if leds.pc_bridge.reason == Reason::NotFound {
        Some(Block::NoBridge)
    } else if leds.pc_bridge.state != LedState::Ok {
        Some(Block::BridgeNotReady)
    } else if let Some(info) = snap.and_then(|s| s.required_update.as_ref()) {
        Some(Block::FirmwareUpdate {
            controller_proto: info.proto_ver,
            app_proto: PROTO_VERSION,
        })
    } else if leds.bridge_controller.state != LedState::Ok {
        Some(Block::ControllerLink)
    } else if leds.switch.state != LedState::Ok {
        Some(Block::SwitchNotReady)
    } else {
        None
    };
    Gate {
        gamepad: block,
        play: block,
    }
}

/// Decides when the firmware dialog is requested: once per mismatching
/// connection. A connection is one INFO of one session, identified by
/// `LinkSnapshot.connected_us` (worker clock, new on every open), so a
/// reconnect is recognised even when the UI never saw the snapshot in
/// between (the snapshot channel keeps only the newest).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FirmwareWatch {
    shown_for: Option<u64>,
}

impl FirmwareWatch {
    /// The mismatching INFO and its `connected_us` the first time a
    /// snapshot of that connection is seen; `None` otherwise. Never resets
    /// on a snapshot without a mismatch.
    pub fn take_new(&mut self, snap: Option<&LinkSnapshot>) -> Option<(Info, u64)> {
        let snap = snap?;
        let info = snap.required_update?;
        let at = snap.connected_us?;
        if self.shown_for == Some(at) {
            return None;
        }
        self.shown_for = Some(at);
        Some((info, at))
    }
}

/// The `gate` log line for `gate` (without the kind tag): each gate with
/// its own reason (Story 4.8 AC 10, 4.6 finding 14); the protocol versions
/// of a firmware block of either gate, `-` otherwise.
#[must_use]
pub fn gate_line(gate: &Gate) -> String {
    let on = |b: &Option<Block>| if b.is_some() { "off" } else { "on" };
    let reason = |b: &Option<Block>| b.as_ref().map_or("none", Block::as_str);
    let protos = [gate.gamepad, gate.play]
        .into_iter()
        .flatten()
        .find_map(|b| match b {
            Block::FirmwareUpdate {
                controller_proto,
                app_proto,
            } => Some((controller_proto.to_string(), app_proto.to_string())),
            _ => None,
        });
    let (controller, app) = protos.unwrap_or_else(|| ("-".to_owned(), "-".to_owned()));
    format!(
        "gamepad={} gamepad_reason={} play={} play_reason={} controller_proto={controller} app_proto={app}",
        on(&gate.gamepad),
        reason(&gate.gamepad),
        on(&gate.play),
        reason(&gate.play)
    )
}

/// The `dialog` line written when the firmware dialog is requested.
#[must_use]
pub fn dialog_shown_line(info: &Info, connected_us: u64) -> String {
    format!(
        "shown id={DIALOG_LOG_ID} connected_us={connected_us} controller_proto={} app_proto={PROTO_VERSION} fw={} bridge={}",
        info.proto_ver,
        triple(info.fw_ver),
        triple(info.bridge_ver)
    )
}

fn triple([a, b, c]: [u8; 3]) -> String {
    format!("{a}.{b}.{c}")
}

/// The `dialog` line written when the firmware dialog closes; `outcome` is
/// `primary` (OK) or `dismissed` (Esc).
#[must_use]
pub fn dialog_closed_line(outcome: &str) -> String {
    format!("closed id={DIALOG_LOG_ID} outcome={outcome}")
}

/// Writes one `gate` line per change of the gate, nothing otherwise.
/// Starts at "no block", so a first frame without a mismatch logs nothing.
/// Logging never blocks (`LogHandle::log` is a `try_send`, AD-16).
#[derive(Debug)]
pub struct GateTracker {
    last: Gate,
    log: LogHandle,
}

impl GateTracker {
    /// A tracker at "no block".
    #[must_use]
    pub fn new(log: LogHandle) -> Self {
        Self {
            last: Gate::default(),
            log,
        }
    }

    /// Logs `gate` if it differs from the last one seen.
    pub fn observe(&mut self, gate: &Gate) {
        if *gate != self.last {
            self.log.log("gate", gate_line(gate));
            self.last = *gate;
        }
    }
}
