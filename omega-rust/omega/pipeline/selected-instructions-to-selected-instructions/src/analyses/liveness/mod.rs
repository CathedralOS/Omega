//! Optimizer module role: executable entrance. Selected-CFG liveness compute -> independent validation entrance.

use crate::*;

pub(crate) mod compute;
pub(crate) mod edge_values;
pub(crate) mod model;
pub(crate) mod validate;

#[cfg(test)]
pub(crate) mod tests;

pub use model::{LivenessError, LivenessValidationReceipt, ValidatedLiveness};
pub use validate::validate_liveness;

#[cfg(test)]
pub(crate) use compute::FUNCTION_COMPUTATIONS as LIVENESS_FUNCTION_COMPUTATIONS;

/// Compute and independently replay bounded selected-CFG liveness facts.
/// The result grants no interval, allocation, emission, or publication
/// authority.
pub fn analyze_liveness<S: ValidatedSelectedAnalysis>(
    selected: &S,
) -> Result<ValidatedLiveness, LivenessError> {
    let plan = compute::compute_terminal_liveness(selected)?;
    validate_liveness(selected, plan)
}

/// Reuse unchanged function computations, then independently replay the entire
/// result. Prior facts are candidates, never authority for the new receipt.
pub fn analyze_liveness_reusing(
    previous: &impl ValidatedSelectedAnalysis,
    previous_liveness: &ValidatedLiveness,
    selected: &impl ValidatedSelectedAnalysis,
) -> Result<ValidatedLiveness, LivenessError> {
    let plan = compute::compute_terminal_liveness_reusing(previous, previous_liveness, selected)?;
    validate_liveness(selected, plan)
}

mod staging;
pub use staging::*;
