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

#[derive(Debug, Clone)]
struct ActiveHome {
    registers: Vec<TerminalVirtualRegisterId>,
    end: TerminalLiveRangePoint,
    view: RegisterViewId,
}

#[derive(Debug)]
struct AllocationGroup<'a> {
    registers: Vec<&'a crate::TerminalVirtualRegisterAllocationLegality>,
    start: TerminalLiveRangePoint,
    end: TerminalLiveRangePoint,
    candidates: BTreeSet<RegisterViewId>,
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
    let mut tie_participants = BTreeSet::new();
    for tie in &ranges.tied_pairs {
        if tie.use_virtual_register == tie.def_virtual_register
            || !tie_participants.insert(tie.use_virtual_register)
            || !tie_participants.insert(tie.def_virtual_register)
        {
            return Err(TerminalRegisterHomeError::UnsupportedTiedTopology {
                function: function_index,
                instruction: tie.instruction.0,
            });
        }
        if interferes(
            tie.use_virtual_register,
            tie.def_virtual_register,
            &ranges.interference,
        ) {
            let (lower, higher) = ordered_pair(tie.use_virtual_register, tie.def_virtual_register);
            return Err(TerminalRegisterHomeError::TiedRegistersInterfere {
                function: function_index,
                lower: lower.0,
                higher: higher.0,
            });
        }
    }
    let mut grouped = BTreeSet::new();
    let mut groups = Vec::new();
    for register in &legality.virtual_registers {
        if grouped.contains(&register.virtual_register) {
            continue;
        }
        let tie = ranges.tied_pairs.iter().find(|tie| {
            tie.use_virtual_register == register.virtual_register
                || tie.def_virtual_register == register.virtual_register
        });
        let members = if let Some(tie) = tie {
            if !grouped.insert(tie.use_virtual_register)
                || !grouped.insert(tie.def_virtual_register)
            {
                return Err(TerminalRegisterHomeError::UnsupportedTiedTopology {
                    function: function_index,
                    instruction: tie.instruction.0,
                });
            }
            let use_register = legality
                .virtual_registers
                .iter()
                .find(|row| row.virtual_register == tie.use_virtual_register);
            let def_register = legality
                .virtual_registers
                .iter()
                .find(|row| row.virtual_register == tie.def_virtual_register);
            let (Some(use_register), Some(def_register)) = (use_register, def_register) else {
                return Err(TerminalRegisterHomeError::UnsupportedTiedTopology {
                    function: function_index,
                    instruction: tie.instruction.0,
                });
            };
            if use_register.class != tie.class || def_register.class != tie.class {
                return Err(TerminalRegisterHomeError::UnsupportedTiedTopology {
                    function: function_index,
                    instruction: tie.instruction.0,
                });
            }
            vec![use_register, def_register]
        } else {
            grouped.insert(register.virtual_register);
            vec![register]
        };
        groups.push(build_group(function_index, members)?);
    }
    if grouped.len() != legality.virtual_registers.len() {
        return Err(TerminalRegisterHomeError::FunctionMismatch {
            function: function_index,
        });
    }
    groups.sort_by_key(|group| (group.start, group.registers[0].virtual_register));
    let mut homes = BTreeMap::<TerminalVirtualRegisterId, RegisterViewId>::new();
    let mut active = Vec::<ActiveHome>::new();
    for group in groups {
        active.retain(|entry| entry.end > group.start);
        let mut selected = None;
        for candidate in group.candidates {
            let representative = group.registers[0];
            let view = checked_view(
                function_index,
                representative.virtual_register,
                representative.class,
                candidate,
                physical,
            )?;
            let conflicts = active.iter().any(|entry| {
                group.registers.iter().any(|register| {
                    entry.registers.iter().any(|active_register| {
                        interferes(
                            register.virtual_register,
                            *active_register,
                            &ranges.interference,
                        )
                    })
                }) && footprints_overlap(view, &physical.model().views[usize::from(entry.view.0)])
            });
            if !conflicts {
                selected = Some(candidate);
                break;
            }
        }
        let selected = selected.ok_or(TerminalRegisterHomeError::NoCompatibleHome {
            function: function_index,
            register: group.registers[0].virtual_register.0,
        })?;
        for register in &group.registers {
            homes.insert(register.virtual_register, selected);
        }
        active.push(ActiveHome {
            registers: group
                .registers
                .iter()
                .map(|register| register.virtual_register)
                .collect(),
            end: group.end,
            view: selected,
        });
        active.sort_by_key(|entry| (entry.end, entry.registers[0]));
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

fn build_group<'a>(
    function_index: usize,
    mut registers: Vec<&'a crate::TerminalVirtualRegisterAllocationLegality>,
) -> Result<AllocationGroup<'a>, TerminalRegisterHomeError> {
    registers.sort_by_key(|register| register.virtual_register);
    let mut start = None;
    let mut end = None;
    let mut candidates = None::<BTreeSet<RegisterViewId>>;
    for register in &registers {
        if !register.entry_transitions.is_empty() {
            return Err(TerminalRegisterHomeError::UnresolvedEntryTransitions {
                function: function_index,
                register: register.virtual_register.0,
                count: register.entry_transitions.len(),
            });
        }
        let (member_start, member_end) = interval_bounds(function_index, register)?;
        start = Some(start.map_or(member_start, |point: TerminalLiveRangePoint| {
            point.min(member_start)
        }));
        end = Some(end.map_or(member_end, |point: TerminalLiveRangePoint| {
            point.max(member_end)
        }));
        let member_candidates = common_candidates(function_index, register)?;
        if let Some(shared) = &mut candidates {
            shared.retain(|candidate| member_candidates.contains(candidate));
        } else {
            candidates = Some(member_candidates);
        }
    }
    let candidates = candidates.expect("allocation group is nonempty");
    if candidates.is_empty() {
        if registers.len() == 2 {
            return Err(TerminalRegisterHomeError::NoCommonTiedCandidate {
                function: function_index,
                lower: registers[0].virtual_register.0,
                higher: registers[1].virtual_register.0,
            });
        }
        return Err(TerminalRegisterHomeError::NoCommonCandidate {
            function: function_index,
            register: registers[0].virtual_register.0,
        });
    }
    Ok(AllocationGroup {
        registers,
        start: start.expect("allocation group is nonempty"),
        end: end.expect("allocation group is nonempty"),
        candidates,
    })
}

fn ordered_pair(
    left: TerminalVirtualRegisterId,
    right: TerminalVirtualRegisterId,
) -> (TerminalVirtualRegisterId, TerminalVirtualRegisterId) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
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
        TerminalDistinctUseDefTie, TerminalFunctionAllocationLegality, TerminalFunctionLiveRanges,
        TerminalLivenessPosition, TerminalVirtualLiveRange, TerminalVirtualPointLegality,
        TerminalVirtualRegisterAllocationLegality,
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
            tied_pairs: Vec::new(),
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

    fn tied_ranges(interference: &[(u32, u32)]) -> TerminalFunctionLiveRanges {
        let mut ranges = ranges(interference);
        ranges.tied_pairs.push(TerminalDistinctUseDefTie {
            block: TerminalSelectedBlockId(0),
            position: TerminalLivenessPosition(1),
            instruction: omega_terminal_selected_instructions::TerminalSelectedInstructionId(1),
            use_operand: 0,
            use_virtual_register: TerminalVirtualRegisterId(0),
            use_point: TerminalLiveRangePoint(2),
            def_operand: 1,
            def_virtual_register: TerminalVirtualRegisterId(1),
            def_point: TerminalLiveRangePoint(3),
            class: RegisterClassId(0),
        });
        ranges
    }

    #[test]
    fn distinct_use_def_ties_allocate_as_one_bundle_and_replay_independently() {
        let physical = physical();
        let legality = legality(&[(1, 2), (3, 4), (0, 4)]);
        let ranges = tied_ranges(&[(0, 2), (1, 2)]);
        let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
        assert_eq!(homes.assignments[0].view, RegisterViewId(1));
        assert_eq!(homes.assignments[1].view, RegisterViewId(1));
        assert_eq!(homes.assignments[2].view, RegisterViewId(0));
        assert_eq!(
            crate::home_assignment_validate::replay_function(0, &legality, &ranges, &physical)
                .unwrap(),
            homes
        );

        let mut fixed = legality.clone();
        for point in &mut fixed.virtual_registers[1].points {
            point.candidates = vec![RegisterViewId(1)];
        }
        let fixed_homes = compute_function(0, &fixed, &tied_ranges(&[]), &physical).unwrap();
        assert_eq!(fixed_homes.assignments[0].view, RegisterViewId(1));
        assert_eq!(fixed_homes.assignments[1].view, RegisterViewId(1));

        let mut disjoint = legality.clone();
        for point in &mut disjoint.virtual_registers[0].points {
            point.candidates = vec![RegisterViewId(0)];
        }
        for point in &mut disjoint.virtual_registers[1].points {
            point.candidates = vec![RegisterViewId(1)];
        }
        assert!(matches!(
            compute_function(0, &disjoint, &tied_ranges(&[]), &physical),
            Err(TerminalRegisterHomeError::NoCommonTiedCandidate { .. })
        ));
        assert!(matches!(
            compute_function(0, &legality, &tied_ranges(&[(0, 1)]), &physical),
            Err(TerminalRegisterHomeError::TiedRegistersInterfere { .. })
        ));
    }
}
