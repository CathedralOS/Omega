//! Optimizer module role: executable entrance. Epoch-two logical recovery actions.
//!
//! This boundary turns a validated compiler-private reload-victim choice into
//! target-neutral store, reload, and use-rewrite obligations. It grants no
//! physical slot, frame, memory-effect, instruction, trap, or publication authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::generalized_spill_recovery_action_identity;
pub use model::*;
pub use validate::{
    validate_generalized_original_spill_recovery_actions,
    validate_generalized_spill_recovery_actions,
};

pub fn plan_generalized_spill_recovery_actions(
    insertion: &crate::ValidatedGeneralizedSpillInsertion,
    homes: &crate::ValidatedGeneralizedReloadValueHomes,
    choices: &crate::ValidatedGeneralizedSpillRecoveryChoices,
    policy: GeneralizedSpillRecoveryActionPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedGeneralizedSpillRecoveryActions, GeneralizedSpillRecoveryActionError> {
    let plan = compute::compute(insertion, homes, choices, policy, budget)?;
    validate_generalized_spill_recovery_actions(insertion, homes, choices, plan)
}

pub fn plan_generalized_original_spill_recovery_actions<S: crate::ValidatedSelectedAnalysis>(
    insertion: &crate::ValidatedGeneralizedSpillInsertion,
    homes: &crate::ValidatedGeneralizedReloadValueHomes,
    choices: &crate::ValidatedGeneralizedSpillRecoveryChoices,
    selected: &S,
    ranges: &crate::ValidatedLiveRanges,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedGeneralizedSpillRecoveryActions, GeneralizedSpillRecoveryActionError> {
    let plan = compute::compute_original(insertion, homes, choices, selected, ranges, budget)?;
    validate_generalized_original_spill_recovery_actions(
        insertion, homes, choices, selected, ranges, plan,
    )
}
