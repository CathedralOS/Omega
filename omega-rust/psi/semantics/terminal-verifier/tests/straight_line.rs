//! Fixtures shared by the straight-line verification tests: placed view
//! inputs and the reshuffle, content, wrapping, saturating and guard modules.

#[path = "straight_line/affine_local_frontier.rs"]
mod affine_local_frontier;
#[path = "straight_line/arithmetic_obligations.rs"]
mod arithmetic_obligations;
#[path = "straight_line/case_access.rs"]
mod case_access;
#[path = "straight_line/case_payload.rs"]
mod case_payload;
#[path = "straight_line/contract_fields.rs"]
mod contract_fields;
#[path = "straight_line/false_edge_custody.rs"]
mod false_edge_custody;
#[path = "straight_line/guards_and_partitions.rs"]
mod guards_and_partitions;
#[path = "straight_line/integer_axioms_and_carriers.rs"]
mod integer_axioms_and_carriers;
#[path = "straight_line/literal_byte_extent.rs"]
mod literal_byte_extent;
#[path = "straight_line/scalar_record.rs"]
mod record;
#[path = "straight_line/scalar_qualifications.rs"]
mod scalar_qualifications;
#[path = "straight_line/structural_byte_sequence_store.rs"]
mod structural_byte_sequence_store;
#[path = "straight_line/unit_returns_and_certificates.rs"]
mod unit_returns_and_certificates;
#[path = "straight_line/wrapping_and_saturating_axioms.rs"]
mod wrapping_and_saturating_axioms;

use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceError, EvidenceRoute, PrimitiveJudgment,
    ProofNode, ProofRule, ProofSystemMarker, RecursiveComponentCertificate,
    RecursiveEdgeCertificate,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation,
    ContentDomainId, ContentPlaceSegment, ContentPlaceVersion, ContentProjectionExpression,
    ContentProjectionIdentity, ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralCaseId,
    StructuralCaseSubject, StructuralPlaceKind, StructuralTypeId, ValueId,
    content_conservation_report_fingerprint,
};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimTransfer, CompletionReceipt,
    ContentConservationGuarantee, ContentEntryClaim, ContentIdentityReshuffle,
    ContentPartitionComposition, ContentPlaceSubstitution, ContractClause, CrashCause, EntryClaim,
    MachineContract, Operation, OperationKind, OperationResult, OutcomeSpecificEnsure,
    OutcomeSpecificGuard, StructuralAccess, StructuralArgument, StructuralCaseDeclaration,
    StructuralContentProjection, StructuralDomainDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralResultClaimBinding,
    StructuralResultClaimTransfer, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, SuccessorEdge, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalPlacedViewInput, TerminalProofRankingRelation, TerminalProofRecursiveCallSite,
    TerminalProofRecursiveComponent, TerminalProofRecursiveEdge, TerminalProofRecursiveField,
    TerminalProofRecursiveMember, TerminalProofRecursiveType, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, RecursiveComponentEvidence, VerificationError,
    proof_recursive_component_identity, reconstruct_operation_obligations,
    reconstruct_proof_recursive_component_obligations, reconstruct_terminal_obligations,
    validate_module, verify_module,
};

fn placed_view_input(machine: MachineId, position: u32) -> TerminalPlacedViewInput {
    placed_view_input_at_state(machine, "inspect::entry", position)
}

fn placed_view_input_at_state(
    machine: MachineId,
    state: &str,
    position: u32,
) -> TerminalPlacedViewInput {
    let identity = |name: &str| format!("package:{}::{name}", "01".repeat(32));
    let policy_identity = identity("Uart");
    let schema_identity = identity("Registers");
    TerminalPlacedViewInput {
        machine,
        position,
        source_machine_identity: identity("inspect"),
        source_state_identity: identity(state),
        source_parameter_identity: identity(&format!("{state}::view{position}")),
        access: StructuralAccess::MutableBorrow,
        binding_is_const: false,
        binding_is_mutable: true,
        view_identity: terminal_psi::canonical_placed_view_identity(
            &policy_identity,
            &schema_identity,
        ),
        policy_identity,
        policy_plan_machine_identity: identity("Uart::plan"),
        schema_identity,
        placement_report_fingerprint: 41,
        placement_commitment: [0x5a; 32],
    }
}

fn identity_reshuffle_module() -> (TerminalModule, Proposition, ObligationId) {
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

fn structural_call_module() -> TerminalModule {
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

fn multi_claim_structural_call_module() -> TerminalModule {
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

fn partition_composition_module() -> (TerminalModule, Proposition, ObligationId) {
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

fn reflexive_content_module() -> (TerminalModule, Proposition, ObligationId) {
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

fn wrapping_add_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(20).expect("left parameter");
    let right = ValueId::new(21).expect("right parameter");
    let sum = ValueId::new(22).expect("sum result");
    let result = ValueId::new(23).expect("machine result");
    let obligation = ObligationId::new(20).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_add(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(20).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(20).expect("block"),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(20).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(20).expect("add operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: sum,
                    scalar_type,
                }),
                kind: OperationKind::WrappingIntegerAdd { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(20).expect("return edge"),
                value: sum,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(20).expect("contract"),
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

fn saturating_add_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(30).expect("left parameter");
    let right = ValueId::new(31).expect("right parameter");
    let sum = ValueId::new(32).expect("sum result");
    let result = ValueId::new(33).expect("machine result");
    let obligation = ObligationId::new(30).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_add(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(30).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(30).expect("block"),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(30).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(30).expect("add operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: sum,
                    scalar_type,
                }),
                kind: OperationKind::SaturatingIntegerAdd { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(30).expect("return edge"),
                value: sum,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(30).expect("contract"),
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

fn wrapping_subtract_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(40).expect("left parameter");
    let right = ValueId::new(41).expect("right parameter");
    let difference = ValueId::new(42).expect("difference result");
    let result = ValueId::new(43).expect("machine result");
    let obligation = ObligationId::new(40).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_subtract(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(40).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(40).expect("block"),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(40).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(40).expect("subtract operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: difference,
                    scalar_type,
                }),
                kind: OperationKind::WrappingIntegerSubtract { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(40).expect("return edge"),
                value: difference,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(40).expect("contract"),
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

fn saturating_subtract_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(50).expect("left parameter");
    let right = ValueId::new(51).expect("right parameter");
    let difference = ValueId::new(52).expect("difference result");
    let result = ValueId::new(53).expect("machine result");
    let obligation = ObligationId::new(50).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_subtract(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(50).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(50).expect("block"),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(50).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(50).expect("subtract operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: difference,
                    scalar_type,
                }),
                kind: OperationKind::SaturatingIntegerSubtract { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(50).expect("return edge"),
                value: difference,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(50).expect("contract"),
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

fn wrapping_multiply_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(60).expect("left parameter");
    let right = ValueId::new(61).expect("right parameter");
    let product = ValueId::new(62).expect("product result");
    let result = ValueId::new(63).expect("machine result");
    let obligation = ObligationId::new(60).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::wrapping_integer_multiply(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(60).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(60).expect("block"),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(60).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(60).expect("multiply operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: product,
                    scalar_type,
                }),
                kind: OperationKind::WrappingIntegerMultiply { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(60).expect("return edge"),
                value: product,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(60).expect("contract"),
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

fn saturating_multiply_module() -> (TerminalModule, Proposition, ObligationId) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(70).expect("left parameter");
    let right = ValueId::new(71).expect("right parameter");
    let product = ValueId::new(72).expect("product result");
    let result = ValueId::new(73).expect("machine result");
    let obligation = ObligationId::new(70).expect("obligation");
    let term = |id| ScalarTerm::value(id, scalar_type);
    let goal = Proposition::Equal(
        term(result),
        ScalarTerm::saturating_integer_multiply(integer, term(left), term(right)).unwrap(),
    );
    let machine = TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: MachineId::new(70).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                qualifications: Default::default(),
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                qualifications: Default::default(),
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(70).expect("block"),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(70).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: OperationId::new(70).expect("multiply operation"),
                result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: product,
                    scalar_type,
                }),
                kind: OperationKind::SaturatingIntegerMultiply { left, right },
            }],
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: EdgeId::new(70).expect("return edge"),
                value: product,
            },
        }],
        contract: MachineContract {
            id: ContractId::new(70).expect("contract"),
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

fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).expect("nonzero contract identity")
}

fn proof_recursive_module() -> TerminalModule {
    let mut module = unit_module();
    module.proof_recursive_components = vec![TerminalProofRecursiveComponent {
        ranking_relation: TerminalProofRankingRelation::StructuralSubterm,
        rank_type_identity: "package::Node".into(),
        types: vec![TerminalProofRecursiveType {
            identity: "package::Node".into(),
            fields: vec![
                TerminalProofRecursiveField {
                    identity: "package::Node::left".into(),
                    type_identity: "package::Node".into(),
                },
                TerminalProofRecursiveField {
                    identity: "package::Node::right".into(),
                    type_identity: "package::Node".into(),
                },
            ],
        }],
        members: vec![
            TerminalProofRecursiveMember {
                contract: contract_id(1001),
                machine_identity: "package::left".into(),
                rank_parameter_identity: "package::left::node".into(),
            },
            TerminalProofRecursiveMember {
                contract: contract_id(1002),
                machine_identity: "package::right".into(),
                rank_parameter_identity: "package::right::node".into(),
            },
        ],
        edges: vec![
            TerminalProofRecursiveEdge {
                caller: contract_id(1001),
                callee: contract_id(1002),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::left::step".into(),
                    statement_index: 0,
                },
                strict_member_path: vec!["package::Node::left".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract_id(1001),
                callee: contract_id(1002),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::left::step".into(),
                    statement_index: 1,
                },
                strict_member_path: vec!["package::Node::right".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract_id(1002),
                callee: contract_id(1001),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::right::step".into(),
                    statement_index: 0,
                },
                strict_member_path: vec!["package::Node::left".into()],
            },
            TerminalProofRecursiveEdge {
                caller: contract_id(1002),
                callee: contract_id(1001),
                site: TerminalProofRecursiveCallSite::Statement {
                    state_identity: "package::right::step".into(),
                    statement_index: 1,
                },
                strict_member_path: vec!["package::Node::right".into()],
            },
        ],
    }];
    module
}

fn proof_recursive_bundle(module: &TerminalModule) -> ProofBundle {
    let [component] = module.proof_recursive_components.as_slice() else {
        panic!("one recursive component")
    };
    let mut reconstructed =
        reconstruct_proof_recursive_component_obligations(module).expect("canonical component");
    let obligation = reconstructed.pop().expect("one recursive obligation");
    let route = |identity, obligation: &proof_admission::CertificateObligation| {
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(identity).expect("nonzero certificate identity"),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode {
                conclusion: obligation.obligation.proposition.clone(),
                rule: ProofRule::SemanticAxiom { index: 0 },
            },
        })
    };
    ProofBundle {
        evidence: Vec::new(),
        control_cycles: Vec::new(),
        recursive_components: vec![RecursiveComponentEvidence {
            component: proof_recursive_component_identity(component),
            certificate: RecursiveComponentCertificate {
                identity: EvidenceIdentity::new(2000).unwrap(),
                ranking_relation: obligation.ranking_relation.expect("measured component"),
                well_foundedness: route(2001, &obligation.well_foundedness),
                edges: obligation
                    .edges
                    .iter()
                    .enumerate()
                    .map(|(index, edge)| RecursiveEdgeCertificate {
                        obligation: edge.decrease.obligation.id,
                        evidence: route(
                            2010 + u64::try_from(index).expect("edge index fits u64"),
                            &edge.decrease,
                        ),
                    })
                    .collect(),
            },
        }],
        evidence_producers: Vec::new(),
    }
}

fn unit_module() -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(900).unwrap(),
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
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(900).unwrap(),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(900).unwrap(),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: BlockId::new(900).unwrap(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(900).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(900).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn payloadless_guard_module() -> (TerminalModule, StructuralCaseId, StructuralCaseId, PlaceId) {
    let mut module = unit_module();
    let operation = OperationId::new(920).expect("operation");
    let operation_place = PlaceId::new(920).expect("operation place");
    let result_place = PlaceId::new(921).expect("result place");
    let structural_type = StructuralTypeId::new(920).expect("result type");
    let success = StructuralCaseId::new(920).expect("success case");
    let failure = StructuralCaseId::new(921).expect("failure case");
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "Outcome".to_owned(),
        shape: StructuralTypeShape::Sum {
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
        },
    }];
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: operation_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: StructuralPlaceKind::Result,
        },
    ];
    machine.blocks[0].operations = vec![Operation {
        static_reach_binding: None,
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            place: operation_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarCase {
            result_case: success,
            fields: vec![],
        },
    }];
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(920).expect("return edge"),
        source: operation_place,
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    (module, success, failure, result_place)
}

fn multi_exit_payloadless_guard_module() -> (
    TerminalModule,
    StructuralCaseId,
    StructuralCaseId,
    StructuralCaseId,
    PlaceId,
) {
    let mut module = unit_module();
    let first = ValueId::new(930).expect("first condition");
    let second = ValueId::new(931).expect("second condition");
    let structural_type = StructuralTypeId::new(930).expect("result type");
    let success = StructuralCaseId::new(930).expect("success case");
    let failure = StructuralCaseId::new(931).expect("failure case");
    let absent = StructuralCaseId::new(932).expect("absent case");
    let success_one_place = PlaceId::new(930).expect("first Success place");
    let success_two_place = PlaceId::new(931).expect("second Success place");
    let failure_place = PlaceId::new(932).expect("Failure place");
    let result_place = PlaceId::new(933).expect("result place");
    let success_one_operation = OperationId::new(930).expect("first Success operation");
    let success_two_operation = OperationId::new(931).expect("second Success operation");
    let failure_operation = OperationId::new(932).expect("Failure operation");
    module.structural_types = vec![StructuralTypeDeclaration {
        id: structural_type,
        identity: "Outcome".to_owned(),
        shape: StructuralTypeShape::Sum {
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
                StructuralCaseDeclaration {
                    id: absent,
                    identity: "Absent".to_owned(),
                    fields: Vec::new(),
                },
            ],
        },
    }];
    let machine = &mut module.machines[0];
    machine.parameters = vec![
        ValueDeclaration {
            qualifications: Default::default(),
            id: first,
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: second,
            scalar_type: ScalarType::Boolean,
        },
    ];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    machine.structural_places = vec![
        StructuralPlaceDeclaration {
            id: success_one_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: success_one_operation,
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: success_two_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: success_two_operation,
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: failure_place,
            kind: StructuralPlaceKind::OperationResult {
                producer: failure_operation,
                structural_type,
            },
        },
        StructuralPlaceDeclaration {
            id: result_place,
            kind: StructuralPlaceKind::Result,
        },
    ];
    let return_block = |block_raw, operation, place, result_case, edge_raw| Block {
        structural_parameters: Vec::new(),
        id: BlockId::new(block_raw).unwrap(),
        parameters: Vec::new(),
        operations: vec![Operation {
            static_reach_binding: None,
            id: operation,
            result: OperationResult::Structural(StructuralOperationResult {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishScalarCase {
                result_case,
                fields: vec![],
            },
        }],
        terminator: Terminator::ReturnStructural {
            edge: EdgeId::new(edge_raw).unwrap(),
            source: place,
            returned_claims: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    machine.entry = BlockId::new(930).expect("entry block");
    machine.blocks = vec![
        Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(930).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: first,
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: EdgeId::new(930).unwrap(),
                    target: BlockId::new(931).unwrap(),
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: EdgeId::new(931).unwrap(),
                    target: BlockId::new(934).unwrap(),
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            structural_parameters: Vec::new(),
            id: BlockId::new(931).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: second,
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: EdgeId::new(932).unwrap(),
                    target: BlockId::new(932).unwrap(),
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: EdgeId::new(933).unwrap(),
                    target: BlockId::new(933).unwrap(),
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        return_block(932, success_one_operation, success_one_place, success, 934),
        return_block(933, success_two_operation, success_two_place, success, 935),
        return_block(934, failure_operation, failure_place, failure, 936),
    ];
    (module, success, failure, absent, result_place)
}

struct Fixture {
    module: TerminalModule,
    integer: IntegerType,
    constant: ValueId,
    forwarded: ValueId,
    result: ValueId,
    obligation: ObligationId,
}

impl Fixture {
    fn new() -> Self {
        let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let scalar_type = ScalarType::Integer(integer);
        let constant = ValueId::new(1).expect("constant value");
        let forwarded = ValueId::new(2).expect("forwarded value");
        let result = ValueId::new(3).expect("result value");
        let obligation = ObligationId::new(1).expect("ensures obligation");
        let seven = ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("seven");
        let goal = Proposition::Equal(ScalarTerm::value(result, scalar_type), seven);

        let machine = TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).expect("entry block"),
            blocks: vec![
                Block {
                    structural_parameters: Vec::new(),
                    id: BlockId::new(1).expect("entry block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: OperationId::new(1).expect("constant operation"),
                        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: constant,
                            scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7),
                        },
                    }],
                    terminator: Terminator::Jump {
                        structural_arguments: Vec::new(),
                        edge: EdgeId::new(1).expect("jump edge"),
                        target: BlockId::new(2).expect("exit block"),
                        arguments: vec![constant],
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: BlockId::new(2).expect("exit block"),
                    parameters: vec![ValueDeclaration {
                        qualifications: Default::default(),
                        id: forwarded,
                        scalar_type,
                    }],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(2).expect("return edge"),
                        value: forwarded,
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
                outcome_specific_ensures: Vec::new(),
            },
        };
        Self {
            module: TerminalModule {
                scalar_qualifications: Default::default(),
                scalar_block_invariants: Vec::new(),
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
            integer,
            constant,
            forwarded,
            result,
            obligation,
        }
    }

    fn proof_bundle(&self) -> ProofBundle {
        let scalar_type = ScalarType::Integer(self.integer);
        let term = |id| ScalarTerm::value(id, scalar_type);
        let seven = ScalarTerm::integer(self.integer, IntegerValue::Signed(7)).expect("seven");
        let constant_fact = Proposition::Equal(term(self.constant), seven.clone());
        let forwarding_fact = Proposition::Equal(term(self.forwarded), term(self.constant));
        let return_fact = Proposition::Equal(term(self.result), term(self.forwarded));
        let forwarded_is_seven = Proposition::Equal(term(self.forwarded), seven.clone());
        let goal = Proposition::Equal(term(self.result), seven);
        let proof = ProofNode {
            conclusion: goal,
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(ProofNode {
                    conclusion: return_fact,
                    rule: ProofRule::SemanticAxiom { index: 2 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: forwarded_is_seven,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: forwarding_fact,
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: constant_fact,
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                }),
            },
        };
        ProofBundle {
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation: self.obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(1).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof,
                }),
            }],
        }
    }
}
