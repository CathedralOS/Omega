//! Optimizer module role: executable entrance. Sign-extended x86-64 i64 materialization with `MOV r64, imm32`.

mod codec;
mod compute;
mod identity;
mod model;
mod validate;

#[cfg(test)]
mod tests;

pub use codec::X86MovR64Imm32SignExtendedMaterializationDecodeError;
pub use identity::x86_mov_r64_imm32_sign_extended_materialization_identity;
pub use model::*;
pub use validate::validate_x86_mov_r64_imm32_sign_extended_materialization;

/// Select the flag-preserving seven-byte form for an exact sign-extended i32
/// bit pattern, then independently replay the symbolic decision.
pub fn optimize_x86_materialize_i64_with_mov_r64_imm32_sign_extended<
    S: omega_selected_instructions_to_register_homes::ValidatedSelectedAnalysis,
>(
    selected: &S,
    source: &crate::ValidatedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<
    ValidatedX86MovR64Imm32SignExtendedMaterialization,
    X86MovR64Imm32SignExtendedMaterializationError,
> {
    let plan = compute::compute(selected, source, physical, budget)?;
    validate_x86_mov_r64_imm32_sign_extended_materialization(selected, source, physical, plan)
}
