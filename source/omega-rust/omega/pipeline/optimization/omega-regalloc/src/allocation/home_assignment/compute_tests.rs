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
    legality.virtual_registers[2].early_clobber_points = vec![VirtualEarlyClobberPointLegality {
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
    legality.virtual_registers[2].early_clobber_points = vec![VirtualEarlyClobberPointLegality {
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
    legality.virtual_registers[2].early_clobber_points = vec![VirtualEarlyClobberPointLegality {
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
    legality.virtual_registers[4].early_clobber_points = vec![VirtualEarlyClobberPointLegality {
        block: SelectedBlockId(0),
        position: LivenessPosition(4),
        instruction: SelectedInstructionId(4),
        operand: 1,
        point: LiveRangePoint(8),
        candidates: vec![RegisterViewId(0), RegisterViewId(1)],
    }];
    legality.virtual_registers[5].early_clobber_points = vec![VirtualEarlyClobberPointLegality {
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
