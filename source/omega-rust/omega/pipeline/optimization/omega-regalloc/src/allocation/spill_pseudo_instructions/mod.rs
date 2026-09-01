//! Optimizer module role: executable entrance. Logical spill-pseudo lowering.
//!
//! This join names compiler-private store/reload pseudos and operand rewrites
//! from the recursive logical schedule. It creates no selected or machine
//! instruction, address, memory effect, frame, trap, encoding, or publication
//! authority.

mod compute;
mod homed;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::spill_pseudo_instruction_plan_identity;
pub use homed::*;
pub use model::*;
pub use validate::validate_spill_pseudo_instructions;

pub fn lower_recursive_spill_pseudos(
    source: &crate::ValidatedRecursiveSpillInsertion,
    policy: SpillPseudoInstructionPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedSpillPseudoInstructions, SpillPseudoInstructionError> {
    let plan = compute::compute(source, policy, budget)?;
    validate_spill_pseudo_instructions(source, plan)
}
