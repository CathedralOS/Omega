use super::{
    fixed_array_custody_fixture, fixture, partial_affine_fixture, project_boundary_argument,
    project_boundary_path_only, provider_attachment_root, provider_boundary_call,
    structural_effect_fixture, unit_fixture, unused_provider_attachment_fixture,
};
use crate::canonical::{
    block_id, claim_id, contract_id, edge_id, machine_id, operation_id, place_id,
    structural_domain_id, structural_field_id, structural_type_id, value_id,
};
use semantic_vocabulary::{ScalarType, StructuralPlaceKind};
use terminal_codec::{CodecError, decode_module, encode_module, semantic_fingerprint};
use terminal_psi::{
    BindingRelevance, Block, EntryClaim, MachineContract, Operation, OperationKind,
    OperationResult, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    Terminator, ValueDeclaration, VocabularyMarker,
};

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
    module.root_service_reach.concrete.clear();

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
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
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
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            StructuralParameterDeclaration {
                place: first_affine,
                position: 1,
                is_self: false,
                structural_type: local_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            StructuralParameterDeclaration {
                place: second_affine,
                position: 2,
                is_self: false,
                structural_type: local_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: result,
            structural_type: source_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
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
                    construction: None,
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
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
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
            erased_scalar_formals: Vec::new(),
            id: contract_id(1),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
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
        qualifications: Default::default(),
        id: value_id(50),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(51),
        scalar_type: ScalarType::Boolean,
    });
    let place = machine.structural_parameters[0].place;
    machine.blocks[0].terminator = Terminator::Jump {
        structural_arguments: Vec::new(),
        edge: edge_id(101),
        target: block_id(102),
        arguments: vec![value_id(50)],
        erased_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: vec![place],
    };
    machine.blocks.push(Block {
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        id: block_id(102),
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
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
    module.root_service_reach.concrete.clear();

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
        qualifications: Default::default(),
        id: value_id(50),
        scalar_type: ScalarType::Boolean,
    }];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(51),
        scalar_type: ScalarType::Boolean,
    });
    let place = machine.structural_parameters[0].place;
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(101),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: value_id(50),
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(101),
                    target: block_id(102),
                    arguments: vec![value_id(50)],
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: vec![place],
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(102),
                    target: block_id(103),
                    arguments: vec![value_id(50)],
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: vec![place],
                },
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(102),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
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
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: block_id(103),
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
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
    module.root_service_reach.concrete.clear();

    let bytes = encode_module(&module).expect("conditional affine discards should encode");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
}

#[test]
fn structural_effect_foundation_round_trips_and_has_stable_identity() {
    let module = structural_effect_fixture();
    let bytes = encode_module(&module).expect("structural/effect foundation should encode");

    assert_eq!(
        &bytes[10..12],
        VocabularyMarker::CURRENT.get().to_le_bytes()
    );
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
    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Linear;
    module.machines[0].structural_parameters[0]
        .qualifications
        .clear();
    module.machines[0].entry_claims[0].path = vec![StructuralPathSegment::FixedIndex(0)];
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
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(0)];
    let caller_input = module.machines[0].structural_parameters[0].place;
    module.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(2),
        input: caller_input,
        path: vec![StructuralPathSegment::FixedIndex(1)],
    });
    let mut second_call = module.machines[0].blocks[0].operations[0].clone();
    second_call.id = operation_id(4);
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut second_call.kind
    else {
        unreachable!()
    };
    structural_arguments[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    claim_transfers[0].claim = claim_id(2);
    module.machines[0].blocks[0].operations.push(second_call);

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
        reference_sources: Vec::new(),
        place: result_place,
        structural_type: structural_type_id(1),
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: vec![structural_domain_id(1)],
        projected_qualifications: Vec::new(),
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
    module.root_service_reach.concrete.clear();

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
    } = &mut module.machines[0].blocks[0].operations[2].kind
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
fn unused_provider_attachment_round_trips_with_exact_relevant_opaque_identity() {
    let module = unused_provider_attachment_fixture();
    let bytes = encode_module(&module).expect("unused attachment encodes without roots or calls");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));
    assert_eq!(encode_module(&decode_module(&bytes).unwrap()), Ok(bytes));
    let identity = semantic_fingerprint(&module).unwrap();

    let mut changed = module.clone();
    let StructuralTypeShape::Record { fields } = &mut changed.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].field_type = StructuralFieldType::Erased {
        type_identity: "named(name(example::OtherProvider))".into(),
    };
    assert_ne!(semantic_fingerprint(&changed).unwrap(), identity);

    let mut erased = module;
    let StructuralTypeShape::Record { fields } = &mut erased.structural_types[0].shape else {
        unreachable!()
    };
    fields[0].relevance = BindingRelevance::Erased;
    assert_ne!(semantic_fingerprint(&erased).unwrap(), identity);
}

#[test]
fn provider_attachment_encoding_requires_exact_direct_call_roots() {
    let mut module = unused_provider_attachment_fixture();
    module.machines[0].blocks[0]
        .operations
        .push(provider_boundary_call());
    let incomplete = Err(CodecError::MalformedStructuralFoundation(
        "provider-backed attachment specialization is incomplete",
    ));
    assert_eq!(encode_module(&module), incomplete);

    module.machines[0]
        .structural_places
        .push(provider_attachment_root());
    let bytes = encode_module(&module).expect("one direct call has its exact provider root");
    assert_eq!(decode_module(&bytes), Ok(module.clone()));

    module.machines[0].blocks[0].operations.clear();
    assert_eq!(encode_module(&module), incomplete, "orphan root rejects");
}

#[test]
fn unused_provider_attachment_encoding_rejects_runtime_field_projection() {
    let mut module = unused_provider_attachment_fixture();
    module.machines[0]
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: place_id(2),
            position: 0,
            is_self: true,
            structural_type: structural_type_id(1),
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: place_id(2),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        });
    module.machines[0].entry_claims.push(EntryClaim {
        claim: claim_id(1),
        input: place_id(2),
        path: vec![StructuralPathSegment::Field("console".into())],
    });
    assert_eq!(
        encode_module(&module),
        Err(CodecError::MalformedStructuralFoundation(
            "structural path must retain structural custody",
        )),
    );
}

#[test]
fn unused_provider_attachment_encoding_rejects_nonattachment_and_multiple_fields() {
    let mut unattached = unused_provider_attachment_fixture();
    unattached.machines[0].attachment = None;
    let incomplete = Err(CodecError::MalformedStructuralFoundation(
        "provider-backed attachment specialization is incomplete",
    ));
    assert_eq!(encode_module(&unattached), incomplete);
    unattached.machines[0]
        .structural_places
        .push(provider_attachment_root());
    assert_eq!(
        encode_module(&unattached),
        incomplete,
        "a forged root cannot authorize a type"
    );

    let mut multiple = unused_provider_attachment_fixture();
    let StructuralTypeShape::Record { fields } = &mut multiple.structural_types[0].shape else {
        unreachable!()
    };
    let mut second = fields[0].clone();
    second.id = structural_field_id(2);
    second.identity = "second".into();
    fields.push(second);
    assert_eq!(encode_module(&multiple), incomplete);
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
            "provider-backed attachment specialization is incomplete"
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
    bytes[10..12].copy_from_slice(&36_u16.to_le_bytes());

    assert_eq!(
        decode_module(&bytes),
        Err(CodecError::UnsupportedVocabularyMarker(36))
    );
}
