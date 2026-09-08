use super::*;
use semantic_vocabulary::{PsiSemanticId, StructuralFieldId};
use terminal_psi::{BindingRelevance, ByteSequenceCarrier};

#[path = "structural_byte_sequence_store/indexed.rs"]
mod indexed;

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).unwrap()
}
fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}
fn count(raw: u128) -> ScalarTerm {
    ScalarTerm::integer(integer(), IntegerValue::Unsigned(raw)).unwrap()
}
fn measured() -> ScalarTerm {
    ScalarTerm::value(id(1), ScalarType::Integer(integer()))
}

fn fixture(access: StructuralAccess) -> (TerminalModule, ProofBundle) {
    let mut module = unit_module();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: id(1),
            identity: "test::Buffer".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(1),
                    identity: "out".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::ByteSequence(
                        ByteSequenceCarrier::BoundedOwned { capacity: 3 },
                    ),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: id(2),
            identity: "test::View".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        },
    ];
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(1),
            position: 0,
            is_self: false,
            structural_type: id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(1),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: id(2),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 0,
                structural_type: id(2),
            },
        },
    ];
    machine.blocks[0].operations = vec![
        Operation {
            id: id(1),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(2),
                scalar_type: ScalarType::Integer(integer()),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        },
        Operation {
            id: id(2),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: id(2),
                bytes: b"XXX".to_vec(),
            },
        },
        Operation {
            id: id(3),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(1),
                scalar_type: ScalarType::Integer(integer()),
            }),
            kind: OperationKind::ByteSequenceLength { source: id(2) },
        },
        Operation {
            id: id(4),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralByteSequenceFieldStore {
                destination: id(1),
                path: Vec::new(),
                field: id(1),
                source: id(2),
                length: id(1),
                obligation: id(1),
            },
        },
    ];
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::LessOrEqual(measured(), count(3)),
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(ProofNode {
                            conclusion: Proposition::LessOrEqual(count(3), count(3)),
                            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                        }),
                        equality: Box::new(ProofNode {
                            conclusion: Proposition::Equal(measured(), count(3)),
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        endpoint: 0,
                    },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    (module, bundle)
}

#[test]
fn inline_literal_replacement_reconstructs_and_verifies_capacity() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let (module, bundle) = fixture(access);
        validate_module(&module).unwrap();
        let obligations = reconstruct_operation_obligations(&module).unwrap();
        assert_eq!(obligations.len(), 1);
        assert_eq!(
            obligations[0].obligation.proposition,
            Proposition::LessOrEqual(measured(), count(3))
        );
        assert!(obligations[0].canonical_certificate);
        verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    }
}

#[test]
fn changed_capacity_or_literal_rejects_stale_proof() {
    let (module, bundle) = fixture(StructuralAccess::MutableBorrow);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    for change_capacity in [false, true] {
        let mut changed = module.clone();
        if change_capacity {
            let StructuralTypeShape::Record { fields } = &mut changed.structural_types[0].shape
            else {
                unreachable!()
            };
            fields[0].field_type =
                StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity: 2,
                });
        } else {
            let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
                &mut changed.machines[0].blocks[0].operations[1].kind
            else {
                unreachable!()
            };
            bytes.push(b'X');
        }
        validate_module(&changed).unwrap();
        assert!(verify_module(&changed, &bundle, &AdmissionProfile::default()).is_err());
    }
}

#[test]
fn field_source_length_and_access_mutations_reject() {
    let (module, bundle) = fixture(StructuralAccess::MutableBorrow);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    for mutation in 0..8 {
        let mut changed = module.clone();
        let machine = &mut changed.machines[0];
        match mutation {
            0 => machine.structural_parameters[0].access = StructuralAccess::SharedBorrow,
            1 => {
                let OperationKind::StructuralByteSequenceFieldStore { length, .. } =
                    &mut machine.blocks[0].operations[3].kind
                else {
                    unreachable!()
                };
                *length = id(2);
            }
            2 => {
                machine.blocks[0].operations.swap(2, 3);
            }
            3 => {
                machine.blocks[0].operations.remove(2);
            }
            4 => {
                let OperationKind::StructuralByteSequenceFieldStore { path, .. } =
                    &mut machine.blocks[0].operations[3].kind
                else {
                    unreachable!()
                };
                path.push(terminal_psi::StructuralPathSegment::Field("missing".into()));
            }
            5 => {
                let StructuralTypeShape::Record { fields } = &mut changed.structural_types[0].shape
                else {
                    unreachable!()
                };
                fields[0].field_type = StructuralFieldType::Scalar(ScalarType::Integer(integer()));
            }
            6 => {
                let OperationKind::StructuralByteSequenceFieldStore { source, .. } =
                    &mut machine.blocks[0].operations[3].kind
                else {
                    unreachable!()
                };
                *source = id(1);
            }
            7 => {
                let OperationKind::StructuralByteSequenceFieldStore { field, .. } =
                    &mut machine.blocks[0].operations[3].kind
                else {
                    unreachable!()
                };
                *field = id::<StructuralFieldId>(2);
            }
            _ => unreachable!(),
        }
        assert!(validate_module(&changed).is_err(), "mutation {mutation}");
    }
}

#[test]
fn second_literal_cannot_supply_another_sources_length() {
    let (mut module, bundle) = fixture(StructuralAccess::MutableBorrow);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: id(3),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 1,
            structural_type: id(2),
        },
    });
    machine.blocks[0].operations.insert(
        2,
        Operation {
            id: id(5),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: id(3),
                bytes: b"Z".to_vec(),
            },
        },
    );
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let OperationKind::StructuralByteSequenceFieldStore { source, .. } =
        &mut machine_store(&mut module).kind
    else {
        unreachable!()
    };
    *source = id(3);
    assert!(validate_module(&module).is_err());
    let OperationKind::ByteSequenceLength { source } =
        &mut module.machines[0].blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    *source = id(3);
    validate_module(&module).unwrap();
    let obligations = reconstruct_operation_obligations(&module).unwrap();
    assert!(
        obligations[0]
            .semantic_axioms
            .contains(&Proposition::Equal(measured(), count(1)))
    );
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}

fn machine_store(module: &mut TerminalModule) -> &mut Operation {
    module.machines[0].blocks[0]
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::StructuralByteSequenceFieldStore { .. }
            )
        })
        .unwrap()
}

#[test]
fn owned_source_and_qualified_destination_are_not_silently_accepted() {
    let (mut module, _) = fixture(StructuralAccess::MutableBorrow);
    let machine = &mut module.machines[0];
    machine.structural_places[1].kind = StructuralPlaceKind::Parameter {
        position: 1,
        is_self: false,
    };
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: id(2),
            position: 1,
            is_self: false,
            structural_type: id(2),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.blocks[0].operations.remove(1);
    validate_module(&module).unwrap();
    module.machines[0].structural_parameters[1].access = StructuralAccess::Owned;
    assert!(validate_module(&module).is_err());
    module.machines[0].structural_parameters[1].access = StructuralAccess::SharedBorrow;
    module.structural_domains.push(StructuralDomainDeclaration {
        id: id(1),
        semantic_domain: id(1),
        identity: "test::QualifiedBuffer".into(),
        carrier: id(1),
        content_projection: None,
    });
    module.machines[0].structural_parameters[0]
        .qualifications
        .push(id(1));
    assert!(validate_module(&module).is_err());
}

#[test]
fn cross_block_literal_requires_dominating_establishment() {
    let (mut module, bundle) = fixture(StructuralAccess::MutableBorrow);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let machine = &mut module.machines[0];
    let operations = machine.blocks[0].operations.split_off(2);
    let terminator = machine.blocks[0].terminator.clone();
    machine.blocks[0].terminator = Terminator::Jump {
        structural_arguments: Vec::new(),
        edge: id(901),
        target: id(901),
        arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks.push(Block {
        structural_parameters: Vec::new(),
        id: id(901),
        parameters: Vec::new(),
        operations,
        terminator,
    });
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("literal established in predecessor supplies the exact later bound");

    let mut late = module.clone();
    let literal = late.machines[0].blocks[0].operations.remove(1);
    late.machines[0].blocks[1].operations.push(literal);
    assert!(validate_module(&late).is_err());

    let machine = &mut module.machines[0];
    let literal = machine.blocks[0].operations.remove(1);
    machine.parameters.push(ValueDeclaration {
        id: id(3),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(3),
        when_true: SuccessorEdge {
            structural_arguments: Vec::new(),
            edge: id(901),
            target: id(902),
            arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            structural_arguments: Vec::new(),
            edge: id(902),
            target: id(901),
            arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    machine.blocks.push(Block {
        structural_parameters: Vec::new(),
        id: id(902),
        parameters: Vec::new(),
        operations: vec![literal],
        terminator: Terminator::Jump {
            structural_arguments: Vec::new(),
            edge: id(903),
            target: id(901),
            arguments: Vec::new(),
            residual_affine_discards: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    });
    assert!(
        validate_module(&module).is_err(),
        "one sibling's establishment cannot authorize the unestablished incoming path"
    );
}

#[path = "structural_byte_sequence_store/views.rs"]
mod views;
