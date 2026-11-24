//! `ocb-core`: the GUI-free core of the Bonooru app (AD-18). No egui/eframe,
//! no threads, no I/O (checked by `tests/purity.rs`). Story 3.2 brings the
//! protocol codec; the sans-IO link session follows in Story 3.3.

#![forbid(unsafe_code)]

pub mod protocol;
pub mod version;
#[cfg(test)]
mod version_parse;
