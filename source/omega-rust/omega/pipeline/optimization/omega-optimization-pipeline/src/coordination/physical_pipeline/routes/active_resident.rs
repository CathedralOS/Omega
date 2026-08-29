use omega_regalloc::{
    PressureRematerializationPolicy, RecoveryClassificationPolicy, SpillChoicePolicy,
};

use crate::{
    StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization,
    StagedOptimizedLiveRanges, ValidatedOptimizedTargetOperations,
    stage_optimized_active_resident_rematerialization,
    stage_optimized_active_resident_rematerialization_function_relative_realization,
    stage_optimized_active_resident_rematerialization_resolved_selected_form_layout,
    stage_optimized_active_resident_rematerialization_selected_form_encoding,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_instruction_selection, stage_optimized_live_ranges, stage_optimized_liveness,
    stage_optimized_post_allocation_machine_plan_after_active_resident_rematerialization,
};

use super::super::OptimizedVerifiedPhysicalPipelineError;

#[inline(never)]
pub(in crate::coordination::physical_pipeline) fn stage_active_resident_rematerialization_pipeline(
    ranges: StagedOptimizedLiveRanges,
) -> Result<
    Box<StagedOptimizedActiveResidentRematerializationFunctionRelativeRealization>,
    OptimizedVerifiedPhysicalPipelineError,
> {
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
    let encoding = stage_optimized_active_resident_rematerialization_selected_form_encoding(
        rematerialization,
        machine,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::ActiveResidentRematerializationEncoding)?;
    let layout = stage_optimized_active_resident_rematerialization_resolved_selected_form_layout(
        encoding,
    )
    .map_err(OptimizedVerifiedPhysicalPipelineError::ActiveResidentRematerializationLayout)?;
    stage_optimized_active_resident_rematerialization_function_relative_realization(layout)
        .map(Box::new)
        .map_err(
            OptimizedVerifiedPhysicalPipelineError::ActiveResidentRematerializationFunctionRelative,
        )
}

pub(in crate::coordination::physical_pipeline) fn stage_active_resident_rematerialization_live_ranges(
    optimized_target: ValidatedOptimizedTargetOperations,
) -> Result<StagedOptimizedLiveRanges, OptimizedVerifiedPhysicalPipelineError> {
    let selected = stage_optimized_instruction_selection(optimized_target)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Selection)?;
    let liveness = stage_optimized_liveness(selected)
        .map_err(OptimizedVerifiedPhysicalPipelineError::Liveness)?;
    stage_optimized_live_ranges(liveness)
        .map_err(OptimizedVerifiedPhysicalPipelineError::LiveRanges)
}
