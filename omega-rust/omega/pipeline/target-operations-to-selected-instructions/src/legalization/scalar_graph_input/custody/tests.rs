//! Real verifier custody for ordinary cycles: an unranked loop with no
//! arithmetic or proof obligations, a Natural-ranked loop discharged by a
//! grouped control-cycle certificate, and the countdown idiom carried by the
//! ordinary Natural route.
use super::validate_unit_custody;
use proof_admission::{
    CertificateEnvelope, EvidenceRoute, IntegerAffineWitness, ProofNode, ProofRule,
    ProofSystemMarker,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, EvidenceIdentity, FuelScheduleIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType,
    ValueId,
};
use terminal_psi::{
    Block, ControlCycleEvidence, MachineContract, ObligationEvidence, Operation, OperationKind,
    OperationResult, ProofBundle, RecursiveComponentCertificate, RecursiveEdgeCertificate,
    SuccessorEdge, TerminalBlockNaturalRank, TerminalMachine, TerminalMachineResult,
    TerminalModule, TerminalNaturalCycle, TerminalNaturalRankComparison, TerminalNaturalRankEdge,
    TerminalRankedScc, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_psi_to_abstract_operations::{
    VerifiedPsiOptimizationUnit, build_verified_psi_optimization_unit,
    lower_artifact_for_optimization,
};

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}

fn verified(backedge_ordinal: u64) -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1).unwrap();
    let entry = BlockId::new(1).unwrap();
    let header = BlockId::new(2).unwrap();
    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: [(entry, 1), (header, backedge_ordinal)]
                .map(|(id, edge)| Block {
                    erased_scalar_formals: Vec::new(),
                    id,
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        edge: EdgeId::new(edge).unwrap(),
                        target: header,
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                })
                .to_vec(),
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let input = lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(&module).unwrap(),
            proof_bytes: &terminal_codec::encode_proof_section(&module, &ProofBundle::default())
                .unwrap(),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap();
    build_verified_psi_optimization_unit(input, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}

fn scalar(raw: u64, integer: IntegerType) -> ValueDeclaration {
    ValueDeclaration {
        qualifications: Default::default(),
        id: id(raw, ValueId::new),
        scalar_type: ScalarType::Integer(integer),
    }
}

fn jump(edge: u64, target: BlockId, arguments: Vec<ValueId>) -> Terminator {
    Terminator::Jump {
        erased_arguments: Vec::new(),
        edge: id(edge, EdgeId::new),
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

fn successor(edge: u64, target: BlockId, arguments: Vec<ValueId>) -> SuccessorEdge {
    SuccessorEdge {
        erased_arguments: Vec::new(),
        edge: id(edge, EdgeId::new),
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn operation(number: u64, result: ValueDeclaration, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        id: id(number, OperationId::new),
        result: OperationResult::Scalar(result),
        kind,
    }
}

fn empty_module(machine: TerminalMachine) -> TerminalModule {
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine.id,
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![machine],
    }
}

fn contract() -> MachineContract {
    MachineContract {
        erased_scalar_formals: Vec::new(),
        id: id(1, ContractId::new),
        crash_routes: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        outcome_specific_ensures: Vec::new(),
    }
}

fn machine(
    parameters: Vec<ValueDeclaration>,
    ranked_scc: Option<TerminalRankedScc>,
    entry: BlockId,
    blocks: Vec<Block>,
) -> TerminalMachine {
    TerminalMachine {
        closed_reach_application: None,
        declared_service_reach: Vec::new(),
        id: id(1, MachineId::new),
        attachment: None,
        parameters,
        structural_parameters: Vec::new(),
        ranked_scc,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks,
        contract: contract(),
    }
}

fn plain_block(
    block: BlockId,
    parameters: Vec<ValueDeclaration>,
    operations: Vec<Operation>,
    terminator: Terminator,
) -> Block {
    Block {
        erased_scalar_formals: Vec::new(),
        id: block,
        parameters,
        structural_parameters: Vec::new(),
        operations,
        terminator,
    }
}

fn build(module: &TerminalModule, proof: &ProofBundle) -> VerifiedPsiOptimizationUnit {
    let input = lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &terminal_codec::encode_module(module).unwrap(),
            proof_bytes: &terminal_codec::encode_proof_section(module, proof).unwrap(),
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap();
    build_verified_psi_optimization_unit(input, FuelScheduleIdentity::new(1).unwrap()).unwrap()
}

/// A Natural-ranked self loop: `header` owns the `rank` parameter and repeats
/// while `0 < rank`, passing the literal `zero` as the successor rank. The
/// strict decrease `zero < rank` is exactly the reconstructed true-edge guard.
fn verified_natural_cycle() -> VerifiedPsiOptimizationUnit {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let entry = id(1, BlockId::new);
    let header = id(2, BlockId::new);
    let done = id(3, BlockId::new);
    let initial = id(1, ValueId::new);
    let rank = id(2, ValueId::new);
    let zero = id(3, ValueId::new);
    let condition = id(4, ValueId::new);
    let backedge = id(2, EdgeId::new);
    let module = empty_module(machine(
        vec![scalar(1, integer)],
        Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
            rank_type: integer,
            ranks: vec![TerminalBlockNaturalRank {
                block: header,
                value: rank,
            }],
            edges: vec![TerminalNaturalRankEdge {
                edge: backedge,
                source: header,
                target: header,
                successor_rank: zero,
                comparison: TerminalNaturalRankComparison::Strict,
            }],
        }])),
        entry,
        vec![
            plain_block(
                entry,
                Vec::new(),
                Vec::new(),
                jump(1, header, vec![initial]),
            ),
            plain_block(
                header,
                vec![scalar(2, integer)],
                vec![
                    operation(
                        1,
                        scalar(3, integer),
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(0),
                        },
                    ),
                    operation(
                        2,
                        ValueDeclaration {
                            qualifications: Default::default(),
                            id: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        OperationKind::IntegerLessThan {
                            left: zero,
                            right: rank,
                        },
                    ),
                ],
                Terminator::Conditional {
                    condition,
                    when_true: successor(2, header, vec![zero]),
                    when_false: successor(3, done, Vec::new()),
                },
            ),
            plain_block(
                done,
                Vec::new(),
                Vec::new(),
                Terminator::ReturnUnit {
                    edge: id(4, EdgeId::new),
                    trivial_affine_discards: Vec::new(),
                },
            ),
        ],
    ));
    let questions = terminal_verifier::reconstruct_control_cycle_obligations(&module)
        .expect("natural cycle obligations reconstruct");
    let [question] = questions.as_slice() else {
        panic!("one natural component question");
    };
    let obligation = &question.obligation;
    let mut envelope = 2u64;
    let mut envelope_proof = |proof: ProofNode| {
        envelope += 1;
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: id(envelope, EvidenceIdentity::new),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof,
        })
    };
    let well_foundedness = envelope_proof(ProofNode {
        conclusion: obligation.well_foundedness.obligation.proposition.clone(),
        rule: ProofRule::SemanticAxiom { index: 0 },
    });
    let zero_literal =
        ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).expect("unsigned zero literal");
    let edges = obligation
        .edges
        .iter()
        .map(|edge| {
            let decrease = &edge.decrease;
            let conclusion = decrease.obligation.proposition.clone();
            let Proposition::LessThan(after, before) = &conclusion else {
                panic!("strict decrease proposition");
            };
            let guard = Proposition::LessThan(zero_literal.clone(), before.clone());
            let equality = Proposition::Equal(after.clone(), zero_literal.clone());
            let index = |wanted: &Proposition| {
                decrease
                    .semantic_axioms
                    .iter()
                    .position(|axiom| axiom == wanted)
                    .expect("reconstructed edge axiom")
            };
            RecursiveEdgeCertificate {
                obligation: decrease.obligation.id,
                evidence: envelope_proof(ProofNode {
                    conclusion,
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(ProofNode {
                            conclusion: guard.clone(),
                            rule: ProofRule::SemanticAxiom {
                                index: index(&guard),
                            },
                        }),
                        equality: Box::new(ProofNode {
                            conclusion: equality.clone(),
                            rule: ProofRule::SemanticAxiom {
                                index: index(&equality),
                            },
                        }),
                        endpoint: 0,
                    },
                }),
            }
        })
        .collect();
    let proof = ProofBundle {
        evidence: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: vec![ControlCycleEvidence {
            component: question.component,
            certificate: RecursiveComponentCertificate {
                identity: id(1, EvidenceIdentity::new),
                ranking_relation: obligation
                    .ranking_relation
                    .expect("natural cycles reconstruct a relation"),
                well_foundedness,
                edges,
            },
        }],
        evidence_producers: Vec::new(),
    };
    build(&module, &proof)
}

/// The countdown idiom carried by the ordinary Natural route. The verifier
/// admits it under the same reconstructed natural-cycle question as every
/// other ranked component, and legalizer custody replays it like any cycle.
fn verified_countdown() -> VerifiedPsiOptimizationUnit {
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let preheader = id(1, BlockId::new);
    let header = id(2, BlockId::new);
    let decrement = id(3, BlockId::new);
    let done = id(4, BlockId::new);
    let initial = id(1, ValueId::new);
    let rank = id(2, ValueId::new);
    let zero = id(3, ValueId::new);
    let condition = id(4, ValueId::new);
    let one = id(5, ValueId::new);
    let next = id(6, ValueId::new);
    let backedge = id(4, EdgeId::new);
    let module = empty_module(machine(
        vec![scalar(1, integer)],
        Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
            rank_type: integer,
            ranks: vec![
                TerminalBlockNaturalRank {
                    block: header,
                    value: rank,
                },
                TerminalBlockNaturalRank {
                    block: decrement,
                    value: rank,
                },
            ],
            edges: vec![
                TerminalNaturalRankEdge {
                    edge: id(2, EdgeId::new),
                    source: header,
                    target: decrement,
                    successor_rank: rank,
                    comparison: TerminalNaturalRankComparison::Preserving,
                },
                TerminalNaturalRankEdge {
                    edge: backedge,
                    source: decrement,
                    target: header,
                    successor_rank: next,
                    comparison: TerminalNaturalRankComparison::Strict,
                },
            ],
        }])),
        preheader,
        vec![
            plain_block(
                preheader,
                Vec::new(),
                Vec::new(),
                jump(1, header, vec![initial]),
            ),
            plain_block(
                header,
                vec![scalar(2, integer)],
                vec![
                    operation(
                        1,
                        scalar(3, integer),
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(0),
                        },
                    ),
                    operation(
                        2,
                        ValueDeclaration {
                            qualifications: Default::default(),
                            id: condition,
                            scalar_type: ScalarType::Boolean,
                        },
                        OperationKind::IntegerLessThan {
                            left: zero,
                            right: rank,
                        },
                    ),
                ],
                Terminator::Conditional {
                    condition,
                    when_true: successor(2, decrement, Vec::new()),
                    when_false: successor(3, done, Vec::new()),
                },
            ),
            plain_block(
                decrement,
                Vec::new(),
                vec![
                    operation(
                        3,
                        scalar(5, integer),
                        OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(1),
                        },
                    ),
                    operation(
                        4,
                        scalar(6, integer),
                        OperationKind::ExactIntegerSubtract {
                            left: rank,
                            right: one,
                            obligation: id(1, ObligationId::new),
                        },
                    ),
                ],
                jump(4, header, vec![next]),
            ),
            plain_block(
                done,
                Vec::new(),
                Vec::new(),
                Terminator::ReturnUnit {
                    edge: id(5, EdgeId::new),
                    trivial_affine_discards: Vec::new(),
                },
            ),
        ],
    ));
    let optimizable = terminal_verifier::validate_module_for_optimization(&module)
        .expect("countdown fixture remains optimizer-admitted");
    let obligations = terminal_verifier::reconstruct_optimizable_terminal_obligations(optimizable)
        .expect("countdown obligation reconstructs");
    let [reconstructed] = obligations.obligations() else {
        panic!("the exact subtract carries one obligation");
    };
    let rank_term = ScalarTerm::value(rank, scalar_type);
    let one_term = ScalarTerm::value(one, scalar_type);
    let literal_one =
        ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).expect("unsigned one literal");
    let literal_guard = Proposition::LessOrEqual(literal_one.clone(), rank_term.clone());
    let one_landing = Proposition::Equal(one_term.clone(), literal_one);
    let index = |wanted: &Proposition| {
        reconstructed
            .semantic_axioms
            .iter()
            .position(|axiom| axiom == wanted)
            .expect("reconstructed countdown axiom")
    };
    let questions = terminal_verifier::reconstruct_control_cycle_obligations(&module)
        .expect("natural cycle obligations reconstruct");
    let [question] = questions.as_slice() else {
        panic!("one natural component question");
    };
    let cycle_obligation = &question.obligation;
    let mut cycle_envelope = 0x10u64;
    let mut cycle_proof = |proof: ProofNode| {
        cycle_envelope += 1;
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: id(cycle_envelope, EvidenceIdentity::new),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof,
        })
    };
    let well_foundedness = cycle_proof(ProofNode {
        conclusion: cycle_obligation
            .well_foundedness
            .obligation
            .proposition
            .clone(),
        rule: ProofRule::SemanticAxiom { index: 0 },
    });
    let zero_literal =
        ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).expect("unsigned zero literal");
    let edges = cycle_obligation
        .edges
        .iter()
        .map(|edge| {
            let decrease = &edge.decrease;
            let conclusion = decrease.obligation.proposition.clone();
            let axiom = |wanted: &Proposition| {
                decrease
                    .semantic_axioms
                    .iter()
                    .position(|axiom| axiom == wanted)
                    .expect("reconstructed countdown edge axiom")
            };
            let proof = match &conclusion {
                Proposition::LessOrEqual(after, before) if after == before => ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::IntegerOrderWeakening {
                        relation: Box::new(ProofNode {
                            conclusion: Proposition::Equal(after.clone(), before.clone()),
                            rule: ProofRule::Primitive(
                                proof_admission::PrimitiveJudgment::ReflexiveEquality,
                            ),
                        }),
                    },
                },
                Proposition::LessThan(after, before) => {
                    let difference = Proposition::Equal(
                        after.clone(),
                        ScalarTerm::exact_integer_subtract(
                            integer,
                            before.clone(),
                            ScalarTerm::value(one, scalar_type),
                        )
                        .expect("countdown subtraction term"),
                    );
                    let positive = Proposition::LessThan(
                        zero_literal.clone(),
                        ScalarTerm::value(one, scalar_type),
                    );
                    let one_landing = Proposition::Equal(
                        ScalarTerm::value(one, scalar_type),
                        ScalarTerm::integer(integer, IntegerValue::Unsigned(1))
                            .expect("unsigned one literal"),
                    );
                    ProofNode {
                        conclusion: conclusion.clone(),
                        rule: ProofRule::IntegerSubtractOrder {
                            difference: Box::new(ProofNode {
                                conclusion: difference.clone(),
                                rule: ProofRule::SemanticAxiom {
                                    index: axiom(&difference),
                                },
                            }),
                            positive: Box::new(ProofNode {
                                conclusion: positive,
                                rule: ProofRule::IntegerOrderSubstitution {
                                    relation: Box::new(ProofNode {
                                        conclusion: Proposition::LessThan(
                                            zero_literal.clone(),
                                            ScalarTerm::integer(
                                                integer,
                                                IntegerValue::Unsigned(1),
                                            )
                                            .expect("unsigned one literal"),
                                        ),
                                        rule: ProofRule::Primitive(
                                            proof_admission::PrimitiveJudgment::ClosedIntegerRelation,
                                        ),
                                    }),
                                    equality: Box::new(ProofNode {
                                        conclusion: one_landing.clone(),
                                        rule: ProofRule::SemanticAxiom {
                                            index: axiom(&one_landing),
                                        },
                                    }),
                                    endpoint: 1,
                                },
                            }),
                        },
                    }
                }
                other => panic!("unexpected countdown decrease question {other:?}"),
            };
            RecursiveEdgeCertificate {
                obligation: decrease.obligation.id,
                evidence: cycle_proof(proof),
            }
        })
        .collect();
    let proof = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: reconstructed.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(1, EvidenceIdentity::new),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: reconstructed.obligation.proposition.clone(),
                    rule: ProofRule::IntegerAffineBound {
                        root_bound: Box::new(ProofNode {
                            conclusion: Proposition::LessOrEqual(one_term.clone(), rank_term),
                            rule: ProofRule::IntegerOrderSubstitution {
                                relation: Box::new(ProofNode {
                                    conclusion: literal_guard.clone(),
                                    rule: ProofRule::SemanticAxiom {
                                        index: index(&literal_guard),
                                    },
                                }),
                                equality: Box::new(ProofNode {
                                    conclusion: one_landing.clone(),
                                    rule: ProofRule::SemanticAxiom {
                                        index: index(&one_landing),
                                    },
                                }),
                                endpoint: 0,
                            },
                        }),
                        witness: IntegerAffineWitness {
                            root: one_term,
                            target: ScalarTerm::exact_integer_subtract(
                                integer,
                                ScalarTerm::value(rank, scalar_type),
                                ScalarTerm::value(one, scalar_type),
                            )
                            .expect("countdown subtraction term"),
                            definition_axioms: Vec::new(),
                            literal_axioms: Vec::new(),
                        },
                    },
                },
            }),
        }],
        recursive_components: Vec::new(),
        control_cycles: vec![ControlCycleEvidence {
            component: question.component,
            certificate: RecursiveComponentCertificate {
                identity: id(0x11, EvidenceIdentity::new),
                ranking_relation: cycle_obligation
                    .ranking_relation
                    .expect("natural cycles reconstruct a relation"),
                well_foundedness,
                edges,
            },
        }],
        evidence_producers: Vec::new(),
    };
    build(&module, &proof)
}

fn target(source: &VerifiedPsiOptimizationUnit) -> target_operations::TargetOperationPlan {
    abstract_operations_to_target_operations::lower_to_target_operations(
        source.input().plan(),
        abstract_operations_to_target_operations::TargetLoweringRequest::new(
            target::NativeTarget::macos_arm64(),
        ),
    )
    .unwrap()
}

#[test]
fn verified_unranked_cycle_has_legalizer_custody_without_rank_evidence() {
    let source = verified(2);
    assert!(
        source.input().context().module().machines[0]
            .ranked_scc
            .is_none()
    );
    assert!(
        source
            .input()
            .context()
            .proof_bundle()
            .control_cycles
            .is_empty()
    );
    validate_unit_custody(
        &target(&source),
        source.input().plan(),
        source.unit(),
        Some(source.input()),
    )
    .unwrap();
}

#[test]
fn unranked_cycle_still_rejects_without_verified_source() {
    let source = verified(2);
    assert!(
        validate_unit_custody(&target(&source), source.input().plan(), source.unit(), None)
            .is_err()
    );
}

#[test]
fn unranked_cycle_rejects_a_different_verified_source() {
    let source = verified(2);
    let other = verified(3);
    assert_ne!(source.input().plan().psi, other.input().plan().psi);
    assert!(
        validate_unit_custody(
            &target(&source),
            source.input().plan(),
            source.unit(),
            Some(other.input())
        )
        .is_err()
    );
}

#[test]
fn unranked_cycle_rejects_coherent_current_backedge_redirection() {
    let source = verified(2);
    let mut changed = source.unit().clone();
    let entry = changed.functions[0].entry;
    let node = &mut changed.functions[0].blocks[1].nodes[0];
    let abstract_operations::AbstractOperation::Jump { target, .. } = &mut node.operation else {
        panic!("loop jump");
    };
    *target = entry;
    node.successors[0].target = entry;
    changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
    assert!(
        validate_unit_custody(
            &self::target(&source),
            source.input().plan(),
            &changed,
            Some(source.input())
        )
        .is_err()
    );
}

#[test]
fn verified_natural_cycle_has_legalizer_custody_through_ordinary_scc_replay() {
    let source = verified_natural_cycle();
    assert!(matches!(
        source.input().context().module().machines[0].ranked_scc,
        Some(TerminalRankedScc::Natural(_))
    ));
    assert_eq!(
        source.input().context().proof_bundle().control_cycles.len(),
        1
    );
    validate_unit_custody(
        &target(&source),
        source.input().plan(),
        source.unit(),
        Some(source.input()),
    )
    .unwrap();
}

#[test]
fn natural_cycle_still_rejects_without_verified_source() {
    let source = verified_natural_cycle();
    assert!(
        validate_unit_custody(&target(&source), source.input().plan(), source.unit(), None)
            .is_err()
    );
}

#[test]
fn natural_cycle_rejects_coherent_current_backedge_redirection() {
    let source = verified_natural_cycle();
    let mut changed = source.unit().clone();
    let entry = changed.functions[0].entry;
    let header = changed.functions[0].blocks[1].id;
    for node in &mut changed.functions[0].blocks[1].nodes {
        let abstract_operations::AbstractOperation::Conditional { when_true, .. } =
            &mut node.operation
        else {
            continue;
        };
        if when_true.target == header {
            when_true.target = entry;
        }
        for successor in &mut node.successors {
            if successor.target == header {
                successor.target = entry;
            }
        }
    }
    changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
    assert!(
        validate_unit_custody(
            &self::target(&source),
            source.input().plan(),
            &changed,
            Some(source.input())
        )
        .is_err()
    );
}

#[test]
fn countdown_cycle_has_legalizer_custody_through_ordinary_natural_replay() {
    let source = verified_countdown();
    assert!(matches!(
        source.input().context().module().machines[0].ranked_scc,
        Some(TerminalRankedScc::Natural(_))
    ));
    validate_unit_custody(
        &target(&source),
        source.input().plan(),
        source.unit(),
        Some(source.input()),
    )
    .unwrap();
}
