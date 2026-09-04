use crate::{
    StagedOptimizedPostAllocationMachinePlan, StagedOptimizedResolvedSelectedFormLayout,
    StagedOptimizedSelectedFormEncoding, ValidatedWholeFunctionExitContract,
    stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_resolved_selected_form_layout, stage_whole_function_exit_contract,
    validate_optimized_post_allocation_machine_plan_custody,
};

use super::custody::receipt;
use super::manifest::expected_manifest;
use super::model::{
    AllocationRecoveryFunctionRelativeRealizationError,
    StagedAllocationRecoveryFunctionRelativeRealization,
};
use super::selection::validate_phase_selection;
use omega_selected_instructions_to_register_homes::RetainedAllocation;

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

fn build_for_selected<S: omega_regalloc::ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
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
