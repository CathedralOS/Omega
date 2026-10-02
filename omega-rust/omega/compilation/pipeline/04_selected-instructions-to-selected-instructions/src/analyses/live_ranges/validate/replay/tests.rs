use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};
use semantic_vocabulary::MachineId;

use super::{
    canonical::validate as validate_canonical,
    comparison::require_early_clobber_rows,
    constraints::{
        derive_early_clobbers as independently_derive_early_clobbers,
        derive_ties as independently_derive_ties,
    },
};
use crate::LiveRangeError;
use selected_instructions::{
    ArchitecturalUnitLiveRange, CopyAffinity, FunctionLiveRanges, LiveRangeFragment,
    LiveRangePoint, LivenessPosition, OperandPosition, VirtualInterference, VirtualLiveRange,
};

#[test]
fn edge_transfer_replay_retains_edge_and_both_value_identities() {
    let selected = crate::analyses::liveness::tests::successor_parameter_function();
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    let expected = super::function::replay_function(0, &selected, &live).unwrap();
    assert_eq!(expected.edge_transfers.len(), 1);
    assert_eq!(expected.edge_transfers[0].argument, VirtualRegisterId(0));
    assert_eq!(expected.edge_transfers[0].parameter, VirtualRegisterId(2));
    let mut changed_argument = expected.clone();
    changed_argument.edge_transfers[0].argument = VirtualRegisterId(1);
    assert!(super::comparison::require_function(0, &changed_argument, &expected).is_err());
    let mut changed_edge = expected.clone();
    changed_edge.edge_transfers[0].psi_edge = semantic_vocabulary::EdgeId::new(3).unwrap();
    assert!(super::comparison::require_function(0, &changed_edge, &expected).is_err());
    let mut missing = expected.clone();
    missing.edge_transfers.clear();
    assert!(super::comparison::require_function(0, &missing, &expected).is_err());
}

#[test]
fn membership_replay_preserves_sparse_roster_edges_and_block_boundaries() {
    use selected_instructions::{SelectedTerminator, SelectedValueTransport};

    let mut selected = crate::analyses::liveness::tests::successor_parameter_function();
    let identities = [17, 100_000, u32::MAX].map(VirtualRegisterId);
    for (register, identity) in selected.virtual_registers.iter_mut().zip(identities) {
        register.id = identity;
    }
    let SelectedTerminator::Jump { successor, .. } = &mut selected.blocks[0].terminator else {
        unreachable!()
    };
    successor.bindings[0].transport = SelectedValueTransport::Registers {
        argument: identities[0],
        parameter: identities[2],
    };
    let SelectedTerminator::Return { instruction, .. } = &mut selected.blocks[1].terminator else {
        unreachable!()
    };
    instruction.operands[0].virtual_register = identities[2];
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    let expected = super::function::replay_function(0, &selected, &live).unwrap();
    validate_canonical(0, &expected).unwrap();
    assert_eq!(
        expected.virtual_registers[0].fragments,
        vec![fragment(0, 0, 2)]
    );
    assert_eq!(
        expected.virtual_registers[2].fragments,
        vec![fragment(1, 2, 3)]
    );
    assert!(expected.virtual_registers[1].fragments.is_empty());
    assert!(expected.virtual_registers[1].occurrences.is_empty());
    assert!(expected.virtual_registers[1].edge_connectors.is_empty());
    assert_eq!(
        expected.virtual_registers[0].edge_connectors,
        vec![selected_instructions::LiveRangeEdgeConnector {
            source: SelectedBlockId(0),
            terminator: selected_instructions::SelectedInstructionId(0),
            polarity_ordinal: 0,
            psi_edge: semantic_vocabulary::EdgeId::new(1).unwrap(),
            target: SelectedBlockId(1),
        }]
    );
    assert!(expected.virtual_registers[2].edge_connectors.is_empty());
    assert!(expected.interference.is_empty());
    assert_eq!(expected.edge_transfers[0].argument, identities[0]);
    assert_eq!(expected.edge_transfers[0].parameter, identities[2]);

    // Lookup order is private scratch: it must not silently canonicalize a
    // reordered selected roster and thereby change what admission rejects.
    selected.virtual_registers.reverse();
    let reordered = super::function::replay_function(0, &selected, &live).unwrap();
    assert_eq!(
        reordered.virtual_registers,
        expected
            .virtual_registers
            .iter()
            .rev()
            .cloned()
            .collect::<Vec<_>>()
    );
    assert!(validate_canonical(0, &reordered).is_err());
    let mut changed = expected.clone();
    changed.virtual_registers[0].edge_connectors.clear();
    assert!(super::comparison::require_function(0, &changed, &expected).is_err());
}

fn fragment(block: u32, start: u32, end: u32) -> LiveRangeFragment {
    LiveRangeFragment {
        block: SelectedBlockId(block),
        start: LiveRangePoint(start),
        end: LiveRangePoint(end),
    }
}

#[test]
fn membership_replay_retains_gaps_fixed_sites_occurrence_order_and_interference() {
    use register_model::RegisterViewId;
    use selected_instructions::{
        SelectedInstructionId, SelectedTerminator, VirtualFixedConstraintSite,
    };

    let mut selected =
        crate::analyses::liveness::tests::function_with_operand(RegisterOperandAccess::Def);
    let template = selected.blocks[0].instructions[0].clone();
    selected.virtual_registers =
        crate::analyses::liveness::tests::successor_parameter_function().virtual_registers;
    for (register, identity) in selected
        .virtual_registers
        .iter_mut()
        .zip([17, 31, u32::MAX])
    {
        register.id = VirtualRegisterId(identity);
    }
    selected.virtual_registers[0].entry_fixed_view = Some(RegisterViewId(0));
    selected.virtual_registers[2].entry_fixed_view = Some(RegisterViewId(3));
    selected.blocks[0].instructions = (0..5)
        .map(|position| {
            let mut instruction = template.clone();
            instruction.id = SelectedInstructionId(position);
            instruction.operands[0].virtual_register = VirtualRegisterId(17);
            instruction.operands[0].access = if position == 0 || position == 3 {
                RegisterOperandAccess::Def
            } else {
                RegisterOperandAccess::Use
            };
            if position == 0 {
                instruction.operands[0].fixed_view = Some(RegisterViewId(1));
            }
            if position == 1 {
                instruction.operands[0].fixed_view = Some(RegisterViewId(2));
                let mut repeated = instruction.operands[0];
                repeated.operand = 1;
                repeated.fixed_view = None;
                instruction.operands.push(repeated);
                repeated.operand = 2;
                repeated.virtual_register = VirtualRegisterId(31);
                instruction.operands.push(repeated);
            }
            if position == 2 {
                instruction.operands.clear();
            }
            instruction
        })
        .collect();
    let SelectedTerminator::Return { instruction, .. } = &mut selected.blocks[0].terminator else {
        unreachable!()
    };
    instruction.id = SelectedInstructionId(5);
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    let expected = super::function::replay_function(0, &selected, &live).unwrap();
    validate_canonical(0, &expected).unwrap();
    let range = &expected.virtual_registers[0];
    assert_eq!(range.fragments, vec![fragment(0, 1, 3), fragment(0, 7, 9)]);
    assert_eq!(
        range
            .occurrences
            .iter()
            .map(|row| (row.position.0, row.point.0, row.operand, row.access))
            .collect::<Vec<_>>(),
        vec![
            (0, 1, 0, RegisterOperandAccess::Def),
            (1, 2, 0, RegisterOperandAccess::Use),
            (1, 2, 1, RegisterOperandAccess::Use),
            (3, 7, 0, RegisterOperandAccess::Def),
            (4, 8, 0, RegisterOperandAccess::Use),
        ]
    );
    assert_eq!(range.fixed_constraints.len(), 3);
    assert_eq!(
        range.fixed_constraints[0].site,
        VirtualFixedConstraintSite::Entry
    );
    assert_eq!(
        range
            .fixed_constraints
            .iter()
            .map(|row| row.view)
            .collect::<Vec<_>>(),
        vec![RegisterViewId(0), RegisterViewId(1), RegisterViewId(2)]
    );
    for (fixed, occurrence) in range.fixed_constraints[1..].iter().zip(&range.occurrences) {
        assert_eq!(
            fixed.site,
            VirtualFixedConstraintSite::Operand {
                position: occurrence.position,
                point: occurrence.point,
                instruction: occurrence.instruction,
                operand: occurrence.operand,
                access: occurrence.access,
            }
        );
    }
    assert_eq!(
        expected.virtual_registers[1].fragments,
        vec![fragment(0, 0, 3)]
    );
    assert!(expected.virtual_registers[2].fragments.is_empty());
    assert_eq!(expected.virtual_registers[2].fixed_constraints.len(), 1);
    assert_eq!(
        expected.virtual_registers[2].fixed_constraints[0].site,
        VirtualFixedConstraintSite::Entry
    );
    assert_eq!(
        expected.interference,
        vec![VirtualInterference {
            lower: VirtualRegisterId(17),
            higher: VirtualRegisterId(31),
        }]
    );
    for mutation in 0..5 {
        let mut changed = expected.clone();
        match mutation {
            0 => changed.virtual_registers[0].occurrences.swap(1, 2),
            1 => changed.virtual_registers[0].fixed_constraints.swap(1, 2),
            2 => changed.virtual_registers[0].fragments[0].end = LiveRangePoint(7),
            3 => changed.virtual_registers[2].fixed_constraints.clear(),
            4 => changed.interference.clear(),
            _ => unreachable!(),
        }
        assert!(
            super::comparison::require_function(0, &changed, &expected).is_err(),
            "mutation {mutation}"
        );
    }
}

fn function() -> FunctionLiveRanges {
    FunctionLiveRanges {
        machine: MachineId::new(1).unwrap(),
        block_domains: Vec::new(),
        virtual_registers: vec![VirtualLiveRange {
            virtual_register: VirtualRegisterId(0),
            class: RegisterClassId(0),
            occurrences: Vec::new(),
            fixed_constraints: Vec::new(),
            fragments: vec![LiveRangeFragment {
                block: SelectedBlockId(0),
                start: LiveRangePoint(0),
                end: LiveRangePoint(1),
            }],
            edge_connectors: Vec::new(),
        }],
        edge_transfers: Vec::new(),
        copy_affinities: Vec::new(),
        tied_pairs: Vec::new(),
        early_clobbers: Vec::new(),
        architectural_units: vec![ArchitecturalUnitLiveRange {
            unit: RegisterUnitId(0),
            actions: Vec::new(),
            fragments: Vec::new(),
            edge_connectors: Vec::new(),
        }],
        interference: Vec::new(),
    }
}

#[test]
fn canonical_validation_rejects_nonmaximal_fragments_and_reversed_pairs() {
    let mut adjacent = function();
    adjacent.virtual_registers[0]
        .fragments
        .push(LiveRangeFragment {
            block: SelectedBlockId(0),
            start: LiveRangePoint(1),
            end: LiveRangePoint(2),
        });
    assert!(matches!(
        validate_canonical(0, &adjacent),
        Err(LiveRangeError::NonCanonicalRows { .. })
    ));

    let mut reversed = function();
    reversed.interference.push(VirtualInterference {
        lower: VirtualRegisterId(2),
        higher: VirtualRegisterId(1),
    });
    assert!(matches!(
        validate_canonical(0, &reversed),
        Err(LiveRangeError::NonCanonicalRows { .. })
    ));
}

#[test]
fn copy_affinity_replay_rejects_drifted_missing_and_unordered_rows() {
    let mut expected = function();
    expected.copy_affinities = vec![
        CopyAffinity {
            block: SelectedBlockId(0),
            instruction: selected_instructions::SelectedInstructionId(0),
            source: VirtualRegisterId(0),
            destination: VirtualRegisterId(1),
        },
        CopyAffinity {
            block: SelectedBlockId(0),
            instruction: selected_instructions::SelectedInstructionId(1),
            source: VirtualRegisterId(2),
            destination: VirtualRegisterId(3),
        },
    ];
    let mut changed = expected.clone();
    changed.copy_affinities[0].destination = VirtualRegisterId(9);
    assert!(super::comparison::require_function(0, &changed, &expected).is_err());
    let mut missing = expected.clone();
    missing.copy_affinities.clear();
    assert!(super::comparison::require_function(0, &missing, &expected).is_err());
    let mut reversed = expected.clone();
    reversed.copy_affinities.reverse();
    assert!(matches!(
        validate_canonical(0, &reversed),
        Err(LiveRangeError::NonCanonicalRows { .. })
    ));
}

#[test]
fn parallel_early_definition_replay_requires_each_exact_use_only_hazard() {
    let selected =
        crate::analyses::liveness::tests::supported_parallel_early_definitions_function();
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    let expected = crate::analyses::live_ranges::compute::derive_early_clobbers(0, &live).unwrap();
    assert_eq!(
        independently_derive_early_clobbers(0, &live).unwrap(),
        expected
    );
    assert_eq!(expected.len(), 2);
    assert_eq!(expected[0].instruction, expected[1].instruction);
    for mutation in 0..5 {
        let mut changed = expected.clone();
        match mutation {
            0 => {
                changed.pop();
            }
            1 => changed[1].def_virtual_register = changed[0].def_virtual_register,
            2 => changed[1].uses[0].virtual_register = changed[0].def_virtual_register,
            3 => changed[1].uses.clear(),
            4 => changed.swap(0, 1),
            _ => unreachable!(),
        }
        assert!(require_early_clobber_rows(0, &changed, &expected).is_err());
    }
    for mutation in 0..7 {
        let mut changed = live.clone();
        match mutation {
            0 => changed.operand_positions[2].tied_to = Some(0),
            1 => changed.operand_positions[2].early_clobber = false,
            2 => {
                changed.operand_positions[2].virtual_register =
                    changed.operand_positions[1].virtual_register
            }
            3 => changed.operand_positions[2].operand = 1,
            4 => changed.operand_positions[0].early_clobber = true,
            5 => changed.operand_positions[2].access = RegisterOperandAccess::UseDef,
            6 => {
                let mut source = changed.operand_positions[0];
                source.instruction = selected_instructions::SelectedInstructionId(9);
                source.position = LivenessPosition(9);
                source.virtual_register = VirtualRegisterId(2);
                let mut definition = source;
                definition.operand = 1;
                definition.virtual_register = VirtualRegisterId(3);
                definition.access = RegisterOperandAccess::Def;
                definition.tied_to = Some(0);
                changed.operand_positions.extend([source, definition]);
            }
            _ => unreachable!(),
        }
        assert!(
            independently_derive_early_clobbers(0, &changed).is_err(),
            "replay mutation {mutation}"
        );
        assert!(
            crate::analyses::live_ranges::compute::derive_early_clobbers(0, &changed).is_err(),
            "producer mutation {mutation}"
        );
    }
}

#[test]
fn component_tied_early_clobber_replay_matches_and_rejects_a_second_early_member() {
    let selected =
        crate::analyses::liveness::tests::supported_component_tied_early_clobber_function();
    let live = crate::analyses::liveness::compute::compute_function(0, &selected).unwrap();
    assert_eq!(
        independently_derive_early_clobbers(0, &live).unwrap(),
        crate::analyses::live_ranges::compute::derive_early_clobbers(0, &live).unwrap()
    );
    assert_eq!(
        independently_derive_ties(0, &live).unwrap(),
        crate::analyses::live_ranges::compute::derive_tied_pairs(0, &live).unwrap()
    );

    let mut two_early = live;
    two_early
        .operand_positions
        .iter_mut()
        .find(|operand| operand.virtual_register == VirtualRegisterId(1))
        .unwrap()
        .early_clobber = true;
    two_early.operand_positions.push(OperandPosition {
        position: LivenessPosition(0),
        instruction: selected_instructions::SelectedInstructionId(0),
        operand: 2,
        virtual_register: VirtualRegisterId(4),
        access: RegisterOperandAccess::Use,
        class: RegisterClassId(0),
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    });
    assert!(matches!(
        independently_derive_early_clobbers(0, &two_early),
        Err(LiveRangeError::UnsupportedEarlyClobber { .. })
    ));
    assert!(matches!(
        crate::analyses::live_ranges::compute::derive_early_clobbers(0, &two_early),
        Err(LiveRangeError::UnsupportedEarlyClobber { .. })
    ));
}

#[test]
fn tied_component_receipt_count_uses_transitive_closure() {
    let edge = |use_register, def_register, instruction| selected_instructions::DistinctUseDefTie {
        block: SelectedBlockId(0),
        position: LivenessPosition(instruction),
        instruction: selected_instructions::SelectedInstructionId(instruction),
        use_operand: 0,
        use_virtual_register: VirtualRegisterId(use_register),
        use_point: LiveRangePoint(instruction * 2),
        def_operand: 1,
        def_virtual_register: VirtualRegisterId(def_register),
        def_point: LiveRangePoint(instruction * 2 + 1),
        class: RegisterClassId(0),
    };
    assert_eq!(
        super::super::receipt::tied_component_count(
            &[edge(0, 1, 0), edge(1, 2, 1), edge(3, 4, 2),]
        ),
        2
    );
}
