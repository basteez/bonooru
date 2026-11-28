//! `ocb-core`: the GUI-free core of the Bonooru app (AD-18). No egui/eframe,
//! no threads, no I/O (checked by `tests/purity.rs`). Story 3.2 brought the
//! protocol codec, Story 3.3 the sans-IO link session, Story 4.4 the bridge
//! discovery rules, Story 5.1 the mapping profile module.

#![forbid(unsafe_code)]

pub mod device;
pub mod link;
pub mod profile;
pub mod protocol;
pub mod version;
#[cfg(test)]
mod version_parse;
