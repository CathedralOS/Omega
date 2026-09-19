use super::{
    AdmissionProfile, CrashCause, CrashRouteBucket, CrashRouteGuard, IntegerSign, IntegerType,
    ModuleError, OperationResult, PlaceId, ProofBundle, Proposition, ScalarTerm, ScalarType,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    TerminalMachineResult, TerminalModule, Terminator, boolean_declaration, boolean_value,
    boundary_id, call_module, edge_id, machine_id, provider_candidate_module, validate_module,
    value_id, verify_module,
};
use terminal_psi::{
    BoundaryMachineResult, BoundaryStructuralResultDeclaration, CrashPredicateTerm,
    ProviderParameterRefinement, ProviderSignatureParameter, StructuralAccess,
    StructuralParameterDeclaration,
};

fn provider_module() -> TerminalModule {
    let mut module = provider_candidate_module();
    module.machines[0].blocks[0].operations.clear();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    module.boundary_machines[0].scalar_parameters[0] = scalar_type;
    module.machines[1].parameters[0].scalar_type = scalar_type;
    let structural_type = StructuralTypeId::new(2).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::OwnedValue".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let parameter = StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let mut boundary_parameter = parameter.clone();
    boundary_parameter.place = PlaceId::new(3).unwrap();
    module.boundary_machines[0]
        .parameter_order
        .push(terminal_psi::BoundaryParameterKind::Structural);
    module.boundary_machines[0].structural_parameters = vec![boundary_parameter];
    module.boundary_machines[0].result =
        BoundaryMachineResult::Structural(BoundaryStructuralResultDeclaration {
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
        });
    module.provider_candidates[0].signature.parameters = vec![ProviderSignatureParameter {
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    module.provider_candidates[0]
        .refinement
        .positional_parameters = vec![ProviderParameterRefinement {
        boundary_index: 0,
        candidate_index: 0,
    }];
    let candidate = &mut module.machines[1];
    candidate.structural_parameters = vec![parameter];
    candidate.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        reference_sources: Vec::new(),
        place: PlaceId::new(2).unwrap(),
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    });
    candidate.structural_places = vec![
        StructuralPlaceDeclaration {
            id: PlaceId::new(1).unwrap(),
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: PlaceId::new(2).unwrap(),
            kind: StructuralPlaceKind::Result,
        },
    ];
    candidate.blocks[0].terminator = Terminator::ReturnStructural {
        edge: edge_id(2),
        source: PlaceId::new(1).unwrap(),
        returned_claims: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module
}

#[test]
fn provider_result_conformance_joins_structural_results_and_scalar_parameters() {
    let module = provider_module();
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let mut missing_scalar = module.clone();
    missing_scalar.machines[1].parameters.clear();
    assert!(matches!(
        validate_module(&missing_scalar),
        Err(ModuleError::InvalidProviderCandidate { .. })
    ));
}

#[test]
fn provider_result_conformance_rejects_signature_and_contract_drift() {
    let baseline = provider_module();
    validate_module(&baseline).unwrap();
    for mutation in 0..7 {
        let mut module = baseline.clone();
        match mutation {
            0 => module.boundary_machines[0].result = BoundaryMachineResult::Unit,
            1 => module.machines[1].result = TerminalMachineResult::Unit,
            2 => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    unreachable!()
                };
                result.structural_type = StructuralTypeId::new(1).unwrap();
            }
            3 | 4 => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    unreachable!()
                };
                result.multiplicity = if mutation == 3 {
                    StructuralMultiplicity::Linear
                } else {
                    StructuralMultiplicity::Unrestricted
                };
                let BoundaryMachineResult::Structural(required) =
                    &mut module.boundary_machines[0].result
                else {
                    unreachable!()
                };
                required.multiplicity = result.multiplicity;
            }
            5 => {
                module.machines[1].contract.requires =
                    call_module().machines[1].contract.requires.clone()
            }
            6 => {
                module.machines[1].contract.ensures =
                    call_module().machines[1].contract.ensures.clone()
            }
            _ => unreachable!(),
        }
        assert_eq!(
            validate_module(&module).unwrap_err(),
            ModuleError::InvalidProviderCandidate {
                boundary: boundary_id(1),
                candidate: machine_id(2)
            },
            "mutation {mutation}"
        );
    }
}

/// A scalar-result provider candidate: the caller exercises the boundary
/// through an ordinary scalar-result `BoundaryCall`, and the candidate
/// returns its own parameter.
fn scalar_provider_module() -> TerminalModule {
    let mut module = provider_candidate_module();
    module.boundary_machines[0].result = BoundaryMachineResult::Scalar(ScalarType::Boolean);
    // The boundary call's declared result is the caller-local value the
    // provider's successful scalar return commits to.
    module.machines[0].blocks[0].operations[1].result =
        OperationResult::Scalar(boolean_declaration(value_id(4)));
    let candidate = &mut module.machines[1];
    candidate.result = TerminalMachineResult::Scalar(boolean_declaration(value_id(3)));
    candidate.blocks[0].terminator = Terminator::Return {
        edge: edge_id(2),
        value: value_id(2),
        cleanup_actions: Vec::new(),
    };
    module
}

#[test]
fn provider_result_conformance_admits_scalar_results() {
    let module = scalar_provider_module();
    validate_module(&module).expect("scalar provider result conforms");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("scalar provider row admits through verification");
    // Codec representation validation and artifact admission share this gate.
    terminal_verifier::validate_module_representation(&module)
        .expect("scalar provider row admits through representation validation");
}

#[test]
fn provider_result_conformance_rejects_scalar_result_drift() {
    let baseline = scalar_provider_module();
    validate_module(&baseline).unwrap();
    for mutation in 0..6 {
        let mut module = baseline.clone();
        match mutation {
            // Result-kind drift in either direction.
            0 => module.boundary_machines[0].result = BoundaryMachineResult::Unit,
            1 => module.machines[1].result = TerminalMachineResult::Unit,
            // The boundary publishes the exact payload type.
            2 => {
                let TerminalMachineResult::Scalar(result) = &mut module.machines[1].result else {
                    unreachable!()
                };
                result.scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
            }
            3 => {
                module.boundary_machines[0].result = BoundaryMachineResult::Scalar(
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
                );
            }
            // Contract surfaces the provider relation does not carry.
            4 => {
                module.machines[1].contract.requires =
                    call_module().machines[1].contract.requires.clone()
            }
            5 => {
                module.machines[1].contract.ensures =
                    call_module().machines[1].contract.ensures.clone()
            }
            _ => unreachable!(),
        }
        assert_eq!(
            validate_module(&module).unwrap_err(),
            ModuleError::InvalidProviderCandidate {
                boundary: boundary_id(1),
                candidate: machine_id(2)
            },
            "mutation {mutation}"
        );
    }
}

#[test]
fn provider_crash_refinement_applies_to_scalar_results() {
    let mut module = scalar_provider_module();
    // The caller does not exercise this boundary; provider conformance is the
    // subject under test.
    module.machines[0].blocks[0].operations.clear();
    // The ceiling guard belongs to the boundary's scalar telescope: the first
    // formal is value 1. The candidate contract names its own parameter,
    // value 2; positional substitution joins the two namespaces.
    module.boundary_machines[0].crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true)),
        ))],
    }];
    module.machines[1].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(boolean_value(2), ScalarTerm::boolean(true)),
        ))],
    }];
    validate_module(&module).expect("scalar provider guarded crash refines the ceiling");
    module.machines[1].contract.crash_routes[0].cause = CrashCause::Abort;
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidProviderCandidate {
            boundary: boundary_id(1),
            candidate: machine_id(2)
        }
    );
}

fn crashing_provider_module() -> TerminalModule {
    let mut module = provider_candidate_module();
    // The caller does not exercise this boundary; provider conformance is the
    // subject under test.
    module.machines[0].blocks[0].operations.clear();
    // The ceiling guard belongs to the boundary's scalar telescope: the first
    // formal is value 1. The candidate contract names its own parameter,
    // value 2; positional substitution joins the two namespaces.
    module.boundary_machines[0].crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true)),
        ))],
    }];
    module.machines[1].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
            Proposition::Equal(boolean_value(2), ScalarTerm::boolean(true)),
        ))],
    }];
    module
}

#[test]
fn provider_crash_refinement_admits_candidate_routes_covered_by_the_ceiling() {
    let module = crashing_provider_module();
    validate_module(&module).expect("guarded provider route refines the identical ceiling");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("provider crash coverage is independently reconstructed");
}

#[test]
fn provider_crash_refinement_lets_an_unconditional_ceiling_cover_guarded_routes() {
    let mut module = crashing_provider_module();
    module.boundary_machines[0].crash_routes[0].alternatives = vec![CrashRouteGuard::Truth];
    validate_module(&module).expect("an unconditional ceiling covers same-cause guards");
    module.machines[1].contract.crash_routes[0].alternatives = vec![CrashRouteGuard::Truth];
    validate_module(&module).expect("an unconditional candidate refines an unconditional ceiling");
}

#[test]
fn provider_crash_refinement_rejects_routes_outside_the_boundary_ceiling() {
    let baseline = crashing_provider_module();
    validate_module(&baseline).unwrap();
    for mutation in 0..4 {
        let mut module = baseline.clone();
        match mutation {
            // A cause the ceiling does not publish.
            0 => module.machines[1].contract.crash_routes[0].cause = CrashCause::Abort,
            // A guarded alternative the ceiling does not list after
            // positional substitution.
            1 => {
                module.machines[1].contract.crash_routes[0].alternatives =
                    vec![CrashRouteGuard::Predicate(CrashPredicateTerm::new(
                        Proposition::Equal(boolean_value(2), ScalarTerm::boolean(false)),
                    ))]
            }
            // An unconditional route needs an unconditional ceiling.
            2 => {
                module.machines[1].contract.crash_routes[0].alternatives =
                    vec![CrashRouteGuard::Truth]
            }
            // A crash-free ceiling cannot publish a crashing provider.
            3 => module.boundary_machines[0].crash_routes.clear(),
            _ => unreachable!(),
        }
        assert_eq!(
            validate_module(&module).unwrap_err(),
            ModuleError::InvalidProviderCandidate {
                boundary: boundary_id(1),
                candidate: machine_id(2)
            },
            "mutation {mutation}"
        );
    }
}

#[test]
fn provider_crash_refinement_applies_to_structural_results() {
    use semantic_vocabulary::IntegerValue;
    let mut module = provider_module();
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let guard = |value: u64| {
        CrashRouteGuard::Predicate(CrashPredicateTerm::new(Proposition::Equal(
            ScalarTerm::value(value_id(value), ScalarType::Integer(integer_type)),
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(7)).unwrap(),
        )))
    };
    module.boundary_machines[0].crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![guard(1)],
    }];
    module.machines[1].contract.crash_routes = vec![CrashRouteBucket {
        cause: CrashCause::Abort,
        alternatives: vec![guard(2)],
    }];
    validate_module(&module).expect("structural provider guarded crash refines the ceiling");
    module.machines[1].contract.crash_routes[0].cause = CrashCause::Trap;
    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::InvalidProviderCandidate {
            boundary: boundary_id(1),
            candidate: machine_id(2)
        }
    );
}
