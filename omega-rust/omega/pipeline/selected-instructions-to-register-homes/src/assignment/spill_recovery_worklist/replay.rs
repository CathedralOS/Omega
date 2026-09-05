//! Independent reconstruction using a point-indexed legality table.

use std::collections::BTreeMap;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    RegisterViewId, TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};

use crate::{
    LiveRangePoint, ReloadValueHomeError, ReloadValueHomePolicy, SpillRecoveryEpoch,
    SpillRecoveryWorkItem, SpillRecoveryWorklistError, SpillRecoveryWorklistPlan,
    SpillRecoveryWorklistPolicy, SyntheticReloadValueId, ValidatedAbstractSpillInsertion,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLogicalSpillOperations,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn replay(
    insertion: &ValidatedAbstractSpillInsertion,
    logical: &ValidatedLogicalSpillOperations,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    reload_home_policy: ReloadValueHomePolicy,
    reload_home_budget: OptimizationWorkBudget,
    policy: SpillRecoveryWorklistPolicy,
    budget: OptimizationWorkBudget,
) -> Result<SpillRecoveryWorklistPlan, SpillRecoveryWorklistError> {
    admit_replay_policy(reload_home_policy, policy)?;
    let trigger = match crate::assignment::reload_value_homes::replay::replay(
        insertion,
        logical,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
        reload_home_policy,
        reload_home_budget,
    ) {
        Err(ReloadValueHomeError::ReloadPressure { function, result }) => (function, result),
        Err(error) => return Err(SpillRecoveryWorklistError::SourceReloadHome(error)),
        Ok(_) => return Err(SpillRecoveryWorklistError::ReloadPressureRequired),
    };
    let item = reconstruct_item(insertion, legality, trigger.0, trigger.1)?;
    let usage = reconstruct_usage(&item)?;
    if !usage.within(budget) {
        return Err(SpillRecoveryWorklistError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let insertion_receipt = insertion.receipt();
    let logical_receipt = logical.receipt();
    Ok(SpillRecoveryWorklistPlan {
        abstract_spill_insertion: insertion_receipt.identity(),
        logical_spill_operations: logical_receipt.identity(),
        legality: legality.receipt().identity(),
        ranges: ranges.receipt().identity(),
        register_environment: logical_receipt.register_environment(),
        allocator_availability: logical_receipt.allocator_availability(),
        optimization_unit: insertion_receipt.optimization_unit(),
        fuel_schedule: insertion_receipt.fuel_schedule(),
        reload_home_policy,
        reload_home_budget,
        policy,
        budget,
        usage,
        epochs: vec![SpillRecoveryEpoch {
            epoch: 1,
            work_items: vec![item],
        }],
    })
}

fn admit_replay_policy(
    reload_home_policy: ReloadValueHomePolicy,
    policy: SpillRecoveryWorklistPolicy,
) -> Result<(), SpillRecoveryWorklistError> {
    if reload_home_policy
        != ReloadValueHomePolicy::BlockLocalSingleSpillReloadFirstLowestCompatibleViewV1
        || policy != SpillRecoveryWorklistPolicy::SingleReloadPressureEpochOneV1
    {
        return Err(SpillRecoveryWorklistError::UnsupportedPolicy);
    }
    Ok(())
}

fn reconstruct_usage(
    item: &SpillRecoveryWorkItem,
) -> Result<OptimizationWorkUsage, SpillRecoveryWorklistError> {
    const FIXED_ITEM_FIELDS: u64 = 8;
    let mut candidates = 0_u64;
    for _ in &item.candidates {
        candidates = candidates
            .checked_add(1)
            .ok_or(SpillRecoveryWorklistError::WorkOverflow)?;
    }
    let mut duration = 0_u64;
    for _ in item.start.0..item.exclusive_end.0 {
        duration = duration
            .checked_add(1)
            .ok_or(SpillRecoveryWorklistError::WorkOverflow)?;
    }
    let validation_steps = FIXED_ITEM_FIELDS
        .checked_add(duration)
        .and_then(|total| total.checked_add(candidates))
        .ok_or(SpillRecoveryWorklistError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates,
        validation_steps,
        commits: 1,
        iterations: 1,
    })
}

fn reconstruct_item(
    insertion: &ValidatedAbstractSpillInsertion,
    legality: &ValidatedAllocationLegality,
    function: usize,
    result: u32,
) -> Result<SpillRecoveryWorkItem, SpillRecoveryWorklistError> {
    let inserted = insertion
        .plan()
        .functions
        .get(function)
        .ok_or(SpillRecoveryWorklistError::TriggerMismatch { function })?;
    let action = inserted
        .action
        .as_ref()
        .filter(|action| action.reload.result.0 == result)
        .ok_or(SpillRecoveryWorklistError::TriggerMismatch { function })?;
    let first = action
        .rewrites
        .first()
        .ok_or(SpillRecoveryWorklistError::TriggerMismatch { function })?;
    let last = action
        .rewrites
        .last()
        .ok_or(SpillRecoveryWorklistError::TriggerMismatch { function })?;
    let exclusive_end = LiveRangePoint(
        last.point
            .0
            .checked_add(1)
            .ok_or(SpillRecoveryWorklistError::IntervalOverflow { function })?,
    );
    let legality_function = legality
        .plan()
        .functions
        .get(function)
        .filter(|candidate| candidate.machine == inserted.machine)
        .ok_or(SpillRecoveryWorklistError::TriggerMismatch { function })?;
    let victim = legality_function
        .virtual_registers
        .iter()
        .find(|candidate| candidate.virtual_register == action.victim)
        .filter(|candidate| candidate.class == action.reload.destination_class)
        .ok_or(SpillRecoveryWorklistError::TriggerMismatch { function })?;
    let point_rows = victim
        .points
        .iter()
        .map(|row| ((row.block, row.point), row.candidates.as_slice()))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = None::<Vec<RegisterViewId>>;
    for raw in first.point.0..exclusive_end.0 {
        let views = point_rows
            .get(&(first.block, LiveRangePoint(raw)))
            .ok_or(SpillRecoveryWorklistError::InvalidCandidateDomain { function })?;
        match &mut candidates {
            None => candidates = Some(views.to_vec()),
            Some(shared) => shared.retain(|view| views.binary_search(view).is_ok()),
        }
    }
    let candidates = candidates
        .filter(|candidates| !candidates.is_empty())
        .ok_or(SpillRecoveryWorklistError::InvalidCandidateDomain { function })?;
    Ok(SpillRecoveryWorkItem {
        synthetic: SyntheticReloadValueId {
            epoch: 1,
            ordinal: 0,
        },
        machine: inserted.machine,
        source_reload: action.reload.result,
        block: first.block,
        start: first.point,
        exclusive_end,
        class: action.reload.destination_class,
        candidates,
    })
}
