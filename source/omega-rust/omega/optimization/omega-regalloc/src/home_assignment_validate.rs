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
    TerminalRegisterHomePlan, TerminalRegisterHomeValidationReceipt, TerminalVirtualRegisterHome,
    ValidatedTerminalAllocationLegality, ValidatedTerminalLiveRanges,
    ValidatedTerminalRegisterHomes, terminal_register_home_identity,
};

#[derive(Debug, Clone)]
struct ReplayActiveHome {
    registers: Vec<TerminalVirtualRegisterId>,
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
        || plan.allocator_availability != legality.receipt().allocator_availability()
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
        allocator_availability: plan.allocator_availability,
        function_count: plan.functions.len(),
        assignment_count: plan
            .functions
            .iter()
            .map(|function| function.assignments.len())
            .sum(),
        tied_pair_count: ranges
            .plan()
            .functions
            .iter()
            .map(|function| function.tied_pairs.len())
            .sum(),
        tied_component_count: ranges
            .plan()
            .functions
            .iter()
            .map(|function| receipt_tied_component_count(&function.tied_pairs))
            .sum(),
        early_clobber_count: ranges
            .plan()
            .functions
            .iter()
            .map(|function| function.early_clobbers.len())
            .sum(),
    };
    Ok(ValidatedTerminalRegisterHomes { plan, receipt })
}

fn receipt_tied_component_count(ties: &[crate::TerminalDistinctUseDefTie]) -> usize {
    let mut components = Vec::<BTreeSet<TerminalVirtualRegisterId>>::new();
    for tie in ties {
        let used = components
            .iter()
            .position(|component| component.contains(&tie.use_virtual_register));
        let defined = components
            .iter()
            .position(|component| component.contains(&tie.def_virtual_register));
        match (used, defined) {
            (None, None) => components.push(BTreeSet::from([
                tie.use_virtual_register,
                tie.def_virtual_register,
            ])),
            (Some(component), None) => {
                components[component].insert(tie.def_virtual_register);
            }
            (None, Some(component)) => {
                components[component].insert(tie.use_virtual_register);
            }
            (Some(left), Some(right)) if left != right => {
                let (keep, remove) = if left < right {
                    (left, right)
                } else {
                    (right, left)
                };
                let removed = components.remove(remove);
                components[keep].extend(removed);
            }
            (Some(_), Some(_)) => {}
        }
    }
    components.len()
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
    let classes = legality
        .virtual_registers
        .iter()
        .map(|register| (register.virtual_register, register.class))
        .collect::<BTreeMap<_, _>>();
    let mut components = legality
        .virtual_registers
        .iter()
        .map(|register| BTreeSet::from([register.virtual_register]))
        .collect::<Vec<_>>();
    for tie in &ranges.tied_pairs {
        if tie.use_virtual_register == tie.def_virtual_register
            || classes.get(&tie.use_virtual_register) != Some(&tie.class)
            || classes.get(&tie.def_virtual_register) != Some(&tie.class)
        {
            return Err(TerminalRegisterHomeError::UnsupportedTiedTopology {
                function: function_index,
                instruction: tie.instruction.0,
            });
        }
        let used = components
            .iter()
            .position(|component| component.contains(&tie.use_virtual_register))
            .expect("validated use register has a singleton component");
        let defined = components
            .iter()
            .position(|component| component.contains(&tie.def_virtual_register))
            .expect("validated definition register has a singleton component");
        if used != defined {
            let (keep, remove) = if used < defined {
                (used, defined)
            } else {
                (defined, used)
            };
            let removed = components.remove(remove);
            components[keep].extend(removed);
        }
    }
    components.sort_by_key(|component| {
        component
            .first()
            .copied()
            .expect("component has at least one member")
    });
    for component in &components {
        let members = component.iter().copied().collect::<Vec<_>>();
        for (left_index, left) in members.iter().enumerate() {
            for right in members.iter().skip(left_index + 1) {
                if ranges.interference.iter().any(|pair| {
                    (pair.lower == *left && pair.higher == *right)
                        || (pair.higher == *left && pair.lower == *right)
                }) {
                    return Err(TerminalRegisterHomeError::TiedRegistersInterfere {
                        function: function_index,
                        lower: left.0.min(right.0),
                        higher: left.0.max(right.0),
                    });
                }
            }
        }
    }
    let mut positions = components
        .into_iter()
        .map(|component| {
            let members = legality
                .virtual_registers
                .iter()
                .enumerate()
                .filter_map(|(position, register)| {
                    component
                        .contains(&register.virtual_register)
                        .then_some(position)
                })
                .collect::<Vec<_>>();
            replay_group(function_index, &members, legality)
        })
        .collect::<Result<Vec<_>, _>>()?;
    positions.sort_by_key(|(members, start, _, _)| {
        (
            *start,
            legality.virtual_registers[members[0]].virtual_register,
        )
    });
    let mut selected = BTreeMap::new();
    let mut active = Vec::<ReplayActiveHome>::new();
    for (members, start, end, candidates) in positions {
        active.retain(|entry| entry.end > start);
        let representative = &legality.virtual_registers[members[0]];
        let mut home = None;
        for candidate in candidates {
            let view = find_view(
                function_index,
                representative.virtual_register,
                representative.class,
                candidate,
                physical,
            )?;
            let blocked = active.iter().any(|entry| {
                members.iter().any(|member| {
                    entry.registers.iter().any(|active_register| {
                        let register = legality.virtual_registers[*member].virtual_register;
                        ranges.interference.iter().any(|pair| {
                            (pair.lower == register && pair.higher == *active_register)
                                || (pair.higher == register && pair.lower == *active_register)
                        })
                    })
                }) && overlaps(
                    view,
                    physical
                        .model()
                        .views
                        .iter()
                        .find(|view| view.id == entry.view)
                        .expect("previously validated home view remains present"),
                )
            }) || replay_early_clobber_blocks(
                &members, view, &selected, legality, ranges, physical,
            );
            if !blocked {
                home = Some(candidate);
                break;
            }
        }
        let home = home.ok_or(TerminalRegisterHomeError::NoCompatibleHome {
            function: function_index,
            register: representative.virtual_register.0,
        })?;
        let registers = members
            .iter()
            .map(|member| legality.virtual_registers[*member].virtual_register)
            .collect::<Vec<_>>();
        for register in &registers {
            selected.insert(*register, home);
        }
        active.push(ReplayActiveHome {
            registers,
            end,
            view: home,
        });
        active.sort_by(|left, right| {
            left.end
                .cmp(&right.end)
                .then(left.registers[0].cmp(&right.registers[0]))
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

type ReplayGroup = (
    Vec<usize>,
    TerminalLiveRangePoint,
    TerminalLiveRangePoint,
    Vec<RegisterViewId>,
);

fn replay_group(
    function: usize,
    members: &[usize],
    legality: &crate::TerminalFunctionAllocationLegality,
) -> Result<ReplayGroup, TerminalRegisterHomeError> {
    if members.is_empty() {
        return Err(TerminalRegisterHomeError::FunctionMismatch { function });
    }
    let mut start = None;
    let mut end = None;
    let mut shared = None::<BTreeSet<RegisterViewId>>;
    for member in members {
        let register = &legality.virtual_registers[*member];
        if !register.entry_transitions.is_empty() {
            return Err(TerminalRegisterHomeError::UnresolvedEntryTransitions {
                function,
                register: register.virtual_register.0,
                count: register.entry_transitions.len(),
            });
        }
        let (lower, upper) = replay_interval_bounds(function, register)?;
        start = Some(start.map_or(lower, |value: TerminalLiveRangePoint| value.min(lower)));
        end = Some(end.map_or(upper, |value: TerminalLiveRangePoint| value.max(upper)));
        let mut candidates = register.points[0]
            .candidates
            .iter()
            .copied()
            .filter(|candidate| {
                register.points[1..]
                    .iter()
                    .all(|point| point.candidates.contains(candidate))
            })
            .collect::<BTreeSet<_>>();
        for point in &register.early_clobber_points {
            candidates.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
        }
        if let Some(existing) = &mut shared {
            existing.retain(|candidate| candidates.contains(candidate));
        } else {
            shared = Some(candidates);
        }
    }
    let shared = shared.expect("replay group is nonempty");
    if shared.is_empty() {
        if members.len() > 1 {
            return Err(TerminalRegisterHomeError::NoCommonTiedComponent {
                function,
                leader: legality.virtual_registers[members[0]].virtual_register.0,
                member_count: members.len(),
            });
        }
        return Err(TerminalRegisterHomeError::NoCommonCandidate {
            function,
            register: legality.virtual_registers[members[0]].virtual_register.0,
        });
    }
    Ok((
        members.to_vec(),
        start.expect("replay group is nonempty"),
        end.expect("replay group is nonempty"),
        shared.into_iter().collect(),
    ))
}

fn replay_early_clobber_blocks(
    members: &[usize],
    candidate: &RegisterView,
    selected: &BTreeMap<TerminalVirtualRegisterId, RegisterViewId>,
    legality: &crate::TerminalFunctionAllocationLegality,
    ranges: &crate::TerminalFunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    for member in members {
        let register = legality.virtual_registers[*member].virtual_register;
        for early in &ranges.early_clobbers {
            if register == early.def_virtual_register {
                for used in &early.uses {
                    if let Some(view) = selected.get(&used.virtual_register) {
                        let used_view = physical
                            .model()
                            .views
                            .iter()
                            .find(|candidate| candidate.id == *view)
                            .expect("selected input home was validated");
                        if candidate
                            .write_units
                            .iter()
                            .any(|unit| used_view.units.contains(unit))
                        {
                            return true;
                        }
                    }
                }
            } else if early
                .uses
                .iter()
                .any(|used| used.virtual_register == register)
                && let Some(view) = selected.get(&early.def_virtual_register)
            {
                let def_view = physical
                    .model()
                    .views
                    .iter()
                    .find(|candidate| candidate.id == *view)
                    .expect("selected definition home was validated");
                if def_view
                    .write_units
                    .iter()
                    .any(|unit| candidate.units.contains(unit))
                {
                    return true;
                }
            }
        }
    }
    false
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
