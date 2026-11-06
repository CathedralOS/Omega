//! Graph reachability and cycle controls.
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
                        fuel: vec![],
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
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        nominal_cleanup_subject: None,
        provenance: Default::default(),
        structural: None,
        local_storage_slots: vec![],
        outgoing_arguments: vec![],
        calls: vec![],
        normalized_foreign_calls: vec![],
        memory_accesses: vec![],
        boundary_settlements: vec![],
        entry_block: SelectedBlockId(0),
        virtual_registers: vec![],
        blocks: blocks.into(),
    }
}
#[test]
fn installed_publication_must_reach_work_without_a_cycle_or_refusal() {
    let valid = function(&[Some(1), Some(2), None, None]);
    let graph = Graph::new(&valid, operation()).unwrap();
    assert!(
        graph
            .only_success_path(
                SelectedInstructionId(0),
                SelectedInstructionId(4),
                SelectedBlockId(3)
            )
            .is_ok()
    );
    let unreachable = function(&[Some(1), None, None, None]);
    let graph = Graph::new(&unreachable, operation()).unwrap();
    assert!(
        !graph
            .precedes(SelectedInstructionId(0), SelectedInstructionId(4))
            .unwrap()
    );
    assert!(
        graph
            .only_success_path(
                SelectedInstructionId(0),
                SelectedInstructionId(4),
                SelectedBlockId(3)
            )
            .is_err()
    );
    let cycle = function(&[Some(1), Some(0), None, None]);
    assert!(
        Graph::new(&cycle, operation())
            .unwrap()
            .only_success_path(
                SelectedInstructionId(0),
                SelectedInstructionId(4),
                SelectedBlockId(3)
            )
            .is_err()
    );
    let refusal = function(&[Some(3), Some(2), None, None]);
    assert!(
        Graph::new(&refusal, operation())
            .unwrap()
            .only_success_path(
                SelectedInstructionId(0),
                SelectedInstructionId(4),
                SelectedBlockId(3)
            )
            .is_err()
    );
}
