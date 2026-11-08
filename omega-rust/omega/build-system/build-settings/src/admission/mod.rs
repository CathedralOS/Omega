//! Pre-effect semantic and package admission for the Build program.
//! `build_evaluation.rs` sequences checking before filesystem preparation;
//! `program.rs` then binds the exact entry and host grants to
//! the checker-transformed program, without preparing another copy.

pub(crate) mod checking;
pub(crate) mod program;
pub(crate) mod provider_execution;
mod selected_constants;
pub(crate) mod target_helpers;
