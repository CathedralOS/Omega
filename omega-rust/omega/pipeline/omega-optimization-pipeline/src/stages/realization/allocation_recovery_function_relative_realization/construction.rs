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
use super::source::{
    StagedAllocationRecoveryFunctionRelativeSource, validate_active_resident_source,
    validate_fixed_view_source,
};

pub(super) fn construct(
    source: StagedAllocationRecoveryFunctionRelativeSource,
    machine: StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedAllocationRecoveryFunctionRelativeRealization,
    AllocationRecoveryFunctionRelativeRealizationError,
> {
    source.validate_phase_selection()?;
    let source_custody = match &source {
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) => {
            let custody = validate_fixed_view_source(homes)?;
            validate_optimized_post_allocation_machine_plan_custody(homes, &machine)
                .map_err(AllocationRecoveryFunctionRelativeRealizationError::Machine)?;
            custody
        }
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(
            rematerialization,
        ) => {
            let custody = validate_active_resident_source(rematerialization)?;
            validate_optimized_post_allocation_machine_plan_custody(rematerialization, &machine)
                .map_err(AllocationRecoveryFunctionRelativeRealizationError::Machine)?;
            custody
        }
    };
    let (encoding, layout, exit_contract) = build_physical_artifacts(&source, &machine)?;
    let manifest = expected_manifest(&source, &machine, &encoding, &layout, &exit_contract)?;
    let custody = receipt(
        source_custody,
        &machine,
        &encoding,
        &layout,
        &exit_contract,
        &manifest,
    );
    Ok(StagedAllocationRecoveryFunctionRelativeRealization {
        source,
        machine,
        encoding,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}

fn build_physical_artifacts(
    source: &StagedAllocationRecoveryFunctionRelativeSource,
    machine: &StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    (
        StagedOptimizedSelectedFormEncoding,
        StagedOptimizedResolvedSelectedFormLayout,
        ValidatedWholeFunctionExitContract,
    ),
    AllocationRecoveryFunctionRelativeRealizationError,
> {
    match source {
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) => {
            build_for_selected(
                homes.reanalysis_stage().transformation_stage().copies(),
                machine,
                source.register_environment().physical(),
            )
        }
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(
            rematerialization,
        ) => build_for_selected(
            rematerialization.rematerialization(),
            machine,
            source.register_environment().physical(),
        ),
    }
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
