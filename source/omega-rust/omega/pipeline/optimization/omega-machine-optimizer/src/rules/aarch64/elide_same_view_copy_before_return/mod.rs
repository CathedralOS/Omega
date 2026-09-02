//! Optimizer module role: executable entrance. Exact same-view CopyI64 elision.
//!
//! This owner proposes one terminal `CopyI64; ReturnI64` disposition through
//! the bounded declarative matcher, then joins it to an independent replay.
//! The machine catalog owns compiler admission; downstream pipeline custody
//! carries this independently validated disposition into realization.

mod compute;
mod pattern;
mod validate;

#[cfg(test)]
pub(crate) mod tests;

pub use validate::validate_aarch64_same_view_copy_elision;

use super::same_view_copy_elision::{
    Aarch64SameViewCopyElisionError, ValidatedAarch64SameViewCopyElision,
};

/// Propose and independently validate the core symbolic disposition.
pub fn optimize_aarch64_same_view_copy_i64_before_return<
    S: omega_regalloc::ValidatedSelectedAnalysis,
>(
    selected: &S,
    liveness: &omega_regalloc::ValidatedLiveness,
    source: &crate::ValidatedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAarch64SameViewCopyElision, Aarch64SameViewCopyElisionError> {
    let plan = compute::compute(selected, liveness, source, physical, budget)?;
    validate_aarch64_same_view_copy_elision(selected, liveness, source, physical, plan)
}
