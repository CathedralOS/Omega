use crate::{
    StagedOptimizedRegisterHomes, stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_post_allocation_machine_plan, stage_optimized_resolved_selected_form_layout,
    stage_whole_function_exit_contract,
};

use super::custody::unit_realization_receipt;
use super::manifest::expected_manifest;
use super::model::{
    OptimizedUnitFunctionRelativeRealizationError, StagedOptimizedUnitFunctionRelativeRealization,
};
use super::source::{selected_stage, validate_source};

pub(super) fn construct_unit_function_relative_realization(
    homes: StagedOptimizedRegisterHomes,
) -> Result<
    StagedOptimizedUnitFunctionRelativeRealization,
    OptimizedUnitFunctionRelativeRealizationError,
> {
    let source = validate_source(&homes)?;
    let selected_stage = selected_stage(&homes);
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let machine = stage_optimized_post_allocation_machine_plan(&homes)
        .map_err(OptimizedUnitFunctionRelativeRealizationError::Machine)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Layout)?;
    let exit_contract =
        stage_whole_function_exit_contract(selected, &machine, physical, &encoding, &layout)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(&homes, &machine, &encoding, &layout, &exit_contract)?;
    let custody = unit_realization_receipt(source, &machine, &exit_contract, &manifest);
    Ok(StagedOptimizedUnitFunctionRelativeRealization {
        homes,
        machine,
        encoding,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}
