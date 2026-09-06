use selected_instructions_to_register_homes::{AllocationSource, RetainedAllocation};

use crate::stage_whole_function_exit_contract_for_layout;
use post_allocation_machine_to_selected_form_encoding::stage_optimized_layout_independent_selected_form_encoding;
use register_homes_to_post_allocation_machine::{
    StagedOptimizedPostAllocationMachinePlan,
    validate_optimized_post_allocation_machine_plan_custody,
};
use resolved_layout_to_resolved_layout::execute_resolved_layout_optimization;
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
    machine: StagedOptimizedPostAllocationMachinePlan,
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
    validate_optimized_post_allocation_machine_plan_custody(&current, &machine)
        .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Machine)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Layout)?;
    let layout_optimization = execute_resolved_layout_optimization(
        selected,
        &machine,
        physical,
        &encoding,
        None,
        &layout,
        &current
            .selections()
            .project_phase(optimization_core::OptimizationExecutionPhase::FunctionRelativeLayout),
        current.budget_per_pass(),
    )
    .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::LayoutOptimization)?;
    let exit_contract = stage_whole_function_exit_contract_for_layout(
        selected,
        &machine,
        physical,
        &encoding,
        None,
        &layout,
        &layout_optimization,
        None,
    )
    .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(
        &current,
        &machine,
        &encoding,
        layout_optimization.layout(),
        &exit_contract,
    )?;
    let custody = structural_unit_realization_receipt(source, &machine, &exit_contract, &manifest);
    Ok(StagedOptimizedStructuralUnitFunctionRelativeRealization {
        allocation,
        machine,
        encoding,
        layout,
        layout_optimization,
        exit_contract,
        manifest,
        custody,
    })
}
