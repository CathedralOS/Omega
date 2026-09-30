//! Source-free replay of IEEE field predicates and exact store/read observations.
use super::{
    BindingRelevance, CanonicalStructuralPathSegment, ContractClause, Operation, OperationKind,
    OperationResult, Proposition, ScalarTerm, ScalarType, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, decode_module, edge_id, encode_module,
    obligation_id, operation_id, place_id, structural_field_id, structural_type_id, unit_fixture,
    value_id,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker, check_value_equality_denotation,
};
use semantic_vocabulary::{
    EvidenceIdentity, IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue,
};
use terminal_codec::{decode_proof_bundle, encode_proof_bundle};
use terminal_verifier::{
    ObligationEvidence, ProofBundle, reconstruct_terminal_obligations, verify_module,
};

fn fixture(format: IeeeFloatFormat) -> TerminalModule {
    let mut module = unit_fixture();
    let root = place_id(901);
    let record = structural_type_id(901);
    let scalar_type = ScalarType::IeeeFloat(format);
    let field = ScalarTerm::ieee_float_field_path(
        root,
        vec![CanonicalStructuralPathSegment::Field(structural_field_id(
            1,
        ))],
        format,
    );
    let value = |identity| ScalarTerm::value(value_id(identity), scalar_type);
    let bits = match format {
        IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0x8000_0000),
        IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
    };
    let predicate = |subject| Proposition::ScalarIeeeFloatComparison {
        kind: IeeeFloatComparisonKind::LessOrEqual,
        format,
        left: subject,
        right: ScalarTerm::IeeeFloatLiteral(bits),
    };
    module.structural_types.push(StructuralTypeDeclaration {
        id: record,
        identity: "FloatRecord".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: structural_field_id(1),
                identity: "value".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::IeeeFloat(format),
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: root,
            position: 0,
            is_self: false,
            structural_type: record,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: root,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    machine.contract.requires = vec![predicate(field.clone())];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(905),
        scalar_type,
        qualifications: Default::default(),
    });
    machine.blocks[0].terminator = Terminator::Return {
        edge: edge_id(900),
        value: value_id(901),
        cleanup_actions: Vec::new(),
    };
    machine.contract.ensures = [predicate(value(905)), predicate(field)]
        .into_iter()
        .enumerate()
        .map(|(index, proposition)| ContractClause {
            obligation: obligation_id(901 + index as u64),
            proposition,
        })
        .collect();
    let read = || OperationKind::PrimitiveScalarRead {
        source: root,
        path: vec![StructuralPathSegment::Field("value".into())],
    };
    for (identity, kind) in [
        (901, read()),
        (902, OperationKind::IeeeFloatConstant { value: bits }),
        (
            903,
            OperationKind::StructuralScalarFieldStore {
                destination: root,
                path: Vec::new(),
                field: structural_field_id(1),
                value: value_id(902),
                range_obligation: None,
            },
        ),
        (904, read()),
    ] {
        machine.blocks[0].operations.push(Operation {
            id: operation_id(identity),
            static_reach_binding: None,
            suspension_crossing: None,
            result: if identity == 903 {
                OperationResult::Unit
            } else {
                OperationResult::Scalar(ValueDeclaration {
                    id: value_id(identity),
                    scalar_type,
                    qualifications: Default::default(),
                })
            },
            kind,
        });
    }
    module
}

#[test]
fn ieee_float_field_module_and_proof_roundtrip_replay_store_and_capture_facts() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let original = fixture(format);
        let module_bytes = encode_module(&original).expect("field module encodes");
        let module = decode_module(&module_bytes).expect("field module decodes without source");
        assert_eq!(module, original);
        assert_eq!(encode_module(&module).unwrap(), module_bytes);
        let questions = reconstruct_terminal_obligations(&module).unwrap();
        assert_eq!(questions.obligations().len(), 2);
        let validated = terminal_verifier::validate_module(&module).unwrap();
        let context = validated.value_context(&module.machines[0]).unwrap();
        let proof = ProofBundle {
            evidence: questions
                .obligations()
                .iter()
                .map(|question| {
                    let conclusion = question.obligation.proposition.clone();
                    let index = question
                        .semantic_axioms
                        .iter()
                        .position(|fact| fact == &conclusion);
                    let proof = if let Some(index) = index {
                        ProofNode {
                            conclusion: conclusion.clone(),
                            rule: ProofRule::SemanticAxiom { index },
                        }
                    } else {
                        let equations = question
                            .semantic_axioms
                            .iter()
                            .filter(|fact| matches!(fact, Proposition::Equal(..)))
                            .collect::<Vec<_>>();
                        let denotation = check_value_equality_denotation(
                            &context,
                            &conclusion,
                            equations.iter().copied(),
                        )
                        .unwrap();
                        let premise = if denotation == Proposition::Truth {
                            ProofNode {
                                conclusion: Proposition::Truth,
                                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                            }
                        } else {
                            let index = question
                                .semantic_axioms
                                .iter()
                                .position(|fact| {
                                    check_value_equality_denotation(
                                        &context,
                                        fact,
                                        equations.iter().copied(),
                                    )
                                    .as_ref()
                                        == Ok(&denotation)
                                })
                                .expect("captured entry predicate");
                            ProofNode {
                                conclusion: question.semantic_axioms[index].clone(),
                                rule: ProofRule::SemanticAxiom { index },
                            }
                        };
                        ProofNode {
                            conclusion: conclusion.clone(),
                            rule: ProofRule::ValueEqualityTransport {
                                premise: Box::new(premise),
                                equalities: question
                                    .semantic_axioms
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, fact)| matches!(fact, Proposition::Equal(..)))
                                    .map(|(index, fact)| ProofNode {
                                        conclusion: fact.clone(),
                                        rule: ProofRule::SemanticAxiom { index },
                                    })
                                    .collect(),
                            },
                        }
                    };
                    ObligationEvidence {
                        obligation: question.obligation.id,
                        route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                            identity: EvidenceIdentity::new(question.obligation.id.get()).unwrap(),
                            proof_system_marker: ProofSystemMarker::CURRENT,
                            proof,
                        }),
                    }
                })
                .collect(),
            ..ProofBundle::default()
        };
        let proof_bytes = encode_proof_bundle(&proof).unwrap();
        let decoded_proof = decode_proof_bundle(&proof_bytes).unwrap();
        assert_eq!(decoded_proof, proof);
        assert_eq!(encode_proof_bundle(&decoded_proof).unwrap(), proof_bytes);
        verify_module(&module, &decoded_proof, &AdmissionProfile::default())
            .expect("only decoded module and decoded evidence authorize the contract");

        let mut wrong_evidence = decoded_proof;
        let EvidenceRoute::CertificateDerived(envelope) = &mut wrong_evidence.evidence[0].route
        else {
            unreachable!()
        };
        envelope.proof.rule = ProofRule::SemanticAxiom { index: 99 };
        let wrong_evidence =
            decode_proof_bundle(&encode_proof_bundle(&wrong_evidence).unwrap()).unwrap();
        assert!(verify_module(&module, &wrong_evidence, &AdmissionProfile::default()).is_err());
        for wrong_format in [false, true] {
            let mut changed = module.clone();
            let Proposition::ScalarIeeeFloatComparison {
                left: ScalarTerm::IeeeFloatField { path, format, .. },
                ..
            } = &mut changed.machines[0].contract.ensures[1].proposition
            else {
                unreachable!()
            };
            if wrong_format {
                *format = if *format == IeeeFloatFormat::Binary32 {
                    IeeeFloatFormat::Binary64
                } else {
                    IeeeFloatFormat::Binary32
                };
            } else {
                path[0] = CanonicalStructuralPathSegment::Field(structural_field_id(99));
            }
            assert!(
                encode_module(&changed).is_err(),
                "changed field format/path"
            );
        }
    }
}
