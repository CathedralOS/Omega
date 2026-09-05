use crate::{ValidatedWholeFunctionExitContract, stage_whole_function_exit_contract};
use post_allocation_machine_to_selected_form_encoding::{
    StagedOptimizedSelectedFormEncoding, stage_optimized_layout_independent_selected_form_encoding,
};
use register_homes_to_post_allocation_machine::{
    StagedOptimizedPostAllocationMachinePlan,
    validate_optimized_post_allocation_machine_plan_custody,
};
use selected_form_encoding_to_resolved_layout::{
    StagedOptimizedResolvedSelectedFormLayout, stage_optimized_resolved_selected_form_layout,
};

use super::custody::receipt;
use super::manifest::expected_manifest;
use super::model::{
    AllocationRecoveryFunctionRelativeRealizationError,
    StagedAllocationRecoveryFunctionRelativeRealization,
};
use super::selection::validate_phase_selection;
use selected_instructions_to_register_homes::RetainedAllocation;

pub(super) fn construct(
    allocation: RetainedAllocation,
    machine: StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedAllocationRecoveryFunctionRelativeRealization,
    AllocationRecoveryFunctionRelativeRealizationError,
> {
    let current = allocation.current();
    validate_phase_selection(&current)?;
    validate_optimized_post_allocation_machine_plan_custody(&current, &machine)
        .map_err(AllocationRecoveryFunctionRelativeRealizationError::Machine)?;
    let (encoding, layout, exit_contract) = build_for_selected(
        current.selected(),
        &machine,
        current.register_environment().physical(),
    )?;
    let manifest = expected_manifest(&current, &machine, &encoding, &layout, &exit_contract)?;
    let custody = receipt(
        current.evidence().clone(),
        &machine,
        &encoding,
        &layout,
        &exit_contract,
        &manifest,
    );
    Ok(StagedAllocationRecoveryFunctionRelativeRealization {
        allocation,
        machine,
        encoding,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}

fn build_for_selected<S: selected_instructions_to_register_homes::ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &register_model::ValidatedPhysicalRegisterModel,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        ValidatedWholeFunctionExitContract,
    ),
    AllocationRecoveryFunctionRelativeRealizationError,
> {
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, machine, physical)
            .map_err(AllocationRecoveryFunctionRelativeRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, machine, physical, &encoding)
            .map_err(AllocationRecoveryFunctionRelativeRealizationError::Layout)?;
    let exit = stage_whole_function_exit_contract(selected, machine, physical, &encoding, &layout)
        .map_err(AllocationRecoveryFunctionRelativeRealizationError::ExitContract)?;
    Ok((encoding, layout, exit))
}
