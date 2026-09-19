use super::{
    AdmissionProfile, Block, BlockId, CertificateEnvelope, ContractId, EdgeId, EvidenceIdentity,
    EvidenceRoute, IntegerSign, IntegerType, MachineId, ModuleError, ObligationEvidence,
    ObligationId, Operation, OperationId, OperationKind, OperationResult, PlaceId, ProofBundle,
    ProofNode, ProofRule, ProofSystemMarker, Proposition, ScalarTerm, ScalarType, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, ValueId, id,
    unranked_scalar_cycle, validate_module, validate_module_for_interpretation,
    verify_module_for_interpretation,
};
use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
use terminal_psi::{
    BindingRelevance, ClaimTransfer, ContractClause, CrashCause, CrashRouteBucket, CrashRouteGuard,
    ScalarCaseField, StructuralArgument, StructuralCaseDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralOperationResult, StructuralResultDeclaration,
};

fn scalar_call_cycle() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let mut callee = module.machines[0].clone();
    callee.id = id(2, MachineId::new);
    callee.contract.id = id(2, ContractId::new);
    callee.parameters[0].id = id(20, ValueId::new);
    callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: id(21, ValueId::new),
        scalar_type: ScalarType::Boolean,
    });
    callee.blocks.truncate(1);
    callee.entry = id(100, BlockId::new);
    callee.blocks[0].id = callee.entry;
    callee.blocks[0].terminator = Terminator::Return {
        edge: id(20, EdgeId::new),
        value: id(20, ValueId::new),
        cleanup_actions: Vec::new(),
    };
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: id(30, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(30, ValueId::new),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::Call {
            callee: callee.id,
            arguments: vec![id(10, ValueId::new)],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    });
    module.machines.push(callee);
    module
}

fn verifies(module: &TerminalModule, proofs: &ProofBundle) -> bool {
    verify_module_for_interpretation(module, proofs, &AdmissionProfile::default()).is_ok()
}

fn assert_verifies(module: &TerminalModule, proofs: &ProofBundle) {
    verify_module_for_interpretation(module, proofs, &AdmissionProfile::default())
        .expect("canonical cyclic scalar-call fixture verifies");
}

fn boolean_requirement(value: u64) -> Proposition {
    let mut terms = [
        ScalarTerm::value(id(value, ValueId::new), ScalarType::Boolean),
        ScalarTerm::boolean(true),
    ];
    terms.sort();
    Proposition::Equal(terms[0].clone(), terms[1].clone())
}

fn evidence(obligation: u64, conclusion: Proposition, rule: ProofRule) -> ProofBundle {
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: id(obligation, ObligationId::new),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(obligation, EvidenceIdentity::new),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode { conclusion, rule },
            }),
        }],
        ..ProofBundle::default()
    }
}

#[test]
fn cyclic_scalar_call_keeps_ordinary_result_and_full_graph_validation() {
    let module = scalar_call_cycle();
    assert_verifies(&module, &ProofBundle::default());
    let mut reordered = module.clone();
    reordered.machines[0].blocks.reverse();
    assert_verifies(&reordered, &ProofBundle::default());
}

#[test]
fn cyclic_scalar_call_rejects_unknown_target_wrong_arguments_and_result() {
    let module = scalar_call_cycle();
    assert_verifies(&module, &ProofBundle::default());
    for mutation in 0..6 {
        let mut changed = module.clone();
        let operation = &mut changed.machines[0].blocks[0].operations[0];
        let OperationKind::Call {
            callee, arguments, ..
        } = &mut operation.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => *callee = id(999, MachineId::new),
            1 => arguments.clear(),
            2 => arguments[0] = id(999, ValueId::new),
            3 => arguments[0] = id(30, ValueId::new),
            4 => operation.result = OperationResult::Unit,
            5 => {
                operation.result = OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id(30, ValueId::new),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    ),
                })
            }
            _ => unreachable!(),
        }
        assert!(
            !verifies(&changed, &ProofBundle::default()),
            "mutation {mutation}"
        );
    }
}

#[test]
fn cyclic_scalar_call_requires_independent_invocation_evidence() {
    let mut module = scalar_call_cycle();
    module.machines[0].contract.requires = vec![boolean_requirement(10)];
    module.machines[1].contract.requires = vec![boolean_requirement(20)];
    let OperationKind::Call {
        requirement_obligations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    requirement_obligations.push(id(40, ObligationId::new));
    let proof = evidence(
        40,
        boolean_requirement(10),
        ProofRule::Assumption { index: 0 },
    );
    assert_verifies(&module, &proof);
    assert!(!verifies(&module, &ProofBundle::default()));
    module.machines[0].contract.requires.clear();
    assert!(!verifies(&module, &proof));
}

#[test]
fn cyclic_scalar_call_preserves_surviving_crash_continuations() {
    let mut module = scalar_call_cycle();
    let routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    module.machines[0].contract.crash_routes = routes.clone();
    module.machines[1].contract.crash_routes = routes.clone();
    module.machines[1].blocks[0].terminator = Terminator::Crash {
        edge: id(20, EdgeId::new),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *crash_continuations = routes;
    assert_verifies(&module, &ProofBundle::default());
    let mut omitted = module.clone();
    let OperationKind::Call {
        crash_continuations,
        ..
    } = &mut omitted.machines[0].blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    crash_continuations.clear();
    assert!(!verifies(&omitted, &ProofBundle::default()));
    module.machines[0].contract.crash_routes.clear();
    assert!(!verifies(&module, &ProofBundle::default()));
}

/// A cyclic caller whose loop block hosts a structural-argument call with a
/// scalar result: the argument borrows a primitive local re-established every
/// iteration, and the callee returns its scalar parameter.
fn structural_scalar_call_cycle() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let mut callee = module.machines[0].clone();
    callee.id = id(2, MachineId::new);
    callee.contract.id = id(2, ContractId::new);
    callee.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: id(20, ValueId::new),
        scalar_type: ScalarType::Boolean,
    }];
    callee.structural_parameters = vec![StructuralParameterDeclaration {
        place: id(100, PlaceId::new),
        position: 0,
        is_self: false,
        structural_type: id(1, StructuralTypeId::new),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    callee.structural_places = vec![StructuralPlaceDeclaration {
        id: id(100, PlaceId::new),
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    callee.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: id(21, ValueId::new),
        scalar_type: ScalarType::Boolean,
    });
    callee.entry = id(100, BlockId::new);
    callee.blocks = vec![Block {
        structural_parameters: Vec::new(),
        id: id(100, BlockId::new),
        parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::Return {
            edge: id(20, EdgeId::new),
            value: id(20, ValueId::new),
            cleanup_actions: Vec::new(),
        },
    }];
    module.structural_types.push(StructuralTypeDeclaration {
        id: id(1, StructuralTypeId::new),
        identity: "test::FlagCell".into(),
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
    });
    let machine = &mut module.machines[0];
    machine.structural_places = vec![StructuralPlaceDeclaration {
        id: id(1, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(20, OperationId::new),
            structural_type: id(1, StructuralTypeId::new),
        },
    }];
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: id(20, OperationId::new),
            result: OperationResult::Structural(StructuralOperationResult {
                place: id(1, PlaceId::new),
                structural_type: id(1, StructuralTypeId::new),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishPrimitiveLocal {
                value: id(10, ValueId::new),
            },
        },
        Operation {
            static_reach_binding: None,
            id: id(30, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(30, ValueId::new),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::CallStructuralScalar {
                callee: callee.id,
                arguments: vec![id(10, ValueId::new)],
                structural_arguments: vec![StructuralArgument {
                    place: id(1, PlaceId::new),
                    path: Vec::new(),
                    access: StructuralAccess::SharedBorrow,
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
    ];
    module.machines.push(callee);
    module
}

#[test]
fn cyclic_structural_scalar_call_borrows_iteration_local_storage() {
    let module = structural_scalar_call_cycle();
    assert_verifies(&module, &ProofBundle::default());
    let mut reordered = module.clone();
    reordered.machines[0].blocks.reverse();
    assert_verifies(&reordered, &ProofBundle::default());
}

#[test]
fn cyclic_structural_scalar_call_requires_independent_invocation_evidence() {
    let mut module = structural_scalar_call_cycle();
    module.machines[0].contract.requires = vec![boolean_requirement(10)];
    module.machines[1].contract.requires = vec![boolean_requirement(20)];
    let OperationKind::CallStructuralScalar {
        requirement_obligations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    requirement_obligations.push(id(40, ObligationId::new));
    let proof = evidence(
        40,
        boolean_requirement(10),
        ProofRule::Assumption { index: 0 },
    );
    assert_verifies(&module, &proof);
    assert!(!verifies(&module, &ProofBundle::default()));
    let mut omitted = module.clone();
    let OperationKind::CallStructuralScalar {
        requirement_obligations,
        ..
    } = &mut omitted.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    requirement_obligations.clear();
    assert!(matches!(
        validate_module(&omitted),
        Err(ModuleError::CallRequirementArityMismatch { .. })
    ));
    module.machines[0].contract.requires.clear();
    assert!(!verifies(&module, &proof));
}

#[test]
fn cyclic_structural_scalar_call_preserves_surviving_crash_continuations() {
    let mut module = structural_scalar_call_cycle();
    let routes = vec![CrashRouteBucket {
        cause: CrashCause::Trap,
        alternatives: vec![CrashRouteGuard::Truth],
    }];
    module.machines[0].contract.crash_routes = routes.clone();
    module.machines[1].contract.crash_routes = routes.clone();
    module.machines[1].blocks[0].terminator = Terminator::Crash {
        edge: id(20, EdgeId::new),
        cause: CrashCause::Trap,
        site_guard: Vec::new(),
        frontier_lower_bound: Vec::new(),
    };
    let OperationKind::CallStructuralScalar {
        crash_continuations,
        ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *crash_continuations = routes;
    assert_verifies(&module, &ProofBundle::default());
    let mut omitted = module.clone();
    let OperationKind::CallStructuralScalar {
        crash_continuations,
        ..
    } = &mut omitted.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    crash_continuations.clear();
    assert!(matches!(
        validate_module(&omitted),
        Err(ModuleError::CallCrashContinuationsMismatch { .. })
    ));
    module.machines[0].contract.crash_routes.clear();
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::CallCrashContinuationUncovered { .. })
    ));
}

#[test]
fn cyclic_structural_scalar_call_keeps_claim_transfers_outside_bounded_eligibility() {
    let mut module = structural_scalar_call_cycle();
    let OperationKind::CallStructuralScalar {
        claim_transfers, ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    claim_transfers.push(ClaimTransfer {
        claim: semantic_vocabulary::ClaimId::new(1).unwrap(),
        argument_index: 0,
    });
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::UnitCallClaimTransferCountMismatch { .. })
    ));
}

#[test]
fn cyclic_scalar_call_does_not_justify_a_false_callee_guarantee() {
    let mut module = scalar_call_cycle();
    let guarantee = boolean_requirement(21);
    module.machines[1].contract.ensures.push(ContractClause {
        obligation: id(50, ObligationId::new),
        proposition: guarantee.clone(),
    });
    validate_module_for_interpretation(&module)
        .expect("false guarantee remains structurally valid and requires evidence");
    let proof = evidence(50, guarantee, ProofRule::SemanticAxiom { index: 0 });
    assert!(!verifies(&module, &proof));
}

/// A cyclic caller whose loop block hosts a `CallStructuralWithScalarArguments`
/// producing an unrestricted plain-sum result: the copy payload never enters
/// `owned_places`, so the producer may re-execute on every traversal with no
/// disposal roster at all, and the member membership read observes it through
/// ordinary availability. The affine counterpart stays confined to the same
/// block's dispatch or return; this is the copy-payload form.
fn unrestricted_structural_call_cycle() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let sum_type = id(1, StructuralTypeId::new);
    let flag_case = id(1, StructuralCaseId::new);
    let flag_field = id(1, StructuralFieldId::new);
    let scalar = ScalarType::Boolean;
    module.structural_types.push(StructuralTypeDeclaration {
        id: sum_type,
        identity: "test::Flag".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: flag_case,
                    identity: "Set".into(),
                    fields: vec![StructuralFieldDeclaration {
                        id: flag_field,
                        identity: "value".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar),
                    }],
                },
                StructuralCaseDeclaration {
                    id: id(2, StructuralCaseId::new),
                    identity: "Clear".into(),
                    fields: Vec::new(),
                },
            ],
        },
    });
    let mut callee = module.machines[0].clone();
    callee.id = id(2, MachineId::new);
    callee.contract.id = id(2, ContractId::new);
    callee.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: id(20, ValueId::new),
        scalar_type: scalar,
    }];
    callee.structural_places = vec![
        StructuralPlaceDeclaration {
            id: id(10, PlaceId::new),
            kind: StructuralPlaceKind::Result,
        },
        StructuralPlaceDeclaration {
            id: id(11, PlaceId::new),
            kind: StructuralPlaceKind::OperationResult {
                producer: id(20, OperationId::new),
                structural_type: sum_type,
            },
        },
    ];
    callee.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: id(10, PlaceId::new),
        structural_type: sum_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    callee.entry = id(100, BlockId::new);
    callee.blocks = vec![Block {
        structural_parameters: Vec::new(),
        id: id(100, BlockId::new),
        parameters: Vec::new(),
        operations: vec![Operation {
            static_reach_binding: None,
            id: id(20, OperationId::new),
            result: OperationResult::Structural(StructuralOperationResult {
                place: id(11, PlaceId::new),
                structural_type: sum_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::EstablishScalarCase {
                result_case: flag_case,
                fields: vec![ScalarCaseField {
                    field: flag_field,
                    value: id(20, ValueId::new),
                    range_obligation: None,
                }],
            },
        }],
        terminator: Terminator::ReturnStructural {
            edge: id(20, EdgeId::new),
            source: id(11, PlaceId::new),
            returned_claims: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    }];
    let machine = &mut module.machines[0];
    machine.structural_places = vec![StructuralPlaceDeclaration {
        id: id(1, PlaceId::new),
        kind: StructuralPlaceKind::OperationResult {
            producer: id(30, OperationId::new),
            structural_type: sum_type,
        },
    }];
    machine.blocks[0].operations = vec![
        Operation {
            static_reach_binding: None,
            id: id(30, OperationId::new),
            result: OperationResult::Structural(StructuralOperationResult {
                place: id(1, PlaceId::new),
                structural_type: sum_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::CallStructuralWithScalarArguments {
                callee: callee.id,
                arguments: vec![id(10, ValueId::new)],
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                returned_claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        },
        Operation {
            static_reach_binding: None,
            id: id(31, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(31, ValueId::new),
                scalar_type: scalar,
            }),
            kind: OperationKind::StructuralCaseMembership {
                source: id(1, PlaceId::new),
                path: Vec::new(),
                case: flag_case,
            },
        },
    ];
    module.machines.push(callee);
    module
}

#[test]
fn cyclic_unrestricted_structural_call_result_verifies() {
    let module = unrestricted_structural_call_cycle();
    assert_verifies(&module, &ProofBundle::default());
    let mut reordered = module.clone();
    reordered.machines[0].blocks.reverse();
    assert_verifies(&reordered, &ProofBundle::default());
}

#[test]
fn cyclic_structural_call_result_needs_its_plain_or_confined_shape() {
    let module = unrestricted_structural_call_cycle();
    // An affine result not confined to the block's dispatch or return keeps
    // the fence closed: nothing schedules its disposal. The callee's result
    // declaration and its producer flip with it so only the cyclic custody
    // confinement is exercised.
    let mut affine = module.clone();
    let callee = &mut affine.machines[1];
    if let TerminalMachineResult::Structural(result) = &mut callee.result {
        result.multiplicity = StructuralMultiplicity::Affine;
    }
    if let OperationResult::Structural(result) = &mut callee.blocks[0].operations[0].result {
        result.multiplicity = StructuralMultiplicity::Affine;
    }
    let operation = &mut affine.machines[0].blocks[0].operations[0];
    let OperationResult::Structural(result) = &mut operation.result else {
        unreachable!()
    };
    result.multiplicity = StructuralMultiplicity::Affine;
    assert!(
        matches!(
            validate_module(&affine),
            Err(ModuleError::ControlCycle { .. })
        ),
        "affine unconfined result: {:?}",
        validate_module(&affine)
    );
    // An unrestricted result may never be dispatched inside the cycle: the
    // confinement `local_case_result` proves is affine-only, so spelling the
    // exact affine confinement — every case edge discarding the source —
    // over the copy payload still keeps the fence closed.
    let mut dispatched = module.clone();
    let place = id(1, PlaceId::new);
    dispatched.machines[0].blocks[0].terminator = Terminator::StructuralCase {
        source: place,
        cases: vec![
            terminal_psi::StructuralCaseSuccessorEdge {
                edge: id(10, EdgeId::new),
                target: id(1, BlockId::new),
                case: id(1, StructuralCaseId::new),
                payload_fields: vec![id(1, StructuralFieldId::new)],
                trivial_affine_discards: vec![place],
            },
            terminal_psi::StructuralCaseSuccessorEdge {
                edge: id(11, EdgeId::new),
                target: id(4, BlockId::new),
                case: id(2, StructuralCaseId::new),
                payload_fields: Vec::new(),
                trivial_affine_discards: vec![place],
            },
        ],
    };
    assert!(matches!(
        validate_module(&dispatched),
        Err(ModuleError::ControlCycle { .. })
    ));
}
