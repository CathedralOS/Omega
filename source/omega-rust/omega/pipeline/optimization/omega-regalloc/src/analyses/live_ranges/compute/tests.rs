//! Focused live-range computation fixtures.

use omega_register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId};
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use psi_core::{BlockId, MachineId};

use super::{
    block_domain, build_unit, compute_structural_function, derive_early_clobbers,
    derive_tied_pairs, fragments_overlap, virtual_fragments,
};
use crate::{
    BlockLiveness, FunctionLiveness, InstructionLiveness, LiveRangeFragment, LiveRangePoint,
    LivenessPosition, OperandPosition,
};

fn instruction(
    position: u32,
    uses: &[u32],
    defs: &[u32],
    live_in: &[u32],
    live_out: &[u32],
) -> InstructionLiveness {
    InstructionLiveness {
        position: LivenessPosition(position),
        instruction: SelectedInstructionId(position),
        virtual_uses: uses.iter().copied().map(VirtualRegisterId).collect(),
        virtual_defs: defs.iter().copied().map(VirtualRegisterId).collect(),
        virtual_live_in: live_in.iter().copied().map(VirtualRegisterId).collect(),
        virtual_live_out: live_out.iter().copied().map(VirtualRegisterId).collect(),
        unit_uses: Vec::new(),
        unit_defs: Vec::new(),
        unit_clobbers: Vec::new(),
        unit_live_in: Vec::new(),
        unit_live_out: Vec::new(),
    }
}

fn block(id: u32, instructions: Vec<InstructionLiveness>) -> BlockLiveness {
    BlockLiveness {
        block: SelectedBlockId(id),
        source_block: BlockId::new(u64::from(id) + 1).unwrap(),
        virtual_live_in: Vec::new(),
        virtual_live_out: Vec::new(),
        unit_live_in: Vec::new(),
        unit_live_out: Vec::new(),
        instructions,
        successors: Vec::new(),
    }
}

#[test]
fn structural_unit_ranges_retain_architecture_without_inventing_virtuals() {
    let mut call = instruction(0, &[], &[], &[], &[]);
    call.unit_uses = vec![RegisterUnitId(1)];
    call.unit_defs = vec![RegisterUnitId(2)];
    call.unit_clobbers = vec![RegisterUnitId(3)];
    call.unit_live_in = vec![RegisterUnitId(1)];
    call.unit_live_out = vec![RegisterUnitId(2)];
    let mut returned = instruction(1, &[], &[], &[], &[]);
    returned.unit_uses = vec![RegisterUnitId(2)];
    returned.unit_live_in = vec![RegisterUnitId(2)];
    let live = FunctionLiveness {
        machine: MachineId::new(9).unwrap(),
        entry_definitions: Vec::new(),
        operand_positions: Vec::new(),
        blocks: vec![block(0, vec![call, returned])],
    };
    let ranges = compute_structural_function(0, live.machine, &live).unwrap();
    assert_eq!(ranges.machine, live.machine);
    assert!(ranges.virtual_registers.is_empty());
    assert!(ranges.tied_pairs.is_empty());
    assert!(ranges.early_clobbers.is_empty());
    assert!(ranges.interference.is_empty());
    assert_eq!(ranges.block_domains.len(), 1);
    assert_eq!(ranges.architectural_units.len(), 3);
    assert_eq!(ranges.architectural_units[0].actions.len(), 1);
    assert_eq!(ranges.architectural_units[1].actions.len(), 2);
    assert_eq!(ranges.architectural_units[2].actions.len(), 1);
}

#[test]
fn distinct_use_def_tie_has_exact_before_and_after_points() {
    let live = FunctionLiveness {
        machine: MachineId::new(1).unwrap(),
        entry_definitions: Vec::new(),
        operand_positions: vec![
            OperandPosition {
                position: LivenessPosition(1),
                instruction: SelectedInstructionId(1),
                operand: 0,
                virtual_register: VirtualRegisterId(0),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            OperandPosition {
                position: LivenessPosition(1),
                instruction: SelectedInstructionId(1),
                operand: 1,
                virtual_register: VirtualRegisterId(1),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: Some(0),
                early_clobber: false,
            },
        ],
        blocks: vec![block(0, vec![instruction(1, &[0], &[1], &[0], &[1])])],
    };
    let ties = derive_tied_pairs(0, &live).unwrap();
    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0].use_point, LiveRangePoint(2));
    assert_eq!(ties[0].def_point, LiveRangePoint(3));
    assert_eq!(ties[0].use_virtual_register, VirtualRegisterId(0));
    assert_eq!(ties[0].def_virtual_register, VirtualRegisterId(1));

    let mut chained = live;
    chained.operand_positions.extend([
        OperandPosition {
            position: LivenessPosition(2),
            instruction: SelectedInstructionId(2),
            operand: 0,
            virtual_register: VirtualRegisterId(1),
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        },
        OperandPosition {
            position: LivenessPosition(2),
            instruction: SelectedInstructionId(2),
            operand: 1,
            virtual_register: VirtualRegisterId(2),
            access: RegisterOperandAccess::Def,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: Some(0),
            early_clobber: false,
        },
    ]);
    chained.blocks[0]
        .instructions
        .push(instruction(2, &[1], &[2], &[1], &[2]));
    let ties = derive_tied_pairs(0, &chained).unwrap();
    assert_eq!(ties.len(), 2);
    assert_eq!(ties[1].use_virtual_register, VirtualRegisterId(1));
    assert_eq!(ties[1].def_virtual_register, VirtualRegisterId(2));
    assert_eq!(ties[1].use_point, LiveRangePoint(4));
    assert_eq!(ties[1].def_point, LiveRangePoint(5));
}

#[test]
fn early_clobber_retains_before_phase_without_extending_definition_liveness() {
    let live = FunctionLiveness {
        machine: MachineId::new(1).unwrap(),
        entry_definitions: Vec::new(),
        operand_positions: vec![
            OperandPosition {
                position: LivenessPosition(1),
                instruction: SelectedInstructionId(1),
                operand: 0,
                virtual_register: VirtualRegisterId(0),
                access: RegisterOperandAccess::Use,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            OperandPosition {
                position: LivenessPosition(1),
                instruction: SelectedInstructionId(1),
                operand: 1,
                virtual_register: VirtualRegisterId(1),
                access: RegisterOperandAccess::Def,
                class: RegisterClassId(0),
                fixed_view: None,
                tied_to: None,
                early_clobber: true,
            },
        ],
        blocks: vec![block(0, vec![instruction(1, &[0], &[1], &[0], &[1])])],
    };
    let rows = derive_early_clobbers(0, &live).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].early_point, LiveRangePoint(2));
    assert_eq!(rows[0].def_point, LiveRangePoint(3));
    assert_eq!(rows[0].uses[0].virtual_register, VirtualRegisterId(0));
    assert_eq!(rows[0].def_virtual_register, VirtualRegisterId(1));
    assert_eq!(
        virtual_fragments(0, &live.blocks[0], VirtualRegisterId(1)).unwrap(),
        vec![LiveRangeFragment {
            block: SelectedBlockId(0),
            start: LiveRangePoint(3),
            end: LiveRangePoint(4),
        }]
    );

    let mut multiple = live;
    multiple.operand_positions.extend([
        OperandPosition {
            position: LivenessPosition(2),
            instruction: SelectedInstructionId(2),
            operand: 0,
            virtual_register: VirtualRegisterId(1),
            access: RegisterOperandAccess::Use,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: false,
        },
        OperandPosition {
            position: LivenessPosition(2),
            instruction: SelectedInstructionId(2),
            operand: 1,
            virtual_register: VirtualRegisterId(2),
            access: RegisterOperandAccess::Def,
            class: RegisterClassId(0),
            fixed_view: None,
            tied_to: None,
            early_clobber: true,
        },
    ]);
    multiple.blocks[0]
        .instructions
        .push(instruction(2, &[1], &[2], &[1], &[2]));
    let rows = derive_early_clobbers(0, &multiple).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].early_point, LiveRangePoint(4));
    assert_eq!(rows[1].def_point, LiveRangePoint(5));
    assert_eq!(rows[1].uses[0].virtual_register, VirtualRegisterId(1));
    assert_eq!(rows[1].def_virtual_register, VirtualRegisterId(2));
}

#[test]
fn isolated_tied_early_clobber_separates_tie_from_unrelated_hazard_uses() {
    let selected =
        crate::analyses::liveness::tests::supported_isolated_tied_early_clobber_function();
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    let ties = derive_tied_pairs(0, &live).unwrap();
    let rows = derive_early_clobbers(0, &live).unwrap();

    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0].use_virtual_register, VirtualRegisterId(0));
    assert_eq!(ties[0].def_virtual_register, VirtualRegisterId(2));
    assert_eq!(ties[0].use_point, LiveRangePoint(0));
    assert_eq!(ties[0].def_point, LiveRangePoint(1));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].early_point, LiveRangePoint(0));
    assert_eq!(rows[0].def_point, LiveRangePoint(1));
    assert_eq!(
        rows[0]
            .uses
            .iter()
            .map(|operand| operand.virtual_register)
            .collect::<Vec<_>>(),
        vec![VirtualRegisterId(1)]
    );
    assert_eq!(
        virtual_fragments(0, &live.blocks[0], VirtualRegisterId(2)).unwrap(),
        vec![LiveRangeFragment {
            block: SelectedBlockId(0),
            start: LiveRangePoint(1),
            end: LiveRangePoint(2),
        }]
    );

    let selected =
        crate::analyses::liveness::tests::supported_multiple_isolated_tied_early_clobber_function();
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    assert_eq!(derive_tied_pairs(0, &live).unwrap().len(), 2);
    assert_eq!(derive_early_clobbers(0, &live).unwrap().len(), 2);
}

#[test]
fn component_tied_early_clobber_keeps_transitive_ties_and_only_unrelated_hazards() {
    let selected =
        crate::analyses::liveness::tests::supported_component_tied_early_clobber_function();
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    let ties = derive_tied_pairs(0, &live).unwrap();
    let rows = derive_early_clobbers(0, &live).unwrap();

    assert_eq!(ties.len(), 2);
    assert_eq!(
        ties.iter()
            .map(|tie| (tie.use_virtual_register, tie.def_virtual_register))
            .collect::<Vec<_>>(),
        vec![
            (VirtualRegisterId(0), VirtualRegisterId(1)),
            (VirtualRegisterId(1), VirtualRegisterId(3)),
        ]
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].def_virtual_register, VirtualRegisterId(3));
    assert_eq!(rows[0].early_point, LiveRangePoint(2));
    assert_eq!(rows[0].def_point, LiveRangePoint(3));
    assert_eq!(
        rows[0]
            .uses
            .iter()
            .map(|used| used.virtual_register)
            .collect::<Vec<_>>(),
        vec![VirtualRegisterId(2)]
    );

    let multiple =
        crate::analyses::liveness::tests::supported_multiple_component_tied_early_clobber_function(
        );
    let live = crate::analyses::liveness::compute::compute_function(0, &multiple).unwrap();
    assert_eq!(derive_tied_pairs(0, &live).unwrap().len(), 4);
    assert_eq!(derive_early_clobbers(0, &live).unwrap().len(), 2);
}

#[test]
fn conditional_fixture_has_exact_block_domains_and_virtual_fragments() {
    let blocks = [
        block(
            0,
            vec![
                instruction(0, &[0], &[], &[0], &[]),
                instruction(1, &[], &[], &[], &[]),
            ],
        ),
        block(
            1,
            vec![
                instruction(2, &[], &[1], &[], &[1]),
                instruction(3, &[1], &[], &[1], &[]),
            ],
        ),
        block(
            2,
            vec![
                instruction(4, &[], &[2], &[], &[2]),
                instruction(5, &[2], &[], &[2], &[]),
            ],
        ),
    ];
    assert_eq!(
        blocks
            .iter()
            .map(|row| {
                let row = block_domain(0, row).unwrap();
                (row.start.0, row.end.0)
            })
            .collect::<Vec<_>>(),
        vec![(0, 4), (4, 8), (8, 12)]
    );
    assert_eq!(
        virtual_fragments(0, &blocks[0], VirtualRegisterId(0)).unwrap(),
        vec![LiveRangeFragment {
            block: SelectedBlockId(0),
            start: LiveRangePoint(0),
            end: LiveRangePoint(1),
        }]
    );
    assert_eq!(
        virtual_fragments(0, &blocks[1], VirtualRegisterId(1)).unwrap(),
        vec![LiveRangeFragment {
            block: SelectedBlockId(1),
            start: LiveRangePoint(5),
            end: LiveRangePoint(7),
        }]
    );
    assert_eq!(
        virtual_fragments(0, &blocks[2], VirtualRegisterId(2)).unwrap(),
        vec![LiveRangeFragment {
            block: SelectedBlockId(2),
            start: LiveRangePoint(9),
            end: LiveRangePoint(11),
        }]
    );
    let v0 = virtual_fragments(0, &blocks[0], VirtualRegisterId(0)).unwrap();
    let v1 = virtual_fragments(0, &blocks[1], VirtualRegisterId(1)).unwrap();
    let v2 = virtual_fragments(0, &blocks[2], VirtualRegisterId(2)).unwrap();
    assert!(!fragments_overlap(&v0, &v1));
    assert!(!fragments_overlap(&v0, &v2));
    assert!(!fragments_overlap(&v1, &v2));
}

#[test]
fn architectural_actions_do_not_turn_dead_machine_writes_into_live_state() {
    let unit = RegisterUnitId(7);
    let mut first = instruction(0, &[], &[], &[], &[]);
    first.unit_uses = vec![unit];
    first.unit_defs = vec![unit];
    first.unit_live_in = vec![unit];
    first.unit_live_out = vec![unit];
    let mut second = instruction(1, &[], &[], &[], &[]);
    second.unit_uses = vec![unit];
    second.unit_defs = vec![unit];
    second.unit_live_in = vec![unit];
    let function = FunctionLiveness {
        machine: MachineId::new(1).unwrap(),
        entry_definitions: Vec::new(),
        operand_positions: Vec::new(),
        blocks: vec![block(0, vec![first, second])],
    };
    let row = build_unit(0, &function, unit).unwrap();
    assert_eq!(row.actions.len(), 4);
    assert_eq!(row.actions[0].point, LiveRangePoint(0));
    assert_eq!(row.actions[1].point, LiveRangePoint(1));
    assert_eq!(row.actions[2].point, LiveRangePoint(2));
    assert_eq!(row.actions[3].point, LiveRangePoint(3));
    assert_eq!(
        row.fragments,
        vec![LiveRangeFragment {
            block: SelectedBlockId(0),
            start: LiveRangePoint(0),
            end: LiveRangePoint(3),
        }]
    );
    assert_eq!(row.actions[3].point, row.fragments[0].end);
}
