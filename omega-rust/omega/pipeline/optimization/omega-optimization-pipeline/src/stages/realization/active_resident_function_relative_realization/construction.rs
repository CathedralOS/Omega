use crate::{
    StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
    stage_whole_function_exit_contract,
    validate_optimized_active_resident_rematerialization_resolved_selected_form_layout,
};

use super::custody::active_resident_realization_custody;
use super::manifest::expected_manifest;
use super::model::{
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
};
use super::source::artifacts;

pub(super) fn construct_active_resident_function_relative_realization(
    source: StagedOptimizedActiveResidentRematerializationResolvedSelectedFormLayout,
) -> Result<
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
> {
    let source_custody =
        validate_optimized_active_resident_rematerialization_resolved_selected_form_layout(&source)
            .map_err(
                OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::Source,
            )?;
    let artifacts = artifacts(&source)?;
    let exit_contract = stage_whole_function_exit_contract(
        artifacts.selected,
        artifacts.machine,
        artifacts.physical,
        artifacts.encoding,
        artifacts.layout,
    )
    .map_err(
        OptimizedActiveResidentRematerializationFunctionRelativeRealizationError::ExitContract,
    )?;
    let manifest = expected_manifest(&source, &exit_contract)?;
    let custody = active_resident_realization_custody(source_custody, &exit_contract, &manifest);
    let staged = StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization {
        source,
        exit_contract,
        manifest,
        custody,
    };
    Ok(staged)
}
