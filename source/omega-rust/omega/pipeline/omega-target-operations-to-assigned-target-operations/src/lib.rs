#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Transitional baseline target-operation assignment.
//!
//! Enter `assignment/mod.rs` for plan coordination, then descend through
//! function routing, structural carriers, placement, control, and expression
//! assignment.

mod assignment;
mod model;

pub use assignment::assign_registers;
pub use model::AssignmentError;

#[cfg(test)]
mod tests;
