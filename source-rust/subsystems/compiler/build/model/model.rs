//! Records that Build shares with the source compiler, the native compiler and
//! the interpreter: evaluation contracts, accounting, work metering and
//! receipts, build-time values, evaluation admission, requested behavior
//! exclusions and optimization requests. Nothing here runs a project Build or
//! names a native target, so both compilers may depend on this crate.

#![forbid(unsafe_code)]

pub mod accounting;
pub mod admission;
pub mod behavior_exclusions;
pub mod contracts;
pub mod optimization_requests;
pub mod receipts;
pub mod values;
pub mod work;
