//! Direct blocker-roster traversal and epoch-two victim proposal.

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use omega_selected_instructions::SelectedFunction;

use crate::{
    GeneralizedReloadCoexistingHome, GeneralizedReloadCoexistingValue,
    GeneralizedReloadValueHomeOutcome, GeneralizedSpillRecoveryChoiceError,
    GeneralizedSpillRecoveryChoicePlan, GeneralizedSpillRecoveryChoicePolicy,
    GeneralizedSpillRecoveryContender, GeneralizedSpillRecoveryResident,
    GeneralizedSpillRecoveryVictimChoice, LiveRangePoint, ValidatedAllocationLegality,
    ValidatedGeneralizedReloadValueHomes, ValidatedGeneralizedSpillRecoveryWorklist,
    ValidatedLiveRanges, ValidatedSelectedAnalysis,
};

mod original_eligibility;

#[allow(clippy::too_many_arguments)]
pub(super) fn compute<S: ValidatedSelectedAnalysis>(
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
    admit_roots(
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
    admit_policy(policy)?;
    let mut work = Work::default();
    let mut choices = Vec::new();
    for (function, row) in worklist.plan().functions.iter().enumerate() {
        let Some(item) = &row.item else { continue };
        let home_row = homes
            .plan()
            .functions
            .get(function)
            .filter(|candidate| candidate.machine == row.machine)
            .ok_or(GeneralizedSpillRecoveryChoiceError::FunctionMismatch { function })?;
        let legality_row = legality
            .plan()
            .functions
            .get(function)
            .filter(|candidate| candidate.machine == row.machine)
            .ok_or(GeneralizedSpillRecoveryChoiceError::FunctionMismatch { function })?;
        let selected_row = selected
            .selected_plan()
            .functions
            .get(function)
            .filter(|candidate| candidate.machine == row.machine)
            .ok_or(GeneralizedSpillRecoveryChoiceError::FunctionMismatch { function })?;
        let ranges_row = ranges
            .plan()
            .functions
            .get(function)
            .filter(|candidate| candidate.machine == row.machine)
            .ok_or(GeneralizedSpillRecoveryChoiceError::FunctionMismatch { function })?;
        let pressure = home_row
            .outcomes
            .iter()
            .find_map(|outcome| match outcome {
                GeneralizedReloadValueHomeOutcome::Pressure(pressure)
                    if pressure.result == item.source_pressure =>
                {
                    Some(pressure)
                }
                _ => None,
            })
            .ok_or(GeneralizedSpillRecoveryChoiceError::MissingPressure { function })?;
        if pressure.source != item.source
            || pressure.block != item.block
            || pressure.start != item.start
            || pressure.exclusive_end != item.exclusive_end
            || pressure.class != item.class
            || pressure.candidates != item.candidates
            || pressure.blocking_homes != item.blocking_homes
        {
            return Err(GeneralizedSpillRecoveryChoiceError::MissingPressure { function });
        }

        let mut residents = Vec::with_capacity(item.blocking_homes.len());
        for blocker in &item.blocking_homes {
            add(&mut work.rules, 1)?;
            add(&mut work.steps, 1)?;
            residents.push(resolve_resident(
                function,
                blocker,
                item.block,
                item.start,
                home_row,
                legality_row,
                physical,
            )?);
        }
        residents.sort_by_key(|resident| resident.value);
        if residents
            .iter()
            .map(|resident| GeneralizedReloadCoexistingHome {
                value: resident.value,
                class: resident.class,
                view: resident.view,
            })
            .ne(item.blocking_homes.iter().copied())
        {
            return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function });
        }
        let contenders = contenders(function, item, &residents, physical, &mut work)?;
        let selected = select(
            function,
            policy,
            item.block,
            item.start,
            &contenders,
            &residents,
            selected_row,
            ranges_row,
            &mut work,
        )?
        .ok_or(GeneralizedSpillRecoveryChoiceError::NoRecoverableVictim { function })?;
        add(&mut work.commits, 1)?;
        add(&mut work.steps, 8)?;
        choices.push(GeneralizedSpillRecoveryVictimChoice {
            work_item: item.id,
            function,
            machine: row.machine,
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
    let usage = work.usage(count(worklist.plan().functions.len())?);
    if !usage.within(budget) {
        return Err(GeneralizedSpillRecoveryChoiceError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let worklist_receipt = worklist.receipt();
    let homes_receipt = homes.receipt();
    Ok(GeneralizedSpillRecoveryChoicePlan {
        worklist: worklist_receipt.identity(),
        reload_value_homes: homes_receipt.identity(),
        selected: homes_receipt.selected(),
        ranges: homes_receipt.ranges(),
        legality: homes_receipt.legality(),
        register_environment: homes_receipt.register_environment(),
        allocator_availability: homes_receipt.allocator_availability(),
        optimization_unit: homes_receipt.optimization_unit(),
        fuel_schedule: homes_receipt.fuel_schedule(),
        policy,
        budget,
        usage,
        choices,
    })
}

fn resolve_resident(
    function: usize,
    blocker: &GeneralizedReloadCoexistingHome,
    block: omega_selected_instructions::SelectedBlockId,
    point: LiveRangePoint,
    homes: &crate::FunctionGeneralizedReloadValueHomes,
    legality: &crate::FunctionAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<GeneralizedSpillRecoveryResident, GeneralizedSpillRecoveryChoiceError> {
    checked_view(function, blocker.class, blocker.view, physical)?;
    let (start, exclusive_end) = match blocker.value {
        GeneralizedReloadCoexistingValue::Original(register) => {
            let row = legality
                .virtual_registers
                .iter()
                .find(|row| row.virtual_register == register && row.class == blocker.class)
                .ok_or(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function })?;
            let first = row
                .points
                .first()
                .ok_or(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function })?;
            let last = row.points.last().expect("nonempty legality row");
            let end = LiveRangePoint(
                last.point
                    .0
                    .checked_add(1)
                    .ok_or(GeneralizedSpillRecoveryChoiceError::IntervalOverflow { function })?,
            );
            if !row.points.iter().any(|candidate| {
                candidate.block == block
                    && candidate.point == point
                    && candidate.candidates.binary_search(&blocker.view).is_ok()
            }) {
                return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function });
            }
            (first.point, end)
        }
        GeneralizedReloadCoexistingValue::Reload(action) => {
            let assigned = homes
                .outcomes
                .iter()
                .find_map(|outcome| match outcome {
                    GeneralizedReloadValueHomeOutcome::Assigned(row) if row.result == action => {
                        Some(row)
                    }
                    _ => None,
                })
                .filter(|row| {
                    row.block == block
                        && row.class == blocker.class
                        && row.view == blocker.view
                        && row.start <= point
                        && point < row.exclusive_end
                })
                .ok_or(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function })?;
            (assigned.start, assigned.exclusive_end)
        }
    };
    if !(start <= point && point < exclusive_end) {
        return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function });
    }
    Ok(GeneralizedSpillRecoveryResident {
        value: blocker.value,
        class: blocker.class,
        start,
        exclusive_end,
        view: blocker.view,
    })
}

fn contenders(
    function: usize,
    item: &crate::GeneralizedSpillRecoveryWorkItem,
    residents: &[GeneralizedSpillRecoveryResident],
    physical: &ValidatedPhysicalRegisterModel,
    work: &mut Work,
) -> Result<Vec<GeneralizedSpillRecoveryContender>, GeneralizedSpillRecoveryChoiceError> {
    for candidate in &item.candidates {
        checked_view(function, item.class, *candidate, physical)?;
        if !residents
            .iter()
            .any(|resident| overlap(*candidate, resident.view, physical))
        {
            return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker { function });
        }
    }
    let mut contenders = Vec::new();
    for omitted in residents {
        let mut reclaimed = None;
        for candidate in &item.candidates {
            add(&mut work.steps, 1)?;
            if residents.iter().all(|resident| {
                resident.value == omitted.value || !overlap(*candidate, resident.view, physical)
            }) {
                reclaimed = Some(*candidate);
                break;
            }
        }
        if let Some(reclaimed_view) = reclaimed {
            contenders.push(GeneralizedSpillRecoveryContender {
                value: omitted.value,
                exclusive_end: omitted.exclusive_end,
                resident_view: omitted.view,
                reclaimed_view,
            });
        }
    }
    contenders.sort_by_key(|contender| contender.value);
    add(&mut work.candidates, count(contenders.len())?)?;
    Ok(contenders)
}

#[allow(clippy::too_many_arguments)]
fn admit_roots(
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
    let homes_receipt = homes.receipt();
    let legality_receipt = legality.receipt();
    let ranges_receipt = ranges.receipt();
    let environment = target_register_environment_identity(
        reservations.target(),
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    if source.reload_value_homes != homes_receipt.identity()
        || source.legality != legality_receipt.identity()
        || homes_receipt.selected() != selected.selected_identity()
        || homes_receipt.ranges() != ranges_receipt.identity()
        || ranges_receipt.selected() != selected.selected_identity()
        || legality_receipt.ranges() != ranges_receipt.identity()
        || homes_receipt.legality() != legality_receipt.identity()
        || source.register_environment != environment
        || homes_receipt.register_environment() != environment
        || legality_receipt.register_environment() != environment
        || source.allocator_availability != legality_receipt.allocator_availability()
        || homes_receipt.allocator_availability() != legality_receipt.allocator_availability()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
    {
        return Err(GeneralizedSpillRecoveryChoiceError::RootMismatch);
    }
    Ok(())
}

fn checked_view<'a>(
    function: usize,
    class: omega_register_model::RegisterClassId,
    id: RegisterViewId,
    physical: &'a ValidatedPhysicalRegisterModel,
) -> Result<&'a RegisterView, GeneralizedSpillRecoveryChoiceError> {
    physical
        .model()
        .views
        .iter()
        .find(|view| view.id == id && view.class == class)
        .ok_or(GeneralizedSpillRecoveryChoiceError::InvalidView {
            function,
            view: id.0,
        })
}

fn overlap(
    left: RegisterViewId,
    right: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    let lookup = |id| physical.model().views.iter().find(|view| view.id == id);
    match (lookup(left), lookup(right)) {
        (Some(left), Some(right)) => left
            .units
            .iter()
            .chain(&left.write_units)
            .any(|unit| right.units.contains(unit) || right.write_units.contains(unit)),
        _ => true,
    }
}

#[derive(Default)]
struct Work {
    rules: u64,
    candidates: u64,
    steps: u64,
    commits: u64,
}

impl Work {
    const fn usage(&self, iterations: u64) -> OptimizationWorkUsage {
        OptimizationWorkUsage {
            rule_evaluations: self.rules,
            candidates: self.candidates,
            validation_steps: self.steps,
            commits: self.commits,
            iterations,
        }
    }
}

fn add(value: &mut u64, amount: u64) -> Result<(), GeneralizedSpillRecoveryChoiceError> {
    *value = value
        .checked_add(amount)
        .ok_or(GeneralizedSpillRecoveryChoiceError::WorkOverflow)?;
    Ok(())
}

fn count(value: usize) -> Result<u64, GeneralizedSpillRecoveryChoiceError> {
    u64::try_from(value).map_err(|_| GeneralizedSpillRecoveryChoiceError::WorkOverflow)
}

fn admit_policy(
    policy: GeneralizedSpillRecoveryChoicePolicy,
) -> Result<(), GeneralizedSpillRecoveryChoiceError> {
    match policy {
        GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1
        | GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1 => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn select(
    function: usize,
    policy: GeneralizedSpillRecoveryChoicePolicy,
    block: omega_selected_instructions::SelectedBlockId,
    point: LiveRangePoint,
    contenders: &[GeneralizedSpillRecoveryContender],
    residents: &[GeneralizedSpillRecoveryResident],
    selected: &SelectedFunction,
    ranges: &crate::FunctionLiveRanges,
    work: &mut Work,
) -> Result<Option<GeneralizedSpillRecoveryContender>, GeneralizedSpillRecoveryChoiceError> {
    let mut eligible = Vec::new();
    for contender in contenders {
        let admitted = match policy {
            GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1 => true,
            GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1 => {
                add(&mut work.rules, 1)?;
                add(&mut work.steps, 1)?;
                match contender.value {
                    GeneralizedReloadCoexistingValue::Reload(_) => true,
                    GeneralizedReloadCoexistingValue::Original(register) => {
                        let Some(resident) = residents.iter().find(|row| row.value == contender.value)
                        else {
                            return Err(GeneralizedSpillRecoveryChoiceError::InvalidBlocker {
                                function,
                            });
                        };
                        original_eligibility::is_eligible(
                            register, block, point, resident, selected, ranges, work,
                        )?
                    }
                }
            }
        };
        if admitted {
            eligible.push(*contender);
        }
    }
    Ok(eligible
        .into_iter()
        .max_by_key(|contender| ranking_key(policy, *contender)))
}

fn ranking_key(
    policy: GeneralizedSpillRecoveryChoicePolicy,
    contender: GeneralizedSpillRecoveryContender,
) -> (u8, LiveRangePoint, GeneralizedReloadCoexistingValue) {
    let original_priority = match policy {
        GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1 => 0,
        GeneralizedSpillRecoveryChoicePolicy::EpochTwoEligibleOriginalBeforeReloadThenFarthestEndThenHighestValueV1 => u8::from(matches!(
            contender.value,
            GeneralizedReloadCoexistingValue::Original(_)
        )),
    };
    (original_priority, contender.exclusive_end, contender.value)
}

#[cfg(test)]
mod tests {
    use omega_register_model::RegisterViewId;
    use omega_selected_instructions::VirtualRegisterId;

    use super::*;

    #[test]
    fn equal_end_tie_selects_the_highest_canonical_value() {
        let contender = |register| GeneralizedSpillRecoveryContender {
            value: GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(register)),
            exclusive_end: LiveRangePoint(20),
            resident_view: RegisterViewId(register as u16),
            reclaimed_view: RegisterViewId(9),
        };
        assert_eq!(
            [contender(3), contender(7)]
                .into_iter()
                .max_by_key(|contender| ranking_key(
                    GeneralizedSpillRecoveryChoicePolicy::EpochTwoFarthestEndThenHighestValueV1,
                    *contender
                ))
                .unwrap()
                .value,
            GeneralizedReloadCoexistingValue::Original(VirtualRegisterId(7))
        );
    }
}
