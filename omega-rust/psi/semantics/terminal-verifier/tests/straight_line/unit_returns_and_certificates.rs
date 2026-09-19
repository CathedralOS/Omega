use super::{
    Fixture, contract_id, placed_view_input, placed_view_input_at_state, proof_recursive_bundle,
    proof_recursive_module, unit_module,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceError, EvidenceRoute, PrimitiveJudgment,
    ProofError, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, MachineId,
    ObligationId, OperationId, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralCaseId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, ContractClause, MachineContract, Operation, OperationKind, OperationResult,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, VerificationError,
    proof_recursive_component_identity, reconstruct_terminal_obligations, validate_module,
    verify_module,
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
fn optimizer_transition_moves_the_existing_checked_evidence() {
    let module = proof_recursive_module();
    let bundle = proof_recursive_bundle(&module);
    let profile = AdmissionProfile::default();
    let verified = verify_module(&module, &bundle, &profile).expect("ordinary admission");
    assert!(!verified.proof_bundle().recursive_components.is_empty());
    let evidence_storage = verified.proof_bundle().recursive_components.as_ptr();
    let expected = terminal_verifier::verify_module_for_optimization(&module, &bundle, &profile)
        .expect("independent optimizer admission");

    let optimizable = verified.into_optimization().expect("optimizer eligibility");
    assert!(std::ptr::eq(optimizable.module(), &module));
    assert_eq!(
        optimizable.proof_bundle().recursive_components.as_ptr(),
        evidence_storage
    );
    assert_eq!(optimizable.proof_bundle(), expected.proof_bundle());
    assert_eq!(
        optimizable.reconstructed_obligations(),
        expected.reconstructed_obligations()
    );
    assert_eq!(optimizable.accepted_facts(), expected.accepted_facts());
    assert_eq!(
        optimizable.structural_frontiers(),
        expected.structural_frontiers()
    );
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
            error: proof_admission::RecursiveComponentError::NonCanonicalCertificateEdges,
        }
    );

    let mut extra = bundle;
    let mut extra_row = extra.recursive_components[0].clone();
    extra_row.component = semantic_vocabulary::RecursiveComponentId::new(u64::MAX).unwrap();
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
        qualifications: Default::default(),
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
        static_reach_binding: None,
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
                        id: semantic_vocabulary::StructuralFieldId::new(902).unwrap(),
                        identity: "value".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                    }],
                },
            ],
        },
    });
    let machine = &mut module.machines[0];
    machine.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: result_place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
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
            result_case: payloadless,
            fields: vec![],
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
    let OperationKind::EstablishScalarCase { result_case, .. } =
        &mut unknown.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *result_case = StructuralCaseId::new(904).unwrap();
    assert!(matches!(
        validate_module(&unknown),
        Err(ModuleError::ScalarCaseFieldMismatch { .. })
    ));

    let mut bearing = module.clone();
    let OperationKind::EstablishScalarCase { result_case, .. } =
        &mut bearing.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *result_case = payload_bearing;
    assert!(matches!(
        validate_module(&bearing),
        Err(ModuleError::ScalarCaseFieldMismatch { .. })
    ));

    let mut non_sum = module.clone();
    non_sum.structural_types.last_mut().unwrap().shape =
        StructuralTypeShape::Record { fields: Vec::new() };
    assert!(matches!(
        validate_module(&non_sum),
        Err(ModuleError::ScalarCaseRequiresSum { .. })
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
        .push(semantic_vocabulary::StructuralDomainId::new(902).unwrap());
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
    let fact_storage = verified.accepted_facts().as_ptr();
    let optimizable = verified.into_optimization().expect("optimizer eligibility");
    assert_eq!(optimizable.accepted_facts().as_ptr(), fact_storage);
    let independent = terminal_verifier::verify_module_for_optimization(
        &fixture.module,
        &fixture.proof_bundle(),
        &AdmissionProfile::default(),
    )
    .expect("independent optimizer admission");
    assert_eq!(optimizable.accepted_facts(), independent.accepted_facts());
    assert_eq!(
        optimizable.reconstructed_obligations(),
        independent.reconstructed_obligations()
    );
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
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        }],
    };
    let certificate_bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
        control_cycles: Vec::new(),
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
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(10).expect("machine"),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(10).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(10).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(10).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
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
                erased_scalar_formals: Vec::new(),
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
        control_cycles: Vec::new(),
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
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(20).expect("machine"),
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
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(20).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(20).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(20).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
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
                erased_scalar_formals: Vec::new(),
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
        control_cycles: Vec::new(),
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
                            rule: ProofRule::SemanticAxiom { index: 3 },
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

    let mut polarized = module.clone();
    let original_goal = Proposition::Equal(term(result), ScalarTerm::boolean(true));
    let operand_fact = Proposition::Equal(term(parameter), ScalarTerm::boolean(false));
    let result_fact = Proposition::Equal(term(negated), ScalarTerm::boolean(true));
    let implication = Proposition::Implication {
        premise: Box::new(operand_fact.clone()),
        conclusion: Box::new(result_fact.clone()),
    };
    polarized.machines[0].contract.requires = vec![operand_fact.clone()];
    polarized.machines[0].contract.ensures[0].proposition = original_goal.clone();
    let mut polarized_bundle = bundle.clone();
    let EvidenceRoute::CertificateDerived(certificate) = &mut polarized_bundle.evidence[0].route
    else {
        unreachable!()
    };
    certificate.proof = ProofNode {
        conclusion: original_goal,
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: Proposition::Equal(term(result), term(negated)),
                rule: ProofRule::SemanticAxiom { index: 3 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: result_fact,
                rule: ProofRule::ImplicationElimination {
                    implication: Box::new(ProofNode {
                        conclusion: implication.clone(),
                        rule: ProofRule::SemanticAxiom { index: 1 },
                    }),
                    premise: Box::new(ProofNode {
                        conclusion: operand_fact,
                        rule: ProofRule::Assumption { index: 0 },
                    }),
                },
            }),
        },
    };
    let reconstructed = reconstruct_terminal_obligations(&polarized).unwrap();
    let site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == obligation)
        .unwrap();
    assert_eq!(site.semantic_axioms.len(), 4);
    assert_eq!(site.semantic_axioms[1], implication);
    assert!(
        matches!(&site.semantic_axioms[0], Proposition::Equal(left, ScalarTerm::BooleanNot { .. }) if left == &term(negated))
    );
    assert_eq!(
        site.semantic_axioms[3],
        Proposition::Equal(term(result), term(negated))
    );
    verify_module(&polarized, &polarized_bundle, &AdmissionProfile::default()).expect(
        "kernel checks the original result goal through operation polarity and return alias",
    );

    let mut changed_operation = polarized.clone();
    changed_operation.machines[0].blocks[0].operations[0].kind =
        OperationKind::BooleanConstant { value: false };
    assert!(
        verify_module(
            &changed_operation,
            &polarized_bundle,
            &AdmissionProfile::default()
        )
        .is_err()
    );
    let mut missing_operation = polarized.clone();
    missing_operation.machines[0].blocks[0].operations.clear();
    assert!(
        verify_module(
            &missing_operation,
            &polarized_bundle,
            &AdmissionProfile::default()
        )
        .is_err()
    );
    let mut redirected_result = polarized;
    redirected_result.machines[0].blocks[0].operations[0]
        .result
        .scalar_mut()
        .unwrap()
        .id = ValueId::new(23).unwrap();
    assert!(
        verify_module(
            &redirected_result,
            &polarized_bundle,
            &AdmissionProfile::default()
        )
        .is_err()
    );

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
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
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
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
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
                    scalar_type: ScalarType::Boolean,
                },
                ValueDeclaration {
                    qualifications: Default::default(),
                    id: right,
                    scalar_type: ScalarType::Boolean,
                },
            ],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: result,
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(30).expect("block"),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(30).expect("block"),
                parameters: Vec::new(),
                operations: vec![Operation {
                    static_reach_binding: None,
                    id: OperationId::new(30).expect("operation"),
                    result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
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
                erased_scalar_formals: Vec::new(),
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
        control_cycles: Vec::new(),
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
                            rule: ProofRule::SemanticAxiom { index: 3 },
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
