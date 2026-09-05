//! Direct canonical traversal for the epoch-two recovery worklist.

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FunctionGeneralizedSpillRecoveryWorklist, GeneralizedReloadValueHomeOutcome,
    GeneralizedSpillActionSource, GeneralizedSpillRecoveryWorkItem,
    GeneralizedSpillRecoveryWorkItemId, GeneralizedSpillRecoveryWorklistError,
    GeneralizedSpillRecoveryWorklistPlan, GeneralizedSpillRecoveryWorklistPolicy,
    ValidatedGeneralizedReloadValueHomes,
};

pub(super) fn compute(
    source: &ValidatedGeneralizedReloadValueHomes,
    policy: GeneralizedSpillRecoveryWorklistPolicy,
    budget: OptimizationWorkBudget,
) -> Result<GeneralizedSpillRecoveryWorklistPlan, GeneralizedSpillRecoveryWorklistError> {
    if policy != GeneralizedSpillRecoveryWorklistPolicy::EpochOnePressureToEpochTwoV1 {
        return Err(GeneralizedSpillRecoveryWorklistError::UnsupportedPolicy);
    }
    let mut outcome_count = 0_u64;
    let mut item_count = 0_u64;
    let mut candidate_count = 0_u64;
    let mut blocking_home_count = 0_u64;
    let mut functions = Vec::with_capacity(source.plan().functions.len());
    for (function, row) in source.plan().functions.iter().enumerate() {
        let mut item = None;
        for (index, outcome) in row.outcomes.iter().enumerate() {
            outcome_count = add(outcome_count, 1)?;
            match outcome {
                GeneralizedReloadValueHomeOutcome::Assigned(assignment) => {
                    if item.is_some() || assignment.result.epoch != 0 {
                        return Err(invalid(function));
                    }
                }
                GeneralizedReloadValueHomeOutcome::Pressure(pressure) => {
                    if item.is_some()
                        || index + 1 != row.outcomes.len()
                        || pressure.result.epoch != 1
                        || pressure.result.ordinal != 0
                        || !matches!(
                            pressure.source,
                            GeneralizedSpillActionSource::EpochOne { .. }
                        )
                        || pressure.candidates.is_empty()
                        || pressure.blocking_homes.is_empty()
                        || !strict(&pressure.candidates)
                        || !strict(&pressure.blocking_homes)
                    {
                        return Err(invalid(function));
                    }
                    let epoch =
                        pressure.result.epoch.checked_add(1).ok_or(
                            GeneralizedSpillRecoveryWorklistError::EpochOverflow { function },
                        )?;
                    candidate_count = add(candidate_count, count(pressure.candidates.len())?)?;
                    blocking_home_count =
                        add(blocking_home_count, count(pressure.blocking_homes.len())?)?;
                    item_count = add(item_count, 1)?;
                    item = Some(GeneralizedSpillRecoveryWorkItem {
                        id: GeneralizedSpillRecoveryWorkItemId { epoch, ordinal: 0 },
                        source_pressure: pressure.result,
                        source: pressure.source,
                        block: pressure.block,
                        start: pressure.start,
                        exclusive_end: pressure.exclusive_end,
                        class: pressure.class,
                        candidates: pressure.candidates.clone(),
                        blocking_homes: pressure.blocking_homes.clone(),
                    });
                }
            }
        }
        functions.push(FunctionGeneralizedSpillRecoveryWorklist {
            machine: row.machine,
            item,
        });
    }
    if item_count == 0 {
        return Err(GeneralizedSpillRecoveryWorklistError::PressureRequired);
    }
    let usage = usage(
        outcome_count,
        count(functions.len())?,
        item_count,
        candidate_count,
        blocking_home_count,
    )?;
    if !usage.within(budget) {
        return Err(GeneralizedSpillRecoveryWorklistError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let receipt = source.receipt();
    Ok(GeneralizedSpillRecoveryWorklistPlan {
        reload_value_homes: receipt.identity(),
        generalized_spill_insertion: receipt.generalized_spill_insertion(),
        abstract_spill_insertion: receipt.abstract_spill_insertion(),
        spill_recovery_actions: receipt.spill_recovery_actions(),
        selected: receipt.selected(),
        ranges: receipt.ranges(),
        legality: receipt.legality(),
        register_environment: receipt.register_environment(),
        allocator_availability: receipt.allocator_availability(),
        optimization_unit: receipt.optimization_unit(),
        fuel_schedule: receipt.fuel_schedule(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn usage(
    outcomes: u64,
    functions: u64,
    items: u64,
    candidates: u64,
    blocking_homes: u64,
) -> Result<OptimizationWorkUsage, GeneralizedSpillRecoveryWorklistError> {
    const FIXED_ITEM_FIELDS: u64 = 7;
    let fixed = items
        .checked_mul(FIXED_ITEM_FIELDS)
        .ok_or(GeneralizedSpillRecoveryWorklistError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: outcomes,
        candidates,
        validation_steps: add(add(add(fixed, outcomes)?, candidates)?, blocking_homes)?,
        commits: items,
        iterations: functions,
    })
}

fn strict<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn invalid(function: usize) -> GeneralizedSpillRecoveryWorklistError {
    GeneralizedSpillRecoveryWorklistError::InvalidSourceOutcomes { function }
}

fn add(left: u64, right: u64) -> Result<u64, GeneralizedSpillRecoveryWorklistError> {
    left.checked_add(right)
        .ok_or(GeneralizedSpillRecoveryWorklistError::WorkOverflow)
}

fn count(value: usize) -> Result<u64, GeneralizedSpillRecoveryWorklistError> {
    u64::try_from(value).map_err(|_| GeneralizedSpillRecoveryWorklistError::WorkOverflow)
}
