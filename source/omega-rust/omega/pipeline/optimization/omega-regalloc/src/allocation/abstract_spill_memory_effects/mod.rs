//! Optimizer module role: executable entrance. Abstract spill-area effects.
//!
//! These rows describe reads and writes of compiler-private abstract spill
//! storage. They grant no executable memory operation, address, frame, fault,
//! opcode, encoding, emission, or publication authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::abstract_spill_memory_effect_plan_identity;
pub use model::*;
pub use validate::validate_abstract_spill_memory_effects;

pub fn derive_abstract_spill_memory_effects(
    source: &crate::ValidatedHomedSpillPseudoInstructions,
    policy: AbstractSpillMemoryEffectPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAbstractSpillMemoryEffects, AbstractSpillMemoryEffectError> {
    let plan = compute::compute(source, policy, budget)?;
    validate_abstract_spill_memory_effects(source, plan)
}
