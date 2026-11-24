//! `ocb-dev` (Story 3.4): a dev-only bench tool that opens the bridge and
//! streams a timeline through `ocb-core`'s session. Never shipped to users
//! (FR-15 is about the GUI app). The modules are public for the dry-run
//! tests, which drive the same runner against `ocb-sim`.

pub mod port_rule;
pub mod runner;
pub mod serial;
pub mod soak;
pub mod tap;
pub mod timeline;
