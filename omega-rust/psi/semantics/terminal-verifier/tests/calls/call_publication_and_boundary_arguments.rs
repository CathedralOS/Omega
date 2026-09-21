use super::{
    boolean_declaration, boolean_value, boundary_arguments_mut, boundary_call_module, boundary_id,
    call_crash_continuations_mut, call_kind_mut, call_module, edge_id, machine_id, operation_id,
    provider_candidate_module, service_id, value_id,
};
use semantic_vocabulary::{IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType};
use terminal_psi::{
    CrashCause, CrashRouteBucket, CrashRouteGuard, InstallationReachDependency, Operation,
    OperationKind, OperationResult, ServiceDeclaration, TerminalMachineResult, Terminator,
};
use terminal_verifier::{ModuleError, validate_module};

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
            terminal_psi::CrashPredicateTerm::new(Proposition::Equal(
                boolean_value(4),
                ScalarTerm::boolean(true),
            )),
        )],
    };
    let caller_guarded = CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(
            terminal_psi::CrashPredicateTerm::new(Proposition::Equal(
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
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation_id(3),
        result: terminal_psi::OperationResult::Unit,
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
        static_reach_binding: None,
        suspension_crossing: None,
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
        static_reach_binding: None,
        suspension_crossing: None,
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
