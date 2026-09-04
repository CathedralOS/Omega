use omega_regalloc::FixedViewCopyPolicy;

use crate::{
    StagedAllocationRecoveryFunctionRelativeSource, StagedOptimizedLiveRanges,
    StagedOptimizedVerifiedPhysicalPipeline,
    stage_allocation_recovery_function_relative_realization, stage_optimized_allocation_legality,
    stage_optimized_fixed_precolored_segment_homes, stage_optimized_fixed_view_copies,
    stage_optimized_post_allocation_machine_plan,
    stage_optimized_register_homes_after_fixed_view_copies, stage_optimized_selected_reanalysis,
};

use super::super::super::OptimizedVerifiedPhysicalPipelineError;

#[inline(never)]
pub(super) fn stage_fixed_view(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let budget = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .budget_per_pass();
    let legality = stage_optimized_allocation_legality(ranges)
        .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
    let segment_homes = stage_optimized_fixed_precolored_segment_homes(legality, budget)
        .map_err(OptimizedVerifiedPhysicalPipelineError::FixedPrecoloredSegmentHomes)?;
    let copies = stage_optimized_fixed_view_copies(
        segment_homes,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
        budget,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::FixedViewCopies)?;
    let reanalysis = stage_optimized_selected_reanalysis(copies)
        .map_err(OptimizedVerifiedPhysicalPipelineError::SelectedReanalysis)?;
    let homes = stage_optimized_register_homes_after_fixed_view_copies(reanalysis)
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostCopyRegisterHomes)?;
    let machine = stage_optimized_post_allocation_machine_plan(&homes)
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
    let realization = stage_allocation_recovery_function_relative_realization(
        StagedAllocationRecoveryFunctionRelativeSource::FixedViewCopies(Box::new(homes)),
        machine,
    )
    .map_err(|error| {
        OptimizedVerifiedPhysicalPipelineError::AllocationRecoveryFunctionRelative(Box::new(error))
    })?;
    Ok(
        StagedOptimizedVerifiedPhysicalPipeline::AllocationRecovery {
            realization: Box::new(realization),
        },
    )
}
