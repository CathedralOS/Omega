//! Opt-in compilation measurement: the phase-timing ladder and the
//! process-wide activity counters.
//!
//! Trust records and validation belong to realization's trust-model, not this
//! host utility. Timing phase collection is disabled by default and the CLI
//! owns its stderr presentation. Process-wide activity counters are separate
//! diagnostic measurements.

pub mod compile_timings;
