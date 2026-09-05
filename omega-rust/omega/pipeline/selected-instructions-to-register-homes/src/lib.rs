#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Register assignment after selected-program optimization.
//!
//! The preceding X-to-X stage owns selected-lowering rewrites and reusable
//! selected-program analyses. This phase consumes its current program and proof,
//! assigns homes, and handles pressure recovery without rerunning that phase.

mod assignment;
mod output;
mod preservation;
mod rewrites;
#[cfg(test)]
mod selected_rewrite_tests;

pub use assignment::*;
pub use output::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource,
    RetainedAllocation,
};
pub use preservation::*;
pub use register_model::*;
pub use rewrites::*;
pub use selected_instructions_to_selected_instructions::*;
