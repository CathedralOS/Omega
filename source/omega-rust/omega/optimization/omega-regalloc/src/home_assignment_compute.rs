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
        allocator_availability: legality.receipt().allocator_availability(),
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

pub(crate) fn compute_function(
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

#[cfg(test)]
mod tests {
    use omega_register_model::{
        PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterUnit, RegisterUnitId,
        RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
        validate_physical_register_model,
    };
    use omega_terminal_selected_instructions::{
        TerminalSelectedBlockId, TerminalVirtualRegisterId,
    };
    use psi_core::MachineId;

    use super::*;
    use crate::{
        TerminalFunctionAllocationLegality, TerminalFunctionLiveRanges, TerminalVirtualLiveRange,
        TerminalVirtualPointLegality, TerminalVirtualRegisterAllocationLegality,
    };

    fn physical() -> ValidatedPhysicalRegisterModel {
        validate_physical_register_model(PhysicalRegisterModel {
            architecture: omega_target::Architecture::X86_64,
            units: (0..2)
                .map(|index| RegisterUnit {
                    id: RegisterUnitId(index),
                    name: format!("r{index}.storage"),
                    bits: 64,
                    kind: RegisterUnitKind::IntegerLane,
                })
                .collect(),
            views: (0..2)
                .map(|index| RegisterView {
                    id: RegisterViewId(index),
                    name: format!("r{index}"),
                    class: RegisterClassId(0),
                    units: vec![RegisterUnitId(index)],
                    write_units: vec![RegisterUnitId(index)],
                    bits: 64,
                    write_semantics: RegisterWriteSemantics::ExactView,
                    allocatable: true,
                })
                .collect(),
            classes: vec![RegisterClass {
                id: RegisterClassId(0),
                name: "integer".into(),
                views: vec![RegisterViewId(0), RegisterViewId(1)],
            }],
            conventions: Vec::new(),
            reservations: Vec::new(),
        })
        .unwrap()
    }

    fn legality(points: &[(u32, u32)]) -> TerminalFunctionAllocationLegality {
        TerminalFunctionAllocationLegality {
            machine: MachineId::new(1).unwrap(),
            virtual_registers: points
                .iter()
                .enumerate()
                .map(
                    |(register, (start, end))| TerminalVirtualRegisterAllocationLegality {
                        virtual_register: TerminalVirtualRegisterId(register as u32),
                        class: RegisterClassId(0),
                        points: (*start..=*end)
                            .map(|point| TerminalVirtualPointLegality {
                                block: TerminalSelectedBlockId(0),
                                point: TerminalLiveRangePoint(point),
                                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                            })
                            .collect(),
                        entry_transitions: Vec::new(),
                    },
                )
                .collect(),
        }
    }

    fn ranges(interference: &[(u32, u32)]) -> TerminalFunctionLiveRanges {
        TerminalFunctionLiveRanges {
            machine: MachineId::new(1).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: (0..3)
                .map(|register| TerminalVirtualLiveRange {
                    virtual_register: TerminalVirtualRegisterId(register),
                    class: RegisterClassId(0),
                    occurrences: Vec::new(),
                    fixed_constraints: Vec::new(),
                    fragments: Vec::new(),
                    edge_connectors: Vec::new(),
                })
                .collect(),
            architectural_units: Vec::new(),
            interference: interference
                .iter()
                .map(|(lower, higher)| TerminalVirtualInterference {
                    lower: TerminalVirtualRegisterId(*lower),
                    higher: TerminalVirtualRegisterId(*higher),
                })
                .collect(),
        }
    }

    #[test]
    fn flexible_competitors_rank_stably_expire_and_fail_at_exact_pressure() {
        let physical = physical();
        let reusable = compute_function(
            0,
            &legality(&[(0, 2), (1, 2), (3, 4)]),
            &ranges(&[(0, 1)]),
            &physical,
        )
        .unwrap();
        assert_eq!(
            reusable
                .assignments
                .iter()
                .map(|assignment| assignment.view)
                .collect::<Vec<_>>(),
            vec![RegisterViewId(0), RegisterViewId(1), RegisterViewId(0)]
        );
        assert_eq!(
            crate::home_assignment_validate::replay_function(
                0,
                &legality(&[(0, 2), (1, 2), (3, 4)]),
                &ranges(&[(0, 1)]),
                &physical,
            )
            .unwrap(),
            reusable
        );

        let expected_pressure = Err(TerminalRegisterHomeError::NoCompatibleHome {
            function: 0,
            register: 2,
        });
        let pressure_legality = legality(&[(0, 3), (1, 3), (2, 3)]);
        let pressure_ranges = ranges(&[(0, 1), (0, 2), (1, 2)]);
        assert_eq!(
            compute_function(0, &pressure_legality, &pressure_ranges, &physical),
            expected_pressure
        );
        assert_eq!(
            crate::home_assignment_validate::replay_function(
                0,
                &pressure_legality,
                &pressure_ranges,
                &physical,
            ),
            expected_pressure
        );
    }
}
