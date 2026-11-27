//! `ocb-probe` (Story 4.4 AC 9): the discovery bench tool. Dev-only, never
//! shipped. It runs the app's own `ocb-services` worker and log headless and
//! prints machine-checkable lines (`docs/test-plan.md` H.28–H.31). The
//! verdicts live in [`runner`], generic over a [`runner::Driver`]: the bench
//! uses [`real::RealDriver`], the dry runs (`tests/dry_run.rs`) a driver on
//! the fake backend and clock.

pub mod cli;
pub mod real;
pub mod runner;
