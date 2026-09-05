//! Optimizer module role: executable entrance. `CMP Xn, #0; B.NE` to `CBNZ Xn` symbolic fusion.

mod codec;
mod compute;
mod identity;
mod model;
mod pattern;
mod validate;

#[cfg(test)]
mod tests;

pub use codec::Aarch64CbnzFusionDecodeError;
pub use identity::aarch64_cbnz_fusion_identity;
pub use model::*;
pub use validate::validate_aarch64_cbnz_fusion;

/// Apply this exact named transformation without assigning a displacement or
/// encoding bytes.
pub fn optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz<
    S: selected_instructions_to_register_homes::ValidatedSelectedAnalysis,
>(
    selected: &S,
    liveness: &selected_instructions_to_register_homes::ValidatedLiveness,
    source: &register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAarch64CbnzFusion, Aarch64CbnzFusionError> {
    let plan = compute::compute(selected, liveness, source, physical, budget)?;
    validate_aarch64_cbnz_fusion(selected, liveness, source, physical, plan)
}
