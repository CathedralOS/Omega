use crate::{
    PostAllocationSelectedTransformation, PressureRematerializationPolicy,
    RecoveryClassificationPolicy, SpillChoicePolicy, analyze_allocation_legality,
    analyze_live_ranges, analyze_liveness, assign_register_homes, choose_spill_victims,
    classify_pressure_recovery, project_post_allocation_optimization_manifest,
    rematerialize_selected_active_resident,
};
use optimization_core::OptimizationWorkBudget;

use crate::StagedOptimizedAllocationLegality;

use super::custody::{custody_receipt, pressure_custody_receipt};
use super::validation::validate_source;
use super::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationPressure,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_active_resident_rematerialization_pressure(
    source: StagedOptimizedAllocationLegality,
    choice_policy: SpillChoicePolicy,
    classification_policy: RecoveryClassificationPolicy,
    rematerialization_policy: PressureRematerializationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<
    StagedOptimizedActiveResidentRematerializationPressure,
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
    let environment = source.register_environment();
    let selected = source.selected();
    let source_ranges = source.ranges();
    let choices = choose_spill_victims(
        source.legality(),
        source_ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
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
        &environment.allocation_constraint_keys(),
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
        &environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Legality)?;
    if legality.receipt().entry_transition_count() != 0 {
        return Err(
            OptimizedActiveResidentRematerializationError::RemainingTransitions {
                count: legality.receipt().entry_transition_count(),
            },
        );
    }
    let custody = pressure_custody_receipt(
        source_receipt,
        &choices,
        &classifications,
        &rematerialization,
        &liveness,
        &ranges,
        &legality,
    );
    Ok(StagedOptimizedActiveResidentRematerializationPressure {
        source,
        choices,
        classifications,
        rematerialization,
        liveness,
        ranges,
        legality,
        custody,
    })
}

/// Terminal completion over a proven pressure prefix: the caller supplies the
/// homes a successful assignment produced, so residual `NoCompatibleHome`
/// pressure is a route decision — it hands the prefix to runtime-spill
/// recovery — not a failure this function interprets.
pub(super) fn complete_active_resident_rematerialization(
    pressure: StagedOptimizedActiveResidentRematerializationPressure,
    homes: crate::ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedActiveResidentRematerialization,
    OptimizedActiveResidentRematerializationError,
> {
    let StagedOptimizedActiveResidentRematerializationPressure {
        source,
        choices,
        classifications,
        rematerialization,
        liveness,
        ranges,
        legality,
        custody: prefix_custody,
    } = pressure;
    let manifest = project_post_allocation_optimization_manifest(
        prefix_custody.source().manifest(),
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
        prefix_custody.source(),
        &choices,
        &classifications,
        &rematerialization,
        &liveness,
        &ranges,
        &legality,
        &homes,
        &manifest,
    );
    Ok(StagedOptimizedActiveResidentRematerialization {
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
    })
}

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
    let pressure = compute_active_resident_rematerialization_pressure(
        source,
        choice_policy,
        classification_policy,
        rematerialization_policy,
        budget,
    )?;
    let environment = pressure.source.register_environment();
    let homes = assign_register_homes(
        &pressure.legality,
        &pressure.ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Homes)?;
    complete_active_resident_rematerialization(pressure, homes)
}
