use super::{
    Fixture, identity_reshuffle_module, multi_exit_payloadless_guard_module,
    partition_composition_module, payloadless_guard_module,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, ClaimId, ContentConservation, ContentPlaceSegment, ContentPlaceVersion, ContentTerm,
    EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue, MachineId, ObligationId,
    OperationId, PlaceId, Proposition, PropositionError, ScalarTerm, ScalarType, StructuralCaseId,
    StructuralCaseSubject, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    ContentEntryClaim, ContractClause, CrashCause, EntryClaim, EvidenceContractLane,
    EvidenceContractLaneKind, EvidenceInterfaceIdentity, EvidenceTermDeclaration, Operation,
    OperationKind, OperationResult, OutcomeSpecificEnsure, OutcomeSpecificEvidence,
    OutcomeSpecificGuard, PropositionApplicationIdentity, PropositionDeclaration,
    PropositionEvidence, StructuralAccess, StructuralCaseDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalMachineResult, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    EvidenceProducerProvenance, ModuleError, ObligationEvidence, ProofBundle, VerificationError,
    reconstruct_operation_obligations, reconstruct_terminal_obligations, validate_module,
    verify_module,
};

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
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(777).unwrap(),
        result: OperationResult::Unit,
        kind: OperationKind::EstablishTrivialAffineLocal {
            destination: local_place,
        },
    });
    machine.blocks[0].operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
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
        id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
        identity: "marker".into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
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
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
    let proposition = semantic_vocabulary::PropositionId::new(1).expect("proposition");
    let term = semantic_vocabulary::EvidenceTermId::new(1).expect("term");
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
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
    let proposition = semantic_vocabulary::PropositionId::new(1).expect("proposition");
    let term = semantic_vocabulary::EvidenceTermId::new(1).expect("term");
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
    assert!(
        terminal_verifier::reconstruct_terminal_obligations(&module)
            .unwrap()
            .obligations()
            .is_empty()
    );
    assert_eq!(
        terminal_verifier::maximum_registered_obligation_id(&module).unwrap(),
        920,
        "fresh allocation includes evidence-backed rows absent from the ordinary proof questions"
    );
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
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
    let proposition = semantic_vocabulary::PropositionId::new(1).expect("proposition");
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
            id: semantic_vocabulary::EvidenceTermId::new(raw).unwrap(),
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
                term: semantic_vocabulary::EvidenceTermId::new(index as u64 + 1).unwrap(),
                output_field: output_field.to_owned(),
            }),
        },
    )
    .collect();
    let producer = |raw| EvidenceProducerProvenance {
        id: EvidenceIdentity::new(raw).unwrap(),
        term: semantic_vocabulary::EvidenceTermId::new(raw).unwrap(),
        conformance_identity: "ConcreteEvidence".to_owned(),
        evidence_trait_identity: "ReadyEvidence".to_owned(),
        rows: Vec::new(),
    };
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
        VerificationError::UnusedEvidenceProducerTerm(
            semantic_vocabulary::EvidenceTermId::new(3).unwrap()
        )
    );
}

#[test]
fn partition_composition_is_available_after_its_exact_successful_call() {
    let (module, goal, obligation) = partition_composition_module();
    validate_module(&module).expect("the partition substitution remains valid replay evidence");
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
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
        qualifications: Default::default(),
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
        qualifications: Default::default(),
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
        static_reach_binding: None,
        suspension_crossing: None,
        id: noncall_operation,
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
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
    module.machines[0].contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
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
    module.machines[0].contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
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
        terminal_psi::CrashRouteGuard::Truth,
        terminal_psi::CrashRouteGuard::Predicate(terminal_psi::CrashPredicateTerm::new(
            Proposition::Equal(ScalarTerm::boolean(true), ScalarTerm::boolean(true)),
        )),
    ];
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::NonCanonicalCrashRouteAlternatives { .. })
    ));

    module.machines[0].contract.crash_routes = vec![
        terminal_psi::CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
        },
        terminal_psi::CrashRouteBucket {
            cause: CrashCause::Trap,
            alternatives: vec![terminal_psi::CrashRouteGuard::Predicate(
                terminal_psi::CrashPredicateTerm::new(Proposition::Equal(
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
    let predicate = terminal_psi::CrashPredicateTerm::new(Proposition::Equal(seven.clone(), seven));
    module.machines[0].contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![terminal_psi::CrashRouteGuard::Predicate(predicate.clone())],
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
    module.machines[0].contract.crash_routes = vec![terminal_psi::CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
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
