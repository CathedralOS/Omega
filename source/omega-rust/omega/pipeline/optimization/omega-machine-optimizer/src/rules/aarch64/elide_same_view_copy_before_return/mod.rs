//! Optimizer module role: executable entrance. Exact same-view CopyI64 elision.
//!
//! This owner proposes one terminal `CopyI64; ReturnI64` disposition through
//! the bounded declarative matcher, then joins it to an independent replay.
//! The machine catalog owns compiler admission; downstream pipeline custody
//! carries this independently validated disposition into realization.

mod codec;
mod compute;
mod identity;
mod model;
mod pattern;
mod validate;

#[cfg(test)]
mod tests;

pub use identity::aarch64_same_view_copy_elision_identity;
pub use model::*;
pub use validate::validate_aarch64_same_view_copy_elision;

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
