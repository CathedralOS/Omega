use crate::{
    validate_optimized_layout_independent_selected_form_encoding,
    validate_optimized_post_allocation_machine_plan_custody,
    validate_optimized_resolved_selected_form_layout, validate_whole_function_exit_contract,
};

use super::custody::structural_unit_realization_receipt;
use super::manifest::expected_manifest;
use super::model::{
    OptimizedStructuralUnitFunctionRelativeRealizationError,
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt,
};
use super::source::{selected_stage, validate_source};

pub fn validate_optimized_structural_unit_function_relative_realization(
    staged: &StagedOptimizedStructuralUnitFunctionRelativeRealization,
) -> Result<
    StagedOptimizedStructuralUnitFunctionRelativeRealizationCustodyReceipt,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let source = validate_source(&staged.homes)?;
    let machine =
        validate_optimized_post_allocation_machine_plan_custody(&staged.homes, &staged.machine)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Machine)?;
    if &machine != staged.machine.custody() {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::ReceiptMismatch);
    }
    let selected_stage = selected_stage(&staged.homes);
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
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
        &staged.homes,
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
