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
    stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_post_allocation_machine_plan,
    stage_optimized_post_allocation_machine_plan_after_selected_lowering,
    stage_optimized_resolved_selected_form_layout, stage_optimized_x86_branch_relaxation,
    stage_whole_function_exit_contract,
    stage_whole_function_exit_contract_after_x86_branch_relaxation,
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_post_allocation_machine_plan_after_selected_lowering_custody,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_register_home_after_selected_lowering_custody,
    validate_optimized_register_home_custody, validate_optimized_resolved_selected_form_layout,
    validate_optimized_x86_branch_relaxation, validate_whole_function_exit_contract,
    validate_whole_function_exit_contract_after_x86_branch_relaxation,
    OptimizedActiveResidentRematerializationError, OptimizedPostAllocationMachineOptimizationError,
    OptimizedPostAllocationMachinePipelineError, OptimizedPostCopyRegisterHomeCustodyError,
    OptimizedPostSelectedLoweringHomeCustodyError, OptimizedRegisterHomeCustodyError,
    OptimizedResolvedSelectedFormLayoutError, OptimizedSelectedFormEncodingError,
    OptimizedX86BranchRelaxationError, PostAllocationMachineOptimizationCustody,
    ResolvedSelectedFormLayoutIdentity, SelectedFormEncodingIdentity, SelectedFunctionLayoutPolicy,
    StagedAllocationRecoveryFunctionRelativeSource, StagedAllocationRecoverySourceCustodyReceipt,
    StagedOptimizedPostAllocationMachineCustodyReceipt,
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomes,
    StagedOptimizedRegisterHomesAfterSelectedLowering, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, StagedOptimizedX86BranchRelaxation,
    ValidatedWholeFunctionExitContract, WholeFunctionExitContractError,
    WholeFunctionExitContractIdentity, X86BranchRelaxationIdentity,
};
