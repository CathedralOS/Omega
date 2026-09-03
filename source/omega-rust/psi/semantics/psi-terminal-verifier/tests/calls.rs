use psi_core::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, EvidenceIdentity, EvidenceTermId, IntegerSign,
    IntegerType, MachineId, ObligationId, OperationId, PlaceId, Proposition, PropositionId,
    ScalarTerm, ScalarType, ServiceId, StructuralCaseId, StructuralCaseSubject,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ContractClause, CrashCause, CrashRouteBucket,
    CrashRouteGuard, EvidenceContractLane, EvidenceContractLaneKind, EvidenceInterfaceIdentity,
    EvidenceTermDeclaration, InstallationReachDependency, MachineContract, Operation,
    OperationKind, OperationResult, OutcomeSpecificCallEvidence,
    OutcomeSpecificCallEvidenceValidity, OutcomeSpecificCallResultSubstitution,
    OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificGuard,
    PropositionApplicationIdentity, PropositionDeclaration, PropositionEvidence,
    ProviderCandidateConformance, ProviderUnitRefinement, ProviderUnitSignature,
    ServiceDeclaration, StructuralCaseDeclaration, StructuralMultiplicity,
    StructuralOperationResult, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_verifier::{
    EvidenceProducerProvenance, ModuleError, ObligationEvidence, ProofBundle,
    ReconstructedTerminalObligationOwner, VerificationError, reconstruct_operation_obligations,
    reconstruct_terminal_obligations, validate_module, verify_module,
};

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
        recursive_components: Vec::new(),
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
            recursive_components: Vec::new(),
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
        ModuleError::StructuralResultMustBeOwned(machine_id(1)),
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
        ModuleError::StructuralResultMustBeOwned(machine_id(1)),
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
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(2),
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
        }],
        recursive_components: Vec::new(),
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

#[test]
fn scalar_call_cannot_target_a_unit_machine() {
    let mut module = call_module();
    module.machines[1].result = TerminalMachineResult::Unit;
    module.machines[1].blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(2),
        trivial_affine_discards: Vec::new(),
    };

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::CallTargetReturnsUnit {
            operation: operation_id(2),
            callee: machine_id(2),
        }
    );
}

#[test]
fn scalar_call_rejects_incomplete_or_crash_erasing_shapes() {
    let mut unknown = call_module();
    *call_kind_mut(&mut unknown).0 = machine_id(3);
    assert_eq!(
        validate_module(&unknown).unwrap_err(),
        ModuleError::UnknownCallTarget {
            operation: operation_id(2),
            callee: machine_id(3),
        }
    );

    let mut missing_argument = call_module();
    call_kind_mut(&mut missing_argument).1.clear();
    assert_eq!(
        validate_module(&missing_argument).unwrap_err(),
        ModuleError::CallArgumentArityMismatch {
            operation: operation_id(2),
            expected: 1,
            actual: 0,
        }
    );

    let mut missing_requirement = call_module();
    call_kind_mut(&mut missing_requirement).2.clear();
    assert_eq!(
        validate_module(&missing_requirement).unwrap_err(),
        ModuleError::CallRequirementArityMismatch {
            operation: operation_id(2),
            expected: 1,
            actual: 0,
        }
    );

    let unconditional_trap = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    };
    let mut may_crash = call_module();
    may_crash.machines[1].contract.crash_routes = vec![unconditional_trap.clone()];
    assert_eq!(
        validate_module(&may_crash).unwrap_err(),
        ModuleError::CallCrashContinuationsMismatch {
            operation: operation_id(2),
            callee: machine_id(2),
        }
    );

    call_crash_continuations_mut(&mut may_crash).push(unconditional_trap.clone());
    assert_eq!(
        validate_module(&may_crash).unwrap_err(),
        ModuleError::CallCrashContinuationUncovered {
            operation: operation_id(2),
            cause: CrashCause::Trap,
        }
    );
    may_crash.machines[0].contract.crash_routes = vec![unconditional_trap.clone()];
    validate_module(&may_crash)
        .expect("an explicit covered in-module crash continuation validates");

    let callee_guarded = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(
            psi_terminal::CrashPredicateTerm::new(Proposition::Equal(
                boolean_value(4),
                ScalarTerm::boolean(true),
            )),
        )],
    };
    let caller_guarded = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(
            psi_terminal::CrashPredicateTerm::new(Proposition::Equal(
                boolean_value(1),
                ScalarTerm::boolean(true),
            )),
        )],
    };
    let mut guarded_call = call_module();
    guarded_call.machines[0].contract.crash_routes = vec![unconditional_trap];
    guarded_call.machines[1].contract.crash_routes = vec![callee_guarded.clone()];
    *call_crash_continuations_mut(&mut guarded_call) = vec![caller_guarded];
    validate_module(&guarded_call)
        .expect("guarded call substitutes the callee parameter with the caller-local value");

    *call_crash_continuations_mut(&mut guarded_call) = vec![callee_guarded];
    assert_eq!(
        validate_module(&guarded_call).unwrap_err(),
        ModuleError::CallCrashContinuationsMismatch {
            operation: operation_id(2),
            callee: machine_id(2),
        }
    );

    let mut recursive = call_module();
    let (callee, arguments, requirements) = call_kind_mut(&mut recursive);
    *callee = machine_id(1);
    arguments.clear();
    requirements.clear();
    assert_eq!(
        validate_module(&recursive).unwrap_err(),
        ModuleError::RecursiveCallSliceNotYetSupported(machine_id(1))
    );
}

#[test]
fn scalar_calls_publish_every_reachable_service() {
    let mut module = call_module();
    let service = service_id(1);
    module.services.push(ServiceDeclaration {
        id: service,
        identity: "DebugIo".to_owned(),
        parents: Vec::new(),
    });
    module.machines[1].published_service_ceiling.push(service);
    module.machines[1].blocks[0].operations.push(Operation {
        id: operation_id(3),
        result: psi_terminal::OperationResult::Unit,
        kind: OperationKind::PortWrite {
            service,
            port: 0x3f8,
            value: b'X',
        },
    });

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::OperationServiceOutsidePublishedCeiling {
            operation: operation_id(2),
            service,
        }
    );

    module.machines[0].published_service_ceiling.push(service);
    module.root_service_reach.concrete.push(service);
    validate_module(&module).expect("the caller publishes its scalar callee's service reach");
}

#[test]
fn installation_reach_dependencies_are_exact_closed_service_rows() {
    let mut module = boundary_call_module();
    module.services.push(ServiceDeclaration {
        id: service_id(1),
        identity: "PortIo".into(),
        parents: Vec::new(),
    });
    module.boundary_machines[0].identity = "InterruptCompletion::complete".into();
    module.boundary_machines[0].published_service_ceiling = vec![service_id(1)];
    module.machines[0].published_service_ceiling = vec![service_id(1)];
    module.root_service_reach.installation_dependencies = vec![InstallationReachDependency {
        requirement_identity: "InterruptCompletion::complete".into(),
        upper_bound: vec![service_id(1)],
    }];
    validate_module(&module).expect("canonical installation reach dependency");

    module.machines[0].blocks[0].operations.push(Operation {
        id: operation_id(3),
        result: OperationResult::Unit,
        kind: OperationKind::PortWrite {
            service: service_id(1),
            port: 0x20,
            value: 0x20,
        },
    });
    module.root_service_reach.concrete = vec![service_id(1)];
    validate_module(&module)
        .expect("concrete use survives even when it overlaps an abstract upper bound");

    let mut drifted_bound = module.clone();
    drifted_bound.root_service_reach.installation_dependencies[0]
        .upper_bound
        .clear();
    assert_eq!(
        validate_module(&drifted_bound).unwrap_err(),
        ModuleError::InstallationReachBoundaryMismatch(boundary_id(1))
    );

    let mut unused_dependency = module.clone();
    unused_dependency.machines[0].blocks[0]
        .operations
        .retain(|operation| !matches!(operation.kind, OperationKind::BoundaryCall { .. }));
    assert_eq!(
        validate_module(&unused_dependency).unwrap_err(),
        ModuleError::RootInstallationReachDependenciesMismatch
    );

    module.root_service_reach.concrete.clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::RootConcreteServiceReachMismatch {
            declared: Vec::new(),
            derived: vec![service_id(1)],
        }
    );
    module.root_service_reach.concrete = vec![service_id(1)];

    module.root_service_reach.installation_dependencies[0]
        .requirement_identity
        .clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidInstallationReachDependency(0)
    );
}

#[test]
fn boundary_scalar_arguments_validate_in_declared_order() {
    validate_module(&boundary_call_module())
        .expect("a defined exact-type boundary scalar argument validates");
}

#[test]
fn boundary_scalar_arguments_fail_closed_on_arity_definedness_and_type() {
    let mut arity = boundary_call_module();
    boundary_arguments_mut(&mut arity).clear();
    assert_eq!(
        validate_module(&arity).unwrap_err(),
        ModuleError::BoundaryCallArgumentArityMismatch {
            operation: operation_id(2),
            expected: 1,
            actual: 0,
        }
    );

    let mut undefined = boundary_call_module();
    *boundary_arguments_mut(&mut undefined) = vec![value_id(2)];
    undefined.machines[0].blocks[0].operations.push(Operation {
        id: operation_id(3),
        result: OperationResult::Scalar(boolean_declaration(value_id(2))),
        kind: OperationKind::BooleanConstant { value: false },
    });
    assert_eq!(
        validate_module(&undefined).unwrap_err(),
        ModuleError::BoundaryCallArgumentUsedBeforeDefinition {
            operation: operation_id(2),
            argument: value_id(2),
        }
    );

    let mut unknown = boundary_call_module();
    *boundary_arguments_mut(&mut unknown) = vec![value_id(9)];
    assert_eq!(
        validate_module(&unknown).unwrap_err(),
        ModuleError::UnknownBoundaryCallArgument {
            operation: operation_id(2),
            argument: value_id(9),
        }
    );

    let mut wrong_type = boundary_call_module();
    let integer = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 boundary parameter"),
    );
    wrong_type.boundary_machines[0].scalar_parameters = vec![integer];
    assert_eq!(
        validate_module(&wrong_type).unwrap_err(),
        ModuleError::BoundaryCallArgumentTypeMismatch {
            operation: operation_id(2),
            argument: value_id(1),
            expected: integer,
            actual: ScalarType::Boolean,
        }
    );
}

#[test]
fn provider_candidates_bind_the_exact_scalar_signature() {
    let mut module = provider_candidate_module();
    validate_module(&module).expect("matching scalar provider candidate remains admitted");

    module.machines[1].parameters.clear();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidProviderCandidate {
            boundary: boundary_id(1),
            candidate: machine_id(2),
        }
    );
}

fn boundary_call_module() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: "test::observe".into(),
            attachment: None,
            scalar_parameters: vec![ScalarType::Boolean],
            structural_parameters: Vec::new(),
            result: psi_terminal::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
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
            id: machine_id(1),
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
            entry: block_id(1),
            blocks: vec![Block {
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: operation_id(1),
                        result: OperationResult::Scalar(boolean_declaration(value_id(1))),
                        kind: OperationKind::BooleanConstant { value: true },
                    },
                    Operation {
                        id: operation_id(2),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: vec![value_id(1)],
                            structural_arguments: Vec::new(),
                            completion_receipts: Vec::new(),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn boundary_arguments_mut(module: &mut TerminalModule) -> &mut Vec<ValueId> {
    let OperationKind::BoundaryCall { arguments, .. } =
        &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    arguments
}

fn provider_candidate_module() -> TerminalModule {
    let mut module = boundary_call_module();
    let provider_type = psi_core::StructuralTypeId::new(1).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: provider_type,
        identity: "test::Provider".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    module
        .provider_candidates
        .push(ProviderCandidateConformance {
            boundary: boundary_id(1),
            requirement_identity: "test::observe".into(),
            provider_identity: "test::Provider::observe".into(),
            candidate_identity: "test::provider_candidate".into(),
            candidate: machine_id(2),
            signature: ProviderUnitSignature {
                parameters: Vec::new(),
            },
            refinement: ProviderUnitRefinement {
                positional_parameters: Vec::new(),
                required_domains: Vec::new(),
                realized_service_ceiling: Vec::new(),
            },
        });
    module.machines.push(TerminalMachine {
        id: machine_id(2),
        attachment: Some(provider_type),
        structural_parameters: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        parameters: vec![boolean_declaration(value_id(2))],
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
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
            outcome_specific_ensures: Vec::new(),
        },
    });
    module
}

fn call_module() -> TerminalModule {
    let caller_constant = value_id(1);
    let call_result = value_id(2);
    let caller_result = value_id(3);
    let callee_parameter = value_id(4);
    let callee_result = value_id(5);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
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
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(boolean_declaration(caller_result)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: operation_id(1),
                            result: psi_terminal::OperationResult::Scalar(boolean_declaration(
                                caller_constant,
                            )),
                            kind: OperationKind::BooleanConstant { value: true },
                        },
                        Operation {
                            id: operation_id(2),
                            result: psi_terminal::OperationResult::Scalar(boolean_declaration(
                                call_result,
                            )),
                            kind: OperationKind::Call {
                                callee: machine_id(2),
                                arguments: vec![caller_constant],
                                requirement_obligations: vec![obligation_id(1)],
                                crash_continuations: Vec::new(),
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(1),
                        value: call_result,
                    },
                }],
                contract: MachineContract {
                    id: contract_id(1),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: machine_id(2),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![boolean_declaration(callee_parameter)],
                ranked_scc: None,
                result: TerminalMachineResult::Scalar(boolean_declaration(callee_result)),
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: edge_id(2),
                        value: callee_parameter,
                    },
                }],
                contract: MachineContract {
                    id: contract_id(2),
                    crash_routes: Vec::new(),
                    requires: vec![Proposition::Equal(
                        boolean_value(4),
                        ScalarTerm::boolean(true),
                    )],
                    ensures: vec![ContractClause {
                        obligation: obligation_id(2),
                        proposition: Proposition::Equal(boolean_value(5), boolean_value(4)),
                    }],
                    outcome_specific_ensures: Vec::new(),
                },
            },
        ],
    }
}

fn payloadless_guarded_call_module() -> TerminalModule {
    let result_type = structural_type_id(1);
    let success = structural_case_id(1);
    let failure = structural_case_id(2);
    let call_operation = operation_id(1);
    let constructor_operation = operation_id(2);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: result_type,
            identity: "test::Outcome".into(),
            shape: StructuralTypeShape::Sum {
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
            },
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
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                    place: place_id(2),
                    structural_type: result_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }),
                structural_places: vec![
                    StructuralPlaceDeclaration {
                        id: place_id(1),
                        kind: StructuralPlaceKind::OperationResult {
                            producer: call_operation,
                            structural_type: result_type,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: place_id(2),
                        kind: StructuralPlaceKind::Result,
                    },
                ],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(1),
                blocks: vec![Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: call_operation,
                        result: OperationResult::Structural(StructuralOperationResult {
                            place: place_id(1),
                            structural_type: result_type,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            claims: Vec::new(),
                        }),
                        kind: OperationKind::CallStructural {
                            callee: machine_id(2),
                            structural_arguments: Vec::new(),
                            claim_transfers: Vec::new(),
                            returned_claim_transfers: Vec::new(),
                            requirement_obligations: Vec::new(),
                            crash_continuations: Vec::new(),
                            selected_evidence: Vec::new(),
                        },
                    }],
                    terminator: Terminator::ReturnStructural {
                        edge: edge_id(1),
                        source: place_id(1),
                        returned_claims: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: contract_id(1),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: vec![ContractClause {
                        obligation: obligation_id(2),
                        proposition: Proposition::Truth,
                    }],
                    outcome_specific_ensures: Vec::new(),
                },
            },
            TerminalMachine {
                id: machine_id(2),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                    place: place_id(4),
                    structural_type: result_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }),
                structural_places: vec![
                    StructuralPlaceDeclaration {
                        id: place_id(3),
                        kind: StructuralPlaceKind::OperationResult {
                            producer: constructor_operation,
                            structural_type: result_type,
                        },
                    },
                    StructuralPlaceDeclaration {
                        id: place_id(4),
                        kind: StructuralPlaceKind::Result,
                    },
                ],
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block_id(2),
                blocks: vec![Block {
                    id: block_id(2),
                    parameters: Vec::new(),
                    operations: vec![Operation {
                        id: constructor_operation,
                        result: OperationResult::Structural(StructuralOperationResult {
                            place: place_id(3),
                            structural_type: result_type,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                            claims: Vec::new(),
                        }),
                        kind: OperationKind::EstablishPayloadlessCase {
                            result_case: success,
                        },
                    }],
                    terminator: Terminator::ReturnStructural {
                        edge: edge_id(2),
                        source: place_id(3),
                        returned_claims: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: contract_id(2),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: vec![
                        OutcomeSpecificEnsure {
                            guard: OutcomeSpecificGuard {
                                result_type,
                                result_case: success,
                            },
                            position: 0,
                            obligation: obligation_id(1),
                            proposition: Proposition::Truth,
                            evidence: None,
                        },
                        OutcomeSpecificEnsure {
                            guard: OutcomeSpecificGuard {
                                result_type,
                                result_case: failure,
                            },
                            position: 0,
                            obligation: obligation_id(3),
                            proposition: Proposition::Truth,
                            evidence: None,
                        },
                    ],
                },
            },
        ],
    }
}

fn call_kind_mut(
    module: &mut TerminalModule,
) -> (&mut MachineId, &mut Vec<ValueId>, &mut Vec<ObligationId>) {
    let OperationKind::Call {
        callee,
        arguments,
        requirement_obligations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    (callee, arguments, requirement_obligations)
}

fn call_crash_continuations_mut(module: &mut TerminalModule) -> &mut Vec<CrashRouteBucket> {
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    crash_continuations
}

fn semantic_axiom_evidence(
    obligation: ObligationId,
    proposition: Proposition,
    index: usize,
    identity: u64,
) -> ObligationEvidence {
    ObligationEvidence {
        obligation,
        route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(identity).unwrap(),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode {
                conclusion: proposition,
                rule: ProofRule::SemanticAxiom { index },
            },
        }),
    }
}

fn boolean_declaration(id: ValueId) -> ValueDeclaration {
    ValueDeclaration {
        id,
        scalar_type: ScalarType::Boolean,
    }
}

fn boolean_value(raw: u64) -> ScalarTerm {
    ScalarTerm::value(value_id(raw), ScalarType::Boolean)
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn boundary_id(raw: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn operation_id(raw: u64) -> OperationId {
    OperationId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).unwrap()
}

fn obligation_id(raw: u64) -> ObligationId {
    ObligationId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

fn structural_case_id(raw: u64) -> StructuralCaseId {
    StructuralCaseId::new(raw).unwrap()
}

fn service_id(raw: u64) -> ServiceId {
    ServiceId::new(raw).unwrap()
}
