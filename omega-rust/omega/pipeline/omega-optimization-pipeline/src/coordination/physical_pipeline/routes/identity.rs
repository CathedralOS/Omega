//! Baseline physical route brought to the common function-relative boundary.

use omega_selected_instructions_to_register_homes::RetainedAllocation;

use crate::{
    StagedOptimizedRegisterHomes, StagedOptimizedVerifiedPhysicalPipeline,
    stage_fixed_frame_function_relative_realization,
    stage_optimized_structural_unit_function_relative_realization,
    stage_optimized_unit_function_relative_realization, validate_unit_shape,
};

use super::super::OptimizedVerifiedPhysicalPipelineError;

/// Classify the already selected representation before consuming its physical
/// custody. This is a closed shape decision, not speculative route probing.
enum IdentityFunctionRelativeRoute {
    Unit,
    StructuralUnit,
    FixedFrame,
}

pub(in crate::coordination::physical_pipeline) fn stage_identity_function_relative_pipeline(
    homes: StagedOptimizedRegisterHomes,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let allocation = RetainedAllocation::try_from(homes)
        .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationReplay)?;
    let current = allocation.current();
    let selected = current.selected_plan();
    let route = if !selected.structural_unit_functions.is_empty() {
        IdentityFunctionRelativeRoute::StructuralUnit
    } else if validate_unit_shape(selected).is_ok() {
        IdentityFunctionRelativeRoute::Unit
    } else {
        IdentityFunctionRelativeRoute::FixedFrame
    };
    let budget = current.budget_per_pass();

    match route {
        IdentityFunctionRelativeRoute::Unit => stage_optimized_unit_function_relative_realization(allocation)
            .map(StagedOptimizedVerifiedPhysicalPipeline::from)
            .map_err(OptimizedVerifiedPhysicalPipelineError::UnitFunctionRelativeRealization),
        IdentityFunctionRelativeRoute::StructuralUnit => {
            stage_optimized_structural_unit_function_relative_realization(allocation)
                .map(StagedOptimizedVerifiedPhysicalPipeline::from)
                .map_err(
                    OptimizedVerifiedPhysicalPipelineError::StructuralUnitFunctionRelativeRealization,
                )
        }
        IdentityFunctionRelativeRoute::FixedFrame => {
            stage_fixed_frame_function_relative_realization(allocation, budget)
                .map(StagedOptimizedVerifiedPhysicalPipeline::from)
                .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)
        }
    }
}
