#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected-program rewrites before register assignment.
//!
//! Rewrites operate on the selected CFG and reconstruct the analyses they
//! invalidate. Register assignment consumes the resulting current program.

mod analyses;
mod execution;
mod output;
mod rewrites;

pub use analyses::*;
pub use execution::{optimize_analyzed_selected_instructions, optimize_selected_instructions};
pub use output::{
    SelectedInstructionOptimizationError, SelectedInstructionOptimizationEvidence,
    SelectedInstructionOptimizationOutput,
};
use register_model::*;
pub use rewrites::*;

#[cfg(feature = "test-support")]
pub mod test_support;

use selected_instructions::{
    ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
    BlockLiveness, BlockPointDomain, DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse,
    EdgeRegisterTransfer, FunctionLiveRanges, FunctionLiveness, LiveRangeEdgeConnector,
    LiveRangeFragment, LiveRangeIdentity, LiveRangePlan, LiveRangePoint, LivenessIdentity,
    LivenessPosition, OperandPosition, SuccessorLiveness, VirtualFixedConstraint,
    VirtualFixedConstraintSite, VirtualInterference, VirtualLiveRange, VirtualOccurrence,
    live_range_identity,
};

#[cfg(test)]
use selected_instructions::InstructionLiveness;
