//! Message bodies (`docs/link-protocol.md`, AD-6, AD-7, AD-8, AD-19), the same
//! length rules as `firmware/common/proto/msgs.c` (Story 2.2 D10):
//!
//! - HELLO, INFO and HEARTBEAT accept trailing bytes after the v1 body and
//!   ignore them (their prefixes are frozen, AD-7);
//! - STATUS keeps its trailing bytes as `extra` (debug counters);
//! - every other body must have exactly its v1 length.
//!
//! Little-endian, byte by byte. NAK reasons, EVENT codes, modes and HID
//! profile ids decode any byte: unknown values are never fatal (AD-19).

use std::fmt;

use super::consts::{
    BODY_LEN_STATUS, EVT_LATE, EVT_LINK_LOST, EVT_NONE, EVT_PLAYBACK_DONE, EVT_UNDERRUN,
    EVT_USB_SUSPENDED, HID_PROFILE_P1, HID_PROFILE_P2, HID_PROFILE_P3, HID_PROFILE_UNKNOWN,
    MAX_BODY, MODE_DIRECT, MODE_IDLE, MODE_PLAYING, MSG_ACK, MSG_EVENT, MSG_HEARTBEAT, MSG_HELLO,
    MSG_INFO, MSG_NAK, MSG_PING, MSG_QUEUE_EVENT, MSG_SET_STATE, MSG_START, MSG_STATUS, MSG_STOP,
    NAK_ABORTED, NAK_BAD_STATE, NAK_BUSY, NAK_CRC, NAK_FULL, NAK_NO_LINK, NAK_NOT_READY, NAK_ORDER,
    NAK_SEMANTIC_MAX, NAK_SEMANTIC_MIN, NAK_SEQ, NAK_TRANSPORT_MAX, NAK_TRANSPORT_MIN,
    NAK_UNKNOWN_TYPE, NAK_VERSION,
};
use super::frame::{FrameError, encode_frame};
use super::state::State;

/// Why a body could not be decoded or encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgError {
    /// The body length does not fit the message (vector error `BODY_LEN`).
    BodyLen,
    /// No message has this type.
    UnknownType(u8),
    /// Encoding only: the body would exceed `MAX_BODY` (STATUS `extra`).
    TooLong,
}

impl fmt::Display for MsgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BodyLen => f.write_str("message body has the wrong length"),
            Self::UnknownType(t) => write!(f, "unknown message type 0x{t:02x}"),
            Self::TooLong => f.write_str("message body longer than 255 bytes"),
        }
    }
}

impl std::error::Error for MsgError {}

/// Declares a `u8` code enum with an `Unknown(u8)` variant and lossless
/// `from_u8` / `to_u8`.
macro_rules! code_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $var:ident = $val:expr),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vm])* $var,)*
            /// A value this app does not know (never fatal, AD-19).
            Unknown(u8),
        }

        impl $name {
            /// From the wire byte; never fails.
            #[must_use]
            pub fn from_u8(v: u8) -> Self {
                match v {
                    $(x if x == $val => Self::$var,)*
                    other => Self::Unknown(other),
                }
            }

            /// The wire byte.
            #[must_use]
            pub fn to_u8(self) -> u8 {
                match self {
                    $(Self::$var => $val,)*
                    Self::Unknown(v) => v,
                }
            }
        }
    };
}

code_enum! {
    /// NAK reason (AD-6).
    NakReason {
        /// Transport: damaged frame.
        Crc = NAK_CRC,
        /// Transport: seq out of the window.
        Seq = NAK_SEQ,
        /// Transport: queue full.
        Full = NAK_FULL,
        /// No matching HELLO yet (or the controller rebooted).
        Version = NAK_VERSION,
        /// Not allowed in this mode.
        Busy = NAK_BUSY,
        /// `t_us` not strictly increasing or too large.
        Order = NAK_ORDER,
        /// Invalid state or frame for the current queue.
        BadState = NAK_BAD_STATE,
        /// Unassigned app type.
        UnknownType = NAK_UNKNOWN_TYPE,
        /// The bridge link is down.
        NoLink = NAK_NO_LINK,
        /// The console is not ready.
        NotReady = NAK_NOT_READY,
        /// The abort latch is set.
        Aborted = NAK_ABORTED,
    }
}

impl NakReason {
    /// A transport NAK (`0x01..=0x0F`): never advances `expected` (AD-6).
    #[must_use]
    pub fn is_transport(self) -> bool {
        (NAK_TRANSPORT_MIN..=NAK_TRANSPORT_MAX).contains(&self.to_u8())
    }

    /// A semantic NAK (`0x10..=0x7F`): advances `expected` (AD-6).
    #[must_use]
    pub fn is_semantic(self) -> bool {
        (NAK_SEMANTIC_MIN..=NAK_SEMANTIC_MAX).contains(&self.to_u8())
    }
}

code_enum! {
    /// EVENT code (AD-5, AD-8); `None` only in `STATUS.last_event`.
    EventCode {
        /// No event yet.
        None = EVT_NONE,
        /// The END event was applied.
        PlaybackDone = EVT_PLAYBACK_DONE,
        /// No heartbeat in time, or a break.
        LinkLost = EVT_LINK_LOST,
        /// The head event was overdue.
        Late = EVT_LATE,
        /// The queue ran dry.
        Underrun = EVT_UNDERRUN,
        /// USB suspend, unmount or bus reset.
        UsbSuspended = EVT_USB_SUSPENDED,
    }
}

code_enum! {
    /// Controller mode (AD-8).
    Mode {
        /// Neutral, nothing playing.
        Idle = MODE_IDLE,
        /// Live `SET_STATE`.
        Direct = MODE_DIRECT,
        /// Timeline playing.
        Playing = MODE_PLAYING,
    }
}

code_enum! {
    /// HID profile id (`INFO.hid_profile`, AD-12).
    HidProfile {
        /// Not known.
        NotKnown = HID_PROFILE_UNKNOWN,
        /// P1.
        P1 = HID_PROFILE_P1,
        /// P2 (wired Pro Controller).
        P2 = HID_PROFILE_P2,
        /// P3.
        P3 = HID_PROFILE_P3,
    }
}

fn u16_at(lo: u8, hi: u8) -> u16 {
    u16::from_le_bytes([lo, hi])
}

fn u32_at(b: [u8; 4]) -> u32 {
    u32::from_le_bytes(b)
}

fn state_of(b: &[u8]) -> Result<State, MsgError> {
    let arr: &[u8; 8] = b.try_into().map_err(|_| MsgError::BodyLen)?;
    Ok(State::from_bytes(arr))
}

/// `HELLO` (0x01, app → controller): `proto_ver`. Trailing bytes ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hello {
    /// The app's protocol version.
    pub proto_ver: u8,
}

impl Hello {
    /// Body bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        vec![self.proto_ver]
    }

    /// From a body of at least 1 byte.
    pub fn decode(body: &[u8]) -> Result<Self, MsgError> {
        let &[proto_ver, ..] = body else {
            return Err(MsgError::BodyLen);
        };
        Ok(Self { proto_ver })
    }
}

/// `INFO` v1 (0x81, controller → app), answer to HELLO (AD-7). Trailing
/// bytes ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    /// The controller's protocol version (byte 0, frozen).
    pub proto_ver: u8,
    /// Controller firmware version.
    pub fw_ver: [u8; 3],
    /// Bridge firmware version from the last HEARTBEAT, zeros if none.
    pub bridge_ver: [u8; 3],
    /// HID profile.
    pub hid_profile: HidProfile,
    /// Queue capacity in events.
    pub queue_cap: u16,
    /// DIRECT-mode app timeout.
    pub app_timeout_ms: u16,
    /// Measured median IN interval, 0 if unknown.
    pub report_period_us: u16,
}

impl Info {
    /// Body bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(14);
        b.push(self.proto_ver);
        b.extend_from_slice(&self.fw_ver);
        b.extend_from_slice(&self.bridge_ver);
        b.push(self.hid_profile.to_u8());
        b.extend_from_slice(&self.queue_cap.to_le_bytes());
        b.extend_from_slice(&self.app_timeout_ms.to_le_bytes());
        b.extend_from_slice(&self.report_period_us.to_le_bytes());
        b
    }

    /// From a body of at least 14 bytes.
    pub fn decode(body: &[u8]) -> Result<Self, MsgError> {
        let &[pv, f0, f1, f2, b0, b1, b2, hp, q0, q1, a0, a1, r0, r1, ..] = body else {
            return Err(MsgError::BodyLen);
        };
        Ok(Self {
            proto_ver: pv,
            fw_ver: [f0, f1, f2],
            bridge_ver: [b0, b1, b2],
            hid_profile: HidProfile::from_u8(hp),
            queue_cap: u16_at(q0, q1),
            app_timeout_ms: u16_at(a0, a1),
            report_period_us: u16_at(r0, r1),
        })
    }
}

/// `SET_STATE` (0x02): the state to apply now (DIRECT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetState {
    /// The state.
    pub state: State,
}

impl SetState {
    /// Body bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.state.to_bytes().to_vec()
    }

    /// From a body of exactly 8 bytes.
    pub fn decode(body: &[u8]) -> Result<Self, MsgError> {
        Ok(Self {
            state: state_of(body)?,
        })
    }
}

/// `QUEUE_EVENT` (0x03): one timeline event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueEvent {
    /// µs from START.
    pub t_us: u32,
    /// The state at `t_us`.
    pub state: State,
}

impl QueueEvent {
    /// Body bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(12);
        b.extend_from_slice(&self.t_us.to_le_bytes());
        b.extend_from_slice(&self.state.to_bytes());
        b
    }

    /// From a body of exactly 12 bytes.
    pub fn decode(body: &[u8]) -> Result<Self, MsgError> {
        let &[t0, t1, t2, t3, ref rest @ ..] = body else {
            return Err(MsgError::BodyLen);
        };
        Ok(Self {
            t_us: u32_at([t0, t1, t2, t3]),
            state: state_of(rest)?,
        })
    }
}

/// `START` (0x04): start playing after `lead_us`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Start {
    /// Lead time in µs.
    pub lead_us: u32,
}

impl Start {
    /// Body bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.lead_us.to_le_bytes().to_vec()
    }

    /// From a body of exactly 4 bytes.
    pub fn decode(body: &[u8]) -> Result<Self, MsgError> {
        let &[a, b, c, d] = body else {
            return Err(MsgError::BodyLen);
        };
        Ok(Self {
            lead_us: u32_at([a, b, c, d]),
        })
    }
}

/// `HEARTBEAT` (0x40, bridge → controller, AD-4): never reaches the app; in
/// the codec for completeness and the vectors. Trailing bytes ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heartbeat {
    /// Bridge firmware version.
    pub bridge_fw_ver: [u8; 3],
    /// Bit0 = CDC DTR asserted (`HB_FLAG_DTR`).
    pub flags: u8,
}

impl Heartbeat {
    /// Body bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let [a, b, c] = self.bridge_fw_ver;
        vec![a, b, c, self.flags]
    }

    /// From a body of at least 4 bytes.
    pub fn decode(body: &[u8]) -> Result<Self, MsgError> {
        let &[a, b, c, flags, ..] = body else {
            return Err(MsgError::BodyLen);
        };
        Ok(Self {
            bridge_fw_ver: [a, b, c],
            flags,
        })
    }
}

/// `STATUS` (0x83, AD-19). Debug builds append counters, kept in `extra`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// Free queue slots.
    pub free: u16,
    /// µs since t0 (0 when IDLE or during the lead).
    pub now_us: u32,
    /// `expected − 1`.
    pub last_seq: u8,
    /// Mode.
    pub mode: Mode,
    /// Abort latch count (wraps).
    pub abort_count: u8,
    /// Last EVENT.
    pub last_event: EventCode,
    /// Bit0 `link_ok`, bit1 `console_ready`, others reserved.
    pub flags: u8,
    /// Bytes after the v1 body (debug counters).
    pub extra: Vec<u8>,
}

impl Status {
    /// Body bytes; `Err(TooLong)` if `extra` makes the body exceed 255 bytes.
    pub fn encode(&self) -> Result<Vec<u8>, MsgError> {
        if self.extra.len() > MAX_BODY - BODY_LEN_STATUS {
            return Err(MsgError::TooLong);
        }
        let mut b = Vec::with_capacity(BODY_LEN_STATUS + self.extra.len());
        b.extend_from_slice(&self.free.to_le_bytes());
        b.extend_from_slice(&self.now_us.to_le_bytes());
        b.extend_from_slice(&[
            self.last_seq,
            self.mode.to_u8(),
            self.abort_count,
            self.last_event.to_u8(),
            self.flags,
        ]);
        b.extend_from_slice(&self.extra);
        Ok(b)
    }

    /// From a body of at least 11 bytes.
    pub fn decode(body: &[u8]) -> Result<Self, MsgError> {
        let &[
            f0,
            f1,
            n0,
            n1,
            n2,
            n3,
            last_seq,
            mode,
            abort_count,
            last_event,
            flags,
            ref extra @ ..,
        ] = body
        else {
            return Err(MsgError::BodyLen);
        };
        Ok(Self {
            free: u16_at(f0, f1),
            now_us: u32_at([n0, n1, n2, n3]),
            last_seq,
            mode: Mode::from_u8(mode),
            abort_count,
            last_event: EventCode::from_u8(last_event),
            flags,
            extra: extra.to_vec(),
        })
    }
}

/// `ACK` (0x84): cumulative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ack {
    /// Highest accepted seq.
    pub seq: u8,
}

impl Ack {
    /// Body bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        vec![self.seq]
    }

    /// From a body of exactly 1 byte.
    pub fn decode(body: &[u8]) -> Result<Self, MsgError> {
        let &[seq] = body else {
            return Err(MsgError::BodyLen);
        };
        Ok(Self { seq })
    }
}

/// `NAK` (0x85): `reason, expected, got` (AD-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nak {
    /// Why.
    pub reason: NakReason,
    /// The controller's `expected` after handling.
    pub expected: u8,
    /// The refused seq (0 for `CRC`).
    pub got: u8,
}

impl Nak {
    /// Body bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        vec![self.reason.to_u8(), self.expected, self.got]
    }

    /// From a body of exactly 3 bytes.
    pub fn decode(body: &[u8]) -> Result<Self, MsgError> {
        let &[reason, expected, got] = body else {
            return Err(MsgError::BodyLen);
        };
        Ok(Self {
            reason: NakReason::from_u8(reason),
            expected,
            got,
        })
    }
}

/// `EVENT` (0x86): something happened on the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    /// What.
    pub code: EventCode,
}

impl Event {
    /// Body bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        vec![self.code.to_u8()]
    }

    /// From a body of exactly 1 byte.
    pub fn decode(body: &[u8]) -> Result<Self, MsgError> {
        let &[code] = body else {
            return Err(MsgError::BodyLen);
        };
        Ok(Self {
            code: EventCode::from_u8(code),
        })
    }
}

fn empty_body(body: &[u8]) -> Result<(), MsgError> {
    if body.is_empty() {
        Ok(())
    } else {
        Err(MsgError::BodyLen)
    }
}

/// Every message of protocol v1, typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// 0x01.
    Hello(Hello),
    /// 0x02.
    SetState(SetState),
    /// 0x03.
    QueueEvent(QueueEvent),
    /// 0x04.
    Start(Start),
    /// 0x05, empty body.
    Stop,
    /// 0x06, empty body.
    Ping,
    /// 0x40.
    Heartbeat(Heartbeat),
    /// 0x81.
    Info(Info),
    /// 0x83.
    Status(Status),
    /// 0x84.
    Ack(Ack),
    /// 0x85.
    Nak(Nak),
    /// 0x86.
    Event(Event),
}

impl Message {
    /// Decodes the body of a frame of type `ty`.
    pub fn decode(ty: u8, body: &[u8]) -> Result<Self, MsgError> {
        Ok(match ty {
            MSG_HELLO => Self::Hello(Hello::decode(body)?),
            MSG_SET_STATE => Self::SetState(SetState::decode(body)?),
            MSG_QUEUE_EVENT => Self::QueueEvent(QueueEvent::decode(body)?),
            MSG_START => Self::Start(Start::decode(body)?),
            MSG_STOP => {
                empty_body(body)?;
                Self::Stop
            }
            MSG_PING => {
                empty_body(body)?;
                Self::Ping
            }
            MSG_HEARTBEAT => Self::Heartbeat(Heartbeat::decode(body)?),
            MSG_INFO => Self::Info(Info::decode(body)?),
            MSG_STATUS => Self::Status(Status::decode(body)?),
            MSG_ACK => Self::Ack(Ack::decode(body)?),
            MSG_NAK => Self::Nak(Nak::decode(body)?),
            MSG_EVENT => Self::Event(Event::decode(body)?),
            other => return Err(MsgError::UnknownType(other)),
        })
    }

    /// The message type.
    #[must_use]
    pub fn ty(&self) -> u8 {
        match self {
            Self::Hello(_) => MSG_HELLO,
            Self::SetState(_) => MSG_SET_STATE,
            Self::QueueEvent(_) => MSG_QUEUE_EVENT,
            Self::Start(_) => MSG_START,
            Self::Stop => MSG_STOP,
            Self::Ping => MSG_PING,
            Self::Heartbeat(_) => MSG_HEARTBEAT,
            Self::Info(_) => MSG_INFO,
            Self::Status(_) => MSG_STATUS,
            Self::Ack(_) => MSG_ACK,
            Self::Nak(_) => MSG_NAK,
            Self::Event(_) => MSG_EVENT,
        }
    }

    /// The body bytes.
    pub fn encode_body(&self) -> Result<Vec<u8>, MsgError> {
        Ok(match self {
            Self::Hello(m) => m.encode(),
            Self::SetState(m) => m.encode(),
            Self::QueueEvent(m) => m.encode(),
            Self::Start(m) => m.encode(),
            Self::Stop | Self::Ping => Vec::new(),
            Self::Heartbeat(m) => m.encode(),
            Self::Info(m) => m.encode(),
            Self::Status(m) => m.encode()?,
            Self::Ack(m) => m.encode(),
            Self::Nak(m) => m.encode(),
            Self::Event(m) => m.encode(),
        })
    }

    /// The whole wire frame (COBS + `0x00`) with sequence number `seq`.
    pub fn encode_frame(&self, seq: u8) -> Result<Vec<u8>, MsgError> {
        let body = self.encode_body()?;
        encode_frame(self.ty(), seq, &body).map_err(|e| match e {
            FrameError::BodyTooLong => MsgError::TooLong,
            _ => MsgError::BodyLen,
        })
    }
}
