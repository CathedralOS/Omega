use crate::{
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody,
    validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody,
    validate_optimized_resolved_selected_form_layout, validate_whole_function_exit_contract,
};

use super::custody::receipt;
use super::manifest::expected_manifest;
use super::model::{
    AllocationRecoveryFunctionRelativeRealizationError,
    StagedAllocationRecoveryFunctionRelativeRealization,
    StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt,
};
use super::source::{
    StagedAllocationRecoveryFunctionRelativeSource, validate_active_resident_source,
    validate_fixed_view_source,
};

pub fn validate_allocation_recovery_function_relative_realization(
    staged: &StagedAllocationRecoveryFunctionRelativeRealization,
) -> Result<
    StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt,
    AllocationRecoveryFunctionRelativeRealizationError,
> {
    staged.source.validate_phase_selection()?;
    let source_custody = match &staged.source {
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(homes) => {
            let custody = validate_fixed_view_source(homes)?;
            validate_optimized_post_allocation_machine_plan_after_fixed_view_copy_custody(
                homes,
                &staged.machine,
            )
            .map_err(AllocationRecoveryFunctionRelativeRealizationError::Machine)?;
            validate_selected(
                homes.reanalysis_stage().transformation_stage().copies(),
                staged,
            )?;
            custody
        }
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(
            rematerialization,
        ) => {
            let custody = validate_active_resident_source(rematerialization)?;
            validate_optimized_post_allocation_machine_plan_after_active_resident_rematerialization_custody(rematerialization, &staged.machine)
                .map_err(AllocationRecoveryFunctionRelativeRealizationError::Machine)?;
            validate_selected(rematerialization.rematerialization(), staged)?;
            custody
        }
    };
    let manifest = expected_manifest(
        &staged.source,
        &staged.machine,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if manifest.record() != staged.manifest.record() {
        return Err(AllocationRecoveryFunctionRelativeRealizationError::RootMismatch);
    }
    let expected = receipt(
        source_custody,
        &staged.machine,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
        &manifest,
    );
    if expected != staged.custody {
        return Err(AllocationRecoveryFunctionRelativeRealizationError::ReceiptMismatch);
    }
    Ok(expected)
}

fn validate_selected<S: omega_regalloc::ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedAllocationRecoveryFunctionRelativeRealization,
) -> Result<(), AllocationRecoveryFunctionRelativeRealizationError> {
    let physical = staged.source.register_environment().physical();
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
    )
    .map_err(AllocationRecoveryFunctionRelativeRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
    )
    .map_err(AllocationRecoveryFunctionRelativeRealizationError::Layout)?;
    validate_whole_function_exit_contract(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )
    .map_err(AllocationRecoveryFunctionRelativeRealizationError::ExitContract)
}
