//! Independent replay, exact comparison, and receipt sealing.

use crate::{
    SyntheticReloadValueError, SyntheticReloadValuePlan, SyntheticReloadValuePolicy,
    SyntheticReloadValueReceipt, ValidatedAbstractSpillInsertion, ValidatedReloadValueHomes,
    ValidatedSyntheticReloadValues, synthetic_reload_value_plan_identity,
};

pub fn validate_synthetic_reload_values(
    insertion: &ValidatedAbstractSpillInsertion,
    homes: &ValidatedReloadValueHomes,
    plan: SyntheticReloadValuePlan,
) -> Result<ValidatedSyntheticReloadValues, SyntheticReloadValueError> {
    if plan.abstract_spill_insertion != insertion.receipt().identity()
        || plan.reload_value_homes != homes.receipt().identity()
    {
        return Err(SyntheticReloadValueError::RootMismatch);
    }
    if plan.policy != SyntheticReloadValuePolicy::ValidatedSingleSpillEpochZeroCanonicalOrderV1 {
        return Err(SyntheticReloadValueError::UnsupportedPolicy);
    }
    let expected = super::replay::replay(insertion, homes, plan.policy, plan.budget)?;
    if plan.usage != expected.usage {
        return Err(SyntheticReloadValueError::UsageMismatch);
    }
    if plan.functions.len() != expected.functions.len() {
        return Err(SyntheticReloadValueError::RootMismatch);
    }
    for (function, (actual, replayed)) in plan.functions.iter().zip(&expected.functions).enumerate()
    {
        if actual.machine != replayed.machine {
            return Err(SyntheticReloadValueError::FunctionMismatch { function });
        }
        if actual != replayed {
            return Err(SyntheticReloadValueError::NonCanonicalNamespace { function });
        }
    }
    if !plan.usage.within(plan.budget) {
        return Err(SyntheticReloadValueError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let binding_count = plan
        .functions
        .iter()
        .filter(|function| function.binding.is_some())
        .count();
    let receipt = SyntheticReloadValueReceipt {
        identity: synthetic_reload_value_plan_identity(&plan),
        abstract_spill_insertion: plan.abstract_spill_insertion,
        reload_value_homes: plan.reload_value_homes,
        usage: plan.usage,
        function_count: plan.functions.len(),
        binding_count,
    };
    Ok(ValidatedSyntheticReloadValues { plan, receipt })
}
