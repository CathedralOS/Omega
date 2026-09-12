use super::*;
use terminal_psi::{ContractClause, CrashCause, CrashRouteBucket, CrashRouteGuard};

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
