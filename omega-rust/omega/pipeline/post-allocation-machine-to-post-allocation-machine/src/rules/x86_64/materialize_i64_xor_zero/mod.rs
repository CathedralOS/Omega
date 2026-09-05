//! Optimizer module role: executable entrance. Flag-safe x86-64 zero materialization with `XOR r64, r64`.

mod codec;
mod compute;
mod identity;
mod model;
mod validate;

#[cfg(test)]
mod tests;

pub use codec::X86XorZeroMaterializationDecodeError;
pub use identity::x86_xor_zero_materialization_identity;
pub use model::*;
pub use validate::validate_x86_xor_zero_materialization;

/// Select the symbolic three-byte XOR-zero form only when independently
/// validated selected-CFG liveness proves every canonical RFLAGS unit dead.
/// This owns no encoded bytes and grants no emission authority.
pub fn optimize_x86_materialize_i64_zero_with_xor<
    S: selected_instructions_to_register_homes::ValidatedSelectedAnalysis,
>(
    selected: &S,
    liveness: &selected_instructions_to_register_homes::ValidatedLiveness,
    source: &register_homes_to_post_allocation_machine::ValidatedPostAllocationMachinePlan,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedX86XorZeroMaterialization, X86XorZeroMaterializationError> {
    let plan = compute::compute(selected, liveness, source, physical, budget)?;
    validate_x86_xor_zero_materialization(selected, liveness, source, physical, plan)
}
