//! Optimizer module role: executable entrance. V2 homed spill-pseudo lowering.
//!
//! This join enriches validated V1 compiler-private pseudos with the exact
//! destination view from final recursive reload-home closure. It creates no
//! selected or machine instruction, address, memory effect, frame, trap,
//! encoding, emission, or publication authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::homed_spill_pseudo_instruction_plan_identity;
pub use model::*;
pub use validate::validate_homed_spill_pseudo_instructions;

pub fn lower_homed_recursive_spill_pseudos(
    source: &crate::ValidatedSpillPseudoInstructions,
    homes: &crate::ValidatedRecursiveReloadValueHomes,
    policy: HomedSpillPseudoInstructionPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedHomedSpillPseudoInstructions, HomedSpillPseudoInstructionError> {
    let plan = compute::compute(source, homes, policy, budget)?;
    validate_homed_spill_pseudo_instructions(source, homes, plan)
}
