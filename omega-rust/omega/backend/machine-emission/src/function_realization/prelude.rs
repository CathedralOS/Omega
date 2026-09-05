pub(super) use std::fmt::Write;

pub(super) use optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, Optimization,
    OptimizationExecutionPhase, OptimizationSelectionIdentity, OptimizationSelections,
    OptimizationWorkBudget, PostAllocationOptimizationManifestIdentity,
    PrePhysicalOptimizationManifestIdentity, SelectedLoweringOptimizationCompletionIdentity,
};
pub(super) use selected_instructions::SelectedInstructionPlanIdentity;
pub(super) use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;
pub(super) use target::{Architecture, NativeTarget, ObjectFormat};

pub(super) use crate::frame_layout::{
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePolicy,
    ValidatedNonAuthoritativeCalleeSaveStorage, stage_non_authoritative_callee_save_storage,
    validate_non_authoritative_callee_save_storage,
};
pub(super) use crate::frame_layout::{
    TargetFrameLayoutPolicy, ValidatedTargetFrameLayout, stage_target_frame_layout,
    validate_target_frame_layout,
};
pub(super) use crate::{
    TargetFrameProtocolEncodingPolicy, ValidatedTargetFrameProtocolEncoding,
    stage_target_frame_protocol_encoding, validate_target_frame_protocol_encoding,
};
pub(super) use crate::{
    ValidatedWholeFunctionExitContract, WholeFunctionExitContractError,
    stage_whole_function_exit_contract,
    stage_whole_function_exit_contract_after_x86_branch_relaxation,
    stage_whole_function_exit_contract_with_frame, validate_whole_function_exit_contract,
    validate_whole_function_exit_contract_after_x86_branch_relaxation,
    validate_whole_function_exit_contract_with_frame,
};
pub(super) use machine_code::{
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity, SelectedFunctionLayoutPolicy,
    TargetFrameLayoutIdentity, TargetFrameProtocolEncodingIdentity,
    WholeFunctionExitContractIdentity, X86BranchRelaxationIdentity,
};
pub(super) use physical_instructions::PostAllocationMachineOptimizationCustody;
pub(super) use post_allocation_machine_to_post_allocation_machine::{
    OptimizedPostAllocationMachineOptimizationError,
    StagedOptimizedPostAllocationMachineOptimization,
};
pub(super) use post_allocation_machine_to_selected_form_encoding::{
    OptimizedSelectedFormEncodingError, StagedOptimizedSelectedFormEncoding,
    stage_optimized_layout_independent_selected_form_encoding,
    validate_optimized_layout_independent_selected_form_encoding,
};
pub(super) use register_homes_to_post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    validate_optimized_post_allocation_machine_plan_custody,
};
pub(super) use selected_form_encoding_to_resolved_layout::{
    OptimizedResolvedSelectedFormLayoutError, OptimizedX86BranchRelaxationError,
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedX86BranchRelaxation,
    stage_optimized_resolved_selected_form_layout, stage_optimized_x86_branch_relaxation,
    validate_optimized_resolved_selected_form_layout, validate_optimized_x86_branch_relaxation,
};
pub(super) use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPolicy,
    ValidatedAllocatedCalleeSavedRequirements, stage_allocated_callee_saved_requirements,
    validate_allocated_callee_saved_requirements,
};
pub(super) use selected_instructions_to_register_homes::{
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomeCustodyReceipt,
};
