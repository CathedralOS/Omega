use crate::{
    PostAllocationSelectedTransformation, PressureRematerializationPolicy,
    RecoveryClassificationPolicy, SpillChoicePolicy, validate_allocation_legality,
    validate_live_ranges, validate_liveness, validate_post_allocation_optimization_manifest,
    validate_pressure_rematerialization, validate_recovery_classifications,
    validate_register_homes, validate_spill_choices,
};

use crate::{
    StagedOptimizedAllocationLegality, StagedOptimizedAllocationLegalityCustodyReceipt,
    validate_optimized_allocation_legality_custody,
};

use super::custody::custody_receipt;
use super::model::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
};

pub fn validate_optimized_active_resident_rematerialization(
    staged: &StagedOptimizedActiveResidentRematerialization,
) -> Result<
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    OptimizedActiveResidentRematerializationError,
> {
    let source_receipt = validate_source(&staged.source)?;
    if staged.choices.receipt().policy()
        != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1
        || staged.classifications.receipt().policy()
            != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1
        || staged.rematerialization.receipt().policy()
            != PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1
        || staged.choices.plan().budget != staged.classifications.plan().budget
        || staged.choices.plan().budget != staged.rematerialization.plan().budget
    {
        return Err(OptimizedActiveResidentRematerializationError::UnsupportedPolicy);
    }
    let environment = staged
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let selected = staged
        .source
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .selected();
    let source_ranges = staged.source.live_range_stage().ranges();
    let choices = validate_spill_choices(
        staged.source.legality(),
        source_ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        staged.choices.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::SpillChoice)?;
    let classifications = validate_recovery_classifications(
        selected,
        source_ranges,
        staged.source.legality(),
        &choices,
        staged.classifications.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Classification)?;
    let rematerialization = validate_pressure_rematerialization(
        selected,
        source_ranges,
        staged.source.legality(),
        &choices,
        &classifications,
        staged.source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        staged.rematerialization.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Rematerialization)?;
    if rematerialization.receipt().applied_count() == 0 {
        return Err(OptimizedActiveResidentRematerializationError::NoAppliedAction);
    }
    let liveness = validate_liveness(&rematerialization, staged.liveness.plan().clone())
        .map_err(OptimizedActiveResidentRematerializationError::Liveness)?;
    let ranges = validate_live_ranges(&rematerialization, &liveness, staged.ranges.plan().clone())
        .map_err(OptimizedActiveResidentRematerializationError::Ranges)?;
    let legality = validate_allocation_legality(
        &ranges,
        staged.source.allocator_availability(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        staged.legality.plan().clone(),
    )
    .map_err(OptimizedActiveResidentRematerializationError::Legality)?;
    if legality.receipt().entry_transition_count() != 0 {
        return Err(
            OptimizedActiveResidentRematerializationError::RemainingTransitions {
                count: legality.receipt().entry_transition_count(),
            },
        );
    }
    let homes = validate_register_homes(
        &legality,
        &ranges,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
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
