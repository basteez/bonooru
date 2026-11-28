//! `ocb-services` (Story 4.4, AD-16, AD-18): what the app does besides
//! drawing. No egui/eframe. Modules: `port` (the `serialport` adapter and
//! the traits the worker talks to), `log` (the rotating file log on its own
//! thread), `worker` (the serial worker: one thread owns the port and the
//! link session), `profiles` (Story 5.1: the bundled mapping profile, the
//! config dir, the user copies, the `profile` log lines) and, with the
//! dev-only feature `sim`, `sim` (fake backends for tests and dry runs).

pub mod log;
pub mod port;
pub mod profiles;
#[cfg(feature = "sim")]
pub mod sim;
pub mod worker;
