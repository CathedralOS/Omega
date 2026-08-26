use std::collections::BTreeMap;

use omega_register_model::{
    RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_terminal_selected_instructions::TerminalVirtualRegisterId;

use crate::{
    TerminalFunctionRegisterHomes, TerminalLiveRangePoint, TerminalRegisterHomeError,
    TerminalRegisterHomePlan, TerminalRegisterHomeValidationReceipt, TerminalVirtualRegisterHome,
    ValidatedTerminalAllocationLegality, ValidatedTerminalLiveRanges,
    ValidatedTerminalRegisterHomes, terminal_register_home_identity,
};

#[derive(Debug, Clone, Copy)]
struct ReplayActiveHome {
    register: TerminalVirtualRegisterId,
    end: TerminalLiveRangePoint,
    view: RegisterViewId,
}

#[allow(clippy::too_many_arguments)]
pub fn validate_terminal_register_homes(
    legality: &ValidatedTerminalAllocationLegality,
    ranges: &ValidatedTerminalLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: TerminalRegisterHomePlan,
) -> Result<ValidatedTerminalRegisterHomes, TerminalRegisterHomeError> {
    if plan.legality != legality.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.register_environment != register_environment
        || legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
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
        || plan.functions.len() != legality.plan().functions.len()
        || plan.functions.len() != ranges.plan().functions.len()
    {
        return Err(TerminalRegisterHomeError::RootMismatch);
    }
    for (function_index, ((actual, legality), ranges)) in plan
        .functions
        .iter()
        .zip(&legality.plan().functions)
        .zip(&ranges.plan().functions)
        .enumerate()
    {
        if actual.machine != legality.machine || actual.machine != ranges.machine {
            return Err(TerminalRegisterHomeError::FunctionMismatch {
                function: function_index,
            });
        }
        validate_assignment_order(function_index, actual)?;
        let expected = replay_function(function_index, legality, ranges, physical)?;
        if actual != &expected {
            let register = actual
                .assignments
                .iter()
                .zip(&expected.assignments)
                .find_map(|(actual, expected)| {
                    (actual != expected).then_some(expected.virtual_register.0)
                })
                .unwrap_or(u32::MAX);
            return Err(TerminalRegisterHomeError::VirtualRegisterMismatch {
                function: function_index,
                register,
            });
        }
    }
    let receipt = TerminalRegisterHomeValidationReceipt {
        identity: terminal_register_home_identity(&plan),
        legality: plan.legality,
        ranges: plan.ranges,
        register_environment: plan.register_environment,
        function_count: plan.functions.len(),
        assignment_count: plan
            .functions
            .iter()
            .map(|function| function.assignments.len())
            .sum(),
    };
    Ok(ValidatedTerminalRegisterHomes { plan, receipt })
}

pub(crate) fn replay_function(
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
    let mut positions = legality
        .virtual_registers
        .iter()
        .enumerate()
        .map(|(position, register)| {
            replay_interval_bounds(function_index, register)
                .map(|(start, end)| (position, start, end))
        })
        .collect::<Result<Vec<_>, _>>()?;
    positions.sort_by_key(|(position, start, _)| {
        (
            start.0,
            legality.virtual_registers[*position].virtual_register,
        )
    });
    let mut selected = BTreeMap::new();
    let mut active = Vec::<ReplayActiveHome>::new();
    for (position, start, end) in positions {
        let register = &legality.virtual_registers[position];
        if !register.entry_transitions.is_empty() {
            return Err(TerminalRegisterHomeError::UnresolvedEntryTransitions {
                function: function_index,
                register: register.virtual_register.0,
                count: register.entry_transitions.len(),
            });
        }
        let first = register
            .points
            .first()
            .expect("interval reconstruction established nonempty points");
        active.retain(|entry| entry.end > start);
        let candidates = first
            .candidates
            .iter()
            .copied()
            .filter(|candidate| {
                register.points[1..]
                    .iter()
                    .all(|point| point.candidates.contains(candidate))
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err(TerminalRegisterHomeError::NoCommonCandidate {
                function: function_index,
                register: register.virtual_register.0,
            });
        }
        let mut home = None;
        for candidate in candidates {
            let view = find_view(
                function_index,
                register.virtual_register,
                register.class,
                candidate,
                physical,
            )?;
            let blocked = active.iter().any(|entry| {
                ranges.interference.iter().any(|pair| {
                    (pair.lower == register.virtual_register && pair.higher == entry.register)
                        || (pair.higher == register.virtual_register
                            && pair.lower == entry.register)
                }) && overlaps(
                    view,
                    physical
                        .model()
                        .views
                        .iter()
                        .find(|view| view.id == entry.view)
                        .expect("previously validated home view remains present"),
                )
            });
            if !blocked {
                home = Some(candidate);
                break;
            }
        }
        let home = home.ok_or(TerminalRegisterHomeError::NoCompatibleHome {
            function: function_index,
            register: register.virtual_register.0,
        })?;
        selected.insert(register.virtual_register, home);
        active.push(ReplayActiveHome {
            register: register.virtual_register,
            end,
            view: home,
        });
        active.sort_by(|left, right| {
            left.end
                .cmp(&right.end)
                .then(left.register.cmp(&right.register))
        });
    }
    Ok(TerminalFunctionRegisterHomes {
        machine: legality.machine,
        assignments: legality
            .virtual_registers
            .iter()
            .map(|register| TerminalVirtualRegisterHome {
                virtual_register: register.virtual_register,
                class: register.class,
                view: selected[&register.virtual_register],
            })
            .collect(),
    })
}

fn replay_interval_bounds(
    function_index: usize,
    register: &crate::TerminalVirtualRegisterAllocationLegality,
) -> Result<(TerminalLiveRangePoint, TerminalLiveRangePoint), TerminalRegisterHomeError> {
    let first = register
        .points
        .first()
        .ok_or(TerminalRegisterHomeError::NoLivePoints {
            function: function_index,
            register: register.virtual_register.0,
        })?;
    let last = register
        .points
        .last()
        .expect("nonempty points established above");
    let exclusive_end =
        last.point
            .0
            .checked_add(1)
            .ok_or(TerminalRegisterHomeError::IntervalOverflow {
                function: function_index,
                register: register.virtual_register.0,
            })?;
    Ok((first.point, TerminalLiveRangePoint(exclusive_end)))
}

fn find_view(
    function_index: usize,
    register: TerminalVirtualRegisterId,
    class: omega_register_model::RegisterClassId,
    view: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<&RegisterView, TerminalRegisterHomeError> {
    physical
        .model()
        .views
        .iter()
        .find(|candidate| candidate.id == view && candidate.class == class)
        .ok_or(TerminalRegisterHomeError::UnknownOrIncompatibleView {
            function: function_index,
            register: register.0,
            view: view.0,
        })
}

fn overlaps(left: &RegisterView, right: &RegisterView) -> bool {
    let right_footprint = right
        .units
        .iter()
        .chain(&right.write_units)
        .copied()
        .collect::<Vec<_>>();
    left.units
        .iter()
        .chain(&left.write_units)
        .any(|unit| right_footprint.contains(unit))
}

fn validate_assignment_order(
    function_index: usize,
    function: &TerminalFunctionRegisterHomes,
) -> Result<(), TerminalRegisterHomeError> {
    if function
        .assignments
        .windows(2)
        .any(|pair| pair[0].virtual_register >= pair[1].virtual_register)
    {
        return Err(TerminalRegisterHomeError::NonCanonicalAssignments {
            function: function_index,
        });
    }
    Ok(())
}
