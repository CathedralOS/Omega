use super::{
    AdmissionProfile, CertificateEnvelope, EvidenceIdentity, EvidenceRoute, IntegerSign,
    IntegerType, ModuleError, ObligationEvidence, Operation, OperationKind, OperationResult,
    ProofBundle, ProofNode, ProofRule, ProofSystemMarker, Proposition, ScalarType,
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeShape, TerminalExecution, TerminalExecutionStatus,
    TerminalFuelMeter, TerminalModule, TerminalStructuralValue, Terminator, ValueDeclaration,
    byte_sequence_literal_module, edge_id, encode_module, encode_proof_section, obligation_id,
    operation_id, place_id, structural_type_id, value_id, verify_module,
};
use semantic_vocabulary::{PropositionContext, ScalarTerm, StructuralFieldId, StructuralPlaceKind};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{
    ByteSequenceCarrier, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralOperationResult, StructuralTypeDeclaration,
};

fn count_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn operation(identity: u64, kind: OperationKind, result: OperationResult) -> Operation {
    Operation {
        id: operation_id(identity),
        kind,
        result,
        static_reach_binding: None,
        suspension_crossing: None,
    }
}

fn length(identity: u64, source: u64) -> Operation {
    operation(
        identity,
        OperationKind::ByteSequenceLength {
            source: place_id(source),
        },
        OperationResult::Scalar(ValueDeclaration {
            id: value_id(identity),
            scalar_type: count_type(),
            qualifications: Default::default(),
        }),
    )
}

fn module(left: &[u8], right: &[u8], alias: bool) -> TerminalModule {
    let mut module = byte_sequence_literal_module(left.to_vec());
    module.boundary_machines.clear();
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "owned-concatenation".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: 32,
            }),
        },
        StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "output".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "bytes".into(),
                    relevance: Default::default(),
                    field_type: StructuralFieldType::ByteSequence(
                        ByteSequenceCarrier::BoundedOwned { capacity: 32 },
                    ),
                }],
            },
        },
    ]);
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(5),
            position: 0,
            is_self: false,
            structural_type: structural_type_id(3),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.extend([
        StructuralPlaceDeclaration {
            id: place_id(2),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: 1,
                structural_type: structural_type_id(1),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(3),
            kind: StructuralPlaceKind::OperationResult {
                producer: operation_id(5),
                structural_type: structural_type_id(2),
            },
        },
        StructuralPlaceDeclaration {
            id: place_id(5),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
    ]);
    let operations = &mut machine.blocks[0].operations;
    operations.truncate(1);
    operations.extend([
        operation(
            2,
            OperationKind::EstablishByteSequenceLiteral {
                destination: place_id(2),
                bytes: right.to_vec(),
                qualifications: Vec::new(),
            },
            OperationResult::Unit,
        ),
        length(3, 1),
        length(4, 2),
    ]);
    let store = |identity, source, length| {
        operation(
            identity,
            OperationKind::StructuralByteSequenceFieldStore {
                destination: place_id(5),
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
                source: place_id(source),
                length: value_id(length),
                obligation: obligation_id(identity),
            },
            OperationResult::Unit,
        )
    };
    let left = if alias {
        operations.push(store(8, 1, 3));
        operations.push(operation(
            9,
            OperationKind::StructuralByteSequenceFieldLength {
                source: place_id(5),
                path: Vec::new(),
                field: StructuralFieldId::new(1).unwrap(),
            },
            OperationResult::Scalar(ValueDeclaration {
                id: value_id(9),
                scalar_type: count_type(),
                qualifications: Default::default(),
            }),
        ));
        ByteSequenceOperand::Field {
            source: StructuralArgument {
                place: place_id(5),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
            field: StructuralFieldId::new(1).unwrap(),
            length: value_id(9),
        }
    } else {
        ByteSequenceOperand::Whole {
            source: place_id(1),
            length: value_id(3),
        }
    };
    operations.extend([
        operation(
            5,
            OperationKind::EstablishByteSequenceConcatenation {
                left,
                right: ByteSequenceOperand::Whole {
                    source: place_id(2),
                    length: value_id(4),
                },
                capacity_obligation: obligation_id(5),
            },
            OperationResult::Structural(StructuralOperationResult {
                place: place_id(3),
                structural_type: structural_type_id(2),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            }),
        ),
        length(6, 3),
        store(7, 3, 6),
    ]);
    machine.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(1),
        trivial_affine_discards: Vec::new(),
    };
    module
}

fn proof(module: &TerminalModule) -> ProofBundle {
    fn prove(
        goal: &Proposition,
        context: &PropositionContext,
        equalities: &[ProofNode],
    ) -> ProofNode {
        if let Proposition::Conjunction(children) = goal {
            return ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::ConjunctionIntroduction(
                    children
                        .iter()
                        .map(|child| prove(child, context, equalities))
                        .collect(),
                ),
            };
        }
        let closed = proof_admission::check_value_equality_denotation(
            context,
            goal,
            equalities.iter().map(|node| &node.conclusion),
        )
        .unwrap();
        let judgment = if closed == Proposition::Truth {
            proof_admission::PrimitiveJudgment::Truth
        } else {
            proof_admission::PrimitiveJudgment::ClosedIntegerRelation
        };
        ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(ProofNode {
                    conclusion: closed,
                    rule: ProofRule::Primitive(judgment),
                }),
                equalities: equalities.to_vec(),
            },
        }
    }
    let questions = terminal_verifier::reconstruct_terminal_obligations(module).unwrap();
    let context = PropositionContext::from_value_types(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| operation.result.scalar())
            .map(|value| (value.id, value.scalar_type)),
    )
    .unwrap();
    let mut evidence = questions
        .obligations()
        .iter()
        .map(|site| {
            let equalities = site
                .semantic_axioms
                .iter()
                .enumerate()
                .filter(|(_, fact)| matches!(fact, Proposition::Equal(ScalarTerm::Value { .. }, _)))
                .map(|(index, fact)| ProofNode {
                    conclusion: fact.clone(),
                    rule: ProofRule::SemanticAxiom { index },
                })
                .collect::<Vec<_>>();
            ObligationEvidence {
                obligation: site.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(site.obligation.id.get()).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: prove(&site.obligation.proposition, &context, &equalities),
                }),
            }
        })
        .collect::<Vec<_>>();
    evidence.sort_by_key(|row| row.obligation);
    ProofBundle {
        evidence,
        ..Default::default()
    }
}

#[test]
fn owned_byte_concatenation_roundtrips_and_replaces_alias_after_both_reads() {
    for (left, right, alias) in [
        (b"Hello, ".as_slice(), b"Ada!".as_slice(), false),
        (b"== ".as_slice(), b"Gate ==".as_slice(), true),
        (b"".as_slice(), b"".as_slice(), true),
    ] {
        let module = module(left, right, alias);
        let proof = proof(&module);
        verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
        let semantic = encode_module(&module).unwrap();
        let encoded_proof = encode_proof_section(&module, &proof).unwrap();
        let input = [TerminalStructuralValue {
            opaque_identity: 999,
            structural_type: structural_type_id(3),
            qualifications: Vec::new(),
            path: Vec::new(),
        }];
        let mut execution = TerminalExecution::start_artifact(
            &semantic,
            &encoded_proof,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &input,
                ..Default::default()
            },
        )
        .unwrap();
        let mut meter = TerminalFuelMeter::with_allowance(0);
        loop {
            match execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
                TerminalExecutionStatus::Complete(_) => break,
                other => panic!("unexpected status {other:?}"),
            }
        }
        assert_eq!(
            execution
                .structural_byte_sequence_field(999, &[], StructuralFieldId::new(1).unwrap())
                .unwrap(),
            [left, right].concat()
        );
    }
}

#[test]
fn owned_byte_concatenation_rejects_unestablished_or_mismatched_length_sources() {
    let base = module(b"a", b"bc", false);
    let mut wrong_length = base.clone();
    let OperationKind::EstablishByteSequenceConcatenation { left, .. } =
        &mut wrong_length.machines[0].blocks[0].operations[4].kind
    else {
        unreachable!()
    };
    *left = ByteSequenceOperand::Whole {
        source: place_id(1),
        length: value_id(4),
    };
    assert!(matches!(
        terminal_verifier::validate_module_representation(&wrong_length),
        Err(ModuleError::InvalidByteSequenceConcatenation(_))
    ));
    let mut before_establishment = base;
    before_establishment.machines[0].blocks[0]
        .operations
        .swap(4, 5);
    assert!(terminal_verifier::validate_module_representation(&before_establishment).is_err());
}

fn owned_write_module(qualified: bool) -> TerminalModule {
    let mut module = module(b"abc", b"", false);
    if qualified {
        module
            .structural_domains
            .push(terminal_psi::StructuralDomainDeclaration {
                id: semantic_vocabulary::StructuralDomainId::new(1).unwrap(),
                semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
                identity: "test::Utf8".into(),
                carrier: structural_type_id(2),
                byte_predicate: Some(terminal_psi::ByteSequencePredicate::ValidUtf8),
                content_projection: None,
                establishment_routes: Vec::new(),
            });
    }
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[2].shape else {
        unreachable!()
    };
    for ordinal in [2, 3] {
        fields.push(StructuralFieldDeclaration {
            id: StructuralFieldId::new(ordinal).unwrap(),
            identity: format!("snapshot{ordinal}"),
            relevance: Default::default(),
            field_type: StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                capacity: 32,
            }),
        });
    }
    let operations = &mut module.machines[0].blocks[0].operations;
    if qualified {
        let OperationResult::Structural(result) = &mut operations[4].result else {
            unreachable!()
        };
        result
            .qualifications
            .push(semantic_vocabulary::StructuralDomainId::new(1).unwrap());
    }
    let final_store = operations.pop().unwrap();
    for (ordinal, source, length, field) in [(10, 3, 6, 2), (11, 1, 3, 3)] {
        // Qualified whole-field source transport remains a separate runtime
        // capability; observe the qualified owner's write directly below.
        if qualified && source == 3 {
            continue;
        }
        operations.push(operation(
            ordinal,
            OperationKind::StructuralByteSequenceFieldStore {
                destination: place_id(5),
                path: Vec::new(),
                field: StructuralFieldId::new(field).unwrap(),
                source: place_id(source),
                length: value_id(length),
                obligation: obligation_id(ordinal),
            },
            OperationResult::Unit,
        ));
    }
    for (ordinal, bits, integer) in [(12, 64, 1), (13, 8, if qualified { 0 } else { 255 })] {
        operations.push(operation(
            ordinal,
            OperationKind::IntegerConstant {
                value: semantic_vocabulary::IntegerValue::Unsigned(integer),
            },
            OperationResult::Scalar(ValueDeclaration {
                id: value_id(ordinal),
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, bits).unwrap(),
                ),
                qualifications: Default::default(),
            }),
        ));
    }
    operations.push(operation(
        14,
        OperationKind::ByteSequenceWrite {
            destination: place_id(3),
            index: value_id(12),
            value: value_id(13),
            length: value_id(6),
            obligation: obligation_id(14),
        },
        OperationResult::Unit,
    ));
    if !qualified {
        operations.push(final_store);
    }
    operations.push(operation(
        15,
        OperationKind::ByteSequenceRead {
            source: place_id(3),
            index: value_id(12),
            length: value_id(6),
            obligation: obligation_id(15),
        },
        OperationResult::Scalar(super::byte_sequence_read::scalar(15, 8)),
    ));
    module.machines[0].result =
        terminal_psi::TerminalMachineResult::Scalar(super::byte_sequence_read::scalar(16, 8));
    module.machines[0].blocks[0].terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(15),
        cleanup_actions: Vec::new(),
    };
    module
}

#[test]
fn owned_byte_write_preserves_literal_and_earlier_field_snapshot_through_fuel_stops() {
    for qualified in [false, true] {
        let module = owned_write_module(qualified);
        let proof = proof(&module);
        verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
        let semantic = encode_module(&module).unwrap();
        let encoded_proof = encode_proof_section(&module, &proof).unwrap();
        let input = [TerminalStructuralValue {
            opaque_identity: 999,
            structural_type: structural_type_id(3),
            qualifications: Vec::new(),
            path: Vec::new(),
        }];
        let mut execution = TerminalExecution::start_artifact(
            &semantic,
            &encoded_proof,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs {
                arguments: &input,
                ..Default::default()
            },
        )
        .unwrap();
        let mut meter = TerminalFuelMeter::with_allowance(0);
        loop {
            match execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap()
            {
                TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
                TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(
                        result,
                        super::TerminalExecutionResult::Scalar(
                            super::TerminalScalarValue::Integer {
                                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                                value: semantic_vocabulary::IntegerValue::Unsigned(if qualified {
                                    0
                                } else {
                                    255
                                }),
                            },
                        )
                    );
                    break;
                }
                other => panic!("unexpected status {other:?}"),
            }
        }
        if !qualified {
            assert_eq!(
                execution
                    .structural_byte_sequence_field(999, &[], StructuralFieldId::new(1).unwrap(),)
                    .unwrap(),
                &[b'a', 255, b'c']
            );
        }
        for ordinal in [2, 3] {
            if qualified && ordinal == 2 {
                continue;
            }
            assert_eq!(
                execution
                    .structural_byte_sequence_field(
                        999,
                        &[],
                        StructuralFieldId::new(ordinal).unwrap(),
                    )
                    .unwrap(),
                b"abc"
            );
        }
    }
}

#[test]
fn owned_byte_write_rejects_literal_root_and_out_of_live_extent() {
    let mut literal = owned_write_module(false);
    let operation = literal.machines[0].blocks[0]
        .operations
        .iter_mut()
        .find(|operation| operation.id == operation_id(14))
        .unwrap();
    let OperationKind::ByteSequenceWrite {
        destination,
        length,
        ..
    } = &mut operation.kind
    else {
        unreachable!()
    };
    *destination = place_id(1);
    *length = value_id(3);
    assert!(terminal_verifier::validate_module_representation(&literal).is_err());

    let mut outside = owned_write_module(false);
    outside.machines[0].blocks[0]
        .operations
        .iter_mut()
        .find(|operation| operation.id == operation_id(12))
        .unwrap()
        .kind = OperationKind::IntegerConstant {
        value: semantic_vocabulary::IntegerValue::Unsigned(3),
    };
    let invalid_proof = proof(&outside);
    assert!(verify_module(&outside, &invalid_proof, &AdmissionProfile::default()).is_err());
}
