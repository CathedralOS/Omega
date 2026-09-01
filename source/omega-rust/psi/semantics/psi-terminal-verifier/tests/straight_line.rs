use psi_core::{
    BlockId, BoundaryMachineId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation,
    ContentDomainId, ContentPlaceSegment, ContentPlaceVersion, ContentProjectionExpression,
    ContentProjectionIdentity, ContentProjectionScalar, ContentStructuralPlace, ContentTerm,
    ContractId, EdgeId, EvidenceIdentity, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue,
    MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionError, ScalarTerm,
    ScalarType, StructuralCaseId, StructuralCaseSubject, StructuralPlaceKind, StructuralTypeId,
    ValueId, content_conservation_report_fingerprint,
};
use psi_proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceError, EvidenceRoute, PrimitiveJudgment,
    ProofError, ProofNode, ProofRule, ProofSystemMarker, RecursiveComponentCertificate,
    RecursiveEdgeCertificate,
};
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ClaimContentProjection, ClaimTransfer, CompletionReceipt,
    ContentConservationGuarantee, ContentEntryClaim, ContentIdentityReshuffle,
    ContentPartitionComposition, ContentPlaceSubstitution, ContractClause, CrashCause, EntryClaim,
    EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
    EvidenceTermDeclaration, MachineContract, Operation, OperationKind, OperationResult,
    OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificGuard,
    PropositionApplicationIdentity, PropositionDeclaration, PropositionEvidence, StructuralAccess,
    StructuralArgument, StructuralCaseDeclaration, StructuralContentProjection,
    StructuralDomainDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralResultClaimBinding, StructuralResultClaimTransfer,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalMachine, TerminalMachineResult, TerminalModule, TerminalPlacedViewInput,
    TerminalProofRankingRelation, TerminalProofRecursiveCallSite, TerminalProofRecursiveComponent,
    TerminalProofRecursiveEdge, TerminalProofRecursiveField, TerminalProofRecursiveMember,
    TerminalProofRecursiveType, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_verifier::{
    ContractClauseKind, EvidenceProducerProvenance, ModuleError, ObligationEvidence, ProofBundle,
    RecursiveComponentEvidence, VerificationError, proof_recursive_component_identity,
    reconstruct_operation_obligations, reconstruct_proof_recursive_component_obligations,
    reconstruct_terminal_obligations, validate_module, verify_module,
};

#[test]
fn unit_machine_is_a_value_less_normal_return() {
    let module = unit_module();
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("a unit return is a valid proof-free normal exit");

    assert_eq!(verified.module(), &module);
    assert!(verified.accepted_facts().is_empty());
}

#[test]
fn exact_proof_recursive_component_is_verified_as_one_grouped_certificate() {
    let module = proof_recursive_module();
    let bundle = proof_recursive_bundle(&module);
    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("the exact grouped recursive certificate should verify");

    assert!(verified.accepted_facts().is_empty());
    let [acceptance] = verified.accepted_recursive_components() else {
        panic!("one recursive component acceptance")
    };
    assert_eq!(
        acceptance.members,
        vec![contract_id(1001), contract_id(1002)]
    );
    assert_eq!(acceptance.decreases.len(), 4);
}

#[test]
fn proof_recursive_component_rejects_stale_missing_extra_and_reordered_evidence() {
    let module = proof_recursive_module();
    let bundle = proof_recursive_bundle(&module);

    let mut stale_module = module.clone();
    stale_module.proof_recursive_components[0].edges[0].strict_member_path =
        vec!["package::Node::right".into()];
    assert!(matches!(
        verify_module(&stale_module, &bundle, &AdmissionProfile::default()),
        Err(VerificationError::MissingRecursiveComponentEvidence(_))
    ));

    let mut missing_component = bundle.clone();
    missing_component.recursive_components.clear();
    assert!(matches!(
        verify_module(&module, &missing_component, &AdmissionProfile::default()),
        Err(VerificationError::MissingRecursiveComponentEvidence(_))
    ));

    let mut missing_edge = bundle.clone();
    missing_edge.recursive_components[0].certificate.edges.pop();
    assert!(matches!(
        verify_module(&module, &missing_edge, &AdmissionProfile::default()),
        Err(VerificationError::RejectedRecursiveComponent { .. })
    ));

    let mut reordered_edges = bundle.clone();
    reordered_edges.recursive_components[0]
        .certificate
        .edges
        .reverse();
    assert_eq!(
        verify_module(&module, &reordered_edges, &AdmissionProfile::default()).unwrap_err(),
        VerificationError::RejectedRecursiveComponent {
            component: proof_recursive_component_identity(&module.proof_recursive_components[0]),
            error: psi_proof_admission::RecursiveComponentError::NonCanonicalCertificateEdges,
        }
    );

    let mut extra = bundle;
    let mut extra_row = extra.recursive_components[0].clone();
    extra_row.component = psi_core::RecursiveComponentId::new(u64::MAX).unwrap();
    extra.recursive_components.push(extra_row);
    assert!(matches!(
        verify_module(&module, &extra, &AdmissionProfile::default()),
        Err(VerificationError::UnknownRecursiveComponentEvidence(_))
    ));
}

#[test]
fn proof_recursive_component_representation_rejects_noncanonical_or_unresolved_graphs() {
    let module = proof_recursive_module();

    let mut empty_path = module.clone();
    empty_path.proof_recursive_components[0].edges[0]
        .strict_member_path
        .clear();
    assert!(matches!(
        validate_module(&empty_path),
        Err(ModuleError::InvalidProofRecursiveEdge { .. })
    ));

    let mut unknown_field = module.clone();
    unknown_field.proof_recursive_components[0].edges[0].strict_member_path =
        vec!["package::Node::missing".into()];
    assert!(matches!(
        validate_module(&unknown_field),
        Err(ModuleError::InvalidProofRecursiveEdge { .. })
    ));

    let mut noncanonical_edges = module.clone();
    noncanonical_edges.proof_recursive_components[0]
        .edges
        .reverse();
    assert_eq!(
        validate_module(&noncanonical_edges).unwrap_err(),
        ModuleError::InvalidProofRecursiveComponent
    );

    let mut duplicate_site = module.clone();
    duplicate_site.proof_recursive_components[0].edges[1].site =
        duplicate_site.proof_recursive_components[0].edges[0]
            .site
            .clone();
    assert!(matches!(
        validate_module(&duplicate_site),
        Err(ModuleError::InvalidProofRecursiveEdge { .. })
    ));

    let mut outside = module.clone();
    outside.proof_recursive_components[0].edges[3].callee = contract_id(1999);
    assert!(matches!(
        validate_module(&outside),
        Err(ModuleError::InvalidProofRecursiveEdge { .. })
    ));
}

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
        view_identity: psi_terminal::canonical_placed_view_identity(
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

#[test]
fn placed_view_inputs_require_exact_canonical_custody() {
    let mut module = unit_module();
    let row = placed_view_input(module.entry, 0);
    module.placed_view_inputs.push(row.clone());
    validate_module(&module).expect("exact placed-view input should validate");

    let mut distinct_state_at_same_position = module.clone();
    distinct_state_at_same_position
        .placed_view_inputs
        .push(placed_view_input_at_state(
            distinct_state_at_same_position.entry,
            "inspect::next",
            0,
        ));
    validate_module(&distinct_state_at_same_position)
        .expect("distinct state inputs may reuse the same parameter position");

    let mut zero_commitment = module.clone();
    zero_commitment.placed_view_inputs[0].placement_commitment = [0; 32];
    assert!(matches!(
        validate_module(&zero_commitment),
        Err(ModuleError::InvalidPlacedViewInput { .. })
    ));

    let mut missing_machine = module.clone();
    missing_machine.placed_view_inputs[0].machine = MachineId::new(999).unwrap();
    assert!(matches!(
        validate_module(&missing_machine),
        Err(ModuleError::InvalidPlacedViewInput { .. })
    ));

    let mut nonhermetic = module.clone();
    nonhermetic.placed_view_inputs[0].schema_identity =
        "package:not-a-digest::Registers".to_owned();
    assert!(matches!(
        validate_module(&nonhermetic),
        Err(ModuleError::InvalidPlacedViewInput { .. })
    ));

    let mut redirected_view = module.clone();
    redirected_view.placed_view_inputs[0]
        .view_identity
        .push('x');
    assert!(matches!(
        validate_module(&redirected_view),
        Err(ModuleError::InvalidPlacedViewInput { .. })
    ));

    let mut malformed_state = module.clone();
    malformed_state.placed_view_inputs[0].source_state_identity = "inspect::entry".to_owned();
    assert!(matches!(
        validate_module(&malformed_state),
        Err(ModuleError::InvalidPlacedViewInput { .. })
    ));

    let mut duplicate = module.clone();
    duplicate.placed_view_inputs.push(row);
    assert!(matches!(
        validate_module(&duplicate),
        Err(ModuleError::DuplicatePlacedViewInput { .. })
    ));

    let mut reordered = module;
    reordered
        .placed_view_inputs
        .push(placed_view_input(reordered.entry, 1));
    reordered.placed_view_inputs.reverse();
    assert!(matches!(
        validate_module(&reordered),
        Err(ModuleError::NonCanonicalPlacedViewInputOrder)
    ));
}

#[test]
fn verifier_rejects_mismatched_unit_and_scalar_return_shapes() {
    let mut scalar_return = unit_module();
    scalar_return.machines[0].blocks[0].terminator = Terminator::Return {
        cleanup_actions: Vec::new(),
        edge: EdgeId::new(900).unwrap(),
        value: ValueId::new(900).unwrap(),
    };
    assert_eq!(
        validate_module(&scalar_return).unwrap_err(),
        ModuleError::ScalarReturnFromUnitMachine {
            machine: MachineId::new(900).unwrap(),
            block: BlockId::new(900).unwrap(),
        }
    );

    let mut unit_return = unit_module();
    unit_return.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: ValueId::new(900).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    assert_eq!(
        validate_module(&unit_return).unwrap_err(),
        ModuleError::UnitReturnFromScalarMachine {
            machine: MachineId::new(900).unwrap(),
            block: BlockId::new(900).unwrap(),
        }
    );
}

#[test]
fn verifier_rejects_a_scalar_operation_with_a_unit_result_without_panicking() {
    let mut module = unit_module();
    let operation = OperationId::new(901).unwrap();
    module.machines[0].blocks[0].operations.push(Operation {
        id: operation,
        result: OperationResult::Unit,
        kind: OperationKind::BooleanConstant { value: true },
    });

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::ScalarOperationHasUnitResult(operation)
    );
}

#[test]
fn payloadless_case_establishment_validates_exact_member_and_surface() {
    let mut module = unit_module();
    let operation = OperationId::new(902).unwrap();
    let place = PlaceId::new(902).unwrap();
    let result_place = PlaceId::new(903).unwrap();
    let structural_type = StructuralTypeId::new(902).unwrap();
    let payloadless = StructuralCaseId::new(902).unwrap();
    let payload_bearing = StructuralCaseId::new(903).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "PayloadlessResult".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: payloadless,
                    identity: "Done".into(),
                    fields: Vec::new(),
                },
                StructuralCaseDeclaration {
                    id: payload_bearing,
                    identity: "Value".into(),
                    fields: vec![StructuralFieldDeclaration {
                        id: psi_core::StructuralFieldId::new(902).unwrap(),
                        identity: "value".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                    }],
                },
            ],
        },
    });
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type,
        },
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: result_place,
        kind: StructuralPlaceKind::Result,
    });
    machine.blocks[0].operations.push(Operation {
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishPayloadlessCase {
            result_case: payloadless,
        },
    });
    machine.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(902).unwrap(),
        source: place,
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let machine_id = machine.id;
    validate_module(&module).expect("exact payloadless member validates");

    let mut unknown = module.clone();
    let OperationKind::EstablishPayloadlessCase { result_case } =
        &mut unknown.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *result_case = StructuralCaseId::new(904).unwrap();
    assert!(matches!(
        validate_module(&unknown),
        Err(ModuleError::PayloadlessCaseRequiresPayloadlessMember { .. })
    ));

    let mut bearing = module.clone();
    let OperationKind::EstablishPayloadlessCase { result_case } =
        &mut bearing.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *result_case = payload_bearing;
    assert!(matches!(
        validate_module(&bearing),
        Err(ModuleError::PayloadlessCaseRequiresPayloadlessMember { .. })
    ));

    let mut non_sum = module.clone();
    non_sum.structural_types.last_mut().unwrap().shape =
        StructuralTypeShape::Record { fields: Vec::new() };
    assert!(matches!(
        validate_module(&non_sum),
        Err(ModuleError::PayloadlessCaseRequiresSum { .. })
    ));

    let mut generic_unrestricted = module.clone();
    generic_unrestricted.machines[0].blocks[0].operations[0].kind =
        OperationKind::BooleanConstant { value: true };
    assert_eq!(
        validate_module(&generic_unrestricted).unwrap_err(),
        ModuleError::StructuralResultMustBeOwned(machine_id)
    );

    let mut qualified = module;
    let OperationResult::Structural(result) =
        &mut qualified.machines[0].blocks[0].operations[0].result
    else {
        unreachable!()
    };
    result
        .qualifications
        .push(psi_core::StructuralDomainId::new(902).unwrap());
    assert_eq!(
        validate_module(&qualified).unwrap_err(),
        ModuleError::StructuralResultMustBeOwned(machine_id)
    );
}

#[test]
fn unit_machine_cannot_declare_a_result_structural_place() {
    let mut module = unit_module();
    module.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: PlaceId::new(900).unwrap(),
            kind: StructuralPlaceKind::Result,
        });

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::UnitMachineHasResultStructuralPlace {
            machine: MachineId::new(900).unwrap(),
            place: PlaceId::new(900).unwrap(),
        }
    );
}

#[test]
fn straight_line_integer_contract_is_reconstructed_and_verified() {
    let fixture = Fixture::new();
    let verified = verify_module(
        &fixture.module,
        &fixture.proof_bundle(),
        &AdmissionProfile::default(),
    )
    .expect("terminal module verifies independently of producer state");

    assert_eq!(verified.module(), &fixture.module);
    assert_eq!(verified.accepted_facts().len(), 1);
    assert_eq!(verified.accepted_facts()[0].obligation, fixture.obligation);
}

#[test]
fn verifier_reconstructs_every_contract_obligation() {
    let fixture = Fixture::new();
    assert_eq!(
        verify_module(
            &fixture.module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect_err("proof bundle cannot omit a bodyful guarantee"),
        VerificationError::MissingEvidence(fixture.obligation)
    );
}

#[test]
fn proof_route_changes_do_not_change_the_reconstructed_question() {
    let mut module = unit_module();
    let obligation = ObligationId::new(903).expect("obligation");
    module.machines[0].contract.ensures = vec![ContractClause {
        obligation,
        proposition: Proposition::Truth,
    }];
    let kernel_bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        }],
    };
    let certificate_bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(903).expect("evidence"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                },
            }),
        }],
    };
    let kernel =
        verify_module(&module, &kernel_bundle, &AdmissionProfile::default()).expect("kernel route");
    let certificate = verify_module(&module, &certificate_bundle, &AdmissionProfile::default())
        .expect("certificate route");
    assert_ne!(kernel.proof_bundle(), certificate.proof_bundle());
    assert_eq!(
        kernel.reconstructed_obligations(),
        certificate.reconstructed_obligations()
    );
}

#[test]
fn verifier_does_not_rediscover_an_alternate_route_for_a_malformed_certificate() {
    let mut module = unit_module();
    let obligation = ObligationId::new(904).expect("obligation");
    module.machines[0].contract.ensures = vec![ContractClause {
        obligation,
        proposition: Proposition::Truth,
    }];
    let malformed_selected_route = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(904).expect("evidence"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };

    assert_eq!(
        verify_module(
            &module,
            &malformed_selected_route,
            &AdmissionProfile::default(),
        )
        .expect_err("the verifier must not replace the malformed edge with primitive Truth"),
        VerificationError::RejectedEvidence {
            obligation,
            error: EvidenceError::Certificate(ProofError::UnknownAssumption(0)),
        }
    );
}

#[test]
fn contract_guarantees_are_canonical_by_obligation_identity() {
    let mut module = unit_module();
    module.machines[0].contract.ensures = vec![
        ContractClause {
            obligation: ObligationId::new(902).expect("later obligation"),
            proposition: Proposition::Truth,
        },
        ContractClause {
            obligation: ObligationId::new(901).expect("earlier obligation"),
            proposition: Proposition::Truth,
        },
    ];

    assert_eq!(
        validate_module(&module).expect_err("guarantee evidence order is canonical"),
        ModuleError::NonCanonicalContractEnsures(ContractId::new(900).expect("contract"))
    );
}

#[test]
fn boolean_constant_axiom_proves_the_return_contract() {
    let constant = ValueId::new(10).expect("constant");
    let result = ValueId::new(11).expect("result");
    let obligation = ObligationId::new(10).expect("obligation");
    let term = |id| ScalarTerm::value(id, ScalarType::Boolean);
    let goal = Proposition::Equal(term(result), ScalarTerm::boolean(true));
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(10).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(10).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(10).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(10).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(10).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: constant,
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanConstant { value: true },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(10).expect("edge"),
                    value: constant,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(10).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal.clone(),
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(result), term(constant)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(constant), ScalarTerm::boolean(true)),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(10).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("Boolean semantics should reconstruct both axioms");
}

#[test]
fn boolean_not_axiom_proves_the_return_contract() {
    let parameter = ValueId::new(20).expect("parameter");
    let negated = ValueId::new(21).expect("negated");
    let result = ValueId::new(22).expect("result");
    let obligation = ObligationId::new(20).expect("obligation");
    let term = |id| ScalarTerm::value(id, ScalarType::Boolean);
    let not_parameter = ScalarTerm::boolean_not(term(parameter)).unwrap();
    let goal = Proposition::Equal(term(result), not_parameter.clone());
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(20).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(20).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                id: parameter,
                scalar_type: ScalarType::Boolean,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(20).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(20).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(20).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: negated,
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanNot { operand: parameter },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(20).expect("edge"),
                    value: negated,
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
        }],
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(20).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: Proposition::Equal(term(result), term(negated)),
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: Proposition::Equal(term(negated), not_parameter),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                },
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("Boolean-not semantics should reconstruct operation and return axioms");

    let integer =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 operand type"));
    let mut wrong_operand = module.clone();
    wrong_operand.machines[0].contract.ensures.clear();
    wrong_operand.machines[0].parameters[0].scalar_type = integer;
    assert_eq!(
        validate_module(&wrong_operand).expect_err("Boolean not requires a Boolean operand"),
        ModuleError::BooleanNotOperandTypeMismatch {
            operation: OperationId::new(20).expect("operation"),
            operand: parameter,
            actual: integer,
        }
    );

    let mut wrong_result = module;
    wrong_result.machines[0].contract.ensures.clear();
    wrong_result.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = integer;
    wrong_result.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = integer;
    assert_eq!(
        validate_module(&wrong_result).expect_err("Boolean not requires a Boolean result"),
        ModuleError::BooleanNotRequiresBooleanResult(OperationId::new(20).expect("operation"))
    );
}

#[test]
fn boolean_equality_axiom_proves_the_return_contract() {
    let left = ValueId::new(30).expect("left parameter");
    let right = ValueId::new(31).expect("right parameter");
    let compared = ValueId::new(32).expect("compared");
    let result = ValueId::new(33).expect("result");
    let obligation = ObligationId::new(30).expect("obligation");
    let term = |id| ScalarTerm::value(id, ScalarType::Boolean);
    let equality = ScalarTerm::boolean_equal(term(left), term(right)).unwrap();
    let goal = Proposition::Equal(term(result), equality.clone());
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(30).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(30).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                ValueDeclaration {
                    id: left,
                    scalar_type: ScalarType::Boolean,
                },
                ValueDeclaration {
                    id: right,
                    scalar_type: ScalarType::Boolean,
                },
            ],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(30).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(30).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(30).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: compared,
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::BooleanEqual { left, right },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(30).expect("edge"),
                    value: compared,
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
        }],
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(30).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: Proposition::Equal(term(result), term(compared)),
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: Proposition::Equal(term(compared), equality),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                },
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("Boolean-equality semantics should reconstruct operation and return axioms");

    let integer =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 operand type"));
    let mut wrong_operand = module.clone();
    wrong_operand.machines[0].contract.ensures.clear();
    wrong_operand.machines[0].parameters[1].scalar_type = integer;
    assert_eq!(
        validate_module(&wrong_operand).expect_err("Boolean equality requires Boolean operands"),
        ModuleError::BooleanEqualOperandTypeMismatch {
            operation: OperationId::new(30).expect("operation"),
            operand: right,
            actual: integer,
        }
    );

    let mut wrong_result = module;
    wrong_result.machines[0].contract.ensures.clear();
    wrong_result.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = integer;
    assert_eq!(
        validate_module(&wrong_result).expect_err("Boolean equality requires a Boolean result"),
        ModuleError::BooleanEqualRequiresBooleanResult(OperationId::new(30).expect("operation"))
    );
}

#[test]
fn integer_equality_axiom_proves_the_return_contract() {
    let integer =
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 integer-equality operand type");
    let integer_scalar = ScalarType::Integer(integer);
    let left = ValueId::new(40).expect("left parameter");
    let right = ValueId::new(41).expect("right parameter");
    let compared = ValueId::new(42).expect("compared");
    let result = ValueId::new(43).expect("result");
    let obligation = ObligationId::new(40).expect("obligation");
    let value = |id, scalar_type| ScalarTerm::value(id, scalar_type);
    let equality = ScalarTerm::integer_equal(
        integer,
        value(left, integer_scalar),
        value(right, integer_scalar),
    )
    .expect("matching integer operands form equality");
    let goal = Proposition::Equal(value(result, ScalarType::Boolean), equality.clone());
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(40).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(40).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                ValueDeclaration {
                    id: left,
                    scalar_type: integer_scalar,
                },
                ValueDeclaration {
                    id: right,
                    scalar_type: integer_scalar,
                },
            ],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(40).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(40).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(40).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: compared,
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerEqual { left, right },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(40).expect("edge"),
                    value: compared,
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
        }],
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(40).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: Proposition::Equal(
                                value(result, ScalarType::Boolean),
                                value(compared, ScalarType::Boolean),
                            ),
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: Proposition::Equal(
                                value(compared, ScalarType::Boolean),
                                equality,
                            ),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                },
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("integer-equality semantics should reconstruct operation and return axioms");

    let signed_integer =
        IntegerType::new(IntegerSign::Signed, 8).expect("i8 mismatched operand type");
    let mut wrong_operand = module.clone();
    wrong_operand.machines[0].contract.ensures.clear();
    wrong_operand.machines[0].parameters[1].scalar_type = ScalarType::Integer(signed_integer);
    assert_eq!(
        validate_module(&wrong_operand)
            .expect_err("integer equality requires two operands of one exact integer type"),
        ModuleError::IntegerEqualOperandTypeMismatch {
            operation: OperationId::new(40).expect("operation"),
            left: integer_scalar,
            right: ScalarType::Integer(signed_integer),
        }
    );

    let mut wrong_result = module;
    wrong_result.machines[0].contract.ensures.clear();
    wrong_result.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = integer_scalar;
    assert_eq!(
        validate_module(&wrong_result).expect_err("integer equality requires a Boolean result"),
        ModuleError::IntegerEqualRequiresBooleanResult(OperationId::new(40).expect("operation"))
    );
}

#[test]
fn integer_ordering_axioms_prove_return_contracts() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8 ordering type");
    let integer_scalar = ScalarType::Integer(integer);
    let left = ValueId::new(50).expect("left parameter");
    let right = ValueId::new(51).expect("right parameter");
    let compared = ValueId::new(52).expect("compared");
    let result = ValueId::new(53).expect("result");
    let obligation = ObligationId::new(50).expect("obligation");
    let value = |id, scalar_type| ScalarTerm::value(id, scalar_type);

    for inclusive in [false, true] {
        let ordered = if inclusive {
            ScalarTerm::integer_less_or_equal(
                integer,
                value(left, integer_scalar),
                value(right, integer_scalar),
            )
            .expect("matching operands form less-or-equal")
        } else {
            ScalarTerm::integer_less_than(
                integer,
                value(left, integer_scalar),
                value(right, integer_scalar),
            )
            .expect("matching operands form less-than")
        };
        let goal = Proposition::Equal(value(result, ScalarType::Boolean), ordered.clone());
        let operation = if inclusive {
            OperationKind::IntegerLessOrEqual { left, right }
        } else {
            OperationKind::IntegerLessThan { left, right }
        };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: MachineId::new(50).expect("machine"),
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
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: MachineId::new(50).expect("machine"),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![
                    ValueDeclaration {
                        id: left,
                        scalar_type: integer_scalar,
                    },
                    ValueDeclaration {
                        id: right,
                        scalar_type: integer_scalar,
                    },
                ],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    id: result,
                    scalar_type: ScalarType::Boolean,
                }),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(50).expect("block"),
                blocks: vec![Block {
                    id: BlockId::new(50).expect("block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(50).expect("operation"),
                        result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                            id: compared,
                            scalar_type: ScalarType::Boolean,
                        }),
                        kind: operation,
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(50).expect("edge"),
                        value: compared,
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
            }],
        };
        let bundle = ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(50).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(ProofNode {
                                conclusion: Proposition::Equal(
                                    value(result, ScalarType::Boolean),
                                    value(compared, ScalarType::Boolean),
                                ),
                                rule: ProofRule::SemanticAxiom { index: 1 },
                            }),
                            middle_equals_right: Box::new(ProofNode {
                                conclusion: Proposition::Equal(
                                    value(compared, ScalarType::Boolean),
                                    ordered,
                                ),
                                rule: ProofRule::SemanticAxiom { index: 0 },
                            }),
                        },
                    },
                }),
            }],
        };
        verify_module(&module, &bundle, &AdmissionProfile::default())
            .expect("ordering reconstructs the exact operation and return axioms");

        let mut wrong_operand = module.clone();
        wrong_operand.machines[0].contract.ensures.clear();
        wrong_operand.machines[0].parameters[1].scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 mismatch"));
        assert!(matches!(
            validate_module(&wrong_operand),
            Err(ModuleError::IntegerOrderingOperandTypeMismatch { .. })
        ));

        let mut wrong_result = module.clone();
        wrong_result.machines[0].contract.ensures.clear();
        wrong_result.machines[0].blocks[0].operations[0]
            .result
            .scalar_mut()
            .unwrap()
            .scalar_type = integer_scalar;
        assert_eq!(
            validate_module(&wrong_result).expect_err("integer ordering requires a Boolean result"),
            ModuleError::IntegerOrderingRequiresBooleanResult(
                OperationId::new(50).expect("operation")
            )
        );
    }
}

#[test]
fn integer_bitwise_axioms_prove_exact_result_contracts() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 bitwise type");
    let scalar_type = ScalarType::Integer(integer);
    let left = ValueId::new(60).expect("left");
    let right = ValueId::new(61).expect("right");
    let computed = ValueId::new(62).expect("computed");
    let result = ValueId::new(63).expect("result");
    let value = |id| ScalarTerm::value(id, scalar_type);

    for kind in 0_u8..3 {
        let operation = match kind {
            0 => OperationKind::IntegerBitwiseAnd { left, right },
            1 => OperationKind::IntegerBitwiseOr { left, right },
            2 => OperationKind::IntegerBitwiseXor { left, right },
            _ => unreachable!(),
        };
        let term = match kind {
            0 => ScalarTerm::integer_bitwise_and(integer, value(left), value(right)),
            1 => ScalarTerm::integer_bitwise_or(integer, value(left), value(right)),
            2 => ScalarTerm::integer_bitwise_xor(integer, value(left), value(right)),
            _ => unreachable!(),
        }
        .expect("matching operands form a bitwise term");
        let goal = Proposition::Equal(value(result), term.clone());
        let obligation = ObligationId::new(60 + u64::from(kind)).expect("obligation");
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: MachineId::new(60).expect("machine"),
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
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: MachineId::new(60).expect("machine"),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![
                    ValueDeclaration {
                        id: left,
                        scalar_type,
                    },
                    ValueDeclaration {
                        id: right,
                        scalar_type,
                    },
                ],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    id: result,
                    scalar_type,
                }),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(60).expect("block"),
                blocks: vec![Block {
                    id: BlockId::new(60).expect("block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(60).expect("operation"),
                        result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                            id: computed,
                            scalar_type,
                        }),
                        kind: operation,
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(60).expect("edge"),
                        value: computed,
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
            }],
        };
        let bundle = ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(60 + u64::from(kind)).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(ProofNode {
                                conclusion: Proposition::Equal(value(result), value(computed)),
                                rule: ProofRule::SemanticAxiom { index: 1 },
                            }),
                            middle_equals_right: Box::new(ProofNode {
                                conclusion: Proposition::Equal(value(computed), term),
                                rule: ProofRule::SemanticAxiom { index: 0 },
                            }),
                        },
                    },
                }),
            }],
        };
        verify_module(&module, &bundle, &AdmissionProfile::default())
            .expect("reconstructs the exact bitwise result axiom");

        let mut wrong_operand = module.clone();
        wrong_operand.machines[0].contract.ensures.clear();
        wrong_operand.machines[0].parameters[1].scalar_type =
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).expect("i8 mismatch"));
        assert!(matches!(
            validate_module(&wrong_operand),
            Err(ModuleError::IntegerBitwiseOperandTypeMismatch { .. })
        ));

        let mut wrong_result = module.clone();
        wrong_result.machines[0].contract.ensures.clear();
        wrong_result.machines[0].blocks[0].operations[0]
            .result
            .scalar_mut()
            .unwrap()
            .scalar_type = ScalarType::Boolean;
        assert_eq!(
            validate_module(&wrong_result).expect_err("integer bitwise requires an integer result"),
            ModuleError::IntegerBitwiseRequiresIntegerResult(
                OperationId::new(60).expect("operation")
            )
        );
    }
}

#[test]
fn integer_bitwise_not_reconstructs_its_exact_result_axiom() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type");
    let scalar_type = ScalarType::Integer(integer);
    let operand = ValueId::new(65).expect("operand");
    let computed = ValueId::new(66).expect("computed");
    let result = ValueId::new(67).expect("result");
    let value = |id| ScalarTerm::value(id, scalar_type);
    let term = ScalarTerm::integer_bitwise_not(integer, value(operand)).unwrap();
    let goal = Proposition::Equal(value(result), term.clone());
    let obligation = ObligationId::new(65).expect("obligation");
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(65).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(65).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                id: operand,
                scalar_type,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(65).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(65).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(65).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: computed,
                        scalar_type,
                    }),
                    kind: OperationKind::IntegerBitwiseNot { operand },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(65).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(65).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal.clone(),
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(65).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: Proposition::Equal(value(result), value(computed)),
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: Proposition::Equal(value(computed), term),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                },
            }),
        }],
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("reconstructs the exact bitwise-not result axiom");

    let mut wrong_operand = module;
    wrong_operand.machines[0].contract.ensures.clear();
    wrong_operand.machines[0].parameters[0].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&wrong_operand),
        Err(ModuleError::IntegerBitwiseNotOperandTypeMismatch { .. })
    ));
}

#[test]
fn integer_widen_reconstructs_its_exact_result_axiom_and_rejects_partial_casts() {
    let source_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let target_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let source_scalar = ScalarType::Integer(source_type);
    let target_scalar = ScalarType::Integer(target_type);
    let operand = ValueId::new(68).expect("operand");
    let computed = ValueId::new(69).expect("computed");
    let result = ValueId::new(70).expect("result");
    let source = ScalarTerm::value(operand, source_scalar);
    let widened = ScalarTerm::integer_widen(source_type, target_type, source).unwrap();
    let goal = Proposition::Equal(ScalarTerm::value(result, target_scalar), widened.clone());
    let obligation = ObligationId::new(68).expect("obligation");
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(68).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(68).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                id: operand,
                scalar_type: source_scalar,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: target_scalar,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(68).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(68).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(68).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: computed,
                        scalar_type: target_scalar,
                    }),
                    kind: OperationKind::IntegerWiden { operand },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(68).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(68).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: vec![ContractClause {
                    obligation,
                    proposition: goal.clone(),
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(68).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(ProofNode {
                            conclusion: Proposition::Equal(
                                ScalarTerm::value(result, target_scalar),
                                ScalarTerm::value(computed, target_scalar),
                            ),
                            rule: ProofRule::SemanticAxiom { index: 1 },
                        }),
                        middle_equals_right: Box::new(ProofNode {
                            conclusion: Proposition::Equal(
                                ScalarTerm::value(computed, target_scalar),
                                widened,
                            ),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                    },
                },
            }),
        }],
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("reconstructs the exact widening result axiom");

    let mut narrowing = module.clone();
    narrowing.machines[0].contract.ensures.clear();
    narrowing.machines[0].parameters[0].scalar_type = target_scalar;
    narrowing.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = source_scalar;
    narrowing.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = source_scalar;
    assert!(matches!(
        validate_module(&narrowing),
        Err(ModuleError::IntegerWidenOperandTypeMismatch { .. })
    ));

    let mut cross_signedness = module;
    cross_signedness.machines[0].contract.ensures.clear();
    let unsigned_target = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 cross-sign target"),
    );
    cross_signedness.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = unsigned_target;
    cross_signedness.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = unsigned_target;
    assert!(matches!(
        validate_module(&cross_signedness),
        Err(ModuleError::IntegerWidenOperandTypeMismatch { .. })
    ));
}

#[test]
fn preserves_address_carrier_identity() {
    let address = ScalarType::Integer(IntegerType::address(64).expect("addr"));
    let parameter = ValueId::new(168).expect("parameter");
    let result = ValueId::new(169).expect("result");
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(168).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(168).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                id: parameter,
                scalar_type: address,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: address,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(168).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(168).expect("block"),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(168).expect("edge"),
                    value: parameter,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(168).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };

    validate_module(&module).expect("the distinct address carrier is admitted");
}

#[test]
fn exact_integer_cast_requires_a_distinct_fixed_partial_conversion_and_obligation() {
    let source =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 source"));
    let target =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 target"));
    let operand = ValueId::new(170).expect("operand");
    let computed = ValueId::new(171).expect("computed");
    let result = ValueId::new(172).expect("result");
    let cast_obligation = ObligationId::new(170).expect("cast obligation");
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(170).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(170).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![ValueDeclaration {
                id: operand,
                scalar_type: source,
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: target,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(170).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(170).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(170).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: computed,
                        scalar_type: target,
                    }),
                    kind: OperationKind::IntegerExactCast {
                        operand,
                        obligation: cast_obligation,
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(170).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(170).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits a proof-owned fixed partial conversion");
    let reconstructed =
        reconstruct_operation_obligations(&module).expect("reconstruct exact-cast goal");
    assert_eq!(reconstructed.len(), 1);
    assert!(reconstructed[0].canonical_certificate);
    assert_eq!(
        reconstructed[0].obligation.proposition,
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::MathValue {
                source_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                value: operand,
            },
            IntegerMathTerm::literal(IntegerValue::Unsigned(255)),
        ),
    );

    let mut redundant = module.clone();
    redundant.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = source;
    redundant.machines[0]
        .result
        .scalar_mut()
        .unwrap()
        .scalar_type = source;
    assert!(matches!(
        validate_module(&redundant),
        Err(ModuleError::IntegerExactCastOperandTypeMismatch { .. })
    ));

    let address = ScalarType::Integer(IntegerType::address(64).expect("addr"));
    module.machines[0].parameters[0].scalar_type = address;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::IntegerExactCastOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_right_shift_requires_fixed_integer_operands_and_an_obligation() {
    let value_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 value"));
    let count_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).expect("i16 count"));
    let value = ValueId::new(180).expect("value");
    let count = ValueId::new(181).expect("count");
    let computed = ValueId::new(182).expect("computed");
    let result = ValueId::new(183).expect("result");
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(180).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(180).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                ValueDeclaration {
                    id: value,
                    scalar_type: value_type,
                },
                ValueDeclaration {
                    id: count,
                    scalar_type: count_type,
                },
            ],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: value_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(180).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(180).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(180).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: computed,
                        scalar_type: value_type,
                    }),
                    kind: OperationKind::ExactIntegerShiftRight {
                        value,
                        count,
                        obligation: ObligationId::new(180).expect("shift obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(180).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(180).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact right shift");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerShiftOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_left_shift_requires_fixed_integer_operands_and_an_obligation() {
    let value_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 value"));
    let count_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).expect("i16 count"));
    let value = ValueId::new(190).expect("value");
    let count = ValueId::new(191).expect("count");
    let computed = ValueId::new(192).expect("computed");
    let result = ValueId::new(193).expect("result");
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(190).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(190).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                ValueDeclaration {
                    id: value,
                    scalar_type: value_type,
                },
                ValueDeclaration {
                    id: count,
                    scalar_type: count_type,
                },
            ],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: result,
                scalar_type: value_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(190).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(190).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(190).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: computed,
                        scalar_type: value_type,
                    }),
                    kind: OperationKind::ExactIntegerShiftLeft {
                        value,
                        count,
                        obligation: ObligationId::new(190).expect("shift obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(190).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(190).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact left shift");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerShiftOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_add_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let left = ValueId::new(194).expect("left");
    let right = ValueId::new(195).expect("right");
    let computed = ValueId::new(196).expect("computed");
    let result = ValueId::new(197).expect("result");
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(194).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(194).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(194).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(194).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(194).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerAdd {
                        left,
                        right,
                        obligation: ObligationId::new(194).expect("add obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(194).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(194).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact addition");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerAddOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_subtract_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let left = ValueId::new(198).expect("left");
    let right = ValueId::new(199).expect("right");
    let computed = ValueId::new(200).expect("computed");
    let result = ValueId::new(201).expect("result");
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(198).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(198).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(198).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(198).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(198).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerSubtract {
                        left,
                        right,
                        obligation: ObligationId::new(198).expect("subtract obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(198).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(198).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact subtraction");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerSubtractOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_multiply_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let left = ValueId::new(202).expect("left");
    let right = ValueId::new(203).expect("right");
    let computed = ValueId::new(204).expect("computed");
    let result = ValueId::new(205).expect("result");
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(202).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(202).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(202).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(202).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(202).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerMultiply {
                        left,
                        right,
                        obligation: ObligationId::new(202).expect("multiply obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(202).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(202).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact multiplication");

    let obligations = reconstruct_operation_obligations(&module)
        .expect("reconstruct total canonical multiplication goal");
    assert_eq!(obligations.len(), 1);
    assert!(
        obligations[0].canonical_certificate,
        "the last migrated arithmetic row must not retain legacy sufficient reduction",
    );

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerMultiplyOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_divide_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let left = ValueId::new(212).expect("left");
    let right = ValueId::new(213).expect("right");
    let computed = ValueId::new(214).expect("computed");
    let result = ValueId::new(215).expect("result");
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(212).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(212).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(212).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(212).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(212).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerDivide {
                        left,
                        right,
                        obligation: ObligationId::new(212).expect("divide obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(212).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(212).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact division");

    let without_available_route =
        reconstruct_operation_obligations(&module).expect("reconstruct canonical divide goal");
    assert_eq!(without_available_route.len(), 1);
    assert!(without_available_route[0].canonical_certificate);
    let canonical_question = without_available_route[0].obligation.proposition.clone();

    let mut module_with_alternate_route = module.clone();
    module_with_alternate_route.machines[0]
        .contract
        .requires
        .push(Proposition::LessOrEqual(
            ScalarTerm::integer(
                IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"),
                IntegerValue::Unsigned(1),
            )
            .expect("u32 one"),
            ScalarTerm::value(right, scalar_type),
        ));
    let with_alternate_route = reconstruct_operation_obligations(&module_with_alternate_route)
        .expect("available proof routes do not choose the reconstructed goal");
    assert!(with_alternate_route[0].canonical_certificate);
    assert_eq!(
        with_alternate_route[0].obligation.proposition, canonical_question,
        "available evidence must not make reconstruction select another question",
    );

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerDivideOperandTypeMismatch { .. })
    ));
}

#[test]
fn exact_remainder_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"));
    let left = ValueId::new(222).expect("left");
    let right = ValueId::new(223).expect("right");
    let computed = ValueId::new(224).expect("computed");
    let result = ValueId::new(225).expect("result");
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(222).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(222).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(222).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(222).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(222).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::ExactIntegerRemainder {
                        left,
                        right,
                        obligation: ObligationId::new(222).expect("remainder obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(222).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(222).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated exact remainder");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::ExactIntegerRemainderOperandTypeMismatch { .. })
    ));
}

#[test]
fn wrapping_divide_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let left = ValueId::new(232).expect("left");
    let right = ValueId::new(233).expect("right");
    let computed = ValueId::new(234).expect("computed");
    let result = ValueId::new(235).expect("result");
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(232).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(232).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(232).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(232).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(232).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::WrappingIntegerDivide {
                        left,
                        right,
                        obligation: ObligationId::new(232).expect("divide obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(232).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(232).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated wrapping division");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::WrappingIntegerDivideOperandTypeMismatch { .. })
    ));
}

#[test]
fn wrapping_remainder_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let left = ValueId::new(242).expect("left");
    let right = ValueId::new(243).expect("right");
    let computed = ValueId::new(244).expect("computed");
    let result = ValueId::new(245).expect("result");
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(242).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(242).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(242).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(242).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(242).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::WrappingIntegerRemainder {
                        left,
                        right,
                        obligation: ObligationId::new(242).expect("remainder obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(242).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(242).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated wrapping remainder");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::WrappingIntegerRemainderOperandTypeMismatch { .. })
    ));
}

#[test]
fn saturating_divide_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let left = ValueId::new(252).expect("left");
    let right = ValueId::new(253).expect("right");
    let computed = ValueId::new(254).expect("computed");
    let result = ValueId::new(255).expect("result");
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(252).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(252).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(252).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(252).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(252).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::SaturatingIntegerDivide {
                        left,
                        right,
                        obligation: ObligationId::new(252).expect("divide obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(252).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(252).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated saturating division");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::SaturatingIntegerDivideOperandTypeMismatch { .. })
    ));
}

#[test]
fn saturating_remainder_requires_same_fixed_integer_operands_and_an_obligation() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).expect("i32"));
    let left = ValueId::new(256).expect("left");
    let right = ValueId::new(257).expect("right");
    let computed = ValueId::new(258).expect("computed");
    let result = ValueId::new(259).expect("result");
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let mut module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(256).expect("machine"),
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(256).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![declaration(left), declaration(right)],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(256).expect("block"),
            blocks: vec![Block {
                id: BlockId::new(256).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: OperationId::new(256).expect("operation"),
                    result: psi_terminal::OperationResult::Scalar(declaration(computed)),
                    kind: OperationKind::SaturatingIntegerRemainder {
                        left,
                        right,
                        obligation: ObligationId::new(256).expect("remainder obligation"),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(256).expect("edge"),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(256).expect("contract"),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    validate_module(&module).expect("admits proof-gated saturating remainder");

    module.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::SaturatingIntegerRemainderOperandTypeMismatch { .. })
    ));
}

#[test]
fn wrapping_shift_axioms_preserve_the_count_type() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 value type");
    let count_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16 count type");
    let value_scalar = ScalarType::Integer(value_type);
    let count_scalar = ScalarType::Integer(count_type);
    let value = ValueId::new(70).expect("value");
    let count = ValueId::new(71).expect("count");
    let computed = ValueId::new(72).expect("computed");
    let result = ValueId::new(73).expect("result");
    let value_term = |id| ScalarTerm::value(id, value_scalar);
    let count_term = |id| ScalarTerm::value(id, count_scalar);

    for kind in 0_u8..2 {
        let operation = if kind == 0 {
            OperationKind::WrappingIntegerShiftLeft { value, count }
        } else {
            OperationKind::WrappingIntegerShiftRight { value, count }
        };
        let term = if kind == 0 {
            ScalarTerm::wrapping_integer_shift_left(
                value_type,
                count_type,
                value_term(value),
                count_term(count),
            )
        } else {
            ScalarTerm::wrapping_integer_shift_right(
                value_type,
                count_type,
                value_term(value),
                count_term(count),
            )
        }
        .expect("independently typed shift operands");
        let goal = Proposition::Equal(value_term(result), term.clone());
        let obligation = ObligationId::new(70 + u64::from(kind)).expect("obligation");
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: MachineId::new(70).expect("machine"),
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
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: MachineId::new(70).expect("machine"),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![
                    ValueDeclaration {
                        id: value,
                        scalar_type: value_scalar,
                    },
                    ValueDeclaration {
                        id: count,
                        scalar_type: count_scalar,
                    },
                ],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(ValueDeclaration {
                    id: result,
                    scalar_type: value_scalar,
                }),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(70).expect("block"),
                blocks: vec![Block {
                    id: BlockId::new(70).expect("block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(70).expect("operation"),
                        result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                            id: computed,
                            scalar_type: value_scalar,
                        }),
                        kind: operation,
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(70).expect("edge"),
                        value: computed,
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
            }],
        };
        let bundle = ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(70 + u64::from(kind)).expect("certificate"),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(ProofNode {
                                conclusion: Proposition::Equal(
                                    value_term(result),
                                    value_term(computed),
                                ),
                                rule: ProofRule::SemanticAxiom { index: 1 },
                            }),
                            middle_equals_right: Box::new(ProofNode {
                                conclusion: Proposition::Equal(value_term(computed), term),
                                rule: ProofRule::SemanticAxiom { index: 0 },
                            }),
                        },
                    },
                }),
            }],
        };
        verify_module(&module, &bundle, &AdmissionProfile::default())
            .expect("reconstructs the exact wrapping-shift result axiom");

        if kind == 0 {
            let mut wrong_count = module.clone();
            wrong_count.machines[0].contract.ensures.clear();
            wrong_count.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
            assert!(matches!(
                validate_module(&wrong_count),
                Err(ModuleError::WrappingIntegerShiftOperandTypeMismatch { .. })
            ));

            let mut wrong_result = module.clone();
            wrong_result.machines[0].contract.ensures.clear();
            wrong_result.machines[0].blocks[0].operations[0]
                .result
                .scalar_mut()
                .unwrap()
                .scalar_type = ScalarType::Boolean;
            assert_eq!(
                validate_module(&wrong_result)
                    .expect_err("wrapping shift requires an integer result"),
                ModuleError::WrappingIntegerShiftRequiresIntegerResult(
                    OperationId::new(70).expect("operation")
                )
            );
        }
    }
}

#[test]
fn content_conservation_accepts_a_replaceable_certificate() {
    let (module, goal, obligation) = reflexive_content_module();
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(80).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                },
            }),
        }],
    };

    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("content proposition and certificate");
    assert_eq!(verified.accepted_facts().len(), 1);
}

#[test]
fn identity_reshuffle_reconstructs_content_equality_as_a_semantic_axiom() {
    let (module, goal, obligation) = identity_reshuffle_module();
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(90).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::SemanticAxiom { index: 0 },
                },
            }),
        }],
    };

    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("an exact identity reshuffle should establish its content equality");
    assert_eq!(verified.accepted_facts().len(), 1);
}

#[test]
fn structural_return_rejects_inexact_custody_and_scalar_content_carriers() {
    let (module, _, _) = identity_reshuffle_module();

    let mut missing_structural_claim = module.clone();
    missing_structural_claim.machines[0].entry_claims.clear();
    assert!(matches!(
        validate_module(&missing_structural_claim),
        Err(ModuleError::LinearParameterHasNoEntryClaim { .. })
    ));

    let mut wrong_claim = module.clone();
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut wrong_claim.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *returned_claims = vec![ClaimId::new(2).unwrap()];
    assert!(matches!(
        validate_module(&wrong_claim),
        Err(ModuleError::StructuralReturnClaimSetMismatch { .. })
    ));

    let mut wrong_source = module.clone();
    let result_place = wrong_source.machines[0].result.structural().unwrap().place;
    let Terminator::ReturnStructural { source, .. } =
        &mut wrong_source.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    *source = result_place;
    assert!(matches!(
        validate_module(&wrong_source),
        Err(ModuleError::StructuralReturnRequiresParameterSource { .. })
    ));

    let mut wrong_root = module.clone();
    wrong_root.machines[0].content_identity_reshuffles[0]
        .output
        .root = PlaceId::new(999).unwrap();
    assert!(matches!(
        validate_module(&wrong_root),
        Err(ModuleError::ContentIdentityReshuffleRequiresCurrentResult(
            _
        ))
    ));

    let mut wrong_signature = module.clone();
    let result = wrong_signature.machines[0]
        .result
        .structural()
        .unwrap()
        .clone();
    wrong_signature
        .structural_domains
        .push(psi_terminal::StructuralDomainDeclaration {
            id: psi_core::StructuralDomainId::new(91).unwrap(),
            semantic_domain: psi_core::DomainSemanticId::new(91).unwrap(),
            identity: "Other".into(),
            carrier: result.structural_type,
            content_projection: None,
        });
    let TerminalMachineResult::Structural(result) = &mut wrong_signature.machines[0].result else {
        unreachable!()
    };
    result
        .qualifications
        .push(psi_core::StructuralDomainId::new(91).unwrap());
    assert!(matches!(
        validate_module(&wrong_signature),
        Err(ModuleError::StructuralReturnSignatureMismatch { .. })
    ));

    let mut scalar_carrier = module;
    scalar_carrier.machines[0].result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: ValueId::new(999).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    assert!(matches!(
        validate_module(&scalar_carrier),
        Err(ModuleError::ScalarMachineHasResultStructuralPlace { .. })
    ));

    scalar_carrier.machines[0].result = TerminalMachineResult::Unit;
    assert!(matches!(
        validate_module(&scalar_carrier),
        Err(ModuleError::UnitMachineHasResultStructuralPlace { .. })
    ));
}

#[test]
fn structural_call_rebinds_one_exact_whole_root_claim_and_rejects_tampering() {
    let module = structural_call_module();
    validate_module(&module).expect("one exact whole-root structural call");

    let mut wrong_source_claim = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut wrong_source_claim.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    returned_claim_transfers[0].callee_claim = ClaimId::new(2).unwrap();
    assert_eq!(
        validate_module(&wrong_source_claim).unwrap_err(),
        ModuleError::StructuralCallClaimInterfaceMismatch(OperationId::new(1).unwrap())
    );

    let mut wrong_producer = module.clone();
    let StructuralPlaceKind::OperationResult { producer, .. } =
        &mut wrong_producer.machines[0].structural_places[2].kind
    else {
        unreachable!()
    };
    *producer = OperationId::new(2).unwrap();
    assert_eq!(
        validate_module(&wrong_producer).unwrap_err(),
        ModuleError::StructuralCallResultPlaceMismatch(OperationId::new(2).unwrap())
    );

    let mut duplicated_claim = module.clone();
    let result = duplicated_claim.machines[0].blocks[0].operations[0]
        .result
        .structural()
        .unwrap()
        .clone();
    let OperationResult::Structural(result_mut) =
        &mut duplicated_claim.machines[0].blocks[0].operations[0].result
    else {
        unreachable!()
    };
    result_mut.claims.push(result.claims[0].clone());
    assert_eq!(
        validate_module(&duplicated_claim).unwrap_err(),
        ModuleError::NonCanonicalStructuralOperationResult(OperationId::new(1).unwrap())
    );
}

#[test]
fn structural_call_rebinds_an_exact_multi_claim_whole_root_map() {
    let module = multi_claim_structural_call_module();
    validate_module(&module).expect("two exact projected claims cross a whole-root call");

    let mut swapped_paths = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut swapped_paths.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let first_caller = returned_claim_transfers[0].caller_claim;
    returned_claim_transfers[0].caller_claim = returned_claim_transfers[1].caller_claim;
    returned_claim_transfers[1].caller_claim = first_caller;
    assert_eq!(
        validate_module(&swapped_paths).unwrap_err(),
        ModuleError::StructuralCallClaimInterfaceMismatch(OperationId::new(1).unwrap())
    );

    let mut missing_return = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut missing_return.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    returned_claim_transfers.pop();
    assert_eq!(
        validate_module(&missing_return).unwrap_err(),
        ModuleError::StructuralCallClaimInterfaceMismatch(OperationId::new(1).unwrap())
    );

    let mut duplicate_caller = module.clone();
    let OperationKind::CallStructural {
        returned_claim_transfers,
        ..
    } = &mut duplicate_caller.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    returned_claim_transfers[1].caller_claim = ClaimId::new(1).unwrap();
    assert_eq!(
        validate_module(&duplicate_caller).unwrap_err(),
        ModuleError::StructuralCallClaimInterfaceMismatch(OperationId::new(1).unwrap())
    );

    let mut callee_returns_a_subset = module;
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut callee_returns_a_subset.machines[1].blocks[0].terminator
    else {
        unreachable!()
    };
    returned_claims.pop();
    assert_eq!(
        validate_module(&callee_returns_a_subset).unwrap_err(),
        ModuleError::StructuralCallClaimInterfaceMismatch(OperationId::new(1).unwrap())
    );
}

#[test]
fn structural_return_requires_exact_trivial_affine_local_establishment_and_cleanup() {
    let (mut module, _, _) = identity_reshuffle_module();
    let machine = &mut module.machines[0];
    let second_affine_parameter_place = PlaceId::new(775).unwrap();
    let affine_parameter_place = PlaceId::new(776).unwrap();
    let local_place = PlaceId::new(777).unwrap();
    let second_local_place = PlaceId::new(778).unwrap();
    let local_type = StructuralTypeId::new(777).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: local_type,
        identity: "EmptyScratch".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: local_place,
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            structural_type: local_type,
            construction: None,
        },
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: second_local_place,
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 1,
            structural_type: local_type,
            construction: None,
        },
    });
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: affine_parameter_place,
            position: 1,
            is_self: false,
            structural_type: local_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: second_affine_parameter_place,
            position: 2,
            is_self: false,
            structural_type: local_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: affine_parameter_place,
        kind: StructuralPlaceKind::Parameter {
            position: 1,
            is_self: false,
        },
    });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: second_affine_parameter_place,
        kind: StructuralPlaceKind::Parameter {
            position: 2,
            is_self: false,
        },
    });
    machine.blocks[0].operations.push(Operation {
        id: OperationId::new(777).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::EstablishTrivialAffineLocal {
            destination: local_place,
        },
    });
    machine.blocks[0].operations.push(Operation {
        id: OperationId::new(778).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::EstablishTrivialAffineLocal {
            destination: second_local_place,
        },
    });
    let Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[0].terminator
    else {
        unreachable!()
    };
    *trivial_affine_discards = vec![
        second_local_place,
        local_place,
        second_affine_parameter_place,
        affine_parameter_place,
    ];
    validate_module(&module).expect("exact local establishment and cleanup validate");

    let mut claim_bearing_tail = module.clone();
    claim_bearing_tail.machines[0]
        .entry_claims
        .push(EntryClaim {
            claim: ClaimId::new(775).unwrap(),
            input: second_affine_parameter_place,
            path: Vec::new(),
        });
    assert!(validate_module(&claim_bearing_tail).is_err());

    let mut ordinal_gap = module.clone();
    let StructuralPlaceKind::TrivialAffineLocal {
        declaration_ordinal,
        ..
    } = &mut ordinal_gap.machines[0]
        .structural_places
        .iter_mut()
        .find(|place| place.id == second_local_place)
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    *declaration_ordinal = 2;
    assert!(matches!(
        validate_module(&ordinal_gap),
        Err(ModuleError::NonCanonicalTrivialAffineLocals(_))
    ));

    let mut reordered_establishment = module.clone();
    reordered_establishment.machines[0].blocks[0]
        .operations
        .reverse();
    assert!(matches!(
        validate_module(&reordered_establishment),
        Err(ModuleError::TrivialAffineLocalEstablishmentMismatch(_))
    ));

    let mut missing_cleanup = module.clone();
    let Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut missing_cleanup.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.clear();
    assert!(matches!(
        validate_module(&missing_cleanup),
        Err(ModuleError::StructuralReturnAffineDiscardsMismatch { .. })
    ));

    let mut reordered_cleanup = module.clone();
    let Terminator::ReturnStructural {
        trivial_affine_discards,
        ..
    } = &mut reordered_cleanup.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.reverse();
    assert!(matches!(
        validate_module(&reordered_cleanup),
        Err(ModuleError::StructuralReturnAffineDiscardsMismatch { .. })
    ));

    let mut missing_establishment = module.clone();
    missing_establishment.machines[0].blocks[0]
        .operations
        .clear();
    assert!(matches!(
        validate_module(&missing_establishment),
        Err(ModuleError::TrivialAffineLocalEstablishmentMismatch(_))
    ));

    let mut duplicate_establishment = module.clone();
    let duplicate = duplicate_establishment.machines[0].blocks[0].operations[0].clone();
    duplicate_establishment.machines[0].blocks[0]
        .operations
        .push(Operation {
            id: OperationId::new(779).unwrap(),
            ..duplicate
        });
    assert!(matches!(
        validate_module(&duplicate_establishment),
        Err(ModuleError::TrivialAffineLocalEstablishmentMismatch(_))
    ));

    let mut nonempty = module;
    let StructuralTypeShape::Record { fields } = &mut nonempty.structural_types[1].shape else {
        panic!("expected record shape")
    };
    fields.push(StructuralFieldDeclaration {
        id: psi_core::StructuralFieldId::new(1).unwrap(),
        identity: "marker".into(),
        relevance: psi_terminal::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
    });
    assert!(matches!(
        validate_module(&nonempty),
        Err(ModuleError::TrivialAffineLocalDeclarationRequiresEmptyRecord { .. })
    ));
}

#[test]
fn sum_case_identity_reshuffle_reconstructs_content_equality() {
    let (mut module, _, obligation) = identity_reshuffle_module();
    module.vocabulary_marker = VocabularyMarker::CURRENT;
    module.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    let TerminalMachineResult::Structural(result) = &mut module.machines[0].result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    module.machines[0].entry_claims.clear();
    let segments = vec![
        ContentPlaceSegment::Case("Present".to_owned()),
        ContentPlaceSegment::Field("region".to_owned()),
    ];
    module.machines[0].content_entry_claims[0].input.segments = segments.clone();
    let reshuffle = &mut module.machines[0].content_identity_reshuffles[0];
    reshuffle.input.segments = segments.clone();
    reshuffle.output.segments = segments;
    let goal = reshuffle
        .inferred_propositions()
        .next()
        .expect("one projection yields one proposition");
    module.machines[0].contract.ensures[0].proposition = goal.clone();
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(91).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::SemanticAxiom { index: 0 },
                },
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("a case-plus-field reshuffle should establish its content equality");
}

#[test]
fn payloadless_sum_case_membership_resolves_exact_case_identity() {
    let (mut module, _, _) = identity_reshuffle_module();
    let root = module.machines[0].structural_parameters[0].place;
    let case = StructuralCaseId::new(90).expect("case");
    module.structural_types[0].shape = StructuralTypeShape::Sum {
        cases: vec![StructuralCaseDeclaration {
            id: case,
            identity: "Present".to_owned(),
            fields: Vec::new(),
        }],
    };
    module.machines[0]
        .contract
        .requires
        .push(Proposition::StructuralCaseMembership {
            subject: StructuralCaseSubject::new(root, Vec::new()),
            case,
        });
    validate_module(&module).expect("declared payload-less case membership validates");

    let mut redirected = module.clone();
    let Proposition::StructuralCaseMembership { case, .. } =
        &mut redirected.machines[0].contract.requires[0]
    else {
        unreachable!()
    };
    *case = StructuralCaseId::new(91).expect("redirected case");
    let redirected_error = validate_module(&redirected);
    assert!(
        matches!(
            redirected_error,
            Err(ModuleError::InvalidStructuralCaseMembership { .. })
        ),
        "unexpected redirected-case result: {redirected_error:?}"
    );

    let mut empty = module;
    empty.structural_types[0].shape = StructuralTypeShape::Sum { cases: Vec::new() };
    assert!(matches!(
        validate_module(&empty),
        Err(ModuleError::EmptyStructuralSum(_))
    ));
}

#[test]
fn outcome_specific_contract_carrier_is_exact_and_replay_stays_fail_closed() {
    let (mut module, _, _) = identity_reshuffle_module();
    let result_type = module.structural_types[0].id;
    let success = StructuralCaseId::new(90).expect("success case");
    let failure = StructuralCaseId::new(91).expect("failure case");
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
    let proposition = psi_core::PropositionId::new(1).expect("proposition");
    let term = psi_core::EvidenceTermId::new(1).expect("term");
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
    let executable_shape = module.machines[0].blocks.clone();
    let operation_count = executable_shape
        .iter()
        .map(|block| block.operations.len())
        .sum::<usize>();
    let guard = OutcomeSpecificGuard {
        result_type,
        result_case: success,
    };
    module.machines[0].contract.outcome_specific_ensures = vec![
        OutcomeSpecificEnsure {
            guard,
            position: 0,
            obligation: ObligationId::new(91).expect("named obligation"),
            proposition: Proposition::Atom(proposition),
            evidence: Some(OutcomeSpecificEvidence {
                term,
                output_field: "selected".to_owned(),
            }),
        },
        OutcomeSpecificEnsure {
            guard,
            position: 1,
            obligation: ObligationId::new(92).expect("unnamed obligation"),
            proposition: Proposition::Truth,
            evidence: None,
        },
    ];

    assert_eq!(module.machines[0].blocks, executable_shape);
    assert_eq!(
        module.machines[0]
            .blocks
            .iter()
            .map(|block| block.operations.len())
            .sum::<usize>(),
        operation_count,
        "guarded rows add no runtime operations or fuel-bearing sites",
    );

    validate_module(&module).expect("guarded rows have a valid exact sum/case carrier");
    assert_eq!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        )
        .expect_err("guarded executable replay remains fail closed"),
        VerificationError::Module(ModuleError::OutcomeSpecificGuaranteeReplayUnavailable {
            machine: module.machines[0].id,
            obligation: ObligationId::new(91).unwrap(),
        })
    );

    let mut wrong_case = module.clone();
    wrong_case.machines[0].contract.outcome_specific_ensures[0]
        .guard
        .result_case = StructuralCaseId::new(92).unwrap();
    assert!(matches!(
        validate_module(&wrong_case),
        Err(ModuleError::InvalidOutcomeSpecificGuard { .. })
    ));

    let mut mismatched_term = module.clone();
    mismatched_term.machines[0]
        .contract
        .outcome_specific_ensures[0]
        .proposition = Proposition::Truth;
    assert!(matches!(
        validate_module(&mismatched_term),
        Err(ModuleError::OutcomeSpecificEvidenceMismatch { .. })
    ));

    let mut non_dense = module;
    non_dense.machines[0].contract.outcome_specific_ensures[1].position = 2;
    assert!(matches!(
        validate_module(&non_dense),
        Err(ModuleError::NonDenseOutcomeSpecificEnsures { .. })
    ));
}

#[test]
fn exact_payloadless_guard_rebases_result_case_and_replays_only_matching_unnamed_rows() {
    let (mut module, success, failure, result_place) = payloadless_guard_module();
    let success_obligation = ObligationId::new(920).expect("success obligation");
    let failure_obligation = ObligationId::new(921).expect("failure obligation");
    let result_type = module.machines[0]
        .result
        .structural()
        .unwrap()
        .structural_type;
    module.machines[0].contract.outcome_specific_ensures = vec![
        OutcomeSpecificEnsure {
            guard: OutcomeSpecificGuard {
                result_type,
                result_case: success,
            },
            position: 0,
            obligation: success_obligation,
            proposition: Proposition::Truth,
            evidence: None,
        },
        OutcomeSpecificEnsure {
            guard: OutcomeSpecificGuard {
                result_type,
                result_case: failure,
            },
            position: 0,
            obligation: failure_obligation,
            proposition: Proposition::Truth,
            evidence: None,
        },
    ];
    let reconstructed =
        reconstruct_terminal_obligations(&module).expect("the exact guarded return reconstructs");
    let [site] = reconstructed.obligations() else {
        panic!("only the matching unnamed row creates an obligation")
    };
    assert_eq!(site.obligation.id, success_obligation);
    assert!(
        site.semantic_axioms
            .contains(&Proposition::StructuralCaseMembership {
                subject: StructuralCaseSubject::new(result_place, Vec::new()),
                case: success,
            })
    );
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: success_obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        }],
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("the matching truth row verifies and the sibling is vacuous");

    let mut sibling_route = bundle;
    sibling_route.evidence.push(ObligationEvidence {
        obligation: failure_obligation,
        route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
    });
    assert_eq!(
        verify_module(&module, &sibling_route, &AdmissionProfile::default())
            .expect_err("a vacuous sibling cannot accept a proof route"),
        VerificationError::UnknownEvidence(failure_obligation)
    );
}

#[test]
fn exact_payloadless_guard_accepts_a_forwarded_required_term_without_fresh_provenance() {
    let (mut module, success, _, _) = payloadless_guard_module();
    let machine = module.machines[0].id;
    let result_type = module.machines[0]
        .result
        .structural()
        .unwrap()
        .structural_type;
    let proposition = psi_core::PropositionId::new(1).expect("proposition");
    let term = psi_core::EvidenceTermId::new(1).expect("term");
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
    module.evidence_contract_lanes = vec![EvidenceContractLane {
        machine,
        kind: EvidenceContractLaneKind::Requires,
        position: 0,
        term,
        output_field: None,
    }];
    module.machines[0].contract.outcome_specific_ensures = vec![OutcomeSpecificEnsure {
        guard: OutcomeSpecificGuard {
            result_type,
            result_case: success,
        },
        position: 0,
        obligation: ObligationId::new(920).expect("named row identity"),
        proposition: Proposition::Atom(proposition),
        evidence: Some(OutcomeSpecificEvidence {
            term,
            output_field: "selected".to_owned(),
        }),
    }];

    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("the exact required term identity forwards into the active named row");
}

#[test]
fn multi_exit_payloadless_guards_intersect_only_exits_of_the_same_case() {
    let (mut module, success, failure, absent, result_place) =
        multi_exit_payloadless_guard_module();
    let result_type = module.machines[0]
        .result
        .structural()
        .unwrap()
        .structural_type;
    let global_obligation = ObligationId::new(929).expect("global obligation");
    let success_obligation = ObligationId::new(930).expect("success obligation");
    let failure_obligation = ObligationId::new(931).expect("failure obligation");
    let absent_obligation = ObligationId::new(932).expect("absent obligation");
    module.machines[0].contract.ensures = vec![ContractClause {
        obligation: global_obligation,
        proposition: Proposition::Truth,
    }];
    module.machines[0].contract.outcome_specific_ensures = vec![
        OutcomeSpecificEnsure {
            guard: OutcomeSpecificGuard {
                result_type,
                result_case: success,
            },
            position: 0,
            obligation: success_obligation,
            proposition: Proposition::Truth,
            evidence: None,
        },
        OutcomeSpecificEnsure {
            guard: OutcomeSpecificGuard {
                result_type,
                result_case: failure,
            },
            position: 0,
            obligation: failure_obligation,
            proposition: Proposition::Truth,
            evidence: None,
        },
        OutcomeSpecificEnsure {
            guard: OutcomeSpecificGuard {
                result_type,
                result_case: absent,
            },
            position: 0,
            obligation: absent_obligation,
            proposition: Proposition::Truth,
            evidence: None,
        },
    ];

    let reconstructed = reconstruct_terminal_obligations(&module)
        .expect("every ordinary exit has an exact payloadless case");
    assert_eq!(
        reconstructed
            .obligations()
            .iter()
            .map(|site| site.obligation.id)
            .collect::<Vec<_>>(),
        [global_obligation, success_obligation, failure_obligation],
        "the absent case remains vacuous"
    );
    let success_site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == success_obligation)
        .expect("Success has a reconstructed row");
    let success_membership = Proposition::StructuralCaseMembership {
        subject: StructuralCaseSubject::new(result_place, Vec::new()),
        case: success,
    };
    let failure_membership = Proposition::StructuralCaseMembership {
        subject: StructuralCaseSubject::new(result_place, Vec::new()),
        case: failure,
    };
    assert!(success_site.semantic_axioms.contains(&success_membership));
    assert!(!success_site.semantic_axioms.contains(&failure_membership));
    let failure_site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == failure_obligation)
        .expect("Failure has a reconstructed row");
    assert!(failure_site.semantic_axioms.contains(&failure_membership));
    assert!(!failure_site.semantic_axioms.contains(&success_membership));
    let global_site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == global_obligation)
        .expect("the unconditional row is reconstructed");
    assert!(
        !global_site.semantic_axioms.contains(&success_membership)
            && !global_site.semantic_axioms.contains(&failure_membership),
        "the unconditional intersection cannot inherit a case-local fact"
    );

    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: [global_obligation, success_obligation, failure_obligation]
            .into_iter()
            .map(|obligation| ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            })
            .collect(),
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("both reached cases verify against independent intersections");
    let mut stray = bundle;
    stray.evidence.push(ObligationEvidence {
        obligation: absent_obligation,
        route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
    });
    assert_eq!(
        verify_module(&module, &stray, &AdmissionProfile::default())
            .expect_err("the absent case cannot consume proof evidence"),
        VerificationError::UnknownEvidence(absent_obligation)
    );
}

#[test]
fn multi_exit_payloadless_guards_activate_named_producers_by_reached_case_set() {
    let (mut module, success, failure, absent, _) = multi_exit_payloadless_guard_module();
    let result_type = module.machines[0]
        .result
        .structural()
        .unwrap()
        .structural_type;
    let proposition = psi_core::PropositionId::new(1).expect("proposition");
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
    module.evidence_terms = (1..=3)
        .map(|raw| EvidenceTermDeclaration {
            id: psi_core::EvidenceTermId::new(raw).unwrap(),
            proposition,
            interface: interface.clone(),
        })
        .collect();
    module.machines[0].contract.outcome_specific_ensures = [
        (success, "success"),
        (failure, "failure"),
        (absent, "absent"),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(index, (result_case, output_field))| OutcomeSpecificEnsure {
            guard: OutcomeSpecificGuard {
                result_type,
                result_case,
            },
            position: 0,
            obligation: ObligationId::new(940 + index as u64).unwrap(),
            proposition: Proposition::Atom(proposition),
            evidence: Some(OutcomeSpecificEvidence {
                term: psi_core::EvidenceTermId::new(index as u64 + 1).unwrap(),
                output_field: output_field.to_owned(),
            }),
        },
    )
    .collect();
    let producer = |raw| EvidenceProducerProvenance {
        id: EvidenceIdentity::new(raw).unwrap(),
        term: psi_core::EvidenceTermId::new(raw).unwrap(),
        conformance_identity: "ConcreteEvidence".to_owned(),
        evidence_trait_identity: "ReadyEvidence".to_owned(),
        rows: Vec::new(),
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: vec![producer(1), producer(2)],
        evidence: Vec::new(),
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("each reached case activates its named producer exactly once");

    let mut vacuous = bundle;
    vacuous.evidence_producers[1] = EvidenceProducerProvenance {
        id: EvidenceIdentity::new(2).unwrap(),
        ..producer(3)
    };
    assert_eq!(
        verify_module(&module, &vacuous, &AdmissionProfile::default())
            .expect_err("a producer for the absent case remains unused"),
        VerificationError::UnusedEvidenceProducerTerm(psi_core::EvidenceTermId::new(3).unwrap())
    );
}

#[test]
fn partition_composition_is_available_after_its_exact_successful_call() {
    let (module, goal, obligation) = partition_composition_module();
    validate_module(&module).expect("the partition substitution remains valid replay evidence");
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(92).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::SemanticAxiom { index: 0 },
                },
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("the exact successful producer call establishes the replayed theorem");
}

#[test]
fn partition_composition_is_scheduled_strictly_after_the_producer_call() {
    let (module, goal, _) = partition_composition_module();
    let source_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let add_probe = |operation, result, obligation| Operation {
        id: operation,
        result: OperationResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type: ScalarType::Integer(source_type),
        }),
        kind: OperationKind::ExactIntegerAdd {
            left: ValueId::new(92).expect("probe input"),
            right: ValueId::new(92).expect("probe input"),
            obligation,
        },
    };

    let mut before = module.clone();
    before.machines[0].parameters.push(ValueDeclaration {
        id: ValueId::new(92).expect("probe input"),
        scalar_type: ScalarType::Integer(source_type),
    });
    let before_obligation = ObligationId::new(89).expect("before obligation");
    before.machines[0].blocks[0].operations.insert(
        0,
        add_probe(
            OperationId::new(89).expect("before operation"),
            ValueId::new(93).expect("before result"),
            before_obligation,
        ),
    );
    let before_rows =
        reconstruct_operation_obligations(&before).expect("the before-call probe reconstructs");
    let before_row = before_rows
        .iter()
        .find(|row| row.obligation.id == before_obligation)
        .expect("before-call obligation");
    assert!(!before_row.semantic_axioms.contains(&goal));

    let mut after = module;
    after.machines[0].parameters.push(ValueDeclaration {
        id: ValueId::new(92).expect("probe input"),
        scalar_type: ScalarType::Integer(source_type),
    });
    let after_obligation = ObligationId::new(91).expect("after obligation");
    after.machines[0].blocks[0].operations.push(add_probe(
        OperationId::new(91).expect("after operation"),
        ValueId::new(93).expect("after result"),
        after_obligation,
    ));
    let after_rows =
        reconstruct_operation_obligations(&after).expect("the after-call probe reconstructs");
    let after_row = after_rows
        .iter()
        .find(|row| row.obligation.id == after_obligation)
        .expect("after-call obligation");
    assert!(after_row.semantic_axioms.contains(&goal));
}

#[test]
fn partition_composition_rejects_missing_and_noncall_producers() {
    let (mut missing, _, _) = partition_composition_module();
    missing.machines[0].content_partition_compositions[0].producer_operation =
        OperationId::new(99).expect("missing operation");
    assert_eq!(
        validate_module(&missing).expect_err("the producer operation must exist in this machine"),
        ModuleError::ContentPartitionProducerOperationMissing(
            OperationId::new(99).expect("missing operation")
        )
    );

    let (mut noncall, _, _) = partition_composition_module();
    let noncall_operation = OperationId::new(91).expect("noncall operation");
    noncall.machines[0].blocks[0].operations.push(Operation {
        id: noncall_operation,
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(92).expect("constant result"),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    });
    noncall.machines[0].content_partition_compositions[0].producer_operation = noncall_operation;
    assert_eq!(
        validate_module(&noncall).expect_err("a non-call cannot establish an authored theorem"),
        ModuleError::ContentPartitionProducerNotCall(noncall_operation)
    );
}

#[test]
fn partition_composition_requires_the_target_authored_guarantee() {
    let (mut module, _, _) = partition_composition_module();
    module.boundary_machines[0].content_guarantees.clear();
    assert_eq!(
        validate_module(&module).expect_err("a replay row cannot invent its source theorem"),
        ModuleError::ContentPartitionProducerGuaranteeMissing(
            OperationId::new(90).expect("producer operation")
        )
    );
}

#[test]
fn entry_claims_require_dense_unique_parameter_bindings() {
    let (mut sparse, _, _) = identity_reshuffle_module();
    let reshuffle = sparse.machines[0].content_identity_reshuffles[0].clone();
    sparse.vocabulary_marker = VocabularyMarker::CURRENT;
    sparse.machines[0].content_identity_reshuffles.clear();
    sparse.machines[0].content_entry_claims = vec![ContentEntryClaim {
        claim: ClaimId::new(2).expect("sparse claim"),
        input: reshuffle.input.clone(),
        projections: reshuffle.projections.clone(),
    }];
    assert_eq!(
        validate_module(&sparse).expect_err("entry claims must be dense"),
        ModuleError::NonDenseContentEntryClaim {
            expected: ClaimId::new(1).expect("expected claim"),
            actual: ClaimId::new(2).expect("actual claim"),
        }
    );

    let (mut overlapping, _, _) = identity_reshuffle_module();
    overlapping.vocabulary_marker = VocabularyMarker::CURRENT;
    overlapping.machines[0].content_identity_reshuffles.clear();
    let first = ContentEntryClaim {
        claim: ClaimId::new(1).expect("first claim"),
        input: reshuffle.input.clone(),
        projections: reshuffle.projections.clone(),
    };
    let mut second = first.clone();
    second.claim = ClaimId::new(2).expect("second claim");
    second
        .input
        .segments
        .push(ContentPlaceSegment::Field("child".to_owned()));
    overlapping.machines[0].content_entry_claims = vec![first, second];
    assert!(matches!(
        validate_module(&overlapping),
        Err(ModuleError::OverlappingContentEntryClaimInput { .. })
    ));
}

#[test]
fn crash_is_an_explicit_no_successor_exit() {
    let mut module = Fixture::new().module;
    module.machines[0].contract.crash_routes = vec![psi_terminal::CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![psi_terminal::CrashRouteGuard::Truth],
    }];
    module.machines[0].contract.ensures.clear();
    module.machines[0].blocks[1].terminator = Terminator::Crash {
        edge: EdgeId::new(10).expect("crash edge"),
        cause: CrashCause::Abort,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    validate_module(&module).expect("Psi explicitly represents a no-cleanup crash");

    module.machines[0].contract.crash_routes.clear();
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::CrashRouteUncovered { block, cause: CrashCause::Abort })
            if block == BlockId::new(2).expect("block")
    ));
}

#[test]
fn crash_route_buckets_are_canonical_nonempty_disjunctions() {
    let mut module = Fixture::new().module;
    module.machines[0].contract.crash_routes = vec![psi_terminal::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: Vec::new(),
    }];
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::EmptyCrashRouteBucket {
            cause: CrashCause::Trap,
            ..
        })
    ));

    module.machines[0].contract.crash_routes[0].alternatives = vec![
        psi_terminal::CrashRouteGuard::Truth,
        psi_terminal::CrashRouteGuard::Predicate(psi_terminal::CrashPredicateTerm::new(
            Proposition::Equal(ScalarTerm::boolean(true), ScalarTerm::boolean(true)),
        )),
    ];
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::NonCanonicalCrashRouteAlternatives { .. })
    ));

    module.machines[0].contract.crash_routes = vec![
        psi_terminal::CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![psi_terminal::CrashRouteGuard::Truth],
        },
        psi_terminal::CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![psi_terminal::CrashRouteGuard::Predicate(
                psi_terminal::CrashPredicateTerm::new(Proposition::Equal(
                    ScalarTerm::boolean(true),
                    ScalarTerm::boolean(true),
                )),
            )],
        },
    ];
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::NonCanonicalCrashRoutes(_))
    ));
}

#[test]
fn guarded_crash_requires_a_matching_canonical_route() {
    let mut module = Fixture::new().module;
    module.vocabulary_marker = VocabularyMarker::CURRENT;
    module.machines[0].contract.ensures.clear();
    let seven =
        ScalarTerm::integer(Fixture::new().integer, IntegerValue::Signed(7)).expect("seven");
    let predicate = psi_terminal::CrashPredicateTerm::new(Proposition::Equal(seven.clone(), seven));
    module.machines[0].contract.crash_routes = vec![psi_terminal::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![psi_terminal::CrashRouteGuard::Predicate(predicate.clone())],
    }];
    module.machines[0].blocks[1].terminator = Terminator::Crash {
        edge: EdgeId::new(10).expect("crash edge"),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::CrashRouteUncovered {
            cause: CrashCause::Trap,
            ..
        })
    ));

    let Terminator::Crash { site_guard, .. } = &mut module.machines[0].blocks[1].terminator else {
        unreachable!()
    };
    site_guard.push(predicate);
    validate_module(&module).expect("the site guard implies its exact published route");
}

#[test]
fn crash_frontier_must_name_every_still_live_entry_claim() {
    let (mut module, _, _) = identity_reshuffle_module();
    module.vocabulary_marker = VocabularyMarker::CURRENT;
    let reshuffle = module.machines[0].content_identity_reshuffles.remove(0);
    let claim = ClaimId::new(1).expect("claim");
    module.machines[0].content_entry_claims = vec![ContentEntryClaim {
        claim,
        input: reshuffle.input,
        projections: reshuffle.projections,
    }];
    module.machines[0].contract.ensures.clear();
    module.machines[0].contract.crash_routes = vec![psi_terminal::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![psi_terminal::CrashRouteGuard::Truth],
    }];
    module.machines[0].blocks[0].terminator = Terminator::Crash {
        edge: EdgeId::new(90).expect("crash edge"),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let crash_block = module.machines[0].blocks[0].id;
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::CrashFrontierMismatch { block }) if block == crash_block
    ));

    let Terminator::Crash {
        frontier_lower_bound,
        ..
    } = &mut module.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    frontier_lower_bound.push(claim);
    validate_module(&module).expect("the explicit local crash frontier is complete");
}

#[test]
fn partition_composition_rejects_theorem_drift() {
    let (mut fingerprint_drift, _, _) = partition_composition_module();
    let composition = &mut fingerprint_drift.machines[0].content_partition_compositions[0];
    let reconstructed = composition.source_report_fingerprint;
    composition.source_report_fingerprint ^= 1;
    assert_eq!(
        validate_module(&fingerprint_drift)
            .expect_err("the source theorem fingerprint must be independently reconstructed"),
        ModuleError::ContentPartitionSourceFingerprintMismatch {
            recorded: reconstructed ^ 1,
            reconstructed: Some(reconstructed),
        }
    );

    let (mut drifted, _, _) = partition_composition_module();
    let composition = &mut drifted.machines[0].content_partition_compositions[0];
    let ContentTerm::Separate(children) = composition.derived.right().clone() else {
        panic!("fixture has a separated result")
    };
    composition.derived = ContentConservation::new(
        composition.derived.algebra().clone(),
        composition.derived.left().clone(),
        children[0].clone(),
    );
    assert_eq!(
        validate_module(&drifted).expect_err("the derived equation must replay exactly"),
        ModuleError::ContentPartitionReplayMismatch
    );
}

#[test]
fn sum_case_content_paths_require_nonempty_case_names() {
    let (mut empty, _, _) = identity_reshuffle_module();
    empty.machines[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    let TerminalMachineResult::Structural(result) = &mut empty.machines[0].result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    empty.machines[0].entry_claims.clear();
    empty.machines[0].content_identity_reshuffles[0]
        .input
        .segments = vec![ContentPlaceSegment::Case(String::new())];
    empty.machines[0].content_entry_claims[0].input.segments =
        vec![ContentPlaceSegment::Case(String::new())];
    assert_eq!(
        validate_module(&empty).expect_err("case spellings are semantic identity"),
        ModuleError::MalformedProposition(PropositionError::EmptyContentCaseName)
    );
}

#[test]
fn identity_reshuffles_fail_closed_when_malformed() {
    let (mut parentless_result, _, _) = identity_reshuffle_module();
    parentless_result.machines[0].content_entry_claims.clear();
    assert_eq!(
        validate_module(&parentless_result)
            .expect_err("a qualified result cannot mint content without an exact parent claim"),
        ModuleError::ContentIdentityClaimHasNoEntryBinding(ClaimId::new(1).expect("claim"))
    );

    let (mut empty, _, _) = identity_reshuffle_module();
    empty.machines[0].content_identity_reshuffles[0]
        .projections
        .clear();
    assert_eq!(
        validate_module(&empty).expect_err("a claim must preserve named content"),
        ModuleError::ContentIdentityReshuffleHasNoProjections(ClaimId::new(1).expect("claim"))
    );

    let (mut wrong_input, _, _) = identity_reshuffle_module();
    wrong_input.machines[0].content_identity_reshuffles[0]
        .input
        .version = ContentPlaceVersion::Current;
    wrong_input.machines[0].content_entry_claims[0]
        .input
        .version = ContentPlaceVersion::Current;
    assert_eq!(
        validate_module(&wrong_input).expect_err("input must denote parameter entry content"),
        ModuleError::ContentEntryClaimRequiresEntryParameter(ClaimId::new(1).expect("claim"))
    );

    let (mut reordered, _, _) = identity_reshuffle_module();
    let machine = &mut reordered.machines[0];
    machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    let TerminalMachineResult::Structural(result) = &mut machine.result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    machine.entry_claims.clear();
    machine.content_entry_claims[0].input.segments =
        vec![ContentPlaceSegment::Field("left".to_owned())];
    machine.content_identity_reshuffles[0].input.segments =
        vec![ContentPlaceSegment::Field("left".to_owned())];
    machine.content_identity_reshuffles[0].output.segments =
        vec![ContentPlaceSegment::Field("left".to_owned())];
    let mut second_binding = machine.content_entry_claims[0].clone();
    second_binding.claim = ClaimId::new(2).expect("second claim");
    second_binding.input.segments = vec![ContentPlaceSegment::Field("right".to_owned())];
    machine.content_entry_claims.push(second_binding);
    let mut second_reshuffle = machine.content_identity_reshuffles[0].clone();
    second_reshuffle.claim = ClaimId::new(2).expect("second claim");
    second_reshuffle.input.segments = vec![ContentPlaceSegment::Field("right".to_owned())];
    second_reshuffle.output.segments = vec![ContentPlaceSegment::Field("right".to_owned())];
    machine
        .content_identity_reshuffles
        .insert(0, second_reshuffle);
    assert_eq!(
        validate_module(&reordered).expect_err("semantic axiom indices require canonical claims"),
        ModuleError::NonCanonicalContentIdentityReshuffles(MachineId::new(90).expect("machine"))
    );
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
            psi_language_semantics::content::terminal_projection_report_fingerprint(
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
        entry_claims: vec![psi_terminal::EntryClaim {
            claim,
            input: input_root,
            path: Vec::new(),
        }],
        published_service_ceiling: Vec::new(),
        parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: output_root,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: Vec::new(),
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
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types: vec![StructuralTypeDeclaration {
                id: structural_type,
                identity: "Region".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            }],
            structural_domains: vec![StructuralDomainDeclaration {
                id: psi_core::StructuralDomainId::new(90).expect("structural domain"),
                semantic_domain: psi_core::DomainSemanticId::new(90).expect("semantic domain"),
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
        place: caller_result,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
    };
    let callee_result_declaration = StructuralResultDeclaration {
        place: callee_result,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
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
            id: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: call,
                result: OperationResult::Structural(StructuralOperationResult {
                    place: call_result,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Linear,
                    qualifications: Vec::new(),
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
        vec![psi_terminal::StructuralPathSegment::FixedIndex(0)],
        vec![psi_terminal::StructuralPathSegment::FixedIndex(1)],
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
            psi_language_semantics::content::terminal_projection_report_fingerprint(
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
        id: MachineId::new(90).expect("machine"),
        attachment: None,
        parameters: vec![ValueDeclaration {
            id: parameter,
            scalar_type: ScalarType::Boolean,
        }],
        structural_parameters: vec![structural_parameter.clone()],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
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
            id: BlockId::new(90).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
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
        id: boundary_id,
        identity: "Splitter::partition".to_owned(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![structural_parameter],
        result: None,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: vec![ContentConservationGuarantee {
            report_fingerprint: source_report_fingerprint,
            structural_places: source_structural_places,
            conservation: source,
        }],
        published_service_ceiling: Vec::new(),
    };
    (
        TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine.id,
            structural_types: vec![StructuralTypeDeclaration {
                id: structural_type,
                identity: "Region".to_owned(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            }],
            structural_domains: vec![StructuralDomainDeclaration {
                id: psi_core::StructuralDomainId::new(90).expect("structural domain"),
                semantic_domain: psi_core::DomainSemanticId::new(90).expect("semantic domain"),
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
            quotient_correspondences: Vec::new(),
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

#[test]
fn content_conservation_is_ensures_only_and_entry_cannot_name_result() {
    let (mut requires_module, goal, _) = reflexive_content_module();
    requires_module.machines[0].contract.ensures.clear();
    requires_module.machines[0].contract.requires.push(goal);
    assert_eq!(
        validate_module(&requires_module).expect_err("content is post-state only"),
        ModuleError::ContentConservationRequiresEnsures {
            contract: ContractId::new(80).expect("contract"),
        }
    );

    let (mut result_entry_module, _, _) = reflexive_content_module();
    result_entry_module.machines[0].structural_places[0].kind = StructuralPlaceKind::Result;
    assert_eq!(
        validate_module(&result_entry_module).expect_err("result has no entry version"),
        ModuleError::ScalarMachineHasResultStructuralPlace {
            machine: MachineId::new(80).expect("machine"),
            place: PlaceId::new(80).expect("place"),
        }
    );

    let (mut duplicate_parameter, _, _) = reflexive_content_module();
    duplicate_parameter.machines[0]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: PlaceId::new(81).expect("second place"),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        });
    assert_eq!(
        validate_module(&duplicate_parameter)
            .expect_err("one parameter position has one structural root"),
        ModuleError::DuplicateStructuralPlaceRoot {
            machine: MachineId::new(80).expect("machine"),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        }
    );
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
        id: MachineId::new(80).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![ValueDeclaration {
            id: parameter,
            scalar_type: ScalarType::Boolean,
        }],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
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
            quotient_correspondences: Vec::new(),
            machines: vec![machine],
        },
        goal,
        obligation,
    )
}

#[test]
fn wrapping_add_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = wrapping_add_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let sum = ScalarTerm::wrapping_integer_add(integer, term(20), term(21)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(23), term(22)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(22), sum),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(20).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("wrapping-add semantics should reconstruct both axioms");
}

#[test]
fn wrapping_add_requires_defined_exact_type_operands() {
    let (mut use_before_definition, _, _) = wrapping_add_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::WrappingIntegerAdd {
            left: ValueId::new(22).expect("sum result"),
            right: ValueId::new(21).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(22).expect("sum result"))
    );

    let (mut wrong_type, _, _) = wrapping_add_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::WrappingIntegerAddOperandTypeMismatch {
            operation: OperationId::new(20).expect("add operation"),
            operand: ValueId::new(21).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn saturating_add_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = saturating_add_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let sum = ScalarTerm::saturating_integer_add(integer, term(30), term(31)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(33), term(32)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(32), sum),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(30).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("saturating-add semantics should reconstruct both axioms");
}

#[test]
fn saturating_add_requires_defined_exact_type_operands() {
    let (mut use_before_definition, _, _) = saturating_add_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::SaturatingIntegerAdd {
            left: ValueId::new(32).expect("sum result"),
            right: ValueId::new(31).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(32).expect("sum result"))
    );

    let (mut wrong_type, _, _) = saturating_add_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::SaturatingIntegerAddOperandTypeMismatch {
            operation: OperationId::new(30).expect("add operation"),
            operand: ValueId::new(31).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn wrapping_subtract_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = wrapping_subtract_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let difference = ScalarTerm::wrapping_integer_subtract(integer, term(40), term(41)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(43), term(42)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(42), difference),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(40).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("wrapping-subtract semantics should reconstruct both axioms");
}

#[test]
fn wrapping_subtract_requires_defined_exact_type_operands() {
    let (mut use_before_definition, _, _) = wrapping_subtract_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::WrappingIntegerSubtract {
            left: ValueId::new(42).expect("difference result"),
            right: ValueId::new(41).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(42).expect("difference result"))
    );

    let (mut wrong_type, _, _) = wrapping_subtract_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::WrappingIntegerSubtractOperandTypeMismatch {
            operation: OperationId::new(40).expect("subtract operation"),
            operand: ValueId::new(41).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn saturating_subtract_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = saturating_subtract_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let difference = ScalarTerm::saturating_integer_subtract(integer, term(50), term(51)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(53), term(52)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(52), difference),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(50).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("saturating-subtract semantics should reconstruct both axioms");
}

#[test]
fn saturating_subtract_requires_defined_exact_type_operands() {
    let (mut use_before_definition, _, _) = saturating_subtract_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::SaturatingIntegerSubtract {
            left: ValueId::new(52).expect("difference result"),
            right: ValueId::new(51).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(52).expect("difference result"))
    );

    let (mut wrong_type, _, _) = saturating_subtract_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::SaturatingIntegerSubtractOperandTypeMismatch {
            operation: OperationId::new(50).expect("subtract operation"),
            operand: ValueId::new(51).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn wrapping_multiply_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = wrapping_multiply_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let product = ScalarTerm::wrapping_integer_multiply(integer, term(60), term(61)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(63), term(62)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(62), product),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(60).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("wrapping-multiply semantics should reconstruct both axioms");
}

#[test]
fn wrapping_multiply_requires_defined_exact_type_operands() {
    let (mut use_before_definition, _, _) = wrapping_multiply_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::WrappingIntegerMultiply {
            left: ValueId::new(62).expect("product result"),
            right: ValueId::new(61).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(62).expect("product result"))
    );

    let (mut wrong_type, _, _) = wrapping_multiply_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::WrappingIntegerMultiplyOperandTypeMismatch {
            operation: OperationId::new(60).expect("multiply operation"),
            operand: ValueId::new(61).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn saturating_multiply_axiom_proves_the_return_contract() {
    let (module, goal, obligation) = saturating_multiply_module();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let scalar_type = ScalarType::Integer(integer);
    let term = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar_type);
    let product = ScalarTerm::saturating_integer_multiply(integer, term(70), term(71)).unwrap();
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(73), term(72)),
                rule: ProofRule::SemanticAxiom { index: 1 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(72), product),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(70).expect("certificate"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };

    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("saturating-multiply semantics should reconstruct both axioms");
}

#[test]
fn saturating_multiply_requires_defined_exact_type_operands() {
    let (mut use_before_definition, _, _) = saturating_multiply_module();
    use_before_definition.machines[0].blocks[0].operations[0].kind =
        OperationKind::SaturatingIntegerMultiply {
            left: ValueId::new(72).expect("product result"),
            right: ValueId::new(71).expect("right parameter"),
        };
    assert_eq!(
        validate_module(&use_before_definition).expect_err("self-reference must fail closed"),
        ModuleError::ValueUsedBeforeDefinition(ValueId::new(72).expect("product result"))
    );

    let (mut wrong_type, _, _) = saturating_multiply_module();
    wrong_type.machines[0].parameters[1].scalar_type = ScalarType::Boolean;
    wrong_type.machines[0].contract.ensures.clear();
    assert_eq!(
        validate_module(&wrong_type).expect_err("mixed operand types must fail closed"),
        ModuleError::SaturatingIntegerMultiplyOperandTypeMismatch {
            operation: OperationId::new(70).expect("multiply operation"),
            operand: ValueId::new(71).expect("right parameter"),
            expected: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")),
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn initial_control_vocabulary_rejects_unreachable_semantic_axioms() {
    let mut fixture = Fixture::new();
    fixture.module.machines[0].blocks.push(Block {
        id: BlockId::new(3).expect("unreachable block"),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Return {
            cleanup_actions: Vec::new(),
            edge: EdgeId::new(3).expect("unreachable edge"),
            value: fixture.constant,
        },
    });
    assert_eq!(
        validate_module(&fixture.module).expect_err("unreachable facts must not enter proofs"),
        ModuleError::UnreachableBlock(BlockId::new(3).expect("unreachable block"))
    );
}

#[test]
fn entry_requirements_cannot_assume_an_internal_operation_value() {
    let mut fixture = Fixture::new();
    fixture.module.machines[0]
        .contract
        .requires
        .push(Proposition::Equal(
            ScalarTerm::value(fixture.constant, ScalarType::Integer(fixture.integer)),
            ScalarTerm::integer(fixture.integer, IntegerValue::Signed(7)).expect("seven"),
        ));
    assert_eq!(
        validate_module(&fixture.module).expect_err("internal facts do not exist at entry"),
        ModuleError::ContractValueOutsideScope {
            contract: ContractId::new(1).expect("contract"),
            clause: ContractClauseKind::Requires,
            value: fixture.constant,
        }
    );
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
        id: MachineId::new(20).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(20).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(20).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(20).expect("add operation"),
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
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
        id: MachineId::new(30).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(30).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(30).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(30).expect("add operation"),
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
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
        id: MachineId::new(40).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(40).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(40).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(40).expect("subtract operation"),
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
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
        id: MachineId::new(50).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(50).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(50).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(50).expect("subtract operation"),
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
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
        id: MachineId::new(60).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(60).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(60).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(60).expect("multiply operation"),
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
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
        id: MachineId::new(70).expect("machine"),
        attachment: None,
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![
            ValueDeclaration {
                id: left,
                scalar_type,
            },
            ValueDeclaration {
                id: right,
                scalar_type,
            },
        ],
        ranked_scc: None,
        result: TerminalMachineResult::Scalar(ValueDeclaration {
            id: result,
            scalar_type,
        }),
        structural_places: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(70).expect("block"),
        blocks: vec![Block {
            id: BlockId::new(70).expect("block"),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: OperationId::new(70).expect("multiply operation"),
                result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
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
    let route = |identity, obligation: &psi_proof_admission::CertificateObligation| {
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
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
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
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
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
        id: operation,
        result: OperationResult::Structural(StructuralOperationResult {
            place: operation_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishPayloadlessCase {
            result_case: success,
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
            id: first,
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            id: second,
            scalar_type: ScalarType::Boolean,
        },
    ];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
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
        id: BlockId::new(block_raw).unwrap(),
        parameters: Vec::new(),
        operations: vec![Operation {
            id: operation,
            result: OperationResult::Structural(StructuralOperationResult {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishPayloadlessCase { result_case },
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
            id: BlockId::new(930).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: first,
                when_true: SuccessorEdge {
                    edge: EdgeId::new(930).unwrap(),
                    target: BlockId::new(931).unwrap(),
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    edge: EdgeId::new(931).unwrap(),
                    target: BlockId::new(934).unwrap(),
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            id: BlockId::new(931).unwrap(),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition: second,
                when_true: SuccessorEdge {
                    edge: EdgeId::new(932).unwrap(),
                    target: BlockId::new(932).unwrap(),
                    arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
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
            id: MachineId::new(1).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
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
                    id: BlockId::new(1).expect("entry block"),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: OperationId::new(1).expect("constant operation"),
                        result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                            id: constant,
                            scalar_type,
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Signed(7),
                        },
                    }],
                    terminator: Terminator::Jump {
                        edge: EdgeId::new(1).expect("jump edge"),
                        target: BlockId::new(2).expect("exit block"),
                        arguments: vec![constant],
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    id: BlockId::new(2).expect("exit block"),
                    parameters: vec![ValueDeclaration {
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
