//! Current selected program, allocation constraints, and physical-home assignments.
//!
//! `constraints` owns allocator availability and fixed-register requirements.
//! `storage` owns per-function assignments; `recovery` records spill choices
//! and recovery eligibility. These are raw records, not validated authority.
//! `preservation` records allocation-visible ABI save requirements, not frames
//! or executable save/restore decisions.
//! `identity` and `codec` preserve the canonical version-6 artifact contract.
//! This representation contains no prior pipeline stages or validated authority.

pub mod codec;
pub mod constraints;
pub mod identity;
pub mod logical_spill_operations;
pub mod post_allocation_manifest;
pub mod preservation;
pub mod recovery;
pub mod stack_slot_coloring;
pub mod storage;
pub mod view;

pub use codec::RegisterHomeDecodeError;
pub use constraints::{
    AllocationLegalityIdentity, AllocationLegalityPlan, AllocatorAvailabilityDecodeError,
    AllocatorAvailabilityIdentity, AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy,
    EntryFixedViewTransition, FixedPrecoloredInterval, FixedPrecoloredIntervalPlan,
    FixedPrecoloredIntervalPlanIdentity, FixedPrecoloredIntervalPolicy,
    FixedPrecoloredRegisterSplitRequirements, FixedPrecoloredSourceFragmentRequirements,
    FixedPrecoloredSourceSegment, FixedPrecoloredSourceSegmentId,
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementPlanIdentity, FixedPrecoloredSplitRequirementPolicy,
    FunctionAllocationLegality, FunctionFixedPrecoloredIntervals,
    FunctionFixedPrecoloredSplitRequirements, RegisterClassAvailability,
    VirtualEarlyClobberPointLegality, VirtualPointLegality, VirtualRegisterAllocationLegality,
    allocation_legality, allocation_legality_identity, allocator_availability,
    allocator_availability_identity, fixed_precolored_interval_plan_identity,
    fixed_precolored_intervals, fixed_precolored_split_requirement_plan_identity,
    fixed_precolored_split_requirements,
};
pub use identity::{
    AbstractSpillAccessConstraintPlanIdentity, RegisterHomeIdentity, register_home_identity,
};
pub use logical_spill_operations::{
    FunctionLogicalSpillOperations, LogicalReloadValueId, LogicalSpillAction,
    LogicalSpillOperationDecodeError, LogicalSpillOperationIdentity, LogicalSpillOperationPlan,
    LogicalSpillOperationPolicy, LogicalSpillReload, LogicalSpillStorage, LogicalSpillStorageClass,
    LogicalSpillStorageId, LogicalSpillStore, LogicalSpillUseRewrite,
    logical_spill_operation_identity,
};
pub use post_allocation_manifest::{
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestDecodeError, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
    PostAllocationUnavailableData,
};
pub use preservation::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirementPolicy, AllocatedCalleeSavedUnitRequirement,
    CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
    encode_callee_saved_modification_witness_identity,
};
pub use recovery::{
    FixedViewCopy, FixedViewCopyDecodeError, FixedViewCopyDestination, FixedViewCopyPlan,
    FixedViewCopyPolicy, FixedViewCopySourceEvidence, FunctionRecoveryClassification,
    FunctionSpillChoices, NoAdmittedRecoveryReason, PressureContender,
    PressureRecoveryClassification, PressureResident, RecoveryClassification,
    RecoveryClassificationDecodeError, RecoveryClassificationIdentity, RecoveryClassificationPlan,
    RecoveryClassificationPolicy, RecoveryFutureUse, RecoveryVictimRole, SpillChoice,
    SpillChoiceDecodeError, SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy,
    classification, fixed_view_copy, fixed_view_copy_identity, recovery_classification_identity,
    spill_choice, spill_choice_identity,
};
pub use stack_slot_coloring::{
    FunctionStackSlotColoring, StackSlotAssignment, StackSlotColoringDecodeError,
    StackSlotColoringIdentity, StackSlotColoringPlan, StackSlotColoringPolicy,
    stack_slot_coloring_identity,
};
pub use storage::{
    FixedPrecoloredHomeDomainId, FixedPrecoloredSegmentHomePlan,
    FixedPrecoloredSegmentHomePlanIdentity, FixedPrecoloredSegmentHomePolicy,
    FixedPrecoloredSourceSegmentHome, FunctionFixedPrecoloredSegmentHomes, FunctionRegisterHomes,
    RegisterHomePlan, VirtualRegisterHome, fixed_precolored_segment_home_plan_identity,
    fixed_precolored_segment_homes,
};
pub use view::AllocatedProgramRef;

/// One current allocated program, independent of the route that produced it.
/// Immutable artifacts may be shared with replay evidence without copying
/// their contents or making execution traverse that evidence. Raw data alone
/// grants no allocation, emission, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatedProgram {
    pub selected: std::sync::Arc<selected_instructions::SelectedInstructionPlan>,
    pub homes: std::sync::Arc<RegisterHomePlan>,
}

#[cfg(test)]
mod tests;
