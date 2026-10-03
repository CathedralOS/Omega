use super::*;
use terminal_psi::{ByteSequencePredicate, ByteSequencePredicateSet};

fn restrict_field(module: &mut TerminalModule, predicate: ByteSequencePredicate) {
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(predicate);
    fields[0].field_type = StructuralFieldType::PredicateByteSequence {
        carrier: ByteSequenceCarrier::BoundedOwned { capacity: 3 },
        predicates,
    };
}

fn owned_record_destination() -> (TerminalModule, ProofBundle) {
    let (mut module, proof) = fixture(StructuralAccess::MutableBorrow);
    restrict_field(&mut module, ByteSequencePredicate::ValidUtf8);
    let machine = &mut module.machines[0];
    machine.structural_parameters.clear();
    machine.structural_places[0].kind = StructuralPlaceKind::OperationResult {
        producer: id(5),
        structural_type: id(1),
    };
    machine.blocks[0].operations.insert(
        0,
        Operation {
            id: id(5),
            static_reach_binding: None,
            suspension_crossing: None,
            kind: OperationKind::EstablishZeroedStructural {},
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: id(1),
                structural_type: id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
        },
    );
    (module, proof)
}

#[test]
fn owned_record_byte_field_replacement_retains_predicates_and_capacity() {
    let (module, proof) = owned_record_destination();
    verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("existing owned home retains exact field predicates and capacity proof");
}

#[test]
fn zero_establishment_cannot_supply_a_nonempty_attached_field() {
    let (mut module, _) = owned_record_destination();
    restrict_field(&mut module, ByteSequencePredicate::NonEmpty);
    for attachment in [None, Some(id(1))] {
        module.machines[0].attachment = attachment;
        assert!(
            matches!(
                validate_module(&module),
                Err(ModuleError::ZeroedStructuralResultMismatch(_))
            ),
            "zeroing must reject at the establishing operation, independent of attachment"
        );
    }
}

#[test]
fn owned_record_byte_field_replacement_rejects_unestablished_or_forged_storage() {
    let (valid, _) = owned_record_destination();
    for corruption in 0..5 {
        let mut module = valid.clone();
        match corruption {
            0 => module.machines[0].blocks[0].operations.rotate_left(1),
            1 => {
                let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
                    &mut module.machines[0].blocks[0].operations[2].kind
                else {
                    panic!("literal source")
                };
                bytes[0] = 255;
            }
            2 => {
                let OperationResult::Structural(result) =
                    &mut module.machines[0].blocks[0].operations[0].result
                else {
                    panic!("owned destination")
                };
                result.multiplicity = StructuralMultiplicity::Affine;
            }
            3 => {
                module.machines[0].structural_places[0].kind =
                    StructuralPlaceKind::OperationResult {
                        producer: id(99),
                        structural_type: id(1),
                    };
            }
            4 => {
                let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape
                else {
                    panic!("record destination")
                };
                fields[0].field_type = StructuralFieldType::PredicateByteSequence {
                    carrier: ByteSequenceCarrier::BorrowedView {
                        access: Some(StructuralAccess::SharedBorrow),
                    },
                    predicates: fields[0].field_type.byte_predicates(),
                };
            }
            _ => unreachable!(),
        }
        assert!(
            validate_module(&module).is_err(),
            "corruption {corruption} accepted"
        );
    }
}

#[test]
fn ascii_preserving_writes_expire_literal_ranges_but_not_captured_reads() {
    for before_read in [true, false] {
        let mut module = super::indexed::literal_read_fixture();
        restrict_field(&mut module, ByteSequencePredicate::ValidUtf8);
        module.machines[0].blocks[0].operations[4].kind = OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(65),
        };
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
        module.machines[0].blocks[0]
            .operations
            .insert(if before_read { 5 } else { 6 }, write);
        validate_module(&module).expect("ASCII-preserving stores retain the carrier rule");
        let facts = super::indexed::read_facts(&module);
        for bound in super::indexed::read_bounds(b'E', b'L') {
            assert_eq!(
                facts.contains(&bound),
                !before_read,
                "a pre-read write changes contents; a post-read write leaves the SSA snapshot intact"
            );
        }
    }
}

#[test]
fn restricted_field_replacement_replays_literal_bytes() {
    let (mut module, _) = fixture(StructuralAccess::MutableBorrow);
    restrict_field(&mut module, ByteSequencePredicate::ValidUtf8);
    validate_module(&module).expect("ASCII literal preserves UTF-8 field");
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    bytes[0] = 255;
    validate_module(&module)
        .expect_err("equal-length invalid UTF-8 cannot satisfy restricted field replacement");
}

#[test]
fn literal_qualification_cannot_replace_independent_octet_checking() {
    let (mut module, _) = fixture(StructuralAccess::MutableBorrow);
    module.structural_domains.push(StructuralDomainDeclaration {
        id: id(1),
        semantic_domain: id(1),
        identity: "test::Utf8".into(),
        carrier: id(2),
        byte_predicate: Some(ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: Vec::new(),
    });
    let OperationKind::EstablishByteSequenceLiteral { qualifications, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    qualifications.push(id(1));
    validate_module(&module).expect("exact literal bytes discharge the primitive domain");
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    bytes[0] = 255;
    let error = validate_module(&module)
        .expect_err("forged literal membership must reject before calls consume it");
    assert!(
        matches!(
            error,
            terminal_verifier::ModuleError::InvalidByteSequenceLiteralQualifications(_)
        ),
        "{error:?}"
    );
}

/// Append a field length observation, a `u8` constant and an indexed store of
/// that constant at index zero behind the fixture's literal replacement.
fn indexed_store(module: &mut TerminalModule, byte: u8) {
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
                scalar_type: ScalarType::Integer(byte_type()),
            }),
            kind: OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(u128::from(byte)),
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
}

fn byte_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
}

fn byte(raw: u8) -> ScalarTerm {
    ScalarTerm::integer(byte_type(), IntegerValue::Unsigned(u128::from(raw))).unwrap()
}

fn stored() -> ScalarTerm {
    ScalarTerm::value(id(4), ScalarType::Integer(byte_type()))
}

fn store_question(module: &TerminalModule) -> terminal_verifier::ReconstructedOperationObligation {
    reconstruct_operation_obligations(module)
        .unwrap()
        .into_iter()
        .find(|question| question.obligation.id == id(2))
        .unwrap()
}

/// A certificate for the store's goal: the bound leg from the literal extent
/// and each class leg from the stored constant, conjoined in goal order.
/// `claimed` is the constant the class legs cite, which a test may set apart
/// from the constant actually stored.
fn store_certificate(module: &TerminalModule, claimed: u8) -> ObligationEvidence {
    let question = store_question(module);
    let axiom = |conclusion: Proposition| ProofNode {
        rule: ProofRule::SemanticAxiom {
            index: question
                .semantic_axioms
                .iter()
                .position(|candidate| candidate == &conclusion)
                .unwrap_or(usize::MAX),
        },
        conclusion,
    };
    let index = ScalarTerm::value(id(2), ScalarType::Integer(integer()));
    let length = ScalarTerm::value(id(3), ScalarType::Integer(integer()));
    let bound = ProofNode {
        conclusion: Proposition::LessThan(index.clone(), length.clone()),
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
    let leg = |relation: Proposition, endpoint: usize| {
        let conclusion = match &relation {
            Proposition::LessOrEqual(_, upper) if endpoint == 0 => {
                Proposition::LessOrEqual(stored(), upper.clone())
            }
            Proposition::LessOrEqual(lower, _) => Proposition::LessOrEqual(lower.clone(), stored()),
            _ => unreachable!(),
        };
        ProofNode {
            conclusion,
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(ProofNode {
                    conclusion: relation,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
                equality: Box::new(axiom(Proposition::Equal(stored(), byte(claimed)))),
                endpoint,
            },
        }
    };
    let mut legs = vec![bound];
    let Proposition::Conjunction(goal) = &question.obligation.proposition else {
        panic!("a restricted field's store owes class legs")
    };
    for conjunct in &goal[1..] {
        legs.push(match conjunct {
            Proposition::LessOrEqual(_, upper) if upper == &byte(0x7f) => {
                leg(Proposition::LessOrEqual(byte(claimed), byte(0x7f)), 0)
            }
            Proposition::LessOrEqual(lower, _) if lower == &byte(1) => {
                leg(Proposition::LessOrEqual(byte(1), byte(claimed)), 1)
            }
            other => panic!("unexpected class leg {other:?}"),
        });
    }
    ObligationEvidence {
        obligation: id(2),
        route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: id(2),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode {
                conclusion: question.obligation.proposition,
                rule: ProofRule::ConjunctionIntroduction(legs),
            },
        }),
    }
}

fn restricted_store(predicate: ByteSequencePredicate, stored_byte: u8) -> TerminalModule {
    let (mut module, _) = fixture(StructuralAccess::MutableBorrow);
    restrict_field(&mut module, predicate);
    indexed_store(&mut module, stored_byte);
    module
}

fn with_store_evidence(module: &TerminalModule, claimed: u8) -> ProofBundle {
    let (_, mut bundle) = fixture(StructuralAccess::MutableBorrow);
    bundle.evidence.push(store_certificate(module, claimed));
    bundle
}

#[test]
fn utf8_field_store_owes_an_ascii_byte_beside_its_bound() {
    let module = restricted_store(ByteSequencePredicate::ValidUtf8, b'A');
    validate_module(&module).expect("an ASCII carrier admits the store's custody");
    let question = store_question(&module);
    assert_eq!(
        question.obligation.proposition,
        Proposition::Conjunction(vec![
            Proposition::LessThan(
                ScalarTerm::value(id(2), ScalarType::Integer(integer())),
                ScalarTerm::value(id(3), ScalarType::Integer(integer())),
            ),
            Proposition::LessOrEqual(stored(), byte(0x7f)),
        ])
    );
    let bundle = with_store_evidence(&module, b'A');
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("an ASCII byte over an all-ASCII carrier keeps UTF-8");

    // The bound alone is the goal a plain carrier owes; it no longer
    // discharges a restricted field's store.
    let (mut plain, _) = fixture(StructuralAccess::MutableBorrow);
    indexed_store(&mut plain, b'A');
    let mut bound_only = bundle.clone();
    let EvidenceRoute::CertificateDerived(envelope) = &mut bound_only.evidence[1].route else {
        unreachable!()
    };
    let ProofRule::ConjunctionIntroduction(legs) = envelope.proof.rule.clone() else {
        unreachable!()
    };
    envelope.proof = legs[0].clone();
    assert_eq!(
        store_question(&plain).obligation.proposition,
        envelope.proof.conclusion
    );
    assert!(verify_module(&module, &bound_only, &AdmissionProfile::default()).is_err());

    // Missing evidence is rejected, not assumed.
    let mut missing = bundle;
    missing.evidence.pop();
    assert!(verify_module(&module, &missing, &AdmissionProfile::default()).is_err());
}

#[test]
fn utf8_field_store_of_a_non_ascii_byte_has_no_certificate() {
    let module = restricted_store(ByteSequencePredicate::ValidUtf8, 0xff);
    validate_module(&module).expect("custody holds; the byte's class is the goal's question");
    // Citing an ASCII witness for a non-ASCII constant fails the equation;
    // citing the real constant fails the closed relation `255 <= 127`.
    for claimed in [b'A', 0xff] {
        let bundle = with_store_evidence(&module, claimed);
        assert!(
            verify_module(&module, &bundle, &AdmissionProfile::default()).is_err(),
            "claimed {claimed}"
        );
    }
}

#[test]
fn utf8_field_store_requires_an_all_ascii_carrier_on_every_path() {
    // `éX` is valid UTF-8 of the same extent; an ASCII byte over its first
    // octet would leave a stray continuation byte.
    let mut module = restricted_store(ByteSequencePredicate::ValidUtf8, b'A');
    let OperationKind::EstablishByteSequenceLiteral { bytes, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *bytes = "\u{e9}X".as_bytes().to_vec();
    assert_eq!(
        validate_module(&module).err(),
        Some(terminal_verifier::ModuleError::InvalidStructuralByteSequenceFieldAccess(id(7)))
    );

    // The entry state proves only the declaration, not ASCII contents.
    let mut entry = restricted_store(ByteSequencePredicate::ValidUtf8, b'A');
    entry.machines[0].blocks[0].operations.remove(3);
    assert_eq!(
        validate_module(&entry).err(),
        Some(terminal_verifier::ModuleError::InvalidStructuralByteSequenceFieldAccess(id(7)))
    );

    // Another indexed store into the identical field keeps the class: each
    // one proves its own byte ASCII.
    let mut chained = restricted_store(ByteSequencePredicate::ValidUtf8, b'A');
    let mut second = chained.machines[0].blocks[0].operations[6].clone();
    second.id = id(8);
    let OperationKind::StructuralByteSequenceFieldByteStore { obligation, .. } = &mut second.kind
    else {
        unreachable!()
    };
    *obligation = id(3);
    chained.machines[0].blocks[0].operations.push(second);
    validate_module(&chained).expect("chained ASCII stores keep an ASCII carrier");
    let questions = reconstruct_operation_obligations(&chained).unwrap();
    let second_goal = &questions
        .iter()
        .find(|question| question.obligation.id == id(3))
        .unwrap()
        .obligation
        .proposition;
    assert!(matches!(second_goal, Proposition::Conjunction(legs)
            if legs[1] == Proposition::LessOrEqual(stored(), byte(0x7f))));
}

#[test]
fn ascii_field_store_needs_no_carrier_replay() {
    // A field that declares `ascii_only` is all-ASCII by its own invariant,
    // so even the entry state supports a UTF-8-preserving store.
    let (mut module, _) = fixture(StructuralAccess::MutableBorrow);
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    let mut predicates = ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::AsciiOnly);
    predicates.insert(ByteSequencePredicate::ValidUtf8);
    fields[0].field_type = StructuralFieldType::PredicateByteSequence {
        carrier: ByteSequenceCarrier::BoundedOwned { capacity: 3 },
        predicates,
    };
    indexed_store(&mut module, b'A');
    module.machines[0].blocks[0].operations.remove(3);
    validate_module(&module).expect("the declared class is the carrier premise");
    assert_eq!(
        store_question(&module).obligation.proposition,
        Proposition::Conjunction(vec![
            Proposition::LessThan(
                ScalarTerm::value(id(2), ScalarType::Integer(integer())),
                ScalarTerm::value(id(3), ScalarType::Integer(integer())),
            ),
            Proposition::LessOrEqual(stored(), byte(0x7f)),
        ])
    );
}

#[test]
fn nul_free_field_store_owes_a_nonzero_byte() {
    let module = restricted_store(ByteSequencePredicate::NoNul, b'A');
    validate_module(&module).unwrap();
    assert!(matches!(
        &store_question(&module).obligation.proposition,
        Proposition::Conjunction(legs)
            if legs.len() == 2 && legs[1] == Proposition::LessOrEqual(byte(1), stored())
    ));
    verify_module(
        &module,
        &with_store_evidence(&module, b'A'),
        &AdmissionProfile::default(),
    )
    .expect("a nonzero byte keeps a nul-free field nul-free");
    let zero = restricted_store(ByteSequencePredicate::NoNul, 0);
    assert!(
        verify_module(
            &zero,
            &with_store_evidence(&zero, 0),
            &AdmissionProfile::default()
        )
        .is_err()
    );
}

#[test]
fn nonempty_field_store_owes_only_its_bound() {
    // A store keeps the extent, so `non_empty` adds no leg.
    let module = restricted_store(ByteSequencePredicate::NonEmpty, 0);
    validate_module(&module).unwrap();
    assert!(matches!(
        store_question(&module).obligation.proposition,
        Proposition::LessThan(..)
    ));
}

#[test]
fn attached_borrow_keeps_its_established_nonempty_field() {
    let (mut module, proof) = fixture(StructuralAccess::MutableBorrow);
    restrict_field(&mut module, ByteSequencePredicate::NonEmpty);
    module.machines[0].attachment = Some(id(1));
    // This fixture receives established storage and replaces its field with
    // nonempty bytes; attachment metadata does not zero that incoming value.
    verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("an established attached borrow is not zero initialization");
}
