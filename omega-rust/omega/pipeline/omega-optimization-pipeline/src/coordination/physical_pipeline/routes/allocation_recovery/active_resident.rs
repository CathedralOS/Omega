use omega_machine_optimizer::PostAllocationMachineRuleCatalogEntry;
use omega_regalloc::{
    PressureRematerializationPolicy, RecoveryClassificationPolicy, SpillChoicePolicy,
};

use crate::{
    StagedAllocationRecoveryFunctionRelativeSource, StagedOptimizedLiveRanges,
    StagedOptimizedVerifiedPhysicalPipeline,
    stage_allocation_recovery_function_relative_realization,
    stage_optimized_active_resident_rematerialization,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_post_allocation_machine_optimization_for_catalog_entry,
    stage_optimized_post_allocation_machine_plan,
    stage_post_allocation_machine_function_relative_realization_after_allocation_recovery,
};

use super::super::super::OptimizedVerifiedPhysicalPipelineError;

#[inline(never)]
pub(super) fn stage_active_resident(
    ranges: StagedOptimizedLiveRanges,
    post_allocation: Option<PostAllocationMachineRuleCatalogEntry>,
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
    let machine = stage_optimized_post_allocation_machine_plan(&rematerialization)
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachine)?;
    if let Some(entry) = post_allocation {
        let optimization = stage_optimized_post_allocation_machine_optimization_for_catalog_entry(
            &rematerialization,
            &machine,
            entry,
        )
        .map_err(OptimizedVerifiedPhysicalPipelineError::PostAllocationMachineOptimization)?;
        let realization =
            stage_post_allocation_machine_function_relative_realization_after_allocation_recovery(
                StagedAllocationRecoveryFunctionRelativeSource::ActiveResidentRematerialization(
                    Box::new(rematerialization),
                ),
                machine,
                optimization,
            )
            .map_err(OptimizedVerifiedPhysicalPipelineError::FunctionRelativeRealization)?;
        return Ok(StagedOptimizedVerifiedPhysicalPipeline::PostAllocationMachine { realization });
    }
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
