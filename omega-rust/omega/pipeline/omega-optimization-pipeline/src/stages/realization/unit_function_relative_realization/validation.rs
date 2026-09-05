use omega_selected_instructions_to_register_homes::AllocationSource;

use crate::{
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_resolved_selected_form_layout, validate_whole_function_exit_contract,
};

use super::custody::unit_realization_receipt;
use super::manifest::expected_manifest;
use super::model::{
    OptimizedUnitFunctionRelativeRealizationError, StagedOptimizedUnitFunctionRelativeRealization,
    StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt,
};
use super::source::validate_source;

pub fn validate_optimized_unit_function_relative_realization(
    staged: &StagedOptimizedUnitFunctionRelativeRealization,
) -> Result<
    StagedOptimizedUnitFunctionRelativeRealizationCustodyReceipt,
    OptimizedUnitFunctionRelativeRealizationError,
> {
    let current = staged
        .allocation
        .replay_allocation()
        .map_err(OptimizedUnitFunctionRelativeRealizationError::Allocation)?;
    let source = validate_source(&current)?;
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&current, &staged.machine)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Machine)?;
    if &machine != staged.machine.custody() {
        return Err(OptimizedUnitFunctionRelativeRealizationError::ReceiptMismatch);
    }
    let selected = current.selected();
    let physical = current.register_environment().physical();
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
    )
    .map_err(OptimizedUnitFunctionRelativeRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
    )
    .map_err(OptimizedUnitFunctionRelativeRealizationError::Layout)?;
    validate_whole_function_exit_contract(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )
    .map_err(OptimizedUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(
        &current,
        &staged.machine,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if manifest.record() != staged.manifest.record() {
        return Err(OptimizedUnitFunctionRelativeRealizationError::RootMismatch);
    }
    let custody =
        unit_realization_receipt(source, &staged.machine, &staged.exit_contract, &manifest);
    if custody != staged.custody {
        return Err(OptimizedUnitFunctionRelativeRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}
