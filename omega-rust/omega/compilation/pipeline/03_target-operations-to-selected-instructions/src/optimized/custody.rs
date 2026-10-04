use abstract_operations_to_target_operations::ValidatedOptimizedTargetOperations;

use crate::{ValidatedLegalizedOperations, ValidatedSelectedInstructions};
use register_environment::ValidatedTargetRegisterEnvironment;

use super::model::StagedOptimizedSelectionCustodyReceipt;

/// Record the identities this stage just produced. Legalization and selection
/// each returned only after their own independent checker accepted the
/// result, so the stage does not replay either join again
/// (verification placement in `omega-rust/pipeline.md`).
pub(super) fn selection_custody_receipt(
    optimized_target: &ValidatedOptimizedTargetOperations,
    register_environment: &ValidatedTargetRegisterEnvironment,
    legalized: &ValidatedLegalizedOperations,
    selected: &ValidatedSelectedInstructions,
) -> StagedOptimizedSelectionCustodyReceipt {
    let target = optimized_target.target_operations();
    let unit = optimized_target.optimized().unit();
    StagedOptimizedSelectionCustodyReceipt {
        psi: target.psi,
        target: target.target,
        entry: target.entry,
        optimization: optimized_target.optimized().identity_bundle().identity(),
        projection: optimized_target.optimized().validation().identity(),
        manifest: optimized_target
            .optimized()
            .pre_physical_manifest()
            .record()
            .identity,
        optimization_unit: unit.identity,
        fuel_schedule: unit.fuel_schedule,
        register_environment: register_environment.identity(),
        legalized: legalized.receipt().identity(),
        legalization_validator: legalized.receipt().validator(),
        selected: selected.receipt().identity(),
        function_count: selected.plan().functions.len(),
    }
}
