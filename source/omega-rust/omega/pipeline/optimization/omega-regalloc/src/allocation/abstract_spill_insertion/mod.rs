//! Optimizer module role: stage group. Abstract spill-area insertion scheduling and replay.
//!
//! This join turns validated logical spill actions plus validated slot colors
//! into one exact store/reload/rewrite schedule. It deliberately stops before
//! reload-home allocation, machine opcode selection, frame addressing, unwind,
//! encoding, or publication.

mod compute;
mod identity;
mod model;
mod validate;

pub use identity::abstract_spill_insertion_identity;
pub use model::*;
pub use validate::validate_abstract_spill_insertion;

use crate::{ValidatedLogicalSpillOperations, ValidatedStackSlotColoring};

pub fn schedule_abstract_spill_insertion(
    logical: &ValidatedLogicalSpillOperations,
    slots: &ValidatedStackSlotColoring,
    policy: AbstractSpillInsertionPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedAbstractSpillInsertion, AbstractSpillInsertionError> {
    let plan = compute::compute(logical, slots, policy, budget)?;
    validate_abstract_spill_insertion(logical, slots, plan)
}
