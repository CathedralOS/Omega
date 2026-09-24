//! Optimizer module role: stage group. Rule tests by pass family.
//!
//! `fixtures` owns the typed optimization units the families share, one
//! module per pass family; each test imports its fixtures and vocabulary from
//! their owners.

pub(crate) mod fixtures;

mod borrowed_storage_windows;
mod catalog;
mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod global_value_numbering;
mod primitive_locals;
mod proof_check_elision;
mod scalar_arrays;
mod sparse_conditional_constant_propagation;
