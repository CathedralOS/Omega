use super::{
    boolean_value, call_module, contract_id, machine_id, obligation_id, operation_id,
    payloadless_guarded_call_module, place_id, semantic_axiom_evidence, structural_case_id,
};
use proof_admission::{AdmissionProfile, EvidenceRoute, PrimitiveJudgment};
use semantic_vocabulary::{
    EvidenceIdentity, EvidenceTermId, Proposition, PropositionId, ScalarTerm, StructuralCaseSubject,
};
use terminal_psi::{
    ContractClause, EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
    EvidenceTermDeclaration, OperationKind, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificEvidence, PropositionApplicationIdentity, PropositionDeclaration,
    PropositionEvidence,
};
use terminal_verifier::{
    EvidenceProducerProvenance, ModuleError, ObligationEvidence, ProofBundle,
    ReconstructedTerminalObligationOwner, VerificationError, reconstruct_operation_obligations,
    reconstruct_terminal_obligations, validate_module, verify_module,
};

#[test]
fn module_reconstruction_keeps_machine_order_and_rejects_duplicate_identities() {
    let mut module = call_module();
    let original = reconstruct_terminal_obligations(&module).expect("original obligations");
    module.machines.reverse();
    let reordered = reconstruct_terminal_obligations(&module).expect("reordered machines");
    let mut expected = original.obligations().to_vec();
    expected.reverse();
    assert_eq!(reordered.obligations(), expected);
    assert_eq!(
        reconstruct_operation_obligations(&module).expect("operation obligations"),
        reconstruct_operation_obligations(&call_module()).expect("original operations"),
    );

    let duplicate = module.machines[0].clone();
    let duplicate_id = duplicate.id;
    module.machines.push(duplicate);
    assert!(matches!(reconstruct_terminal_obligations(&module),
        Err(ModuleError::DuplicateMachine(machine)) if machine == duplicate_id));
    assert!(matches!(reconstruct_operation_obligations(&module),
        Err(ModuleError::DuplicateMachine(machine)) if machine == duplicate_id));
}

#[test]
fn scalar_call_reconstructs_requirements_and_imports_verified_guarantees() {
    let module = call_module();
    let obligations = reconstruct_operation_obligations(&module).expect("call obligations");
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].obligation.id, obligation_id(1));
    assert_eq!(
        obligations[0].obligation.proposition,
        Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true))
    );
    let reconstructed =
        reconstruct_terminal_obligations(&module).expect("complete terminal obligations");
    let [call_requirement, callee_guarantee] = reconstructed.obligations() else {
        panic!("one call requirement and one contract guarantee")
    };
    assert_eq!(
        call_requirement.owner,
        ReconstructedTerminalObligationOwner::CallRequires {
            machine: machine_id(1),
            operation: operation_id(2),
            requirement_position: 0,
        }
    );
    assert_eq!(call_requirement.obligation.id, obligation_id(1));
    assert!(call_requirement.requirements.is_empty());
    assert_eq!(
        callee_guarantee.owner,
        ReconstructedTerminalObligationOwner::ContractEnsures {
            machine: machine_id(2),
            contract: contract_id(2),
            clause_position: 0,
        }
    );
    assert_eq!(callee_guarantee.obligation.id, obligation_id(2));
    assert_eq!(callee_guarantee.requirements.len(), 1);

    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![
            semantic_axiom_evidence(
                obligation_id(1),
                Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true)),
                0,
                1,
            ),
            semantic_axiom_evidence(
                obligation_id(2),
                Proposition::Equal(boolean_value(5), boolean_value(4)),
                0,
                2,
            ),
        ],
    };
    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("callee requirement and guarantee verify");
    assert_eq!(verified.accepted_facts().len(), 2);
    assert_eq!(verified.reconstructed_obligations(), &reconstructed);
}

#[test]
fn payloadless_structural_call_imports_guarded_rows_only_as_case_implications() {
    let module = payloadless_guarded_call_module();
    let call_result = place_id(1);
    let success = structural_case_id(1);
    let imported = Proposition::Implication {
        premise: Box::new(Proposition::StructuralCaseMembership {
            subject: StructuralCaseSubject::new(call_result, Vec::new()),
            case: success,
        }),
        conclusion: Box::new(Proposition::Truth),
    };
    let absent_imported = Proposition::Implication {
        premise: Box::new(Proposition::StructuralCaseMembership {
            subject: StructuralCaseSubject::new(call_result, Vec::new()),
            case: structural_case_id(2),
        }),
        conclusion: Box::new(Proposition::Truth),
    };
    let reconstructed =
        reconstruct_terminal_obligations(&module).expect("guarded call facts reconstruct");
    let caller = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == obligation_id(2))
        .expect("caller implication obligation");
    let rebased = Proposition::Implication {
        premise: Box::new(Proposition::StructuralCaseMembership {
            subject: StructuralCaseSubject::new(place_id(2), Vec::new()),
            case: success,
        }),
        conclusion: Box::new(Proposition::Truth),
    };
    let absent_rebased = Proposition::Implication {
        premise: Box::new(Proposition::StructuralCaseMembership {
            subject: StructuralCaseSubject::new(place_id(2), Vec::new()),
            case: structural_case_id(2),
        }),
        conclusion: Box::new(Proposition::Truth),
    };
    assert_eq!(
        caller.semantic_axioms,
        [imported.clone(), absent_imported, rebased, absent_rebased,],
        "even an unreturned sibling remains a conditional contract fact"
    );
    assert!(
        !caller
            .semantic_axioms
            .contains(&Proposition::StructuralCaseMembership {
                subject: StructuralCaseSubject::new(call_result, Vec::new()),
                case: success,
            }),
        "the call does not claim that one guarded case was selected"
    );

    verify_module(
        &module,
        &ProofBundle {
            crash_obligations: Vec::new(),
            recursive_components: Vec::new(),
            control_cycles: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![
                ObligationEvidence {
                    obligation: obligation_id(2),
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                },
                ObligationEvidence {
                    obligation: obligation_id(1),
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                },
            ],
        },
        &AdmissionProfile::default(),
    )
    .expect("the imported implication remains available without selecting a case");

    let mut widened = module.clone();
    widened.machines[1].contract.ensures = vec![ContractClause {
        obligation: obligation_id(4),
        proposition: Proposition::Truth,
    }];
    assert_eq!(
        validate_module(&widened).unwrap_err(),
        ModuleError::StructuralCallTargetMismatch {
            operation: operation_id(1),
            callee: machine_id(2)
        },
        "the first caller-import rung remains guarded-contract-only"
    );

    let mut forwarded = module;
    let proposition = PropositionId::new(1).unwrap();
    let term = EvidenceTermId::new(1).unwrap();
    let interface = EvidenceInterfaceIdentity {
        trait_identity: "ReadyEvidence".into(),
        arguments: Vec::new(),
        requirements: Vec::new(),
    };
    forwarded.proposition_declarations = vec![PropositionDeclaration {
        id: proposition,
        name: "ready".into(),
        binders: Vec::new(),
        parameter_types: Vec::new(),
        evidence: PropositionEvidence::Witness {
            evidence_type: "ReadyEvidence".into(),
        },
    }];
    forwarded.proposition_applications = vec![PropositionApplicationIdentity {
        id: proposition,
        declaration: proposition,
        binder_arguments: Vec::new(),
        arguments: Vec::new(),
        evidence_interface: Some(interface.clone()),
    }];
    forwarded.evidence_terms = vec![EvidenceTermDeclaration {
        id: term,
        proposition,
        interface,
    }];
    forwarded.evidence_contract_lanes = vec![EvidenceContractLane {
        machine: machine_id(2),
        kind: EvidenceContractLaneKind::Requires,
        position: 0,
        term,
        output_field: None,
    }];
    let row = &mut forwarded.machines[1].contract.outcome_specific_ensures[0];
    row.proposition = Proposition::Atom(proposition);
    row.evidence = Some(OutcomeSpecificEvidence {
        term,
        output_field: "selected".into(),
    });
    assert_eq!(
        validate_module(&forwarded).unwrap_err(),
        ModuleError::StructuralCallTargetMismatch {
            operation: operation_id(1),
            callee: machine_id(2)
        },
        "a bare structural call cannot satisfy a forwarded erased evidence input"
    );
}

#[test]
fn payloadless_structural_call_selects_one_exact_guarded_term_without_inventing_case_facts() {
    let mut module = payloadless_guarded_call_module();
    let proposition = PropositionId::new(1).unwrap();
    let callee_term = EvidenceTermId::new(1).unwrap();
    let output = EvidenceTermId::new(2).unwrap();
    let interface = EvidenceInterfaceIdentity {
        trait_identity: "ReadyEvidence".into(),
        arguments: Vec::new(),
        requirements: Vec::new(),
    };
    module.proposition_declarations = vec![PropositionDeclaration {
        id: proposition,
        name: "ready".into(),
        binders: Vec::new(),
        parameter_types: Vec::new(),
        evidence: PropositionEvidence::Witness {
            evidence_type: "ReadyEvidence".into(),
        },
    }];
    module.proposition_applications = vec![PropositionApplicationIdentity {
        id: proposition,
        declaration: proposition,
        binder_arguments: Vec::new(),
        arguments: Vec::new(),
        evidence_interface: Some(interface.clone()),
    }];
    module.evidence_terms = vec![
        EvidenceTermDeclaration {
            id: callee_term,
            proposition,
            interface: interface.clone(),
        },
        EvidenceTermDeclaration {
            id: output,
            proposition,
            interface: interface.clone(),
        },
    ];
    let (guard, position, callee_obligation) = {
        let row = &mut module.machines[1].contract.outcome_specific_ensures[0];
        row.proposition = Proposition::Atom(proposition);
        row.evidence = Some(OutcomeSpecificEvidence {
            term: callee_term,
            output_field: "selected".into(),
        });
        (row.guard, row.position, row.obligation)
    };
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.push(OutcomeSpecificCallEvidence {
        guard,
        position,
        callee_obligation,
        callee_term,
        output_field: "selected".into(),
        callee_proposition: proposition,
        instantiated_proposition: proposition,
        output,
        result_substitution: None,
        validity: OutcomeSpecificCallEvidenceValidity {
            result: place_id(1),
            proposition_dependencies: vec![place_id(1)],
            evidence_interface: interface.clone(),
            interface_dependencies: Vec::new(),
        },
        expected_use_count: 0,
        uses: Vec::new(),
    });

    validate_module(&module).expect("exact selected guarded call validates");
    let reconstructed = reconstruct_terminal_obligations(&module)
        .expect("selected guarded call reconstructs only conditional facts");
    assert!(reconstructed.obligations().iter().any(|site| {
        site.semantic_axioms.iter().any(|axiom| {
            matches!(
                axiom,
                Proposition::Implication { premise, conclusion }
                    if matches!(premise.as_ref(), Proposition::StructuralCaseMembership { .. })
                        && conclusion.as_ref() == &Proposition::Atom(proposition)
            )
        })
    }));
    assert!(!reconstructed.obligations().iter().any(|site| {
        site.semantic_axioms
            .iter()
            .any(|axiom| axiom == &Proposition::Atom(proposition))
    }));

    let mut substituted = module.clone();
    let instantiated = PropositionId::new(2).unwrap();
    substituted.proposition_declarations[0].parameter_types = vec!["test::Outcome".into()];
    substituted.proposition_applications[0].arguments = vec!["callee-result".into()];
    substituted
        .proposition_applications
        .push(PropositionApplicationIdentity {
            id: instantiated,
            declaration: proposition,
            binder_arguments: Vec::new(),
            arguments: vec!["caller-result".into()],
            evidence_interface: Some(interface.clone()),
        });
    substituted.evidence_terms[1].proposition = instantiated;
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut substituted.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence[0].instantiated_proposition = instantiated;
    selected_evidence[0].result_substitution = Some(OutcomeSpecificCallResultSubstitution {
        argument_position: 0,
        callee_result: place_id(4),
        caller_result: place_id(1),
    });
    selected_evidence[0].validity.interface_dependencies = vec![place_id(1)];
    validate_module(&substituted).expect("the exact whole-result substitution validates");
    let reconstructed = reconstruct_terminal_obligations(&substituted)
        .expect("the exact whole-result substitution reconstructs");
    assert!(reconstructed.obligations().iter().any(|site| {
        site.semantic_axioms.iter().any(|axiom| {
            matches!(
                axiom,
                Proposition::Implication { premise, conclusion }
                    if matches!(premise.as_ref(), Proposition::StructuralCaseMembership { .. })
                        && conclusion.as_ref() == &Proposition::Atom(instantiated)
            )
        })
    }));
    assert!(!reconstructed.obligations().iter().any(|site| {
        site.semantic_axioms.iter().any(|axiom| {
            matches!(axiom, Proposition::Implication { conclusion, .. }
                if conclusion.as_ref() == &Proposition::Atom(proposition))
        })
    }));
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(2),
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        }],
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: vec![EvidenceProducerProvenance {
            id: EvidenceIdentity::new(1).unwrap(),
            term: callee_term,
            conformance_identity: "ConcreteEvidence".into(),
            evidence_trait_identity: "ReadyEvidence".into(),
            rows: Vec::new(),
        }],
    };
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("selected caller term reuses exact callee provenance");

    let mut omitted = module.clone();
    let OperationKind::CallStructural {
        selected_evidence, ..
    } = &mut omitted.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    selected_evidence.clear();
    omitted.evidence_terms.retain(|term| term.id != output);
    validate_module(&omitted).expect("omitting the named selector remains fact-only");
    verify_module(&omitted, &bundle, &AdmissionProfile::default())
        .expect("omitted named selector mints no caller term and keeps callee provenance");
    let mut missing_provenance = bundle;
    missing_provenance.evidence_producers.clear();
    assert_eq!(
        verify_module(&module, &missing_provenance, &AdmissionProfile::default(),).unwrap_err(),
        VerificationError::MissingEvidenceProducer(callee_term)
    );

    let baseline = module;
    let tamper = |mut mutate: Box<dyn FnMut(&mut OutcomeSpecificCallEvidence)>| {
        let mut module = baseline.clone();
        let OperationKind::CallStructural {
            selected_evidence, ..
        } = &mut module.machines[0].blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        let [binding] = selected_evidence.as_mut_slice() else {
            unreachable!()
        };
        mutate(binding);
        assert!(matches!(
            validate_module(&module),
            Err(ModuleError::InvalidOutcomeSpecificCallEvidence {
                caller,
                operation
            }) if caller == machine_id(1) && operation == operation_id(1)
        ));
    };
    tamper(Box::new(|binding| {
        binding.guard.result_case = structural_case_id(2)
    }));
    tamper(Box::new(|binding| binding.position = 1));
    tamper(Box::new(|binding| {
        binding.callee_obligation = obligation_id(3)
    }));
    tamper(Box::new(|binding| binding.output_field = "other".into()));
    tamper(Box::new(|binding| {
        binding.callee_proposition = PropositionId::new(2).unwrap()
    }));
    tamper(Box::new(|binding| {
        binding.instantiated_proposition = PropositionId::new(2).unwrap()
    }));
    tamper(Box::new(|binding| binding.output = binding.callee_term));
    tamper(Box::new(|binding| binding.validity.result = place_id(2)));
    tamper(Box::new(|binding| {
        binding.validity.proposition_dependencies = vec![place_id(2)]
    }));
    tamper(Box::new(|binding| {
        binding.validity.evidence_interface.trait_identity = "OtherEvidence".into()
    }));

    let mut unconditional_leak = baseline;
    unconditional_leak
        .evidence_contract_lanes
        .push(EvidenceContractLane {
            machine: machine_id(1),
            kind: EvidenceContractLaneKind::Requires,
            position: 0,
            term: output,
            output_field: None,
        });
    assert!(matches!(
        validate_module(&unconditional_leak),
        Err(ModuleError::InvalidOutcomeSpecificCallEvidence {
            caller,
            operation
        }) if caller == machine_id(1) && operation == operation_id(1)
    ));
}
