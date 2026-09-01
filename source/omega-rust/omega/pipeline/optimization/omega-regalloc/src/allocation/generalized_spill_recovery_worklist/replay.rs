//! Independently keyed reconstruction of epoch-two recovery work.

use std::collections::BTreeMap;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FunctionGeneralizedSpillRecoveryWorklist, GeneralizedReloadValueHomeOutcome,
    GeneralizedSpillActionId, GeneralizedSpillActionSource, GeneralizedSpillRecoveryWorkItem,
    GeneralizedSpillRecoveryWorkItemId, GeneralizedSpillRecoveryWorklistError,
    GeneralizedSpillRecoveryWorklistPlan, GeneralizedSpillRecoveryWorklistPolicy,
    ValidatedGeneralizedReloadValueHomes,
};

pub(super) fn replay(
    source: &ValidatedGeneralizedReloadValueHomes,
    policy: GeneralizedSpillRecoveryWorklistPolicy,
    budget: OptimizationWorkBudget,
) -> Result<GeneralizedSpillRecoveryWorklistPlan, GeneralizedSpillRecoveryWorklistError> {
    if !matches!(
        policy,
        GeneralizedSpillRecoveryWorklistPolicy::EpochOnePressureToEpochTwoV1
    ) {
        return Err(GeneralizedSpillRecoveryWorklistError::UnsupportedPolicy);
    }
    let mut indexed = BTreeMap::<(usize, GeneralizedSpillActionId), _>::new();
    let mut outcome_count = 0_u64;
    for (function, row) in source.plan().functions.iter().enumerate() {
        for outcome in &row.outcomes {
            outcome_count = checked(outcome_count, 1)?;
            let action = match outcome {
                GeneralizedReloadValueHomeOutcome::Assigned(row) => row.result,
                GeneralizedReloadValueHomeOutcome::Pressure(row) => row.result,
            };
            if indexed.insert((function, action), outcome).is_some() {
                return Err(invalid(function));
            }
        }
    }

    let mut functions = Vec::with_capacity(source.plan().functions.len());
    let mut item_count = 0_u64;
    let mut candidate_count = 0_u64;
    let mut blocking_home_count = 0_u64;
    for (function, source_function) in source.plan().functions.iter().enumerate() {
        let rows = indexed
            .range((function, first_action())..=(function, last_action()))
            .map(|(_, outcome)| *outcome)
            .collect::<Vec<_>>();
        let pressure = reconstruct_pressure(function, &rows)?;
        let item = pressure.map(|pressure| {
            item_count = checked(item_count, 1)?;
            candidate_count = checked(candidate_count, to_u64(pressure.candidates.len())?)?;
            blocking_home_count =
                checked(blocking_home_count, to_u64(pressure.blocking_homes.len())?)?;
            let epoch = pressure
                .result
                .epoch
                .checked_add(1)
                .ok_or(GeneralizedSpillRecoveryWorklistError::EpochOverflow { function })?;
            Ok(GeneralizedSpillRecoveryWorkItem {
                id: GeneralizedSpillRecoveryWorkItemId { epoch, ordinal: 0 },
                source_pressure: pressure.result,
                source: pressure.source,
                block: pressure.block,
                start: pressure.start,
                exclusive_end: pressure.exclusive_end,
                class: pressure.class,
                candidates: pressure.candidates.clone(),
                blocking_homes: pressure.blocking_homes.clone(),
            })
        });
        functions.push(FunctionGeneralizedSpillRecoveryWorklist {
            machine: source_function.machine,
            item: item.transpose()?,
        });
    }
    if item_count == 0 {
        return Err(GeneralizedSpillRecoveryWorklistError::PressureRequired);
    }
    let function_count = to_u64(functions.len())?;
    let mut validation_steps = 0_u64;
    for _ in 0..outcome_count {
        validation_steps = checked(validation_steps, 1)?;
    }
    for _ in 0..candidate_count {
        validation_steps = checked(validation_steps, 1)?;
    }
    for _ in 0..blocking_home_count {
        validation_steps = checked(validation_steps, 1)?;
    }
    for _ in 0..item_count {
        validation_steps = checked(validation_steps, 7)?;
    }
    let usage = OptimizationWorkUsage {
        rule_evaluations: outcome_count,
        candidates: candidate_count,
        validation_steps,
        commits: item_count,
        iterations: function_count,
    };
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

fn reconstruct_pressure<'a>(
    function: usize,
    rows: &[&'a GeneralizedReloadValueHomeOutcome],
) -> Result<Option<&'a crate::GeneralizedReloadValuePressure>, GeneralizedSpillRecoveryWorklistError>
{
    let mut pressure = None;
    for (index, outcome) in rows.iter().enumerate() {
        match outcome {
            GeneralizedReloadValueHomeOutcome::Assigned(row)
                if pressure.is_none() && row.result.epoch == 0 => {}
            GeneralizedReloadValueHomeOutcome::Pressure(row)
                if pressure.is_none()
                    && index + 1 == rows.len()
                    && row.result.epoch == 1
                    && row.result.ordinal == 0
                    && matches!(row.source, GeneralizedSpillActionSource::EpochOne { .. })
                    && !row.candidates.is_empty()
                    && !row.blocking_homes.is_empty()
                    && ordered(&row.candidates)
                    && ordered(&row.blocking_homes) =>
            {
                pressure = Some(row);
            }
            _ => return Err(invalid(function)),
        }
    }
    Ok(pressure)
}

const fn first_action() -> GeneralizedSpillActionId {
    GeneralizedSpillActionId {
        epoch: 0,
        ordinal: 0,
    }
}

const fn last_action() -> GeneralizedSpillActionId {
    GeneralizedSpillActionId {
        epoch: u32::MAX,
        ordinal: u32::MAX,
    }
}

fn ordered<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn invalid(function: usize) -> GeneralizedSpillRecoveryWorklistError {
    GeneralizedSpillRecoveryWorklistError::InvalidSourceOutcomes { function }
}

fn checked(left: u64, right: u64) -> Result<u64, GeneralizedSpillRecoveryWorklistError> {
    left.checked_add(right)
        .ok_or(GeneralizedSpillRecoveryWorklistError::WorkOverflow)
}

fn to_u64(value: usize) -> Result<u64, GeneralizedSpillRecoveryWorklistError> {
    u64::try_from(value).map_err(|_| GeneralizedSpillRecoveryWorklistError::WorkOverflow)
}
