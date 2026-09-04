use crate::{
    validate_optimized_active_resident_rematerialization_resolved_selected_form_layout,
    validate_whole_function_exit_contract,
};

use super::custody::active_resident_realization_custody;
use super::manifest::expected_manifest;
use super::model::{
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt,
};
use super::source::artifacts;

pub fn validate_optimized_active_resident_rematerialization_function_relative_realization(
    staged: &StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
) -> Result<
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealizationCustodyReceipt,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
> {
    let source_custody =
        validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(
            &staged.source,
        )
        .map_err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::Source,
        )?;
    let artifacts = artifacts(&staged.source)?;
    validate_whole_function_exit_contract(
        artifacts.selected,
        artifacts.machine,
        artifacts.physical,
        artifacts.encoding,
        artifacts.layout,
        &staged.exit_contract,
    )
    .map_err(
        OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ExitContract,
    )?;
    let manifest = expected_manifest(&staged.source, &staged.exit_contract)?;
    if manifest.record() != staged.manifest.record() {
        return Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::RootMismatch,
        );
    }
    let custody =
        active_resident_realization_custody(source_custody, &staged.exit_contract, &manifest);
    if custody != staged.custody {
        return Err(
            OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ReceiptMismatch,
        );
    }
    Ok(custody)
}
