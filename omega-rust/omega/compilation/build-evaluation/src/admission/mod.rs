//! Pre-effect semantic and package admission for the Build program.
//! `build_evaluation.rs` sequences checking before filesystem preparation;
//! `admitted_build_program.rs` then binds the exact entry and host grants to
//! the checker-transformed program, without preparing another copy.

pub(crate) mod checking;
