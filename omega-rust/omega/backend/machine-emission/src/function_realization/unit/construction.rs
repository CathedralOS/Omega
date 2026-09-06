use selected_instructions_to_register_homes::{AllocationSource, RetainedAllocation};

use crate::stage_whole_function_exit_contract_for_layout;
use post_allocation_machine_to_selected_form_encoding::stage_optimized_layout_independent_selected_form_encoding;
use register_homes_to_post_allocation_machine::{
    StagedOptimizedPostAllocationMachinePlan,
    validate_optimized_post_allocation_machine_plan_custody,
};
use resolved_layout_to_resolved_layout::execute_resolved_layout_optimization;
use selected_form_encoding_to_resolved_layout::stage_optimized_resolved_selected_form_layout;

use super::custody::unit_realization_receipt;
use super::manifest::expected_manifest;
use super::model::{
    OptimizedUnitFunctionRelativeRealizationError, StagedOptimizedUnitFunctionRelativeRealization,
};
use super::source::validate_source;

pub(super) fn construct_unit_function_relative_realization(
    allocation: RetainedAllocation,
    machine: StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedOptimizedUnitFunctionRelativeRealization,
    OptimizedUnitFunctionRelativeRealizationError,
> {
    let current = allocation
        .replay_allocation()
        .map_err(OptimizedUnitFunctionRelativeRealizationError::Allocation)?;
    let source = validate_source(&current)?;
    let selected = current.selected();
    let environment = current.register_environment();
    let physical = environment.physical();
    validate_optimized_post_allocation_machine_plan_custody(&current, &machine)
        .map_err(OptimizedUnitFunctionRelativeRealizationError::Machine)?;
    let encoding =
        stage_optimized_layout_independent_selected_form_encoding(selected, &machine, physical)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Encoding)?;
    let layout =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .map_err(OptimizedUnitFunctionRelativeRealizationError::Layout)?;
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
    .map_err(OptimizedUnitFunctionRelativeRealizationError::LayoutOptimization)?;
    let frame = super::frame::stage_unit_frame(&current, &machine)
        .map_err(OptimizedUnitFunctionRelativeRealizationError::Manifest)?;
    let exit_contract = stage_whole_function_exit_contract_for_layout(
        selected,
        &machine,
        physical,
        &encoding,
        None,
        &layout,
        &layout_optimization,
        frame
            .as_ref()
            .map(|frame| (frame.layout(), frame.protocol())),
    )
    .map_err(OptimizedUnitFunctionRelativeRealizationError::Exit)?;
    let manifest = expected_manifest(
        &current,
        &machine,
        &encoding,
        layout_optimization.layout(),
        frame.as_ref(),
        &exit_contract,
    )?;
    let custody =
        unit_realization_receipt(source, &machine, frame.as_ref(), &exit_contract, &manifest);
    Ok(StagedOptimizedUnitFunctionRelativeRealization {
        allocation,
        machine,
        encoding,
        layout,
        layout_optimization,
        frame,
        exit_contract,
        manifest,
        custody,
    })
}
