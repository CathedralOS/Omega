//! Canonical proposal construction for the bounded one-spill reload lane.

use std::collections::BTreeSet;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    RegisterClassId, RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use selected_instructions::VirtualRegisterId;

use crate::{
    AbstractSpillInsertionAction, FunctionReloadValueHomes, LiveRangePoint, ReloadCoexistingHome,
    ReloadValueHomeAssignment, ReloadValueHomeError, ReloadValueHomePlan, ReloadValueHomePolicy,
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedLogicalSpillOperations, VirtualInterference,
};

#[derive(Clone, Copy)]
struct ActiveHome {
    register: Option<VirtualRegisterId>,
    class: RegisterClassId,
    end: LiveRangePoint,
    view: RegisterViewId,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::assignment) fn compute(
    insertion: &ValidatedAbstractSpillInsertion,
    logical: &ValidatedLogicalSpillOperations,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: ReloadValueHomePolicy,
    budget: OptimizationWorkBudget,
) -> Result<ReloadValueHomePlan, ReloadValueHomeError> {
    admit_roots(
        insertion,
        logical,
        legality,
        ranges,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    if policy != ReloadValueHomePolicy::BlockLocalSingleSpillReloadFirstLowestCompatibleViewV1 {
        return Err(ReloadValueHomeError::UnsupportedPolicy);
    }
    let functions = insertion
        .plan()
        .functions
        .iter()
        .zip(&legality.plan().functions)
        .zip(&ranges.plan().functions)
        .enumerate()
        .map(|(function, ((insertion, legality), ranges))| {
            build_function(function, insertion, legality, ranges, physical)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let usage = work_usage(&functions)?;
    if !usage.within(budget) {
        return Err(ReloadValueHomeError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let logical_receipt = logical.receipt();
    Ok(ReloadValueHomePlan {
        abstract_spill_insertion: insertion.receipt().identity(),
        logical_spill_operations: logical_receipt.identity(),
        legality: legality.receipt().identity(),
        ranges: ranges.receipt().identity(),
        register_environment: logical_receipt.register_environment(),
        allocator_availability: logical_receipt.allocator_availability(),
        policy,
        budget,
        usage,
        functions,
    })
}

#[allow(clippy::too_many_arguments)]
fn admit_roots(
    insertion: &ValidatedAbstractSpillInsertion,
    logical: &ValidatedLogicalSpillOperations,
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), ReloadValueHomeError> {
    let logical_receipt = logical.receipt();
    let environment = target_register_environment_identity(
        ranges.plan().target,
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    if insertion.receipt().logical_spill_operations() != logical_receipt.identity()
        || logical_receipt.legality() != legality.receipt().identity()
        || logical_receipt.ranges() != ranges.receipt().identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || logical_receipt.register_environment() != legality.receipt().register_environment()
        || logical_receipt.allocator_availability() != legality.receipt().allocator_availability()
        || environment != logical_receipt.register_environment()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
        || insertion.plan().functions.len() != legality.plan().functions.len()
        || insertion.plan().functions.len() != ranges.plan().functions.len()
    {
        return Err(ReloadValueHomeError::RootMismatch);
    }
    Ok(())
}

fn build_function(
    function: usize,
    insertion: &crate::FunctionAbstractSpillInsertion,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionReloadValueHomes, ReloadValueHomeError> {
    if insertion.machine != legality.machine || insertion.machine != ranges.machine {
        return Err(ReloadValueHomeError::FunctionMismatch { function });
    }
    if !ranges.tied_pairs.is_empty() || !ranges.early_clobbers.is_empty() {
        return Err(ReloadValueHomeError::UnsupportedConstraintTopology { function });
    }
    let assignment = insertion
        .action
        .as_ref()
        .map(|action| assign(function, action, legality, ranges, physical))
        .transpose()?;
    Ok(FunctionReloadValueHomes {
        machine: insertion.machine,
        assignment,
    })
}

fn assign(
    function: usize,
    action: &AbstractSpillInsertionAction,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<ReloadValueHomeAssignment, ReloadValueHomeError> {
    let first = action
        .rewrites
        .first()
        .ok_or(ReloadValueHomeError::UnsupportedReloadShape { function })?;
    let last = action
        .rewrites
        .last()
        .ok_or(ReloadValueHomeError::UnsupportedReloadShape { function })?;
    let exclusive_end = LiveRangePoint(last.point.0.checked_add(1).ok_or(
        ReloadValueHomeError::IntervalOverflow {
            function,
            register: action.victim.0,
        },
    )?);
    if first.block != action.rewrites[0].block
        || action.reload.before_instruction != first.instruction
        || action
            .rewrites
            .iter()
            .any(|rewrite| rewrite.block != first.block || rewrite.result != action.reload.result)
    {
        return Err(ReloadValueHomeError::UnsupportedReloadShape { function });
    }
    let victim = legality_row(function, legality, action.victim)?;
    let candidates = reload_candidates(function, victim, first.point, exclusive_end, first.block)?;

    let mut schedule = legality
        .virtual_registers
        .iter()
        .map(|register| interval(function, register).map(|(start, end)| (register, start, end)))
        .collect::<Result<Vec<_>, _>>()?;
    schedule.sort_by_key(|(register, start, _)| (*start, register.virtual_register));
    let mut active = Vec::<ActiveHome>::new();
    let mut reload_inserted = false;
    let mut chosen_reload = None;
    let mut coexisting = Vec::<ReloadCoexistingHome>::new();

    for (register, start, end) in schedule {
        if !reload_inserted && first.point <= start {
            chosen_reload = Some(insert_reload(
                function,
                action,
                first.point,
                exclusive_end,
                &candidates,
                &mut active,
                &mut coexisting,
                physical,
            )?);
            reload_inserted = true;
        }
        if start >= exclusive_end {
            break;
        }
        active.retain(|home| home.end > start);
        let range = ranges
            .virtual_registers
            .iter()
            .find(|range| range.virtual_register == register.virtual_register)
            .ok_or(ReloadValueHomeError::VirtualRegisterMismatch {
                function,
                register: register.virtual_register.0,
            })?;
        if end > action.pressure_point
            && start < exclusive_end
            && (range.fragments.len() != 1
                || !range.edge_connectors.is_empty()
                || range.fragments[0].block != first.block)
        {
            return Err(ReloadValueHomeError::UnsupportedReloadShape { function });
        }
        let domain = common_candidates(function, register)?;
        let view = if register.virtual_register == action.incoming {
            if start != action.pressure_point
                || !active.iter().any(|home| {
                    home.register == Some(action.victim) && home.view == action.victim_view
                })
                || domain.iter().any(|candidate| {
                    !blocked_original(
                        register.virtual_register,
                        *candidate,
                        &active,
                        &ranges.interference,
                        physical,
                    )
                })
            {
                return Err(ReloadValueHomeError::PrefixMismatch { function });
            }
            active.retain(|home| home.register != Some(action.victim));
            let selected = domain.iter().copied().find(|candidate| {
                !blocked_original(
                    register.virtual_register,
                    *candidate,
                    &active,
                    &ranges.interference,
                    physical,
                )
            });
            if selected != Some(action.incoming_view) {
                return Err(ReloadValueHomeError::PrefixMismatch { function });
            }
            action.incoming_view
        } else {
            domain
                .iter()
                .copied()
                .find(|candidate| {
                    !blocked_original(
                        register.virtual_register,
                        *candidate,
                        &active,
                        &ranges.interference,
                        physical,
                    )
                })
                .ok_or_else(|| {
                    if start <= action.pressure_point {
                        ReloadValueHomeError::PrefixMismatch { function }
                    } else {
                        ReloadValueHomeError::SecondaryPressure {
                            function,
                            register: register.virtual_register.0,
                        }
                    }
                })?
        };
        active.push(ActiveHome {
            register: Some(register.virtual_register),
            class: register.class,
            end,
            view,
        });
        if reload_inserted && end > first.point {
            coexisting.push(ReloadCoexistingHome {
                virtual_register: register.virtual_register,
                class: register.class,
                view,
            });
        }
    }
    if !reload_inserted {
        chosen_reload = Some(insert_reload(
            function,
            action,
            first.point,
            exclusive_end,
            &candidates,
            &mut active,
            &mut coexisting,
            physical,
        )?);
    }
    coexisting.sort();
    coexisting.dedup();
    Ok(ReloadValueHomeAssignment {
        result: action.reload.result,
        block: first.block,
        start: first.point,
        exclusive_end,
        class: action.reload.destination_class,
        candidates,
        view: chosen_reload.expect("reload insertion always chooses a home"),
        coexisting_homes: coexisting,
    })
}

#[allow(clippy::too_many_arguments)]
fn insert_reload(
    function: usize,
    action: &AbstractSpillInsertionAction,
    start: LiveRangePoint,
    end: LiveRangePoint,
    candidates: &[RegisterViewId],
    active: &mut Vec<ActiveHome>,
    coexisting: &mut Vec<ReloadCoexistingHome>,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<RegisterViewId, ReloadValueHomeError> {
    active.retain(|home| home.end > start);
    let view = candidates
        .iter()
        .copied()
        .find(|candidate| !blocked_reload(*candidate, active, physical))
        .ok_or(ReloadValueHomeError::ReloadPressure {
            function,
            result: action.reload.result.0,
        })?;
    coexisting.extend(active.iter().filter_map(|home| {
        home.register.map(|virtual_register| ReloadCoexistingHome {
            virtual_register,
            class: home.class,
            view: home.view,
        })
    }));
    active.push(ActiveHome {
        register: None,
        class: action.reload.destination_class,
        end,
        view,
    });
    Ok(view)
}

fn legality_row(
    function: usize,
    legality: &crate::FunctionAllocationLegality,
    register: VirtualRegisterId,
) -> Result<&crate::VirtualRegisterAllocationLegality, ReloadValueHomeError> {
    legality
        .virtual_registers
        .iter()
        .find(|row| row.virtual_register == register)
        .ok_or(ReloadValueHomeError::VirtualRegisterMismatch {
            function,
            register: register.0,
        })
}

fn interval(
    function: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<(LiveRangePoint, LiveRangePoint), ReloadValueHomeError> {
    let first = register
        .points
        .first()
        .ok_or(ReloadValueHomeError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let last = register.points.last().expect("nonempty points established");
    let end = LiveRangePoint(last.point.0.checked_add(1).ok_or(
        ReloadValueHomeError::IntervalOverflow {
            function,
            register: register.virtual_register.0,
        },
    )?);
    Ok((first.point, end))
}

fn common_candidates(
    function: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<Vec<RegisterViewId>, ReloadValueHomeError> {
    let first = register
        .points
        .first()
        .ok_or(ReloadValueHomeError::NoLivePoints {
            function,
            register: register.virtual_register.0,
        })?;
    let mut candidates = first.candidates.iter().copied().collect::<BTreeSet<_>>();
    for point in &register.points[1..] {
        candidates.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if candidates.is_empty() {
        return Err(ReloadValueHomeError::NoCommonCandidate {
            function,
            register: register.virtual_register.0,
        });
    }
    Ok(candidates.into_iter().collect())
}

fn reload_candidates(
    function: usize,
    victim: &crate::VirtualRegisterAllocationLegality,
    start: LiveRangePoint,
    end: LiveRangePoint,
    block: selected_instructions::SelectedBlockId,
) -> Result<Vec<RegisterViewId>, ReloadValueHomeError> {
    let points = victim
        .points
        .iter()
        .filter(|point| point.block == block && start <= point.point && point.point < end)
        .collect::<Vec<_>>();
    let expected = usize::try_from(end.0 - start.0)
        .map_err(|_| ReloadValueHomeError::UnsupportedReloadShape { function })?;
    if points.len() != expected || points.first().map(|point| point.point) != Some(start) {
        return Err(ReloadValueHomeError::UnsupportedReloadShape { function });
    }
    let mut candidates = points[0]
        .candidates
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for point in &points[1..] {
        candidates.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if candidates.is_empty() {
        return Err(ReloadValueHomeError::NoCommonCandidate {
            function,
            register: victim.virtual_register.0,
        });
    }
    Ok(candidates.into_iter().collect())
}

fn blocked_original(
    register: VirtualRegisterId,
    candidate: RegisterViewId,
    active: &[ActiveHome],
    interference: &[VirtualInterference],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    active.iter().any(|home| {
        let conflicts = home
            .register
            .is_none_or(|other| interferes(register, other, interference));
        conflicts && views_overlap(candidate, home.view, physical)
    })
}

fn blocked_reload(
    candidate: RegisterViewId,
    active: &[ActiveHome],
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    active
        .iter()
        .any(|home| views_overlap(candidate, home.view, physical))
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

fn views_overlap(
    left: RegisterViewId,
    right: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    let left = view(left, physical);
    let right = view(right, physical);
    match (left, right) {
        (Some(left), Some(right)) => footprints_overlap(left, right),
        _ => true,
    }
}

fn view(id: RegisterViewId, physical: &ValidatedPhysicalRegisterModel) -> Option<&RegisterView> {
    physical.model().views.iter().find(|view| view.id == id)
}

fn footprints_overlap(left: &RegisterView, right: &RegisterView) -> bool {
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right.units.contains(unit) || right.write_units.contains(unit))
}

pub(super) fn work_usage(
    functions: &[FunctionReloadValueHomes],
) -> Result<OptimizationWorkUsage, ReloadValueHomeError> {
    let function_count = count(functions.len())?;
    let assignment_count = count(
        functions
            .iter()
            .filter(|function| function.assignment.is_some())
            .count(),
    )?;
    let candidate_count = functions
        .iter()
        .filter_map(|function| function.assignment.as_ref())
        .try_fold(0_u64, |total, assignment| {
            total
                .checked_add(count(assignment.candidates.len())?)
                .ok_or(ReloadValueHomeError::WorkOverflow)
        })?;
    let coexist_count = functions
        .iter()
        .filter_map(|function| function.assignment.as_ref())
        .try_fold(0_u64, |total, assignment| {
            total
                .checked_add(count(assignment.coexisting_homes.len())?)
                .ok_or(ReloadValueHomeError::WorkOverflow)
        })?;
    let validation_steps = candidate_count
        .checked_add(coexist_count)
        .and_then(|value| value.checked_add(assignment_count.checked_mul(4)?))
        .ok_or(ReloadValueHomeError::WorkOverflow)?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: function_count,
        candidates: candidate_count,
        validation_steps,
        commits: assignment_count,
        iterations: function_count,
    })
}

fn count(value: usize) -> Result<u64, ReloadValueHomeError> {
    u64::try_from(value).map_err(|_| ReloadValueHomeError::WorkOverflow)
}
