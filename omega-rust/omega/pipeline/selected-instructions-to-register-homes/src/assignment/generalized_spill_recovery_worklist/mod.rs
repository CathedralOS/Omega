//! Optimizer module role: executable entrance. Epoch-two recovery work seeding.
//!
//! This boundary consumes retained generalized reload pressure and grants one
//! compiler-private work identity. It chooses no victim or home and creates no
//! selected VReg, instruction, memory effect, frame, trap, or publication
//! authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::generalized_spill_recovery_worklist_identity;
pub use model::*;
pub use validate::validate_generalized_spill_recovery_worklist;

pub fn seed_generalized_spill_recovery_worklist(
    source: &crate::ValidatedGeneralizedReloadValueHomes,
    policy: GeneralizedSpillRecoveryWorklistPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedGeneralizedSpillRecoveryWorklist, GeneralizedSpillRecoveryWorklistError> {
    let plan = compute::compute(source, policy, budget)?;
    validate_generalized_spill_recovery_worklist(source, plan)
}
