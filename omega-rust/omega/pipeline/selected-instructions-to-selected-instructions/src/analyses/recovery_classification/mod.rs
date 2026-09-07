//! Optimizer module role: executable entrance. Exact pressure-recovery eligibility classification entrance.

use crate::*;

pub(crate) mod compute;
pub(crate) mod model;
pub(crate) mod validate;

pub use model::{
    RecoveryClassificationError, RecoveryClassificationValidationReceipt,
    ValidatedRecoveryClassifications,
};
pub use validate::validate_recovery_classifications;

/// Classify an already selected pressure victim under one exact recovery
/// eligibility policy without selecting or applying a recovery strategy.
pub fn classify_pressure_recovery<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    policy: RecoveryClassificationPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedRecoveryClassifications, RecoveryClassificationError> {
    let plan = compute::compute_terminal_recovery_classifications(
        selected,
        ranges,
        legality,
        spill_choices,
        policy,
        budget,
    )?;
    validate_recovery_classifications(selected, ranges, legality, spill_choices, plan)
}
