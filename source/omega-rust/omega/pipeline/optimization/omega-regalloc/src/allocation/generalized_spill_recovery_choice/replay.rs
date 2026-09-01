//! Independently keyed reconstruction of epoch-two victim choices.

use std::collections::BTreeMap;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterViewId, TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};

use crate::{
    GeneralizedReloadCoexistingValue, GeneralizedReloadValueHomeOutcome,
    GeneralizedSpillRecoveryChoiceError, GeneralizedSpillRecoveryChoicePlan,
    GeneralizedSpillRecoveryChoicePolicy, GeneralizedSpillRecoveryContender,
    GeneralizedSpillRecoveryResident, GeneralizedSpillRecoveryVictimChoice, LiveRangePoint,
    ValidatedAllocationLegality, ValidatedGeneralizedReloadValueHomes,
    ValidatedGeneralizedSpillRecoveryWorklist, ValidatedLiveRanges, ValidatedSelectedAnalysis,
};

mod original_eligibility;

#[allow(clippy::too_many_arguments)]
pub(super) fn replay<S: ValidatedSelectedAnalysis>(
    worklist: &ValidatedGeneralizedSpillRecoveryWorklist,
    homes: &ValidatedGeneralizedReloadValueHomes,
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: GeneralizedSpillRecoveryChoicePolicy,
    budget: OptimizationWorkBudget,
) -> Result<GeneralizedSpillRecoveryChoicePlan, GeneralizedSpillRecoveryChoiceError> {
    replay_roots(
        worklist,
        homes,
        selected,
        ranges,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    match policy {
        GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1
        | GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1 => {}
    }
    let mut work_items = BTreeMap::new();
    for (function, row) in worklist.plan().functions.iter().enumerate() {
        if let Some(item) = &row.item {
            if work_items
                .insert((function, item.id), (row.machine, item))
                .is_some()
            {
                return Err(GeneralizedSpillRecoveryChoiceError::MissingPressure { function });
            }
        }
    }
    let mut assignments = BTreeMap::new();
    let mut pressures = BTreeMap::new();
    for (function, row) in homes.plan().functions.iter().enumerate() {
        for outcome in &row.outcomes {
            match outcome {
                GeneralizedReloadValueHomeOutcome::Assigned(assigned) => {
                    if assignments
                        .insert((function, assigned.result), assigned)
                        .is_some()
                    {
                        return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker {
                            function,
                        });
                    }
                }
                GeneralizedReloadValueHomeOutcome::Pressure(pressure) => {
                    if pressures
                        .insert((function, pressure.result), pressure)
                        .is_some()
                    {
                        return Err(GeneralizedSpillRecoveryChoiceError::MissingPressure {
                            function,
                        });
                    }
                }
            }
        }
    }
    let mut originals = BTreeMap::new();
    for (function, row) in legality.plan().functions.iter().enumerate() {
        for original in &row.virtual_registers {
            if originals
                .insert((function, original.virtual_register), original)
                .is_some()
            {
                return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function });
            }
        }
    }

    let mut choices = Vec::new();
    let mut rules = 0_u64;
    let mut candidates = 0_u64;
    let mut steps = 0_u64;
    let mut commits = 0_u64;
    for ((function, _), (machine, item)) in work_items {
        let pressure = pressures
            .get(&(function, item.source_pressure))
            .copied()
            .ok_or(GeneralizedSpillRecoveryChoiceError::MissingPressure { function })?;
        let selected_row = selected
            .selected_plan()
            .functions
            .get(function)
            .filter(|row| row.machine == machine)
            .ok_or(GeneralizedSpillRecoveryChoiceError::FunctionMismatch { function })?;
        let ranges_row = ranges
            .plan()
            .functions
            .get(function)
            .filter(|row| row.machine == machine)
            .ok_or(GeneralizedSpillRecoveryChoiceError::FunctionMismatch { function })?;
        if pressure.source != item.source
            || pressure.block != item.block
            || pressure.start != item.start
            || pressure.exclusive_end != item.exclusive_end
            || pressure.class != item.class
            || pressure.candidates != item.candidates
            || pressure.blocking_homes != item.blocking_homes
            || homes.plan().functions.get(function).map(|row| row.machine) != Some(machine)
            || legality
                .plan()
                .functions
                .get(function)
                .map(|row| row.machine)
                != Some(machine)
        {
            return Err(GeneralizedSpillRecoveryChoiceError::FunctionMismatch { function });
        }
        let mut resident_map = BTreeMap::new();
        for blocker in &item.blocking_homes {
            rules = checked(rules, 1)?;
            steps = checked(steps, 1)?;
            replay_view(function, blocker.class, blocker.view, physical)?;
            let (start, exclusive_end) = match blocker.value {
                GeneralizedReloadCoexistingValue::Original(register) => {
                    let row = originals
                        .get(&(function, register))
                        .copied()
                        .filter(|row| row.class == blocker.class)
                        .ok_or(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function })?;
                    let first = row
                        .points
                        .first()
                        .ok_or(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function })?;
                    let last = row.points.last().expect("nonempty legality row");
                    let end = LiveRangePoint(last.point.0.checked_add(1).ok_or(
                        GeneralizedSpillRecoveryChoiceError::IntervalOverflow { function },
                    )?);
                    if row
                        .points
                        .binary_search_by_key(&(item.block, item.start), |point| {
                            (point.block, point.point)
                        })
                        .ok()
                        .and_then(|index| row.points.get(index))
                        .is_none_or(|point| point.candidates.binary_search(&blocker.view).is_err())
                    {
                        return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker {
                            function,
                        });
                    }
                    (first.point, end)
                }
                GeneralizedReloadCoexistingValue::Reload(action) => {
                    let assigned = assignments
                        .get(&(function, action))
                        .copied()
                        .filter(|row| {
                            row.block == item.block
                                && row.class == blocker.class
                                && row.view == blocker.view
                        })
                        .ok_or(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function })?;
                    (assigned.start, assigned.exclusive_end)
                }
            };
            if !(start <= item.start && item.start < exclusive_end)
                || resident_map
                    .insert(
                        blocker.value,
                        GeneralizedSpillRecoveryResident {
                            value: blocker.value,
                            class: blocker.class,
                            start,
                            exclusive_end,
                            view: blocker.view,
                        },
                    )
                    .is_some()
            {
                return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function });
            }
        }
        let residents = resident_map.into_values().collect::<Vec<_>>();
        if residents.len() != item.blocking_homes.len() {
            return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function });
        }
        for candidate in &item.candidates {
            replay_view(function, item.class, *candidate, physical)?;
            if residents
                .iter()
                .all(|resident| !replay_overlap(*candidate, resident.view, physical))
            {
                return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function });
            }
        }
        let mut contenders = Vec::new();
        for resident in &residents {
            let mut reclaimed = None;
            for candidate in &item.candidates {
                steps = checked(steps, 1)?;
                let still_blocked = residents.iter().any(|other| {
                    other.value != resident.value
                        && replay_overlap(*candidate, other.view, physical)
                });
                if !still_blocked {
                    reclaimed = Some(*candidate);
                    break;
                }
            }
            if let Some(reclaimed_view) = reclaimed {
                contenders.push(GeneralizedSpillRecoveryContender {
                    value: resident.value,
                    exclusive_end: resident.exclusive_end,
                    resident_view: resident.view,
                    reclaimed_view,
                });
            }
        }
        contenders.sort_by_key(|contender| contender.value);
        candidates = checked(candidates, to_u64(contenders.len())?)?;
        let mut ranked = BTreeMap::new();
        for contender in &contenders {
            let admitted = match policy {
                GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1 => {
                    true
                }
                GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1 => {
                    rules = checked(rules, 1)?;
                    steps = checked(steps, 1)?;
                    match contender.value {
                        GeneralizedReloadCoexistingValue::Reload(_) => true,
                        GeneralizedReloadCoexistingValue::Original(register) => {
                            let resident = residents
                                .iter()
                                .find(|resident| resident.value == contender.value)
                                .copied()
                                .ok_or(GeneralizedSpillRecoveryChoiceError::InvalidBlocker {
                                    function,
                                })?;
                            original_eligibility::replay(
                                register,
                                item.block,
                                item.start,
                                resident,
                                selected_row,
                                ranges_row,
                                &mut steps,
                            )?
                        }
                    }
                }
            };
            if admitted {
                let original_priority = match policy {
                    GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1 => {
                        0
                    }
                    GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1 => {
                        u8::from(matches!(
                            contender.value,
                            GeneralizedReloadCoexistingValue::Original(_)
                        ))
                    }
                };
                ranked.insert(
                    (original_priority, contender.exclusive_end, contender.value),
                    *contender,
                );
            }
        }
        let selected = ranked
            .last_key_value()
            .map(|(_, contender)| *contender)
            .ok_or(GeneralizedSpillRecoveryChoiceError::NoRecoverableVictim { function })?;
        commits = checked(commits, 1)?;
        steps = checked(steps, 8)?;
        choices.push(GeneralizedSpillRecoveryVictimChoice {
            work_item: item.id,
            function,
            machine,
            block: item.block,
            point: item.start,
            source_pressure: item.source_pressure,
            reload_class: item.class,
            reload_candidates: item.candidates.clone(),
            blocking_residents: residents,
            contenders,
            selected_victim: selected.value,
            selected_victim_view: selected.resident_view,
            reclaimed_view: selected.reclaimed_view,
        });
    }
    let usage = OptimizationWorkUsage {
        rule_evaluations: rules,
        candidates,
        validation_steps: steps,
        commits,
        iterations: to_u64(worklist.plan().functions.len())?,
    };
    if !usage.within(budget) {
        return Err(GeneralizedSpillRecoveryChoiceError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let receipt = homes.receipt();
    Ok(GeneralizedSpillRecoveryChoicePlan {
        worklist: worklist.receipt().identity(),
        reload_value_homes: receipt.identity(),
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
        choices,
    })
}

#[allow(clippy::too_many_arguments)]
fn replay_roots(
    worklist: &ValidatedGeneralizedSpillRecoveryWorklist,
    homes: &ValidatedGeneralizedReloadValueHomes,
    selected: &impl ValidatedSelectedAnalysis,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), GeneralizedSpillRecoveryChoiceError> {
    let source = worklist.plan();
    let home = homes.receipt();
    let legal = legality.receipt();
    let range = ranges.receipt();
    let environment = target_register_environment_identity(
        reservations.target(),
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    if source.reload_value_homes != home.identity()
        || source.legality != legal.identity()
        || home.selected() != selected.selected_identity()
        || home.ranges() != range.identity()
        || range.selected() != selected.selected_identity()
        || legal.ranges() != range.identity()
        || home.legality() != legal.identity()
        || source.register_environment != environment
        || home.register_environment() != environment
        || legal.register_environment() != environment
        || source.allocator_availability != legal.allocator_availability()
        || home.allocator_availability() != legal.allocator_availability()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
    {
        return Err(GeneralizedSpillRecoveryChoiceError::RootMismatch);
    }
    Ok(())
}

fn replay_view(
    function: usize,
    class: omega_register_model::RegisterClassId,
    id: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), GeneralizedSpillRecoveryChoiceError> {
    physical
        .model()
        .views
        .iter()
        .any(|view| view.id == id && view.class == class)
        .then_some(())
        .ok_or(GeneralizedSpillRecoveryChoiceError::InvalidView {
            function,
            view: id.0,
        })
}

fn replay_overlap(
    left: RegisterViewId,
    right: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    let left = physical.model().views.iter().find(|view| view.id == left);
    let right = physical.model().views.iter().find(|view| view.id == right);
    match (left, right) {
        (Some(left), Some(right)) => {
            left.units
                .iter()
                .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
                || left
                    .write_units
                    .iter()
                    .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
        }
        _ => true,
    }
}

fn checked(left: u64, right: u64) -> Result<u64, GeneralizedSpillRecoveryChoiceError> {
    left.checked_add(right)
        .ok_or(GeneralizedSpillRecoveryChoiceError::WorkOverflow)
}

fn to_u64(value: usize) -> Result<u64, GeneralizedSpillRecoveryChoiceError> {
    u64::try_from(value).map_err(|_| GeneralizedSpillRecoveryChoiceError::WorkOverflow)
}
