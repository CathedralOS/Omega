//! Canonical producer for the single admitted recursive-recovery work item.

use std::collections::BTreeSet;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};

use crate::{
    LiveRangePoint, ReloadValueHomeError, ReloadValueHomePolicy, SpillRecoveryEpoch,
    SpillRecoveryWorkItem, SpillRecoveryWorklistError, SpillRecoveryWorklistPlan,
    SpillRecoveryWorklistPolicy, SyntheticReloadValueId, ValidatedAbstractSpillInsertion,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLogicalSpillOperations,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute(
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
    admit_producer_policy(reload_home_policy, policy)?;
    let trigger = match crate::assignment::reload_value_homes::compute::compute(
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
    let item = build_item(insertion, legality, trigger.0, trigger.1)?;
    let usage = work_usage(&item)?;
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

fn admit_producer_policy(
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

fn build_item(
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
    let mut shared = None::<BTreeSet<_>>;
    for raw in first.point.0..exclusive_end.0 {
        let point = LiveRangePoint(raw);
        let row = victim
            .points
            .iter()
            .find(|row| row.block == first.block && row.point == point)
            .ok_or(SpillRecoveryWorklistError::InvalidCandidateDomain { function })?;
        let row = row.candidates.iter().copied().collect::<BTreeSet<_>>();
        match &mut shared {
            None => shared = Some(row),
            Some(shared) => shared.retain(|candidate| row.contains(candidate)),
        }
    }
    let candidates = shared
        .filter(|candidates| !candidates.is_empty())
        .ok_or(SpillRecoveryWorklistError::InvalidCandidateDomain { function })?
        .into_iter()
        .collect();
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

pub(super) fn work_usage(
    item: &SpillRecoveryWorkItem,
) -> Result<OptimizationWorkUsage, SpillRecoveryWorklistError> {
    // Namespace pair plus the six retained source fields are fixed-cost checks;
    // every interval point and every candidate is then accounted explicitly.
    const FIXED_ITEM_FIELDS: u64 = 8;
    let candidates = u64::try_from(item.candidates.len())
        .map_err(|_| SpillRecoveryWorklistError::WorkOverflow)?;
    let duration = u64::from(
        item.exclusive_end
            .0
            .checked_sub(item.start.0)
            .ok_or(SpillRecoveryWorklistError::NonCanonicalWorklist)?,
    );
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
