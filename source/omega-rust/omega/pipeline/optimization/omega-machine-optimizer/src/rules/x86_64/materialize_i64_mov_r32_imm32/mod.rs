//! Optimizer module role: executable entrance. Zero-extended x86-64 i64 materialization with `MOV r32, imm32`.

mod codec;
mod compute;
mod identity;
mod model;
mod validate;

#[cfg(test)]
mod tests;

pub use codec::X86MovR32Imm32MaterializationDecodeError;
pub use identity::x86_mov_r32_imm32_materialization_identity;
pub use model::*;
pub use validate::validate_x86_mov_r32_imm32_materialization;

/// Select the flag-preserving five- or six-byte form for an exact bit pattern
/// in `0..=u32::MAX`, then independently replay the symbolic decision.
pub fn optimize_x86_materialize_i64_with_mov_r32_imm32<
    S: omega_regalloc::ValidatedSelectedAnalysis,
>(
    selected: &S,
    source: &crate::ValidatedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedX86MovR32Imm32Materialization, X86MovR32Imm32MaterializationError> {
    let plan = compute::compute(selected, source, physical, budget)?;
    validate_x86_mov_r32_imm32_materialization(selected, source, physical, plan)
}
