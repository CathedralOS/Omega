//! Optimizer module role: executable entrance. Selected-CFG liveness compute -> independent validation entrance.

use crate::*;

pub(crate) mod compute;
pub(crate) mod identity;
pub(crate) mod model;
pub(crate) mod validate;

#[cfg(test)]
pub(crate) mod tests;

pub use identity::liveness_identity;
pub use model::*;
pub use validate::validate_liveness;

/// Compute and independently replay bounded selected-CFG liveness facts.
/// The result grants no interval, allocation, emission, or publication
/// authority.
pub fn analyze_liveness<S: ValidatedSelectedAnalysis>(
    selected: &S,
) -> Result<ValidatedLiveness, LivenessError> {
    let plan = compute::compute_terminal_liveness(selected)?;
    validate_liveness(selected, plan)
}

mod staging;
pub use staging::*;
