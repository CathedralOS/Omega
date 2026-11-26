use super::control::Graph;
use register_model::{RegisterConstraintFamily, RegisterConstraintKey};
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind as Kind, SelectedInstructionProvenance,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId};
fn operation() -> OperationId {
    OperationId::new(1).unwrap()
}
fn instruction(id: u32, kind: Kind) -> SelectedInstruction {
    SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: RegisterConstraintKey {
            family: RegisterConstraintFamily::Instruction,
            variant: 0,
        },
        operands: vec![],
        implicit_uses: vec![],
        implicit_defs: vec![],
        clobbers: vec![],
        provenance: SelectedInstructionProvenance {
            operations: vec![operation()],
            ..Default::default()
        },
    }
}
fn function(destinations: &[Option<u32>]) -> SelectedFunction {
    let blocks = destinations
        .iter()
        .enumerate()
        .map(|(index, next)| {
            let id = index as u32;
            let row = instruction(
                id * 2 + 1,
                if next.is_some() {
                    Kind::Jump
                } else {
                    Kind::ReturnUnit
                },
            );
            let terminator = match next {
                Some(next) => SelectedTerminator::Jump {
                    instruction: row,
                    successor: SelectedSuccessor {
                        role: SelectedSuccessorRole::OperationContinuation {
                            operation: operation(),
                        },
                        psi_edge: None,
                        block: SelectedBlockId(*next),
                        source_target: BlockId::new(1).unwrap(),
                        bindings: vec![],
                        structural_bindings: vec![],
                        structural_case: None,
                        work: vec![],
                    },
                },
                None => SelectedTerminator::Return {
                    instruction: row,
                    psi_return_edge: EdgeId::new(1).unwrap(),
                },
            };
            SelectedBlock {
                id: SelectedBlockId(id),
                origin: SelectedBlockOrigin::Operation {
                    source: BlockId::new(1).unwrap(),
                    operation: operation(),
                },
                instructions: vec![instruction(
                    id * 2,
                    Kind::MaterializeI64 {
                        value: semantic_vocabulary::IntegerValue::Unsigned(1),
                    },
                )],
                terminator,
            }
        })
        .collect::<Vec<_>>();
    SelectedFunction {
        contracts: std::sync::Arc::new(selected_instructions::SelectedFunctionContracts {
            structural_block_parameters: Vec::new(),
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: Default::default(),
            structural: None,
            local_storage_slots: vec![],
            outgoing_arguments: vec![],
            calls: vec![],
            normalized_foreign_calls: vec![],
            memory_accesses: vec![],
            boundary_settlements: vec![],
            virtual_registers: vec![],
        }),
        entry_block: SelectedBlockId(0),
        blocks: blocks.into(),
    }
}
#[test]
fn unreachable_instructions_do_not_precede_work() {
    let valid = function(&[Some(1), Some(2), None, None]);
    let graph = Graph::new(&valid).unwrap();
    assert!(
        graph
            .precedes(SelectedInstructionId(0), SelectedInstructionId(4))
            .unwrap()
    );
    let unreachable = function(&[Some(1), None, None, None]);
    let graph = Graph::new(&unreachable).unwrap();
    assert!(
        !graph
            .precedes(SelectedInstructionId(0), SelectedInstructionId(4))
            .unwrap()
    );
}

fn branching() -> SelectedFunction {
    use register_model::{RegisterClassId, RegisterOperandAccess};
    use selected_instructions::{
        SelectedOperand, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin,
    };
    let mut value = function(&[Some(1), Some(2), Some(3), Some(5), None, Some(1), None]);
    let operand = |register, access| SelectedOperand {
        operand: 0,
        virtual_register: VirtualRegisterId(register),
        access,
        class: RegisterClassId(0),
        fixed_view: None,
        tied_to: None,
        early_clobber: false,
    };
    for (register, block) in [(0, 0), (1, 3)] {
        value.blocks[block].instructions[0].operands =
            vec![operand(register, RegisterOperandAccess::Def)];
        value.virtual_registers.push(VirtualRegister {
            id: VirtualRegisterId(register),
            scalar_type: semantic_vocabulary::ScalarType::Integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    64,
                )
                .unwrap(),
            ),
            class: RegisterClassId(0),
            origin: VirtualRegisterOrigin::InstructionScratch {
                instruction: SelectedInstructionId(block as u32 * 2),
                operand: 0,
            },
            definition_site: None,
            entry_fixed_view: None,
        });
    }
    for (block, register, yes, no) in [(1, 0, 2, 5), (3, 1, 5, 6)] {
        let successor = |destination| SelectedSuccessor {
            role: SelectedSuccessorRole::OperationContinuation {
                operation: operation(),
            },
            psi_edge: None,
            block: SelectedBlockId(destination),
            source_target: BlockId::new(1).unwrap(),
            bindings: vec![],
            structural_bindings: vec![],
            structural_case: None,
            work: vec![],
        };
        let mut compare = instruction(1000 + block as u32, Kind::CompareI64Zero);
        compare.operands = vec![operand(register, RegisterOperandAccess::Use)];
        value.blocks[block].instructions.push(compare);
        value.blocks[block].terminator = SelectedTerminator::ConditionalBranch {
            instruction: instruction(block as u32 * 2 + 1, Kind::ConditionalBranchNonZero),
            when_nonzero: successor(yes),
            when_zero: successor(no),
        };
    }
    value
}

#[test]
fn precedes_follows_branch_dominators() {
    let function = branching();
    let graph = Graph::new(&function).unwrap();
    let guard = SelectedInstructionId(7);
    assert!(
        graph
            .precedes(SelectedInstructionId(0), SelectedInstructionId(6))
            .unwrap()
    );
    assert!(graph.precedes(SelectedInstructionId(6), guard).unwrap());
    assert!(
        !graph
            .precedes(SelectedInstructionId(6), SelectedInstructionId(10))
            .unwrap()
    );
    assert!(
        !graph
            .precedes(SelectedInstructionId(4), SelectedInstructionId(0))
            .unwrap()
    );
    assert!(
        !graph
            .precedes(SelectedInstructionId(6), SelectedInstructionId(9))
            .unwrap()
    );
    assert!(
        graph
            .precedes(SelectedInstructionId(u32::MAX), guard)
            .is_err()
    );
}
