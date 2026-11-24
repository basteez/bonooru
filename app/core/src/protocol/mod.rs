//! The link protocol codec (AD-3, AD-7, AD-10, AD-14): COBS, CRC-16, frames,
//! the receive splitter, the canonical state and every message body.
//! Checked against `firmware/common/proto/vectors/` by `tests/vectors.rs`.

pub mod cobs;
pub mod consts;
pub mod crc16;
pub mod frame;
pub mod msgs;
pub mod rx;
pub mod state;

pub use frame::{Frame, FrameError, RxSide, decode_frame, encode_frame};
pub use msgs::{
    Ack, Event, EventCode, Heartbeat, Hello, HidProfile, Info, Message, Mode, MsgError, Nak,
    NakReason, QueueEvent, SetState, Start, Status,
};
pub use rx::{RxEvent, RxStats, Splitter};
pub use state::{NEUTRAL, State};
