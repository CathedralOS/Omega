#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected-program rewrites before register assignment.
//!
//! Rewrites operate on the selected CFG and reconstruct the analyses they
//! invalidate. Register assignment consumes the resulting current program.

mod analyses;
mod execution;
mod output;
mod rewrites;

use register_homes::{
    AllocationLegalityIdentity, AllocationLegalityPlan, AllocatorAvailabilityIdentity,
    AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy, EntryFixedViewTransition,
    FixedPrecoloredHomeDomainId, FixedPrecoloredInterval, FixedPrecoloredIntervalPlan,
    FixedPrecoloredIntervalPlanIdentity, FixedPrecoloredIntervalPolicy,
    FixedPrecoloredRegisterSplitRequirements, FixedPrecoloredSegmentHomePlan,
    FixedPrecoloredSegmentHomePlanIdentity, FixedPrecoloredSegmentHomePolicy,
    FixedPrecoloredSourceFragmentRequirements, FixedPrecoloredSourceSegment,
    FixedPrecoloredSourceSegmentHome, FixedPrecoloredSourceSegmentId,
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementPlanIdentity, FixedPrecoloredSplitRequirementPolicy,
    FunctionAllocationLegality, FunctionFixedPrecoloredIntervals,
    FunctionFixedPrecoloredSegmentHomes, FunctionFixedPrecoloredSplitRequirements,
    FunctionRecoveryClassification, FunctionSpillChoices, NoAdmittedRecoveryReason,
    PressureContender, PressureRecoveryClassification, PressureResident, RecoveryClassification,
    RecoveryClassificationIdentity, RecoveryClassificationPlan, RecoveryClassificationPolicy,
    RecoveryFutureUse, RecoveryVictimRole, RegisterClassAvailability, SpillChoice,
    SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy, VirtualEarlyClobberPointLegality,
    VirtualPointLegality, VirtualRegisterAllocationLegality, allocation_legality_identity,
    allocator_availability_identity, fixed_precolored_interval_plan_identity,
    fixed_precolored_segment_home_plan_identity, fixed_precolored_split_requirement_plan_identity,
    recovery_classification_identity, spill_choice_identity,
};

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
