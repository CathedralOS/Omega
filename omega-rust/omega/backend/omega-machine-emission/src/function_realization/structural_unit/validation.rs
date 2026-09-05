use omega_selected_instructions_to_register_homes::AllocationSource;

use crate::validate_whole_function_exit_contract;
use omega_post_allocation_machine_to_selected_form_encoding::validate_optimized_layout_independent_selected_form_encoding;
use omega_register_homes_to_post_allocation_machine::validate_optimized_post_allocation_machine_plan_custody;
use omega_selected_form_encoding_to_resolved_layout::validate_optimized_resolved_selected_form_layout;

use super::custody::structural_unit_realization_receipt;
use super::manifest::expected_manifest;
use super::model::{
    OptimizedStructuralUnitFunctionRelativeRealizationError,
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt,
};
use super::source::validate_source;

pub fn validate_optimized_structural_unit_function_relative_realization(
    staged: &StagedOptimizedStructuralUnitFunctionRelativeRealization,
) -> Result<
    StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let current = staged
        .allocation
        .replay_allocation()
        .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Allocation)?;
    let source = validate_source(&current)?;
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&current, &staged.machine)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Machine)?;
    if &machine != staged.machine.custody() {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::ReceiptMismatch);
    }
    let selected = current.selected();
    let physical = current.register_environment().physical();
    validate_optimized_layout_independent_selected_form_encoding(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
    )
    .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
    )
    .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Layout)?;
    validate_whole_function_exit_contract(
        selected,
        &staged.machine,
        physical,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )
    .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(
        &current,
        &staged.machine,
        &staged.encoding,
        &staged.layout,
        &staged.exit_contract,
    )?;
    if manifest.record() != staged.manifest.record() {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::RootMismatch);
    }
    let custody = structural_unit_realization_receipt(
        source,
        &staged.machine,
        &staged.exit_contract,
        &manifest,
    );
    if custody != staged.custody {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::ReceiptMismatch);
    }
    Ok(custody)
}
