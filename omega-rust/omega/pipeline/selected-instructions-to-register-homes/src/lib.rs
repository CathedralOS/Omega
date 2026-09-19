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
//!   `stage_register_allocation` does not yet call.
//!
//! Every name this crate owns is re-exported below from the module that owns
//! it, so the root reads as a map. The `register_model` and
//! `selected_instructions_to_selected_instructions` vocabularies are upstream
//! representation crates this stage consumes; they are re-exported whole.

mod assignment;
mod output;
mod preservation;
mod register_allocation;
mod rewrites;
mod unsequenced_spill_stages;

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
    project_post_allocation_optimization_manifest,
    project_post_allocation_optimization_manifest_after_selected_lowering,
    validate_post_allocation_optimization_manifest,
    validate_post_allocation_optimization_manifest_after_selected_lowering,
};
pub use assignment::recovery::{
    stage_active_resident_register_allocation, stage_fixed_view_register_allocation,
    stage_leaf_local_fixed_view_register_allocation,
    stage_leaf_local_fixed_view_register_allocation_composing,
    stage_shared_entry_fixed_view_register_allocation,
};
pub use assignment::runtime_spill::RuntimeSpillAllocationError;
pub use assignment::transformed::{
    OptimizedPostLiteralFoldHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomesAfterLiteralFolds,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
    stage_optimized_register_homes_after_literal_folds,
    stage_optimized_register_homes_after_selected_lowering,
    validate_optimized_register_home_after_literal_fold_custody,
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
    OptimizedPostSelectedLoweringHomeCustodyFieldForTest,
};
#[cfg(feature = "test-support")]
pub use rewrites::{
    OptimizedActiveResidentRematerializationCustodyFieldForTest,
    OptimizedActiveResidentRematerializationPressureCustodyFieldForTest,
    corrupt_active_resident_rematerialization_custody_for_test,
    corrupt_active_resident_rematerialization_pressure_custody_for_test,
};
pub use rewrites::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedActiveResidentRematerializationPressure,
    StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
    complete_optimized_active_resident_rematerialization,
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
pub use unsequenced_spill_stages::abstract_spill_access_constraints::{
    AbstractSpillAccessConstraintError, AbstractSpillAccessConstraintPlan,
    AbstractSpillAccessConstraintPlanIdentity, AbstractSpillAccessConstraintPolicy,
    AbstractSpillAccessConstraintReceipt, AbstractSpillAccessDependency,
    AbstractSpillAccessDependencyReason, AbstractSpillAccessKind, AbstractSpillAccessPlacement,
    FunctionAbstractSpillAccessConstraints, ValidatedAbstractSpillAccessConstraints,
    abstract_spill_access_constraint_plan_identity, constrain_abstract_spill_accesses,
    validate_abstract_spill_access_constraints,
};
pub use unsequenced_spill_stages::abstract_spill_insertion::{
    AbstractSpillAreaReload, AbstractSpillAreaSlot, AbstractSpillAreaStore,
    AbstractSpillInsertionAction, AbstractSpillInsertionError, AbstractSpillInsertionIdentity,
    AbstractSpillInsertionPlan, AbstractSpillInsertionPolicy, AbstractSpillInsertionReceipt,
    FunctionAbstractSpillInsertion, ValidatedAbstractSpillInsertion,
    abstract_spill_insertion_identity, schedule_abstract_spill_insertion,
    validate_abstract_spill_insertion,
};
pub use unsequenced_spill_stages::abstract_spill_memory_effects::{
    AbstractSpillMemoryEffect, AbstractSpillMemoryEffectError, AbstractSpillMemoryEffectPlan,
    AbstractSpillMemoryEffectPlanIdentity, AbstractSpillMemoryEffectPolicy,
    AbstractSpillMemoryEffectReceipt, FunctionAbstractSpillMemoryEffects,
    ValidatedAbstractSpillMemoryEffects, abstract_spill_memory_effect_plan_identity,
    derive_abstract_spill_memory_effects, validate_abstract_spill_memory_effects,
};
pub use unsequenced_spill_stages::generalized_reload_value_homes::{
    FunctionGeneralizedReloadValueHomes, GeneralizedReloadCoexistingHome,
    GeneralizedReloadCoexistingValue, GeneralizedReloadValueHomeAssignment,
    GeneralizedReloadValueHomeError, GeneralizedReloadValueHomeIdentity,
    GeneralizedReloadValueHomeOutcome, GeneralizedReloadValueHomePlan,
    GeneralizedReloadValueHomePolicy, GeneralizedReloadValueHomeReceipt,
    GeneralizedReloadValuePressure, ValidatedGeneralizedReloadValueHomes,
    assign_generalized_reload_value_homes, generalized_reload_value_home_identity,
    validate_generalized_reload_value_homes,
};
pub use unsequenced_spill_stages::generalized_spill_insertion::{
    FunctionGeneralizedSpillInsertion, GeneralizedSpillActionId, GeneralizedSpillActionSource,
    GeneralizedSpillEvent, GeneralizedSpillInsertionError, GeneralizedSpillInsertionIdentity,
    GeneralizedSpillInsertionPlan, GeneralizedSpillInsertionPolicy,
    GeneralizedSpillInsertionReceipt, GeneralizedSpillSlot, ValidatedGeneralizedSpillInsertion,
    generalized_spill_insertion_identity, schedule_generalized_spill_insertion,
    validate_generalized_spill_insertion,
};
pub use unsequenced_spill_stages::generalized_spill_recovery_actions::{
    GeneralizedSpillRecoveryActionError, GeneralizedSpillRecoveryActionIdentity,
    GeneralizedSpillRecoveryActionPlan, GeneralizedSpillRecoveryActionPolicy,
    GeneralizedSpillRecoveryActionReceipt, GeneralizedSpillRecoveryLogicalAction,
    GeneralizedSpillRecoveryLogicalReload, GeneralizedSpillRecoveryLogicalStorage,
    GeneralizedSpillRecoveryLogicalStore, GeneralizedSpillRecoveryLogicalUseRewrite,
    GeneralizedSpillRecoveryVictim, ValidatedGeneralizedSpillRecoveryActions,
    generalized_spill_recovery_action_identity, plan_generalized_original_spill_recovery_actions,
    plan_generalized_spill_recovery_actions, validate_generalized_original_spill_recovery_actions,
    validate_generalized_spill_recovery_actions,
};
pub use unsequenced_spill_stages::generalized_spill_recovery_choice::{
    GeneralizedSpillRecoveryChoiceError, GeneralizedSpillRecoveryChoiceIdentity,
    GeneralizedSpillRecoveryChoicePlan, GeneralizedSpillRecoveryChoicePolicy,
    GeneralizedSpillRecoveryChoiceReceipt, GeneralizedSpillRecoveryContender,
    GeneralizedSpillRecoveryResident, GeneralizedSpillRecoveryVictimChoice,
    ValidatedGeneralizedSpillRecoveryChoices, choose_generalized_spill_recovery_victims,
    generalized_spill_recovery_choice_identity, validate_generalized_spill_recovery_choices,
};
pub use unsequenced_spill_stages::generalized_spill_recovery_worklist::{
    FunctionGeneralizedSpillRecoveryWorklist, GeneralizedSpillRecoveryWorkItem,
    GeneralizedSpillRecoveryWorkItemId, GeneralizedSpillRecoveryWorklistError,
    GeneralizedSpillRecoveryWorklistIdentity, GeneralizedSpillRecoveryWorklistPlan,
    GeneralizedSpillRecoveryWorklistPolicy, GeneralizedSpillRecoveryWorklistReceipt,
    ValidatedGeneralizedSpillRecoveryWorklist, generalized_spill_recovery_worklist_identity,
    seed_generalized_spill_recovery_worklist, validate_generalized_spill_recovery_worklist,
};
pub use unsequenced_spill_stages::logical_spill_operations::{
    FunctionLogicalSpillOperations, LogicalReloadValueId, LogicalSpillAction,
    LogicalSpillOperationDecodeError, LogicalSpillOperationError, LogicalSpillOperationIdentity,
    LogicalSpillOperationPlan, LogicalSpillOperationPolicy, LogicalSpillOperationValidationReceipt,
    LogicalSpillReload, LogicalSpillStorage, LogicalSpillStorageClass, LogicalSpillStorageId,
    LogicalSpillStore, LogicalSpillUseRewrite, ValidatedLogicalSpillOperations,
    logical_spill_operation_identity, plan_logical_spill_operations,
    validate_logical_spill_operations,
};
pub use unsequenced_spill_stages::recursive_reload_value_homes::{
    FunctionRecursiveReloadValueHomes, RecursiveReloadCoexistingHome,
    RecursiveReloadCoexistingValue, RecursiveReloadValueHomeAssignment,
    RecursiveReloadValueHomeError, RecursiveReloadValueHomeIdentity, RecursiveReloadValueHomePlan,
    RecursiveReloadValueHomePolicy, RecursiveReloadValueHomeReceipt,
    ValidatedRecursiveReloadValueHomes, assign_recursive_reload_value_homes,
    recursive_reload_value_home_identity, validate_recursive_reload_value_homes,
};
pub use unsequenced_spill_stages::recursive_spill_insertion::{
    FunctionRecursiveSpillInsertion, RecursiveSpillActionSource, RecursiveSpillEvent,
    RecursiveSpillInsertionError, RecursiveSpillInsertionIdentity, RecursiveSpillInsertionPlan,
    RecursiveSpillInsertionPolicy, RecursiveSpillInsertionReceipt, RecursiveSpillSlot,
    RecursiveSpillStoredValue, ValidatedRecursiveSpillInsertion,
    recursive_spill_insertion_identity, schedule_recursive_spill_insertion,
    validate_recursive_spill_insertion,
};
pub use unsequenced_spill_stages::reload_value_homes::{
    FunctionReloadValueHomes, ReloadCoexistingHome, ReloadValueHomeAssignment,
    ReloadValueHomeError, ReloadValueHomeIdentity, ReloadValueHomePlan, ReloadValueHomePolicy,
    ReloadValueHomeReceipt, ValidatedReloadValueHomes, assign_reload_value_homes,
    reload_value_home_identity, validate_reload_value_homes,
};
pub use unsequenced_spill_stages::spill_pseudo_instructions::{
    FunctionHomedSpillPseudoInstructions, FunctionSpillPseudoInstructions,
    HomedSpillPseudoInstruction, HomedSpillPseudoInstructionError, HomedSpillPseudoInstructionPlan,
    HomedSpillPseudoInstructionPlanIdentity, HomedSpillPseudoInstructionPolicy,
    HomedSpillPseudoInstructionReceipt, SpillPseudoInstruction, SpillPseudoInstructionError,
    SpillPseudoInstructionId, SpillPseudoInstructionPlan, SpillPseudoInstructionPlanIdentity,
    SpillPseudoInstructionPolicy, SpillPseudoInstructionReceipt, SpillPseudoOperandRewrite,
    SpillPseudoStorage, SpillPseudoStoredValue, ValidatedHomedSpillPseudoInstructions,
    ValidatedSpillPseudoInstructions, homed_spill_pseudo_instruction_plan_identity,
    lower_homed_recursive_spill_pseudos, lower_recursive_spill_pseudos,
    spill_pseudo_instruction_plan_identity, validate_homed_spill_pseudo_instructions,
    validate_spill_pseudo_instructions,
};
pub use unsequenced_spill_stages::spill_recovery_actions::{
    SpillRecoveryActionError, SpillRecoveryActionIdentity, SpillRecoveryActionPlan,
    SpillRecoveryActionPolicy, SpillRecoveryActionReceipt, SpillRecoveryLogicalAction,
    SpillRecoveryLogicalReload, SpillRecoveryLogicalReloadId, SpillRecoveryLogicalStorage,
    SpillRecoveryLogicalStorageId, SpillRecoveryLogicalStore, SpillRecoveryLogicalUseRewrite,
    ValidatedSpillRecoveryActions, plan_spill_recovery_actions, spill_recovery_action_identity,
    validate_spill_recovery_actions,
};
pub use unsequenced_spill_stages::spill_recovery_choice::{
    SpillRecoveryChoiceError, SpillRecoveryChoiceIdentity, SpillRecoveryChoicePlan,
    SpillRecoveryChoicePolicy, SpillRecoveryChoiceReceipt, SpillRecoveryContender,
    SpillRecoveryResident, SpillRecoveryVictimChoice, ValidatedSpillRecoveryChoices,
    choose_spill_recovery_victims, spill_recovery_choice_identity, validate_spill_recovery_choices,
};
pub use unsequenced_spill_stages::spill_recovery_worklist::{
    SpillRecoveryEpoch, SpillRecoveryWorkItem, SpillRecoveryWorklistError,
    SpillRecoveryWorklistIdentity, SpillRecoveryWorklistPlan, SpillRecoveryWorklistPolicy,
    SpillRecoveryWorklistReceipt, ValidatedSpillRecoveryWorklist, seed_spill_recovery_worklist,
    spill_recovery_worklist_identity, validate_spill_recovery_worklist,
};
pub use unsequenced_spill_stages::stack_slot_coloring::{
    FunctionStackSlotColoring, StackSlotAssignment, StackSlotColoringDecodeError,
    StackSlotColoringError, StackSlotColoringIdentity, StackSlotColoringPlan,
    StackSlotColoringPolicy, StackSlotColoringValidationReceipt, ValidatedStackSlotColoring,
    color_logical_spill_stack_slots, stack_slot_coloring_identity, validate_stack_slot_coloring,
};
pub use unsequenced_spill_stages::synthetic_reload_values::{
    FunctionSyntheticReloadValues, SyntheticReloadValueBinding, SyntheticReloadValueError,
    SyntheticReloadValueId, SyntheticReloadValuePlan, SyntheticReloadValuePlanIdentity,
    SyntheticReloadValuePolicy, SyntheticReloadValueReceipt, ValidatedSyntheticReloadValues,
    bind_synthetic_reload_values, synthetic_reload_value_plan_identity,
    validate_synthetic_reload_values,
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
