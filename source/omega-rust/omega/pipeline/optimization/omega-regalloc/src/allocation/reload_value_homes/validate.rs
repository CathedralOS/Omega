//! Independent root admission, allocation replay, comparison, and receipt sealing.

use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};

use crate::{
    ReloadValueHomeError, ReloadValueHomePlan, ReloadValueHomePolicy, ReloadValueHomeReceipt,
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedLogicalSpillOperations, ValidatedReloadValueHomes, reload_value_home_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_reload_value_homes(
    insertion: &ValidatedAbstractSpillInsertion,
    logical: &ValidatedLogicalSpillOperations,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: ReloadValueHomePlan,
) -> Result<ValidatedReloadValueHomes, ReloadValueHomeError> {
    let logical_receipt = logical.receipt();
    if plan.abstract_spill_insertion != insertion.receipt().identity()
        || plan.logical_spill_operations != logical_receipt.identity()
        || plan.legality != legality.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.register_environment != logical_receipt.register_environment()
        || plan.allocator_availability != logical_receipt.allocator_availability()
    {
        return Err(ReloadValueHomeError::RootMismatch);
    }
    if plan.policy != ReloadValueHomePolicy::BlockLocalSingleSpillReloadFirstLowestCompatibleViewV1
    {
        return Err(ReloadValueHomeError::UnsupportedPolicy);
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
        plan.policy,
        plan.budget,
    )?;
    if plan.usage != expected.usage {
        return Err(ReloadValueHomeError::UsageMismatch);
    }
    if plan.functions.len() != expected.functions.len() {
        return Err(ReloadValueHomeError::RootMismatch);
    }
    for (function, (actual, expected)) in plan.functions.iter().zip(&expected.functions).enumerate()
    {
        if actual.machine != expected.machine {
            return Err(ReloadValueHomeError::FunctionMismatch { function });
        }
        if actual != expected {
            return Err(ReloadValueHomeError::NonCanonicalAssignment { function });
        }
    }
    if !plan.usage.within(plan.budget) {
        return Err(ReloadValueHomeError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let assignment_count = plan
        .functions
        .iter()
        .filter(|function| function.assignment.is_some())
        .count();
    let coexisting_home_count = plan
        .functions
        .iter()
        .filter_map(|function| function.assignment.as_ref())
        .try_fold(0_usize, |total, assignment| {
            total
                .checked_add(assignment.coexisting_homes.len())
                .ok_or(ReloadValueHomeError::WorkOverflow)
        })?;
    let receipt = ReloadValueHomeReceipt {
        identity: reload_value_home_identity(&plan),
        abstract_spill_insertion: plan.abstract_spill_insertion,
        logical_spill_operations: plan.logical_spill_operations,
        legality: plan.legality,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        usage: plan.usage,
        function_count: plan.functions.len(),
        assignment_count,
        coexisting_home_count,
    };
    Ok(ValidatedReloadValueHomes { plan, receipt })
}
