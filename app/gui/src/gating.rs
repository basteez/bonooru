//! The gate as data (Story 4.6; FR-10, UX-DR13, UX-DR38, AD-7): whether
//! the gamepad and Play are blocked, and why, as a pure function of the
//! latest worker snapshot. No drawing here (G.17 checks this file never
//! names the GUI crate): `ui::main_screen` draws the hint, `ui::firmware`
//! builds the dialog, the app logs every change through [`GateTracker`].
//!
//! Story 4.6 has one reason, a required firmware update. The decision reads
//! `LinkSnapshot.required_update` only: never `info` (it is `None` in
//! `Incompatible`) and never `status` (a mismatching controller's STATUS is
//! decoded with this app's layout and is not trusted; AD-7: only INFO byte 0
//! is frozen). Every string the gate can show lives in this module.

use ocb_core::protocol::Info;
use ocb_core::protocol::consts::PROTO_VERSION;
use ocb_services::log::LogHandle;
use ocb_services::worker::LinkSnapshot;

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

/// Every string the gate can show, for the banned-words check (G.17).
pub const STRINGS: [&str; 4] = [HINT_FIRMWARE, MODAL_TITLE, MODAL_BODY, MODAL_OK];

/// Why the gamepad or Play is off.
///
/// Story 4.8 adds `NoBridge`, `LinkDown` and `SwitchNotReady`, Epic 6 the
/// song reasons. A required update outranks the Controller and Switch link
/// reasons because they are its consequence: no STATUS is trusted in
/// `Incompatible`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Block {
    /// The controller speaks another protocol version (FR-10).
    FirmwareUpdate {
        /// INFO byte 0 of the controller.
        controller_proto: u8,
        /// This app's `PROTO_VERSION`.
        app_proto: u8,
    },
}

impl Block {
    /// The inline hint the gamepad panel shows for this block.
    #[must_use]
    pub fn gamepad_hint(&self) -> &'static str {
        match self {
            Self::FirmwareUpdate { .. } => HINT_FIRMWARE,
        }
    }

    /// The reason in the `gate` log line.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FirmwareUpdate { .. } => "firmware_update",
        }
    }
}

/// What is blocked now. `None` = enabled (as far as this story knows: 4.8
/// adds the link reasons).
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

/// The gate for the latest snapshot. Independent of the worker's health: a
/// dead worker's last known mismatch keeps blocking (the safe side; 4.8
/// adds the dead-worker reason).
#[must_use]
pub fn evaluate(snap: Option<&LinkSnapshot>) -> Gate {
    let block = snap
        .and_then(|s| s.required_update.as_ref())
        .map(|info| Block::FirmwareUpdate {
            controller_proto: info.proto_ver,
            app_proto: PROTO_VERSION,
        });
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

/// The `gate` log line for `gate` (without the kind tag).
#[must_use]
pub fn gate_line(gate: &Gate) -> String {
    let on = |b: &Option<Block>| if b.is_some() { "off" } else { "on" };
    let reason = gate.gamepad.or(gate.play);
    let (name, controller, app) = match reason {
        Some(Block::FirmwareUpdate {
            controller_proto,
            app_proto,
        }) => (
            "firmware_update",
            controller_proto.to_string(),
            app_proto.to_string(),
        ),
        None => ("none", "-".to_owned(), "-".to_owned()),
    };
    format!(
        "gamepad={} play={} reason={name} controller_proto={controller} app_proto={app}",
        on(&gate.gamepad),
        on(&gate.play)
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
