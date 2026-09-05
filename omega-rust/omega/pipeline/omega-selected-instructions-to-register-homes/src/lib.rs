#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected-program analysis, rewriting, and register assignment.
//!
//! Analyses, pre-allocation rewrites, and home assignment share one phase owner.
//! Rewrites invalidate allocation facts and independently replay their replacements.
//! Rule-specific implementation modules are not separate pipeline stages.
//! Liveness, live ranges, legality, recovery and assignment are internal steps
//! of this transform, not a second allocator crate behind its public entrance.

mod analyses;
mod assignment;
mod output;
mod preservation;
mod rewrites;

pub use analyses::*;
pub use assignment::*;
pub use omega_register_model::*;
pub use output::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource,
    RetainedAllocation,
};
pub use preservation::*;
pub use rewrites::*;
