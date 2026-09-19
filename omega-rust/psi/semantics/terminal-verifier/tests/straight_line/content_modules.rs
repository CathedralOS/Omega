//! Content fixtures: identity reshuffle, structural call, partition
//! composition and reflexive content modules.

use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation,
    ContentDomainId, ContentPlaceSegment, ContentPlaceVersion, ContentProjectionExpression,
    ContentProjectionIdentity, ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    ContractId, EdgeId, MachineId, ObligationId, OperationId, PlaceId, Proposition, ScalarType,
    StructuralPlaceKind, StructuralTypeId, ValueId, content_conservation_report_fingerprint,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimTransfer, CompletionReceipt,
    ContentConservationGuarantee, ContentEntryClaim, ContentIdentityReshuffle,
    ContentPartitionComposition, ContentPlaceSubstitution, ContractClause, EntryClaim,
    MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralContentProjection, StructuralDomainDeclaration,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralResultClaimBinding, StructuralResultClaimTransfer,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};

pub(super) fn identity_reshuffle_module() -> (TerminalModule, Proposition, ObligationId) {
    let input_root = PlaceId::new(90).expect("input place");
    let output_root = PlaceId::new(91).expect("output place");
    let structural_type = StructuralTypeId::new(90).expect("structural type");
    let claim = ClaimId::new(1).expect("claim");
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Byte".to_owned(),
    };
    let expression = ContentProjectionExpression::CountedQuantity(
        ContentProjectionScalar::Natural("1".to_owned()),
    );
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(90).expect("content domain"),
        projection_report_fingerprint:
            language_semantics::content::terminal_projection_report_fingerprint(
                &algebra,
                &expression,
            ),
    };
    let reshuffle = ContentIdentityReshuffle {
        claim,
        input: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: input_root,
            segments: Vec::new(),
        },
        output: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root: output_root,
            segments: Vec::new(),
        },
        projections: vec![ClaimContentProjection {
            projection,
            algebra: algebra.clone(),
        }],
    };
    let goal = reshuffle
        .inferred_propositions()
        .next()
        .expect("one projection yields one proposition");
    let obligation = ObligationId::new(90).expect("obligation");
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(90).expect("machine"),
        attachment: None,
        structural_parameters: vec![StructuralParameterDeclaration {
            place: input_root,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        entry_claims: vec![terminal_psi::EntryClaim {
            claim,
            input: input_root,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place: output_root,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: input_root,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: output_root,
                kind: StructuralPlaceKind::Result,
            },
        ],
        content_entry_claims: vec![ContentEntryClaim {
            claim,
            input: reshuffle.input.clone(),
            projections: reshuffle.projections.clone(),
        }],
        content_identity_reshuffles: vec![reshuffle],
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(90).expect("block"),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(90).expect("block"),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnStructural {
                trivial_affine_discards: Vec::new(),
                edge: EdgeId::new(90).expect("edge"),
                source: input_root,
                returned_claims: vec![claim],
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(90).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };
    (
        TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types: vec![StructuralTypeDeclaration {
                id: structural_type,
                identity: "Region".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            }],
            structural_domains: vec![StructuralDomainDeclaration {
                id: semantic_vocabulary::StructuralDomainId::new(90).expect("structural domain"),
                semantic_domain: semantic_vocabulary::DomainSemanticId::new(90)
                    .expect("semantic domain"),
                identity: "Region::Content".to_owned(),
                carrier: structural_type,
                content_projection: Some(StructuralContentProjection {
                    identity: projection,
                    algebra,
                    expression,
                }),
            }],
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
        },
        goal,
        obligation,
    )
}

pub(super) fn structural_call_module() -> TerminalModule {
    let structural_type = StructuralTypeId::new(1).unwrap();
    let caller = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let caller_input = PlaceId::new(1).unwrap();
    let caller_result = PlaceId::new(2).unwrap();
    let callee_input = PlaceId::new(3).unwrap();
    let callee_result = PlaceId::new(4).unwrap();
    let call_result = PlaceId::new(5).unwrap();
    let claim = ClaimId::new(1).unwrap();
    let call = OperationId::new(1).unwrap();
    let caller_result_declaration = StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: caller_result,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let callee_result_declaration = StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: callee_result,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let parameter = |place, position| StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let caller_machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: caller,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![parameter(caller_input, 0)],
        ranked_scc: None,
        result: TerminalMachineResult::Structural(caller_result_declaration),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: caller_input,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: caller_result,
                kind: StructuralPlaceKind::Result,
            },
            StructuralPlaceDeclaration {
                id: call_result,
                kind: StructuralPlaceKind::OperationResult {
                    producer: call,
                    structural_type,
                },
            },
        ],
        entry_claims: vec![EntryClaim {
            claim,
            input: caller_input,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: call,
                result: OperationResult::Structural(StructuralOperationResult {
                    place: call_result,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Linear,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: vec![StructuralResultClaimBinding {
                        claim,
                        path: Vec::new(),
                    }],
                }),
                kind: OperationKind::CallStructural {
                    callee,
                    structural_arguments: vec![StructuralArgument {
                        place: caller_input,
                        path: Vec::new(),
                        access: StructuralAccess::Owned,
                    }],
                    claim_transfers: vec![ClaimTransfer {
                        claim,
                        argument_index: 0,
                    }],
                    returned_claim_transfers: vec![StructuralResultClaimTransfer {
                        callee_claim: claim,
                        caller_claim: claim,
                    }],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    selected_evidence: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnStructural {
                edge: EdgeId::new(1).unwrap(),
                source: call_result,
                returned_claims: vec![claim],
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(1).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let callee_machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: callee,
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: vec![parameter(callee_input, 0)],
        ranked_scc: None,
        result: TerminalMachineResult::Structural(callee_result_declaration),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: callee_input,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            },
            StructuralPlaceDeclaration {
                id: callee_result,
                kind: StructuralPlaceKind::Result,
            },
        ],
        entry_claims: vec![EntryClaim {
            claim,
            input: callee_input,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(2).unwrap(),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(2).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnStructural {
                edge: EdgeId::new(2).unwrap(),
                source: callee_input,
                returned_claims: vec![claim],
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(2).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: caller,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "Receipt".to_owned(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
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
        machines: vec![caller_machine, callee_machine],
    }
}

pub(super) fn multi_claim_structural_call_module() -> TerminalModule {
    let mut module = structural_call_module();
    let element_type = StructuralTypeId::new(2).unwrap();
    module.structural_types[0].shape = StructuralTypeShape::FixedArray {
        element: element_type,
        length: 2,
    };
    module.structural_types.push(StructuralTypeDeclaration {
        id: element_type,
        identity: "ReceiptElement".to_owned(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });

    let paths = [
        vec![terminal_psi::StructuralPathSegment::FixedIndex(0)],
        vec![terminal_psi::StructuralPathSegment::FixedIndex(1)],
    ];
    for machine in &mut module.machines {
        machine.entry_claims = paths
            .iter()
            .enumerate()
            .map(|(index, path)| EntryClaim {
                claim: ClaimId::new(index as u64 + 1).unwrap(),
                input: machine.structural_parameters[0].place,
                path: path.clone(),
            })
            .collect();
        let Terminator::ReturnStructural {
            returned_claims, ..
        } = &mut machine.blocks[0].terminator
        else {
            unreachable!()
        };
        *returned_claims = vec![ClaimId::new(1).unwrap(), ClaimId::new(2).unwrap()];
    }

    let operation = &mut module.machines[0].blocks[0].operations[0];
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!()
    };
    result.claims = paths
        .iter()
        .enumerate()
        .map(|(index, path)| StructuralResultClaimBinding {
            claim: ClaimId::new(index as u64 + 1).unwrap(),
            path: path.clone(),
        })
        .collect();
    let OperationKind::CallStructural {
        claim_transfers,
        returned_claim_transfers,
        ..
    } = &mut operation.kind
    else {
        unreachable!()
    };
    *claim_transfers = vec![
        ClaimTransfer {
            claim: ClaimId::new(1).unwrap(),
            argument_index: 0,
        },
        ClaimTransfer {
            claim: ClaimId::new(2).unwrap(),
            argument_index: 0,
        },
    ];
    *returned_claim_transfers = vec![
        StructuralResultClaimTransfer {
            callee_claim: ClaimId::new(1).unwrap(),
            caller_claim: ClaimId::new(1).unwrap(),
        },
        StructuralResultClaimTransfer {
            callee_claim: ClaimId::new(2).unwrap(),
            caller_claim: ClaimId::new(2).unwrap(),
        },
    ];
    module
}

pub(super) fn partition_composition_module() -> (TerminalModule, Proposition, ObligationId) {
    let input_root = PlaceId::new(90).expect("input place");
    let structural_type = StructuralTypeId::new(90).expect("structural type");
    let claim = ClaimId::new(1).expect("claim");
    let parameter = ValueId::new(90).expect("scalar parameter");
    let result = ValueId::new(91).expect("scalar result");
    let operation = OperationId::new(90).expect("producer operation");
    let boundary_id = BoundaryMachineId::new(90).expect("boundary");
    let obligation = ObligationId::new(90).expect("obligation");
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Byte".to_owned(),
    };
    let expression = ContentProjectionExpression::CountedQuantity(
        ContentProjectionScalar::Natural("1".to_owned()),
    );
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(90).expect("content domain"),
        projection_report_fingerprint:
            language_semantics::content::terminal_projection_report_fingerprint(
                &algebra,
                &expression,
            ),
    };
    let source_input_root = PlaceId::new(190).expect("source input");
    let place = |version, root, segments| ContentStructuralPlace {
        version,
        root,
        segments,
    };
    let term = |subject| ContentTerm::Projection {
        projection,
        subject,
    };
    let source_input = place(ContentPlaceVersion::Entry, source_input_root, Vec::new());
    let source_left = place(
        ContentPlaceVersion::Current,
        source_input_root,
        vec![ContentPlaceSegment::Field("left".to_owned())],
    );
    let source_right = place(
        ContentPlaceVersion::Current,
        source_input_root,
        vec![ContentPlaceSegment::Field("right".to_owned())],
    );
    let target_input = place(ContentPlaceVersion::Entry, input_root, Vec::new());
    let target_left = place(
        ContentPlaceVersion::Current,
        input_root,
        vec![ContentPlaceSegment::Field("left".to_owned())],
    );
    let target_right = place(
        ContentPlaceVersion::Current,
        input_root,
        vec![ContentPlaceSegment::Field("right".to_owned())],
    );
    let source = ContentConservation::new(
        algebra.clone(),
        term(source_input.clone()),
        ContentTerm::separate([term(source_left.clone()), term(source_right.clone())])
            .expect("separated source"),
    );
    let derived = ContentConservation::new(
        algebra.clone(),
        term(target_input.clone()),
        ContentTerm::separate([term(target_left.clone()), term(target_right.clone())])
            .expect("separated target"),
    );
    let mut substitutions = vec![
        ContentPlaceSubstitution {
            source: source_input,
            target: target_input.clone(),
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
    let source_structural_places = vec![StructuralPlaceDeclaration {
        id: source_input_root,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    let source_place_kinds = source_structural_places
        .iter()
        .map(|place| (place.id, place.kind))
        .collect();
    let source_report_fingerprint =
        content_conservation_report_fingerprint(&source, &source_place_kinds)
            .expect("the fixture source theorem has a checked fingerprint preimage");
    let composition = ContentPartitionComposition {
        producer_operation: operation,
        source_report_fingerprint,
        source_structural_places: source_structural_places.clone(),
        source: source.clone(),
        input_claims: vec![claim],
        substitutions,
        derived: derived.clone(),
    };
    let goal = Proposition::ContentConservation(derived);
    let structural_parameter = StructuralParameterDeclaration {
        place: input_root,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(90).expect("machine"),
        attachment: None,
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
            id: parameter,
            scalar_type: ScalarType::Boolean,
        }],
        structural_parameters: vec![structural_parameter.clone()],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type: ScalarType::Boolean,
        }),
        structural_places: vec![StructuralPlaceDeclaration {
            id: input_root,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        entry_claims: vec![EntryClaim {
            claim,
            input: input_root,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        content_entry_claims: vec![ContentEntryClaim {
            claim,
            input: target_input,
            projections: vec![ClaimContentProjection {
                projection,
                algebra: algebra.clone(),
            }],
        }],
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: vec![composition],
        entry: BlockId::new(90).expect("block"),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(90).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: operation,
                result: OperationResult::Unit,
                kind: OperationKind::BoundaryCall {
                    boundary: boundary_id,
                    arguments: Vec::new(),
                    structural_arguments: vec![StructuralArgument {
                        place: input_root,
                        access: StructuralAccess::Owned,
                        path: Vec::new(),
                    }],
                    completion_receipts: vec![CompletionReceipt {
                        claim,
                        argument_index: 0,
                    }],
                },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(90).expect("edge"),
                value: parameter,
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(90).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };
    let boundary = BoundaryMachineDeclaration {
        fixed_service_reach: Vec::new(),
        crash_routes: Vec::new(),
        id: boundary_id,
        identity: "Splitter::partition".to_owned(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![structural_parameter],
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: vec![terminal_psi::BoundaryContentGuarantee::Conservation(
            ContentConservationGuarantee {
                report_fingerprint: source_report_fingerprint,
                structural_places: source_structural_places,
                conservation: source,
            },
        )],
        published_service_ceiling: Vec::new(),
    };
    (
        TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types: vec![StructuralTypeDeclaration {
                id: structural_type,
                identity: "Region".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            }],
            structural_domains: vec![StructuralDomainDeclaration {
                id: semantic_vocabulary::StructuralDomainId::new(90).expect("structural domain"),
                semantic_domain: semantic_vocabulary::DomainSemanticId::new(90)
                    .expect("semantic domain"),
                identity: "Region::Content".to_owned(),
                carrier: structural_type,
                content_projection: Some(StructuralContentProjection {
                    identity: projection,
                    algebra,
                    expression,
                }),
            }],
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: vec![boundary],
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
        },
        goal,
        obligation,
    )
}

pub(super) fn reflexive_content_module() -> (TerminalModule, Proposition, ObligationId) {
    let parameter = ValueId::new(80).expect("parameter");
    let result = ValueId::new(81).expect("result");
    let place = PlaceId::new(80).expect("place");
    let subject = ContentTerm::Projection {
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(80).expect("content domain"),
            projection_report_fingerprint: 0x8055,
        },
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: place,
            segments: Vec::new(),
        },
    };
    let goal = Proposition::ContentConservation(ContentConservation::new(
        ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "Byte".to_owned(),
        },
        subject.clone(),
        subject,
    ));
    let obligation = ObligationId::new(80).expect("obligation");
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(80).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![ValueDeclaration {
            qualifications: Default::default(),
            id: parameter,
            scalar_type: ScalarType::Boolean,
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type: ScalarType::Boolean,
        }),
        structural_places: vec![StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(80).expect("block"),
        blocks: vec![Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: BlockId::new(80).expect("block"),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(80).expect("edge"),
                value: parameter,
            },
        }],
        contract: MachineContract {
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(80).expect("contract"),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: vec![ContractClause {
                obligation,
                proposition: goal.clone(),
            }],
            outcome_specific_ensures: Vec::new(),
        },
    };
    (
        TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types: Vec::new(),
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
        },
        goal,
        obligation,
    )
}
