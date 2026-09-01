//! Independent replay comparison and generalized worklist receipt sealing.

use crate::{
    GeneralizedSpillRecoveryWorklistError, GeneralizedSpillRecoveryWorklistPlan,
    GeneralizedSpillRecoveryWorklistReceipt, ValidatedGeneralizedReloadValueHomes,
    ValidatedGeneralizedSpillRecoveryWorklist, generalized_spill_recovery_worklist_identity,
};

pub fn validate_generalized_spill_recovery_worklist(
    source: &ValidatedGeneralizedReloadValueHomes,
    plan: GeneralizedSpillRecoveryWorklistPlan,
) -> Result<ValidatedGeneralizedSpillRecoveryWorklist, GeneralizedSpillRecoveryWorklistError> {
    let source_receipt = source.receipt();
    if plan.reload_value_homes != source_receipt.identity()
        || plan.generalized_spill_insertion != source_receipt.generalized_spill_insertion()
        || plan.abstract_spill_insertion != source_receipt.abstract_spill_insertion()
        || plan.spill_recovery_actions != source_receipt.spill_recovery_actions()
        || plan.selected != source_receipt.selected()
        || plan.ranges != source_receipt.ranges()
        || plan.legality != source_receipt.legality()
        || plan.register_environment != source_receipt.register_environment()
        || plan.allocator_availability != source_receipt.allocator_availability()
        || plan.optimization_unit != source_receipt.optimization_unit()
        || plan.fuel_schedule != source_receipt.fuel_schedule()
    {
        return Err(GeneralizedSpillRecoveryWorklistError::RootMismatch);
    }
    let expected = super::replay::replay(source, plan.policy, plan.budget)?;
    if plan.usage != expected.usage {
        return Err(GeneralizedSpillRecoveryWorklistError::UsageMismatch);
    }
    if plan.functions.len() != expected.functions.len() {
        return Err(GeneralizedSpillRecoveryWorklistError::RootMismatch);
    }
    for (function, (candidate, replayed)) in
        plan.functions.iter().zip(&expected.functions).enumerate()
    {
        if candidate != replayed {
            return Err(GeneralizedSpillRecoveryWorklistError::NonCanonicalWorklist { function });
        }
    }
    if !plan.usage.within(plan.budget) {
        return Err(GeneralizedSpillRecoveryWorklistError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let work_item_count = plan
        .functions
        .iter()
        .filter(|function| function.item.is_some())
        .count();
    let blocking_home_count = plan.functions.iter().try_fold(0_usize, |total, function| {
        total
            .checked_add(
                function
                    .item
                    .as_ref()
                    .map_or(0, |item| item.blocking_homes.len()),
            )
            .ok_or(GeneralizedSpillRecoveryWorklistError::WorkOverflow)
    })?;
    let receipt = GeneralizedSpillRecoveryWorklistReceipt {
        identity: generalized_spill_recovery_worklist_identity(&plan),
        reload_value_homes: plan.reload_value_homes,
        generalized_spill_insertion: plan.generalized_spill_insertion,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        usage: plan.usage,
        function_count: plan.functions.len(),
        work_item_count,
        blocking_home_count,
    };
    Ok(ValidatedGeneralizedSpillRecoveryWorklist { plan, receipt })
}
