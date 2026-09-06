//! Independent reload-pressure replay, exact comparison, and receipt sealing.

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};

use crate::{
    SpillRecoveryWorklistError, SpillRecoveryWorklistPlan, SpillRecoveryWorklistReceipt,
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedLogicalSpillOperations, ValidatedSpillRecoveryWorklist,
    spill_recovery_worklist_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_spill_recovery_worklist(
    insertion: &ValidatedAbstractSpillInsertion,
    logical: &ValidatedLogicalSpillOperations,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    plan: SpillRecoveryWorklistPlan,
) -> Result<ValidatedSpillRecoveryWorklist, SpillRecoveryWorklistError> {
    let insertion_receipt = insertion.receipt();
    let logical_receipt = logical.receipt();
    if plan.abstract_spill_insertion != insertion_receipt.identity()
        || plan.logical_spill_operations != logical_receipt.identity()
        || plan.legality != legality.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.register_environment != logical_receipt.register_environment()
        || plan.allocator_availability != logical_receipt.allocator_availability()
        || plan.optimization_unit != insertion_receipt.optimization_unit()
        || plan.fuel_schedule != insertion_receipt.fuel_schedule()
    {
        return Err(SpillRecoveryWorklistError::RootMismatch);
    }
    let expected = super::replay::replay(
        insertion,
        logical,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan.reload_home_policy,
        plan.reload_home_budget,
        plan.policy,
        plan.budget,
    )?;
    if plan.usage != expected.usage {
        return Err(SpillRecoveryWorklistError::UsageMismatch);
    }
    if plan.epochs != expected.epochs {
        return Err(SpillRecoveryWorklistError::NonCanonicalWorklist);
    }
    if !plan.usage.within(plan.budget) {
        return Err(SpillRecoveryWorklistError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let work_item_count = plan.epochs.iter().try_fold(0_usize, |total, epoch| {
        total
            .checked_add(epoch.work_items.len())
            .ok_or(SpillRecoveryWorklistError::WorkOverflow)
    })?;
    let receipt = SpillRecoveryWorklistReceipt {
        identity: spill_recovery_worklist_identity(&plan),
        abstract_spill_insertion: plan.abstract_spill_insertion,
        logical_spill_operations: plan.logical_spill_operations,
        legality: plan.legality,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        usage: plan.usage,
        epoch_count: plan.epochs.len(),
        work_item_count,
    };
    Ok(ValidatedSpillRecoveryWorklist { plan, receipt })
}
