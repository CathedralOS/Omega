//! Independent replay comparison and epoch-two action receipt sealing.

use crate::{
    GeneralizedSpillRecoveryActionError, GeneralizedSpillRecoveryActionPlan,
    GeneralizedSpillRecoveryActionPolicy, GeneralizedSpillRecoveryActionReceipt,
    ValidatedGeneralizedReloadValueHomes, ValidatedGeneralizedSpillInsertion,
    ValidatedGeneralizedSpillRecoveryActions, ValidatedGeneralizedSpillRecoveryChoices,
    ValidatedLiveRanges, ValidatedSelectedAnalysis, generalized_spill_recovery_action_identity,
};

pub fn validate_generalized_spill_recovery_actions(
    insertion: &ValidatedGeneralizedSpillInsertion,
    homes: &ValidatedGeneralizedReloadValueHomes,
    choices: &ValidatedGeneralizedSpillRecoveryChoices,
    plan: GeneralizedSpillRecoveryActionPlan,
) -> Result<ValidatedGeneralizedSpillRecoveryActions, GeneralizedSpillRecoveryActionError> {
    let inserted = insertion.receipt();
    let home = homes.receipt();
    let choice = choices.receipt();
    if plan.generalized_spill_insertion != inserted.identity()
        || plan.reload_value_homes != home.identity()
        || plan.choices != choice.identity()
        || plan.selected.is_some()
        || plan.ranges.is_some()
        || plan.register_environment != home.register_environment()
        || plan.allocator_availability != home.allocator_availability()
        || plan.optimization_unit != home.optimization_unit()
        || plan.fuel_schedule != home.fuel_schedule()
    {
        return Err(GeneralizedSpillRecoveryActionError::RootMismatch);
    }
    let expected = super::replay::replay(insertion, homes, choices, plan.policy, plan.budget)?;
    seal(plan, expected)
}

pub fn validate_generalized_original_spill_recovery_actions<S: ValidatedSelectedAnalysis>(
    insertion: &ValidatedGeneralizedSpillInsertion,
    homes: &ValidatedGeneralizedReloadValueHomes,
    choices: &ValidatedGeneralizedSpillRecoveryChoices,
    selected: &S,
    ranges: &ValidatedLiveRanges,
    plan: GeneralizedSpillRecoveryActionPlan,
) -> Result<ValidatedGeneralizedSpillRecoveryActions, GeneralizedSpillRecoveryActionError> {
    let inserted = insertion.receipt();
    let home = homes.receipt();
    let choice = choices.receipt();
    if plan.generalized_spill_insertion != inserted.identity()
        || plan.reload_value_homes != home.identity()
        || plan.choices != choice.identity()
        || plan.selected != Some(selected.selected_identity())
        || plan.ranges != Some(ranges.receipt().identity())
        || choice.selected() != selected.selected_identity()
        || choice.ranges() != ranges.receipt().identity()
        || ranges.receipt().selected() != selected.selected_identity()
        || plan.register_environment != home.register_environment()
        || plan.allocator_availability != home.allocator_availability()
        || plan.optimization_unit != home.optimization_unit()
        || plan.fuel_schedule != home.fuel_schedule()
        || plan.policy
            != GeneralizedSpillRecoveryActionPolicy::EpochTwoOriginalVictimLaterSelectedRewritesV1
    {
        return Err(GeneralizedSpillRecoveryActionError::RootMismatch);
    }
    let expected =
        super::replay::replay_original(insertion, homes, choices, selected, ranges, plan.budget)?;
    seal(plan, expected)
}

fn seal(
    plan: GeneralizedSpillRecoveryActionPlan,
    expected: GeneralizedSpillRecoveryActionPlan,
) -> Result<ValidatedGeneralizedSpillRecoveryActions, GeneralizedSpillRecoveryActionError> {
    if plan.usage != expected.usage {
        return Err(GeneralizedSpillRecoveryActionError::UsageMismatch);
    }
    if plan.actions != expected.actions {
        return Err(GeneralizedSpillRecoveryActionError::NonCanonicalActions);
    }
    if !plan.usage.within(plan.budget) {
        return Err(GeneralizedSpillRecoveryActionError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let rewrite_count = plan.actions.iter().try_fold(0_usize, |total, action| {
        total
            .checked_add(action.rewrites.len())
            .ok_or(GeneralizedSpillRecoveryActionError::WorkOverflow)
    })?;
    let receipt = GeneralizedSpillRecoveryActionReceipt {
        identity: generalized_spill_recovery_action_identity(&plan),
        generalized_spill_insertion: plan.generalized_spill_insertion,
        reload_value_homes: plan.reload_value_homes,
        choices: plan.choices,
        selected: plan.selected,
        ranges: plan.ranges,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        usage: plan.usage,
        action_count: plan.actions.len(),
        rewrite_count,
    };
    Ok(ValidatedGeneralizedSpillRecoveryActions { plan, receipt })
}
