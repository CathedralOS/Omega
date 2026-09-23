use super::{
    AdmissionProfile, CrashCause, CrashRouteBucket, CrashRouteGuard, IntegerSign, IntegerType,
    ModuleError, OperationResult, PlaceId, ProofBundle, Proposition, ScalarTerm, ScalarType,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    TerminalMachineResult, TerminalModule, Terminator, block_id, boolean_declaration,
    boolean_value, boundary_id, call_module, edge_id, machine_id, provider_candidate_module,
    validate_module, value_id, verify_module,
};
use semantic_vocabulary::{
    ClaimId, ContentAlgebra, ContentAlgebraKind, ContentDomainId, ContentProjectionExpression,
    ContentProjectionIdentity, ContentProjectionScalar, StructuralDomainId,
};
use terminal_psi::{
    BindingRelevance, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    CrashPredicateTerm, EntryClaim, ProgramLocalRootIntroductionSchema,
    ProviderParameterRefinement, ProviderSignatureParameter, StructuralAccess,
    StructuralDomainRequirement, StructuralFieldDeclaration, StructuralFieldType,
    StructuralParameterDeclaration, StructuralPathQualification, StructuralPathSegment,
    program_local_root_introduction_compatibility_report_identity,
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

/// A claimed linear provider row: the boundary publishes `in Live` on its
/// parameter (mirrored onto the conformance row as `required_domains`) and on
/// its linear result, and the candidate binds the parameter's entry claim at
/// its root and returns it — the registration-ledger shape provider_result
/// admits. The member field and second domain exist for the drift pins.
fn linear_provider_module() -> TerminalModule {
    let mut module = provider_module();
    let live = StructuralDomainId::new(1).unwrap();
    let member_domain = StructuralDomainId::new(2).unwrap();
    let carrier = StructuralTypeId::new(2).unwrap();
    let member_type = StructuralTypeId::new(3).unwrap();
    module.structural_types[1] = StructuralTypeDeclaration {
        id: carrier,
        identity: "test::OwnedValue".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                identity: "member".into(),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Structural(member_type),
            }],
        },
    };
    module.structural_types.push(StructuralTypeDeclaration {
        id: member_type,
        identity: "test::Member".into(),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "A".into(),
    };
    let expression =
        ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural("1".into()));
    module
        .structural_domains
        .push(terminal_psi::StructuralDomainDeclaration {
            id: live,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: "test::Live".into(),
            carrier,
            content_projection: Some(terminal_psi::StructuralContentProjection {
                identity: ContentProjectionIdentity {
                    domain: ContentDomainId::new(1).unwrap(),
                    projection_report_fingerprint:
                        language_semantics::content::terminal_projection_report_fingerprint(
                            &algebra,
                            &expression,
                        ),
                },
                algebra,
                expression,
            }),
            establishment_routes: Vec::new(),
        });
    module
        .structural_domains
        .push(terminal_psi::StructuralDomainDeclaration {
            id: member_domain,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(2).unwrap(),
            identity: "test::MemberLive".into(),
            carrier: member_type,
            content_projection: None,
            establishment_routes: Vec::new(),
        });
    for parameter in [
        &mut module.boundary_machines[0].structural_parameters[0],
        &mut module.machines[1].structural_parameters[0],
    ] {
        parameter.multiplicity = StructuralMultiplicity::Linear;
        parameter.qualifications = vec![live];
    }
    module.boundary_machines[0].requires = vec![StructuralDomainRequirement {
        argument_index: 0,
        domain: live,
    }];
    let BoundaryMachineResult::Structural(required) = &mut module.boundary_machines[0].result
    else {
        unreachable!()
    };
    required.multiplicity = StructuralMultiplicity::Linear;
    required.qualifications = vec![live];
    let row = &mut module.provider_candidates[0];
    row.signature.parameters[0].multiplicity = StructuralMultiplicity::Linear;
    row.signature.parameters[0].qualifications = vec![live];
    row.refinement.required_domains = module.boundary_machines[0].requires.clone();
    let candidate = &mut module.machines[1];
    let TerminalMachineResult::Structural(result) = &mut candidate.result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Linear;
    result.qualifications = vec![live];
    let argument_claim = ClaimId::new(1).unwrap();
    candidate.entry_claims = vec![EntryClaim {
        claim: argument_claim,
        input: PlaceId::new(1).unwrap(),
        path: Vec::new(),
    }];
    let Terminator::ReturnStructural {
        returned_claims, ..
    } = &mut candidate.blocks[0].terminator
    else {
        unreachable!()
    };
    *returned_claims = vec![argument_claim];
    module
}

#[test]
fn provider_result_conformance_admits_claimed_linear_results() {
    let module = linear_provider_module();
    validate_module(&module).expect("claimed linear provider result conforms");
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("claimed linear provider row admits through verification");
}

#[test]
fn provider_result_conformance_rejects_claim_and_authority_drift() {
    let baseline = linear_provider_module();
    validate_module(&baseline).unwrap();
    for mutation in 0..7 {
        let mut module = baseline.clone();
        match mutation {
            // A candidate may not declare a domain the boundary does not
            // promise. `75b27af297` relaxed the provider-result match from
            // equality to this containment, because the boundary's
            // `qualifications` fold the requirement's own `ensures` mints,
            // which are replayed on the caller at the call site rather than
            // carried by the candidate's signature. Containment is what is
            // left to control, so drift is spelled as an EXTRA domain.
            0 => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    unreachable!()
                };
                result
                    .qualifications
                    .push(StructuralDomainId::new(2).unwrap());
            }
            // A payload-path qualification has no boundary-side declaration to
            // join: projected qualifications stay outside installed-provider
            // results until the boundary grammar can publish them.
            1 => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    unreachable!()
                };
                result.projected_qualifications = vec![StructuralPathQualification {
                    path: vec![StructuralPathSegment::Field("member".into())],
                    domain: StructuralDomainId::new(2).unwrap(),
                }];
            }
            // Entry claims bind only the parameter root: the caller's claims
            // transfer in by position, not into a path beneath it.
            2 => {
                module.machines[1].entry_claims[0].path =
                    vec![StructuralPathSegment::Field("member".into())]
            }
            // The conformance row must mirror the boundary's requirement rows.
            3 => module.provider_candidates[0]
                .refinement
                .required_domains
                .clear(),
            // A boundary-declared root introduction has no candidate-side
            // evidence model: the provider cannot perform the introduction an
            // installed boundary contract promises.
            4 => {
                let live = StructuralDomainId::new(1).unwrap();
                let domain = module
                    .structural_domains
                    .iter()
                    .find(|domain| domain.id == live)
                    .expect("live domain")
                    .clone();
                let projection = domain
                    .content_projection
                    .as_ref()
                    .expect("live domain carries its owner projection")
                    .clone();
                let mut schema = ProgramLocalRootIntroductionSchema {
                    argument_index: 0,
                    source_parameter_position: 0,
                    qualification: live,
                    carrier: StructuralTypeId::new(2).unwrap(),
                    projection: projection.identity,
                    algebra: projection.algebra,
                    capacity: projection.expression,
                    compatibility_report_identity: 0,
                };
                schema.compatibility_report_identity =
                    program_local_root_introduction_compatibility_report_identity(
                        &module.boundary_machines[0].identity,
                        &domain.identity,
                        "test::OwnedValue",
                        &schema,
                    );
                module.boundary_machines[0].program_local_root_introductions = vec![schema];
            }
            // The requirement the boundary publishes must still hold against
            // the candidate's signature positions.
            5 => {
                module.provider_candidates[0].refinement.required_domains =
                    vec![StructuralDomainRequirement {
                        argument_index: 0,
                        domain: StructuralDomainId::new(2).unwrap(),
                    }]
            }
            // The other direction of that containment is no longer an invalid
            // candidate: a result declaring FEWER domains than the boundary
            // promises passes the provider match and is caught where the
            // signature itself is compared. Pinned so the rule is stated from
            // both sides, and so this shape cannot quietly stop rejecting.
            6 => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    unreachable!()
                };
                result.qualifications.clear();
            }
            _ => unreachable!(),
        }
        let expected = if mutation == 6 {
            ModuleError::StructuralReturnSignatureMismatch {
                machine: machine_id(2),
                block: block_id(2),
            }
        } else {
            ModuleError::InvalidProviderCandidate {
                boundary: boundary_id(1),
                candidate: machine_id(2),
            }
        };
        assert_eq!(
            validate_module(&module).unwrap_err(),
            expected,
            "mutation {mutation}"
        );
    }
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
            3 => {
                // Candidate drifts from the boundary's Affine result: Linear
                // custody is admissible only when the boundary declares it.
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    unreachable!()
                };
                result.multiplicity = StructuralMultiplicity::Linear;
            }
            4 => {
                let TerminalMachineResult::Structural(result) = &mut module.machines[1].result
                else {
                    unreachable!()
                };
                result.multiplicity = StructuralMultiplicity::Unrestricted;
                let BoundaryMachineResult::Structural(required) =
                    &mut module.boundary_machines[0].result
                else {
                    unreachable!()
                };
                // Mutation 3 widens the requirement to linear while the
                // candidate stays affine — a supported multiplicity, but the
                // candidate must match it. Mutation 4 widens the requirement
                // to unrestricted, which has no installed-provider leg at all.
                required.multiplicity = if mutation == 3 {
                    StructuralMultiplicity::Linear
                } else {
                    StructuralMultiplicity::Unrestricted
                };
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
