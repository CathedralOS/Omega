use crate::validate_whole_function_exit_contract;
use post_allocation_machine_to_resolved_layout::selected_form_encoding::validate_optimized_layout_independent_selected_form_encoding;
use post_allocation_machine_to_resolved_layout::validate_optimized_resolved_selected_form_layout;
use register_homes_to_post_allocation_machine::validate_optimized_post_allocation_machine_plan_custody;

use super::custody::receipt;
use super::manifest::expected_manifest;
use super::model::{
    AllocationRecoveryFunctionRelativeRealizationError,
    StagedAllocationRecoveryFunctionRelativeRealization,
    StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt,
};
use super::selection::validate_phase_selection;
use selected_instructions_to_register_homes::AllocationSource;

pub fn validate_allocation_recovery_function_relative_realization(
    staged: &StagedAllocationRecoveryFunctionRelativeRealization,
) -> Result<
    StagedAllocationRecoveryFunctionRelativeRealizationCustodyReceipt,
    AllocationRecoveryFunctionRelativeRealizationError,
> {
    let current = staged
        .allocation
        .replay_allocation()
        .map_err(AllocationRecoveryFunctionRelativeRealizationError::Allocation)?;
    validate_phase_selection(&current)?;
    validate_optimized_post_allocation_machine_plan_custody(&current, &staged.machine)
        .map_err(AllocationRecoveryFunctionRelativeRealizationError::Machine)?;
    validate_selected(current.selected(), staged)?;
    let manifest = expected_manifest(
        &current,
        &staged.machine,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if manifest.record() != staged.manifest.record() {
        return Err(AllocationRecoveryFunctionRelativeRealizationError::RootMismatch);
    }
    let expected = receipt(
        current.evidence().clone(),
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

fn validate_selected<S: selected_instructions_to_register_homes::ValidatedSelectedAnalysis>(
    selected: &S,
    staged: &StagedAllocationRecoveryFunctionRelativeRealization,
) -> Result<(), AllocationRecoveryFunctionRelativeRealizationError> {
    let physical = staged
        .allocation
        .current()
        .register_environment()
        .physical();
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
