//! One typed post-allocation exit join, with orthogonal optional frame evidence.

use super::{
    ValidatedWholeFunctionExitContract, WholeFunctionExitContractError,
    stage_whole_function_exit_contract_with_post_allocation_machine_optimization_and_frame,
    validation, validation_rules,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use post_allocation_machine_to_post_allocation_machine::StagedOptimizedPostAllocationMachineOptimization;
use post_allocation_machine_to_selected_form_encoding::StagedOptimizedSelectedFormEncoding;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_form_encoding_to_resolved_layout::{
    StagedOptimizedResolvedSelectedFormLayout,
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
};

/// Replay the canonical typed post-allocation join and reject detached rule,
/// encoding, layout, source-machine, or exit-contract custody.
pub fn validate_whole_function_exit_contract_with_post_allocation_machine_optimization_and_frame<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    frame: Option<(
        &crate::frame_layout::ValidatedTargetFrameLayout,
        &crate::ValidatedTargetFrameProtocolEncoding,
    )>,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        encoding,
        Some(optimization),
        layout,
    )
    .map_err(WholeFunctionExitContractError::Layout)?;
    let layout_custody = validation_rules::post_allocation_layout_custody(
        machine,
        encoding,
        layout.program(),
        optimization,
    )?;
    validation::validate(
        selected,
        machine,
        physical,
        encoding,
        layout.program(),
        layout_custody,
        frame,
        contract.contract(),
    )
}
/// Frameless convenience for the same typed join; it grants no frame exemption.
pub fn stage_whole_function_exit_contract_with_post_allocation_machine_optimization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<ValidatedWholeFunctionExitContract, WholeFunctionExitContractError> {
    stage_whole_function_exit_contract_with_post_allocation_machine_optimization_and_frame(
        selected,
        machine,
        physical,
        encoding,
        optimization,
        layout,
        None,
    )
}

/// Replay the frameless convenience through the same independent checker.
pub fn validate_whole_function_exit_contract_with_post_allocation_machine_optimization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    optimization: &StagedOptimizedPostAllocationMachineOptimization,
    layout: &StagedOptimizedResolvedSelectedFormLayout,
    contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), WholeFunctionExitContractError> {
    validate_whole_function_exit_contract_with_post_allocation_machine_optimization_and_frame(
        selected,
        machine,
        physical,
        encoding,
        optimization,
        layout,
        None,
        contract,
    )
}
