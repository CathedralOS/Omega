#![forbid(unsafe_code)]

//! Physical register homes as data, independent of the allocating algorithm.
//!
//! Start at [`register_homes`]. Decode and identity checks establish canonical
//! representation only; allocation validity belongs to the independent verifier.

pub mod register_homes;
pub use register_homes::{
    AbstractSpillAccessConstraintPlanIdentity, AllocatedCalleeSavedRequirementIdentity,
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementPolicy,
    AllocatedCalleeSavedUnitRequirement, AllocatedProgram, AllocatedProgramRef,
    AllocationLegalityIdentity, AllocationLegalityPlan, AllocatorAvailabilityDecodeError,
    AllocatorAvailabilityIdentity, AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy,
    CalleeSavedModificationWitness, EntryFixedViewTransition, FixedPrecoloredHomeDomainId,
    FixedPrecoloredInterval, FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredIntervalPolicy, FixedPrecoloredRegisterSplitRequirements,
    FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomePlanIdentity,
    FixedPrecoloredSegmentHomePolicy, FixedPrecoloredSourceFragmentRequirements,
    FixedPrecoloredSourceSegment, FixedPrecoloredSourceSegmentHome, FixedPrecoloredSourceSegmentId,
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementPlanIdentity, FixedPrecoloredSplitRequirementPolicy,
    FixedViewCopy, FixedViewCopyDecodeError, FixedViewCopyDestination, FixedViewCopyPlan,
    FixedViewCopyPolicy, FixedViewCopySourceEvidence, FunctionAllocatedCalleeSavedRequirements,
    FunctionAllocationLegality, FunctionFixedPrecoloredIntervals,
    FunctionFixedPrecoloredSegmentHomes, FunctionFixedPrecoloredSplitRequirements,
    FunctionLogicalSpillOperations, FunctionRecoveryClassification, FunctionRegisterHomes,
    FunctionSpillChoices, FunctionStackSlotColoring, LogicalReloadValueId, LogicalSpillAction,
    LogicalSpillOperationDecodeError, LogicalSpillOperationIdentity, LogicalSpillOperationPlan,
    LogicalSpillOperationPolicy, LogicalSpillReload, LogicalSpillStorage, LogicalSpillStorageClass,
    LogicalSpillStorageId, LogicalSpillStore, LogicalSpillUseRewrite, NoAdmittedRecoveryReason,
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestDecodeError, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
    PostAllocationUnavailableData, PressureContender, PressureRecoveryClassification,
    PressureResident, RecoveryClassification, RecoveryClassificationDecodeError,
    RecoveryClassificationIdentity, RecoveryClassificationPlan, RecoveryClassificationPolicy,
    RecoveryFutureUse, RecoveryVictimRole, RegisterClassAvailability, RegisterHomeDecodeError,
    RegisterHomeIdentity, RegisterHomePlan, SpillChoice, SpillChoiceDecodeError,
    SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy, StackSlotAssignment,
    StackSlotColoringDecodeError, StackSlotColoringIdentity, StackSlotColoringPlan,
    StackSlotColoringPolicy, VirtualEarlyClobberPointLegality, VirtualPointLegality,
    VirtualRegisterAllocationLegality, VirtualRegisterHome, allocation_legality,
    allocation_legality_identity, allocator_availability, allocator_availability_identity,
    classification, codec, constraints, encode_callee_saved_modification_witness_identity,
    fixed_precolored_interval_plan_identity, fixed_precolored_intervals,
    fixed_precolored_segment_home_plan_identity, fixed_precolored_segment_homes,
    fixed_precolored_split_requirement_plan_identity, fixed_precolored_split_requirements,
    fixed_view_copy, fixed_view_copy_identity, identity, logical_spill_operation_identity,
    logical_spill_operations, post_allocation_manifest, preservation, recovery,
    recovery_classification_identity, register_home_identity, spill_choice, spill_choice_identity,
    stack_slot_coloring, stack_slot_coloring_identity, storage, view,
};
