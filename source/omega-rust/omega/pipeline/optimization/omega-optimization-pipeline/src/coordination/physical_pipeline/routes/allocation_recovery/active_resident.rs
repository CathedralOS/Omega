use omega_regalloc::{
    PressureRematerializationPolicy, RecoveryClassificationPolicy, SpillChoicePolicy,
};

use crate::{
    StagedAllocationRecoveryFunctionRelativeSource, StagedOptimizedLiveRanges,
    StagedOptimizedVerifiedPhysicalPipeline,
    stage_allocation_recovery_function_relative_realization,
    stage_optimized_active_resident_rematerialization,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization,
};

use super::super::super::OptimizedVerifiedPhysicalPipelineError;

#[inline(never)]
pub(super) fn stage_active_resident(
    ranges: StagedOptimizedLiveRanges,
) -> Result<StagedOptimizedVerifiedPhysicalPipeline, OptimizedVerifiedPhysicalPipelineError> {
    let budget = ranges
        .liveness_stage()
        .selected_stage()
        .optimized_target()
        .optimized()
        .budget_per_pass();
    let legality = stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1(ranges)
        .map_err(OptimizedVerifiedPhysicalPipelineError::AllocationLegality)?;
    let rematerialization = stage_optimized_active_resident_rematerialization(
        legality,
        SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
        PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
        budget,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::ActiveResidentRematerialization)?;
    let machine =
        stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization(
            &rematerialization,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
    let realization = stage_allocation_recovery_function_relative_realization(
        StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(Box::new(
            rematerialization,
        )),
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
