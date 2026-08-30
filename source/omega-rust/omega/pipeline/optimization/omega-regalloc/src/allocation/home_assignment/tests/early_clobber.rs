use omega_register_model::{RegisterClassId, RegisterViewId};
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};

use super::{compute_function, fixtures::*, validate};
use crate::{
    DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse, LiveRangePoint, LivenessPosition,
    RegisterHomeError, VirtualEarlyClobberPointLegality, VirtualLiveRange,
};

#[test]
fn early_clobber_def_avoids_expired_input_homes_and_replay_agrees() {
    let physical = physical();
    let mut legality = legality(&[(0, 2), (0, 2), (3, 4)]);
    legality.virtual_registers[2].early_clobber_points = vec![early_point(1, 1, 2, 2)];
    let ranges = early_clobber_ranges();
    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(
        homes
            .assignments
            .iter()
            .map(|assignment| assignment.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(1), RegisterViewId(1), RegisterViewId(0)]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    for register in 0..legality.virtual_registers.len() {
        set_candidates(&mut legality, register, &[0]);
    }
    let expected = Err(RegisterHomeError::NoCompatibleHome {
        function: 0,
        register: 0,
    });
    assert_eq!(compute_function(0, &legality, &ranges, &physical), expected);
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical),
        expected
    );
}

#[test]
fn isolated_tied_early_def_shares_source_home_and_avoids_unrelated_use() {
    let physical = physical();
    let mut legality = legality(&[(0, 0), (0, 0), (1, 1)]);
    legality.virtual_registers[2].early_clobber_points = vec![early_point(0, 0, 2, 0)];
    let mut ranges = ranges(3, &[(0, 1)]);
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
        uses: vec![early_use(1, 1)],
    });

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(homes.assignments[0].view, homes.assignments[2].view);
    assert_ne!(homes.assignments[1].view, homes.assignments[2].view);
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    for register in 0..legality.virtual_registers.len() {
        set_candidates(&mut legality, register, &[0]);
    }
    let expected = Err(RegisterHomeError::NoCompatibleHome {
        function: 0,
        register: 1,
    });
    assert_eq!(compute_function(0, &legality, &ranges, &physical), expected);
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical),
        expected
    );
}

#[test]
fn early_def_in_transitive_tied_component_shares_home_and_avoids_unrelated_use() {
    let physical = physical();
    let mut legality = legality(&[(0, 0), (1, 4), (5, 5), (4, 4)]);
    legality.virtual_registers[2].early_clobber_points = vec![early_point(2, 2, 2, 4)];
    let mut ranges = tied_component_ranges(&[(1, 3)]);
    ranges.tied_pairs[1].def_operand = 2;
    ranges.virtual_registers.push(virtual_range(3));
    ranges.early_clobbers.push(EarlyClobberConstraint {
        block: SelectedBlockId(0),
        position: LivenessPosition(2),
        instruction: SelectedInstructionId(2),
        early_point: LiveRangePoint(4),
        def_operand: 2,
        def_virtual_register: VirtualRegisterId(2),
        def_class: RegisterClassId(0),
        def_point: LiveRangePoint(5),
        uses: vec![early_use(1, 3)],
    });

    let homes = compute_function(0, &legality, &ranges, &physical).unwrap();
    assert_eq!(homes.assignments[0].view, homes.assignments[1].view);
    assert_eq!(homes.assignments[1].view, homes.assignments[2].view);
    assert_ne!(homes.assignments[2].view, homes.assignments[3].view);
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );

    for register in 0..legality.virtual_registers.len() {
        set_candidates(&mut legality, register, &[0]);
    }
    let expected = Err(RegisterHomeError::NoCompatibleHome {
        function: 0,
        register: 3,
    });
    assert_eq!(compute_function(0, &legality, &ranges, &physical), expected);
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical),
        expected
    );
}

#[test]
fn tied_component_coexists_with_multiple_early_clobber_rows() {
    let physical = physical();
    let mut legality = legality(&[(0, 1), (2, 3), (4, 5), (6, 8), (9, 10), (11, 12)]);
    legality.virtual_registers[4].early_clobber_points = vec![early_point(4, 4, 1, 8)];
    legality.virtual_registers[5].early_clobber_points = vec![early_point(5, 5, 1, 10)];

    let mut ranges = tied_component_ranges(&[]);
    ranges.virtual_registers.extend((3..=5).map(virtual_range));
    ranges.early_clobbers.push(EarlyClobberConstraint {
        block: SelectedBlockId(0),
        position: LivenessPosition(4),
        instruction: SelectedInstructionId(4),
        early_point: LiveRangePoint(8),
        def_operand: 1,
        def_virtual_register: VirtualRegisterId(4),
        def_class: RegisterClassId(0),
        def_point: LiveRangePoint(9),
        uses: vec![early_use(0, 3)],
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
        uses: vec![early_use(0, 4)],
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
            RegisterViewId(1),
            RegisterViewId(0),
            RegisterViewId(1),
        ]
    );
    assert_eq!(
        validate::replay_function(0, &legality, &ranges, &physical).unwrap(),
        homes
    );
}

fn early_point(
    position: u32,
    instruction: u32,
    operand: u16,
    point: u32,
) -> VirtualEarlyClobberPointLegality {
    VirtualEarlyClobberPointLegality {
        block: SelectedBlockId(0),
        position: LivenessPosition(position),
        instruction: SelectedInstructionId(instruction),
        operand,
        point: LiveRangePoint(point),
        candidates: vec![RegisterViewId(0), RegisterViewId(1)],
    }
}

fn early_use(operand: u16, register: u32) -> EarlyClobberUse {
    EarlyClobberUse {
        operand,
        virtual_register: VirtualRegisterId(register),
        class: RegisterClassId(0),
    }
}

fn virtual_range(register: u32) -> VirtualLiveRange {
    VirtualLiveRange {
        virtual_register: VirtualRegisterId(register),
        class: RegisterClassId(0),
        occurrences: Vec::new(),
        fixed_constraints: Vec::new(),
        fragments: Vec::new(),
        edge_connectors: Vec::new(),
    }
}
