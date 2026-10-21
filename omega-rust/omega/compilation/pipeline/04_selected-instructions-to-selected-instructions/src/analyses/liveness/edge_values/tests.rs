//! Operation control has physical liveness, but no authored edge transport.
use super::*;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind,
    SelectedInstructionProvenance,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId};

fn function() -> SelectedFunction {
    let source = BlockId::new(1).unwrap();
    let operation = OperationId::new(2).unwrap();
    let returned = EdgeId::new(3).unwrap();
    let instruction = |id, kind| SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: register_model::RegisterConstraintKey {
            family: register_model::RegisterConstraintFamily::Instruction,
            variant: 0,
        },
        operands: Vec::new(),
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance {
            operations: vec![operation],
            ..Default::default()
        },
    };
    let successor = |block| SelectedSuccessor {
        role: SelectedSuccessorRole::OperationContinuation { operation },
        psi_edge: None,
        block: SelectedBlockId(block),
        source_target: source,
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        structural_case: None,
        fuel: Vec::new(),
    };
    SelectedFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        nominal_cleanup_subject: None,
        provenance: target_operations::TerminalPsiProvenance {
            operations: vec![operation],
            edges: vec![returned],
        },
        structural: None,
        local_storage_slots: Vec::new(),
        outgoing_arguments: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: SelectedBlockId(0),
        virtual_registers: Vec::new(),
        blocks: vec![
            SelectedBlock {
                id: SelectedBlockId(0),
                origin: SelectedBlockOrigin::Source(source),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(0, SelectedInstructionKind::Jump),
                    successor: successor(1),
                },
            },
            SelectedBlock {
                id: SelectedBlockId(1),
                origin: SelectedBlockOrigin::Operation { source, operation },
                instructions: Vec::new(),
                terminator: SelectedTerminator::ConditionalBranch {
                    instruction: instruction(1, SelectedInstructionKind::ConditionalBranchNonZero),
                    when_nonzero: successor(1),
                    when_zero: successor(2),
                },
            },
            SelectedBlock {
                id: SelectedBlockId(2),
                origin: SelectedBlockOrigin::Operation { source, operation },
                instructions: Vec::new(),
                terminator: SelectedTerminator::Return {
                    instruction: instruction(2, SelectedInstructionKind::ReturnUnit),
                    psi_return_edge: returned,
                },
            },
        ]
        .into(),
    }
}

#[test]
fn operation_trap_has_no_successor_and_rejects_forged_lineage() {
    let mut original = function();
    let operation = OperationId::new(2).unwrap();
    let mut instruction =
        selected_instructions::terminator_instruction(&original.blocks[2].terminator).clone();
    instruction.kind = SelectedInstructionKind::Crash;
    original.blocks[2].terminator = SelectedTerminator::OperationTrap {
        instruction,
        operation,
    };
    validate_transports(0, &original).unwrap();
    let liveness = super::super::compute::compute_function(0, &original).unwrap();
    assert!(liveness.blocks[2].successors.is_empty());
    for fault in 0..5 {
        let mut changed = original.clone();
        let SelectedTerminator::OperationTrap {
            instruction,
            operation,
        } = &mut changed.blocks[2].terminator
        else {
            panic!("trap");
        };
        match fault {
            0 => *operation = OperationId::new(9).unwrap(),
            1 => instruction.provenance.edges.push(EdgeId::new(3).unwrap()),
            2 => instruction.provenance.operations.clear(),
            3 => instruction.kind = SelectedInstructionKind::ReturnUnit,
            _ => instruction
                .provenance
                .fuel
                .push(optimization_unit::FuelSettlement {
                    site: optimization_unit::PsiProvenance::Operation(*operation),
                    units: 1,
                }),
        }
        assert!(validate_transports(0, &changed).is_err());
    }
}

#[test]
fn operation_loop_liveness_retains_physical_edges_without_semantic_transfers() {
    let function = function();
    validate_transports(0, &function).unwrap();
    let liveness = super::super::compute::compute_function(0, &function).unwrap();
    assert_eq!(liveness.blocks.len(), 3);
    assert_eq!(
        liveness
            .blocks
            .iter()
            .map(|block| block.successors.len())
            .sum::<usize>(),
        3
    );
    assert!(
        liveness
            .blocks
            .iter()
            .flat_map(|block| &block.successors)
            .all(|edge| edge.psi_edge.is_none())
    );
}

#[test]
fn operation_control_rejects_semantic_authority_and_changed_owners() {
    let original = function();
    let changed = |edit: &dyn Fn(&mut SelectedFunction)| {
        let mut candidate = original.clone();
        edit(&mut candidate);
        assert!(validate_transports(0, &candidate).is_err());
    };
    changed(&|function| {
        selected_instructions::terminator_successors_mut(&mut function.blocks[0].terminator)[0]
            .psi_edge = EdgeId::new(3)
    });
    changed(&|function| {
        selected_instructions::terminator_successors_mut(&mut function.blocks[0].terminator)[0]
            .role = SelectedSuccessorRole::Semantic
    });
    changed(&|function| {
        selected_instructions::terminator_instruction_mut(&mut function.blocks[0].terminator)
            .provenance
            .operations
            .clear()
    });
    changed(&|function| {
        selected_instructions::terminator_instruction_mut(&mut function.blocks[0].terminator)
            .provenance
            .edges
            .push(EdgeId::new(3).unwrap())
    });
    changed(&|function| {
        selected_instructions::terminator_successors_mut(&mut function.blocks[0].terminator)[0]
            .fuel
            .push(optimization_unit::FuelSettlement {
                site: optimization_unit::PsiProvenance::Operation(OperationId::new(2).unwrap()),
                units: 1,
            })
    });
    changed(&|function| {
        function.blocks[1].origin = SelectedBlockOrigin::Operation {
            source: BlockId::new(1).unwrap(),
            operation: OperationId::new(9).unwrap(),
        }
    });
    changed(&|function| {
        function.blocks[1].origin = SelectedBlockOrigin::Source(BlockId::new(1).unwrap())
    });
    changed(&|function| {
        selected_instructions::terminator_successors_mut(&mut function.blocks[1].terminator)[0]
            .block = SelectedBlockId(0)
    });
    changed(&|function| function.entry_block = SelectedBlockId(1));
}
