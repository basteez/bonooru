//! `ocb-sim`: a fake controller, an impaired channel and a driver on a fake
//! clock, for testing the `ocb-core` link session (Story 3.3) and for dry
//! runs of the dev tools (Story 3.4). Dev-only: never shipped.

pub mod channel;
pub mod controller;
pub mod run;

pub use channel::{Channel, ChannelStats, Impair, Rng};
pub use controller::{ControllerConfig, ControllerLog, FakeController};
pub use run::{Observed, Sim};
