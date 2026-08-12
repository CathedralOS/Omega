use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation,
    ContentDomainId, ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity,
    ContentStructuralPlace, ContentTerm, ContractId, EdgeId, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionId,
    ScalarTerm, ScalarType, ServiceId, StructuralDomainId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use psi_terminal::{
    BindingRelevance, Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimSettlement,
    ClaimTransfer, ContentEntryClaim, ContentIdentityReshuffle, ContentPartitionComposition,
    ContentPlaceSubstitution, ContractClause, CrashCause, EntryClaim, MachineContract, Operation,
    OperationKind, OperationResult, PropositionApplicationIdentity,
    PropositionBinderArgumentIdentity, PropositionBinderArgumentKind, PropositionBinderDeclaration,
    PropositionBinderKind, PropositionDeclaration, PropositionEvidence, ServiceDeclaration,
    StructuralArgument, StructuralDomainDeclaration, StructuralDomainRequirement,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{
    CodecError, decode_module, encode_module, semantic_fingerprint, terminal_psi_identity,
};

#[test]
fn current_vocabulary_has_one_stable_canonical_encoding_and_identity() {
    let module = fixture();
    let bytes = encode_module(&module).expect("fixture should encode");

    assert_eq!(&bytes[..8], b"PSITERM\0");
    assert_eq!(&bytes[8..10], 1_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));

    let identity = terminal_psi_identity(&module).expect("fixture should have an identity");
    assert_eq!(identity.vocabulary_marker, VocabularyMarker::CURRENT);
    assert_eq!(
        identity.program_fingerprint.to_string(),
        "b0acc1e2c3e67866962381d36dd2e61599c7f7ee40d3bfd8206577530a898947"
    );
    assert_eq!(
        identity.program_fingerprint,
        semantic_fingerprint(&module).unwrap()
    );
}

#[test]
fn unit_result_and_return_have_a_canonical_value_less_encoding() {
    let module = unit_fixture();
    let bytes = encode_module(&module).expect("unit terminal module should encode");
    let decoded = decode_module(&bytes).expect("unit terminal module should decode");

    assert_eq!(decoded, module);
    assert_eq!(encode_module(&decoded), Ok(bytes));
    assert_ne!(
        semantic_fingerprint(&unit_fixture()).unwrap(),
        semantic_fingerprint(&fixture()).unwrap(),
        "unit result shape is semantic identity, not an erased scalar convention"
    );
}

#[test]
fn structural_effect_foundation_round_trips_and_has_stable_identity() {
    let module = structural_effect_fixture();
    let bytes = encode_module(&module).expect("structural/effect foundation should encode");

    assert_eq!(&bytes[10..12], 2_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));

    let baseline = semantic_fingerprint(&module).unwrap();
    let mut changed = module.clone();
    let OperationKind::PortWrite { value, .. } =
        &mut changed.machines[1].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *value = 0x5b;
    assert_ne!(semantic_fingerprint(&changed).unwrap(), baseline);

    let mut changed = module.clone();
    changed.structural_types[0].identity.push_str("::changed");
    assert_ne!(semantic_fingerprint(&changed).unwrap(), baseline);

    let mut changed = module.clone();
    changed.machines[0].attachment = Some(structural_type_id(3));
    assert_ne!(semantic_fingerprint(&changed).unwrap(), baseline);
}

#[test]
fn erased_structural_field_round_trips_and_changes_semantic_identity() {
    let baseline = structural_effect_fixture();
    let mut module = baseline.clone();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape;
    fields.push(StructuralFieldDeclaration {
        id: structural_field_id(3),
        identity: "proof".to_owned(),
        relevance: BindingRelevance::Erased,
        field_type: StructuralFieldType::Erased {
            type_identity: "named(name(example::Evidence))".to_owned(),
        },
    });

    let bytes = encode_module(&module).expect("erased semantic field should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_ne!(
        semantic_fingerprint(&module).unwrap(),
        semantic_fingerprint(&baseline).unwrap(),
        "erased bindings remain terminal semantic identity"
    );
}

#[test]
fn numbered_entry_claim_path_round_trips_and_enters_semantic_identity() {
    let baseline = structural_effect_fixture();
    let mut module = baseline.clone();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape;
    fields[1].identity = "#7".to_owned();
    for machine in &mut module.machines {
        machine.entry_claims[0].field_path = vec!["#7".to_owned()];
    }

    let bytes = encode_module(&module).expect("numbered aggregate claim path should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_ne!(
        semantic_fingerprint(&module).unwrap(),
        semantic_fingerprint(&baseline).unwrap(),
        "the exact numbered claim path is terminal semantic identity"
    );
}

#[test]
fn nested_record_claim_path_round_trips_and_enters_semantic_identity() {
    let baseline = structural_effect_fixture();
    let mut module = baseline.clone();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape;
    fields[1].identity = "#7".to_owned();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape;
    fields.push(StructuralFieldDeclaration {
        id: structural_field_id(1),
        identity: "#9".to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(3)),
    });
    module.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    for machine in &mut module.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims[0].field_path = vec!["#7".to_owned(), "#9".to_owned()];
    }

    let bytes = encode_module(&module).expect("nested aggregate claim path should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_ne!(
        semantic_fingerprint(&module).unwrap(),
        semantic_fingerprint(&baseline).unwrap(),
        "every nested path segment enters terminal semantic identity"
    );
}

#[test]
fn disjoint_sibling_claim_set_round_trips_as_canonical_identity() {
    let baseline = structural_effect_fixture();
    let mut module = baseline.clone();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape;
    fields[1].identity = "#7".to_owned();
    fields.push(StructuralFieldDeclaration {
        id: structural_field_id(3),
        identity: "#9".to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    module.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    for machine in &mut module.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims[0].field_path = vec!["#7".to_owned()];
        machine.entry_claims.push(EntryClaim {
            claim: claim_id(2),
            input: machine.structural_parameters[0].place,
            field_path: vec!["#9".to_owned()],
        });
    }
    let OperationKind::CallUnit {
        claim_transfers, ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers.push(ClaimTransfer {
        claim: claim_id(2),
        argument_index: 0,
    });
    let OperationKind::BoundaryCallUnit {
        claim_settlements, ..
    } = &mut module.machines[1].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    claim_settlements.push(ClaimSettlement {
        claim: claim_id(2),
        argument_index: 0,
    });

    let bytes = encode_module(&module).expect("canonical sibling claim set should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_ne!(
        semantic_fingerprint(&module).unwrap(),
        semantic_fingerprint(&baseline).unwrap(),
        "the complete sibling claim set enters terminal semantic identity"
    );
}

#[test]
fn structural_foundation_rejects_opaque_relevant_and_nonopaque_erased_fields() {
    let mut opaque_relevant = structural_effect_fixture();
    let StructuralTypeShape::Record { fields } = &mut opaque_relevant.structural_types[0].shape;
    fields.push(StructuralFieldDeclaration {
        id: structural_field_id(3),
        identity: "bad".to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Erased {
            type_identity: "named(name(example::Evidence))".to_owned(),
        },
    });
    assert_eq!(
        encode_module(&opaque_relevant),
        Err(CodecError::MalformedStructuralFoundation(
            "opaque structural field type must have erased relevance and a nonempty type identity"
        ))
    );

    let mut nonopaque_erased = structural_effect_fixture();
    let StructuralTypeShape::Record { fields } = &mut nonopaque_erased.structural_types[0].shape;
    fields.push(StructuralFieldDeclaration {
        id: structural_field_id(3),
        identity: "bad".to_owned(),
        relevance: BindingRelevance::Erased,
        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    assert_eq!(
        encode_module(&nonopaque_erased),
        Err(CodecError::MalformedStructuralFoundation(
            "erased structural field must use its opaque semantic type identity"
        ))
    );
}

#[test]
fn decoder_rejects_a_noncurrent_vocabulary_marker() {
    let mut bytes = encode_module(&structural_effect_fixture()).unwrap();
    bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());

    assert_eq!(
        decode_module(&bytes),
        Err(CodecError::UnsupportedVocabularyMarker(1))
    );
}

#[test]
fn structural_foundation_rejects_noncanonical_rows() {
    let mut module = structural_effect_fixture();
    module.structural_types.swap(0, 1);
    assert_eq!(
        encode_module(&module),
        Err(CodecError::NonCanonicalOrder(
            "structural types by StructuralTypeId"
        ))
    );

    let mut module = structural_effect_fixture();
    module.machines[0].entry_claims = vec![
        EntryClaim {
            claim: claim_id(2),
            input: place_id(10),
            field_path: Vec::new(),
        },
        EntryClaim {
            claim: claim_id(1),
            input: place_id(10),
            field_path: Vec::new(),
        },
    ];
    assert_eq!(
        encode_module(&module),
        Err(CodecError::NonCanonicalOrder("entry claims by ClaimId"))
    );
}

#[test]
fn structural_foundation_rejects_wrong_domain_carrier() {
    let mut module = structural_effect_fixture();
    module.structural_domains[0].carrier = structural_type_id(2);

    assert_eq!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "structural parameter qualification has the wrong carrier"
        ))
    );
}

#[test]
fn structural_foundation_rejects_wrong_call_argument_type() {
    let mut module = structural_effect_fixture();
    module.machines[1].structural_parameters[0].structural_type = structural_type_id(3);
    module.machines[1].structural_parameters[0]
        .qualifications
        .clear();

    assert_eq!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "structural argument has the wrong concrete type"
        ))
    );
}

#[test]
fn structural_foundation_rejects_recursive_by_value_types() {
    let mut module = unit_fixture();
    module.structural_types = vec![
        StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "example::A".to_owned(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: "b".to_owned(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(2)),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "example::B".to_owned(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: "a".to_owned(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(1)),
                }],
            },
        },
    ];

    assert_eq!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "structural type graph contains a by-value cycle"
        ))
    );
}

#[test]
fn structural_foundation_rejects_cyclic_or_incomplete_service_closure() {
    let mut cyclic = unit_fixture();
    cyclic.services = vec![
        ServiceDeclaration {
            id: service_id(1),
            identity: "example::A".to_owned(),
            parents: vec![service_id(2)],
        },
        ServiceDeclaration {
            id: service_id(2),
            identity: "example::B".to_owned(),
            parents: vec![service_id(1)],
        },
    ];
    assert_eq!(
        encode_module(&cyclic),
        Err(CodecError::MalformedStructuralFoundation(
            "service parent graph contains a cycle"
        ))
    );

    let mut incomplete = unit_fixture();
    incomplete.services = vec![
        ServiceDeclaration {
            id: service_id(1),
            identity: "example::Leaf".to_owned(),
            parents: vec![service_id(2)],
        },
        ServiceDeclaration {
            id: service_id(2),
            identity: "example::Middle".to_owned(),
            parents: vec![service_id(3)],
        },
        ServiceDeclaration {
            id: service_id(3),
            identity: "example::Root".to_owned(),
            parents: Vec::new(),
        },
    ];
    assert_eq!(
        encode_module(&incomplete),
        Err(CodecError::MalformedStructuralFoundation(
            "service parent closure is incomplete"
        ))
    );
}

#[test]
fn structural_declarations_do_not_bypass_semantic_graph_validation() {
    let mut module = fixture();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(1),
        identity: "example::Marker".to_owned(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let unknown = value_id(999);

    let mut malformed = module.clone();
    malformed.machines[0].blocks[1].terminator = Terminator::Return {
        edge: edge_id(2),
        value: unknown,
    };
    assert_eq!(
        encode_module(&malformed),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::ValueUsedBeforeDefinition(unknown)
        ))
    );

    let mut bytes = encode_module(&module).expect("complete semantic module should encode");
    let mut return_encoding = vec![2_u8];
    return_encoding.extend_from_slice(&2_u64.to_le_bytes());
    return_encoding.extend_from_slice(&3_u64.to_le_bytes());
    let offset = bytes
        .windows(return_encoding.len())
        .position(|window| window == return_encoding)
        .expect("fixture has one scalar return encoding");
    bytes[offset + 9..offset + 17].copy_from_slice(&unknown.get().to_le_bytes());
    assert_eq!(
        decode_module(&bytes),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::ValueUsedBeforeDefinition(unknown)
        ))
    );
}

#[test]
fn structural_unit_calls_participate_in_call_graph_validation() {
    let mut module = structural_effect_fixture();
    let OperationKind::CallUnit { callee, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *callee = machine_id(100);

    assert_eq!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::RecursiveCallSliceNotYetSupported(machine_id(100))
        ))
    );
}

#[test]
fn decoder_rejects_an_unknown_machine_result_shape() {
    let mut bytes = encode_module(&unit_fixture()).expect("unit terminal module should encode");
    // magic + format + vocabulary + entry + four empty foundation tables +
    // two empty proposition counts + machine count + machine id + no attachment +
    // empty scalar and structural parameter counts
    bytes[65] = 0xff;

    assert_eq!(
        decode_module(&bytes),
        Err(CodecError::InvalidTag("TerminalMachineResult", 0xff))
    );
}

#[test]
fn scalar_call_round_trips_with_arguments_requirements_and_crash_continuations() {
    let module = call_fixture();
    let bytes = encode_module(&module).expect("call module should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(
        semantic_fingerprint(&decode_module(&bytes).unwrap()),
        semantic_fingerprint(&module)
    );

    let mut crash_capable = module.clone();
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut crash_capable.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    let route = psi_terminal::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![psi_terminal::CrashRouteGuard::Truth],
    };
    crash_continuations.push(route.clone());
    crash_capable.machines[0].contract.crash_routes = vec![route.clone()];
    crash_capable.machines[1].contract.crash_routes = vec![route];
    crash_capable.machines[1].blocks[0].terminator = Terminator::Crash {
        edge: edge_id(101),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let crash_bytes = encode_module(&crash_capable).expect("crash continuation should encode");
    assert_eq!(decode_module(&crash_bytes), Ok(crash_capable.clone()));
    assert_ne!(
        semantic_fingerprint(&module).unwrap(),
        semantic_fingerprint(&crash_capable).unwrap(),
        "call crash continuations are fingerprinted semantic content"
    );
}

#[test]
fn crash_round_trips_and_every_semantic_field_enters_identity() {
    let mut module = fixture();
    module.machines[0].contract.crash_routes = vec![psi_terminal::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![psi_terminal::CrashRouteGuard::Truth],
    }];
    module.machines[0].blocks[1].terminator = Terminator::Crash {
        edge: edge_id(2),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let bytes = encode_module(&module).expect("crash encodes");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let baseline = semantic_fingerprint(&module).expect("crash identity");
    if let Terminator::Crash { cause, .. } = &mut module.machines[0].blocks[1].terminator {
        *cause = CrashCause::Abort;
    } else {
        unreachable!()
    }
    module.machines[0].contract.crash_routes[0].cause = CrashCause::Abort;
    assert_ne!(
        semantic_fingerprint(&module).expect("changed cause identity"),
        baseline
    );
    let Terminator::Crash {
        cause, site_guard, ..
    } = &mut module.machines[0].blocks[1].terminator
    else {
        unreachable!()
    };
    *cause = CrashCause::Trap;
    let predicate = psi_terminal::CrashPredicateTerm::new(Proposition::Equal(
        ScalarTerm::value(value_id(5), ScalarType::Boolean),
        ScalarTerm::boolean(true),
    ));
    site_guard.push(predicate.clone());
    module.machines[0].contract.crash_routes[0].cause = CrashCause::Trap;
    assert_ne!(
        semantic_fingerprint(&module).expect("changed site-guard identity"),
        baseline
    );
    module.machines[0].contract.crash_routes[0].alternatives =
        vec![psi_terminal::CrashRouteGuard::Predicate(predicate)];
    assert_ne!(
        semantic_fingerprint(&module).expect("changed route identity"),
        baseline
    );
}

#[test]
fn proposition_vocabulary_round_trips_and_enters_identity() {
    let mut module = fixture();
    module.proposition_declarations = vec![PropositionDeclaration {
        id: proposition_id(1),
        name: "converges_together".to_owned(),
        binders: vec![
            PropositionBinderDeclaration {
                name: "Left".to_owned(),
                kind: PropositionBinderKind::Machine,
            },
            PropositionBinderDeclaration {
                name: "Precision".to_owned(),
                kind: PropositionBinderKind::Const {
                    type_identity: "u32".to_owned(),
                },
            },
        ],
        parameter_types: vec!["CauchySeq<Left>".to_owned()],
        evidence: PropositionEvidence::Witness {
            evidence_type: "ConvergenceEvidence<Left>".to_owned(),
        },
    }];
    module.proposition_applications = vec![PropositionApplicationIdentity {
        id: proposition_id(1),
        declaration: proposition_id(1),
        binder_arguments: vec![
            PropositionBinderArgumentIdentity {
                kind: PropositionBinderArgumentKind::Machine,
                identity: "unit_sample".to_owned(),
            },
            PropositionBinderArgumentIdentity {
                kind: PropositionBinderArgumentKind::Const,
                identity: "32u32".to_owned(),
            },
        ],
        arguments: vec!["sequence".to_owned()],
    }];

    let bytes = encode_module(&module).expect("proposition vocabulary should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let original = semantic_fingerprint(&module).expect("vocabulary has identity");
    module.proposition_declarations[0].evidence = PropositionEvidence::Witness {
        evidence_type: "AlternativeEvidence<Left>".to_owned(),
    };
    assert_ne!(
        semantic_fingerprint(&module).expect("changed evidence interface has identity"),
        original
    );
    module.proposition_declarations[0].evidence = PropositionEvidence::FactOnly;
    assert_ne!(
        semantic_fingerprint(&module).expect("changed vocabulary has identity"),
        original
    );
}

#[test]
fn proposition_vocabulary_is_category_checked() {
    let mut module = fixture();
    module.proposition_declarations = vec![PropositionDeclaration {
        id: proposition_id(1),
        name: "related".to_owned(),
        binders: vec![PropositionBinderDeclaration {
            name: "Carrier".to_owned(),
            kind: PropositionBinderKind::Type,
        }],
        parameter_types: vec!["Carrier".to_owned()],
        evidence: PropositionEvidence::FactOnly,
    }];
    module.proposition_applications = vec![PropositionApplicationIdentity {
        id: proposition_id(1),
        declaration: proposition_id(1),
        binder_arguments: vec![PropositionBinderArgumentIdentity {
            kind: PropositionBinderArgumentKind::Machine,
            identity: "Generator".to_owned(),
        }],
        arguments: vec!["value".to_owned()],
    }];
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::PropositionApplicationBinderMismatch(_)
        ))
    ));
}

#[test]
fn entry_claim_encoding_is_canonically_ordered() {
    let mut projections = entry_claim_fixture();
    projections.machines[0].content_entry_claims[0]
        .projections
        .swap(0, 1);
    assert_eq!(
        encode_module(&projections),
        Err(CodecError::NonCanonicalOrder(
            "entry-claim content projections by identity and algebra"
        ))
    );
}

#[test]
fn partition_composition_encoding_is_canonically_ordered() {
    let mut substitutions = partition_composition_fixture();
    substitutions.machines[0].content_partition_compositions[0]
        .substitutions
        .swap(0, 1);
    assert_eq!(
        encode_module(&substitutions),
        Err(CodecError::NonCanonicalOrder(
            "partition place substitutions"
        ))
    );
}

#[test]
fn identity_reshuffle_encoding_is_canonically_ordered() {
    let mut projections = identity_reshuffle_fixture(VocabularyMarker::CURRENT);
    projections.machines[0].content_identity_reshuffles[0]
        .projections
        .swap(0, 1);
    assert_eq!(
        encode_module(&projections),
        Err(CodecError::NonCanonicalOrder(
            "claim content projections by identity and algebra"
        ))
    );
}

#[test]
fn semantic_mutation_changes_the_program_fingerprint() {
    let original = fixture();
    let mut changed = original.clone();
    changed.machines[0].blocks[1].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Signed(-6),
    };

    assert_ne!(
        semantic_fingerprint(&original).unwrap(),
        semantic_fingerprint(&changed).unwrap()
    );
}

#[test]
fn decoder_rejects_noncanonical_or_ambiguous_bytes() {
    let bytes = encode_module(&fixture()).unwrap();

    let mut reordered_requirements = bytes.clone();
    let contract_prefix = [
        1, 0, 0, 0, 0, 0, 0, 0, // ContractId(1)
        0, 0, 0, 0, // zero crash route buckets
        8, 0, 0, 0, // eight requirements
        1, 2, 3, // Truth, Falsehood, Atom
    ];
    let contract_offset = reordered_requirements
        .windows(contract_prefix.len())
        .position(|window| window == contract_prefix)
        .expect("fixture contract prefix should be unique");
    reordered_requirements.swap(contract_offset + 16, contract_offset + 17);
    assert_eq!(
        decode_module(&reordered_requirements),
        Err(CodecError::NonCanonicalOrder("requires propositions"))
    );

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(decode_module(&trailing), Err(CodecError::TrailingBytes(1)));

    let mut future_format = bytes.clone();
    future_format[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_module(&future_format),
        Err(CodecError::UnsupportedFormatMarker(2))
    );

    assert_eq!(
        decode_module(&bytes[..bytes.len() - 1]),
        Err(CodecError::UnexpectedEnd)
    );
}

#[test]
fn encoder_refuses_noncanonical_semantic_ordering_and_forms() {
    let mut blocks = fixture();
    blocks.machines[0].blocks.swap(0, 1);
    assert_eq!(
        encode_module(&blocks),
        Err(CodecError::NonCanonicalOrder("blocks by BlockId"))
    );

    let mut requirements = fixture();
    requirements.machines[0].contract.requires.swap(0, 1);
    assert_eq!(
        encode_module(&requirements),
        Err(CodecError::NonCanonicalOrder("requires propositions"))
    );

    let mut equality = fixture();
    equality.machines[0].contract.ensures[0].proposition = Proposition::Equal(
        ScalarTerm::integer(i32_type(), IntegerValue::Signed(-7)).unwrap(),
        ScalarTerm::value(value_id(4), ScalarType::Integer(i32_type())),
    );
    assert_eq!(
        encode_module(&equality),
        Err(CodecError::NonCanonicalOrder("equality operands"))
    );

    let mut conjunction = fixture();
    conjunction.machines[0].contract.ensures[0].proposition = Proposition::Conjunction(vec![
        Proposition::Truth,
        Proposition::Conjunction(vec![Proposition::Truth, Proposition::Falsehood]),
    ]);
    assert_eq!(
        encode_module(&conjunction),
        Err(CodecError::NestedConjunction)
    );

    let mut call = call_fixture();
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut call.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    crash_continuations.extend([
        psi_terminal::CrashRouteBucket {
            cause: CrashCause::Abort,
            alternatives: vec![psi_terminal::CrashRouteGuard::Truth],
        },
        psi_terminal::CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![psi_terminal::CrashRouteGuard::Truth],
        },
    ]);
    assert_eq!(
        encode_module(&call),
        Err(CodecError::NonCanonicalOrder(
            "call crash continuation buckets"
        ))
    );
}

#[test]
fn proposition_nesting_has_a_total_bound() {
    let mut module = fixture();
    let mut proposition = Proposition::Truth;
    for _ in 0..257 {
        proposition = Proposition::Implication {
            premise: Box::new(Proposition::Truth),
            conclusion: Box::new(proposition),
        };
    }
    module.machines[0].contract.ensures[0].proposition = proposition;

    assert_eq!(
        encode_module(&module),
        Err(CodecError::PropositionNestingTooDeep)
    );
}

#[test]
fn scalar_term_nesting_has_a_total_bound() {
    let mut module = fixture();
    let integer = i32_type();
    let literal = || ScalarTerm::integer(integer, IntegerValue::Signed(1)).unwrap();
    let mut term = literal();
    for _ in 0..257 {
        term = ScalarTerm::wrapping_integer_add(integer, term, literal()).unwrap();
    }
    module.machines[0].contract.ensures[0].proposition = Proposition::Equal(literal(), term);

    assert_eq!(
        encode_module(&module),
        Err(CodecError::ScalarTermNestingTooDeep)
    );
}

fn structural_effect_fixture() -> TerminalModule {
    let resource_type = structural_type_id(1);
    let domain = structural_domain_id(1);
    let service = service_id(1);
    let caller_place = place_id(10);
    let callee_place = place_id(20);
    let structural_parameter =
        |place, position, structural_type, is_self| StructuralParameterDeclaration {
            place,
            position,
            is_self,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: vec![domain],
        };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(100),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: resource_type,
                identity: "example::OccurrenceToken".to_owned(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: structural_field_id(1),
                            identity: "sequence".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                            )),
                        },
                        StructuralFieldDeclaration {
                            id: structural_field_id(2),
                            identity: "metadata".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(structural_type_id(2)),
                        },
                    ],
                },
            },
            StructuralTypeDeclaration {
                id: structural_type_id(2),
                identity: "example::Root".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: structural_type_id(3),
                identity: "example::Worker".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: vec![StructuralDomainDeclaration {
            id: domain,
            identity: "example::Occurrence::Pending".to_owned(),
            carrier: resource_type,
        }],
        services: vec![ServiceDeclaration {
            id: service,
            identity: "example::DeviceIo".to_owned(),
            parents: Vec::new(),
        }],
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_machine_id(1),
            identity: "example::Occurrence::settle".to_owned(),
            attachment: Some(resource_type),
            structural_parameters: vec![structural_parameter(place_id(30), 0, resource_type, true)],
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain,
            }],
            published_service_ceiling: vec![service],
        }],
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(100),
                attachment: Some(structural_type_id(2)),
                parameters: Vec::new(),
                structural_parameters: vec![structural_parameter(
                    caller_place,
                    0,
                    resource_type,
                    false,
                )],
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: caller_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: vec![EntryClaim {
                    claim: claim_id(1),
                    input: caller_place,
                    field_path: Vec::new(),
                }],
                published_service_ceiling: vec![service],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(100),
                blocks: vec![Block {
                    id: block_id(100),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: operation_id(1),
                        result: OperationResult::Unit,
                        kind: OperationKind::CallUnit {
                            callee: machine_id(101),
                            structural_arguments: vec![StructuralArgument {
                                place: caller_place,
                            }],
                            claim_transfers: vec![ClaimTransfer {
                                claim: claim_id(1),
                                argument_index: 0,
                            }],
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnUnit { edge: edge_id(100) },
                }],
                contract: MachineContract {
                    id: contract_id(100),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: machine_id(101),
                attachment: Some(structural_type_id(3)),
                parameters: Vec::new(),
                structural_parameters: vec![structural_parameter(
                    callee_place,
                    0,
                    resource_type,
                    false,
                )],
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: callee_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: vec![EntryClaim {
                    claim: claim_id(1),
                    input: callee_place,
                    field_path: Vec::new(),
                }],
                published_service_ceiling: vec![service],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(101),
                blocks: vec![Block {
                    id: block_id(101),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: operation_id(2),
                            result: OperationResult::Unit,
                            kind: OperationKind::PortWrite {
                                service,
                                port: 0x3f8,
                                value: 0x5a,
                            },
                        },
                        Operation {
                            id: operation_id(3),
                            result: OperationResult::Unit,
                            kind: OperationKind::BoundaryCallUnit {
                                boundary: boundary_machine_id(1),
                                structural_arguments: vec![StructuralArgument {
                                    place: callee_place,
                                }],
                                claim_settlements: vec![ClaimSettlement {
                                    claim: claim_id(1),
                                    argument_index: 0,
                                }],
                                requirement_obligations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::ReturnUnit { edge: edge_id(101) },
                }],
                contract: MachineContract {
                    id: contract_id(101),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            },
        ],
    }
}

fn unit_fixture() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(900),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(900),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(900),
            blocks: vec![Block {
                id: block_id(900),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit { edge: edge_id(900) },
            }],
            contract: MachineContract {
                id: contract_id(900),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    }
}

fn fixture() -> TerminalModule {
    let integer = i32_type();
    let scalar_type = ScalarType::Integer(integer);
    let signed = |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
    let unsigned_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let unsigned =
        |value| ScalarTerm::integer(unsigned_type, IntegerValue::Unsigned(value)).unwrap();

    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                id: value_id(5),
                scalar_type: ScalarType::Boolean,
            }],
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: value_id(4),
                scalar_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![
                Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: operation_id(1),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: value_id(1),
                            scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(-7),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: edge_id(1),
                        target: block_id(2),
                        arguments: vec![value_id(1)],
                    },
                },
                Block {
                    id: block_id(2),
                    parameters: vec![ValueDeclaration {
                        id: value_id(2),
                        scalar_type,
                    }],
                    operations: vec![Operation {
                        id: operation_id(2),
                        result: OperationResult::Scalar(ValueDeclaration {
                            id: value_id(3),
                            scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(-7),
                        },
                    }],
                    terminator: Terminator::Return {
                        edge: edge_id(2),
                        value: value_id(3),
                    },
                },
            ],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: vec![
                    Proposition::Truth,
                    Proposition::Falsehood,
                    Proposition::Atom(proposition_id(1)),
                    Proposition::Equal(ScalarTerm::boolean(false), ScalarTerm::boolean(true)),
                    Proposition::LessThan(signed(-8), signed(-7)),
                    Proposition::LessOrEqual(unsigned(1), unsigned(2)),
                    Proposition::Conjunction(vec![Proposition::Truth, Proposition::Falsehood]),
                    Proposition::Implication {
                        premise: Box::new(Proposition::Truth),
                        conclusion: Box::new(Proposition::Atom(proposition_id(2))),
                    },
                ],
                ensures: vec![
                    ContractClause {
                        obligation: obligation_id(1),
                        proposition: Proposition::Equal(
                            ScalarTerm::value(value_id(4), scalar_type),
                            signed(-7),
                        ),
                    },
                    ContractClause {
                        obligation: obligation_id(2),
                        proposition: Proposition::Truth,
                    },
                ],
            },
        }],
    }
}

fn content_conservation_fixture(vocabulary_marker: VocabularyMarker) -> TerminalModule {
    let parameter_place = place_id(1);
    let result_place = place_id(2);
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(7).expect("domain"),
        projection_fingerprint: 0x1234,
    };
    let projected = |version, root, field: Option<&str>| ContentTerm::Projection {
        projection,
        subject: ContentStructuralPlace {
            version,
            root,
            segments: field
                .map(|field| vec![ContentPlaceSegment::Field(field.to_owned())])
                .unwrap_or_default(),
        },
    };
    let entry = projected(ContentPlaceVersion::Entry, parameter_place, None);
    let left = projected(ContentPlaceVersion::Current, result_place, Some("left"));
    let right = projected(ContentPlaceVersion::Current, result_place, Some("right"));
    let proposition = Proposition::ContentConservation(ContentConservation::new(
        ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: "Address".to_owned(),
        },
        entry,
        ContentTerm::separate([right, left]).expect("canonical separation"),
    ));
    TerminalModule {
        vocabulary_marker,
        entry: machine_id(80),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(80),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                id: value_id(80),
                scalar_type: ScalarType::Boolean,
            }],
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: value_id(81),
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: parameter_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                },
                StructuralPlaceDeclaration {
                    id: result_place,
                    kind: StructuralPlaceKind::Result,
                },
            ],
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(80),
            blocks: vec![Block {
                id: block_id(80),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Return {
                    edge: edge_id(80),
                    value: value_id(80),
                },
            }],
            contract: MachineContract {
                id: contract_id(80),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation: obligation_id(80),
                    proposition,
                }],
            },
        }],
    }
}

fn identity_reshuffle_fixture(vocabulary_marker: VocabularyMarker) -> TerminalModule {
    let mut module = content_conservation_fixture(vocabulary_marker);
    let input_root = module.machines[0].structural_places[0].id;
    let output_root = module.machines[0].structural_places[1].id;
    module.machines[0].content_identity_reshuffles = vec![ContentIdentityReshuffle {
        claim: claim_id(1),
        input: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: input_root,
            segments: vec![ContentPlaceSegment::Field("payload".to_owned())],
        },
        output: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: output_root,
            segments: vec![ContentPlaceSegment::Field("payload".to_owned())],
        },
        projections: vec![
            ClaimContentProjection {
                projection: ContentProjectionIdentity {
                    domain: ContentDomainId::new(7).expect("domain"),
                    projection_fingerprint: 0x1234,
                },
                algebra: ContentAlgebra {
                    kind: ContentAlgebraKind::IntervalSet,
                    parameter: "Address".to_owned(),
                },
            },
            ClaimContentProjection {
                projection: ContentProjectionIdentity {
                    domain: ContentDomainId::new(8).expect("domain"),
                    projection_fingerprint: 0x5678,
                },
                algebra: ContentAlgebra {
                    kind: ContentAlgebraKind::CountedQuantity,
                    parameter: "Byte".to_owned(),
                },
            },
        ],
    }];
    module
}

fn entry_claim_fixture() -> TerminalModule {
    let mut module = identity_reshuffle_fixture(VocabularyMarker::CURRENT);
    let reshuffle = module.machines[0].content_identity_reshuffles[0].clone();
    module.machines[0].content_entry_claims = vec![ContentEntryClaim {
        claim: reshuffle.claim,
        input: reshuffle.input,
        projections: reshuffle.projections,
    }];
    module
}

fn partition_composition_fixture() -> TerminalModule {
    let mut module = identity_reshuffle_fixture(VocabularyMarker::CURRENT);
    let machine = &mut module.machines[0];
    machine.content_identity_reshuffles[0]
        .projections
        .truncate(1);
    let projection = machine.content_identity_reshuffles[0].projections[0].projection;
    let algebra = machine.content_identity_reshuffles[0].projections[0]
        .algebra
        .clone();
    let result_root = machine.content_identity_reshuffles[0].output.root;
    let source_input_root = place_id(90);
    let source_result_root = place_id(91);
    let place = |version, root, field: Option<&str>| ContentStructuralPlace {
        version,
        root,
        segments: field
            .into_iter()
            .map(|field| ContentPlaceSegment::Field(field.to_owned()))
            .collect(),
    };
    let term = |subject| ContentTerm::Projection {
        projection,
        subject,
    };
    let source_input = place(ContentPlaceVersion::Entry, source_input_root, None);
    let source_left = place(
        ContentPlaceVersion::Current,
        source_result_root,
        Some("left"),
    );
    let source_right = place(
        ContentPlaceVersion::Current,
        source_result_root,
        Some("right"),
    );
    let target_input = machine.content_identity_reshuffles[0].input.clone();
    let target_left = place(ContentPlaceVersion::Current, result_root, Some("left"));
    let target_right = place(ContentPlaceVersion::Current, result_root, Some("right"));
    machine.content_identity_reshuffles[0].input = target_input.clone();
    machine.content_identity_reshuffles[0].output = target_left.clone();
    let source = ContentConservation::new(
        algebra.clone(),
        term(source_input.clone()),
        ContentTerm::separate([term(source_left.clone()), term(source_right.clone())])
            .expect("source partition"),
    );
    let derived = ContentConservation::new(
        algebra,
        term(target_input.clone()),
        ContentTerm::separate([term(target_left.clone()), term(target_right.clone())])
            .expect("derived partition"),
    );
    let mut substitutions = vec![
        ContentPlaceSubstitution {
            source: source_input,
            target: target_input,
        },
        ContentPlaceSubstitution {
            source: source_left,
            target: target_left,
        },
        ContentPlaceSubstitution {
            source: source_right,
            target: target_right,
        },
    ];
    substitutions.sort();
    machine.content_partition_compositions = vec![ContentPartitionComposition {
        source_fingerprint: 0xfeed_face_dead_beef,
        source_structural_places: vec![
            StructuralPlaceDeclaration {
                id: source_input_root,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: source_result_root,
                kind: StructuralPlaceKind::Result,
            },
        ],
        source,
        input_claims: vec![claim_id(1)],
        substitutions,
        derived,
    }];
    module
}

fn call_fixture() -> TerminalModule {
    let boolean = |id| ValueDeclaration {
        id: value_id(id),
        scalar_type: ScalarType::Boolean,
    };
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(100),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(100),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(boolean(102)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(100),
                blocks: vec![Block {
                    id: block_id(100),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: operation_id(100),
                            result: OperationResult::Scalar(boolean(100)),
                            kind: OperationKind::BooleanConstant { value: true },
                        },
                        Operation {
                            id: operation_id(101),
                            result: OperationResult::Scalar(boolean(101)),
                            kind: OperationKind::Call {
                                callee: machine_id(101),
                                arguments: vec![value_id(100)],
                                requirement_obligations: Vec::new(),
                                crash_continuations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        edge: edge_id(100),
                        value: value_id(101),
                    },
                }],
                contract: MachineContract {
                    id: contract_id(100),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: machine_id(101),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![boolean(103)],
                result: TerminalMachineResult::Scalar(boolean(104)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(101),
                blocks: vec![Block {
                    id: block_id(101),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: edge_id(101),
                        value: value_id(103),
                    },
                }],
                contract: MachineContract {
                    id: contract_id(101),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            },
        ],
    }
}

fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).expect("i32")
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("test identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(place_id, PlaceId);
id_constructor!(claim_id, ClaimId);
id_constructor!(operation_id, OperationId);
id_constructor!(structural_type_id, StructuralTypeId);
id_constructor!(structural_field_id, StructuralFieldId);
id_constructor!(structural_domain_id, StructuralDomainId);
id_constructor!(service_id, ServiceId);
id_constructor!(boundary_machine_id, BoundaryMachineId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(proposition_id, PropositionId);
