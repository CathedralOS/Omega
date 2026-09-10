//! Canonical guarded byte-view operations and their reconstructed bounds proofs.
use super::super::*;
use proof_admission::{
    CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, Proposition, ScalarType,
};
use terminal_psi::{
    Block, Operation, OperationResult, StructuralMultiplicity, StructuralOperationResult,
    SuccessorEdge, Terminator, ValueDeclaration,
};
use terminal_verifier::ObligationEvidence;

pub(in super::super) fn suffix_module() -> TerminalModule {
    let mut module = byte_view_read_module();
    let machine = &mut module.machines[0];
    let structural_type = machine.structural_parameters[0].structural_type;
    let suffix = PlaceId::new(30).unwrap();
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: suffix,
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(13).unwrap(),
            structural_type,
        },
    });
    machine.blocks[0].operations[1].kind = OperationKind::IntegerLessOrEqual {
        left: ValueId::new(10).unwrap(),
        right: ValueId::new(5).unwrap(),
    };
    machine.blocks[1].operations[0].result =
        OperationResult::Structural(StructuralOperationResult {
            place: suffix,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        });
    machine.blocks[1].operations[0].kind = OperationKind::ByteSequenceSubslice {
        source: PlaceId::new(3).unwrap(),
        start: ValueId::new(10).unwrap(),
        end: ValueId::new(5).unwrap(),
        length: ValueId::new(5).unwrap(),
        obligation: ObligationId::new(1).unwrap(),
    };
    machine.blocks[1].operations[1].kind = OperationKind::ByteSequenceLength { source: suffix };
    module
}

pub(super) fn nested_suffix_module() -> TerminalModule {
    let mut module = suffix_module();
    let machine = &mut module.machines[0];
    let scalar_type = machine.parameters[0].scalar_type;
    let structural_type = machine.structural_parameters[0].structural_type;
    let start = ValueId::new(31).unwrap();
    let length = ValueId::new(14).unwrap();
    let condition = ValueId::new(32).unwrap();
    let nested = PlaceId::new(40).unwrap();
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: start,
        scalar_type,
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: nested,
        kind: StructuralPlaceKind::OperationResult {
            producer: OperationId::new(41).unwrap(),
            structural_type,
        },
    });
    machine.blocks[1].operations.push(Operation {
        id: OperationId::new(32).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: condition,
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::IntegerLessOrEqual {
            left: start,
            right: length,
        },
    });
    machine.blocks[1].terminator = Terminator::Conditional {
        condition,
        when_true: successor(33, 40),
        when_false: successor(34, 15),
    };
    machine.blocks.push(Block {
        id: BlockId::new(40).unwrap(),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![
            Operation {
                id: OperationId::new(41).unwrap(),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: nested,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::ByteSequenceSubslice {
                    source: PlaceId::new(30).unwrap(),
                    start,
                    end: length,
                    length,
                    obligation: ObligationId::new(2).unwrap(),
                },
            },
            Operation {
                id: OperationId::new(42).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: ValueId::new(42).unwrap(),
                    scalar_type,
                }),
                kind: OperationKind::ByteSequenceLength { source: nested },
            },
        ],
        terminator: Terminator::Return {
            edge: EdgeId::new(43).unwrap(),
            value: ValueId::new(42).unwrap(),
            cleanup_actions: Vec::new(),
        },
    });
    module
}

pub(super) fn derived_read_module(nested: bool) -> TerminalModule {
    let mut module = if nested {
        nested_suffix_module()
    } else {
        suffix_module()
    };
    let machine = &mut module.machines[0];
    let scalar_type = machine.parameters[0].scalar_type;
    let position = ValueId::new(50).unwrap();
    let condition = ValueId::new(51).unwrap();
    let source = PlaceId::new(if nested { 40 } else { 30 }).unwrap();
    let length = ValueId::new(if nested { 42 } else { 14 }).unwrap();
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: position,
        scalar_type,
    });
    let producer_block = &mut machine.blocks[if nested { 3 } else { 1 }];
    producer_block.operations.push(Operation {
        id: OperationId::new(51).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: condition,
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::IntegerLessThan {
            left: position,
            right: length,
        },
    });
    producer_block.terminator = Terminator::Conditional {
        condition,
        when_true: successor(52, 60),
        when_false: successor(53, 15),
    };
    machine.blocks.push(Block {
        id: BlockId::new(60).unwrap(),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![
            Operation {
                id: OperationId::new(60).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: ValueId::new(60).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    ),
                }),
                kind: OperationKind::ByteSequenceRead {
                    source,
                    index: position,
                    length,
                    obligation: ObligationId::new(if nested { 3 } else { 2 }).unwrap(),
                },
            },
            Operation {
                id: OperationId::new(61).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: ValueId::new(61).unwrap(),
                    scalar_type,
                }),
                kind: OperationKind::IntegerWiden {
                    operand: ValueId::new(60).unwrap(),
                },
            },
        ],
        terminator: Terminator::Return {
            edge: EdgeId::new(62).unwrap(),
            value: ValueId::new(61).unwrap(),
            cleanup_actions: Vec::new(),
        },
    });
    module
}

pub(super) fn subrange_module(read: bool) -> TerminalModule {
    let mut module = if read {
        derived_read_module(false)
    } else {
        suffix_module()
    };
    let machine = &mut module.machines[0];
    let end = ValueId::new(70).unwrap();
    machine.parameters.insert(
        1,
        ValueDeclaration {
            qualifications: Default::default(),
            id: end,
            scalar_type: machine.parameters[0].scalar_type,
        },
    );
    machine.blocks[0].operations[1].kind = OperationKind::IntegerLessOrEqual {
        left: ValueId::new(10).unwrap(),
        right: end,
    };
    let Terminator::Conditional { when_true, .. } = &mut machine.blocks[0].terminator else {
        panic!("endpoint order guard")
    };
    when_true.target = BlockId::new(70).unwrap();
    let OperationKind::ByteSequenceSubslice { end: endpoint, .. } =
        &mut machine.blocks[1].operations[0].kind
    else {
        panic!("subrange")
    };
    *endpoint = end;
    machine.blocks.push(Block {
        id: BlockId::new(70).unwrap(),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: vec![Operation {
            id: OperationId::new(71).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: ValueId::new(71).unwrap(),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::IntegerLessOrEqual {
                left: end,
                right: ValueId::new(5).unwrap(),
            },
        }],
        terminator: Terminator::Conditional {
            condition: ValueId::new(71).unwrap(),
            when_true: successor(72, 12),
            when_false: successor(73, 15),
        },
    });
    module
}

fn successor(edge: u64, block: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(block).unwrap(),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn prove(goal: &Proposition, axioms: &[Proposition]) -> ProofNode {
    let rule = if let Some(index) = axioms.iter().position(|fact| fact == goal) {
        ProofRule::SemanticAxiom { index }
    } else {
        match goal {
            Proposition::Conjunction(children) => ProofRule::ConjunctionIntroduction(
                children.iter().map(|child| prove(child, axioms)).collect(),
            ),
            Proposition::LessOrEqual(left, right) if left == right => {
                ProofRule::IntegerOrderWeakening {
                    relation: Box::new(ProofNode {
                        conclusion: Proposition::Equal(left.clone(), right.clone()),
                        rule: ProofRule::Primitive(
                            proof_admission::PrimitiveJudgment::ReflexiveEquality,
                        ),
                    }),
                }
            }
            _ => panic!("fixture needs its exact guard or reflexive endpoint: {goal:?}"),
        }
    };
    ProofNode {
        conclusion: goal.clone(),
        rule,
    }
}

pub(in super::super) fn suffix_proof(module: &TerminalModule) -> ProofBundle {
    let questions = terminal_verifier::reconstruct_terminal_obligations(module).unwrap();
    let evidence = questions
        .obligations()
        .iter()
        .map(|site| {
            assert!(site.canonical_certificate);
            assert_eq!(
                site.obligation.class,
                proof_admission::ObligationClass::Derivable
            );
            ObligationEvidence {
                obligation: site.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(site.obligation.id.get()).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: prove(&site.obligation.proposition, &site.semantic_axioms),
                }),
            }
        })
        .collect();
    let proof = ProofBundle {
        evidence,
        ..ProofBundle::default()
    };
    terminal_verifier::verify_module(module, &proof, &AdmissionProfile::default()).unwrap();
    proof
}
