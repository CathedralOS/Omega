//! Baseline physical route brought to the common function-relative boundary.

use omega_regalloc::ValidatedSelectedAnalysis;

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
    let optimized = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = optimized.selected().selected_plan();
    let route = if !selected.structural_unit_functions.is_empty() {
        IdentityFunctionRelativeRoute::StructuralUnit
    } else if validate_unit_shape(selected).is_ok() {
        IdentityFunctionRelativeRoute::Unit
    } else {
        IdentityFunctionRelativeRoute::FixedFrame
    };
    let budget = optimized.optimized_target().optimized().budget_per_pass();

    match route {
        IdentityFunctionRelativeRoute::Unit => stage_optimized_unit_function_relative_realization(homes)
            .map(StagedOptimizedVerifiedPhysicalPipeline::from)
            .map_err(OptimizedVerifiedPhysicalPipelineError::UnitFunctionRelativeRealization),
        IdentityFunctionRelativeRoute::StructuralUnit => {
            stage_optimized_structural_unit_function_relative_realization(homes)
                .map(StagedOptimizedVerifiedPhysicalPipeline::from)
                .map_err(
                    OptimizedVerifiedPhysicalPipelineError::StructuralUnitFunctionRelativeRealization,
                )
        }
        IdentityFunctionRelativeRoute::FixedFrame => {
            stage_fixed_frame_function_relative_realization(homes, budget)
                .map(StagedOptimizedVerifiedPhysicalPipeline::from)
                .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)
        }
    }
}
