//! Whole-function exit-contract staging, replay validation, and canonical identity.

mod compute;
mod error;
mod identity;
mod model;
mod stage;
mod validation_rules;

pub use error::*;
pub use model::*;
pub use stage::*;

use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::ValidatedPhysicalRegisterModel;

use crate::{
    StagedOptimizedPostAllocationMachineOptimization, StagedOptimizedPostAllocationMachinePlan,
    StagedOptimizedResolvedSelectedFormLayout, StagedOptimizedSelectedFormEncoding,
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization,
};

/// Establish the canonical whole-function exit contract for one owning typed
/// post-allocation result. The join independently replays resolved layout and
/// binds the exact typed leaf to normalized encoding and layout custody.
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
    let layout_custody =
        validation_rules::post_allocation_layout_custody(machine, encoding, layout, optimization)?;
    let contract = compute::compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        layout_custody,
    )?;
    let validated = ValidatedWholeFunctionExitContract { contract };
    validate_whole_function_exit_contract_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        encoding,
        optimization,
        layout,
        &validated,
    )?;
    Ok(validated)
}

/// Replay the canonical typed post-allocation join and reject detached rule,
/// encoding, layout, source-machine, or exit-contract custody.
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
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        encoding,
        Some(optimization),
        layout,
    )
    .map_err(WholeFunctionExitContractError::Layout)?;
    let layout_custody =
        validation_rules::post_allocation_layout_custody(machine, encoding, layout, optimization)?;
    let replayed = compute::compute(
        selected,
        machine,
        physical,
        encoding,
        layout,
        layout_custody,
    )?;
    if replayed != contract.contract {
        return Err(WholeFunctionExitContractError::ArtifactMismatch);
    }
    Ok(())
}
