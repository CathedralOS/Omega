#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected-program analysis, rewriting, and register assignment.
//!
//! Analyses, pre-allocation rewrites, and home assignment share one phase owner.
//! Rewrites invalidate allocation facts and independently replay their replacements.
//! Rule-specific implementation modules are not separate pipeline stages.

mod analyses;
mod assignment;
mod output;
mod rewrites;

pub use analyses::*;
pub use assignment::*;
pub use output::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource,
    RetainedAllocation,
};
pub use rewrites::*;
