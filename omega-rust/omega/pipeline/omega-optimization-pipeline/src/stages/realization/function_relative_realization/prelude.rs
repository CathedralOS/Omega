pub(super) use std::fmt::Write;

pub(super) use omega_optimization_core::{
    FunctionRelativeOptimizationRealizationManifestIdentity, Optimization,
    OptimizationExecutionPhase, OptimizationSelectionIdentity, OptimizationSelections,
    OptimizationWorkBudget, PostAllocationOptimizationManifestIdentity,
    PrePhysicalOptimizationManifestIdentity, SelectedLoweringOptimizationCompletionIdentity,
};
pub(super) use omega_regalloc::ValidatedSelectedAnalysis;
pub(super) use omega_selected_instructions::SelectedInstructionPlanIdentity;
pub(super) use omega_target::{Architecture, NativeTarget, ObjectFormat};

pub(super) use crate::{
    AllocatedCalleeSavedRequirementIdentity, AllocatedCalleeSavedRequirementPolicy,
    NonAuthoritativeCalleeSaveStorageIdentity, NonAuthoritativeCalleeSaveStoragePolicy,
    OptimizedPostAllocationMachineOptimizationError, OptimizedPostAllocationMachinePipelineError,
    OptimizedResolvedSelectedFormLayoutError, OptimizedSelectedFormEncodingError,
    OptimizedX86BranchRelaxationError, PostAllocationMachineOptimizationCustody,
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity, SelectedFunctionLayoutPolicy,
    StagedOptimizedPostAllocationMachineCustodyReceipt,
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomeCustodyReceipt, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, StagedOptimizedX86BranchRelaxation,
    TargetFrameLayoutIdentity, TargetFrameLayoutPolicy, TargetFrameProtocolEncodingIdentity,
    TargetFrameProtocolEncodingPolicy, ValidatedAllocatedCalleeSavedRequirements,
    ValidatedNonAuthoritativeCalleeSaveStorage, ValidatedTargetFrameLayout,
    ValidatedTargetFrameProtocolEncoding, ValidatedWholeFunctionExitContract,
    WholeFunctionExitContractError, WholeFunctionExitContractIdentity, X86BranchRelaxationIdentity,
    stage_allocated_callee_saved_requirements, stage_non_authoritative_callee_save_storage,
    stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_post_allocation_machine_plan, stage_optimized_resolved_selected_form_layout,
    stage_optimized_x86_branch_relaxation, stage_target_frame_layout,
    stage_target_frame_protocol_encoding, stage_whole_function_exit_contract,
    stage_whole_function_exit_contract_after_x86_branch_relaxation,
    stage_whole_function_exit_contract_with_frame, validate_allocated_callee_saved_requirements,
    validate_non_authoritative_callee_save_storage,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_resolved_selected_form_layout, validate_optimized_x86_branch_relaxation,
    validate_target_frame_layout, validate_target_frame_protocol_encoding,
    validate_whole_function_exit_contract,
    validate_whole_function_exit_contract_after_x86_branch_relaxation,
    validate_whole_function_exit_contract_with_frame,
};
