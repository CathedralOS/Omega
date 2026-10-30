#![forbid(unsafe_code)]

//! Target machine-code programs and their independently replayable records.
//!
//! Start at [`machine_code::MachineCodePlan`]. Its subordinate owners separate
//! functions, calls, storage, control flow, ownership, and boundary evidence.
//! Data construction alone does not grant execution or publication authority.

mod codec_bytes;
/// The bounded byte cursor every wire decoder reads through.
pub(crate) use optimization_core::cursor;

pub(crate) use optimization_core::digest_encoder;
pub mod machine_code;
/// The crate facade exposes exactly the roster `machine_code` owns.
pub use machine_code::*;
