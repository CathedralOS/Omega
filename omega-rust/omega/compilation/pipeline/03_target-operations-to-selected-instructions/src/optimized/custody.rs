use crate::{ValidatedLegalizedOperations, ValidatedSelectedInstructions};
use register_environment::ValidatedTargetRegisterEnvironment;

use super::input::TargetSelectionInput;
use super::model::StagedOptimizedSelectionCustodyReceipt;

use selected_instructions::StagedOptimizedCustodyCore;

/// Record the identities this stage just produced. Legalization and selection
/// each returned only after their own independent checker accepted the
/// result, so the stage does not replay either join again
/// (verification placement in `omega-rust/pipeline.md`).
pub(super) fn selection_custody_receipt(
    input: &TargetSelectionInput<'_>,
    register_environment: &ValidatedTargetRegisterEnvironment,
    legalized: &ValidatedLegalizedOperations,
    selected: &ValidatedSelectedInstructions,
) -> StagedOptimizedSelectionCustodyReceipt {
    let (target, unit) = (input.target, input.unit);
    StagedOptimizedSelectionCustodyReceipt {
        core: StagedOptimizedCustodyCore {
            psi: target.psi,
            target: target.target,
            entry: target.entry,
            optimization: input.optimization,
            projection: input.projection,
            manifest: input.manifest,
            optimization_unit: unit.identity,
            fuel_schedule: unit.fuel_schedule,
            register_environment: register_environment.identity(),
        },
        legalized: legalized.receipt().identity(),
        legalization_validator: legalized.receipt().validator(),
        selected: selected.receipt().identity(),
        function_count: selected.plan().functions.len(),
    }
}
