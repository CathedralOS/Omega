use selected_instructions_to_register_homes::{AllocationSource, RetainedAllocation};

use crate::stage_whole_function_exit_contract;
use post_allocation_machine_to_selected_form_encoding::stage_optimized_layout_independent_selected_form_encoding;
use register_homes_to_post_allocation_machine::stage_optimized_post_allocation_machine_plan;
use selected_form_encoding_to_resolved_layout::stage_optimized_resolved_selected_form_layout;

use super::custody::structural_unit_realization_receipt;
use super::manifest::expected_manifest;
use super::model::{
    OptimizedStructuralUnitFunctionRelativeRealizationError,
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
};
use super::source::validate_source;

pub(super) fn construct_structural_unit_function_relative_realization(
    allocation: RetainedAllocation,
) -> Result<
    StagedOptimizedStructuralUnitFunctionRelativeRealization,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let current = allocation
        .replay_allocation()
        .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Allocation)?;
    let source = validate_source(&current)?;
    let selected = current.selected();
    let physical = current.register_environment().physical();
    let machine = stage_optimized_post_allocation_machine_plan(&current)
        .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Machine)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Layout)?;
    let exit_contract =
        stage_whole_function_exit_contract(selected, &machine, physical, &encoding, &layout)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(&current, &machine, &encoding, &layout, &exit_contract)?;
    let custody = structural_unit_realization_receipt(source, &machine, &exit_contract, &manifest);
    Ok(StagedOptimizedStructuralUnitFunctionRelativeRealization {
        allocation,
        machine,
        encoding,
        layout,
        exit_contract,
        manifest,
        custody,
    })
}
