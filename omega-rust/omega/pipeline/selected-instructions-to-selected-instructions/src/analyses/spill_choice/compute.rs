use std::collections::BTreeSet;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use selected_instructions::VirtualRegisterId;

use crate::{SpillChoiceError, ValidatedAllocationLegality, ValidatedLiveRanges};
use register_homes::{
    FunctionSpillChoices, PressureContender, PressureResident, SpillChoice, SpillChoicePlan,
    SpillChoicePolicy,
};
use selected_instructions::{LiveRangePoint, VirtualInterference};

#[derive(Debug, Clone, Copy)]
struct ActiveHome {
    register: VirtualRegisterId,
    class: register_model::RegisterClassId,
    start: LiveRangePoint,
    end: LiveRangePoint,
    view: RegisterViewId,
}

#[derive(Default)]
struct WorkCounter {
    rule_evaluations: u64,
    candidates: u64,
    validation_steps: u64,
    commits: u64,
}

impl WorkCounter {
    fn add(value: &mut u64, amount: u64) -> Result<(), SpillChoiceError> {
        *value = value
            .checked_add(amount)
            .ok_or(SpillChoiceError::WorkOverflow)?;
        Ok(())
    }
    fn usage(&self) -> OptimizationWorkUsage {
        OptimizationWorkUsage {
            rule_evaluations: self.rule_evaluations,
            candidates: self.candidates,
            validation_steps: self.validation_steps,
            commits: self.commits,
            iterations: 1,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_spill_choices(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: SpillChoicePolicy,
    budget: OptimizationWorkBudget,
) -> Result<SpillChoicePlan, SpillChoiceError> {
    validate_roots(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    if policy != SpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1 {
        return Err(SpillChoiceError::UnsupportedPolicy);
    }
    let mut work = WorkCounter::default();
    let mut functions = Vec::with_capacity(legality.plan().functions.len());
    for (function_index, (legality_function, range_function)) in legality
        .plan()
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .enumerate()
    {
        reject_constraint_topologies(function_index, range_function)?;
        functions.push(compute_function(
            function_index,
            legality_function,
            range_function,
            physical,
            &mut work,
        )?);
    }
    let usage = work.usage();
    if !usage.within(budget) {
        return Err(SpillChoiceError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(SpillChoicePlan {
        legality: legality.receipt().identity(),
        ranges: ranges.receipt().identity(),
        register_environment,
        allocator_availability: legality.receipt().allocator_availability(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn reject_constraint_topologies(
    function: usize,
    ranges: &selected_instructions::FunctionLiveRanges,
) -> Result<(), SpillChoiceError> {
    if !ranges.tied_pairs.is_empty() {
        return Err(SpillChoiceError::UnsupportedTiedOperands { function });
    }
    // Untied early-clobber rows are admitted: the home-assignment conflict
    // model already names their hazard, so this stage carries the same
    // directional write-before-read check against every assigned home.
    // Pressure handling under an early-clobber topology still refuses
    // inside `compute_function` once no legal home remains.
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_roots(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), SpillChoiceError> {
    if legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
        || ranges.plan().target.architecture != physical.model().architecture
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
        || target_register_environment_identity(
            ranges.plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
        || legality.plan().functions.len() != ranges.plan().functions.len()
    {
        return Err(SpillChoiceError::RootMismatch);
    }
    Ok(())
}

fn compute_function(
    function_index: usize,
    legality: &register_homes::FunctionAllocationLegality,
    ranges: &selected_instructions::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    work: &mut WorkCounter,
) -> Result<FunctionSpillChoices, SpillChoiceError> {
    if legality.machine != ranges.machine
        || legality.virtual_registers.len() != ranges.virtual_registers.len()
    {
        return Err(SpillChoiceError::FunctionMismatch {
            function: function_index,
        });
    }
    let mut order = legality
        .virtual_registers
        .iter()
        .enumerate()
        .map(|(position, register)| {
            interval_bounds(function_index, register).map(|(start, end)| (position, start, end))
        })
        .collect::<Result<Vec<_>, _>>()?;
    order.sort_by_key(|(position, start, _)| {
        (
            start.0,
            legality.virtual_registers[*position].virtual_register,
        )
    });
    let mut active = Vec::<ActiveHome>::new();
    // Every register already given a home, regardless of whether its
    // interval still overlaps the scheduling frontier. Early-clobber
    // hazards are pointwise — the definition's write precedes the uses'
    // read inside one instruction — so a use whose interval just closed
    // still forbids the incoming definition's write units, and an
    // early definition already seated still forbids an incoming use.
    let mut assigned = Vec::<(VirtualRegisterId, RegisterViewId)>::new();
    for (position, start, end) in order {
        WorkCounter::add(&mut work.rule_evaluations, 1)?;
        let register = &legality.virtual_registers[position];
        let range = ranges
            .virtual_registers
            .get(position)
            .filter(|range| {
                range.virtual_register == register.virtual_register && range.class == register.class
            })
            .ok_or(SpillChoiceError::VirtualRegisterMismatch {
                function: function_index,
                register: register.virtual_register.0,
            })?;
        if !register.entry_transitions.is_empty() {
            return Err(SpillChoiceError::UnresolvedEntryTransitions {
                function: function_index,
                register: register.virtual_register.0,
            });
        }
        active.retain(|entry| entry.end > start);
        let candidates = common_candidates(function_index, register)?;
        let mut selected = None;
        for candidate in &candidates {
            WorkCounter::add(&mut work.validation_steps, 1)?;
            let view = checked_view(
                function_index,
                register.virtual_register,
                register.class,
                *candidate,
                physical,
            )?;
            if !blocked(
                register.virtual_register,
                view,
                &active,
                &ranges.interference,
                physical,
                None,
            ) && !early_clobber_conflicts(
                register.virtual_register,
                view,
                &assigned,
                ranges,
                physical,
            ) {
                selected = Some(*candidate);
                break;
            }
        }
        if let Some(view) = selected {
            active.push(ActiveHome {
                register: register.virtual_register,
                class: register.class,
                start,
                end,
                view,
            });
            active.sort_by_key(|entry| (entry.end, entry.register));
            assigned.push((register.virtual_register, view));
            continue;
        }

        // Victim selection around an early-clobbered home is not modeled:
        // the definition already occupies its view while its uses are still
        // read, so evicting either side of that hazard needs evidence this
        // bounded policy does not carry.
        if !ranges.early_clobbers.is_empty() {
            return Err(SpillChoiceError::UnsupportedEarlyClobber {
                function: function_index,
            });
        }

        let block = register.points[0].block;
        if register.points.iter().any(|point| point.block != block) {
            return Err(SpillChoiceError::UnsupportedPressureShape {
                function: function_index,
                register: register.virtual_register.0,
            });
        }
        validate_local_shape(
            function_index,
            register.virtual_register,
            start,
            end,
            block,
            range,
        )?;
        for resident in &active {
            let resident_range = ranges
                .virtual_registers
                .iter()
                .find(|candidate| candidate.virtual_register == resident.register)
                .ok_or(SpillChoiceError::VirtualRegisterMismatch {
                    function: function_index,
                    register: resident.register.0,
                })?;
            validate_local_shape(
                function_index,
                resident.register,
                resident.start,
                resident.end,
                block,
                resident_range,
            )?;
        }
        if active.iter().any(|resident| {
            legality
                .virtual_registers
                .iter()
                .find(|candidate| candidate.virtual_register == resident.register)
                .is_none_or(|candidate| candidate.points.iter().any(|point| point.block != block))
        }) {
            return Err(SpillChoiceError::UnsupportedPressureShape {
                function: function_index,
                register: register.virtual_register.0,
            });
        }
        let active_residents = active
            .iter()
            .map(|entry| PressureResident {
                virtual_register: entry.register,
                class: entry.class,
                start: entry.start,
                exclusive_end: entry.end,
                view: entry.view,
            })
            .collect::<Vec<_>>();
        let mut contenders = vec![PressureContender {
            virtual_register: register.virtual_register,
            exclusive_end: end,
            reclaimed_view: None,
        }];
        for resident in &active {
            let mut reclaimed = None;
            for candidate in &candidates {
                WorkCounter::add(&mut work.validation_steps, 1)?;
                let view = checked_view(
                    function_index,
                    register.virtual_register,
                    register.class,
                    *candidate,
                    physical,
                )?;
                if !blocked(
                    register.virtual_register,
                    view,
                    &active,
                    &ranges.interference,
                    physical,
                    Some(resident.register),
                ) {
                    reclaimed = Some(*candidate);
                    break;
                }
            }
            if let Some(view) = reclaimed {
                contenders.push(PressureContender {
                    virtual_register: resident.register,
                    exclusive_end: resident.end,
                    reclaimed_view: Some(view),
                });
            }
        }
        contenders.sort_by_key(|contender| contender.virtual_register);
        WorkCounter::add(
            &mut work.candidates,
            u64::try_from(contenders.len()).map_err(|_| SpillChoiceError::WorkOverflow)?,
        )?;
        WorkCounter::add(&mut work.commits, 1)?;
        let selected_victim = contenders
            .iter()
            .max_by_key(|contender| (contender.exclusive_end, contender.virtual_register))
            .expect("incoming contender is always present")
            .virtual_register;
        return Ok(FunctionSpillChoices {
            machine: legality.machine,
            choice: Some(SpillChoice {
                block,
                point: start,
                incoming: register.virtual_register,
                incoming_class: register.class,
                incoming_common_candidates: candidates.into_iter().collect(),
                active_residents,
                contenders,
                selected_victim,
            }),
        });
    }
    Ok(FunctionSpillChoices {
        machine: legality.machine,
        choice: None,
    })
}

fn validate_local_shape(
    function: usize,
    register: VirtualRegisterId,
    start: LiveRangePoint,
    end: LiveRangePoint,
    block: selected_instructions::SelectedBlockId,
    range: &selected_instructions::VirtualLiveRange,
) -> Result<(), SpillChoiceError> {
    if !range.edge_connectors.is_empty() || range.fragments.len() != 1 {
        return Err(SpillChoiceError::UnsupportedPressureShape {
            function,
            register: register.0,
        });
    }
    let fragment = range.fragments[0];
    if fragment.block != block || fragment.start != start || fragment.end != end {
        return Err(SpillChoiceError::UnsupportedPressureShape {
            function,
            register: register.0,
        });
    }
    Ok(())
}

fn interval_bounds(
    function: usize,
    register: &register_homes::VirtualRegisterAllocationLegality,
) -> Result<(LiveRangePoint, LiveRangePoint), SpillChoiceError> {
    let first = register
        .points
        .first()
        .ok_or(SpillChoiceError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let last = register.points.last().expect("nonempty points established");
    let end = last.point.0.checked_add(1).map(LiveRangePoint).ok_or(
        SpillChoiceError::IntervalOverflow {
            function,
            register: register.virtual_register.0,
        },
    )?;
    Ok((first.point, end))
}

fn common_candidates(
    function: usize,
    register: &register_homes::VirtualRegisterAllocationLegality,
) -> Result<BTreeSet<RegisterViewId>, SpillChoiceError> {
    let first = register
        .points
        .first()
        .ok_or(SpillChoiceError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let mut common = first.candidates.iter().copied().collect::<BTreeSet<_>>();
    for point in &register.points[1..] {
        common.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    // Early-clobber definition points carry their own legality candidates:
    // the home must also satisfy every view constraint the early point
    // names, matching the home-assignment domain construction.
    for point in &register.early_clobber_points {
        common.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if common.is_empty() {
        return Err(SpillChoiceError::NoCommonCandidate {
            function,
            register: register.virtual_register.0,
        });
    }
    Ok(common)
}

/// Directional write-before-read conflicts for early-clobber definitions,
/// mirroring the home-assignment model: an incoming definition may not take
/// a view whose write units touch a seated use's storage, and an incoming
/// use may not take a view whose storage a seated definition writes early.
fn early_clobber_conflicts(
    incoming: VirtualRegisterId,
    view: &RegisterView,
    assigned: &[(VirtualRegisterId, RegisterViewId)],
    ranges: &selected_instructions::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    assigned.iter().any(|(seated, seated_view_id)| {
        let seated_view = &physical.model().views[usize::from(seated_view_id.0)];
        ranges.early_clobbers.iter().any(|early| {
            (early.def_virtual_register == incoming
                && early
                    .uses
                    .iter()
                    .any(|used| used.virtual_register == *seated)
                && view
                    .write_units
                    .iter()
                    .any(|unit| seated_view.units.contains(unit)))
                || (early.def_virtual_register == *seated
                    && early
                        .uses
                        .iter()
                        .any(|used| used.virtual_register == incoming)
                    && seated_view
                        .write_units
                        .iter()
                        .any(|unit| view.units.contains(unit)))
        })
    })
}

fn checked_view(
    function: usize,
    register: VirtualRegisterId,
    class: register_model::RegisterClassId,
    candidate: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<&RegisterView, SpillChoiceError> {
    physical
        .model()
        .views
        .get(usize::from(candidate.0))
        .filter(|view| view.id == candidate && view.class == class)
        .ok_or(SpillChoiceError::UnknownOrIncompatibleView {
            function,
            register: register.0,
            view: candidate.0,
        })
}

fn blocked(
    incoming: VirtualRegisterId,
    view: &RegisterView,
    active: &[ActiveHome],
    interference: &[VirtualInterference],
    physical: &ValidatedPhysicalRegisterModel,
    omitted: Option<VirtualRegisterId>,
) -> bool {
    active
        .iter()
        .filter(|entry| Some(entry.register) != omitted)
        .any(|entry| {
            interferes(incoming, entry.register, interference)
                && footprints_overlap(view, &physical.model().views[usize::from(entry.view.0)])
        })
}

fn interferes(
    left: VirtualRegisterId,
    right: VirtualRegisterId,
    interference: &[VirtualInterference],
) -> bool {
    let pair = if left < right {
        VirtualInterference {
            lower: left,
            higher: right,
        }
    } else {
        VirtualInterference {
            lower: right,
            higher: left,
        }
    };
    interference.binary_search(&pair).is_ok()
}

fn footprints_overlap(left: &RegisterView, right: &RegisterView) -> bool {
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
}

#[cfg(test)]
mod tests;
