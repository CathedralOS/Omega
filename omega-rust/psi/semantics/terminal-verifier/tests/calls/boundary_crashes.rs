use super::*;
use terminal_psi::CrashPredicateTerm;
use terminal_verifier::{BoundaryCrashOutcomeError, validate_boundary_crash_outcome};

#[test]
fn reported_boundary_crash_requires_true_same_cause_route() {
    let module = guarded_boundary_module();
    let boundary = &module.boundary_machines[0];
    let permitted = [ScalarTerm::boolean(true), ScalarTerm::boolean(false)];
    validate_boundary_crash_outcome(boundary, &permitted, CrashCause::Trap).unwrap();
    assert_eq!(
        validate_boundary_crash_outcome(boundary, &permitted, CrashCause::Abort),
        Err(BoundaryCrashOutcomeError::UndeclaredCause)
    );
    let denied = [ScalarTerm::boolean(false), ScalarTerm::boolean(true)];
    assert_eq!(
        validate_boundary_crash_outcome(boundary, &denied, CrashCause::Trap),
        Err(BoundaryCrashOutcomeError::GuardFalse)
    );
    let mut unconditional = boundary.clone();
    unconditional.crash_routes[0].alternatives = vec![CrashRouteGuard::Truth];
    validate_boundary_crash_outcome(&unconditional, &denied, CrashCause::Trap).unwrap();
}

#[test]
fn reported_boundary_crash_rejects_nonliteral_or_malformed_actuals() {
    let module = guarded_boundary_module();
    let boundary = &module.boundary_machines[0];
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    for arguments in [
        vec![],
        vec![ScalarTerm::boolean(true)],
        vec![ScalarTerm::boolean(true); 3],
        vec![boolean_value(1), ScalarTerm::boolean(false)],
        vec![
            ScalarTerm::BooleanNot {
                operand: Box::new(ScalarTerm::boolean(false)),
            },
            ScalarTerm::boolean(false),
        ],
        vec![
            ScalarTerm::integer(integer_type, semantic_vocabulary::IntegerValue::Unsigned(1))
                .unwrap(),
            ScalarTerm::boolean(false),
        ],
    ] {
        assert_eq!(
            validate_boundary_crash_outcome(boundary, &arguments, CrashCause::Trap),
            Err(BoundaryCrashOutcomeError::InvalidArguments)
        );
    }
    let mut integer = boundary.clone();
    integer.scalar_parameters = vec![ScalarType::Integer(integer_type)];
    integer.crash_routes[0].alternatives = vec![CrashRouteGuard::Truth];
    assert_eq!(
        validate_boundary_crash_outcome(
            &integer,
            &[ScalarTerm::Integer {
                scalar_type: integer_type,
                value: semantic_vocabulary::IntegerValue::Unsigned(256),
            }],
            CrashCause::Trap,
        ),
        Err(BoundaryCrashOutcomeError::InvalidArguments)
    );
    let mut malformed = boundary.clone();
    malformed.crash_routes = guarded_routes(3);
    assert!(matches!(
        validate_boundary_crash_outcome(
            &malformed,
            &[ScalarTerm::boolean(true), ScalarTerm::boolean(true)],
            CrashCause::Trap
        ),
        Err(BoundaryCrashOutcomeError::InvalidDeclaration(_))
    ));
}

#[test]
fn reported_boundary_crash_scalar_positions_ignore_structural_lane() {
    let module = guarded_boundary_module();
    let mut boundary = module.boundary_machines[0].clone();
    boundary
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: place_id(1),
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Affine,
            access: terminal_psi::StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    boundary.crash_routes = guarded_routes(2);
    validate_boundary_crash_outcome(
        &boundary,
        &[ScalarTerm::boolean(false), ScalarTerm::boolean(true)],
        CrashCause::Trap,
    )
    .unwrap();
}

#[test]
fn reported_boundary_crash_uses_closed_integer_semantics_and_rejects_unsupported_guards() {
    use semantic_vocabulary::{IntegerMathTerm, IntegerValue};
    let mut boundary = boundary_call_module().boundary_machines.remove(0);
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    boundary.scalar_parameters = vec![ScalarType::Integer(integer_type)];
    let formal = ScalarTerm::value(value_id(1), ScalarType::Integer(integer_type));
    let literal = |value| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(value)).unwrap();
    boundary.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::LessThan(formal.clone(), literal(10)),
        ))],
    }];
    validate_boundary_crash_outcome(&boundary, &[literal(9)], CrashCause::Trap).unwrap();
    assert_eq!(
        validate_boundary_crash_outcome(&boundary, &[literal(10)], CrashCause::Trap),
        Err(BoundaryCrashOutcomeError::GuardFalse)
    );
    // A partial exact operation is not a false guard and cannot authorize crash.
    boundary.crash_routes[0].alternatives = vec![CrashRouteGuard::Predicate(
        CrashPredicateTerm::new(Proposition::LessThan(
            ScalarTerm::ExactIntegerDivide {
                scalar_type: integer_type,
                left: Box::new(formal),
                right: Box::new(literal(0)),
            },
            literal(10),
        )),
    )];
    assert_eq!(
        validate_boundary_crash_outcome(&boundary, &[literal(9)], CrashCause::Trap),
        Err(BoundaryCrashOutcomeError::UnsupportedGuard)
    );
    boundary.crash_routes[0].alternatives = vec![CrashRouteGuard::Predicate(
        CrashPredicateTerm::new(Proposition::IntegerMathLessThan(
            IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: value_id(1),
            },
            IntegerMathTerm::literal(IntegerValue::Unsigned(10)),
        )),
    )];
    assert_eq!(
        validate_boundary_crash_outcome(&boundary, &[literal(9)], CrashCause::Trap),
        Err(BoundaryCrashOutcomeError::UnsupportedGuard)
    );
}

#[test]
fn observation_v1_rejects_unrepresentable_boundary_crash_sites() {
    let module = guarded_boundary_module();
    assert_eq!(
        terminal_verifier::reconstruct_terminal_trace_v1_rows(&module),
        Err(
            terminal_verifier::TerminalTraceV1ReconstructionError::UnsupportedBoundaryCrashRoutes(
                boundary_id(1)
            )
        )
    );
}

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
