//! Optimizer module role: executable entrance. Exact right-consumed same-view copy elision.
//!
//! This owner proposes one adjacent body `CopyI64; CompareI64` disposition
//! through the bounded descriptor matcher, then joins it to an independent
//! shared-family replay under this leaf's exact right-operand contract. The
//! machine catalog remains the only compiler admission point.

mod compute;
mod contract;
mod pattern;
mod validate;

#[cfg(test)]
mod tests;

use crate::{Aarch64SameViewCopyElisionError, ValidatedAarch64SameViewCopyElision};

pub fn optimize_aarch64_same_view_copy_i64_before_compare_i64_right_operand<
    S: selected_instructions_to_register_homes::ValidatedSelectedAnalysis,
>(
    selected: &S,
    liveness: &selected_instructions_to_register_homes::ValidatedLiveness,
    source: &register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAarch64SameViewCopyElision, Aarch64SameViewCopyElisionError> {
    let plan = compute::compute(selected, liveness, source, physical, budget)?;
    validate::validate(selected, liveness, source, physical, plan)
}

pub use validate::validate_aarch64_same_view_copy_i64_before_compare_i64_right_operand;
