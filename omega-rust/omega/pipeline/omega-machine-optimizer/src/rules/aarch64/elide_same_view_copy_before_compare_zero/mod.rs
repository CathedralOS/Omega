//! Optimizer module role: executable entrance. Exact non-terminal same-view copy elision.
//!
//! This owner proposes one adjacent body `CopyI64; CompareI64Zero` disposition
//! through the bounded descriptor matcher, then joins it to an independent
//! shared-family replay under this leaf's exact contract. The shared artifact
//! family supplies codec and custody, and the machine catalog remains the only
//! compiler admission point.

mod compute;
mod contract;
mod pattern;
mod validate;

#[cfg(test)]
mod tests;

use crate::{Aarch64SameViewCopyElisionError, ValidatedAarch64SameViewCopyElision};

pub fn optimize_aarch64_same_view_copy_i64_before_compare_zero<
    S: omega_regalloc::ValidatedSelectedAnalysis,
>(
    selected: &S,
    liveness: &omega_regalloc::ValidatedLiveness,
    source: &crate::ValidatedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAarch64SameViewCopyElision, Aarch64SameViewCopyElisionError> {
    let plan = compute::compute(selected, liveness, source, physical, budget)?;
    validate::validate(selected, liveness, source, physical, plan)
}

pub use validate::validate_aarch64_same_view_copy_i64_before_compare_zero;
