use super::super::super::{
    EvidenceIdentity, IntegerSign, IntegerType, ObligationId, OperationId, StructuralPlaceKind,
    StructuralTypeId,
};

use super::{
    AbstractOperation, BlockId, EdgeId, OptimizationUnitValidationError, PlaceId, ScalarType,
    TerminalFuelSchedule, ValueId, id, refresh_identity,
    validate_transformed_psi_optimization_unit, validate_verified_psi_optimization_unit,
    verified_unit,
};
use terminal_psi::{
    Block, ByteSequenceCarrier, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, Terminator, ValueDeclaration,
};

#[test]
fn surviving_byte_field_length_retains_exact_operation_and_storage_identity() {
    let initial = verified_byte_operation(false);
    let mut module = initial.input().context().module().clone();
    let byte_type = module.structural_types[0].id;
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: (1..=2)
            .map(|ordinal| terminal_psi::StructuralFieldDeclaration {
                id: id(ordinal, semantic_vocabulary::StructuralFieldId::new),
                identity: format!("bytes{ordinal}"),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::ByteSequence(
                    ByteSequenceCarrier::BoundedOwned { capacity: 3 },
                ),
            })
            .collect(),
    };
    let machine = &mut module.machines[0];
    machine.blocks.truncate(1);
    machine.blocks[0].operations.truncate(1);
    machine.blocks[0].operations[0].kind = OperationKind::StructuralByteSequenceFieldLength {
        source: machine.structural_parameters[0].place,
        path: Vec::new(),
        field: id(1, semantic_vocabulary::StructuralFieldId::new),
    };
    machine.blocks[0].terminator = Terminator::ReturnUnit {
        edge: id(1, EdgeId::new),
        trivial_affine_discards: Vec::new(),
    };
    assert_eq!(machine.structural_parameters[1].structural_type, byte_type);
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&module).unwrap(),
            proof_bytes: &terminal_codec::encode_proof_section(
                &module,
                &terminal_verifier::ProofBundle::default(),
            )
            .unwrap(),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap();
    let verified = terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap();
    validate_verified_psi_optimization_unit(&verified).unwrap();
    let (input, baseline) = verified.into_parts();
    for mutation in 0..7 {
        let mut changed = baseline.clone();
        let operation = &mut changed.functions[0].blocks[0].nodes[0].operation;
        let AbstractOperation::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            source,
            path,
            field,
        } = operation
        else {
            panic!("field length")
        };
        match mutation {
            0 => *source = id(2, PlaceId::new),
            1 => *field = id(2, semantic_vocabulary::StructuralFieldId::new),
            2 => path.push(terminal_psi::StructuralPathSegment::Field("bytes1".into())),
            3 => *psi_operation = id(9, OperationId::new),
            4 => result.value = id(9, ValueId::new),
            5 => result.scalar_type = ScalarType::Boolean,
            6 => {
                *operation = AbstractOperation::ByteSequenceLength {
                    psi_operation: *psi_operation,
                    result: *result,
                    source: *source,
                }
            }
            _ => panic!("bounded mutation"),
        }
        refresh_identity(&mut changed);
        assert_ne!(changed.identity, baseline.identity);
        assert!(
            validate_transformed_psi_optimization_unit(&input, &changed).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn surviving_byte_read_cannot_reuse_bounds_after_operand_drift() {
    let verified = verified_byte_operation(false);
    validate_verified_psi_optimization_unit(&verified).unwrap();
    let (input, baseline) = verified.into_parts();
    for mutation in 0..7 {
        let mut changed = baseline.clone();
        let function = &mut changed.functions[0];
        let read = &mut function.blocks[1].nodes[0];
        let AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            source,
            index,
            length,
            obligation,
        } = &mut read.operation
        else {
            panic!("guarded read")
        };
        match mutation {
            0 => {
                *index = id(5, ValueId::new);
                read.uses[0].value = *index;
            }
            1 => *source = id(2, PlaceId::new),
            2 => *length = id(1, ValueId::new),
            3 => *obligation = id(9, ObligationId::new),
            4 => result.value = id(9, ValueId::new),
            5 => result.scalar_type = ScalarType::Boolean,
            6 => *psi_operation = id(9, OperationId::new),
            _ => panic!("bounded mutation"),
        }
        refresh_identity(&mut changed);
        let failure = validate_transformed_psi_optimization_unit(&input, &changed).unwrap_err();
        if mutation == 0 {
            assert!(matches!(
                failure,
                OptimizationUnitValidationError::OperationObligationOwnerMismatch { .. }
            ));
        }
    }
    let mut substituted_view = baseline.clone();
    let function = &mut substituted_view.functions[0];
    let AbstractOperation::ByteSequenceLength { source, .. } =
        &mut function.blocks[0].nodes[0].operation
    else {
        panic!("length")
    };
    *source = id(2, PlaceId::new);
    let AbstractOperation::ByteSequenceRead { source, .. } =
        &mut function.blocks[1].nodes[0].operation
    else {
        panic!("read")
    };
    *source = id(2, PlaceId::new);
    refresh_identity(&mut substituted_view);
    assert!(matches!(
        validate_transformed_psi_optimization_unit(&input, &substituted_view),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { .. })
    ));

    let mut missing_fact = baseline.clone();
    missing_fact.accepted_obligation_facts.clear();
    refresh_identity(&mut missing_fact);
    assert!(validate_transformed_psi_optimization_unit(&input, &missing_fact).is_err());
}

pub(super) fn verified_byte_operation(
    subslice: bool,
) -> terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit {
    let initial = verified_unit();
    let mut module = initial.input().context().module().clone();
    let byte_type = id(1, StructuralTypeId::new);
    module.structural_types = vec![StructuralTypeDeclaration {
        id: byte_type,
        identity: "test::Bytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
    }];
    let count_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = |ordinal| id(ordinal, ValueId::new);
    let machine = &mut module.machines[0];
    machine.contract.ensures.clear();
    machine.parameters = vec![
        ValueDeclaration {
            qualifications: Default::default(),
            id: value(1),
            scalar_type: count_type,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: value(5),
            scalar_type: count_type,
        },
    ];
    machine.structural_parameters = (0..2)
        .map(|position| StructuralParameterDeclaration {
            place: id(u64::from(position) + 1, PlaceId::new),
            position,
            is_self: false,
            structural_type: byte_type,
            access: StructuralAccess::SharedBorrow,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        })
        .collect();
    machine.structural_places = machine
        .structural_parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: false,
            },
        })
        .collect();
    let successor = |edge, block| SuccessorEdge {
        erased_arguments: Vec::new(),
        edge: id(edge, EdgeId::new),
        target: id(block, BlockId::new),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.entry = id(1, BlockId::new);
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            id: id(1, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: id(1, OperationId::new),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: value(2),
                        scalar_type: count_type,
                    }),
                    kind: OperationKind::ByteSequenceLength {
                        source: id(1, PlaceId::new),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: id(2, OperationId::new),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
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
                when_true: successor(1, 2),
                when_false: successor(2, 3),
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            id: id(2, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id(3, OperationId::new),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: value(4),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    ),
                }),
                kind: OperationKind::ByteSequenceRead {
                    source: id(1, PlaceId::new),
                    index: value(1),
                    length: value(2),
                    obligation: id(1, ObligationId::new),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id(3, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            id: id(3, BlockId::new),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(4, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        },
    ];
    if subslice {
        let result = terminal_psi::StructuralOperationResult {
            place: id(3, PlaceId::new),
            structural_type: byte_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        };
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: id(3, OperationId::new),
                structural_type: byte_type,
            },
        });
        machine.blocks[1].operations[0].result = OperationResult::Structural(result);
        machine.blocks[1].operations[0].kind = OperationKind::ByteSequenceSubslice {
            source: id(1, PlaceId::new),
            start: value(1),
            end: value(2),
            length: value(2),
            obligation: id(1, ObligationId::new),
        };
        machine.blocks[1].operations.push(Operation {
            static_reach_binding: None,
            id: id(4, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value(6),
                scalar_type: count_type,
            }),
            kind: OperationKind::ByteSequenceLength {
                source: id(3, PlaceId::new),
            },
        });
    }
    let questions = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    let [question] = questions.obligations() else {
        panic!("one read obligation")
    };
    let axiom = question
        .semantic_axioms
        .iter()
        .position(|proposition| {
            proposition
                == &semantic_vocabulary::Proposition::LessThan(
                    semantic_vocabulary::ScalarTerm::value(value(1), count_type),
                    semantic_vocabulary::ScalarTerm::value(value(2), count_type),
                )
        })
        .unwrap();
    let rule = if subslice {
        use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};
        use semantic_vocabulary::{Proposition, ScalarTerm};
        let Proposition::Conjunction(legs) = &question.obligation.proposition else {
            panic!("two range legs")
        };
        ProofRule::ConjunctionIntroduction(vec![
            ProofNode {
                conclusion: legs[0].clone(),
                rule: ProofRule::IntegerOrderWeakening {
                    relation: Box::new(ProofNode {
                        conclusion: question.semantic_axioms[axiom].clone(),
                        rule: ProofRule::SemanticAxiom { index: axiom },
                    }),
                },
            },
            ProofNode {
                conclusion: legs[1].clone(),
                rule: ProofRule::IntegerOrderWeakening {
                    relation: Box::new(ProofNode {
                        conclusion: Proposition::Equal(
                            ScalarTerm::value(value(2), count_type),
                            ScalarTerm::value(value(2), count_type),
                        ),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                    }),
                },
            },
        ])
    } else {
        proof_admission::ProofRule::SemanticAxiom { index: axiom }
    };
    let proof = terminal_verifier::ProofBundle {
        evidence: vec![terminal_verifier::ObligationEvidence {
            obligation: question.obligation.id,
            route: proof_admission::EvidenceRoute::CertificateDerived(
                proof_admission::CertificateEnvelope {
                    identity: id(1, EvidenceIdentity::new),
                    proof_system_marker: proof_admission::ProofSystemMarker::CURRENT,
                    proof: proof_admission::ProofNode {
                        conclusion: question.obligation.proposition.clone(),
                        rule,
                    },
                },
            ),
        }],
        ..terminal_verifier::ProofBundle::default()
    };
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&module).unwrap(),
            proof_bytes: &terminal_codec::encode_proof_section(&module, &proof).unwrap(),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap();
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}
