#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected instructions to register homes.
//!
//! Start at [`stage_register_allocation`] in `register_allocation.rs`. It
//! consumes the preceding selected X-to-X stage's current program and proof,
//! sequences home assignment and admitted pressure recovery, and publishes one
//! [`RetainedAllocation`]. That preceding stage owns selected-lowering rewrites
//! and reusable selected-program analyses; allocation does not rerun it.
//!
//! Module map, in route order:
//! - `register_allocation` owns the entry and its exact route.
//! - `assignment` holds the stages that route sequences: `transformed` homes
//!   after selected lowering, the optional `recovery` rules, `runtime_spill`
//!   pressure recovery, `baseline` homes over `home_assignment`, and the
//!   `post_allocation_manifest` every route publishes.
//! - `rewrites` stages active-resident rematerialization for a recovery rule.
//! - `output` owns current allocation facts and their separate replay evidence.
//! - `preservation` derives allocation-visible callee-saved requirements.
//! - `unsequenced_spill_stages` holds validated spill boundaries that
//!   `stage_register_allocation` does not yet call; they are reachable only
//!   through that module path, so the root's names are the route's names.
//!
//! Every name the route owns is re-exported below from the module that owns
//! it, so the root reads as a map. The `register_model` and
//! `selected_instructions_to_selected_instructions` vocabularies are upstream
//! representation crates this stage consumes; they are re-exported whole.

mod assignment;
mod output;
mod preservation;
mod register_allocation;
mod rewrites;
pub mod unsequenced_spill_stages;

// The entry and its route.
pub use register_allocation::{RegisterAllocationError, stage_register_allocation};

// Stages sequenced by `stage_register_allocation`, in route order.
pub use assignment::baseline::{
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
    stage_optimized_register_homes, stage_optimized_register_homes_after_fixed_view_copies,
    validate_optimized_register_home_after_fixed_view_copy_custody,
    validate_optimized_register_home_custody,
};
pub use assignment::home_assignment::{
    FunctionRegisterHomes, RegisterHomeDecodeError, RegisterHomeError, RegisterHomeIdentity,
    RegisterHomePlan, RegisterHomeValidationReceipt, ValidatedRegisterHomes, VirtualRegisterHome,
    assign_register_homes, register_home_identity, validate_register_homes,
};
pub use assignment::post_allocation_manifest::{
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestDecodeError, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
    PostAllocationUnavailableData, ValidatedPostAllocationOptimizationManifest,
    validate_post_allocation_optimization_manifest,
};
pub use assignment::recovery::{
    stage_active_resident_register_allocation, stage_leaf_local_fixed_view_register_allocation,
    stage_leaf_local_fixed_view_register_allocation_composing,
    stage_shared_entry_fixed_view_register_allocation,
};
pub use assignment::runtime_spill::RuntimeSpillAllocationError;
pub use assignment::transformed::{
    OptimizedPostLiteralFoldHomeCustodyError, OptimizedPostPreAllocationHomeCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyError,
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostPreAllocationHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomesAfterLiteralFolds, StagedOptimizedRegisterHomesAfterPreAllocation,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    stage_optimized_register_homes_after_literal_folds,
    stage_optimized_register_homes_after_pre_allocation,
    stage_optimized_register_homes_after_selected_lowering,
    validate_optimized_register_home_after_literal_fold_custody,
    validate_optimized_register_home_after_pre_allocation_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
};

// Rematerialization staged by the active-resident recovery rule.
#[cfg(feature = "test-support")]
pub use assignment::baseline::{
    OptimizedPostCopyRegisterHomeCustodyFieldForTest, OptimizedRegisterHomeCustodyFieldForTest,
};
#[cfg(feature = "test-support")]
pub use assignment::transformed::{
    OptimizedPostLiteralFoldHomeCustodyFieldForTest,
    OptimizedPostPreAllocationHomeCustodyFieldForTest,
    OptimizedPostSelectedLoweringHomeCustodyFieldForTest,
};
#[cfg(feature = "test-support")]
pub use rewrites::{
    OptimizedActiveResidentRematerializationCustodyFieldForTest,
    OptimizedActiveResidentRematerializationPressureCustodyFieldForTest,
    corrupt_active_resident_rematerialization_custody_for_test,
};
pub use rewrites::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedActiveResidentRematerializationPressure,
    StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
    stage_optimized_active_resident_rematerialization,
    stage_optimized_active_resident_rematerialization_pressure,
    validate_optimized_active_resident_rematerialization,
    validate_optimized_active_resident_rematerialization_pressure,
};

// Current allocation facts and replay evidence.
pub use output::{
    AllocationEvidence, AllocationOutput, AllocationReplayError, AllocationSource,
    RetainedAllocation,
};

// Allocation-visible callee-saved requirements.
pub use preservation::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementIdentity,
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementPolicy,
    AllocatedCalleeSavedRequirementReceipt, AllocatedCalleeSavedUnitRequirement,
    CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
    ValidatedAllocatedCalleeSavedRequirements, allocated_callee_saved_requirement_identity,
    stage_allocated_callee_saved_requirements, validate_allocated_callee_saved_requirements,
};

// Validated spill boundaries not yet sequenced by `stage_register_allocation`.
pub use assignment::logical_spill_operations::{
    FunctionLogicalSpillOperations, LogicalReloadValueId, LogicalSpillAction,
    LogicalSpillOperationDecodeError, LogicalSpillOperationError, LogicalSpillOperationIdentity,
    LogicalSpillOperationPlan, LogicalSpillOperationPolicy, LogicalSpillOperationValidationReceipt,
    LogicalSpillReload, LogicalSpillStorage, LogicalSpillStorageClass, LogicalSpillStorageId,
    LogicalSpillStore, LogicalSpillUseRewrite, ValidatedLogicalSpillOperations,
    logical_spill_operation_identity, plan_logical_spill_operations,
    validate_logical_spill_operations,
};
pub use assignment::stack_slot_coloring::{
    FunctionStackSlotColoring, StackSlotAssignment, StackSlotColoringDecodeError,
    StackSlotColoringError, StackSlotColoringIdentity, StackSlotColoringPlan,
    StackSlotColoringPolicy, StackSlotColoringValidationReceipt, ValidatedStackSlotColoring,
    color_logical_spill_stack_slots, stack_slot_coloring_identity, validate_stack_slot_coloring,
};

// Upstream vocabularies consumed by every stage above.
pub use register_model::*;
pub use selected_instructions_to_selected_instructions::*;

use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FunctionAllocationLegality,
    FunctionSpillChoices, RecoveryClassificationIdentity, RecoveryClassificationPolicy,
    SpillChoiceIdentity, SpillChoicePolicy, VirtualRegisterAllocationLegality,
};
#[cfg(test)]
use register_homes::{
    PressureContender, PressureResident, SpillChoice, VirtualEarlyClobberPointLegality,
    VirtualPointLegality,
};
#[cfg(test)]
use selected_instructions::{
    BlockPointDomain, EarlyClobberConstraint, EarlyClobberUse, EdgeRegisterTransfer,
    LivenessPosition, VirtualFixedConstraint, VirtualOccurrence,
};
use selected_instructions::{
    CopyAffinity, DistinctUseDefTie, FunctionLiveRanges, LiveRangeFragment, LiveRangeIdentity,
    LiveRangePoint, LivenessIdentity, VirtualFixedConstraintSite, VirtualInterference,
    VirtualLiveRange,
};
