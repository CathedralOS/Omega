//! Exact natural countdown evidence using the existing ranked-SCC test rules.

use super::*;
use proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, RecursiveComponentCertificate,
    RecursiveEdgeCertificate,
};
use terminal_psi::{
    ControlCycleEvidence, TerminalBlockNaturalRank, TerminalNaturalCycle,
    TerminalNaturalRankComparison, TerminalNaturalRankEdge, TerminalRankedScc,
};

pub(super) fn module() -> TerminalModule {
    let mut module = local_module();
    let caller = &mut module.machines[0];
    let mut entry = caller.blocks[0].clone();
    entry.id = block_id(200);
    entry.operations = vec![constant(200, 3)];
    let jump = |edge, target, arguments| Terminator::Jump {
        edge: edge_id(edge),
        target: block_id(target),
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    entry.terminator = jump(200, 201, vec![value_id(200)]);
    let mut header = caller.blocks[0].clone();
    header.id = block_id(201);
    header.parameters = vec![scalar(201)];
    header.operations = vec![
        constant(210, 0),
        Operation {
            id: operation_id(211),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(211),
                scalar_type: ScalarType::Boolean,
            }),
            kind: OperationKind::IntegerLessThan {
                left: value_id(210),
                right: value_id(201),
            },
        },
    ];
    let successor = |edge, target| SuccessorEdge {
        edge: edge_id(edge),
        target: block_id(target),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    header.terminator = Terminator::Conditional {
        condition: value_id(211),
        when_true: successor(2, 1),
        when_false: successor(3, 202),
    };
    let mut exit = caller.blocks[0].clone();
    exit.id = block_id(202);
    exit.operations.clear();
    exit.terminator = Terminator::Return {
        edge: edge_id(1),
        value: value_id(210),
        cleanup_actions: Vec::new(),
    };
    caller.blocks[0].operations.extend([
        constant(6, 1),
        Operation {
            id: operation_id(7),
            result: OperationResult::Scalar(scalar(7)),
            kind: OperationKind::ExactIntegerSubtract {
                left: value_id(201),
                right: value_id(6),
                obligation: obligation_id(7),
            },
        },
    ]);
    caller.blocks[0].terminator = jump(4, 201, vec![value_id(7)]);
    caller.entry = entry.id;
    caller.blocks.extend([entry, header, exit]);
    let ScalarType::Integer(rank_type) = scalar(201).scalar_type else {
        panic!("integer rank");
    };
    caller.ranked_scc = Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
        rank_type,
        ranks: [1, 201]
            .into_iter()
            .map(|block| TerminalBlockNaturalRank {
                block: block_id(block),
                value: value_id(201),
            })
            .collect(),
        edges: vec![
            TerminalNaturalRankEdge {
                edge: edge_id(2),
                source: block_id(201),
                target: block_id(1),
                successor_rank: value_id(201),
                comparison: TerminalNaturalRankComparison::Preserving,
            },
            TerminalNaturalRankEdge {
                edge: edge_id(4),
                source: block_id(1),
                target: block_id(201),
                successor_rank: value_id(7),
                comparison: TerminalNaturalRankComparison::Strict,
            },
        ],
    }]));
    module
}

#[test]
fn ranked_primitive_local_borrow_supports_unit_mutators() {
    let mut module = module();
    let operation = &mut module.machines[0].blocks[0].operations[2];
    let OperationKind::CallStructuralScalar {
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = operation.kind.clone()
    else {
        panic!("borrowed primitive mutator");
    };
    operation.result = OperationResult::Unit;
    operation.kind = OperationKind::CallUnit {
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    };
    module.machines[1].result = TerminalMachineResult::Unit;
    module.machines[1].blocks[0].terminator = Terminator::ReturnUnit {
        edge: edge_id(92),
        trivial_affine_discards: Vec::new(),
    };
    assert_eq!(run(&module, true), run(&module, false));
    assert_eq!(run(&module, false).0, expected(0));
    let identities = observe_local_identities(&module);
    assert_eq!(identities.len(), 3);
    assert_eq!(
        identities
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    let mut missing_rank_proof = proof(&module);
    missing_rank_proof.control_cycles.clear();
    assert!(verify_module(&module, &missing_rank_proof, &AdmissionProfile::default()).is_err());
    module.machines[0].blocks[0].operations.swap(1, 2);
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::PrimitiveLocalNotAvailable { .. })
    ));
}

fn axiom(axioms: &[Proposition], conclusion: Proposition) -> ProofNode {
    let index = axioms
        .iter()
        .position(|axiom| *axiom == conclusion)
        .expect("exact reconstructed ranking premise");
    ProofNode {
        conclusion,
        rule: ProofRule::SemanticAxiom { index },
    }
}

fn envelope(identity: u64, proof: ProofNode) -> EvidenceRoute {
    EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: EvidenceIdentity::new(identity).unwrap(),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof,
    })
}

pub(super) fn proof(module: &TerminalModule) -> ProofBundle {
    if module.machines[0].ranked_scc.is_none() {
        return ProofBundle::default();
    }
    let validated = terminal_verifier::validate_module_for_interpretation(module)
        .expect("ranked primitive loop is structurally interpretable");
    let questions =
        terminal_verifier::reconstruct_interpretable_operation_obligations(validated).unwrap();
    let [subtract] = questions.as_slice() else {
        panic!("one exact subtraction");
    };
    let scalar_type = scalar(201).scalar_type;
    let ScalarType::Integer(integer_type) = scalar_type else {
        panic!("integer rank");
    };
    let rank = ScalarTerm::value(value_id(201), scalar_type);
    let one = ScalarTerm::value(value_id(6), scalar_type);
    let literal_one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).unwrap();
    let zero = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(0)).unwrap();
    let difference =
        ScalarTerm::exact_integer_subtract(integer_type, rank.clone(), one.clone()).unwrap();
    let literal_guard = Proposition::LessOrEqual(literal_one.clone(), rank.clone());
    let one_landing = Proposition::Equal(one.clone(), literal_one.clone());
    let subtraction_proof = ProofNode {
        conclusion: subtract.obligation.proposition.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: Proposition::LessOrEqual(one.clone(), rank.clone()),
                rule: ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(axiom(&subtract.semantic_axioms, literal_guard)),
                    equality: Box::new(axiom(&subtract.semantic_axioms, one_landing.clone())),
                    endpoint: 0,
                },
            }),
            witness: IntegerAffineWitness {
                root: one.clone(),
                target: difference.clone(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    };
    let mut proof = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: subtract.obligation.id,
            route: envelope(7, subtraction_proof),
        }],
        ..ProofBundle::default()
    };
    for question in terminal_verifier::reconstruct_control_cycle_obligations(module).unwrap() {
        let well_foundedness = &question.obligation.well_foundedness;
        let mut edges = Vec::new();
        for edge in &question.obligation.edges {
            let conclusion = edge.decrease.obligation.proposition.clone();
            let rule = match &conclusion {
                Proposition::LessOrEqual(left, right) if left == right => {
                    ProofRule::IntegerOrderWeakening {
                        relation: Box::new(ProofNode {
                            conclusion: Proposition::Equal(left.clone(), right.clone()),
                            rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                        }),
                    }
                }
                Proposition::LessThan(_, _) => ProofRule::IntegerSubtractOrder {
                    difference: Box::new(axiom(
                        &edge.decrease.semantic_axioms,
                        Proposition::Equal(
                            ScalarTerm::value(value_id(7), scalar_type),
                            difference.clone(),
                        ),
                    )),
                    positive: Box::new(ProofNode {
                        conclusion: Proposition::LessThan(zero.clone(), one.clone()),
                        rule: ProofRule::IntegerOrderSubstitution {
                            relation: Box::new(ProofNode {
                                conclusion: Proposition::LessThan(
                                    zero.clone(),
                                    literal_one.clone(),
                                ),
                                rule: ProofRule::Primitive(
                                    PrimitiveJudgment::ClosedIntegerRelation,
                                ),
                            }),
                            equality: Box::new(axiom(
                                &edge.decrease.semantic_axioms,
                                one_landing.clone(),
                            )),
                            endpoint: 1,
                        },
                    }),
                },
                other => panic!("unexpected rank question: {other:?}"),
            };
            edges.push(RecursiveEdgeCertificate {
                obligation: edge.decrease.obligation.id,
                evidence: envelope(
                    edge.decrease.obligation.id.get(),
                    ProofNode { conclusion, rule },
                ),
            });
        }
        proof.control_cycles.push(ControlCycleEvidence {
            component: question.component,
            certificate: RecursiveComponentCertificate {
                identity: EvidenceIdentity::new(question.component.get()).unwrap(),
                ranking_relation: question.obligation.ranking_relation.unwrap(),
                well_foundedness: envelope(
                    well_foundedness.obligation.id.get(),
                    axiom(
                        &well_foundedness.semantic_axioms,
                        well_foundedness.obligation.proposition.clone(),
                    ),
                ),
                edges,
            },
        });
    }
    proof
}
