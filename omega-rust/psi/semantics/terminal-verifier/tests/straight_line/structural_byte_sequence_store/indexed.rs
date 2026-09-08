use super::*;

fn indexed_fixture(access: StructuralAccess) -> (TerminalModule, ProofBundle) {
    let (mut module, mut bundle) = fixture(access);
    module.machines[0].blocks[0].operations.extend([
        Operation {
            id: id(5),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(3),
                scalar_type: ScalarType::Integer(integer()),
            }),
            kind: OperationKind::StructuralByteSequenceFieldLength {
                source: id(1),
                path: Vec::new(),
                field: id(1),
            },
        },
        Operation {
            id: id(6),
            result: OperationResult::Scalar(ValueDeclaration {
                id: id(4),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                ),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(255),
            },
        },
        Operation {
            id: id(7),
            result: OperationResult::Unit,
            kind: OperationKind::StructuralByteSequenceFieldByteStore {
                destination: id(1),
                path: Vec::new(),
                field: id(1),
                index: id(2),
                value: id(4),
                length: id(3),
                obligation: id(2),
            },
        },
    ]);
    let question = reconstruct_operation_obligations(&module)
        .unwrap()
        .remove(1);
    let index = ScalarTerm::value(id(2), ScalarType::Integer(integer()));
    let length = ScalarTerm::value(id(3), ScalarType::Integer(integer()));
    let axiom = |conclusion: Proposition| ProofNode {
        rule: ProofRule::SemanticAxiom {
            index: question
                .semantic_axioms
                .iter()
                .position(|candidate| candidate == &conclusion)
                .unwrap(),
        },
        conclusion,
    };
    let proof = ProofNode {
        conclusion: question.obligation.proposition,
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(ProofNode {
                conclusion: Proposition::LessThan(count(0), length.clone()),
                rule: ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(ProofNode {
                        conclusion: Proposition::LessThan(count(0), count(3)),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                    }),
                    equality: Box::new(axiom(Proposition::Equal(length, count(3)))),
                    endpoint: 1,
                },
            }),
            equality: Box::new(axiom(Proposition::Equal(index, count(0)))),
            endpoint: 0,
        },
    };
    bundle.evidence.push(ObligationEvidence {
        obligation: id(2),
        route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: id(2),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof,
        }),
    });
    (module, bundle)
}

#[test]
fn literal_replacement_supplies_current_length_for_raw_byte_mutation() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let (module, bundle) = indexed_fixture(access);
        verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    }
}

#[test]
fn exact_field_length_operands_and_access_are_independently_rejoined() {
    let (module, bundle) = indexed_fixture(StructuralAccess::MutableBorrow);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    for mutation in 0..10 {
        let mut changed = module.clone();
        let machine = &mut changed.machines[0];
        match mutation {
            0 => machine.structural_parameters[0].access = StructuralAccess::SharedBorrow,
            1 => machine.structural_parameters[0].access = StructuralAccess::Owned,
            2 => {
                machine.blocks[0].operations[4]
                    .result
                    .scalar_mut()
                    .unwrap()
                    .scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap())
            }
            3 => {
                machine.blocks[0].operations[5]
                    .result
                    .scalar_mut()
                    .unwrap()
                    .scalar_type = ScalarType::Integer(integer())
            }
            4 => machine.blocks[0].operations.swap(4, 6),
            mutation => {
                let OperationKind::StructuralByteSequenceFieldByteStore {
                    destination,
                    field,
                    path,
                    length,
                    index,
                    ..
                } = &mut machine.blocks[0].operations[6].kind
                else {
                    unreachable!()
                };
                match mutation {
                    5 => *destination = id(2),
                    6 => *field = id(2),
                    7 => path.push("missing".into()),
                    8 => *length = id(1),
                    9 => *index = id(4),
                    _ => unreachable!(),
                }
            }
        }
        assert!(validate_module(&changed).is_err(), "mutation {mutation}");
    }
}

#[test]
fn saved_field_length_is_stale_after_whole_replacement() {
    let (mut module, _) = indexed_fixture(StructuralAccess::MutableBorrow);
    let mut replacement = module.machines[0].blocks[0].operations[3].clone();
    replacement.id = id(8);
    let OperationKind::StructuralByteSequenceFieldStore { obligation, .. } = &mut replacement.kind
    else {
        unreachable!()
    };
    *obligation = id(3);
    module.machines[0].blocks[0]
        .operations
        .insert(6, replacement);
    assert!(validate_module(&module).is_err());
}

#[test]
fn byte_store_preserves_length_for_a_second_store() {
    let (mut module, mut bundle) = indexed_fixture(StructuralAccess::MutableBorrow);
    let mut store = module.machines[0].blocks[0].operations[6].clone();
    store.id = id(8);
    let OperationKind::StructuralByteSequenceFieldByteStore { obligation, .. } = &mut store.kind
    else {
        unreachable!()
    };
    *obligation = id(3);
    module.machines[0].blocks[0].operations.push(store);
    let mut evidence = bundle.evidence[1].clone();
    evidence.obligation = id(3);
    let EvidenceRoute::CertificateDerived(envelope) = &mut evidence.route else {
        unreachable!()
    };
    envelope.identity = id(3);
    bundle.evidence.push(evidence);
    verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
}

#[test]
fn equal_capacity_is_not_evidence_of_current_live_length() {
    let (mut module, bundle) = indexed_fixture(StructuralAccess::MutableBorrow);
    module.machines[0].blocks[0].operations.remove(3);
    let questions = reconstruct_operation_obligations(&module).unwrap();
    let length = ScalarTerm::value(id(3), ScalarType::Integer(integer()));
    assert!(
        !questions[0]
            .semantic_axioms
            .contains(&Proposition::Equal(length, count(3)))
    );
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn changing_literal_extent_rejects_stale_index_certificate() {
    let (mut module, bundle) = indexed_fixture(StructuralAccess::MutableBorrow);
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    bytes.clear();
    validate_module(&module).unwrap();
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn shared_borrow_can_observe_metadata_but_cannot_mutate_bytes() {
    let (mut module, _) = indexed_fixture(StructuralAccess::MutableBorrow);
    module.machines[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    module.machines[0].blocks[0]
        .operations
        .retain(|operation| operation.id == id(5));
    module.machines[0]
        .structural_places
        .retain(|place| place.id == id(1));
    validate_module(&module).unwrap();
}

#[path = "indexed/graph.rs"]
mod graph;
