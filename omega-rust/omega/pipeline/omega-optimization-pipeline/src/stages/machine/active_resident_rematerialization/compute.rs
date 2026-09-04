use omega_optimization_core::OptimizationWorkBudget;
use omega_regalloc::{
    PostAllocationSelectedTransformation, PressureRematerializationPolicy,
    RecoveryClassificationPolicy, SpillChoicePolicy, analyze_allocation_legality,
    analyze_live_ranges, analyze_liveness, assign_register_homes, choose_spill_victims,
    classify_pressure_recovery, project_post_allocation_optimization_manifest,
    rematerialize_selected_active_resident,
};

use crate::StagedOptimizedAllocationLegality;

use super::custody::custody_receipt;
use super::model::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
};
use super::validation::validate_source;

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_active_resident_rematerialization(
    source: StagedOptimizedAllocationLegality,
    choice_policy: SpillChoicePolicy,
    classification_policy: RecoveryClassificationPolicy,
    rematerialization_policy: PressureRematerializationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedActiveResidentRematerialization,
    OptimizedActiveResidentRematerializationError,
> {
    if choice_policy != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1
        || classification_policy
            != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1
        || rematerialization_policy
            != PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1
    {
        return Err(OptimizedActiveResidentRematerializationError::UnsupportedPolicy);
    }
    let source_receipt = validate_source(&source)?;
    let environment = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let selected = source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected();
    let source_ranges = source.live_range_stage().ranges();
    let choices = choose_spill_victims(
        source.legality(),
        source_ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        choice_policy,
        budget,
    )
    .map_err(OptimizedActiveResidentRematerializationError::SpillChoice)?;
    let classifications = classify_pressure_recovery(
        selected,
        source_ranges,
        source.legality(),
        &choices,
        classification_policy,
        budget,
    )
    .map_err(OptimizedActiveResidentRematerializationError::Classification)?;
    let rematerialization = rematerialize_selected_active_resident(
        selected,
        source_ranges,
        source.legality(),
        &choices,
        &classifications,
        source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        rematerialization_policy,
        budget,
    )
    .map_err(OptimizedActiveResidentRematerializationError::Rematerialization)?;
    if rematerialization.receipt().applied_count() == 0 {
        return Err(OptimizedActiveResidentRematerializationError::NoAppliedAction);
    }

    let liveness = analyze_liveness(&rematerialization)
        .map_err(OptimizedActiveResidentRematerializationError::Liveness)?;
    let ranges = analyze_live_ranges(&rematerialization, &liveness)
        .map_err(OptimizedActiveResidentRematerializationError::Ranges)?;
    let legality = analyze_allocation_legality(
        &ranges,
        source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Legality)?;
    if legality.receipt().entry_transition_count() != 0 {
        return Err(
            OptimizedActiveResidentRematerializationError::RemainingTransitions {
                count: legality.receipt().entry_transition_count(),
            },
        );
    }
    let homes = assign_register_homes(
        &legality,
        &ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Homes)?;
    let manifest = project_post_allocation_optimization_manifest(
        source_receipt.manifest(),
        &[
            PostAllocationSelectedTransformation::PressureRematerialization(
                rematerialization.receipt().identity(),
            ),
        ],
        &ranges,
        &legality,
        &homes,
    )
    .map_err(OptimizedActiveResidentRematerializationError::Manifest)?;
    let custody = custody_receipt(
        source_receipt,
        &choices,
        &classifications,
        &rematerialization,
        &liveness,
        &ranges,
        &legality,
        &homes,
        &manifest,
    );
    let staged = StagedOptimizedActiveResidentRematerialization {
        source,
        choices,
        classifications,
        rematerialization,
        liveness,
        ranges,
        legality,
        homes,
        manifest,
        custody,
    };
    Ok(staged)
}
