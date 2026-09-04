use omega_regalloc::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    ValidatedPostAllocationOptimizationManifest, ValidatedPressureRematerialization,
    ValidatedRecoveryClassifications, ValidatedRegisterHomes, ValidatedSpillChoices,
};

use omega_live_ranges_to_allocation_legality::StagedOptimizedAllocationLegalityCustodyReceipt;

use super::model::StagedOptimizedActiveResidentRematerializationCustodyReceipt;

#[allow(clippy::too_many_arguments)]
pub(super) fn custody_receipt(
    source: StagedOptimizedAllocationLegalityCustodyReceipt,
    choices: &ValidatedSpillChoices,
    classifications: &ValidatedRecoveryClassifications,
    rematerialization: &ValidatedPressureRematerialization,
    liveness: &ValidatedLiveness,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
) -> StagedOptimizedActiveResidentRematerializationCustodyReceipt {
    StagedOptimizedActiveResidentRematerializationCustodyReceipt {
        source,
        choices: choices.receipt().identity(),
        choice_policy: choices.receipt().policy(),
        choice_usage: choices.receipt().usage(),
        classifications: classifications.receipt().identity(),
        classification_policy: classifications.receipt().policy(),
        classification_usage: classifications.receipt().usage(),
        rematerialization: rematerialization.receipt().identity(),
        rematerialization_policy: rematerialization.receipt().policy(),
        rematerialization_usage: rematerialization.receipt().usage(),
        budget: rematerialization.plan().budget,
        transformed_selected: rematerialization.receipt().transformed_selected(),
        liveness: liveness.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        homes: homes.receipt().identity(),
        manifest: manifest.record().identity,
        function_count: rematerialization.receipt().function_count(),
        virtual_register_count: legality.receipt().virtual_register_count(),
        applied_count: rematerialization.receipt().applied_count(),
        rewritten_use_count: rematerialization.receipt().rewritten_use_count(),
        assignment_count: homes.receipt().assignment_count(),
    }
}
