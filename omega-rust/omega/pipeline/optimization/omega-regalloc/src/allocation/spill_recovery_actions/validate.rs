//! Independent source admission, replay, comparison, and receipt sealing.

use crate::{
    SpillRecoveryActionError, SpillRecoveryActionPlan, SpillRecoveryActionReceipt,
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedSelectedAnalysis, ValidatedSpillRecoveryActions, ValidatedSpillRecoveryChoices,
    ValidatedSpillRecoveryWorklist, spill_recovery_action_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_spill_recovery_actions<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    insertion: &ValidatedAbstractSpillInsertion,
    worklist: &ValidatedSpillRecoveryWorklist,
    choices: &ValidatedSpillRecoveryChoices,
    plan: SpillRecoveryActionPlan,
) -> Result<ValidatedSpillRecoveryActions, SpillRecoveryActionError> {
    let choice = choices.receipt();
    let work = worklist.receipt();
    if plan.selected != selected.selected_identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.legality != legality.receipt().identity()
        || plan.abstract_spill_insertion != insertion.receipt().identity()
        || plan.worklist != work.identity()
        || plan.choices != choice.identity()
        || plan.register_environment != choice.register_environment()
        || plan.allocator_availability != choice.allocator_availability()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || insertion.receipt().optimization_unit() != selected.optimization_unit_identity()
        || insertion.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || work.abstract_spill_insertion() != insertion.receipt().identity()
        || work.legality() != legality.receipt().identity()
        || work.ranges() != ranges.receipt().identity()
        || work.optimization_unit() != selected.optimization_unit_identity()
        || work.fuel_schedule() != selected.fuel_schedule_identity()
        || choice.worklist() != work.identity()
        || choice.abstract_spill_insertion() != insertion.receipt().identity()
        || choice.legality() != legality.receipt().identity()
        || choice.ranges() != ranges.receipt().identity()
        || choice.register_environment() != work.register_environment()
        || choice.allocator_availability() != work.allocator_availability()
    {
        return Err(SpillRecoveryActionError::RootMismatch);
    }
    super::compute::admit_policy(plan.policy)?;
    for action in &plan.actions {
        let namespace = (
            action.source_work_item.epoch,
            action.source_work_item.ordinal,
        );
        if namespace != (action.storage.id.epoch, action.storage.id.ordinal)
            || namespace != (action.reload.result.epoch, action.reload.result.ordinal)
            || action.store.storage != action.storage.id
            || action.reload.storage != action.storage.id
            || action
                .rewrites
                .iter()
                .any(|rewrite| rewrite.result != action.reload.result)
        {
            return Err(SpillRecoveryActionError::NonCanonicalNamespace);
        }
    }
    let expected = super::replay::replay(
        selected,
        ranges,
        legality,
        insertion,
        worklist,
        choices,
        plan.policy,
        plan.budget,
    )?;
    if plan.usage != expected.usage {
        return Err(SpillRecoveryActionError::UsageMismatch);
    }
    if plan.actions != expected.actions {
        return Err(SpillRecoveryActionError::NonCanonicalActions);
    }
    if !plan.usage.within(plan.budget) {
        return Err(SpillRecoveryActionError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let rewrite_count = plan.actions.iter().try_fold(0_usize, |total, action| {
        total
            .checked_add(action.rewrites.len())
            .ok_or(SpillRecoveryActionError::WorkOverflow)
    })?;
    let receipt = SpillRecoveryActionReceipt {
        identity: spill_recovery_action_identity(&plan),
        choices: plan.choices,
        worklist: plan.worklist,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        usage: plan.usage,
        action_count: plan.actions.len(),
        rewrite_count,
    };
    Ok(ValidatedSpillRecoveryActions { plan, receipt })
}
