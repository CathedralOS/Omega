#![forbid(unsafe_code)]

//! Target machine-code programs and their independently replayable records.
//!
//! Start at [`machine_code::MachineCodePlan`]. Its subordinate owners separate
//! functions, calls, storage, control flow, ownership, and boundary evidence.
//! Data construction alone does not grant execution or publication authority.

pub mod machine_code;
pub use machine_code::*;
