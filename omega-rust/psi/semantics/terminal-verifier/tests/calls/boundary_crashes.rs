use super::*;
use terminal_psi::CrashPredicateTerm;

fn guarded_routes(value: u64) -> Vec<CrashRouteBucket> {
    vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(boolean_value(value), ScalarTerm::boolean(true)),
        ))],
    }]
}

fn guarded_boundary_module() -> TerminalModule {
    let mut module = boundary_call_module();
    module.boundary_machines[0].scalar_parameters = vec![ScalarType::Boolean; 2];
    module.boundary_machines[0].crash_routes = guarded_routes(1);
    let caller = &mut module.machines[0];
    caller.parameters = vec![
        boolean_declaration(value_id(1)),
        boolean_declaration(value_id(2)),
    ];
    caller.blocks[0].operations.remove(0);
    let OperationKind::BoundaryCall { arguments, .. } = &mut caller.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *arguments = vec![value_id(2), value_id(1)];
    caller.contract.crash_routes = guarded_routes(2);
    module
}

#[test]
fn boundary_crash_contract_substitutes_positional_formals_simultaneously() {
    let module = guarded_boundary_module();
    validate_module(&module).expect("swapped actuals do not recursively substitute each other");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("boundary crash coverage is independently reconstructed");
}

#[test]
fn boundary_crash_contract_rejects_actual_and_cause_drift_without_new_evidence() {
    let module = guarded_boundary_module();
    for mutation in 0..3 {
        let mut changed = module.clone();
        match mutation {
            0 => {
                let OperationKind::BoundaryCall { arguments, .. } =
                    &mut changed.machines[0].blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                arguments.swap(0, 1);
            }
            1 => changed.boundary_machines[0].crash_routes[0].cause = CrashCause::Abort,
            2 => changed.machines[0].contract.crash_routes.clear(),
            _ => unreachable!(),
        }
        assert!(matches!(
            verify_module(
                &changed,
                &ProofBundle::default(),
                &AdmissionProfile::default()
            ),
            Err(VerificationError::Module(
                ModuleError::CallCrashContinuationUncovered { .. }
            ))
        ));
    }
}

#[test]
fn boundary_crash_contract_uses_only_its_own_typed_formals() {
    let mut unknown = guarded_boundary_module();
    // A real caller value cannot be used as a declaration-local third formal.
    unknown.machines[0]
        .parameters
        .push(boolean_declaration(value_id(3)));
    unknown.boundary_machines[0].crash_routes = guarded_routes(3);
    assert!(matches!(
        validate_module(&unknown),
        Err(ModuleError::MalformedProposition(_))
    ));

    let mut wrong_type = guarded_boundary_module();
    wrong_type.boundary_machines[0].scalar_parameters[0] =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
    assert!(matches!(
        validate_module(&wrong_type),
        Err(ModuleError::MalformedProposition(_))
    ));

    let mut opaque = guarded_boundary_module();
    opaque.boundary_machines[0].crash_routes[0].alternatives = vec![CrashRouteGuard::Predicate(
        CrashPredicateTerm::new(Proposition::Atom(PropositionId::new(1).unwrap())),
    )];
    assert_eq!(
        validate_module(&opaque).unwrap_err(),
        ModuleError::UnsupportedBoundaryCrashPredicate(boundary_id(1))
    );

    let mut structural = guarded_boundary_module();
    structural.boundary_machines[0].crash_routes[0].alternatives =
        vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(
                ScalarTerm::boolean_field(
                    place_id(1),
                    semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                ),
                ScalarTerm::boolean(true),
            ),
        ))];
    assert_eq!(
        validate_module(&structural).unwrap_err(),
        ModuleError::UnsupportedBoundaryCrashPredicate(boundary_id(1))
    );
}

#[test]
fn boundary_crash_contract_rejects_noncanonical_routes() {
    let module = guarded_boundary_module();
    for mutation in 0..6 {
        let mut changed = module.clone();
        let routes = &mut changed.boundary_machines[0].crash_routes;
        match mutation {
            0 => routes.push(routes[0].clone()),
            1 => {
                let duplicate = routes[0].alternatives[0].clone();
                routes[0].alternatives.push(duplicate);
            }
            2 => routes[0].alternatives.clear(),
            3 => routes[0].alternatives.insert(0, CrashRouteGuard::Truth),
            4 => {
                routes[0].alternatives = vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                    Proposition::Truth,
                ))]
            }
            5 => {
                routes[0].alternatives = vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                    Proposition::Falsehood,
                ))]
            }
            _ => unreachable!(),
        }
        assert_eq!(
            validate_module(&changed).unwrap_err(),
            ModuleError::NonCanonicalBoundaryCrashRoutes(boundary_id(1))
        );
    }
}

#[test]
fn noncrashing_provider_does_not_narrow_boundary_crash_permission() {
    let mut module = provider_candidate_module();
    module.boundary_machines[0].crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::CallCrashContinuationUncovered { .. })
    ));
    module.machines[0].contract.crash_routes = module.boundary_machines[0].crash_routes.clone();
    validate_module(&module).expect("crash-free provider refines wider boundary permission");

    module.machines[1].contract.crash_routes = module.boundary_machines[0].crash_routes.clone();
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidProviderCandidate {
            boundary: boundary_id(1),
            candidate: machine_id(2),
        }
    );
}
