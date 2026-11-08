use super::*;
use semantic_vocabulary::{
    BoundaryMachineId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId, ScalarType,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
    TerminalMachineResult, ValueDeclaration,
};

fn integer(id: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
        qualifications: Default::default(),
    }
}

fn graph(base: u64) -> TerminalMachine {
    let value = |id| integer(base + id);
    let condition = ValueDeclaration {
        scalar_type: ScalarType::Boolean,
        ..value(8)
    };
    let constant = |operation, id| {
        Operation::unsuspended(
            OperationId::new(operation).unwrap(),
            OperationResult::Scalar(value(id)),
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(17),
            },
        )
    };
    let edge = |id, raw| SuccessorEdge {
        edge: EdgeId::new(id).unwrap(),
        target: BlockId::new(2).unwrap(),
        arguments: vec![value(raw).id, value(3).id],
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        cleanup_actions: Vec::new(),
    };
    let entry = Block {
        id: BlockId::new(1).unwrap(),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: vec![
            constant(1, 1),
            constant(2, 4),
            Operation::unsuspended(
                OperationId::new(3).unwrap(),
                OperationResult::Scalar(value(3)),
                OperationKind::BoundaryCall {
                    boundary: BoundaryMachineId::new(1).unwrap(),
                    arguments: vec![value(1).id],
                    structural_arguments: Vec::new(),
                    completion_receipts: Vec::new(),
                    requirement_obligations: Vec::new(),
                },
            ),
        ],
        terminator: Terminator::Conditional {
            condition: condition.id,
            when_true: edge(1, 1),
            when_false: edge(2, 4),
        },
    };
    let target = Block {
        id: BlockId::new(2).unwrap(),
        parameters: vec![value(10), value(11)],
        structural_parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        operations: vec![Operation::unsuspended(
            OperationId::new(4).unwrap(),
            OperationResult::Scalar(value(12)),
            OperationKind::IntegerBitwiseNot {
                operand: value(11).id,
            },
        )],
        terminator: Terminator::Return {
            edge: EdgeId::new(3).unwrap(),
            value: value(12).id,
            cleanup_actions: Vec::new(),
        },
    };
    TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id: MachineId::new(1).unwrap(),
        declaration: None,
        attachment: None,
        nominal_cleanup_subject: None,
        parameters: vec![condition],
        parameter_order: vec![terminal_psi::BoundaryParameterKind::Scalar],
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(value(19)),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: entry.id,
        blocks: vec![entry, target],
        contract: MachineContract::empty(ContractId::new(1).unwrap()),
    }
}

#[test]
fn generated_scalar_transport_removes_only_unused_slots_on_every_incoming_edge() {
    for base in [0, u64::MAX - 32] {
        let mut machine = graph(base);
        let original = machine.clone();
        assert!(complete_scalar_demand(
            &mut machine,
            [],
            [],
            ScalarDemandMode::KeepAllOperations
        ));
        assert_eq!(machine.parameters, original.parameters);
        assert_eq!(machine.result, original.result);
        assert_eq!(machine.blocks[1].parameters, [integer(base + 11)]);
        for (before, after) in original.blocks.iter().zip(&machine.blocks) {
            assert_eq!(
                before.operations, after.operations,
                "source operations are never erased"
            );
        }
        let Terminator::Conditional {
            when_true,
            when_false,
            ..
        } = &machine.blocks[0].terminator
        else {
            panic!("original control retained")
        };
        assert_eq!(when_true.arguments, [ValueId::new(base + 3).unwrap()]);
        assert_eq!(when_false.arguments, when_true.arguments);
    }
}

#[test]
fn generated_scalar_transport_keeps_real_reads_and_evidence_and_refuses_partial_inputs() {
    let mut read = graph(0);
    // A real read stays evaluated even when its result is unused. Removing
    // only transport must not hide an ordinary post-close scalar use.
    read.blocks[1].operations.push(Operation::unsuspended(
        OperationId::new(5).unwrap(),
        OperationResult::Scalar(integer(13)),
        OperationKind::IntegerBitwiseNot {
            operand: integer(10).id,
        },
    ));
    let before = read.clone();
    assert!(complete_scalar_demand(
        &mut read,
        [],
        [],
        ScalarDemandMode::KeepAllOperations
    ));
    assert_eq!(read, before);
    let mut optimized = before.clone();
    assert!(complete_scalar_demand(
        &mut optimized,
        [],
        [],
        ScalarDemandMode::DropUnusedTotalOperations
    ));
    assert!(
        !optimized
            .operations()
            .any(|operation| operation.id == OperationId::new(5).unwrap())
    );
    assert!(
        optimized
            .operations()
            .any(|operation| operation.id == OperationId::new(3).unwrap())
    );
    assert_eq!(optimized.blocks[1].parameters, [integer(11)]);

    for (retained, protected) in [
        (vec![integer(10).id], Vec::new()),
        (Vec::new(), vec![BlockId::new(2).unwrap()]),
    ] {
        let mut pinned = graph(0);
        let before = pinned.clone();
        assert!(complete_scalar_demand(
            &mut pinned,
            retained,
            protected,
            ScalarDemandMode::KeepAllOperations
        ));
        assert_eq!(pinned, before);
    }
    // Evidence-only and path-only uses are independent of the ordinary
    // transported runtime slots. Each one must retain the original scalar.
    for proof in [false, true] {
        let mut evidence = graph(0);
        let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
        let type_identity = identities.intern("u64");
        let term = semantic_vocabulary::ScalarTerm::Value {
            id: integer(10).id,
            scalar_type: integer(10).scalar_type,
        };
        evidence.blocks[1].terminator = Terminator::Jump {
            edge: EdgeId::new(3).unwrap(),
            target: BlockId::new(3).unwrap(),
            arguments: Vec::new(),
            erased_arguments: if proof {
                Vec::new()
            } else {
                vec![term.clone()]
            },
            erased_proof_arguments: if proof {
                vec![semantic_vocabulary::ProofTerm::Scalar(term)]
            } else {
                Vec::new()
            },
            structural_arguments: Vec::new(),
            cleanup_actions: Vec::new(),
        };
        evidence.blocks.push(Block {
            id: BlockId::new(3).unwrap(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            erased_scalar_formals: if proof { Vec::new() } else { vec![integer(20)] },
            erased_proof_formals: if proof {
                vec![terminal_psi::ErasedProofFormal {
                    source_position: 0,
                    type_identity,
                }]
            } else {
                Vec::new()
            },
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: EdgeId::new(4).unwrap(),
                value: integer(12).id,
                cleanup_actions: Vec::new(),
            },
        });
        let before = evidence.clone();
        assert!(complete_scalar_demand(
            &mut evidence,
            [],
            [],
            ScalarDemandMode::KeepAllOperations
        ));
        assert_eq!(evidence, before);
        let Terminator::Jump {
            erased_arguments,
            erased_proof_arguments,
            ..
        } = &mut evidence.blocks[1].terminator
        else {
            unreachable!()
        };
        let missing = semantic_vocabulary::ScalarTerm::Value {
            id: integer(30).id,
            scalar_type: integer(30).scalar_type,
        };
        if proof {
            erased_proof_arguments[0] = semantic_vocabulary::ProofTerm::Scalar(missing);
        } else {
            erased_arguments[0] = missing;
        }
        let before = evidence.clone();
        assert!(!complete_scalar_demand(
            &mut evidence,
            [],
            [],
            ScalarDemandMode::KeepAllOperations
        ));
        assert_eq!(evidence, before);
    }
    let mut cleanup = graph(0);
    let Terminator::Return {
        cleanup_actions, ..
    } = &mut cleanup.blocks[1].terminator
    else {
        unreachable!()
    };
    cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
        terminal_psi::StructuralAffineDiscard {
            place: semantic_vocabulary::PlaceId::new(1).unwrap(),
            structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            path: vec![terminal_psi::StructuralPathSegment::RuntimeIndex {
                index: integer(10).id,
                obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            }],
        },
    ));
    let before = cleanup.clone();
    assert!(complete_scalar_demand(
        &mut cleanup,
        [],
        [],
        ScalarDemandMode::KeepAllOperations
    ));
    assert_eq!(cleanup, before);

    for actual in [integer(30).id, ValueId::new(8).unwrap(), integer(12).id] {
        let mut unused = graph(0);
        let Terminator::Conditional { when_false, .. } = &mut unused.blocks[0].terminator else {
            unreachable!()
        };
        when_false.arguments[0] = actual;
        let before = unused.clone();
        assert!(!complete_scalar_demand(
            &mut unused,
            [],
            [],
            ScalarDemandMode::KeepAllOperations
        ));
        assert_eq!(
            unused, before,
            "unused undefined, wrongly typed or unavailable inputs cannot disappear"
        );
    }
    let mut incomplete = graph(0);
    let Terminator::Conditional { when_false, .. } = &mut incomplete.blocks[0].terminator else {
        unreachable!()
    };
    when_false.arguments.pop();
    let before = incomplete.clone();
    assert!(!complete_scalar_demand(
        &mut incomplete,
        [],
        [],
        ScalarDemandMode::KeepAllOperations
    ));
    assert_eq!(incomplete, before);
    let mut unresolved = graph(0);
    unresolved.blocks[1].operations[0].kind = OperationKind::IntegerBitwiseNot {
        operand: integer(30).id,
    };
    let before = unresolved.clone();
    assert!(!complete_scalar_demand(
        &mut unresolved,
        [],
        [],
        ScalarDemandMode::KeepAllOperations
    ));
    assert_eq!(unresolved, before);
}
