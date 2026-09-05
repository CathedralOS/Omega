#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected-form encoding to resolved layout.
//!
//! Baseline layout and explicit function-relative relaxation are separate phase
//! entrances. They share the private layout-admission boundary, not a producer
//! algorithm: each entrance independently checks its result before publication.
//! Current layout data and content identity belong to machine-code.

mod resolved_selected_form_layout;
mod x86_branch_relaxation;

pub use resolved_selected_form_layout::*;
pub use x86_branch_relaxation::*;
