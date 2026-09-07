//! Verified immutable byte-operation retention and unsupported subslice coverage.

use super::*;

pub(super) fn byte_operation_fence(subslice: bool) {
    use proof_admission::{
        CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    };
    use semantic_vocabulary::{
        EvidenceIdentity, IntegerSign, IntegerType, ObligationId, Proposition, ScalarTerm,
        ScalarType, ValueId,
    };
    use terminal_psi::{SuccessorEdge, ValueDeclaration};
    let mut module = byte_sequence_module(Vec::new());
    module.boundary_machines.clear();
    let machine = &mut module.machines[0];
    machine.entry = block_id(2);
    let count_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = |ordinal| ValueId::new(ordinal).unwrap();
    machine.parameters = vec![ValueDeclaration {
        id: value(1),
        scalar_type: count_type,
    }];
    machine.structural_places[0].kind = semantic_vocabulary::StructuralPlaceKind::Parameter {
        position: 0,
        is_self: false,
    };
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    let successor = |edge, block| SuccessorEdge {
        structural_arguments: Vec::new(),
        edge: edge_id(edge),
        target: block_id(block),
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    // Storage order need not be execution order. Visit the read block first
    // during native lowering to exercise declaration-order independence;
    // verification still requires the entry length and true edge to dominate it.
    machine.blocks = vec![
        Block {
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(3),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: value(4),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    ),
                }),
                kind: OperationKind::ByteSequenceRead {
                    source: place_id(1),
                    index: value(1),
                    length: value(2),
                    obligation: ObligationId::new(1).unwrap(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(3),
                trivial_affine_discards: Vec::new(),
            },
        },
        Block {
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: operation_id(1),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value(2),
                        scalar_type: count_type,
                    }),
                    kind: OperationKind::ByteSequenceLength {
                        source: place_id(1),
                    },
                },
                Operation {
                    id: operation_id(2),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value(3),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerLessThan {
                        left: value(1),
                        right: value(2),
                    },
                },
            ],
            terminator: Terminator::Conditional {
                condition: value(3),
                when_true: successor(1, 1),
                when_false: successor(2, 3),
            },
        },
        Block {
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(4),
                trivial_affine_discards: Vec::new(),
            },
        },
    ];
    if subslice {
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(3),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(3),
                structural_type: StructuralTypeId::new(1).unwrap(),
            },
        });
        machine.blocks[0].operations[0].result =
            OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: place_id(3),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            });
        machine.blocks[0].operations[0].kind = OperationKind::ByteSequenceSubslice {
            source: place_id(1),
            start: value(1),
            end: value(2),
            length: value(2),
            obligation: ObligationId::new(1).unwrap(),
        };
    }
    let questions = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    let [site] = questions.obligations() else {
        panic!("one read question")
    };
    let index = site
        .semantic_axioms
        .iter()
        .position(|fact| {
            fact == &Proposition::LessThan(
                ScalarTerm::value(value(1), count_type),
                ScalarTerm::value(value(2), count_type),
            )
        })
        .unwrap();
    let rule = if subslice {
        let Proposition::Conjunction(children) = &site.obligation.proposition else {
            panic!("range")
        };
        ProofRule::ConjunctionIntroduction(vec![
            ProofNode {
                conclusion: children[0].clone(),
                rule: ProofRule::IntegerOrderWeakening {
                    relation: Box::new(ProofNode {
                        conclusion: site.semantic_axioms[index].clone(),
                        rule: ProofRule::SemanticAxiom { index },
                    }),
                },
            },
            ProofNode {
                conclusion: children[1].clone(),
                rule: ProofRule::IntegerOrderWeakening {
                    relation: Box::new(ProofNode {
                        conclusion: Proposition::Equal(
                            ScalarTerm::value(value(2), count_type),
                            ScalarTerm::value(value(2), count_type),
                        ),
                        rule: ProofRule::Primitive(
                            proof_admission::PrimitiveJudgment::ReflexiveEquality,
                        ),
                    }),
                },
            },
        ])
    } else {
        ProofRule::SemanticAxiom { index }
    };
    let bundle = ProofBundle {
        evidence: vec![terminal_verifier::ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: site.obligation.proposition.clone(),
                    rule,
                },
            }),
        }],
        ..ProofBundle::default()
    };
    terminal_verifier::verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&bundle).unwrap();
    if !subslice {
        validate_retained_read(&semantic, &proof);
        return;
    }
    let error =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap_err();
    match error {
        terminal_psi_to_abstract_operations::ArtifactLoweringError::Lowering(
            terminal_psi_to_abstract_operations::LoweringError::UnsupportedByteSequenceSubslice(
                operation,
            ),
        ) if subslice => assert_eq!(operation, operation_id(3)),
        other => panic!("wrong native fence: {other:?}"),
    }
}

fn validate_retained_read(semantic: &[u8], proof: &[u8]) {
    use terminal_psi_to_abstract_operations::{
        ProviderInstallationError, admit_provider_installation,
        build_verified_psi_optimization_unit, lower_artifact_sections_for_optimization,
    };
    let profile = AdmissionProfile::default();
    let input = lower_artifact_sections_for_optimization(semantic, proof, &profile).unwrap();
    let verified = build_verified_psi_optimization_unit(
        input,
        semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(verified.unit()).unwrap();
    let plan = lower_artifact_sections(semantic, proof, &profile).unwrap();
    let read_position = plan.functions[0]
        .operations
        .iter()
        .position(|operation| matches!(operation, AbstractOperation::ByteSequenceRead { .. }))
        .unwrap();
    let expected_result = abstract_operations::AbstractResult {
        value: semantic_vocabulary::ValueId::new(4).unwrap(),
        scalar_type: semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
        ),
    };
    assert_eq!(
        plan.functions[0].operations[read_position],
        AbstractOperation::ByteSequenceRead {
            psi_operation: operation_id(3),
            result: expected_result,
            source: place_id(1),
            index: semantic_vocabulary::ValueId::new(1).unwrap(),
            length: semantic_vocabulary::ValueId::new(2).unwrap(),
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
        }
    );
    let [fact] = verified.unit().accepted_obligation_facts.as_slice() else {
        panic!("read keeps its accepted bounds fact")
    };
    assert_eq!(fact.operation, operation_id(3));
    assert_eq!(
        fact.obligation,
        semantic_vocabulary::ObligationId::new(1).unwrap()
    );
    for mutation in 0..7 {
        let mut changed = plan.clone();
        let AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            source,
            index,
            length,
            obligation,
        } = &mut changed.functions[0].operations[read_position]
        else {
            panic!("read")
        };
        match mutation {
            0 => *psi_operation = operation_id(9),
            1 => *source = place_id(9),
            2 => *index = semantic_vocabulary::ValueId::new(2).unwrap(),
            3 => *length = semantic_vocabulary::ValueId::new(1).unwrap(),
            4 => *obligation = semantic_vocabulary::ObligationId::new(9).unwrap(),
            5 => result.value = semantic_vocabulary::ValueId::new(9).unwrap(),
            6 => result.scalar_type = semantic_vocabulary::ScalarType::Boolean,
            _ => panic!("bounded mutations"),
        }
        assert!(matches!(
            admit_provider_installation(&changed, semantic, proof, &profile, &[]),
            Err(ProviderInstallationError::PlanReplayMismatch)
        ));
        let original_seed = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &plan,
            semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let changed_seed = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &changed,
            semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        assert_ne!(original_seed.identity, changed_seed.identity);
    }
}
