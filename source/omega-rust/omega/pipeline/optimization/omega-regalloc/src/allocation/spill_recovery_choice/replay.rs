//! Independent point-indexed reconstruction of the epoch-one victim choice.

use std::collections::BTreeMap;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{RegisterClassId, RegisterViewId, ValidatedPhysicalRegisterModel};
use omega_selected_instructions::VirtualRegisterId;

use crate::{
    LiveRangePoint, SpillRecoveryChoiceError, SpillRecoveryChoicePlan, SpillRecoveryChoicePolicy,
    SpillRecoveryContender, SpillRecoveryResident, SpillRecoveryVictimChoice,
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedSpillRecoveryWorklist, VirtualInterference,
};

#[derive(Clone, Copy)]
struct Resident {
    register: VirtualRegisterId,
    class: RegisterClassId,
    start: LiveRangePoint,
    end: LiveRangePoint,
    view: RegisterViewId,
}

#[derive(Default)]
struct ReplayWork {
    rules: u64,
    candidates: u64,
    steps: u64,
    commits: u64,
}

impl ReplayWork {
    fn bump(field: &mut u64, amount: u64) -> Result<(), SpillRecoveryChoiceError> {
        *field = field
            .checked_add(amount)
            .ok_or(SpillRecoveryChoiceError::WorkOverflow)?;
        Ok(())
    }
    const fn finish(&self) -> OptimizationWorkUsage {
        OptimizationWorkUsage {
            rule_evaluations: self.rules,
            candidates: self.candidates,
            validation_steps: self.steps,
            commits: self.commits,
            iterations: 1,
        }
    }
}

pub(super) fn replay(
    worklist: &ValidatedSpillRecoveryWorklist,
    insertion: &ValidatedAbstractSpillInsertion,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    policy: SpillRecoveryChoicePolicy,
    budget: OptimizationWorkBudget,
) -> Result<SpillRecoveryChoicePlan, SpillRecoveryChoiceError> {
    super::compute::admit_policy(policy)?;
    let (function, item) = replay_item(worklist, insertion)?;
    let action = insertion.plan().functions[function]
        .action
        .as_ref()
        .ok_or(SpillRecoveryChoiceError::UnsupportedWorklistShape)?;
    let mut work = ReplayWork::default();
    let choice = reconstruct(
        function,
        item,
        action,
        &legality.plan().functions[function],
        &ranges.plan().functions[function],
        physical,
        &mut work,
    )?;
    let usage = work.finish();
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

fn replay_item<'a>(
    worklist: &'a ValidatedSpillRecoveryWorklist,
    insertion: &ValidatedAbstractSpillInsertion,
) -> Result<(usize, &'a crate::SpillRecoveryWorkItem), SpillRecoveryChoiceError> {
    if worklist.plan().epochs.len() != 1 {
        return Err(SpillRecoveryChoiceError::UnsupportedWorklistShape);
    }
    let epoch = &worklist.plan().epochs[0];
    if epoch.epoch != 1 || epoch.work_items.len() != 1 {
        return Err(SpillRecoveryChoiceError::UnsupportedWorklistShape);
    }
    let item = &epoch.work_items[0];
    let candidates = insertion
        .plan()
        .functions
        .iter()
        .enumerate()
        .filter_map(|(index, function)| {
            let action = function.action.as_ref()?;
            let first = action.rewrites.first()?;
            (function.machine == item.machine
                && action.reload.result == item.source_reload
                && action.reload.destination_class == item.class
                && first.block == item.block
                && first.point == item.start)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [function] => Ok((*function, item)),
        [] => Err(SpillRecoveryChoiceError::UnsupportedWorklistShape),
        _ => Err(SpillRecoveryChoiceError::AmbiguousWorkItem),
    }
}

fn reconstruct(
    function: usize,
    item: &crate::SpillRecoveryWorkItem,
    action: &crate::AbstractSpillInsertionAction,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    work: &mut ReplayWork,
) -> Result<SpillRecoveryVictimChoice, SpillRecoveryChoiceError> {
    if legality.machine != item.machine || ranges.machine != item.machine {
        return Err(SpillRecoveryChoiceError::FunctionMismatch { function });
    }
    let mut events = BTreeMap::<LiveRangePoint, Vec<(usize, LiveRangePoint)>>::new();
    for (index, register) in legality.virtual_registers.iter().enumerate() {
        let first = register
            .points
            .first()
            .ok_or(SpillRecoveryChoiceError::NoLivePoints {
                function,
                register: register.virtual_register.0,
            })?;
        let last = register.points.last().expect("nonempty point roster");
        let end = LiveRangePoint(last.point.0.checked_add(1).ok_or(
            SpillRecoveryChoiceError::IntervalOverflow {
                function,
                register: register.virtual_register.0,
            },
        )?);
        events.entry(first.point).or_default().push((index, end));
    }
    for starting in events.values_mut() {
        starting.sort_by_key(|(index, _)| legality.virtual_registers[*index].virtual_register);
    }
    let mut residents = Vec::<Resident>::new();
    for (point, starting) in events.range(..item.start) {
        residents.retain(|resident| resident.end > *point);
        for (index, end) in starting {
            ReplayWork::bump(&mut work.rules, 1)?;
            let register = &legality.virtual_registers[*index];
            let domain = replay_domain(function, register)?;
            let view = if register.virtual_register == action.incoming {
                let blocked = domain.iter().all(|candidate| {
                    original_blocked(
                        register.virtual_register,
                        *candidate,
                        &residents,
                        &ranges.interference,
                        physical,
                    )
                });
                let victim = residents.iter().any(|resident| {
                    resident.register == action.victim && resident.view == action.victim_view
                });
                if *point != action.pressure_point || !blocked || !victim {
                    return Err(SpillRecoveryChoiceError::PrefixMismatch { function });
                }
                residents.retain(|resident| resident.register != action.victim);
                let mut recovered = None;
                for candidate in domain {
                    ReplayWork::bump(&mut work.steps, 1)?;
                    replay_view(function, register.class, candidate, physical)?;
                    if !original_blocked(
                        register.virtual_register,
                        candidate,
                        &residents,
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
                for candidate in domain {
                    ReplayWork::bump(&mut work.steps, 1)?;
                    replay_view(function, register.class, candidate, physical)?;
                    if !original_blocked(
                        register.virtual_register,
                        candidate,
                        &residents,
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
            replay_view(function, register.class, view, physical)?;
            residents.push(Resident {
                register: register.virtual_register,
                class: register.class,
                start: *point,
                end: *end,
                view,
            });
        }
    }
    residents.retain(|resident| resident.end > item.start);
    if item.candidates.iter().any(|candidate| {
        residents
            .iter()
            .all(|resident| !replay_overlap(*candidate, resident.view, physical))
    }) {
        return Err(SpillRecoveryChoiceError::PrefixMismatch { function });
    }
    residents.sort_by_key(|resident| resident.register);
    let active_residents = residents
        .iter()
        .map(|resident| SpillRecoveryResident {
            virtual_register: resident.register,
            class: resident.class,
            start: resident.start,
            exclusive_end: resident.end,
            view: resident.view,
        })
        .collect::<Vec<_>>();
    let mut contenders = Vec::new();
    for omitted in &residents {
        let mut reclaimed = None;
        for candidate in &item.candidates {
            ReplayWork::bump(&mut work.steps, 1)?;
            replay_view(function, item.class, *candidate, physical)?;
            let still_blocked = residents.iter().any(|resident| {
                resident.register != omitted.register
                    && replay_overlap(*candidate, resident.view, physical)
            });
            if !still_blocked {
                reclaimed = Some(*candidate);
                break;
            }
        }
        if let Some(reclaimed_view) = reclaimed {
            contenders.push(SpillRecoveryContender {
                virtual_register: omitted.register,
                exclusive_end: omitted.end,
                resident_view: omitted.view,
                reclaimed_view,
            });
        }
    }
    contenders.sort_by_key(|contender| contender.virtual_register);
    ReplayWork::bump(
        &mut work.candidates,
        u64::try_from(contenders.len()).map_err(|_| SpillRecoveryChoiceError::WorkOverflow)?,
    )?;
    let selected = contenders
        .iter()
        .copied()
        .max_by(|left, right| {
            left.exclusive_end
                .cmp(&right.exclusive_end)
                .then(left.virtual_register.cmp(&right.virtual_register))
        })
        .ok_or(SpillRecoveryChoiceError::NoRecoverableVictim { function })?;
    ReplayWork::bump(&mut work.commits, 1)?;
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

fn replay_domain(
    function: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<Vec<RegisterViewId>, SpillRecoveryChoiceError> {
    let mut rows = register.points.iter();
    let first = rows.next().ok_or(SpillRecoveryChoiceError::NoLivePoints {
        function,
        register: register.virtual_register.0,
    })?;
    let mut shared = first.candidates.clone();
    for row in rows {
        shared.retain(|candidate| row.candidates.binary_search(candidate).is_ok());
    }
    if shared.is_empty() {
        return Err(SpillRecoveryChoiceError::PrefixMismatch { function });
    }
    Ok(shared)
}

fn original_blocked(
    register: VirtualRegisterId,
    candidate: RegisterViewId,
    residents: &[Resident],
    interference: &[VirtualInterference],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    residents.iter().any(|resident| {
        replay_interferes(register, resident.register, interference)
            && replay_overlap(candidate, resident.view, physical)
    })
}

fn replay_interferes(
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    interference: &[VirtualInterference],
) -> bool {
    let (lower, higher) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    interference
        .iter()
        .any(|edge| edge.lower == lower && edge.higher == higher)
}

fn replay_view(
    function: usize,
    class: RegisterClassId,
    id: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), SpillRecoveryChoiceError> {
    physical
        .model()
        .views
        .iter()
        .any(|view| view.id == id && view.class == class)
        .then_some(())
        .ok_or(SpillRecoveryChoiceError::InvalidView {
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
