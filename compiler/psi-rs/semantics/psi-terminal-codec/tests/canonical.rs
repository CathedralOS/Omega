use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation,
    ContentDomainId, ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity,
    ContentStructuralPlace, ContentTerm, ContractId, EdgeId, IeeeFloatFormat, IntegerSign,
    IntegerType, IntegerValue, MachineId, ObligationId, OperationId, PlaceId, Proposition,
    PropositionId, ScalarTerm, ScalarType, ServiceId, StructuralCaseId, StructuralDomainId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_terminal::{
    BindingRelevance, Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimTransfer,
    CompletionReceipt, ContentEntryClaim, ContentIdentityReshuffle, ContentPartitionComposition,
    ContentPlaceSubstitution, ContractClause, CrashCause, EntryClaim, EvidenceInterfaceIdentity,
    FloatMeaningEqualityProposition, FloatMeaningProjection, FloatMeaningProjectionOperation,
    FloatProjectionInput, FloatProjectionInputId, InstallationReachDependency, MachineContract,
    NominalAffineCleanup, Operation, OperationKind, OperationResult, ProofOnlyValueType,
    ProofPropositionId, ProofValueDeclaration, ProofValueId, PropositionApplicationIdentity,
    PropositionBinderArgumentIdentity, PropositionBinderArgumentKind, PropositionBinderDeclaration,
    PropositionBinderKind, PropositionDeclaration, PropositionEvidence, ServiceDeclaration,
    StructuralAffineDiscard, StructuralArgument, StructuralCaseDeclaration,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalAffineCleanupAction,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use psi_terminal_codec::{
    CodecError, decode_module, encode_module, semantic_fingerprint, terminal_psi_identity,
};

#[test]
fn current_vocabulary_has_one_stable_canonical_encoding_and_identity() {
    let module = fixture();
    let bytes = encode_module(&module).expect("fixture should encode");

    assert_eq!(&bytes[..8], b"PSITERM\0");
    assert_eq!(&bytes[8..10], 20_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));

    let identity = terminal_psi_identity(&module).expect("fixture should have an identity");
    assert_eq!(identity.vocabulary_marker, VocabularyMarker::CURRENT);
    assert_eq!(
        identity.program_fingerprint.to_string(),
        "56c1ba0c889364f64d63755e55068bb77dae1de451856cbb7977470e7102744b"
    );
    assert_eq!(
        identity.program_fingerprint,
        semantic_fingerprint(&module).unwrap()
    );
}

#[test]
fn installation_reach_dependencies_round_trip_and_require_canonical_identity() {
    let mut module = fixture();
    module.services = vec![
        ServiceDeclaration {
            id: service_id(1),
            identity: "MachineControl".into(),
            parents: Vec::new(),
        },
        ServiceDeclaration {
            id: service_id(2),
            identity: "PortIo".into(),
            parents: Vec::new(),
        },
    ];
    module.root_service_reach.concrete = vec![service_id(1)];
    module.root_service_reach.installation_dependencies = vec![InstallationReachDependency {
        requirement_identity: "InterruptCompletion::complete".into(),
        upper_bound: vec![service_id(1), service_id(2)],
    }];

    let bytes = encode_module(&module).expect("installation reach dependency encodes");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    module.root_service_reach.installation_dependencies[0]
        .upper_bound
        .reverse();
    assert_eq!(
        encode_module(&module),
        Err(CodecError::NonCanonicalOrder(
            "installation reach upper bounds by ServiceId"
        ))
    );
}

#[test]
fn proof_only_float_projections_round_trip_and_reject_tampering() {
    let mut module = fixture();
    module.float_meaning_projections = vec![
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(0),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatProjectionInput {
                id: FloatProjectionInputId(0),
                format: IeeeFloatFormat::Binary32,
            },
            operation: FloatMeaningProjectionOperation::Meaning32,
        },
        FloatMeaningProjection {
            result: ProofValueDeclaration {
                id: ProofValueId(1),
                value_type: ProofOnlyValueType::FloatMeaning,
            },
            source: FloatProjectionInput {
                id: FloatProjectionInputId(1),
                format: IeeeFloatFormat::Binary64,
            },
            operation: FloatMeaningProjectionOperation::Meaning64,
        },
    ];
    module.float_meaning_equalities = vec![FloatMeaningEqualityProposition {
        id: ProofPropositionId(0),
        left: ProofValueId(0),
        right: ProofValueId(1),
    }];
    let bytes = encode_module(&module).expect("proof-only projections encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let mut reordered = module.clone();
    reordered.float_meaning_projections.swap(0, 1);
    assert!(matches!(
        encode_module(&reordered),
        Err(CodecError::NonCanonicalOrder(
            "float-meaning projections by dense proof value and source IDs"
        ))
    ));

    let mut unknown_operand = module.clone();
    unknown_operand.float_meaning_equalities[0].right = ProofValueId(2);
    assert!(matches!(
        encode_module(&unknown_operand),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::UnknownFloatMeaningEqualityOperand { .. }
        ))
    ));

    let mut noncanonical_operands = module.clone();
    noncanonical_operands.float_meaning_equalities[0].left = ProofValueId(1);
    noncanonical_operands.float_meaning_equalities[0].right = ProofValueId(0);
    assert!(matches!(
        encode_module(&noncanonical_operands),
        Err(CodecError::NonCanonicalOrder(
            "float-meaning equalities by dense proposition ID and ordered operands"
        ))
    ));

    let mut cross_format = module;
    cross_format.float_meaning_projections[0].source.format = IeeeFloatFormat::Binary64;
    assert!(matches!(
        encode_module(&cross_format),
        Err(CodecError::InvalidModule(
            psi_terminal_verifier::ModuleError::InvalidFloatMeaningProjection { .. }
        ))
    ));
}

#[test]
fn payload_sum_shape_round_trips_exact_fields_and_requires_canonical_order() {
    let mut valid = fixture();
    valid.structural_types.push(StructuralTypeDeclaration {
        id: structural_type_id(99),
        identity: "Mode".to_owned(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: structural_case_id(1),
                    identity: "Off".to_owned(),
                    fields: Vec::new(),
                },
                StructuralCaseDeclaration {
                    id: structural_case_id(2),
                    identity: "On".to_owned(),
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: structural_field_id(1),
                            identity: "enabled".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                        },
                        StructuralFieldDeclaration {
                            id: structural_field_id(2),
                            identity: "count".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
                            )),
                        },
                    ],
                },
            ],
        },
    });
    valid
        .structural_types
        .sort_by_key(|declaration| declaration.id);
    let bytes = encode_module(&valid).expect("canonical payload sum encodes");
    assert_eq!(decode_module(&bytes), Ok(valid.clone()));

    let mut reordered = valid.clone();
    let StructuralTypeShape::Sum { cases } = &mut reordered
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.identity == "Mode")
        .expect("sum")
        .shape
    else {
        unreachable!()
    };
    cases.reverse();
    assert_eq!(
        encode_module(&reordered),
        Err(CodecError::NonCanonicalOrder(
            "structural cases by StructuralCaseId"
        ))
    );

    let mut reordered_fields = valid.clone();
    let StructuralTypeShape::Sum { cases } = &mut reordered_fields
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.identity == "Mode")
        .expect("sum")
        .shape
    else {
        unreachable!()
    };
    cases[1].fields.reverse();
    assert_eq!(
        encode_module(&reordered_fields),
        Err(CodecError::NonCanonicalOrder(
            "structural case fields by StructuralFieldId"
        ))
    );

    let mut empty = valid;
    let StructuralTypeShape::Sum { cases } = &mut empty
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.identity == "Mode")
        .expect("sum")
        .shape
    else {
        unreachable!()
    };
    cases.clear();
    assert!(encode_module(&empty).is_err());
}

#[test]
fn partial_affine_unit_return_round_trips_exact_path_and_leaf_type() {
    let module = partial_affine_fixture();
    let bytes = encode_module(&module).expect("partial affine return should encode");
    assert_eq!(&bytes[8..10], 20_u16.to_le_bytes());
    assert_eq!(&bytes[10..12], 24_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn nominal_affine_unit_return_round_trips_exact_root_type_and_cleanup_machine() {
    let module = nominal_affine_fixture();
    let bytes = encode_module(&module).expect("nominal affine return should encode");
    assert_eq!(&bytes[8..10], 20_u16.to_le_bytes());
    assert_eq!(&bytes[10..12], 24_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn scalar_return_round_trips_nominal_affine_cleanup_action() {
    let mut module = nominal_affine_fixture();
    let machine = &mut module.machines[0];
    let source = ValueDeclaration {
        id: value_id(50),
        scalar_type: ScalarType::Boolean,
    };
    machine.parameters = vec![source];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(51),
        scalar_type: ScalarType::Boolean,
    });
    let Terminator::ReturnUnitNominalAffine { edge, cleanups } = &machine.blocks[0].terminator
    else {
        unreachable!()
    };
    machine.blocks[0].terminator = Terminator::Return {
        edge: *edge,
        value: source.id,
        cleanup_actions: vec![TerminalAffineCleanupAction::InvokeNominal(
            cleanups[0].clone(),
        )],
    };

    let bytes = encode_module(&module).expect("scalar nominal cleanup should encode");
    assert_eq!(&bytes[8..10], 20_u16.to_le_bytes());
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn contextual_nominal_affine_cleanup_round_trips_receiver_and_ordered_obligations() {
    let mut module = nominal_affine_fixture();
    let receiver = place_id(99);
    let first = StructuralFieldId::new(1).expect("first field");
    let second = StructuralFieldId::new(2).expect("second field");
    module.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![
            StructuralFieldDeclaration {
                id: first,
                identity: "ready".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            },
            StructuralFieldDeclaration {
                id: second,
                identity: "settled".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            },
        ],
    };
    module.machines[1].contract.requires = [first, second]
        .into_iter()
        .map(|field| {
            Proposition::Equal(
                ScalarTerm::boolean(true),
                ScalarTerm::boolean_field(receiver, field),
            )
        })
        .collect();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].cleanup_receiver = Some(receiver);
    cleanups[0].requirement_obligations = vec![obligation_id(8), obligation_id(3)];

    let bytes = encode_module(&module).expect("contextual nominal cleanup should encode");
    let decoded = decode_module(&bytes).expect("contextual nominal cleanup should decode");
    assert_eq!(decoded, module);
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &decoded.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(cleanups[0].cleanup_receiver, Some(receiver));
    assert_eq!(
        cleanups[0].requirement_obligations,
        vec![obligation_id(8), obligation_id(3)]
    );

    let mut truncated = bytes;
    truncated.pop();
    assert_eq!(decode_module(&truncated), Err(CodecError::UnexpectedEnd));

    let mut reordered = module.clone();
    reordered.machines[1].contract.requires.reverse();
    assert_eq!(
        encode_module(&reordered),
        Err(CodecError::NonCanonicalOrder("requires propositions"))
    );

    let mut duplicate = module;
    duplicate.machines[1].contract.requires[1] = duplicate.machines[1].contract.requires[0].clone();
    assert_eq!(
        encode_module(&duplicate),
        Err(CodecError::NonCanonicalOrder("requires propositions"))
    );
}

#[test]
fn nominal_affine_unit_return_round_trips_two_roots_in_reverse_parameter_order() {
    let module = two_nominal_affine_fixture();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        cleanups
            .iter()
            .map(|cleanup| cleanup.place)
            .collect::<Vec<_>>(),
        vec![place_id(2), place_id(1)]
    );

    let bytes = encode_module(&module).expect("two nominal affine roots should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));

    let mut reordered = module;
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut reordered.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups.reverse();
    assert_eq!(
        encode_module(&reordered),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup list is not in reverse parameter order"
        ))
    );
}

#[test]
fn nominal_affine_unit_return_round_trips_five_roots() {
    let module = five_nominal_affine_fixture();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    assert_eq!(
        cleanups
            .iter()
            .map(|cleanup| cleanup.place)
            .collect::<Vec<_>>(),
        vec![
            place_id(5),
            place_id(4),
            place_id(3),
            place_id(2),
            place_id(1)
        ]
    );
    let bytes = encode_module(&module).expect("five nominal affine roots should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
}

#[test]
fn nominal_affine_unit_return_rejects_malformed_source_carriers() {
    let mut valued = nominal_affine_fixture();
    valued.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(99),
        scalar_type: ScalarType::Integer(i32_type()),
    });
    assert_eq!(
        encode_module(&valued),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup requires a Unit result"
        ))
    );

    let mut wrong_type = nominal_affine_fixture();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut wrong_type.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].structural_type = structural_type_id(2);
    assert_eq!(
        encode_module(&wrong_type),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup type does not match its structural parameter"
        ))
    );

    let mut claimed = nominal_affine_fixture();
    let place = claimed.machines[0].structural_parameters[0].place;
    claimed.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(1),
        input: place,
        path: Vec::new(),
    });
    assert_eq!(
        encode_module(&claimed),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup is duplicated or not a claim-free qualified-free affine root"
        ))
    );

    let mut unrestricted = nominal_affine_fixture();
    unrestricted.machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Unrestricted;
    assert_eq!(
        encode_module(&unrestricted),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup is duplicated or not a claim-free qualified-free affine root"
        ))
    );

    let mut qualified = nominal_affine_fixture();
    qualified
        .structural_domains
        .push(StructuralDomainDeclaration {
            id: structural_domain_id(1),
            identity: "example::NominalResource::Ready".to_owned(),
            carrier: structural_type_id(1),
        });
    qualified.machines[0].structural_parameters[0]
        .qualifications
        .push(structural_domain_id(1));
    assert_eq!(
        encode_module(&qualified),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup is duplicated or not a claim-free qualified-free affine root"
        ))
    );

    let mut missing_root = nominal_affine_fixture();
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
        &mut missing_root.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups[0].place = place_id(99);
    assert_eq!(
        encode_module(&missing_root),
        Err(CodecError::MalformedStructuralFoundation(
            "nominal affine cleanup root is not a structural parameter"
        ))
    );
}

#[test]
fn partial_affine_unit_return_rejects_corrupt_path_type_and_duplicate_action() {
    let mut wrong_type = partial_affine_fixture();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut wrong_type.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards[0].structural_type = structural_type_id(1);
    assert_eq!(
        encode_module(&wrong_type),
        Err(CodecError::MalformedStructuralFoundation(
            "partial affine cleanup leaf type does not match its path"
        ))
    );

    let mut duplicate = partial_affine_fixture();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut duplicate.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.push(residual_affine_discards[0].clone());
    assert_eq!(
        encode_module(&duplicate),
        Err(CodecError::NonCanonicalOrder(
            "partial affine residual discards are unique"
        ))
    );

    let mut corrupt_bytes = encode_module(&partial_affine_fixture()).unwrap();
    let field = corrupt_bytes
        .windows(4)
        .rposition(|window| window == b"left")
        .expect("encoded residual field identity");
    corrupt_bytes[field..field + 4].copy_from_slice(b"gone");
    assert_eq!(
        decode_module(&corrupt_bytes),
        Err(CodecError::MalformedStructuralFoundation(
            "structural path has an unknown structural field"
        ))
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
fn unit_return_affine_discard_round_trips_canonically() {
    let mut module = structural_effect_fixture();
    for machine in &mut module.machines {
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        machine.entry_claims.clear();
    }
    let OperationKind::CallUnit {
        claim_transfers, ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    claim_transfers.clear();
    module.machines[1].blocks[0].operations.clear();
    let callee_place = module.machines[1].structural_parameters[0].place;
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut module.machines[1].blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![callee_place];

    let bytes = encode_module(&module).expect("explicit affine discard should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn trivial_affine_local_declaration_and_establishment_round_trip_canonically() {
    let source_type = structural_type_id(1);
    let local_type = structural_type_id(2);
    let source = place_id(1);
    let local = place_id(50);
    let result = place_id(51);
    let first_affine = place_id(52);
    let second_affine = place_id(53);
    let machine = TerminalMachine {
        id: machine_id(1),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![
            StructuralParameterDeclaration {
                place: source,
                position: 0,
                is_self: false,
                structural_type: source_type,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: Vec::new(),
            },
            StructuralParameterDeclaration {
                place: first_affine,
                position: 1,
                is_self: false,
                structural_type: local_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            },
            StructuralParameterDeclaration {
                place: second_affine,
                position: 2,
                is_self: false,
                structural_type: local_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            },
        ],
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: result,
            structural_type: source_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: Vec::new(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: source,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: local,
                kind: StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal: 0,
                    structural_type: local_type,
                },
            },
            StructuralPlaceDeclaration {
                id: result,
                kind: StructuralPlaceKind::Result,
            },
            StructuralPlaceDeclaration {
                id: first_affine,
                kind: StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: second_affine,
                kind: StructuralPlaceKind::Parameter {
                    position: 2,
                    is_self: false,
                },
            },
        ],
        entry_claims: vec![EntryClaim {
            claim: claim_id(1),
            input: source,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(1),
        blocks: vec![Block {
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(1),
                result: OperationResult::Unit,
                kind: OperationKind::EstablishTrivialAffineLocal { destination: local },
            }],
            terminator: Terminator::ReturnStructural {
                edge: edge_id(1),
                source,
                returned_claims: vec![claim_id(1)],
                trivial_affine_discards: vec![local, second_affine, first_affine],
            },
        }],
        contract: MachineContract {
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
        },
    };
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: source_type,
                identity: "Region".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: local_type,
                identity: "EmptyScratch".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![machine],
    };
    let bytes = encode_module(&module).expect("local semantic rows encode");
    assert_eq!(decode_module(&bytes), Ok(module));
}
#[test]
fn scalar_jump_affine_discard_round_trips_canonically() {
    let mut module = structural_effect_fixture();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.blocks[0].operations.clear();
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    machine.parameters = vec![ValueDeclaration {
        id: value_id(50),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(51),
        scalar_type: ScalarType::Boolean,
    });
    let place = machine.structural_parameters[0].place;
    machine.blocks[0].terminator = Terminator::Jump {
        edge: edge_id(101),
        target: block_id(102),
        arguments: vec![value_id(50)],
        trivial_affine_discards: vec![place],
    };
    machine.blocks.push(Block {
        id: block_id(102),
        parameters: vec![ValueDeclaration {
            id: value_id(52),
            scalar_type: ScalarType::Boolean,
        }],
        operations: Vec::new(),
        terminator: Terminator::Return {
            edge: edge_id(102),
            value: value_id(52),
            cleanup_actions: Vec::new(),
        },
    });
    module.entry = machine.id;
    module.machines = vec![machine];

    let bytes = encode_module(&module).expect("jump affine discard should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn conditional_affine_discards_round_trip_canonically() {
    let mut module = structural_effect_fixture();
    let mut machine = module.machines.pop().expect("callee machine");
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    machine.parameters = vec![ValueDeclaration {
        id: value_id(50),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: value_id(51),
        scalar_type: ScalarType::Boolean,
    });
    let place = machine.structural_parameters[0].place;
    machine.blocks = vec![
        Block {
            id: block_id(101),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(50),
                when_true: SuccessorEdge {
                    edge: edge_id(101),
                    target: block_id(102),
                    arguments: vec![value_id(50)],
                    trivial_affine_discards: vec![place],
                },
                when_false: SuccessorEdge {
                    edge: edge_id(102),
                    target: block_id(103),
                    arguments: vec![value_id(50)],
                    trivial_affine_discards: vec![place],
                },
            },
        },
        Block {
            id: block_id(102),
            parameters: vec![ValueDeclaration {
                id: value_id(52),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(103),
                value: value_id(52),
                cleanup_actions: Vec::new(),
            },
        },
        Block {
            id: block_id(103),
            parameters: vec![ValueDeclaration {
                id: value_id(53),
                scalar_type: ScalarType::Boolean,
            }],
            operations: Vec::new(),
            terminator: Terminator::Return {
                edge: edge_id(104),
                value: value_id(53),
                cleanup_actions: Vec::new(),
            },
        },
    ];
    module.entry = machine.id;
    module.machines = vec![machine];

    let bytes = encode_module(&module).expect("conditional affine discards should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn structural_effect_foundation_round_trips_and_has_stable_identity() {
    let module = structural_effect_fixture();
    let bytes = encode_module(&module).expect("structural/effect foundation should encode");

    assert_eq!(&bytes[10..12], 24_u16.to_le_bytes());
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
fn projected_ordinary_unit_argument_round_trips_canonically() {
    let mut module = structural_effect_fixture();
    let element = structural_type_id(1);
    let array = structural_type_id(4);
    module.structural_types.push(StructuralTypeDeclaration {
        id: array,
        identity: "example::OccurrencePair".to_owned(),
        shape: StructuralTypeShape::FixedArray { element, length: 2 },
    });
    module.machines[0].structural_parameters[0].structural_type = array;
    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    module.machines[0].structural_parameters[0]
        .qualifications
        .clear();
    module.machines[0].entry_claims[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    module.machines[1].structural_parameters[0]
        .qualifications
        .clear();
    module.boundary_machines[0].requires.clear();
    module.boundary_machines[0].structural_parameters[0]
        .qualifications
        .clear();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(1)];

    let bytes = encode_module(&module).expect("projected ordinary call encodes");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn structural_result_and_return_round_trip_as_semantic_identity() {
    let mut module = structural_effect_fixture();
    module.machines.truncate(1);
    module.boundary_machines.clear();
    let machine = &mut module.machines[0];
    machine.blocks[0].operations.clear();
    let result_place = place_id(11);
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: result_place,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: vec![structural_domain_id(1)],
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: result_place,
        kind: StructuralPlaceKind::Result,
    });
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(100),
        source: place_id(10),
        returned_claims: vec![claim_id(1)],
        trivial_affine_discards: Vec::new(),
    };

    let bytes = encode_module(&module).expect("structural return should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    let baseline = semantic_fingerprint(&module).unwrap();
    assert_eq!(
        baseline,
        semantic_fingerprint(&decode_module(&bytes).unwrap()).unwrap()
    );
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    returned_claims.clear();
    assert!(encode_module(&module).is_err());
}

#[test]
fn erased_structural_field_round_trips_and_changes_semantic_identity() {
    let baseline = structural_effect_fixture();
    let mut module = baseline.clone();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
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
    let mut baseline = structural_effect_fixture();
    project_boundary_argument(
        &mut baseline,
        vec![StructuralPathSegment::Field("metadata".to_owned())],
        structural_type_id(2),
    );
    let mut module = baseline.clone();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[1].identity = "#7".to_owned();
    project_boundary_path_only(
        &mut module,
        vec![StructuralPathSegment::Field("#7".to_owned())],
    );

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
    let mut baseline = structural_effect_fixture();
    let StructuralTypeShape::Record { fields } = &mut baseline.structural_types[1].shape else {
        unreachable!()
    };
    fields.push(StructuralFieldDeclaration {
        id: structural_field_id(1),
        identity: "inner".to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(3)),
    });
    project_boundary_argument(
        &mut baseline,
        vec![
            StructuralPathSegment::Field("metadata".to_owned()),
            StructuralPathSegment::Field("inner".to_owned()),
        ],
        structural_type_id(3),
    );
    let mut module = baseline.clone();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[1].identity = "#7".to_owned();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[1].shape else {
        unreachable!()
    };
    fields[0].identity = "#9".to_owned();
    project_boundary_path_only(
        &mut module,
        vec![
            StructuralPathSegment::Field("#7".to_owned()),
            StructuralPathSegment::Field("#9".to_owned()),
        ],
    );

    let bytes = encode_module(&module).expect("nested aggregate claim path should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_ne!(
        semantic_fingerprint(&module).unwrap(),
        semantic_fingerprint(&baseline).unwrap(),
        "every nested path segment enters terminal semantic identity"
    );
}

#[test]
fn fixed_array_claim_and_argument_paths_round_trip_canonically() {
    let module = fixed_array_custody_fixture();
    let bytes = encode_module(&module).expect("literal fixed-index custody should encode");

    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn structural_foundation_rejects_an_out_of_bounds_fixed_index() {
    let mut module = fixed_array_custody_fixture();
    module.machines[0].entry_claims[0].path = vec![StructuralPathSegment::FixedIndex(2)];

    assert_eq!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "structural path fixed index is out of bounds"
        ))
    );
}

#[test]
fn structural_foundation_requires_claim_and_argument_paths_to_match() {
    let mut module = fixed_array_custody_fixture();
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(0)];

    assert_eq!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "claim action does not match its structural argument path"
        ))
    );
}

#[test]
fn disjoint_sibling_claim_set_round_trips_as_canonical_identity() {
    let baseline = structural_effect_fixture();
    let mut module = baseline.clone();
    let StructuralTypeShape::Record { fields } = &mut module.structural_types[0].shape else {
        unreachable!()
    };
    fields[1].identity = "#7".to_owned();
    fields.push(StructuralFieldDeclaration {
        id: structural_field_id(3),
        identity: "#9".to_owned(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(structural_type_id(2)),
    });
    project_boundary_argument(
        &mut module,
        vec![StructuralPathSegment::Field("#7".to_owned())],
        structural_type_id(2),
    );
    let machine_place = module.machines[0].structural_parameters[0].place;
    module.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(2),
        input: machine_place,
        path: vec![StructuralPathSegment::Field("#9".to_owned())],
    });
    let mut second_call = module.machines[0].blocks[0].operations[1].clone();
    second_call.id = operation_id(4);
    let OperationKind::BoundaryCall {
        structural_arguments,
        completion_receipts,
        ..
    } = &mut second_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::Field("#9".to_owned())];
    completion_receipts[0].claim = claim_id(2);
    module.machines[0].blocks[0].operations.push(second_call);

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
    let StructuralTypeShape::Record { fields } = &mut opaque_relevant.structural_types[0].shape
    else {
        unreachable!()
    };
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
    let StructuralTypeShape::Record { fields } = &mut nonopaque_erased.structural_types[0].shape
    else {
        unreachable!()
    };
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
fn decoder_rejects_the_previous_vocabulary_marker() {
    let mut bytes = encode_module(&structural_effect_fixture()).unwrap();
    bytes[10..12].copy_from_slice(&23_u16.to_le_bytes());

    assert_eq!(
        decode_module(&bytes),
        Err(CodecError::UnsupportedVocabularyMarker(23))
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
    let mut machine_prefix = 1_u32.to_le_bytes().to_vec(); // one machine
    machine_prefix.extend(machine_id(900).get().to_le_bytes());
    machine_prefix.push(0); // no attachment
    machine_prefix.extend(0_u32.to_le_bytes()); // no scalar parameters
    machine_prefix.extend(0_u32.to_le_bytes()); // no structural parameters
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
    future_format[8..10].copy_from_slice(&21_u16.to_le_bytes());
    assert_eq!(
        decode_module(&future_format),
        Err(CodecError::UnsupportedFormatMarker(21))
    );

    let mut stale_format = bytes.clone();
    stale_format[8..10].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_module(&stale_format),
        Err(CodecError::UnsupportedFormatMarker(1))
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

fn partial_affine_fixture() -> TerminalModule {
    let pair_type = structural_type_id(1);
    let token_type = structural_type_id(2);
    let root_type = structural_type_id(3);
    let sink_type = structural_type_id(4);
    let pair_place = place_id(1);
    let token_place = place_id(2);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: pair_type,
                identity: "example::Pair".to_owned(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: structural_field_id(1),
                            identity: "left".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(token_type),
                        },
                        StructuralFieldDeclaration {
                            id: structural_field_id(2),
                            identity: "right".to_owned(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(token_type),
                        },
                    ],
                },
            },
            StructuralTypeDeclaration {
                id: token_type,
                identity: "example::Token".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: root_type,
                identity: "example::Root".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: sink_type,
                identity: "example::Sink".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: Some(root_type),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: pair_place,
                    position: 0,
                    is_self: false,
                    structural_type: pair_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                }],
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: pair_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: operation_id(1),
                        result: OperationResult::Unit,
                        kind: OperationKind::CallUnit {
                            callee: machine_id(2),
                            structural_arguments: vec![StructuralArgument {
                                place: pair_place,
                                path: vec![StructuralPathSegment::Field("right".to_owned())],
                            }],
                            claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnUnitPartialAffine {
                        edge: edge_id(1),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: vec![StructuralAffineDiscard {
                            place: pair_place,
                            path: vec![StructuralPathSegment::Field("left".to_owned())],
                            structural_type: token_type,
                        }],
                    },
                }],
                contract: MachineContract {
                    id: contract_id(1),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: machine_id(2),
                attachment: Some(sink_type),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: token_place,
                    position: 0,
                    is_self: false,
                    structural_type: token_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                }],
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: token_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(2),
                        trivial_affine_discards: vec![token_place],
                    },
                }],
                contract: MachineContract {
                    id: contract_id(2),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            },
        ],
    }
}

fn two_nominal_affine_fixture() -> TerminalModule {
    let mut module = nominal_affine_fixture();
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 1,
            is_self: false,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place_id(2),
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    let Terminator::ReturnUnitNominalAffine { cleanups, .. } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    cleanups.insert(
        0,
        NominalAffineCleanup {
            place: place_id(2),
            structural_type: structural_type_id(1),
            cleanup_machine: machine_id(2),
            cleanup_receiver: None,
            requirement_obligations: Vec::new(),
        },
    );
    module
}

fn five_nominal_affine_fixture() -> TerminalModule {
    let mut module = two_nominal_affine_fixture();
    let machine = &mut module.machines[0];
    for position in 2_u32..5 {
        let place = place_id(u64::from(position + 1));
        machine
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position,
                is_self: false,
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            });
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position,
                is_self: false,
            },
        });
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } =
            &mut machine.blocks[0].terminator
        else {
            unreachable!()
        };
        cleanups.insert(
            0,
            NominalAffineCleanup {
                place,
                structural_type: structural_type_id(1),
                cleanup_machine: machine_id(2),
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            },
        );
    }
    module
}

fn nominal_affine_fixture() -> TerminalModule {
    let resource_type = structural_type_id(1);
    let owner_type = structural_type_id(2);
    let source_place = place_id(1);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: resource_type,
                identity: "example::NominalResource".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: owner_type,
                identity: "example::Owner".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: Some(owner_type),
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: source_place,
                    position: 0,
                    is_self: false,
                    structural_type: resource_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                }],
                result: TerminalMachineResult::Unit,
                structural_places: vec![StructuralPlaceDeclaration {
                    id: source_place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                }],
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnitNominalAffine {
                        edge: edge_id(1),
                        cleanups: vec![NominalAffineCleanup {
                            place: source_place,
                            structural_type: resource_type,
                            cleanup_machine: machine_id(2),
                            cleanup_receiver: None,
                            requirement_obligations: Vec::new(),
                        }],
                    },
                }],
                contract: MachineContract {
                    id: contract_id(1),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: machine_id(2),
                attachment: Some(resource_type),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(2),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: contract_id(2),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                },
            },
        ],
    }
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
        root_service_reach: Default::default(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_machine_id(1),
            identity: "example::Occurrence::settle".to_owned(),
            attachment: Some(resource_type),
            scalar_parameters: Vec::new(),
            structural_parameters: vec![structural_parameter(place_id(30), 0, resource_type, true)],
            result: None,
            requires: vec![StructuralDomainRequirement {
                argument_index: 0,
                domain,
            }],
            published_service_ceiling: vec![service],
        }],
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
                    path: Vec::new(),
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
                                path: Vec::new(),
                            }],
                            claim_transfers: vec![ClaimTransfer {
                                claim: claim_id(1),
                                argument_index: 0,
                            }],
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(100),
                        trivial_affine_discards: Vec::new(),
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
                    path: Vec::new(),
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
                            kind: OperationKind::BoundaryCall {
                                boundary: boundary_machine_id(1),
                                arguments: Vec::new(),
                                structural_arguments: vec![StructuralArgument {
                                    place: callee_place,
                                    path: Vec::new(),
                                }],
                                completion_receipts: vec![CompletionReceipt {
                                    claim: claim_id(1),
                                    argument_index: 0,
                                }],
                                requirement_obligations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::ReturnUnit {
                        edge: edge_id(101),
                        trivial_affine_discards: Vec::new(),
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

#[test]
fn boundary_scalar_parameter_and_argument_order_round_trips_canonically() {
    let mut module = structural_effect_fixture();
    let first = value_id(1);
    let second = value_id(2);
    module.boundary_machines[0].scalar_parameters = vec![ScalarType::Boolean; 2];
    let operations = &mut module.machines[1].blocks[0].operations;
    operations[0].id = operation_id(4);
    operations[1].id = operation_id(5);
    operations.splice(
        0..0,
        [
            Operation {
                id: operation_id(2),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: first,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: false },
            },
            Operation {
                id: operation_id(3),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: second,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: true },
            },
        ],
    );
    let OperationKind::BoundaryCall { arguments, .. } =
        &mut module.machines[1].blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    *arguments = vec![second, first];

    let bytes = encode_module(&module).expect("scalar boundary lanes should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    let mut reordered = module;
    let OperationKind::BoundaryCall { arguments, .. } =
        &mut reordered.machines[1].blocks[0].operations[3].kind
    else {
        unreachable!()
    };
    arguments.swap(0, 1);
    assert_ne!(
        semantic_fingerprint(&reordered).unwrap(),
        semantic_fingerprint(&decode_module(&bytes).unwrap()).unwrap(),
        "boundary scalar argument order is semantic",
    );
}

#[test]
fn structural_foundation_rejects_wrong_boundary_scalar_arity() {
    let mut module = structural_effect_fixture();
    module.boundary_machines[0].scalar_parameters = vec![ScalarType::Integer(i32_type())];

    assert_eq!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "boundary call has the wrong scalar arity"
        )),
    );
}

fn project_boundary_argument(
    module: &mut TerminalModule,
    path: Vec<StructuralPathSegment>,
    projected_type: StructuralTypeId,
) {
    let boundary_caller = module.machines.pop().expect("boundary caller fixture");
    module.machines = vec![boundary_caller];
    module.entry = module.machines[0].id;

    let boundary = &mut module.boundary_machines[0];
    boundary.requires.clear();
    boundary.attachment = Some(projected_type);
    boundary.structural_parameters[0].structural_type = projected_type;
    boundary.structural_parameters[0].qualifications.clear();
    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    module.machines[0].structural_parameters[0]
        .qualifications
        .clear();
    project_boundary_path_only(module, path);
}

fn project_boundary_path_only(module: &mut TerminalModule, path: Vec<StructuralPathSegment>) {
    module.machines[0].entry_claims[0].path = path.clone();
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = path;
}

fn fixed_array_custody_fixture() -> TerminalModule {
    let mut module = structural_effect_fixture();
    let element = structural_type_id(1);
    let array = structural_type_id(4);
    module.structural_types.push(StructuralTypeDeclaration {
        id: array,
        identity: "example::OccurrencePair".to_owned(),
        shape: StructuralTypeShape::FixedArray { element, length: 2 },
    });

    let boundary_caller = module.machines.pop().expect("boundary caller fixture");
    module.machines = vec![boundary_caller];
    module.entry = module.machines[0].id;

    for boundary in &mut module.boundary_machines {
        boundary.requires.clear();
        for parameter in &mut boundary.structural_parameters {
            parameter.qualifications.clear();
        }
    }
    for machine in &mut module.machines {
        for parameter in &mut machine.structural_parameters {
            parameter.multiplicity = StructuralMultiplicity::Affine;
            parameter.qualifications.clear();
        }
    }

    module.machines[0].structural_parameters[0].structural_type = array;
    module.machines[0].entry_claims[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    module
}

fn unit_fixture() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(900),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(900),
                    trivial_affine_discards: Vec::new(),
                },
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
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
                        trivial_affine_discards: Vec::new(),
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
                        cleanup_actions: Vec::new(),
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
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
                    cleanup_actions: Vec::new(),
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
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
                        cleanup_actions: Vec::new(),
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
                        cleanup_actions: Vec::new(),
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
id_constructor!(structural_case_id, StructuralCaseId);
id_constructor!(structural_domain_id, StructuralDomainId);
id_constructor!(service_id, ServiceId);
id_constructor!(boundary_machine_id, BoundaryMachineId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(proposition_id, PropositionId);
