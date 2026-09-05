//! Optimizer module role: executable entrance. Epoch-two recursive spill scheduling.
//!
//! This join extends the validated epoch-zero/one schedule with validated
//! epoch-two logical recovery obligations and recolors the complete abstract
//! slot set. It creates no instruction, memory effect, frame address, trap,
//! encoding, emission, or publication authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::recursive_spill_insertion_identity;
pub use model::*;
pub use validate::validate_recursive_spill_insertion;

pub fn schedule_recursive_spill_insertion(
    base: &crate::ValidatedGeneralizedSpillInsertion,
    recovery: &crate::ValidatedGeneralizedSpillRecoveryActions,
    policy: RecursiveSpillInsertionPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedRecursiveSpillInsertion, RecursiveSpillInsertionError> {
    let plan = compute::compute(base, recovery, policy, budget)?;
    validate_recursive_spill_insertion(base, recovery, plan)
}
