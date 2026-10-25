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
};
pub use live_ranges::{
    LiveRangeError, StagedOptimizedLiveRanges, ValidatedLiveRanges, analyze_live_ranges,
    analyze_live_ranges_reusing,
};
pub(crate) use live_ranges::{
    LiveRangeValidationReceipt, OptimizedLiveRangeCustodyError,
    StagedOptimizedLiveRangeCustodyReceipt, stage_optimized_live_ranges,
};
pub use liveness::{LivenessError, ValidatedLiveness, analyze_liveness, analyze_liveness_reusing};
pub(crate) use liveness::{
    LivenessValidationReceipt, OptimizedLivenessCustodyError, StagedOptimizedLiveness,
    StagedOptimizedLivenessCustodyReceipt, stage_optimized_liveness,
};
pub(crate) use machine_effects::MachineEffectError;
pub(crate) use machine_effects::analyze_pre_allocation_machine_effects;
pub(crate) use machine_effects::validated_machine_effect_catalog;
pub use machine_effects::{
    MachineEffectStageError, ValidatedPreAllocationMachineEffects, analyze_machine_effects,
};
pub use reanalysis::{
    OptimizedSelectedReanalysisError, StagedOptimizedSelectedReanalysis,
    StagedOptimizedSelectedReanalysisCustodyReceipt, stage_optimized_selected_reanalysis,
};

pub(crate) mod allocation_legality;
pub(crate) mod allocator_availability;
pub(crate) mod fixed_precolored_intervals;
mod fixed_precolored_segment_homes;
pub(crate) mod fixed_precolored_split_requirements;
pub(crate) mod recovery_classification;
mod selected_input;
pub(crate) mod spill_choice;
pub(crate) use allocation_legality::AllocationLegalityValidationReceipt;
pub use allocation_legality::{
    AllocationLegalityError, ValidatedAllocationLegality, analyze_allocation_legality,
    analyze_allocation_legality_reusing,
};
pub use allocator_availability::ValidatedAllocatorAvailability;
pub(crate) use allocator_availability::materialize_allocator_availability;
pub(crate) use allocator_availability::{
    AllocatorAvailabilityError, AllocatorAvailabilityValidationReceipt,
};
pub(crate) use fixed_precolored_intervals::{
    FixedPrecoloredIntervalError, FixedPrecoloredIntervalValidationReceipt,
    ValidatedFixedPrecoloredIntervals, analyze_fixed_precolored_intervals,
};
pub(crate) use fixed_precolored_segment_homes::{
    FixedPrecoloredSegmentHomeError, FixedPrecoloredSegmentHomeValidationReceipt,
    ValidatedFixedPrecoloredSegmentHomes, assign_fixed_precolored_segment_homes,
};
pub(crate) use fixed_precolored_split_requirements::{
    FixedPrecoloredSplitRequirementError, FixedPrecoloredSplitRequirementValidationReceipt,
    ValidatedFixedPrecoloredSplitRequirements, analyze_fixed_precolored_split_requirements,
};
pub(crate) use recovery_classification::RecoveryClassificationValidationReceipt;
pub use recovery_classification::{
    RecoveryClassificationError, ValidatedRecoveryClassifications, classify_pressure_recovery,
};
pub use selected_input::{OwnedSelectedProgram, SelectedProgramRef, ValidatedSelectedAnalysis};

pub(crate) use spill_choice::SpillChoiceValidationReceipt;
pub use spill_choice::{SpillChoiceError, ValidatedSpillChoices, choose_spill_victims};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryReanalysisError {
    Liveness(LivenessError),
    Ranges(LiveRangeError),
    Legality(AllocationLegalityError),
}

/// Intermediate per-function analysis rows during runtime-spill recovery,
/// deferring whole-plan receipt hashing until placement succeeds.
#[derive(Clone)]
pub struct UnsealedRecoveryFacts {
    liveness_functions: Vec<std::sync::Arc<selected_instructions::FunctionLiveness>>,
    range_functions: Vec<std::sync::Arc<selected_instructions::FunctionLiveRanges>>,
    legality_functions: Vec<std::sync::Arc<register_homes::FunctionAllocationLegality>>,
}

impl UnsealedRecoveryFacts {
    pub fn from_validated(
        liveness: &ValidatedLiveness,
        ranges: &ValidatedLiveRanges,
        legality: &ValidatedAllocationLegality,
    ) -> Self {
        Self {
            liveness_functions: liveness.plan().functions.clone(),
            range_functions: ranges.plan().functions.clone(),
            legality_functions: legality.plan().functions.clone(),
        }
    }

    pub fn range_functions(&self) -> &[std::sync::Arc<selected_instructions::FunctionLiveRanges>] {
        &self.range_functions
    }

    pub fn legality_functions(
        &self,
    ) -> &[std::sync::Arc<register_homes::FunctionAllocationLegality>] {
        &self.legality_functions
    }

    pub fn reanalyze_changed_functions(
        &self,
        previous: &impl ValidatedSelectedAnalysis,
        selected: &impl ValidatedSelectedAnalysis,
        availability: &ValidatedAllocatorAvailability,
        physical: &register_model::ValidatedPhysicalRegisterModel,
        reservations: &register_model::ValidatedRegisterReservationProfile,
    ) -> Result<Self, RecoveryReanalysisError> {
        let prev_plan = previous.selected_plan();
        let next_plan = selected.selected_plan();
        let count = next_plan.functions.len();
        let mut changed = vec![false; count];
        for (index, function) in next_plan.functions.iter().enumerate() {
            if !(prev_plan
                .functions
                .shares_function_storage(&next_plan.functions, index)
                || prev_plan.functions.get(index) == Some(function))
            {
                changed[index] = true;
            }
        }
        let mut liveness_functions = self.liveness_functions.clone();
        for (index, function) in next_plan.functions.iter().enumerate() {
            if changed[index] {
                liveness_functions[index] = std::sync::Arc::new(
                    liveness::compute::compute_function(index, function)
                        .map_err(RecoveryReanalysisError::Liveness)?,
                );
            }
        }
        let mut range_functions = self.range_functions.clone();
        for (index, function) in next_plan.functions.iter().enumerate() {
            if changed[index] {
                range_functions[index] = std::sync::Arc::new(
                    live_ranges::compute::compute_function(
                        index,
                        function,
                        &liveness_functions[index],
                    )
                    .map_err(RecoveryReanalysisError::Ranges)?,
                );
            }
        }
        let mut legality_functions = self.legality_functions.clone();
        for index in 0..count {
            if changed[index] {
                let computed = allocation_legality::compute::compute_function(
                    index,
                    &range_functions[index],
                    availability,
                    physical,
                    reservations,
                )
                .map_err(RecoveryReanalysisError::Legality)?;
                allocation_legality::validate::FunctionCounts::new(&computed, index)
                    .map_err(RecoveryReanalysisError::Legality)?;
                legality_functions[index] = std::sync::Arc::new(computed);
            }
        }
        Ok(Self {
            liveness_functions,
            range_functions,
            legality_functions,
        })
    }

    pub fn seal(
        self,
        anchor_selected: &impl ValidatedSelectedAnalysis,
        anchor_liveness: &ValidatedLiveness,
        anchor_ranges: &ValidatedLiveRanges,
        anchor_legality: &ValidatedAllocationLegality,
        selected: &impl ValidatedSelectedAnalysis,
        availability: &ValidatedAllocatorAvailability,
        register_environment: register_model::TargetRegisterEnvironmentIdentity,
    ) -> Result<
        (
            ValidatedLiveness,
            ValidatedLiveRanges,
            ValidatedAllocationLegality,
        ),
        AllocationLegalityError,
    > {
        let anchor_plan = anchor_selected.selected_plan();
        let next_plan = selected.selected_plan();
        let carried: Vec<bool> = next_plan
            .functions
            .iter()
            .enumerate()
            .map(|(index, function)| {
                (anchor_plan
                    .functions
                    .shares_function_storage(&next_plan.functions, index)
                    || anchor_plan.functions.get(index) == Some(function))
                    && anchor_liveness.plan().functions.get(index)
                        == self.liveness_functions.get(index)
                    && anchor_ranges.plan().functions.get(index) == self.range_functions.get(index)
                    && anchor_legality.plan().functions.get(index)
                        == self.legality_functions.get(index)
            })
            .collect();
        let liveness_plan = selected_instructions::LivenessPlan {
            selected: selected.selected_identity(),
            optimization_unit: selected.optimization_unit_identity(),
            fuel_schedule: selected.fuel_schedule_identity(),
            target: next_plan.target,
            functions: self.liveness_functions,
        };
        let validated_liveness = liveness::validate::admit_validated_liveness(
            selected,
            liveness_plan,
            &carried,
            &anchor_liveness.function_digests,
            &anchor_liveness.function_counts,
        );
        let range_plan = selected_instructions::LiveRangePlan {
            selected: selected.selected_identity(),
            liveness: validated_liveness.receipt().identity(),
            optimization_unit: selected.optimization_unit_identity(),
            fuel_schedule: selected.fuel_schedule_identity(),
            target: next_plan.target,
            functions: self.range_functions,
        };
        let validated_ranges = live_ranges::validate::admit(
            range_plan,
            &carried,
            &anchor_ranges.function_digests,
            &anchor_ranges.function_counts,
        );
        let legality_plan = register_homes::AllocationLegalityPlan {
            ranges: validated_ranges.receipt().identity(),
            register_environment,
            allocator_availability: availability.receipt().identity(),
            functions: self.legality_functions,
        };
        let validated_legality = allocation_legality::validate::admit(
            legality_plan,
            &carried,
            &anchor_legality.function_digests,
            &anchor_legality.function_counts,
        )?;
        Ok((validated_liveness, validated_ranges, validated_legality))
    }
}

#[cfg(test)]
mod report_counts_tests;
