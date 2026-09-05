//! Optimizer module role: executable entrance. Exact non-terminal same-view copy elision.
//!
//! This owner proposes one adjacent body `CopyI64; CompareI64` disposition
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

pub fn optimize_aarch64_same_view_copy_i64_before_compare_i64_left_operand<
    S: omega_selected_instructions_to_register_homes::ValidatedSelectedAnalysis,
>(
    selected: &S,
    liveness: &omega_selected_instructions_to_register_homes::ValidatedLiveness,
    source: &omega_register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAarch64SameViewCopyElision, Aarch64SameViewCopyElisionError> {
    let plan = compute::compute(selected, liveness, source, physical, budget)?;
    validate::validate(selected, liveness, source, physical, plan)
}

pub use validate::validate_aarch64_same_view_copy_i64_before_compare_i64_left_operand;
