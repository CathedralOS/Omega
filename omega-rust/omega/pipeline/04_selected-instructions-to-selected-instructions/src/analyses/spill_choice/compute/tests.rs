//! Focused spill-choice proposal and independent-replay tests.
use super::{
    FunctionSpillChoices, LiveRangePoint, OptimizationWorkUsage, ValidatedPhysicalRegisterModel,
    VirtualInterference, compute_function,
};
use crate::SpillChoiceError;
use crate::analyses::spill_choice::compute::WorkCounter;
use crate::analyses::spill_choice::compute::reject_constraint_topologies;

use register_homes::{
    FunctionAllocationLegality, VirtualPointLegality, VirtualRegisterAllocationLegality,
};
use register_model::{
    PhysicalRegisterModel, RegisterClass, RegisterClassId, RegisterUnit, RegisterUnitId,
    RegisterUnitKind, RegisterView, RegisterViewId, RegisterWriteSemantics,
    validate_physical_register_model,
};
use selected_instructions::{
    EarlyClobberConstraint, EarlyClobberUse, FunctionLiveRanges, LiveRangeFragment,
    LivenessPosition, VirtualLiveRange,
};
use selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use semantic_vocabulary::MachineId;

fn physical() -> ValidatedPhysicalRegisterModel {
    validate_physical_register_model(PhysicalRegisterModel {
        architecture: target::Architecture::X86_64,
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

fn legality(intervals: &[(u32, u32)]) -> FunctionAllocationLegality {
    FunctionAllocationLegality {
        machine: MachineId::new(1).unwrap(),
        virtual_registers: intervals
            .iter()
            .enumerate()
            .map(
                |(index, (start, inclusive_end))| VirtualRegisterAllocationLegality {
                    virtual_register: VirtualRegisterId(index as u32),
                    class: RegisterClassId(0),
                    points: (*start..=*inclusive_end)
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

fn ranges(intervals: &[(u32, u32)]) -> FunctionLiveRanges {
    FunctionLiveRanges {
        machine: MachineId::new(1).unwrap(),
        block_domains: Vec::new(),
        virtual_registers: intervals
            .iter()
            .enumerate()
            .map(|(index, (start, inclusive_end))| VirtualLiveRange {
                virtual_register: VirtualRegisterId(index as u32),
                class: RegisterClassId(0),
                occurrences: Vec::new(),
                fixed_constraints: Vec::new(),
                fragments: vec![LiveRangeFragment {
                    block: SelectedBlockId(0),
                    start: LiveRangePoint(*start),
                    end: LiveRangePoint(inclusive_end + 1),
                }],
                edge_connectors: Vec::new(),
            })
            .collect(),
        edge_transfers: Vec::new(),
        copy_affinities: Vec::new(),
        tied_pairs: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units: Vec::new(),
        interference: vec![(0, 1), (0, 2), (1, 2)]
            .into_iter()
            .map(|(lower, higher)| VirtualInterference {
                lower: VirtualRegisterId(lower),
                higher: VirtualRegisterId(higher),
            })
            .collect(),
    }
}

#[test]
fn spill_choice_admits_untied_early_clobber_but_refuses_its_pressure() {
    // An untied early-clobber topology is admitted: the definition's home
    // must avoid the write-before-read hazard its uses carry, which the
    // directional conflict check models. Pressure under the topology still
    // refuses because victim selection around an early home is not modeled.
    let mut early_legality = legality(&[(0, 1), (0, 1), (2, 2)]);
    early_legality.virtual_registers[2]
        .early_clobber_points
        .push(register_homes::VirtualEarlyClobberPointLegality {
            block: SelectedBlockId(0),
            position: LivenessPosition(0),
            instruction: SelectedInstructionId(1),
            operand: 2,
            point: LiveRangePoint(2),
            candidates: vec![RegisterViewId(0), RegisterViewId(1)],
        });
    let mut early_ranges = ranges(&[(0, 1), (0, 1), (2, 2)]);
    // The uses interfere with each other but not with the definition at
    // fragment resolution; only the early-clobber row forbids sharing.
    early_ranges.interference = vec![VirtualInterference {
        lower: VirtualRegisterId(0),
        higher: VirtualRegisterId(1),
    }];
    early_ranges.early_clobbers.push(EarlyClobberConstraint {
        block: SelectedBlockId(0),
        position: LivenessPosition(0),
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
    assert_eq!(reject_constraint_topologies(0, &early_ranges), Ok(()));

    // Both uses seat first and claim both views; the definition's write at
    // the early point may not clobber either use's storage, so no legal
    // home remains and the bounded policy refuses the pressure.
    let mut work = WorkCounter::default();
    assert_eq!(
        compute_function(0, &early_legality, &early_ranges, &physical(), &mut work),
        Err(SpillChoiceError::UnsupportedEarlyClobber { function: 0 })
    );

    // With one hazard-free view still available the same topology computes
    // cleanly — the definition avoids the conflicting use's home.
    let mut free_view = physical().model().clone();
    free_view.views.push(RegisterView {
        id: RegisterViewId(2),
        name: "r2".into(),
        class: RegisterClassId(0),
        units: vec![RegisterUnitId(2)],
        write_units: vec![RegisterUnitId(2)],
        bits: 64,
        write_semantics: RegisterWriteSemantics::ExactView,
        allocatable: true,
    });
    free_view.units.push(RegisterUnit {
        id: RegisterUnitId(2),
        name: "r2.storage".into(),
        bits: 64,
        kind: RegisterUnitKind::IntegerLane,
    });
    free_view.classes[0].views.push(RegisterViewId(2));
    let free_view = validate_physical_register_model(free_view).unwrap();
    let mut free_legality = legality(&[(0, 1), (0, 1), (2, 2)]);
    for register in &mut free_legality.virtual_registers {
        for point in &mut register.points {
            point.candidates.push(RegisterViewId(2));
        }
    }
    free_legality.virtual_registers[2]
        .early_clobber_points
        .push(register_homes::VirtualEarlyClobberPointLegality {
            block: SelectedBlockId(0),
            position: LivenessPosition(0),
            instruction: SelectedInstructionId(1),
            operand: 2,
            point: LiveRangePoint(2),
            candidates: vec![RegisterViewId(0), RegisterViewId(1), RegisterViewId(2)],
        });
    let mut work = WorkCounter::default();
    let computed =
        compute_function(0, &free_legality, &early_ranges, &free_view, &mut work).unwrap();
    assert!(computed.choice.is_none());
    let replay = crate::analyses::spill_choice::validate::replay_function_for_test(
        0,
        &free_legality,
        &early_ranges,
        &free_view,
    )
    .unwrap();
    assert_eq!((computed, work.usage()), replay);

    let mut tied = ranges(&[(0, 0), (1, 1), (2, 2)]);
    tied.tied_pairs.extend([
        selected_instructions::DistinctUseDefTie {
            block: SelectedBlockId(0),
            position: LivenessPosition(0),
            instruction: SelectedInstructionId(0),
            use_operand: 0,
            use_virtual_register: VirtualRegisterId(0),
            use_point: LiveRangePoint(0),
            def_operand: 1,
            def_virtual_register: VirtualRegisterId(1),
            def_point: LiveRangePoint(1),
            class: RegisterClassId(0),
        },
        selected_instructions::DistinctUseDefTie {
            block: SelectedBlockId(0),
            position: LivenessPosition(1),
            instruction: SelectedInstructionId(1),
            use_operand: 0,
            use_virtual_register: VirtualRegisterId(1),
            use_point: LiveRangePoint(2),
            def_operand: 1,
            def_virtual_register: VirtualRegisterId(2),
            def_point: LiveRangePoint(3),
            class: RegisterClassId(0),
        },
    ]);
    assert_eq!(
        reject_constraint_topologies(0, &tied),
        Err(SpillChoiceError::UnsupportedTiedOperands { function: 0 })
    );

    // A tied component still rejects even when an untied early-clobber row
    // rides beside it.
    let mut composed = ranges(&[(0, 0), (1, 2), (2, 2), (3, 3)]);
    composed.tied_pairs.extend([
        selected_instructions::DistinctUseDefTie {
            block: SelectedBlockId(0),
            position: LivenessPosition(0),
            instruction: SelectedInstructionId(0),
            use_operand: 0,
            use_virtual_register: VirtualRegisterId(0),
            use_point: LiveRangePoint(0),
            def_operand: 1,
            def_virtual_register: VirtualRegisterId(1),
            def_point: LiveRangePoint(1),
            class: RegisterClassId(0),
        },
        selected_instructions::DistinctUseDefTie {
            block: SelectedBlockId(0),
            position: LivenessPosition(1),
            instruction: SelectedInstructionId(1),
            use_operand: 0,
            use_virtual_register: VirtualRegisterId(1),
            use_point: LiveRangePoint(2),
            def_operand: 2,
            def_virtual_register: VirtualRegisterId(3),
            def_point: LiveRangePoint(3),
            class: RegisterClassId(0),
        },
    ]);
    composed.early_clobbers.push(EarlyClobberConstraint {
        block: SelectedBlockId(0),
        position: LivenessPosition(1),
        instruction: SelectedInstructionId(1),
        early_point: LiveRangePoint(2),
        def_operand: 2,
        def_virtual_register: VirtualRegisterId(3),
        def_class: RegisterClassId(0),
        def_point: LiveRangePoint(3),
        uses: vec![EarlyClobberUse {
            operand: 1,
            virtual_register: VirtualRegisterId(2),
            class: RegisterClassId(0),
        }],
    });
    assert_eq!(
        reject_constraint_topologies(0, &composed),
        Err(SpillChoiceError::UnsupportedTiedOperands { function: 0 })
    );
}

fn computed(intervals: &[(u32, u32)]) -> (FunctionSpillChoices, OptimizationWorkUsage) {
    let legality = legality(intervals);
    let ranges = ranges(intervals);
    let physical = physical();
    let mut work = WorkCounter::default();
    let result = compute_function(0, &legality, &ranges, &physical, &mut work).unwrap();
    let replay = crate::analyses::spill_choice::validate::replay_function_for_test(
        0, &legality, &ranges, &physical,
    )
    .unwrap();
    assert_eq!((result.clone(), work.usage()), replay);
    (result, work.usage())
}

#[test]
fn equal_end_pressure_keeps_existing_homes_and_selects_the_incoming_value() {
    let (function, usage) = computed(&[(0, 3), (0, 3), (0, 3)]);
    let choice = function.choice.unwrap();
    assert_eq!(choice.selected_victim, VirtualRegisterId(2));
    assert_eq!(choice.active_residents.len(), 2);
    assert_eq!(choice.contenders.len(), 3);
    assert_eq!(choice.contenders[0].reclaimed_view, Some(RegisterViewId(0)));
    assert_eq!(choice.contenders[1].reclaimed_view, Some(RegisterViewId(1)));
    assert_eq!(choice.contenders[2].reclaimed_view, None);
    assert_eq!(usage.commits, 1);
}

#[test]
fn farther_active_end_wins_only_when_its_eviction_recovers_a_view() {
    let (function, _) = computed(&[(0, 5), (0, 3), (0, 3)]);
    assert_eq!(
        function.choice.unwrap().selected_victim,
        VirtualRegisterId(0)
    );
}

#[test]
fn unsupported_cross_block_pressure_cannot_issue_local_victim_authority() {
    let mut legality = legality(&[(0, 3), (0, 3), (0, 3)]);
    legality.virtual_registers[2].points[3].block = SelectedBlockId(1);
    let ranges = ranges(&[(0, 3), (0, 3), (0, 3)]);
    let mut work = WorkCounter::default();
    assert_eq!(
        compute_function(0, &legality, &ranges, &physical(), &mut work),
        Err(SpillChoiceError::UnsupportedPressureShape {
            function: 0,
            register: 2
        })
    );
}
