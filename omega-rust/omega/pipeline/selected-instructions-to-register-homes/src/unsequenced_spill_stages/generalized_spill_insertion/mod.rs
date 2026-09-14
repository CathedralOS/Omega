//! Optimizer module role: executable entrance. Epoch-zero/one abstract spill scheduling.
//!
//! This join recolors the validated first-spill insertion together with the
//! validated second-spill logical actions, then emits one target-neutral event
//! schedule. It creates no virtual register, instruction, memory effect, frame
//! address, trap claim, encoding, emission, or publication authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::generalized_spill_insertion_identity;
pub use model::*;
pub use validate::validate_generalized_spill_insertion;

use crate::{ValidatedAbstractSpillInsertion, ValidatedSpillRecoveryActions};

pub fn schedule_generalized_spill_insertion(
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    policy: GeneralizedSpillInsertionPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedGeneralizedSpillInsertion, GeneralizedSpillInsertionError> {
    let plan = compute::compute(first, second, policy, budget)?;
    validate_generalized_spill_insertion(first, second, plan)
}
