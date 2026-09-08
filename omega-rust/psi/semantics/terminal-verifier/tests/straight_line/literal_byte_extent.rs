use super::*;

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn value(raw: u64) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(raw).unwrap(), ScalarType::Integer(integer()))
}

fn count(length: usize) -> ScalarTerm {
    ScalarTerm::integer(integer(), IntegerValue::Unsigned(length as u128)).unwrap()
}

fn fixture(bytes: &[u8]) -> (TerminalModule, ProofBundle) {
    let mut module = unit_module();
    let place = PlaceId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::Bytes".into(),
        shape: StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView),
    });
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 0,
            structural_type,
        },
    });
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: ValueId::new(2).unwrap(),
        scalar_type: ScalarType::Integer(integer()),
    });
    machine.blocks[0].operations = vec![
        Operation {
            id: OperationId::new(1).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: place,
                bytes: bytes.to_vec(),
            },
        },
        Operation {
            id: OperationId::new(2).unwrap(),
            result: OperationResult::Scalar(ValueDeclaration {
                id: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Integer(integer()),
            }),
            kind: OperationKind::ByteSequenceLength { source: place },
        },
    ];
    machine.blocks[0].terminator = Terminator::Return {
        cleanup_actions: Vec::new(),
        edge: EdgeId::new(900).unwrap(),
        value: ValueId::new(1).unwrap(),
    };
    let goal = Proposition::Equal(value(2), count(bytes.len()));
    let obligation = ObligationId::new(1).unwrap();
    machine.contract.ensures.push(ContractClause {
        obligation,
        proposition: goal.clone(),
    });
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: Proposition::Equal(value(2), value(1)),
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: Proposition::Equal(value(1), count(bytes.len())),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                },
            }),
        }],
        ..ProofBundle::default()
    };
    (module, bundle)
}

#[test]
fn literal_length_proves_exact_return_extent() {
    for bytes in [b"XXX".as_slice(), b"", "é".as_bytes(), b"a\0b"] {
        let (module, bundle) = fixture(bytes);
        validate_module(&module).expect("the literal and direct length read are well formed");
        let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
        assert!(
            reconstructed.obligations()[0]
                .semantic_axioms
                .contains(&Proposition::Equal(value(1), count(bytes.len())))
        );
        verify_module(&module, &bundle, &AdmissionProfile::default())
            .expect("exact literal extent proves the return contract");
    }
}

#[test]
fn changed_literal_cannot_reuse_the_old_extent_proof() {
    let (mut module, bundle) = fixture(b"XXX");
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    bytes.push(b'X');
    validate_module(&module).unwrap();
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn malformed_literal_establishments_do_not_supply_extent_facts() {
    let (module, bundle) = fixture(b"XXX");
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    for mutation in 0..4 {
        let mut malformed = module.clone();
        let operations = &mut malformed.machines[0].blocks[0].operations;
        match mutation {
            0 => {
                operations.swap(0, 1);
            }
            1 => {
                operations.remove(0);
            }
            2 => {
                let mut duplicate = operations[0].clone();
                duplicate.id = OperationId::new(3).unwrap();
                operations.insert(1, duplicate);
            }
            3 => {
                operations[1].result = OperationResult::Scalar(ValueDeclaration {
                    id: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                    ),
                });
            }
            _ => unreachable!(),
        }
        assert!(validate_module(&malformed).is_err(), "mutation {mutation}");
    }
}

#[test]
fn shared_parameter_has_no_literal_extent() {
    let (mut module, _) = fixture(b"XXX");
    let machine = &mut module.machines[0];
    machine.structural_places[0].kind = StructuralPlaceKind::Parameter {
        position: 0,
        is_self: false,
    };
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.blocks[0].operations.remove(0);
    validate_module(&module).unwrap();
    let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
    assert!(
        !reconstructed.obligations()[0]
            .semantic_axioms
            .contains(&Proposition::Equal(value(1), count(3)))
    );
}

#[test]
fn redirected_source_and_repeated_reads_keep_exact_place_and_value_identity() {
    let (mut module, bundle) = fixture(b"XXX");
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let machine = &mut module.machines[0];
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: PlaceId::new(2).unwrap(),
        kind: StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: 1,
            structural_type: StructuralTypeId::new(1).unwrap(),
        },
    });
    machine.blocks[0].operations.insert(
        1,
        Operation {
            id: OperationId::new(3).unwrap(),
            result: OperationResult::Unit,
            kind: OperationKind::EstablishByteSequenceLiteral {
                destination: PlaceId::new(2).unwrap(),
                bytes: b"Z".to_vec(),
            },
        },
    );
    machine.blocks[0].operations[2].kind = OperationKind::ByteSequenceLength {
        source: PlaceId::new(2).unwrap(),
    };
    machine.blocks[0].operations.push(Operation {
        id: OperationId::new(4).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(3).unwrap(),
            scalar_type: ScalarType::Integer(integer()),
        }),
        kind: OperationKind::ByteSequenceLength {
            source: PlaceId::new(1).unwrap(),
        },
    });
    machine.blocks[0].operations.push(Operation {
        id: OperationId::new(5).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(4).unwrap(),
            scalar_type: ScalarType::Integer(integer()),
        }),
        kind: OperationKind::ByteSequenceLength {
            source: PlaceId::new(1).unwrap(),
        },
    });
    validate_module(&module).unwrap();
    let reconstructed = reconstruct_terminal_obligations(&module).unwrap();
    let axioms = &reconstructed.obligations()[0].semantic_axioms;
    for (result, length) in [(1, 1), (3, 3), (4, 3)] {
        assert!(axioms.contains(&Proposition::Equal(value(result), count(length))));
    }
    assert!(!axioms.contains(&Proposition::Equal(value(1), count(3))));
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}
