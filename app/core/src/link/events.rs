//! What the session reports and how its commands can be refused. Nothing
//! here ends the session: unknown codes and odd frames are events (AD-19).

use std::fmt;

use crate::protocol::{EventCode, Info, Mode, NakReason};

/// Where the session is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// `open` not called yet.
    Closed,
    /// HELLO sent every 100 ms, waiting for INFO.
    Opening,
    /// INFO received with the same protocol version.
    Connected,
    /// STOP sent, nothing else until its ACK.
    Stopping,
    /// INFO with another protocol version: the firmware needs an update.
    Incompatible,
}

/// The last STATUS, as the app reads it (AD-19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusView {
    /// Free queue slots.
    pub free: u16,
    /// µs since t0 (0 when IDLE or during the lead).
    pub now_us: u32,
    /// The controller's `expected − 1`.
    pub last_seq: u8,
    /// The controller's mode; the app derives its mode from here.
    pub mode: Mode,
    /// Abort latch count (wraps).
    pub abort_count: u8,
    /// The last EVENT the controller sent.
    pub last_event: EventCode,
    /// Bridge ↔ controller link up (flags bit 0).
    pub link_ok: bool,
    /// Console handshake done and polling (flags bit 1).
    pub console_ready: bool,
}

/// Why a piece (or DIRECT input) was given up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbandonReason {
    /// A semantic NAK on a QUEUE_EVENT or START.
    Nak(NakReason),
    /// `EVENT LINK_LOST`, or STATUS `link_ok` 0 with `abort_count` + 1.
    LinkLost,
    /// The controller latched an abort (`abort_count` + 1) for another
    /// reason: underrun, LATE, USB, app timeout, DTR edge, core stall.
    ControllerAborted,
    /// The controller rebooted (mid-session `NAK VERSION`).
    ControllerRebooted,
}

/// Something the app should know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    /// INFO with the same protocol version: the session is open.
    Connected(Info),
    /// INFO with another protocol version: a firmware update is required.
    RequiredUpdate {
        /// The controller's `PROTO_VERSION`.
        controller_proto: u8,
        /// The app's.
        app_proto: u8,
        /// The INFO as received.
        info: Info,
    },
    /// `fw_ver` or `bridge_ver` differs from the app version (AD-13).
    RecommendedUpdate {
        /// Controller firmware version.
        fw_ver: [u8; 3],
        /// Bridge firmware version (`[0, 0, 0]`: no heartbeat seen).
        bridge_ver: [u8; 3],
        /// The app version.
        app_ver: [u8; 3],
        /// The controller firmware differs.
        fw_differs: bool,
        /// The bridge firmware differs (never for `[0, 0, 0]`).
        bridge_differs: bool,
    },
    /// A mid-session `NAK VERSION`: the controller rebooted; HELLO again.
    ControllerRebooted,
    /// A STATUS arrived.
    Status(StatusView),
    /// An EVENT arrived (unknown codes included).
    ControllerEvent(EventCode),
    /// The END event was applied: the piece is done (the session sends
    /// STOP to clear the latch).
    PlaybackDone,
    /// The piece (or DIRECT input) was given up; STOP follows.
    PieceAbandoned {
        /// Why.
        reason: AbandonReason,
    },
    /// A semantic NAK on SET_STATE or PING (no STOP).
    Refused {
        /// Why.
        reason: NakReason,
        /// The refused frame's type.
        ty: u8,
    },
    /// A NAK reason this app does not know; frames before `expected` are
    /// taken as accepted, nothing else changes.
    UnknownNak {
        /// The raw reason.
        reason: u8,
        /// The controller's `expected`.
        expected: u8,
        /// The refused seq.
        got: u8,
    },
    /// STOP was acknowledged: idle, ready for a new piece.
    Stopped,
    /// A frame whose body does not fit its type, or an unknown type.
    Malformed {
        /// The frame's type.
        ty: u8,
    },
}

/// Why a command was refused locally (nothing was sent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandError {
    /// No session (before INFO, after a mismatch or a reboot).
    NotConnected,
    /// STOP is waiting for its ACK.
    Stopping,
    /// The last STATUS says the bridge link is down (or no STATUS yet).
    NoLink,
    /// `t_us` not strictly increasing, or `t_us ≥ 2^31` (mirrors `NAK ORDER`).
    Order,
    /// A state or frame the controller would refuse (mirrors `NAK BAD_STATE`).
    BadState,
    /// START with nothing queued (mirrors `NAK BAD_STATE`).
    EmptyQueue,
    /// START twice, or SET_STATE while a piece is started (mirrors `NAK BUSY`).
    Busy,
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotConnected => "no session with the controller",
            Self::Stopping => "waiting for the STOP to be acknowledged",
            Self::NoLink => "the bridge link is down",
            Self::Order => "event time not strictly increasing or too large",
            Self::BadState => "the controller would refuse this state",
            Self::EmptyQueue => "nothing queued to start",
            Self::Busy => "a piece is already started",
        })
    }
}

impl std::error::Error for CommandError {}
