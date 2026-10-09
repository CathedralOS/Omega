//! Host artifact byte writing and opt-in measurement storage.
//!
//! `artifact_writer.rs` owns atomic file writes. Trust records and validation
//! belong to realization's trust-model, not this host utility. Timing
//! phase collection is disabled by default and the CLI owns its stderr presentation.
//! Process-wide activity counters are separate diagnostic measurements.

pub mod allocations;
mod artifact_writer;
pub mod compile_timings;

pub use compile_timings::PhaseTiming;
