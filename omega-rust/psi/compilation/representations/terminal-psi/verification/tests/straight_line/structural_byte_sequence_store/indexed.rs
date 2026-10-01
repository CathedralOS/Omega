use super::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, IntegerSign, IntegerType, IntegerValue,
    ObligationEvidence, Operation, OperationKind, OperationResult, PrimitiveJudgment, ProofBundle,
    ProofNode, ProofRule, ProofSystemMarker, Proposition, ScalarTerm, ScalarType, StructuralAccess,
    TerminalModule, ValueDeclaration, count, fixture, id, integer,
    reconstruct_operation_obligations, validate_module, verify_module,
};

fn nest_byte_field_root(module: &mut TerminalModule) -> Vec<terminal_psi::StructuralPathSegment> {
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };
    for (array_identity, record_identity, child, field_identity, name) in
        [(3, 4, 1, 2, "slots"), (5, 6, 4, 3, "racks")]
    {
        module.structural_types.extend([
            StructuralTypeDeclaration {
                id: id(array_identity),
                identity: format!("test::array{array_identity}"),
                shape: StructuralTypeShape::FixedArray {
                    element: id(child),
                    length: 2,
                },
            },
            StructuralTypeDeclaration {
                id: id(record_identity),
                identity: format!("test::record{record_identity}"),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: id(field_identity),
                        identity: name.into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(id(array_identity)),
                    }],
                },
            },
        ]);
    }
    module.machines[0].structural_parameters[0].structural_type = id(6);
    vec![
        "racks".into(),
        StructuralPathSegment::FixedIndex(1),
        "slots".into(),
        StructuralPathSegment::FixedIndex(0),
    ]
}

#[test]
fn nested_static_byte_field_observations_replay_every_projection() {
    use terminal_psi::StructuralPathSegment;
    let (mut module, _) = fixture(StructuralAccess::SharedBorrow);
    let path = nest_byte_field_root(&mut module);
    let machine = &mut module.machines[0];
    machine.structural_places.truncate(1);
    machine.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(1),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(1),
            scalar_type: ScalarType::Integer(integer()),
        }),
        kind: OperationKind::StructuralByteSequenceFieldLength {
            source: id(1),
            path,
            field: id(1),
        },
    }];
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("length observes the exact nested field without borrowing or writing it");
    for mutation in 0..7 {
        let mut changed = module.clone();
        let OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
        } = &mut changed.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        match mutation {
            0 => path[1] = StructuralPathSegment::FixedIndex(2),
            1 => path[3] = StructuralPathSegment::FixedIndex(2),
            2 => path[2] = "missing".into(),
            3 => {
                path[1] = StructuralPathSegment::RuntimeIndex {
                    index: id(1),
                    obligation: id(1),
                }
            }
            4 => path.insert(0, StructuralPathSegment::Referent),
            5 => *field = id(2),
            6 => *source = id(99),
            _ => unreachable!(),
        }
        assert!(
            validate_module(&changed).is_err(),
            "projection mutation {mutation}"
        );
    }

    // Observation support does not extend the independently bounded store
    // contract. All source values and proofs stay unchanged; only the exact
    // carrier path gains the nested records and fixed arrays above.
    let (mut stores, _) = indexed_fixture(StructuralAccess::MutableBorrow);
    let path = nest_byte_field_root(&mut stores);
    for operation in &mut stores.machines[0].blocks[0].operations {
        match &mut operation.kind {
            OperationKind::StructuralByteSequenceFieldStore {
                path: destination, ..
            }
            | OperationKind::StructuralByteSequenceFieldByteStore {
                path: destination, ..
            }
            | OperationKind::StructuralByteSequenceFieldLength {
                path: destination, ..
            } => {
                *destination = path.clone();
            }
            _ => {}
        }
    }
    assert!(
        validate_module(&stores).is_err(),
        "deep stores remain outside their contract"
    );
}

fn indexed_fixture(access: StructuralAccess) -> (TerminalModule, ProofBundle) {
    let (mut module, mut bundle) = fixture(access);
    module.machines[0].blocks[0].operations.extend([
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(5),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
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
            static_reach_binding: None,
            suspension_crossing: None,
            id: id(6),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
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
            static_reach_binding: None,
            suspension_crossing: None,
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
    assert!(
        questions[0]
            .semantic_axioms
            .contains(&Proposition::LessOrEqual(
                ScalarTerm::value(id(3), ScalarType::Integer(integer())),
                count(3),
            ))
    );
    assert!(verify_module(&module, &bundle, &AdmissionProfile::default()).is_err());

    // A borrowed descriptor has a live length but no declaration-owned capacity.
    let super::StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape
    else {
        unreachable!()
    };
    fields[0].field_type =
        super::StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::MutableBorrow),
        });
    let questions = reconstruct_operation_obligations(&module).unwrap();
    assert!(
        !questions[0]
            .semantic_axioms
            .contains(&Proposition::LessOrEqual(
                ScalarTerm::value(id(3), ScalarType::Integer(integer())),
                count(3),
            ))
    );
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

pub(super) fn literal_read_fixture() -> TerminalModule {
    let (mut module, _) = indexed_fixture(StructuralAccess::MutableBorrow);
    let machine = &mut module.machines[0];
    // The index is a runtime parameter, not the literal zero from the store fixture.
    let index = machine.blocks[0]
        .operations
        .remove(0)
        .result
        .scalar()
        .unwrap();
    machine.parameters.push(index);
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut machine.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *bytes = b"HEL".to_vec();
    let read = &mut machine.blocks[0].operations[5];
    read.result = OperationResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: id(5),
        scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    });
    read.kind = OperationKind::StructuralByteSequenceFieldRead {
        source: id(1),
        path: Vec::new(),
        field: id(1),
        index: id(2),
        length: id(3),
        obligation: id(2),
    };
    // Inspect facts at an actual subsequent operation obligation, not the helper.
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: id(8),
        result: OperationResult::Unit,
        kind: OperationKind::StructuralByteSequenceFieldByteStore {
            destination: id(1),
            path: Vec::new(),
            field: id(1),
            index: id(2),
            value: id(5),
            length: id(3),
            obligation: id(3),
        },
    });
    module
}

pub(super) fn read_bounds(minimum: u8, maximum: u8) -> [Proposition; 2] {
    let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let value = ScalarTerm::value(id(5), ScalarType::Integer(byte));
    [
        Proposition::LessOrEqual(
            ScalarTerm::integer(byte, IntegerValue::Unsigned(minimum.into())).unwrap(),
            value.clone(),
        ),
        Proposition::LessOrEqual(
            value,
            ScalarTerm::integer(byte, IntegerValue::Unsigned(maximum.into())).unwrap(),
        ),
    ]
}

pub(super) fn read_facts(module: &TerminalModule) -> Vec<Proposition> {
    reconstruct_operation_obligations(module)
        .unwrap()
        .into_iter()
        .find(|question| question.obligation.id == id(3))
        .unwrap()
        .semantic_axioms
}

#[test]
fn runtime_field_byte_read_reconstructs_literal_bounds_without_discharging_index() {
    let module = literal_read_fixture();
    let facts = read_facts(&module);
    assert!(
        read_bounds(b'E', b'L')
            .iter()
            .all(|bound| facts.contains(bound))
    );
    let questions = reconstruct_operation_obligations(&module).unwrap();
    let read = questions
        .iter()
        .find(|question| question.obligation.id == id(2))
        .unwrap();
    assert_eq!(
        read.obligation.proposition,
        Proposition::LessThan(
            ScalarTerm::value(id(2), ScalarType::Integer(integer())),
            ScalarTerm::value(id(3), ScalarType::Integer(integer())),
        )
    );
    assert!(
        !read_bounds(b'E', b'L')
            .iter()
            .any(|bound| read.semantic_axioms.contains(bound))
    );

    let mut changed = module;
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut changed.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *bytes = vec![0, 127, 255];
    let facts = read_facts(&changed);
    assert!(
        read_bounds(0, 255)
            .iter()
            .all(|bound| facts.contains(bound))
    );
    assert!(
        !read_bounds(b'E', b'L')
            .iter()
            .any(|bound| facts.contains(bound))
    );
}

#[test]
fn field_byte_read_loses_literal_bounds_without_current_content() {
    for mutation in 0..3 {
        let mut module = literal_read_fixture();
        match mutation {
            0 => {
                module.machines[0].blocks[0].operations.remove(2);
            }
            1 => {
                let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
                    &mut module.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                bytes.clear();
            }
            2 => {
                let mut write = module.machines[0].blocks[0].operations[6].clone();
                write.id = id(9);
                let OperationKind::StructuralByteSequenceFieldByteStore {
                    value, obligation, ..
                } = &mut write.kind
                else {
                    unreachable!()
                };
                *value = id(4);
                *obligation = id(4);
                module.machines[0].blocks[0].operations.insert(5, write);
            }
            _ => unreachable!(),
        }
        let facts = read_facts(&module);
        assert!(
            !read_bounds(b'E', b'L')
                .iter()
                .any(|bound| facts.contains(bound)),
            "mutation {mutation}"
        );
    }
    let mut ambiguous = literal_read_fixture();
    let mut literal = ambiguous.machines[0].blocks[0].operations[0].clone();
    literal.id = id(9);
    ambiguous.machines[0].blocks[0]
        .operations
        .insert(1, literal);
    assert!(validate_module(&ambiguous).is_err());
}

#[test]
fn field_byte_read_rechecks_contents_on_backedges() {
    let mut module = literal_read_fixture();
    let machine = &mut module.machines[0];
    machine.attachment = Some(id(1));
    machine.structural_parameters[0].is_self = true;
    machine.structural_places[0].kind = super::StructuralPlaceKind::Parameter {
        position: machine.structural_parameters[0].position,
        is_self: true,
    };
    let operations = machine.blocks[0].operations.split_off(3);
    let jump = |edge| super::Terminator::Jump {
        edge: id(edge),
        target: id(2),
        arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        structural_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[0].terminator = jump(1);
    machine.blocks.push(super::Block {
        id: id(2),
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator: jump(2),
    });
    // Each read is preceded by a fresh length observation. The byte write on
    // the backedge preserves that length but not the initial literal contents.
    let facts = read_facts(&module);
    assert!(
        !read_bounds(b'E', b'L')
            .iter()
            .any(|bound| facts.contains(bound))
    );
}

#[test]
fn field_byte_read_bounds_follow_call_access_not_callee_names() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut module = literal_read_fixture();
        let mut callee = module.machines[0].clone();
        callee.id = id(901);
        callee.contract.id = id(901);
        callee.entry = id(901);
        callee.parameters.clear();
        callee.structural_parameters[0].place = id(3);
        callee.structural_parameters[0].access = access;
        callee.structural_places = vec![super::StructuralPlaceDeclaration {
            id: id(3),
            kind: super::StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }];
        callee.blocks[0].id = id(901);
        callee.blocks[0].operations.clear();
        if let super::Terminator::ReturnUnit { edge, .. } = &mut callee.blocks[0].terminator {
            *edge = id(901);
        }
        module.machines.push(callee);
        // Observe length after the call: a mutable call retires content facts
        // independently of the separate current-length requirement.
        module.machines[0].blocks[0].operations.insert(
            3,
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: id(9),
                result: OperationResult::Unit,
                kind: OperationKind::CallUnit {
                    callee: id(901),
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    erased_proof_arguments: Vec::new(),
                    structural_arguments: vec![terminal_psi::StructuralArgument {
                        place: id(1),
                        path: Vec::new(),
                        access,
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            },
        );
        let facts = read_facts(&module);
        for bound in read_bounds(b'E', b'L') {
            assert_eq!(
                facts.contains(&bound),
                access == StructuralAccess::SharedBorrow
            );
        }
    }
}
