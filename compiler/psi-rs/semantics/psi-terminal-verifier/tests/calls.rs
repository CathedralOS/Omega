use psi_core::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType,
    MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ServiceId, ValueId,
};
use psi_proof_kernel::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ContractClause, CrashCause, CrashRouteBucket,
    CrashRouteGuard, InstallationReachDependency, MachineContract, Operation, OperationKind,
    OperationResult, ProviderCandidateConformance, ProviderUnitRefinement, ProviderUnitSignature,
    ServiceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, reconstruct_operation_obligations,
    validate_module, verify_module,
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

    let bundle = ProofBundle {
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
    validate_module(&module).expect("the caller publishes its scalar callee's service reach");
}

#[test]
fn installation_reach_dependencies_are_exact_closed_service_rows() {
    let mut module = call_module();
    module.services.push(ServiceDeclaration {
        id: service_id(1),
        identity: "PortIo".into(),
        parents: Vec::new(),
    });
    module.root_service_reach.concrete = vec![service_id(1)];
    module.root_service_reach.installation_dependencies = vec![InstallationReachDependency {
        requirement_identity: "InterruptCompletion::complete".into(),
        upper_bound: vec![service_id(1)],
    }];
    validate_module(&module).expect("canonical installation reach dependency");

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
fn provider_candidates_remain_a_zero_scalar_argument_subset() {
    let mut module = provider_candidate_module();
    validate_module(&module).expect("zero-scalar provider candidate remains admitted");

    module.boundary_machines[0].scalar_parameters = vec![ScalarType::Boolean];
    *boundary_arguments_mut(&mut module) = vec![value_id(1)];
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
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: "test::observe".into(),
            attachment: None,
            scalar_parameters: vec![ScalarType::Boolean],
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        evidence_package_invocations: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
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
                            requirement_obligations: Vec::new(),
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
    module.boundary_machines[0].scalar_parameters.clear();
    boundary_arguments_mut(&mut module).clear();
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
        parameters: Vec::new(),
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
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        evidence_package_invocations: Vec::new(),
        closed_conformance_applications: Vec::new(),
        machines: vec![
            TerminalMachine {
                id: machine_id(1),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
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
                },
            },
            TerminalMachine {
                id: machine_id(2),
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: vec![boolean_declaration(callee_parameter)],
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

fn service_id(raw: u64) -> ServiceId {
    ServiceId::new(raw).unwrap()
}
