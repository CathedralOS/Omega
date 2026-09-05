//! Canonical sorted-schedule reconstruction and second-victim proposal.

use std::collections::BTreeSet;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    RegisterClassId, RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use selected_instructions::VirtualRegisterId;

use crate::{
    LiveRangePoint, SpillRecoveryChoiceError, SpillRecoveryChoicePlan, SpillRecoveryChoicePolicy,
    SpillRecoveryContender, SpillRecoveryResident, SpillRecoveryVictimChoice,
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedSpillRecoveryWorklist, VirtualInterference,
};

#[derive(Clone, Copy)]
struct ActiveHome {
    register: VirtualRegisterId,
    class: RegisterClassId,
    start: LiveRangePoint,
    end: LiveRangePoint,
    view: RegisterViewId,
}

#[derive(Default)]
struct Work {
    rules: u64,
    candidates: u64,
    steps: u64,
    commits: u64,
}

impl Work {
    fn add(value: &mut u64, amount: u64) -> Result<(), SpillRecoveryChoiceError> {
        *value = value
            .checked_add(amount)
            .ok_or(SpillRecoveryChoiceError::WorkOverflow)?;
        Ok(())
    }
    const fn usage(&self) -> OptimizationWorkUsage {
        OptimizationWorkUsage {
            rule_evaluations: self.rules,
            candidates: self.candidates,
            validation_steps: self.steps,
            commits: self.commits,
            iterations: 1,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute(
    worklist: &ValidatedSpillRecoveryWorklist,
    insertion: &ValidatedAbstractSpillInsertion,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: SpillRecoveryChoicePolicy,
    budget: OptimizationWorkBudget,
) -> Result<SpillRecoveryChoicePlan, SpillRecoveryChoiceError> {
    admit_roots(
        worklist,
        insertion,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    admit_policy(policy)?;
    let (function, item) = resolve_item(worklist, insertion)?;
    let action = insertion.plan().functions[function]
        .action
        .as_ref()
        .ok_or(SpillRecoveryChoiceError::UnsupportedWorklistShape)?;
    let mut work = Work::default();
    let choice = choose(
        function,
        item,
        action,
        &legality.plan().functions[function],
        &ranges.plan().functions[function],
        physical,
        &mut work,
    )?;
    let usage = work.usage();
    if !usage.within(budget) {
        return Err(SpillRecoveryChoiceError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let worklist_receipt = worklist.receipt();
    Ok(SpillRecoveryChoicePlan {
        worklist: worklist_receipt.identity(),
        abstract_spill_insertion: insertion.receipt().identity(),
        legality: legality.receipt().identity(),
        ranges: ranges.receipt().identity(),
        register_environment: worklist_receipt.register_environment(),
        allocator_availability: worklist_receipt.allocator_availability(),
        policy,
        budget,
        usage,
        choices: vec![choice],
    })
}

pub(super) fn admit_policy(
    policy: SpillRecoveryChoicePolicy,
) -> Result<(), SpillRecoveryChoiceError> {
    if policy != SpillRecoveryChoicePolicy::EpochOneFarthestEndThenHighestVregV1 {
        return Err(SpillRecoveryChoiceError::UnsupportedPolicy);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn admit_roots(
    worklist: &ValidatedSpillRecoveryWorklist,
    insertion: &ValidatedAbstractSpillInsertion,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), SpillRecoveryChoiceError> {
    let receipt = worklist.receipt();
    let environment = target_register_environment_identity(
        ranges.plan().target,
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    if receipt.abstract_spill_insertion() != insertion.receipt().identity()
        || receipt.legality() != legality.receipt().identity()
        || receipt.ranges() != ranges.receipt().identity()
        || receipt.register_environment() != legality.receipt().register_environment()
        || receipt.allocator_availability() != legality.receipt().allocator_availability()
        || environment != receipt.register_environment()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
    {
        return Err(SpillRecoveryChoiceError::RootMismatch);
    }
    Ok(())
}

fn resolve_item<'a>(
    worklist: &'a ValidatedSpillRecoveryWorklist,
    insertion: &ValidatedAbstractSpillInsertion,
) -> Result<(usize, &'a crate::SpillRecoveryWorkItem), SpillRecoveryChoiceError> {
    let epochs = &worklist.plan().epochs;
    let epoch = epochs
        .first()
        .filter(|epoch| epochs.len() == 1 && epoch.epoch == 1 && epoch.work_items.len() == 1)
        .ok_or(SpillRecoveryChoiceError::UnsupportedWorklistShape)?;
    let item = &epoch.work_items[0];
    let mut matches = insertion
        .plan()
        .functions
        .iter()
        .enumerate()
        .filter(|(_, function)| {
            function.machine == item.machine
                && function.action.as_ref().is_some_and(|action| {
                    action.reload.result == item.source_reload
                        && action.reload.destination_class == item.class
                        && action.rewrites.first().is_some_and(|rewrite| {
                            rewrite.block == item.block && rewrite.point == item.start
                        })
                })
        });
    let Some((function, _)) = matches.next() else {
        return Err(SpillRecoveryChoiceError::UnsupportedWorklistShape);
    };
    if matches.next().is_some() {
        return Err(SpillRecoveryChoiceError::AmbiguousWorkItem);
    }
    Ok((function, item))
}

fn choose(
    function: usize,
    item: &crate::SpillRecoveryWorkItem,
    action: &crate::AbstractSpillInsertionAction,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    work: &mut Work,
) -> Result<SpillRecoveryVictimChoice, SpillRecoveryChoiceError> {
    if legality.machine != item.machine || ranges.machine != item.machine {
        return Err(SpillRecoveryChoiceError::FunctionMismatch { function });
    }
    let mut schedule = legality
        .virtual_registers
        .iter()
        .map(|register| interval(function, register).map(|(start, end)| (register, start, end)))
        .collect::<Result<Vec<_>, _>>()?;
    schedule.sort_by_key(|(register, start, _)| (*start, register.virtual_register));
    let mut active = Vec::<ActiveHome>::new();
    for (register, start, end) in schedule {
        if start >= item.start {
            break;
        }
        Work::add(&mut work.rules, 1)?;
        active.retain(|home| home.end > start);
        let candidates = common_candidates(function, register)?;
        let view = if register.virtual_register == action.incoming {
            let all_blocked = candidates.iter().all(|candidate| {
                blocked_original(
                    register.virtual_register,
                    *candidate,
                    &active,
                    &ranges.interference,
                    physical,
                )
            });
            let victim_matches = active
                .iter()
                .any(|home| home.register == action.victim && home.view == action.victim_view);
            if start != action.pressure_point || !all_blocked || !victim_matches {
                return Err(SpillRecoveryChoiceError::PrefixMismatch { function });
            }
            active.retain(|home| home.register != action.victim);
            let mut recovered = None;
            for candidate in candidates {
                Work::add(&mut work.steps, 1)?;
                checked_view(function, register.class, candidate, physical)?;
                if !blocked_original(
                    register.virtual_register,
                    candidate,
                    &active,
                    &ranges.interference,
                    physical,
                ) {
                    recovered = Some(candidate);
                    break;
                }
            }
            if recovered != Some(action.incoming_view) {
                return Err(SpillRecoveryChoiceError::PrefixMismatch { function });
            }
            action.incoming_view
        } else {
            let mut selected = None;
            for candidate in candidates {
                Work::add(&mut work.steps, 1)?;
                checked_view(function, register.class, candidate, physical)?;
                if !blocked_original(
                    register.virtual_register,
                    candidate,
                    &active,
                    &ranges.interference,
                    physical,
                ) {
                    selected = Some(candidate);
                    break;
                }
            }
            selected.ok_or(SpillRecoveryChoiceError::SecondaryPressure {
                function,
                register: register.virtual_register.0,
            })?
        };
        checked_view(function, register.class, view, physical)?;
        active.push(ActiveHome {
            register: register.virtual_register,
            class: register.class,
            start,
            end,
            view,
        });
    }
    active.retain(|home| home.end > item.start);
    if item.candidates.iter().any(|candidate| {
        !active
            .iter()
            .any(|home| views_overlap(*candidate, home.view, physical))
    }) {
        return Err(SpillRecoveryChoiceError::PrefixMismatch { function });
    }
    active.sort_by_key(|home| home.register);
    let active_residents = active
        .iter()
        .map(|home| SpillRecoveryResident {
            virtual_register: home.register,
            class: home.class,
            start: home.start,
            exclusive_end: home.end,
            view: home.view,
        })
        .collect::<Vec<_>>();
    let mut contenders = Vec::new();
    for omitted in &active {
        for candidate in &item.candidates {
            Work::add(&mut work.steps, 1)?;
            checked_view(function, item.class, *candidate, physical)?;
            if active.iter().all(|home| {
                home.register == omitted.register || !views_overlap(*candidate, home.view, physical)
            }) {
                contenders.push(SpillRecoveryContender {
                    virtual_register: omitted.register,
                    exclusive_end: omitted.end,
                    resident_view: omitted.view,
                    reclaimed_view: *candidate,
                });
                break;
            }
        }
    }
    contenders.sort_by_key(|contender| contender.virtual_register);
    Work::add(
        &mut work.candidates,
        u64::try_from(contenders.len()).map_err(|_| SpillRecoveryChoiceError::WorkOverflow)?,
    )?;
    let selected = contenders
        .iter()
        .max_by_key(|contender| (contender.exclusive_end, contender.virtual_register))
        .copied()
        .ok_or(SpillRecoveryChoiceError::NoRecoverableVictim { function })?;
    Work::add(&mut work.commits, 1)?;
    Ok(SpillRecoveryVictimChoice {
        work_item: item.synthetic,
        function,
        machine: item.machine,
        block: item.block,
        point: item.start,
        reload_class: item.class,
        reload_candidates: item.candidates.clone(),
        active_residents,
        contenders,
        selected_victim: selected.virtual_register,
        selected_victim_view: selected.resident_view,
        reclaimed_view: selected.reclaimed_view,
    })
}

fn interval(
    function: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<(LiveRangePoint, LiveRangePoint), SpillRecoveryChoiceError> {
    let first = register
        .points
        .first()
        .ok_or(SpillRecoveryChoiceError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let last = register.points.last().expect("nonempty points established");
    let end = last.point.0.checked_add(1).map(LiveRangePoint).ok_or(
        SpillRecoveryChoiceError::IntervalOverflow {
            function,
            register: register.virtual_register.0,
        },
    )?;
    Ok((first.point, end))
}

fn common_candidates(
    function: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<Vec<RegisterViewId>, SpillRecoveryChoiceError> {
    let first = register
        .points
        .first()
        .ok_or(SpillRecoveryChoiceError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let mut shared = first.candidates.iter().copied().collect::<BTreeSet<_>>();
    for point in &register.points[1..] {
        shared.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if shared.is_empty() {
        return Err(SpillRecoveryChoiceError::PrefixMismatch { function });
    }
    Ok(shared.into_iter().collect())
}

fn blocked_original(
    register: VirtualRegisterId,
    candidate: RegisterViewId,
    active: &[ActiveHome],
    interference: &[VirtualInterference],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    active.iter().any(|home| {
        interferes(register, home.register, interference)
            && views_overlap(candidate, home.view, physical)
    })
}

fn interferes(
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    interference: &[VirtualInterference],
) -> bool {
    let (lower, higher) = if left < right {
        (left, right)
    } else {
        (right, left)
    };
    interference
        .binary_search(&VirtualInterference { lower, higher })
        .is_ok()
}

fn checked_view(
    function: usize,
    class: RegisterClassId,
    id: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<&RegisterView, SpillRecoveryChoiceError> {
    physical
        .model()
        .views
        .iter()
        .find(|view| view.id == id && view.class == class)
        .ok_or(SpillRecoveryChoiceError::InvalidView {
            function,
            view: id.0,
        })
}

fn views_overlap(
    left: RegisterViewId,
    right: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    let view = |id| physical.model().views.iter().find(|view| view.id == id);
    match (view(left), view(right)) {
        (Some(left), Some(right)) => left
            .units
            .iter()
            .chain(&left.write_units)
            .any(|unit| right.units.contains(unit) || right.write_units.contains(unit)),
        _ => true,
    }
}
