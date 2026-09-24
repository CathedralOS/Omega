use crate::{validate_post_allocation_optimization_manifest, validate_register_homes};
use register_homes::{
    PostAllocationSelectedTransformation, RecoveryClassificationPolicy, SpillChoicePolicy,
};
use selected_instructions_to_selected_instructions::{
    PressureRematerializationPolicy, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedLiveness, ValidatedPressureRematerialization, ValidatedRecoveryClassifications,
    ValidatedSpillChoices, validate_allocation_legality, validate_live_ranges, validate_liveness,
    validate_pressure_rematerialization, validate_recovery_classifications, validate_spill_choices,
};

use selected_instructions_to_selected_instructions::{
    StagedOptimizedAllocationLegality, StagedOptimizedAllocationLegalityCustodyReceipt,
    validate_optimized_allocation_legality_custody,
};

use super::custody::{custody_receipt, pressure_custody_receipt};
use super::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedActiveResidentRematerializationPressure,
    StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
};

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn replay_prefix(
    source: &StagedOptimizedAllocationLegality,
    staged_choices: &ValidatedSpillChoices,
    staged_classifications: &ValidatedRecoveryClassifications,
    staged_rematerialization: &ValidatedPressureRematerialization,
    staged_liveness: &ValidatedLiveness,
    staged_ranges: &ValidatedLiveRanges,
    staged_legality: &ValidatedAllocationLegality,
) -> Result<
    (
        StagedOptimizedAllocationLegalityCustodyReceipt,
        ValidatedSpillChoices,
        ValidatedRecoveryClassifications,
        ValidatedPressureRematerialization,
        ValidatedLiveness,
        ValidatedLiveRanges,
        ValidatedAllocationLegality,
    ),
    OptimizedActiveResidentRematerializationError,
> {
    let source_receipt = validate_source(source)?;
    if staged_choices.receipt().policy()
        != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1
        || staged_classifications.receipt().policy()
            != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1
        || staged_rematerialization.receipt().policy()
            != PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1
        || staged_choices.plan().budget != staged_classifications.plan().budget
        || staged_choices.plan().budget != staged_rematerialization.plan().budget
    {
        return Err(OptimizedActiveResidentRematerializationError::UnsupportedPolicy);
    }
    let environment = source.register_environment();
    let selected = source.selected();
    let source_ranges = source.ranges();
    let choices = validate_spill_choices(
        source.legality(),
        source_ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        staged_choices.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::SpillChoice)?;
    let classifications = validate_recovery_classifications(
        selected,
        source_ranges,
        source.legality(),
        &choices,
        staged_classifications.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Classification)?;
    let rematerialization = validate_pressure_rematerialization(
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
        staged_rematerialization.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Rematerialization)?;
    if rematerialization.receipt().applied_count() == 0 {
        return Err(OptimizedActiveResidentRematerializationError::NoAppliedAction);
    }
    let liveness = validate_liveness(&rematerialization, staged_liveness.plan().clone())
        .map_err(OptimizedActiveResidentRematerializationError::Liveness)?;
    let ranges = validate_live_ranges(&rematerialization, &liveness, staged_ranges.plan().clone())
        .map_err(OptimizedActiveResidentRematerializationError::Ranges)?;
    let legality = validate_allocation_legality(
        &ranges,
        source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        staged_legality.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Legality)?;
    if legality.receipt().entry_transition_count() != 0 {
        return Err(
            OptimizedActiveResidentRematerializationError::RemainingTransitions {
                count: legality.receipt().entry_transition_count(),
            },
        );
    }
    Ok((
        source_receipt,
        choices,
        classifications,
        rematerialization,
        liveness,
        ranges,
        legality,
    ))
}

/// Independently replay the proven prefix before terminal homes assignment:
/// the sweep, its rebuilt analyses, and the recorded pressure custody must
/// reproduce exactly. This is the custody runtime-spill recovery replays
/// before trusting the prefix as its source.
pub fn validate_optimized_active_resident_rematerialization_pressure(
    staged: &StagedOptimizedActiveResidentRematerializationPressure,
) -> Result<
    StagedOptimizedActiveResidentRematerializationPressureCustodyReceipt,
    OptimizedActiveResidentRematerializationError,
> {
    let (source_receipt, choices, classifications, rematerialization, liveness, ranges, legality) =
        replay_prefix(
            &staged.source,
            &staged.choices,
            &staged.classifications,
            &staged.rematerialization,
            &staged.liveness,
            &staged.ranges,
            &staged.legality,
        )?;
    let custody = pressure_custody_receipt(
        source_receipt,
        &choices,
        &classifications,
        &rematerialization,
        &liveness,
        &ranges,
        &legality,
    );
    if choices != staged.choices
        || classifications != staged.classifications
        || rematerialization != staged.rematerialization
        || liveness != staged.liveness
        || ranges != staged.ranges
        || legality != staged.legality
        || custody != staged.custody
    {
        return Err(OptimizedActiveResidentRematerializationError::ReceiptMismatch);
    }
    Ok(custody)
}

pub fn validate_optimized_active_resident_rematerialization(
    staged: &StagedOptimizedActiveResidentRematerialization,
) -> Result<
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    OptimizedActiveResidentRematerializationError,
> {
    let (source_receipt, choices, classifications, rematerialization, liveness, ranges, legality) =
        replay_prefix(
            &staged.source,
            &staged.choices,
            &staged.classifications,
            &staged.rematerialization,
            &staged.liveness,
            &staged.ranges,
            &staged.legality,
        )?;
    let environment = staged.source.register_environment();
    let homes = validate_register_homes(
        &legality,
        &ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        staged.homes.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Homes)?;
    let manifest = validate_post_allocation_optimization_manifest(
        staged.manifest.record(),
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
    if choices != staged.choices
        || classifications != staged.classifications
        || rematerialization != staged.rematerialization
        || liveness != staged.liveness
        || ranges != staged.ranges
        || legality != staged.legality
        || homes != staged.homes
        || manifest != staged.manifest
        || custody != staged.custody
    {
        return Err(OptimizedActiveResidentRematerializationError::ReceiptMismatch);
    }
    Ok(custody)
}
pub(super) fn validate_source(
    source: &StagedOptimizedAllocationLegality,
) -> Result<
    StagedOptimizedAllocationLegalityCustodyReceipt,
    OptimizedActiveResidentRematerializationError,
> {
    validate_optimized_allocation_legality_custody(
        source.live_range_stage(),
        source.allocator_availability(),
        source.legality(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Upstream)
}
