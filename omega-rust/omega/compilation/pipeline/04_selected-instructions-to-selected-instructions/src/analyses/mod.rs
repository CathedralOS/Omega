//! Optimizer module role: stage group. Current selected-program facts and their independent validation.

mod legality;
pub(crate) mod live_ranges;
pub(crate) mod liveness;
pub(crate) mod machine_effects;
mod reanalysis;

pub(crate) use legality::stage_optimized_allocation_legality_for_frameless_leaf;
pub use legality::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality,
    StagedOptimizedAllocationLegalityCustodyReceipt, stage_optimized_allocation_legality,
    stage_optimized_allocation_legality_for_active_resident_immediate_u64_multi_use_rematerialization_v1,
    stage_optimized_allocation_legality_with_availability,
};
pub use live_ranges::{
    LiveRangeError, LiveRangeValidationReceipt, OptimizedLiveRangeCustodyError,
    StagedOptimizedLiveRangeCustodyReceipt, StagedOptimizedLiveRanges, ValidatedLiveRanges,
    analyze_live_ranges, analyze_live_ranges_reusing, stage_optimized_live_ranges,
    validate_live_ranges,
};
pub use liveness::{
    LivenessError, LivenessValidationReceipt, OptimizedLivenessCustodyError,
    StagedOptimizedLiveness, StagedOptimizedLivenessCustodyReceipt, ValidatedLiveness,
    analyze_liveness, analyze_liveness_reusing, stage_optimized_liveness, validate_liveness,
};
pub(crate) use machine_effects::analyze_pre_allocation_machine_effects;
pub(crate) use machine_effects::validated_machine_effect_catalog;
pub use machine_effects::{
    MachineEffectError, MachineEffectStageError, ValidatedPreAllocationMachineEffects,
    analyze_machine_effects, validate_pre_allocation_machine_effects,
};
pub use reanalysis::{
    OptimizedSelectedReanalysisError, StagedOptimizedSelectedReanalysis,
    StagedOptimizedSelectedReanalysisCustodyReceipt, stage_optimized_selected_reanalysis,
    validate_optimized_selected_reanalysis_custody,
};

pub(crate) mod allocation_legality;
pub(crate) mod allocator_availability;
pub(crate) mod fixed_precolored_intervals;
mod fixed_precolored_segment_homes;
pub(crate) mod fixed_precolored_split_requirements;
pub(crate) mod recovery_classification;
mod selected_input;
pub(crate) mod spill_choice;
pub use allocation_legality::{
    AllocationLegalityError, AllocationLegalityValidationReceipt, ValidatedAllocationLegality,
    analyze_allocation_legality, analyze_allocation_legality_reusing, validate_allocation_legality,
};
pub use allocator_availability::{
    AllocatorAvailabilityError, AllocatorAvailabilityValidationReceipt,
    ValidatedAllocatorAvailability, materialize_allocator_availability,
};
pub use fixed_precolored_intervals::{
    FixedPrecoloredIntervalError, FixedPrecoloredIntervalValidationReceipt,
    ValidatedFixedPrecoloredIntervals, analyze_fixed_precolored_intervals,
    validate_fixed_precolored_intervals,
};
pub use fixed_precolored_segment_homes::{
    FixedPrecoloredSegmentHomeError, FixedPrecoloredSegmentHomeValidationReceipt,
    ValidatedFixedPrecoloredSegmentHomes, assign_fixed_precolored_segment_homes,
    validate_fixed_precolored_segment_homes,
};
pub use fixed_precolored_split_requirements::{
    FixedPrecoloredSplitRequirementError, FixedPrecoloredSplitRequirementValidationReceipt,
    ValidatedFixedPrecoloredSplitRequirements, analyze_fixed_precolored_split_requirements,
    validate_fixed_precolored_split_requirements,
};
pub use recovery_classification::{
    RecoveryClassificationError, RecoveryClassificationValidationReceipt,
    ValidatedRecoveryClassifications, classify_pressure_recovery,
    validate_recovery_classifications,
};
pub use selected_input::{OwnedSelectedProgram, SelectedProgramRef, ValidatedSelectedAnalysis};
pub use spill_choice::{
    SpillChoiceError, SpillChoiceValidationReceipt, ValidatedSpillChoices, choose_spill_victims,
    validate_spill_choices,
};

/// Each function's digest for the receipt identity: a carried function's
/// comes from its validated predecessor, whose rows it copies; every other
/// function is hashed afresh.
pub(crate) fn carried_digests<Function>(
    functions: &[Function],
    carried: &[bool],
    previous_digests: &[[u8; 32]],
    digest: impl Fn(&Function) -> [u8; 32],
) -> std::sync::Arc<[[u8; 32]]> {
    functions
        .iter()
        .enumerate()
        .map(|(index, function)| match previous_digests.get(index) {
            Some(previous) if carried.get(index) == Some(&true) => *previous,
            _ => digest(function),
        })
        .collect()
}
