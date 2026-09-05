//! Optimizer module role: executable entrance. Epoch-one logical second-spill actions.
//!
//! This boundary turns validated recovery-victim choices into target-neutral
//! storage, store, later reload, and complete rewrite obligations. It creates
//! no selected identity or instruction and grants no memory, slot, frame,
//! trap, unwind, encoding, emission, or publication authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::spill_recovery_action_identity;
pub use model::*;
pub use validate::validate_spill_recovery_actions;

use crate::{
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedSelectedAnalysis, ValidatedSpillRecoveryChoices, ValidatedSpillRecoveryWorklist,
};

pub fn plan_spill_recovery_actions<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    insertion: &ValidatedAbstractSpillInsertion,
    worklist: &ValidatedSpillRecoveryWorklist,
    choices: &ValidatedSpillRecoveryChoices,
    policy: SpillRecoveryActionPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedSpillRecoveryActions, SpillRecoveryActionError> {
    let plan = compute::compute(
        selected, ranges, legality, insertion, worklist, choices, policy, budget,
    )?;
    validate_spill_recovery_actions(
        selected, ranges, legality, insertion, worklist, choices, plan,
    )
}
