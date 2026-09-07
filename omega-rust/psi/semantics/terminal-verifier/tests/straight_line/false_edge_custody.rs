use super::*;

fn value(identity: u64) -> ValueId {
    ValueId::new(identity).unwrap()
}
fn block(identity: u64) -> BlockId {
    BlockId::new(identity).unwrap()
}
fn edge(identity: u64) -> EdgeId {
    EdgeId::new(identity).unwrap()
}
fn integer_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
}
fn declaration(identity: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: value(identity),
        scalar_type: ScalarType::Integer(integer_type()),
    }
}

fn constant(identity: u64, result: u64, integer: u128) -> Operation {
    Operation {
        id: OperationId::new(identity).unwrap(),
        result: OperationResult::Scalar(declaration(result)),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(integer),
        },
    }
}

fn comparison(identity: u64, result: u64, operand: u64) -> Operation {
    Operation {
        id: OperationId::new(identity).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value(result),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::IntegerEqual {
            left: value(operand),
            right: value(3),
        },
    }
}

/// Divide only on the false edge of denominator == 0. A second independent
/// comparison and parameter provide same-typed adversarial substitutions.
fn module(with_alias_hop: bool) -> TerminalModule {
    let mut module = unit_module();
    let machine = &mut module.machines[0];
    machine.parameters = vec![declaration(1), declaration(2)];
    machine.result = TerminalMachineResult::Scalar(declaration(99));
    machine.entry = block(1);
    machine.blocks = vec![
        Block {
            id: block(1),
            parameters: Vec::new(),
            operations: vec![constant(1, 3, 0), comparison(2, 5, 1), comparison(3, 6, 2)],
            terminator: Terminator::Conditional {
                condition: value(5),
                when_true: SuccessorEdge {
                    edge: edge(1),
                    target: block(2),
                    arguments: vec![value(1)],
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    edge: edge(2),
                    target: block(3),
                    arguments: vec![value(1)],
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            id: block(2),
            parameters: vec![declaration(10)],
            operations: Vec::new(),
            terminator: Terminator::Return {
                cleanup_actions: Vec::new(),
                edge: edge(3),
                value: value(10),
            },
        },
    ];
    let divisor = if with_alias_hop { 14 } else { 11 };
    if with_alias_hop {
        machine.blocks.push(Block {
            id: block(3),
            parameters: vec![declaration(11)],
            operations: Vec::new(),
            terminator: Terminator::Jump {
                edge: edge(4),
                target: block(4),
                arguments: vec![value(11)],
                residual_affine_discards: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        });
    }
    machine.blocks.push(Block {
        id: block(if with_alias_hop { 4 } else { 3 }),
        parameters: vec![declaration(divisor)],
        operations: vec![
            constant(4, 13, 7),
            Operation {
                id: OperationId::new(5).unwrap(),
                result: OperationResult::Scalar(declaration(12)),
                kind: OperationKind::ExactIntegerDivide {
                    left: value(13),
                    right: value(divisor),
                    obligation: ObligationId::new(1).unwrap(),
                },
            },
        ],
        terminator: Terminator::Return {
            cleanup_actions: Vec::new(),
            edge: edge(5),
            value: value(12),
        },
    });
    module
}

fn division_site(module: &TerminalModule) -> terminal_verifier::ReconstructedOperationObligation {
    validate_module(module).expect("shape remains valid independently of its proof");
    let mut obligations = reconstruct_operation_obligations(module).expect("reconstruct");
    assert_eq!(obligations.len(), 1);
    let obligation = obligations.pop().unwrap();
    assert!(obligation.canonical_certificate);
    assert_eq!(obligation.obligation.id, ObligationId::new(1).unwrap());
    obligation
}

fn bundle(module: &TerminalModule, divisor: u64) -> ProofBundle {
    let site = division_site(module);
    assert_eq!(
        site.obligation.proposition,
        Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(1)).unwrap(),
            ScalarTerm::value(value(divisor), ScalarType::Integer(integer_type())),
        )
    );
    let index = site
        .semantic_axioms
        .iter()
        .position(|fact| fact == &site.obligation.proposition)
        .expect("false-edge nonzero fact must reach the exact aliased denominator");
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: site.obligation.proposition,
                    rule: ProofRule::SemanticAxiom { index },
                },
            }),
        }],
        ..Default::default()
    }
}

fn assert_certificate_rejected(module: &TerminalModule, bundle: &ProofBundle) {
    let error = verify_module(module, bundle, &AdmissionProfile::default())
        .expect_err("the original certificate must lose its selected-edge premise");
    assert!(
        matches!(&error, VerificationError::RejectedEvidence {
        obligation,
        error: EvidenceError::Certificate(_),
    } if *obligation == ObligationId::new(1).unwrap()),
        "{error:#?}"
    );
}

#[test]
fn false_edge_proof_uses_exact_compared_operand_across_parameter_aliases() {
    for with_alias_hop in [false, true] {
        let module = module(with_alias_hop);
        let bundle = bundle(&module, if with_alias_hop { 14 } else { 11 });
        verify_module(&module, &bundle, &AdmissionProfile::default())
            .expect("kernel accepts the exact reconstructed false-edge premise");
    }
}

#[test]
fn false_edge_certificate_rejects_sibling_condition_and_operand_substitution() {
    for with_alias_hop in [false, true] {
        let original = module(with_alias_hop);
        let bundle = bundle(&original, if with_alias_hop { 14 } else { 11 });
        verify_module(&original, &bundle, &AdmissionProfile::default()).expect("original verifies");
        for mutation in 0..3 {
            let mut changed = original.clone();
            let Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } = &mut changed.machines[0].blocks[0].terminator
            else {
                unreachable!()
            };
            match mutation {
                0 => std::mem::swap(when_true, when_false),
                1 => *condition = value(6),
                _ => when_false.arguments[0] = value(2),
            }
            let site = division_site(&changed);
            assert!(
                !site.semantic_axioms.contains(&site.obligation.proposition),
                "mutation {mutation}"
            );
            assert_certificate_rejected(&changed, &bundle);
        }
    }
}

#[test]
fn false_edge_only_fact_cannot_escape_a_reconverged_join() {
    let mut module = module(true);
    let bundle = bundle(&module, 14);
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("single incoming proof verifies");
    module.machines[0].blocks[1].terminator = Terminator::Jump {
        edge: edge(6),
        target: block(4),
        arguments: vec![value(10)],
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let site = division_site(&module);
    assert!(
        !site.semantic_axioms.contains(&site.obligation.proposition),
        "true edge admits zero and must participate in the intersection"
    );
    assert_certificate_rejected(&module, &bundle);
}
