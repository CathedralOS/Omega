use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::{
    RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_terminal_selected_instructions::TerminalVirtualRegisterId;

use crate::{
    TerminalFunctionRegisterHomes, TerminalLiveRangePoint, TerminalRegisterHomeError,
    TerminalRegisterHomePlan, TerminalVirtualInterference, TerminalVirtualRegisterHome,
    ValidatedTerminalAllocationLegality, ValidatedTerminalLiveRanges,
};

#[derive(Debug, Clone, Copy)]
struct ActiveHome {
    register: TerminalVirtualRegisterId,
    end: TerminalLiveRangePoint,
    view: RegisterViewId,
}

pub(crate) fn compute_terminal_register_homes(
    legality: &ValidatedTerminalAllocationLegality,
    ranges: &ValidatedTerminalLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<TerminalRegisterHomePlan, TerminalRegisterHomeError> {
    validate_roots(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    let functions = legality
        .plan()
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .enumerate()
        .map(|(index, (legality, ranges))| {
            if legality.machine != ranges.machine {
                return Err(TerminalRegisterHomeError::FunctionMismatch { function: index });
            }
            compute_function(index, legality, ranges, physical)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TerminalRegisterHomePlan {
        legality: legality.receipt().identity(),
        ranges: ranges.receipt().identity(),
        register_environment,
        functions,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_roots(
    legality: &ValidatedTerminalAllocationLegality,
    ranges: &ValidatedTerminalLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), TerminalRegisterHomeError> {
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
        return Err(TerminalRegisterHomeError::RootMismatch);
    }
    Ok(())
}

fn compute_function(
    function_index: usize,
    legality: &crate::TerminalFunctionAllocationLegality,
    ranges: &crate::TerminalFunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<TerminalFunctionRegisterHomes, TerminalRegisterHomeError> {
    if legality.virtual_registers.len() != ranges.virtual_registers.len() {
        return Err(TerminalRegisterHomeError::FunctionMismatch {
            function: function_index,
        });
    }
    let mut order = legality
        .virtual_registers
        .iter()
        .map(|register| {
            interval_bounds(function_index, register).map(|(start, end)| (start, end, register))
        })
        .collect::<Result<Vec<_>, _>>()?;
    order.sort_by_key(|(start, _, register)| (start.0, register.virtual_register.0));
    let mut homes = BTreeMap::<TerminalVirtualRegisterId, RegisterViewId>::new();
    let mut active = Vec::<ActiveHome>::new();
    for (start, end, register) in order {
        if !register.entry_transitions.is_empty() {
            return Err(TerminalRegisterHomeError::UnresolvedEntryTransitions {
                function: function_index,
                register: register.virtual_register.0,
                count: register.entry_transitions.len(),
            });
        }
        active.retain(|entry| entry.end > start);
        let candidates = common_candidates(function_index, register)?;
        let mut selected = None;
        for candidate in candidates {
            let view = checked_view(
                function_index,
                register.virtual_register,
                register.class,
                candidate,
                physical,
            )?;
            let conflicts = active.iter().any(|entry| {
                interferes(
                    register.virtual_register,
                    entry.register,
                    &ranges.interference,
                ) && footprints_overlap(view, &physical.model().views[usize::from(entry.view.0)])
            });
            if !conflicts {
                selected = Some(candidate);
                break;
            }
        }
        let selected = selected.ok_or(TerminalRegisterHomeError::NoCompatibleHome {
            function: function_index,
            register: register.virtual_register.0,
        })?;
        homes.insert(register.virtual_register, selected);
        active.push(ActiveHome {
            register: register.virtual_register,
            end,
            view: selected,
        });
        active.sort_by_key(|entry| (entry.end, entry.register));
    }
    let assignments = legality
        .virtual_registers
        .iter()
        .map(|register| TerminalVirtualRegisterHome {
            virtual_register: register.virtual_register,
            class: register.class,
            view: homes[&register.virtual_register],
        })
        .collect();
    Ok(TerminalFunctionRegisterHomes {
        machine: legality.machine,
        assignments,
    })
}

fn interval_bounds(
    function_index: usize,
    register: &crate::TerminalVirtualRegisterAllocationLegality,
) -> Result<(TerminalLiveRangePoint, TerminalLiveRangePoint), TerminalRegisterHomeError> {
    let Some(first) = register.points.first() else {
        return Err(TerminalRegisterHomeError::NoLivePoints {
            function: function_index,
            register: register.virtual_register.0,
        });
    };
    let last = register
        .points
        .last()
        .expect("nonempty points established above");
    let end = last
        .point
        .0
        .checked_add(1)
        .map(TerminalLiveRangePoint)
        .ok_or(TerminalRegisterHomeError::IntervalOverflow {
            function: function_index,
            register: register.virtual_register.0,
        })?;
    Ok((first.point, end))
}

fn common_candidates(
    function_index: usize,
    register: &crate::TerminalVirtualRegisterAllocationLegality,
) -> Result<BTreeSet<RegisterViewId>, TerminalRegisterHomeError> {
    let Some(first) = register.points.first() else {
        return Err(TerminalRegisterHomeError::NoLivePoints {
            function: function_index,
            register: register.virtual_register.0,
        });
    };
    let mut common = first.candidates.iter().copied().collect::<BTreeSet<_>>();
    for point in &register.points[1..] {
        common.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if common.is_empty() {
        return Err(TerminalRegisterHomeError::NoCommonCandidate {
            function: function_index,
            register: register.virtual_register.0,
        });
    }
    Ok(common)
}

fn checked_view(
    function_index: usize,
    register: TerminalVirtualRegisterId,
    class: omega_register_model::RegisterClassId,
    candidate: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<&RegisterView, TerminalRegisterHomeError> {
    physical
        .model()
        .views
        .get(usize::from(candidate.0))
        .filter(|view| view.id == candidate && view.class == class)
        .ok_or(TerminalRegisterHomeError::UnknownOrIncompatibleView {
            function: function_index,
            register: register.0,
            view: candidate.0,
        })
}

fn interferes(
    left: TerminalVirtualRegisterId,
    right: TerminalVirtualRegisterId,
    interference: &[TerminalVirtualInterference],
) -> bool {
    let pair = if left < right {
        TerminalVirtualInterference {
            lower: left,
            higher: right,
        }
    } else {
        TerminalVirtualInterference {
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
