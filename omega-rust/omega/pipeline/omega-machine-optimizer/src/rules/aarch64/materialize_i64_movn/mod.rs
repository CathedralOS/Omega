//! Optimizer module role: executable entrance. Shortest MOVN-seeded AArch64 i64 materialization selection.

mod codec;
mod compute;
mod identity;
mod model;
mod validate;

pub use codec::Aarch64MovnMaterializationDecodeError;
pub use identity::aarch64_movn_materialization_identity;
pub use model::*;
pub use validate::validate_aarch64_movn_materialization;

/// Select a MOVN-seeded symbolic sequence only when it strictly reduces the
/// declared zero-seeded instruction count. This owns no encoded bytes.
pub fn optimize_aarch64_materialize_i64_with_shortest_movn_seed<
    S: omega_selected_instructions_to_register_homes::ValidatedSelectedAnalysis,
>(
    selected: &S,
    source: &crate::ValidatedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAarch64MovnMaterialization, Aarch64MovnMaterializationError> {
    let plan = compute::compute(selected, source, physical, budget)?;
    validate_aarch64_movn_materialization(selected, source, physical, plan)
}
