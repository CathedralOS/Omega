use std::collections::{BTreeMap, BTreeSet};

use omega_register_model::{
    RegisterView, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use omega_selected_instructions::VirtualRegisterId;

use crate::{
    FunctionRegisterHomes, LiveRangePoint, RegisterHomeError, RegisterHomePlan,
    ValidatedAllocationLegality, ValidatedLiveRanges, VirtualInterference, VirtualRegisterHome,
};

#[derive(Debug, Clone)]
struct ActiveHome {
    registers: Vec<VirtualRegisterId>,
    end: LiveRangePoint,
    view: RegisterViewId,
}

#[derive(Debug)]
struct AllocationGroup<'a> {
    registers: Vec<&'a crate::VirtualRegisterAllocationLegality>,
    start: LiveRangePoint,
    end: LiveRangePoint,
    candidates: BTreeSet<RegisterViewId>,
}

pub(crate) fn compute_terminal_register_homes(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<RegisterHomePlan, RegisterHomeError> {
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
                return Err(RegisterHomeError::FunctionMismatch { function: index });
            }
            compute_function(index, legality, ranges, physical)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let structural_unit_functions = legality
        .plan()
        .structural_unit_functions
        .iter()
        .zip(&ranges.plan().structural_unit_functions)
        .enumerate()
        .map(|(index, (legality, ranges))| {
            if legality.machine != ranges.machine
                || !legality.virtual_registers.is_empty()
                || !ranges.virtual_registers.is_empty()
            {
                return Err(RegisterHomeError::FunctionMismatch { function: index });
            }
            Ok(FunctionRegisterHomes {
                machine: ranges.machine,
                assignments: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RegisterHomePlan {
        legality: legality.receipt().identity(),
        ranges: ranges.receipt().identity(),
        register_environment,
        allocator_availability: legality.receipt().allocator_availability(),
        functions,
        structural_unit_functions,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_roots(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<(), RegisterHomeError> {
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
        || legality.plan().structural_unit_functions.len()
            != ranges.plan().structural_unit_functions.len()
    {
        return Err(RegisterHomeError::RootMismatch);
    }
    Ok(())
}

pub(crate) fn compute_function(
    function_index: usize,
    legality: &crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<FunctionRegisterHomes, RegisterHomeError> {
    if legality.virtual_registers.len() != ranges.virtual_registers.len() {
        return Err(RegisterHomeError::FunctionMismatch {
            function: function_index,
        });
    }
    let mut groups = tied_components(function_index, legality, ranges)?
        .into_iter()
        .map(|members| build_group(function_index, members))
        .collect::<Result<Vec<_>, _>>()?;
    groups.sort_by_key(|group| (group.start, group.registers[0].virtual_register));
    let mut homes = BTreeMap::<VirtualRegisterId, RegisterViewId>::new();
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
            let conflicts =
                active.iter().any(|entry| {
                    group.registers.iter().any(|register| {
                        entry.registers.iter().any(|active_register| {
                            interferes(
                                register.virtual_register,
                                *active_register,
                                &ranges.interference,
                            )
                        })
                    }) && footprints_overlap(
                        view,
                        &physical.model().views[usize::from(entry.view.0)],
                    )
                }) || early_clobber_blocks(&group.registers, view, &homes, ranges, physical);
            if !conflicts {
                selected = Some(candidate);
                break;
            }
        }
        let selected = selected.ok_or(RegisterHomeError::NoCompatibleHome {
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
        .map(|register| VirtualRegisterHome {
            virtual_register: register.virtual_register,
            class: register.class,
            view: homes[&register.virtual_register],
        })
        .collect();
    Ok(FunctionRegisterHomes {
        machine: legality.machine,
        assignments,
    })
}

fn tied_components<'a>(
    function: usize,
    legality: &'a crate::FunctionAllocationLegality,
    ranges: &crate::FunctionLiveRanges,
) -> Result<Vec<Vec<&'a crate::VirtualRegisterAllocationLegality>>, RegisterHomeError> {
    let positions = legality
        .virtual_registers
        .iter()
        .enumerate()
        .map(|(position, register)| (register.virtual_register, position))
        .collect::<BTreeMap<_, _>>();
    let mut parents = (0..legality.virtual_registers.len()).collect::<Vec<_>>();
    for tie in &ranges.tied_pairs {
        let (Some(&used), Some(&defined)) = (
            positions.get(&tie.use_virtual_register),
            positions.get(&tie.def_virtual_register),
        ) else {
            return Err(RegisterHomeError::UnsupportedTiedTopology {
                function,
                instruction: tie.instruction.0,
            });
        };
        if used == defined
            || legality.virtual_registers[used].class != tie.class
            || legality.virtual_registers[defined].class != tie.class
        {
            return Err(RegisterHomeError::UnsupportedTiedTopology {
                function,
                instruction: tie.instruction.0,
            });
        }
        let used_root = component_root(&parents, used);
        let defined_root = component_root(&parents, defined);
        if used_root != defined_root {
            let (leader, follower) = if used_root < defined_root {
                (used_root, defined_root)
            } else {
                (defined_root, used_root)
            };
            parents[follower] = leader;
        }
    }
    let mut grouped = BTreeMap::<VirtualRegisterId, Vec<_>>::new();
    for (position, register) in legality.virtual_registers.iter().enumerate() {
        let root = component_root(&parents, position);
        let leader = legality.virtual_registers[root].virtual_register;
        grouped.entry(leader).or_default().push(register);
    }
    for members in grouped.values() {
        for (left_index, left) in members.iter().enumerate() {
            for right in members.iter().skip(left_index + 1) {
                if interferes(
                    left.virtual_register,
                    right.virtual_register,
                    &ranges.interference,
                ) {
                    let (lower, higher) =
                        ordered_pair(left.virtual_register, right.virtual_register);
                    return Err(RegisterHomeError::TiedRegistersInterfere {
                        function,
                        lower: lower.0,
                        higher: higher.0,
                    });
                }
            }
        }
    }
    Ok(grouped.into_values().collect())
}

fn component_root(parents: &[usize], mut position: usize) -> usize {
    while parents[position] != position {
        position = parents[position];
    }
    position
}

fn build_group<'a>(
    function_index: usize,
    mut registers: Vec<&'a crate::VirtualRegisterAllocationLegality>,
) -> Result<AllocationGroup<'a>, RegisterHomeError> {
    registers.sort_by_key(|register| register.virtual_register);
    let mut start = None;
    let mut end = None;
    let mut candidates = None::<BTreeSet<RegisterViewId>>;
    for register in &registers {
        if !register.entry_transitions.is_empty() {
            return Err(RegisterHomeError::UnresolvedEntryTransitions {
                function: function_index,
                register: register.virtual_register.0,
                count: register.entry_transitions.len(),
            });
        }
        let (member_start, member_end) = interval_bounds(function_index, register)?;
        start = Some(start.map_or(member_start, |point: LiveRangePoint| {
            point.min(member_start)
        }));
        end = Some(end.map_or(member_end, |point: LiveRangePoint| point.max(member_end)));
        let member_candidates = common_candidates(function_index, register)?;
        if let Some(shared) = &mut candidates {
            shared.retain(|candidate| member_candidates.contains(candidate));
        } else {
            candidates = Some(member_candidates);
        }
    }
    let candidates = candidates.expect("allocation group is nonempty");
    if candidates.is_empty() {
        if registers.len() > 1 {
            return Err(RegisterHomeError::NoCommonTiedComponent {
                function: function_index,
                leader: registers[0].virtual_register.0,
                member_count: registers.len(),
            });
        }
        return Err(RegisterHomeError::NoCommonCandidate {
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
    left: VirtualRegisterId,
    right: VirtualRegisterId,
) -> (VirtualRegisterId, VirtualRegisterId) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

fn interval_bounds(
    function_index: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<(LiveRangePoint, LiveRangePoint), RegisterHomeError> {
    let Some(first) = register.points.first() else {
        return Err(RegisterHomeError::NoLivePoints {
            function: function_index,
            register: register.virtual_register.0,
        });
    };
    let last = register
        .points
        .last()
        .expect("nonempty points established above");
    let end = last.point.0.checked_add(1).map(LiveRangePoint).ok_or(
        RegisterHomeError::IntervalOverflow {
            function: function_index,
            register: register.virtual_register.0,
        },
    )?;
    Ok((first.point, end))
}

fn common_candidates(
    function_index: usize,
    register: &crate::VirtualRegisterAllocationLegality,
) -> Result<BTreeSet<RegisterViewId>, RegisterHomeError> {
    let Some(first) = register.points.first() else {
        return Err(RegisterHomeError::NoLivePoints {
            function: function_index,
            register: register.virtual_register.0,
        });
    };
    let mut common = first.candidates.iter().copied().collect::<BTreeSet<_>>();
    for point in &register.points[1..] {
        common.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    for point in &register.early_clobber_points {
        common.retain(|candidate| point.candidates.binary_search(candidate).is_ok());
    }
    if common.is_empty() {
        return Err(RegisterHomeError::NoCommonCandidate {
            function: function_index,
            register: register.virtual_register.0,
        });
    }
    Ok(common)
}

fn early_clobber_blocks(
    current: &[&crate::VirtualRegisterAllocationLegality],
    current_view: &RegisterView,
    assigned: &BTreeMap<VirtualRegisterId, RegisterViewId>,
    ranges: &crate::FunctionLiveRanges,
    physical: &ValidatedPhysicalRegisterModel,
) -> bool {
    ranges.early_clobbers.iter().any(|early| {
        current.iter().any(|register| {
            if register.virtual_register == early.def_virtual_register {
                early.uses.iter().any(|used| {
                    assigned.get(&used.virtual_register).is_some_and(|view| {
                        let used_view = &physical.model().views[usize::from(view.0)];
                        early_write_overlaps_use(current_view, used_view)
                    })
                })
            } else if early
                .uses
                .iter()
                .any(|used| used.virtual_register == register.virtual_register)
            {
                assigned
                    .get(&early.def_virtual_register)
                    .is_some_and(|view| {
                        let def_view = &physical.model().views[usize::from(view.0)];
                        early_write_overlaps_use(def_view, current_view)
                    })
            } else {
                false
            }
        })
    })
}

fn early_write_overlaps_use(definition: &RegisterView, used: &RegisterView) -> bool {
    definition
        .write_units
        .iter()
        .any(|unit| used.units.contains(unit))
}

fn checked_view(
    function_index: usize,
    register: VirtualRegisterId,
    class: omega_register_model::RegisterClassId,
    candidate: RegisterViewId,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<&RegisterView, RegisterHomeError> {
    physical
        .model()
        .views
        .get(usize::from(candidate.0))
        .filter(|view| view.id == candidate && view.class == class)
        .ok_or(RegisterHomeError::UnknownOrIncompatibleView {
            function: function_index,
            register: register.0,
            view: candidate.0,
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
mod tests {
    use omega_register_model::{
        PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterUnit, RegisterUnitId,
        RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
        validate_physical_register_model,
    };
    use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
    use psi_core::MachineId;

    use super::*;
    use crate::{
        DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse, FunctionAllocationLegality,
        FunctionLiveRanges, LivenessPosition, VirtualEarlyClobberPointLegality, VirtualLiveRange,
        VirtualPointLegality, VirtualRegisterAllocationLegality,
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

    fn legality(points: &[(u32, u32)]) -> FunctionAllocationLegality {
        FunctionAllocationLegality {
            machine: MachineId::new(1).unwrap(),
            virtual_registers: points
                .iter()
                .enumerate()
                .map(
                    |(register, (start, end))| VirtualRegisterAllocationLegality {
                        virtual_register: VirtualRegisterId(register as u32),
                        class: RegisterClassId(0),
                        points: (*start..=*end)
                            .map(|point| VirtualPointLegality {
                                block: SelectedBlockId(0),
                                point: LiveRangePoint(point),
                                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
                            })
                            .collect(),
                        early_clobber_points: Vec::new(),
                        entry_transitions: Vec::new(),
                    },
                )
                .collect(),
        }
    }

    fn ranges(interference: &[(u32, u32)]) -> FunctionLiveRanges {
        FunctionLiveRanges {
            machine: MachineId::new(1).unwrap(),
            block_domains: Vec::new(),
            virtual_registers: (0..3)
                .map(|register| VirtualLiveRange {
                    virtual_register: VirtualRegisterId(register),
                    class: RegisterClassId(0),
                    occurrences: Vec::new(),
                    fixed_constraints: Vec::new(),
                    fragments: Vec::new(),
                    edge_connectors: Vec::new(),
                })
                .collect(),
            tied_pairs: Vec::new(),
            early_clobbers: Vec::new(),
            architectural_units: Vec::new(),
            interference: interference
                .iter()
                .map(|(lower, higher)| VirtualInterference {
                    lower: VirtualRegisterId(*lower),
                    higher: VirtualRegisterId(*higher),
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
            crate::allocation::home_assignment::validate::replay_function(
                0,
                &legality(&[(0, 2), (1, 2), (3, 4)]),
                &ranges(&[(0, 1)]),
                &physical,
            )
            .unwrap(),
            reusable
        );

        let expected_pressure = Err(RegisterHomeError::NoCompatibleHome {
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
            crate::allocation::home_assignment::validate::replay_function(
                0,
                &pressure_legality,
                &pressure_ranges,
                &physical,
            ),
            expected_pressure
        );
    }

    fn tied_ranges(interference: &[(u32, u32)]) -> FunctionLiveRanges {
        let mut ranges = ranges(interference);
        ranges.tied_pairs.push(DistinctUseDefTie {
            block: SelectedBlockId(0),
            position: LivenessPosition(1),
            instruction: omega_selected_instructions::SelectedInstructionId(1),
            use_operand: 0,
            use_virtual_register: VirtualRegisterId(0),
            use_point: LiveRangePoint(2),
            def_operand: 1,
            def_virtual_register: VirtualRegisterId(1),
            def_point: LiveRangePoint(3),
            class: RegisterClassId(0),
        });
        ranges
    }

    fn tied_component_ranges(interference: &[(u32, u32)]) -> FunctionLiveRanges {
        let mut ranges = tied_ranges(interference);
        ranges.tied_pairs.push(DistinctUseDefTie {
            block: SelectedBlockId(0),
            position: LivenessPosition(2),
            instruction: SelectedInstructionId(2),
            use_operand: 0,
            use_virtual_register: VirtualRegisterId(1),
            use_point: LiveRangePoint(4),
            def_operand: 1,
            def_virtual_register: VirtualRegisterId(2),
            def_point: LiveRangePoint(5),
            class: RegisterClassId(0),
        });
        ranges
    }

    fn early_clobber_ranges() -> FunctionLiveRanges {
        let mut ranges = ranges(&[]);
        ranges.early_clobbers.push(EarlyClobberConstraint {
            block: SelectedBlockId(0),
            position: LivenessPosition(1),
            instruction: SelectedInstructionId(1),
            early_point: LiveRangePoint(2),
            def_operand: 2,
            def_virtual_register: VirtualRegisterId(2),
            def_class: RegisterClassId(0),
            def_point: LiveRangePoint(3),
            uses: vec![
                EarlyClobberUse {
                    operand: 0,
                    virtual_register: VirtualRegisterId(0),
                    class: RegisterClassId(0),
                },
                EarlyClobberUse {
                    operand: 1,
                    virtual_register: VirtualRegisterId(1),
                    class: RegisterClassId(0),
                },
            ],
        });
        ranges
    }

    #[test]
    fn early_clobber_def_avoids_expired_input_homes_and_replay_agrees() {
        let physical = physical();
        let mut legality = legality(&[(0, 2), (0, 2), (3, 4)]);
        legality.virtual_registers[2].early_clobber_points =
            vec![VirtualEarlyClobberPointLegality {
                block: SelectedBlockId(0),
                position: LivenessPosition(1),
                instruction: SelectedInstructionId(1),
                operand: 2,
                point: LiveRangePoint(2),
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
            }];
        let ranges = early_clobber_ranges();
        let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
        assert_eq!(
            homes
                .assignments
                .iter()
                .map(|assignment| assignment.view)
                .collect::<Vec<_>>(),
            vec![RegisterViewId(0), RegisterViewId(0), RegisterViewId(1)]
        );
        assert_eq!(
            crate::allocation::home_assignment::validate::replay_function(
                0, &legality, &ranges, &physical
            )
            .unwrap(),
            homes
        );

        for register in &mut legality.virtual_registers {
            for point in &mut register.points {
                point.candidates = vec![RegisterViewId(0)];
            }
            for point in &mut register.early_clobber_points {
                point.candidates = vec![RegisterViewId(0)];
            }
        }
        let expected = Err(RegisterHomeError::NoCompatibleHome {
            function: 0,
            register: 2,
        });
        assert_eq!(compute_function(0, &legality, &ranges, &physical), expected);
        assert_eq!(
            crate::allocation::home_assignment::validate::replay_function(
                0, &legality, &ranges, &physical
            ),
            expected
        );
    }

    #[test]
    fn isolated_tied_early_def_shares_source_home_and_avoids_unrelated_use() {
        let physical = physical();
        let mut legality = legality(&[(0, 0), (0, 0), (1, 1)]);
        legality.virtual_registers[2].early_clobber_points =
            vec![VirtualEarlyClobberPointLegality {
                block: SelectedBlockId(0),
                position: LivenessPosition(0),
                instruction: SelectedInstructionId(0),
                operand: 2,
                point: LiveRangePoint(0),
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
            }];
        let mut ranges = ranges(&[(0, 1)]);
        ranges.tied_pairs.push(DistinctUseDefTie {
            block: SelectedBlockId(0),
            position: LivenessPosition(0),
            instruction: SelectedInstructionId(0),
            use_operand: 0,
            use_virtual_register: VirtualRegisterId(0),
            use_point: LiveRangePoint(0),
            def_operand: 2,
            def_virtual_register: VirtualRegisterId(2),
            def_point: LiveRangePoint(1),
            class: RegisterClassId(0),
        });
        ranges.early_clobbers.push(EarlyClobberConstraint {
            block: SelectedBlockId(0),
            position: LivenessPosition(0),
            instruction: SelectedInstructionId(0),
            early_point: LiveRangePoint(0),
            def_operand: 2,
            def_virtual_register: VirtualRegisterId(2),
            def_class: RegisterClassId(0),
            def_point: LiveRangePoint(1),
            uses: vec![EarlyClobberUse {
                operand: 1,
                virtual_register: VirtualRegisterId(1),
                class: RegisterClassId(0),
            }],
        });

        let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
        assert_eq!(homes.assignments[0].view, homes.assignments[2].view);
        assert_ne!(homes.assignments[1].view, homes.assignments[2].view);
        assert_eq!(
            crate::allocation::home_assignment::validate::replay_function(
                0, &legality, &ranges, &physical
            )
            .unwrap(),
            homes
        );

        for register in &mut legality.virtual_registers {
            for point in &mut register.points {
                point.candidates = vec![RegisterViewId(0)];
            }
            for point in &mut register.early_clobber_points {
                point.candidates = vec![RegisterViewId(0)];
            }
        }
        let expected = Err(RegisterHomeError::NoCompatibleHome {
            function: 0,
            register: 1,
        });
        assert_eq!(compute_function(0, &legality, &ranges, &physical), expected);
        assert_eq!(
            crate::allocation::home_assignment::validate::replay_function(
                0, &legality, &ranges, &physical
            ),
            expected
        );
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
            crate::allocation::home_assignment::validate::replay_function(
                0, &legality, &ranges, &physical
            )
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
            Err(RegisterHomeError::NoCommonTiedComponent { .. })
        ));
        assert!(matches!(
            compute_function(0, &legality, &tied_ranges(&[(0, 1)]), &physical),
            Err(RegisterHomeError::TiedRegistersInterfere { .. })
        ));
    }

    #[test]
    fn transitive_tied_component_gets_one_home_and_checks_all_member_pairs() {
        let physical = physical();
        let legality = legality(&[(1, 2), (3, 4), (5, 6)]);
        let ranges = tied_component_ranges(&[]);
        let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
        assert_eq!(
            homes
                .assignments
                .iter()
                .map(|assignment| assignment.view)
                .collect::<Vec<_>>(),
            vec![RegisterViewId(0), RegisterViewId(0), RegisterViewId(0)]
        );
        assert_eq!(
            crate::allocation::home_assignment::validate::replay_function(
                0, &legality, &ranges, &physical
            )
            .unwrap(),
            homes
        );

        let interfering = tied_component_ranges(&[(0, 2)]);
        let expected = Err(RegisterHomeError::TiedRegistersInterfere {
            function: 0,
            lower: 0,
            higher: 2,
        });
        assert_eq!(
            compute_function(0, &legality, &interfering, &physical),
            expected
        );
        assert_eq!(
            crate::allocation::home_assignment::validate::replay_function(
                0,
                &legality,
                &interfering,
                &physical
            ),
            expected
        );

        let mut disjoint = legality;
        for point in &mut disjoint.virtual_registers[0].points {
            point.candidates = vec![RegisterViewId(0)];
        }
        for point in &mut disjoint.virtual_registers[2].points {
            point.candidates = vec![RegisterViewId(1)];
        }
        assert!(matches!(
            compute_function(0, &disjoint, &ranges, &physical),
            Err(RegisterHomeError::NoCommonTiedComponent {
                leader: 0,
                member_count: 3,
                ..
            })
        ));
        assert!(matches!(
            crate::allocation::home_assignment::validate::replay_function(
                0, &disjoint, &ranges, &physical
            ),
            Err(RegisterHomeError::NoCommonTiedComponent {
                leader: 0,
                member_count: 3,
                ..
            })
        ));
    }

    #[test]
    fn early_def_in_transitive_tied_component_shares_home_and_avoids_unrelated_use() {
        let physical = physical();
        let mut legality = legality(&[(0, 0), (1, 4), (5, 5), (4, 4)]);
        legality.virtual_registers[2].early_clobber_points =
            vec![VirtualEarlyClobberPointLegality {
                block: SelectedBlockId(0),
                position: LivenessPosition(2),
                instruction: SelectedInstructionId(2),
                operand: 2,
                point: LiveRangePoint(4),
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
            }];
        let mut ranges = tied_component_ranges(&[(1, 3)]);
        ranges.tied_pairs[1].def_operand = 2;
        ranges.virtual_registers.push(VirtualLiveRange {
            virtual_register: VirtualRegisterId(3),
            class: RegisterClassId(0),
            occurrences: Vec::new(),
            fixed_constraints: Vec::new(),
            fragments: Vec::new(),
            edge_connectors: Vec::new(),
        });
        ranges.early_clobbers.push(EarlyClobberConstraint {
            block: SelectedBlockId(0),
            position: LivenessPosition(2),
            instruction: SelectedInstructionId(2),
            early_point: LiveRangePoint(4),
            def_operand: 2,
            def_virtual_register: VirtualRegisterId(2),
            def_class: RegisterClassId(0),
            def_point: LiveRangePoint(5),
            uses: vec![EarlyClobberUse {
                operand: 1,
                virtual_register: VirtualRegisterId(3),
                class: RegisterClassId(0),
            }],
        });

        let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
        assert_eq!(homes.assignments[0].view, homes.assignments[1].view);
        assert_eq!(homes.assignments[1].view, homes.assignments[2].view);
        assert_ne!(homes.assignments[2].view, homes.assignments[3].view);
        assert_eq!(
            crate::allocation::home_assignment::validate::replay_function(
                0, &legality, &ranges, &physical
            )
            .unwrap(),
            homes
        );

        for register in &mut legality.virtual_registers {
            for point in &mut register.points {
                point.candidates = vec![RegisterViewId(0)];
            }
            for point in &mut register.early_clobber_points {
                point.candidates = vec![RegisterViewId(0)];
            }
        }
        let expected = Err(RegisterHomeError::NoCompatibleHome {
            function: 0,
            register: 3,
        });
        assert_eq!(compute_function(0, &legality, &ranges, &physical), expected);
        assert_eq!(
            crate::allocation::home_assignment::validate::replay_function(
                0, &legality, &ranges, &physical
            ),
            expected
        );
    }

    #[test]
    fn tied_component_coexists_with_multiple_early_clobber_rows() {
        let physical = physical();
        let mut legality = legality(&[(0, 1), (2, 3), (4, 5), (6, 8), (9, 10), (11, 12)]);
        legality.virtual_registers[4].early_clobber_points =
            vec![VirtualEarlyClobberPointLegality {
                block: SelectedBlockId(0),
                position: LivenessPosition(4),
                instruction: SelectedInstructionId(4),
                operand: 1,
                point: LiveRangePoint(8),
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
            }];
        legality.virtual_registers[5].early_clobber_points =
            vec![VirtualEarlyClobberPointLegality {
                block: SelectedBlockId(0),
                position: LivenessPosition(5),
                instruction: SelectedInstructionId(5),
                operand: 1,
                point: LiveRangePoint(10),
                candidates: vec![RegisterViewId(0), RegisterViewId(1)],
            }];

        let mut ranges = tied_component_ranges(&[]);
        ranges
            .virtual_registers
            .extend((3..=5).map(|register| VirtualLiveRange {
                virtual_register: VirtualRegisterId(register),
                class: RegisterClassId(0),
                occurrences: Vec::new(),
                fixed_constraints: Vec::new(),
                fragments: Vec::new(),
                edge_connectors: Vec::new(),
            }));
        ranges.early_clobbers.push(EarlyClobberConstraint {
            block: SelectedBlockId(0),
            position: LivenessPosition(4),
            instruction: SelectedInstructionId(4),
            early_point: LiveRangePoint(8),
            def_operand: 1,
            def_virtual_register: VirtualRegisterId(4),
            def_class: RegisterClassId(0),
            def_point: LiveRangePoint(9),
            uses: vec![EarlyClobberUse {
                operand: 0,
                virtual_register: VirtualRegisterId(3),
                class: RegisterClassId(0),
            }],
        });
        ranges.early_clobbers.push(EarlyClobberConstraint {
            block: SelectedBlockId(0),
            position: LivenessPosition(5),
            instruction: SelectedInstructionId(5),
            early_point: LiveRangePoint(10),
            def_operand: 1,
            def_virtual_register: VirtualRegisterId(5),
            def_class: RegisterClassId(0),
            def_point: LiveRangePoint(11),
            uses: vec![EarlyClobberUse {
                operand: 0,
                virtual_register: VirtualRegisterId(4),
                class: RegisterClassId(0),
            }],
        });

        let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
        assert_eq!(
            homes
                .assignments
                .iter()
                .map(|assignment| assignment.view)
                .collect::<Vec<_>>(),
            vec![
                RegisterViewId(0),
                RegisterViewId(0),
                RegisterViewId(0),
                RegisterViewId(0),
                RegisterViewId(1),
                RegisterViewId(0),
            ]
        );
        assert_eq!(
            crate::allocation::home_assignment::validate::replay_function(
                0, &legality, &ranges, &physical
            )
            .unwrap(),
            homes
        );
    }
}
