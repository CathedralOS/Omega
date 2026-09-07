//! Current allocation brought to the common function-relative boundary.

use selected_instructions_to_register_homes::RetainedAllocation;

use crate::StagedOptimizedVerifiedPhysicalPipeline;
use machine_emission::{
    stage_fixed_frame_function_relative_realization,
    stage_optimized_structural_unit_function_relative_realization,
    stage_optimized_unit_function_relative_realization, validate_unit_shape,
};

use super::super::OptimizedVerifiedPhysicalPipelineError;

/// Classify the already selected representation before consuming its physical
/// custody. This is a closed shape decision, not speculative route probing.
enum CurrentAllocationFunctionRelativeRoute {
    Unit,
    StructuralUnit,
    FixedFrame,
}

pub(in crate::native_pipeline::physical_pipeline) fn stage_current_allocation_function_relative_pipeline(
    allocation: RetainedAllocation,
    machine: register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let current = allocation.current();
    let selected = current.selected_plan();
    let has_layout_selection = !current
        .selections()
        .for_phase(optimization_core::OptimizationExecutionPhase::FunctionRelativeLayout)
        .is_empty();
    let route = if !selected.structural_unit_functions.is_empty() {
        if has_layout_selection {
            return Err(
                OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition,
            );
        }
        CurrentAllocationFunctionRelativeRoute::StructuralUnit
    } else if !has_layout_selection && validate_unit_shape(selected).is_ok() {
        CurrentAllocationFunctionRelativeRoute::Unit
    } else {
        CurrentAllocationFunctionRelativeRoute::FixedFrame
    };
    let budget = current.budget_per_pass();

    match route {
        CurrentAllocationFunctionRelativeRoute::Unit => stage_optimized_unit_function_relative_realization(allocation, machine)
            .map(StagedOptimizedVerifiedPhysicalPipeline::from)
            .map_err(OptimizedVerifiedPhysicalPipelineError::UnitFunctionRelativeRealization),
        CurrentAllocationFunctionRelativeRoute::StructuralUnit => {
            stage_optimized_structural_unit_function_relative_realization(allocation, machine)
                .map(StagedOptimizedVerifiedPhysicalPipeline::from)
                .map_err(
                    OptimizedVerifiedPhysicalPipelineError::StructuralUnitFunctionRelativeRealization,
                )
        }
        CurrentAllocationFunctionRelativeRoute::FixedFrame => {
            stage_fixed_frame_function_relative_realization(allocation, machine, budget)
                .map(StagedOptimizedVerifiedPhysicalPipeline::from)
                .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)
        }
    }
}
