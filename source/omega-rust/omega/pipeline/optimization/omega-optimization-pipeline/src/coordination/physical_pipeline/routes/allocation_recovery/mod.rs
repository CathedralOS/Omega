//! Optimizer module role: executable entrance. Single physical route for the closed allocation-recovery catalog.

mod active_resident;
mod fixed_view;

use omega_optimization_core::{Optimization, OptimizationExecutionPhase};

use crate::{
    StagedOptimizedVerifiedPhysicalPipeline, ValidatedOptimizedTargetOperations,
    stage_optimized_instruction_selection, stage_optimized_live_ranges, stage_optimized_liveness,
};

use super::super::OptimizedVerifiedPhysicalPipelineError;
use active_resident::stage_active_resident;
use fixed_view::stage_fixed_view;

pub(in crate::coordination::physical_pipeline) fn stage_allocation_recovery_pipeline(
    optimized_target: ValidatedOptimizedTargetOperations,
    rule: Optimization,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let selections = optimized_target.optimized().selections();
    if [
        OptimizationExecutionPhase::SelectedLowering,
        OptimizationExecutionPhase::PostAllocationMachine,
        OptimizationExecutionPhase::FunctionRelativeLayout,
    ]
    .into_iter()
    .any(|phase| !selections.for_phase(phase).is_empty())
    {
        return Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition);
    }
    let selected = stage_optimized_instruction_selection(optimized_target)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)?;
    let liveness = stage_optimized_liveness(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Liveness)?;
    let ranges = stage_optimized_live_ranges(liveness)
        .map_err(OptimizedVerifiedPhysicalPipelineError::LiveRanges)?;
    match rule {
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1 => {
            stage_fixed_view(ranges)
        }
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1 => {
            stage_active_resident(ranges)
        }
        _ => Err(OptimizedVerifiedPhysicalPipelineError::UnsupportedPhysicalPhaseComposition),
    }
}
