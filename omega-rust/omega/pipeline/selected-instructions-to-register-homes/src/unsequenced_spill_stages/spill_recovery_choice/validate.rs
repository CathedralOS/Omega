//! Independent source admission, replay, comparison, and receipt sealing.

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};

use crate::{
    SpillRecoveryChoiceError, SpillRecoveryChoicePlan, SpillRecoveryChoiceReceipt,
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedSpillRecoveryChoices, ValidatedSpillRecoveryWorklist, spill_recovery_choice_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_spill_recovery_choices(
    worklist: &ValidatedSpillRecoveryWorklist,
    insertion: &ValidatedAbstractSpillInsertion,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    plan: SpillRecoveryChoicePlan,
) -> Result<ValidatedSpillRecoveryChoices, SpillRecoveryChoiceError> {
    let worklist_receipt = worklist.receipt();
    let environment = target_register_environment_identity(
        ranges.plan().target,
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    if plan.worklist != worklist_receipt.identity()
        || plan.abstract_spill_insertion != insertion.receipt().identity()
        || plan.legality != legality.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.register_environment != worklist_receipt.register_environment()
        || plan.allocator_availability != worklist_receipt.allocator_availability()
        || worklist_receipt.abstract_spill_insertion() != insertion.receipt().identity()
        || worklist_receipt.legality() != legality.receipt().identity()
        || worklist_receipt.ranges() != ranges.receipt().identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != environment
        || legality.receipt().allocator_availability() != plan.allocator_availability
        || environment != plan.register_environment
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
    {
        return Err(SpillRecoveryChoiceError::RootMismatch);
    }
    super::compute::admit_policy(plan.policy)?;
    let expected = super::replay::replay(
        worklist,
        insertion,
        legality,
        ranges,
        physical,
        plan.policy,
        plan.budget,
    )?;
    if plan.usage != expected.usage {
        return Err(SpillRecoveryChoiceError::UsageMismatch);
    }
    if plan.choices != expected.choices {
        return Err(SpillRecoveryChoiceError::NonCanonicalChoice);
    }
    if !plan.usage.within(plan.budget) {
        return Err(SpillRecoveryChoiceError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let contender_count = plan.choices.iter().try_fold(0_usize, |total, choice| {
        total
            .checked_add(choice.contenders.len())
            .ok_or(SpillRecoveryChoiceError::WorkOverflow)
    })?;
    let receipt = SpillRecoveryChoiceReceipt {
        identity: spill_recovery_choice_identity(&plan),
        worklist: plan.worklist,
        abstract_spill_insertion: plan.abstract_spill_insertion,
        legality: plan.legality,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        usage: plan.usage,
        choice_count: plan.choices.len(),
        contender_count,
    };
    Ok(ValidatedSpillRecoveryChoices { plan, receipt })
}
