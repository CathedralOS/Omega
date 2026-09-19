use super::{
    call_fixture, entry_claim_fixture, fixture, i32_type, identity_reshuffle_fixture,
    partition_composition_fixture, structural_call_fixture, structural_effect_fixture,
    unit_fixture,
};
use crate::canonical::{
    block_id, boundary_machine_id, claim_id, contract_id, edge_id, machine_id, obligation_id,
    operation_id, place_id, proposition_id, service_id, structural_case_id, structural_field_id,
    structural_type_id, value_id,
};
use semantic_vocabulary::{
    IntegerValue, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralPlaceKind,
    StructuralTypeId,
};
use terminal_codec::{CodecError, decode_module, encode_module, semantic_fingerprint};
use terminal_psi::{
    BindingRelevance, BoundaryMachineDeclaration, BoundaryMachineResult, ByteSequenceCarrier,
    CrashCause, EntryClaim, EvidenceInterfaceIdentity, EvidenceTermDeclaration, Operation,
    OperationKind, OperationResult, OutcomeSpecificEnsure, OutcomeSpecificEvidence,
    OutcomeSpecificGuard, PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence, ServiceDeclaration, StructuralAccess,
    StructuralArgument, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalMachineResult, TerminalModule, Terminator, VocabularyMarker,
};

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
            path: Vec::new(),
        },
        EntryClaim {
            claim: claim_id(1),
            input: place_id(10),
            path: Vec::new(),
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
        cleanup_actions: Vec::new(),
        edge: edge_id(2),
        value: unknown,
    };
    assert_eq!(
        encode_module(&malformed),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::ValueUsedBeforeDefinition(unknown)
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
            terminal_verifier::ModuleError::ValueUsedBeforeDefinition(unknown)
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
            terminal_verifier::ModuleError::RecursiveCallSliceNotYetSupported(machine_id(100))
        ))
    );
}

#[test]
fn decoder_rejects_an_unknown_machine_result_shape() {
    let mut module = unit_fixture();
    // Distinct machine and block IDs keep the empty block envelope from
    // matching the machine prefix used to locate the result tag.
    module.machines[0].entry = block_id(901);
    module.machines[0].blocks[0].id = block_id(901);
    let mut bytes = encode_module(&module).expect("unit terminal module should encode");
    let mut machine_prefix = 1_u32.to_le_bytes().to_vec(); // one machine
    machine_prefix.extend(machine_id(900).get().to_le_bytes());
    machine_prefix.push(0); // no attachment
    machine_prefix.extend(0_u32.to_le_bytes()); // no scalar parameters
    machine_prefix.extend(0_u32.to_le_bytes()); // no structural parameters
    machine_prefix.push(0); // no ranked SCC
    machine_prefix.push(0); // Unit result tag
    let offsets = bytes
        .windows(machine_prefix.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == machine_prefix).then_some(offset))
        .collect::<Vec<_>>();
    let [machine_offset] = offsets.as_slice() else {
        panic!("fixture must contain one unique encoded machine prefix")
    };
    bytes[machine_offset + machine_prefix.len() - 1] = 0xff;

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
    let route = terminal_psi::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
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
    module.machines[0].contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
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
    let predicate = terminal_psi::CrashPredicateTerm::new(Proposition::Equal(
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
        vec![terminal_psi::CrashRouteGuard::Predicate(predicate)];
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
                evidence_projection: None,
            },
            PropositionBinderArgumentIdentity {
                kind: PropositionBinderArgumentKind::Const,
                identity: "32u32".to_owned(),
                evidence_projection: None,
            },
        ],
        arguments: vec!["sequence".to_owned()],
        evidence_interface: Some(EvidenceInterfaceIdentity {
            trait_identity: "ConvergenceEvidence".to_owned(),
            arguments: vec!["unit_sample".to_owned()],
            requirements: Vec::new(),
        }),
    }];

    let bytes = encode_module(&module).expect("proposition vocabulary should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let original = semantic_fingerprint(&module).expect("vocabulary has identity");
    module.proposition_applications[0]
        .evidence_interface
        .as_mut()
        .expect("witness application interface")
        .trait_identity = "AlternativeConvergenceEvidence".to_owned();
    assert_ne!(
        semantic_fingerprint(&module).expect("instantiated interface has identity"),
        original
    );
    module.proposition_applications[0]
        .evidence_interface
        .as_mut()
        .expect("witness application interface")
        .trait_identity = "ConvergenceEvidence".to_owned();
    module.proposition_declarations[0].evidence = PropositionEvidence::Witness {
        evidence_type: "AlternativeEvidence<Left>".to_owned(),
    };
    assert_ne!(
        semantic_fingerprint(&module).expect("changed evidence interface has identity"),
        original
    );
    module.proposition_declarations[0].evidence = PropositionEvidence::FactOnly;
    module.proposition_applications[0].evidence_interface = None;
    assert_ne!(
        semantic_fingerprint(&module).expect("changed vocabulary has identity"),
        original
    );
}

#[test]
fn outcome_specific_guarantees_round_trip_and_guard_enters_identity() {
    let mut module = structural_call_fixture();
    let result_type = module.structural_types[0].id;
    let success = structural_case_id(1);
    let failure = structural_case_id(2);
    module.structural_types[0].shape = StructuralTypeShape::Sum {
        cases: vec![
            StructuralCaseDeclaration {
                id: success,
                identity: "Success".to_owned(),
                fields: Vec::new(),
            },
            StructuralCaseDeclaration {
                id: failure,
                identity: "Failure".to_owned(),
                fields: Vec::new(),
            },
        ],
    };
    let proposition = proposition_id(1);
    let term = semantic_vocabulary::EvidenceTermId::new(1).unwrap();
    let interface = EvidenceInterfaceIdentity {
        trait_identity: "ReadyEvidence".to_owned(),
        arguments: Vec::new(),
        requirements: Vec::new(),
    };
    module.proposition_declarations = vec![PropositionDeclaration {
        id: proposition,
        name: "ready".to_owned(),
        binders: Vec::new(),
        parameter_types: Vec::new(),
        evidence: PropositionEvidence::Witness {
            evidence_type: "ReadyEvidence".to_owned(),
        },
    }];
    module.proposition_applications = vec![PropositionApplicationIdentity {
        id: proposition,
        declaration: proposition,
        binder_arguments: Vec::new(),
        arguments: Vec::new(),
        evidence_interface: Some(interface.clone()),
    }];
    module.evidence_terms = vec![EvidenceTermDeclaration {
        id: term,
        proposition,
        interface,
    }];
    let guard = OutcomeSpecificGuard {
        result_type,
        result_case: success,
    };
    module.machines[0].contract.outcome_specific_ensures = vec![
        OutcomeSpecificEnsure {
            guard,
            position: 0,
            obligation: obligation_id(2),
            proposition: Proposition::Atom(proposition),
            evidence: Some(OutcomeSpecificEvidence {
                term,
                output_field: "selected".to_owned(),
            }),
        },
        OutcomeSpecificEnsure {
            guard,
            position: 1,
            obligation: obligation_id(3),
            proposition: Proposition::Truth,
            evidence: None,
        },
    ];

    let bytes = encode_module(&module).expect("guarded rows encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let baseline = semantic_fingerprint(&module).expect("guarded identity");

    let mut changed_case = module.clone();
    for row in &mut changed_case.machines[0].contract.outcome_specific_ensures {
        row.guard.result_case = failure;
    }
    assert_ne!(
        semantic_fingerprint(&changed_case).expect("sibling guard identity"),
        baseline
    );

    let mut changed_selector = module;
    changed_selector.machines[0]
        .contract
        .outcome_specific_ensures[0]
        .evidence
        .as_mut()
        .unwrap()
        .output_field = "other".to_owned();
    assert_ne!(
        semantic_fingerprint(&changed_selector).expect("selector identity"),
        baseline
    );
}

#[test]
fn payloadless_case_establishment_round_trips_and_case_enters_identity() {
    let mut module = structural_call_fixture();
    module.root_service_reach.concrete.clear();
    let success = structural_case_id(10);
    let failure = structural_case_id(11);
    module.structural_types[0].shape = StructuralTypeShape::Sum {
        cases: vec![
            StructuralCaseDeclaration {
                id: success,
                identity: "Success".into(),
                fields: Vec::new(),
            },
            StructuralCaseDeclaration {
                id: failure,
                identity: "Failure".into(),
                fields: Vec::new(),
            },
        ],
    };
    let operation = &mut module.machines[0].blocks[0].operations[0];
    operation.kind = OperationKind::EstablishScalarCase {
        result_case: success,
        fields: Vec::new(),
    };
    let OperationResult::Structural(result) = &mut operation.result else {
        panic!("fixture operation must have a structural result")
    };
    result.multiplicity = StructuralMultiplicity::Unrestricted;
    result.qualifications.clear();
    result.claims.clear();
    let machine = &mut module.machines[0];
    machine.structural_parameters.clear();
    machine.entry_claims.clear();
    machine.published_service_ceiling.clear();
    machine.structural_places.retain(|place| {
        matches!(
            place.kind,
            StructuralPlaceKind::OperationResult { .. } | StructuralPlaceKind::Result
        )
    });
    let TerminalMachineResult::Structural(machine_result) = &mut machine.result else {
        unreachable!()
    };
    machine_result.multiplicity = StructuralMultiplicity::Unrestricted;
    machine_result.qualifications.clear();
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    returned_claims.clear();

    let bytes = encode_module(&module).expect("payloadless case should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let success_identity = semantic_fingerprint(&module).expect("case has identity");

    let OperationKind::EstablishScalarCase { result_case, .. } =
        &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *result_case = failure;
    assert_ne!(
        semantic_fingerprint(&module).expect("alternate case has identity"),
        success_identity,
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
            evidence_projection: None,
        }],
        arguments: vec!["value".to_owned()],
        evidence_interface: None,
    }];
    assert!(matches!(
        encode_module(&module),
        Err(CodecError::InvalidModule(
            terminal_verifier::ModuleError::PropositionApplicationBinderMismatch(_)
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
    let future_marker = u16::from_le_bytes([bytes[8], bytes[9]]) + 1;
    future_format[8..10].copy_from_slice(&future_marker.to_le_bytes());
    assert_eq!(
        decode_module(&future_format),
        Err(CodecError::UnsupportedFormatMarker(future_marker))
    );

    let mut stale_format = bytes.clone();
    stale_format[8..10].copy_from_slice(&55_u16.to_le_bytes());
    assert_eq!(
        decode_module(&stale_format),
        Err(CodecError::UnsupportedFormatMarker(55))
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

    let mut disjunction = fixture();
    disjunction.machines[0].contract.ensures[0].proposition = Proposition::Disjunction(vec![
        Proposition::Truth,
        Proposition::Disjunction(vec![Proposition::Truth, Proposition::Falsehood]),
    ]);
    assert_eq!(
        encode_module(&disjunction),
        Err(CodecError::NestedDisjunction)
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
        terminal_psi::CrashRouteBucket {
            cause: CrashCause::Abort,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        },
        terminal_psi::CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
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

/// The record field an inline byte presentation projects, and the container
/// field a nested presentation crosses before reaching it.
const BYTE_FIELD: &str = "payload";
const CONTAINER_FIELD: &str = "owner";

/// A standalone `BorrowedView` declaration, the record owning one inline byte
/// field under `carrier`, and a container reaching that record through one more
/// structural field.
///
/// The standalone declaration is what made the closed hole reachable: a field
/// typed `ByteSequence(BorrowedView)` shares its shape, so a path end that
/// resolved a leaf shape would have found this exact declaration and matched a
/// callee's view parameter by type equality alone.
fn inline_byte_carrier_types(carrier: ByteSequenceCarrier) -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "example::ByteView".to_owned(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        },
        StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "example::Buffer".to_owned(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: BYTE_FIELD.to_owned(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::ByteSequence(carrier),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type_id(3),
            identity: "example::BufferContainer".to_owned(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: structural_field_id(1),
                    identity: CONTAINER_FIELD.to_owned(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(structural_type_id(2)),
                }],
            },
        },
    ]
}

fn inline_byte_parameter(
    place: PlaceId,
    structural_type: StructuralTypeId,
    access: StructuralAccess,
) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn byte_field_path() -> Vec<StructuralPathSegment> {
    vec![StructuralPathSegment::Field(BYTE_FIELD.to_owned())]
}

fn nested_byte_field_path() -> Vec<StructuralPathSegment> {
    vec![
        StructuralPathSegment::Field(CONTAINER_FIELD.to_owned()),
        StructuralPathSegment::Field(BYTE_FIELD.to_owned()),
    ]
}

/// A boundary call presenting the caller's inline byte field to a borrowed-view
/// parameter. `root` is the caller's own structural parameter type and `path`
/// reaches the byte field from it.
fn boundary_inline_byte_module(
    carrier: ByteSequenceCarrier,
    root: StructuralTypeId,
    path: Vec<StructuralPathSegment>,
    access: StructuralAccess,
) -> TerminalModule {
    let mut module = unit_fixture();
    module.structural_types = inline_byte_carrier_types(carrier);
    module.boundary_machines = vec![BoundaryMachineDeclaration {
        fixed_service_reach: Vec::new(),
        crash_routes: Vec::new(),
        id: boundary_machine_id(1),
        identity: "example::write_buffer".to_owned(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![inline_byte_parameter(
            place_id(920),
            structural_type_id(1),
            access,
        )],
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    let machine = &mut module.machines[0];
    machine.structural_parameters = vec![inline_byte_parameter(
        place_id(910),
        root,
        StructuralAccess::MutableBorrow,
    )];
    machine.structural_places = vec![StructuralPlaceDeclaration {
        id: place_id(910),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    machine.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation_id(901),
        result: OperationResult::Unit,
        kind: OperationKind::BoundaryCall {
            boundary: boundary_machine_id(1),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: place_id(910),
                path,
                access,
            }],
            completion_receipts: Vec::new(),
        },
    }];
    module
}

/// The same presentation through an ordinary Unit call: the boundary's view
/// parameter becomes a module machine's, so the second implementation of the
/// argument check runs its `Ordinary` arm instead of the `Boundary` one.
fn ordinary_inline_byte_module(
    carrier: ByteSequenceCarrier,
    root: StructuralTypeId,
    path: Vec<StructuralPathSegment>,
) -> TerminalModule {
    let mut module =
        boundary_inline_byte_module(carrier, root, path, StructuralAccess::MutableBorrow);
    let view_parameter = module.boundary_machines[0].structural_parameters[0].clone();
    module.boundary_machines.clear();
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!("boundary presentation fixture")
    };
    let structural_arguments = structural_arguments.clone();
    let mut callee = module.machines[0].clone();
    callee.id = machine_id(901);
    callee.structural_places = vec![StructuralPlaceDeclaration {
        id: view_parameter.place,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    callee.structural_parameters = vec![view_parameter];
    callee.contract.id = contract_id(901);
    callee.entry = block_id(901);
    callee.blocks[0].id = block_id(901);
    callee.blocks[0].operations.clear();
    callee.blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(901),
        trivial_affine_discards: Vec::new(),
    };
    module.machines[0].blocks[0].operations[0].kind = OperationKind::CallUnit {
        erased_arguments: Vec::new(),
        callee: callee.id,
        arguments: Vec::new(),
        structural_arguments,
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    module.machines.push(callee);
    module
}

fn argument_path(module: &TerminalModule) -> &[StructuralPathSegment] {
    let (OperationKind::BoundaryCall {
        structural_arguments,
        ..
    }
    | OperationKind::CallUnit {
        structural_arguments,
        ..
    }) = &module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!("inline byte presentation fixture")
    };
    &structural_arguments[0].path
}

/// The codec's own path-custody refusal. `MalformedStructuralFoundation` is
/// raised only by this crate's foundation validation, which runs ahead of the
/// verifier's `validate_module_representation` in both `encode_module` and
/// `decode_module`; a verifier rejection would arrive as `InvalidModule`.
/// Asserting this exact variant therefore witnesses the codec's independent
/// reconstruction, not the verifier's.
fn path_custody_refusal() -> Result<Vec<u8>, CodecError> {
    Err(CodecError::MalformedStructuralFoundation(
        "structural path must retain structural custody",
    ))
}

#[test]
fn a_borrowed_view_field_cannot_supply_a_boundary_view_parameter() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::SharedBorrow,
    ] {
        let module = boundary_inline_byte_module(
            ByteSequenceCarrier::BorrowedView,
            structural_type_id(2),
            byte_field_path(),
            access,
        );
        assert_eq!(
            encode_module(&module),
            path_custody_refusal(),
            "{access:?} boundary loan of a borrowed-view field must reject"
        );
    }
}

#[test]
fn a_borrowed_view_field_cannot_supply_an_ordinary_view_parameter() {
    let module = ordinary_inline_byte_module(
        ByteSequenceCarrier::BorrowedView,
        structural_type_id(2),
        byte_field_path(),
    );
    assert_eq!(encode_module(&module), path_custody_refusal());
}

#[test]
fn a_nested_borrowed_view_field_cannot_supply_a_view_parameter() {
    let boundary = boundary_inline_byte_module(
        ByteSequenceCarrier::BorrowedView,
        structural_type_id(3),
        nested_byte_field_path(),
        StructuralAccess::MutableBorrow,
    );
    assert_eq!(encode_module(&boundary), path_custody_refusal());

    let ordinary = ordinary_inline_byte_module(
        ByteSequenceCarrier::BorrowedView,
        structural_type_id(3),
        nested_byte_field_path(),
    );
    assert_eq!(encode_module(&ordinary), path_custody_refusal());
}

#[test]
fn a_bounded_owned_field_keeps_every_admitted_inline_presentation() {
    let carrier = ByteSequenceCarrier::BoundedOwned { capacity: 16 };
    let admitted = [
        (
            boundary_inline_byte_module(
                carrier,
                structural_type_id(2),
                byte_field_path(),
                StructuralAccess::MutableBorrow,
            ),
            byte_field_path(),
        ),
        (
            boundary_inline_byte_module(
                carrier,
                structural_type_id(2),
                byte_field_path(),
                StructuralAccess::SharedBorrow,
            ),
            byte_field_path(),
        ),
        (
            boundary_inline_byte_module(
                carrier,
                structural_type_id(3),
                nested_byte_field_path(),
                StructuralAccess::MutableBorrow,
            ),
            nested_byte_field_path(),
        ),
        (
            ordinary_inline_byte_module(carrier, structural_type_id(2), byte_field_path()),
            byte_field_path(),
        ),
        (
            ordinary_inline_byte_module(carrier, structural_type_id(3), nested_byte_field_path()),
            nested_byte_field_path(),
        ),
    ];
    for (module, path) in admitted {
        let bytes = encode_module(&module).expect("an admitted inline presentation validates");
        let decoded = decode_module(&bytes).expect("decoding revalidates the same foundation");
        assert_eq!(argument_path(&decoded), path.as_slice());
        assert_eq!(decoded.structural_types, module.structural_types);
    }
}
