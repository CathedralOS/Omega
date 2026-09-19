use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, IntegerAffineWitness, PrimitiveJudgment,
    ProofNode, ProofRule, ProofSystemMarker, RecursiveComponentCertificate,
    RecursiveEdgeCertificate,
};
use semantic_vocabulary::{
    BlockId, ContractId, CycleComponentId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, PlaceId, Proposition, RankingRelationId,
    ScalarTerm, ScalarType, ServiceId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, ControlCycleEvidence, MachineContract, Operation, OperationKind, OperationResult,
    ServiceDeclaration, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge,
    TerminalBlockNaturalRank, TerminalMachine, TerminalMachineResult, TerminalModule,
    TerminalNaturalCycle, TerminalNaturalRankComparison, TerminalNaturalRankEdge,
    TerminalRankedScc, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{
    ModuleError, ObligationEvidence, ProofBundle, VerificationError,
    reconstruct_control_cycle_obligations, reconstruct_interpretable_operation_obligations,
    validate_module, validate_module_for_interpretation, validate_module_for_optimization,
    validate_module_representation, verify_module, verify_module_for_fixed_fuel,
    verify_module_for_interpretation, verify_module_for_optimization,
};

fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero fixture identity")
}

fn ranked_countdown() -> TerminalModule {
    ranked_countdown_with_width(32)
}

fn ranked_countdown_with_width(bits: u16) -> TerminalModule {
    let machine = id(1, MachineId::new);
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
    let integer = IntegerType::new(IntegerSign::Unsigned, bits).unwrap();
    let scalar = ScalarType::Integer(integer);
    let preheader_edge = id(1, EdgeId::new);
    let guard_edge = id(2, EdgeId::new);
    let exit_edge = id(3, EdgeId::new);
    let backedge = id(4, EdgeId::new);
    let return_edge = id(5, EdgeId::new);

    TerminalModule {
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
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: initial,
                scalar_type: scalar,
            }],
            structural_parameters: Vec::new(),
            // The countdown idiom is carried by the common Natural record:
            // the header's rank is preserved across the guarded forward edge
            // and strictly decreases across the covered backedge.
            ranked_scc: Some(TerminalRankedScc::Natural(vec![TerminalNaturalCycle {
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
                        edge: guard_edge,
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
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: preheader,
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: preheader,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        structural_arguments: Vec::new(),
                        edge: preheader_edge,
                        target: header,
                        arguments: vec![initial],
                        erased_arguments: Vec::new(),
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: header,
                    parameters: vec![ValueDeclaration {
                        qualifications: Default::default(),
                        id: rank,
                        scalar_type: scalar,
                    }],
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: id(1, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: zero,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(0),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: id(2, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: condition,
                                scalar_type: ScalarType::Boolean,
                            }),
                            kind: OperationKind::IntegerLessThan {
                                left: zero,
                                right: rank,
                            },
                        },
                    ],
                    terminator: Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: guard_edge,
                            target: decrement,
                            arguments: Vec::new(),
                            erased_arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: exit_edge,
                            target: done,
                            arguments: Vec::new(),
                            erased_arguments: Vec::new(),
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: decrement,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            static_reach_binding: None,
                            id: id(3, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: one,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(1),
                            },
                        },
                        Operation {
                            static_reach_binding: None,
                            id: id(4, OperationId::new),
                            result: OperationResult::Scalar(ValueDeclaration {
                                qualifications: Default::default(),
                                id: next,
                                scalar_type: scalar,
                            }),
                            kind: OperationKind::ExactIntegerSubtract {
                                left: rank,
                                right: one,
                                obligation: id(1, ObligationId::new),
                            },
                        },
                    ],
                    terminator: Terminator::Jump {
                        structural_arguments: Vec::new(),
                        edge: backedge,
                        target: header,
                        arguments: vec![next],
                        erased_arguments: Vec::new(),
                        residual_affine_discards: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: done,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: return_edge,
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: id(1, ContractId::new),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn unranked_scalar_cycle() -> TerminalModule {
    let mut module = ranked_countdown();
    let machine = &mut module.machines[0];
    let condition = id(10, ValueId::new);
    let entry = id(1, BlockId::new);
    let done = id(4, BlockId::new);
    machine.parameters = vec![ValueDeclaration {
        qualifications: Default::default(),
        id: condition,
        scalar_type: ScalarType::Boolean,
    }];
    machine.ranked_scc = None;
    machine.entry = entry;
    machine.blocks = vec![
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: entry,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Conditional {
                condition,
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: id(10, EdgeId::new),
                    target: entry,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: id(11, EdgeId::new),
                    target: done,
                    arguments: Vec::new(),
                    erased_arguments: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            },
        },
        Block {
            erased_scalar_formals: Vec::new(),
            structural_parameters: Vec::new(),
            id: done,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: id(12, EdgeId::new),
                trivial_affine_discards: Vec::new(),
            },
        },
    ];
    module
}

fn unranked_effectful_unit_cycle() -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let service = id(1, ServiceId::new);
    module.services.push(ServiceDeclaration {
        id: service,
        identity: "DebugIo".to_owned(),
        parents: Vec::new(),
    });
    module.root_service_reach.concrete = vec![service];
    module.machines[0].published_service_ceiling.push(service);
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: id(5, OperationId::new),
        result: OperationResult::Unit,
        kind: OperationKind::PortWrite {
            service,
            port: 0x3f8,
            value: b'X',
        },
    });
    module
}

fn ranked_countdown_proof(module: &TerminalModule) -> ProofBundle {
    let interpretable = validate_module_for_interpretation(module)
        .expect("ranked countdown fixture is interpreter-valid");
    let obligations = reconstruct_interpretable_operation_obligations(interpretable)
        .expect("ranked countdown proof question reconstructs");
    let [reconstructed] = obligations.as_slice() else {
        panic!("ranked countdown has exactly one proof obligation")
    };
    ProofBundle {
        evidence: vec![countdown_decrement_evidence(
            module,
            &reconstructed.obligation,
            &reconstructed.semantic_axioms,
        )],
        recursive_components: Vec::new(),
        control_cycles: vec![countdown_cycle_evidence(module)],
        evidence_producers: Vec::new(),
    }
}

/// Certificate for the decrement's exact `rank - 1` question. The positive
/// guard and the literal-one landing are cited from the exact reconstructed
/// axiom roster, so the same derivation serves a header that carries extra
/// invariant hypotheses.
fn countdown_decrement_evidence(
    module: &TerminalModule,
    reconstructed: &proof_admission::Obligation,
    semantic_axioms: &[Proposition],
) -> ObligationEvidence {
    let scalar_type = module.machines[0].parameters[0].scalar_type;
    let ScalarType::Integer(integer_type) = scalar_type else {
        unreachable!("ranked countdown parameter is an integer")
    };
    let rank = ScalarTerm::value(id(2, ValueId::new), scalar_type);
    let one = ScalarTerm::value(id(5, ValueId::new), scalar_type);
    let literal_one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1))
        .expect("ranked countdown literal one");
    let literal_guard = Proposition::LessOrEqual(literal_one.clone(), rank.clone());
    let guard_axiom = semantic_axioms
        .iter()
        .position(|axiom| *axiom == literal_guard)
        .expect("ranked countdown positive guard is reconstructed as a semantic axiom");
    let one_landing = Proposition::Equal(one.clone(), literal_one);
    let landing_axiom = semantic_axioms
        .iter()
        .position(|axiom| *axiom == one_landing)
        .expect("ranked countdown one is reconstructed as a semantic axiom");
    let ordered_guard = Proposition::LessOrEqual(one.clone(), rank.clone());
    ObligationEvidence {
        obligation: reconstructed.id,
        route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: id(1, EvidenceIdentity::new),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode {
                conclusion: reconstructed.proposition.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(ProofNode {
                        conclusion: ordered_guard,
                        rule: ProofRule::IntegerOrderSubstitution {
                            relation: Box::new(ProofNode {
                                conclusion: literal_guard,
                                rule: ProofRule::SemanticAxiom { index: guard_axiom },
                            }),
                            equality: Box::new(ProofNode {
                                conclusion: one_landing,
                                rule: ProofRule::SemanticAxiom {
                                    index: landing_axiom,
                                },
                            }),
                            endpoint: 0,
                        },
                    }),
                    witness: IntegerAffineWitness {
                        root: one,
                        target: ScalarTerm::exact_integer_subtract(
                            integer_type,
                            rank,
                            ScalarTerm::value(id(5, ValueId::new), scalar_type),
                        )
                        .expect("ranked countdown subtraction"),
                        definition_axioms: Vec::new(),
                        literal_axioms: Vec::new(),
                    },
                },
            },
        }),
    }
}

/// Certificate for the verifier-reconstructed natural-order question the
/// retained countdown claims: forward edges preserve the header's rank and the
/// covered backedge carries the strict `rank - 1 < rank` descent.
fn countdown_cycle_evidence(module: &TerminalModule) -> ControlCycleEvidence {
    let questions = reconstruct_control_cycle_obligations(module)
        .expect("ranked countdown cycle question reconstructs");
    let [question] = questions.as_slice() else {
        panic!("ranked countdown reconstructs exactly one cycle question")
    };
    let obligation = &question.obligation;
    let scalar_type = module.machines[0].parameters[0].scalar_type;
    let ScalarType::Integer(integer_type) = scalar_type else {
        unreachable!("ranked countdown parameter is an integer")
    };
    let one = ScalarTerm::value(id(5, ValueId::new), scalar_type);
    let zero_literal = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(0))
        .expect("ranked countdown literal zero");
    let one_literal = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1))
        .expect("ranked countdown literal one");
    let axiom = |axioms: &[Proposition], conclusion: Proposition| -> ProofNode {
        let index = axioms
            .iter()
            .position(|axiom| *axiom == conclusion)
            .expect("countdown cycle premise is reconstructed as a semantic axiom");
        ProofNode {
            conclusion,
            rule: ProofRule::SemanticAxiom { index },
        }
    };
    let mut envelope = 20_u64;
    let mut wrap = |proof: ProofNode| {
        envelope += 1;
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: id(envelope, EvidenceIdentity::new),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof,
        })
    };
    let well_foundedness = wrap(ProofNode {
        conclusion: obligation.well_foundedness.obligation.proposition.clone(),
        rule: ProofRule::SemanticAxiom { index: 0 },
    });
    let edges = obligation
        .edges
        .iter()
        .map(|edge| {
            let decrease = &edge.decrease;
            let proof = match &decrease.obligation.proposition {
                Proposition::LessOrEqual(left, right) if left == right => ProofNode {
                    conclusion: decrease.obligation.proposition.clone(),
                    rule: ProofRule::IntegerOrderWeakening {
                        relation: Box::new(ProofNode {
                            conclusion: Proposition::Equal(left.clone(), right.clone()),
                            rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                        }),
                    },
                },
                Proposition::LessThan(after, before) => ProofNode {
                    conclusion: decrease.obligation.proposition.clone(),
                    rule: ProofRule::IntegerSubtractOrder {
                        difference: Box::new(axiom(
                            &decrease.semantic_axioms,
                            Proposition::Equal(
                                after.clone(),
                                ScalarTerm::exact_integer_subtract(
                                    integer_type,
                                    before.clone(),
                                    one.clone(),
                                )
                                .expect("ranked countdown subtraction term"),
                            ),
                        )),
                        positive: Box::new(ProofNode {
                            conclusion: Proposition::LessThan(zero_literal.clone(), one.clone()),
                            rule: ProofRule::IntegerOrderSubstitution {
                                relation: Box::new(ProofNode {
                                    conclusion: Proposition::LessThan(
                                        zero_literal.clone(),
                                        one_literal.clone(),
                                    ),
                                    rule: ProofRule::Primitive(
                                        PrimitiveJudgment::ClosedIntegerRelation,
                                    ),
                                }),
                                equality: Box::new(axiom(
                                    &decrease.semantic_axioms,
                                    Proposition::Equal(one.clone(), one_literal.clone()),
                                )),
                                endpoint: 1,
                            },
                        }),
                    },
                },
                other => panic!("unexpected countdown decrease question {other:?}"),
            };
            RecursiveEdgeCertificate {
                obligation: decrease.obligation.id,
                evidence: wrap(proof),
            }
        })
        .collect();
    ControlCycleEvidence {
        component: question.component,
        certificate: RecursiveComponentCertificate {
            identity: id(19, EvidenceIdentity::new),
            ranking_relation: obligation
                .ranking_relation
                .expect("ranked countdown reconstructs its natural relation"),
            well_foundedness,
            edges,
        },
    }
}

fn add_loop_preserved_affine_parameter(module: &mut TerminalModule) -> PlaceId {
    let position = u32::try_from(module.machines[0].structural_parameters.len())
        .expect("fixture structural position fits u32");
    let raw = u64::from(position) + 1;
    let structural_type = id(raw, StructuralTypeId::new);
    let place = id(raw, PlaceId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: format!("LoopCustody{raw}"),
        shape: StructuralTypeShape::Record { fields: Vec::new() },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place,
            position,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter {
            position,
            is_self: false,
        },
    });
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut machine.blocks[3].terminator
    else {
        panic!("countdown exit must return Unit")
    };
    trivial_affine_discards.insert(0, place);
    place
}

#[test]
fn ranked_countdown_with_borrowed_subslice_needs_ordinary_evidence() {
    let mut module = ranked_countdown_with_width(64);
    validate_module_for_interpretation(&module).expect("unchanged countdown is executable");
    let structural_type = id(1, StructuralTypeId::new);
    let source = id(1, PlaceId::new);
    let destination = id(2, PlaceId::new);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::BorrowedBytes".into(),
        shape: StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView),
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: source,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.extend([
        StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        },
        StructuralPlaceDeclaration {
            id: destination,
            kind: StructuralPlaceKind::OperationResult {
                producer: id(6, OperationId::new),
                structural_type,
            },
        },
    ]);
    // An initial countdown of two would revisit this same producer twice.
    // The retained graph remains valid under the ordinary cycle-operation
    // eligibility fence; only its own proof obligations gate verification.
    machine.blocks[2].operations.extend([
        Operation {
            static_reach_binding: None,
            id: id(5, OperationId::new),
            result: OperationResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: id(7, ValueId::new),
                scalar_type: machine.parameters[0].scalar_type,
            }),
            kind: OperationKind::ByteSequenceLength { source },
        },
        Operation {
            static_reach_binding: None,
            id: id(6, OperationId::new),
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: destination,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            }),
            kind: OperationKind::ByteSequenceSubslice {
                source,
                start: id(3, ValueId::new),
                end: id(7, ValueId::new),
                length: id(7, ValueId::new),
                obligation: id(2, ObligationId::new),
            },
        },
    ]);
    validate_module_representation(&module)
        .expect("valid source, result, dominance and unchanged owned frontier");
    validate_module_for_interpretation(&module)
        .expect("borrowed subslice work is interpreter-eligible cycle work");
    assert!(matches!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::MissingEvidence(_))
    ));
}

#[test]
fn ranked_countdown_requires_certificate_under_every_authority() {
    let module = ranked_countdown();
    assert_eq!(validate_module_representation(&module), Ok(()));
    let interpretable = validate_module_for_interpretation(&module)
        .expect("exact ranked countdown is interpreter-valid");
    validate_module_for_optimization(&module)
        .expect("exact ranked countdown is independently optimizer-valid");
    let obligations = reconstruct_interpretable_operation_obligations(interpretable)
        .expect("countdown proof question reconstructs over its acyclic skeleton");
    assert_eq!(obligations.len(), 1);
    assert_eq!(obligations[0].obligation.id, id(1, ObligationId::new));
    let scalar_type = module.machines[0].parameters[0].scalar_type;
    let ScalarType::Integer(integer_type) = scalar_type else {
        unreachable!("ranked countdown parameter is an integer")
    };
    assert!(
        obligations[0]
            .semantic_axioms
            .contains(&Proposition::LessOrEqual(
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).unwrap(),
                ScalarTerm::value(id(2, ValueId::new), scalar_type),
            ))
    );
    assert!(matches!(
        verify_module_for_interpretation(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::MissingEvidence(obligation))
            if obligation == id(1, ObligationId::new)
    ));
    assert!(matches!(
        verify_module_for_fixed_fuel(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::MissingEvidence(obligation))
            if obligation == id(1, ObligationId::new)
    ));
    // Ordinary executable validation no longer consults a countdown shape:
    // the same natural-cycle record validates, and verification then demands
    // the certificate evidence like every other authority.
    assert_eq!(validate_module(&module).map(|_| ()), Ok(()));
    assert!(matches!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::MissingEvidence(obligation))
            if obligation == id(1, ObligationId::new)
    ));
}

#[test]
fn ranked_countdown_rejects_absent_substituted_or_altered_cycle_evidence() {
    let module = ranked_countdown();
    let proof = ranked_countdown_proof(&module);
    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default())
        .expect("the certificate-backed countdown verifies for the interpreter");

    // Certificate absent: shape authority alone no longer admits the cycle.
    let mut missing = proof.clone();
    missing.control_cycles.clear();
    assert!(matches!(
        verify_module_for_interpretation(&module, &missing, &AdmissionProfile::default()),
        Err(VerificationError::MissingControlCycleEvidence(_))
    ));

    // Substituted component identity cannot answer the reconstructed question.
    let mut substituted = proof.clone();
    substituted.control_cycles[0].component = id(77, CycleComponentId::new);
    assert!(matches!(
        verify_module_for_interpretation(&module, &substituted, &AdmissionProfile::default()),
        Err(VerificationError::MissingControlCycleEvidence(_))
    ));

    // An edge certificate naming a different obligation is rejected, not
    // silently attached to the reconstructed decrease.
    let mut wrong_obligation = proof.clone();
    wrong_obligation.control_cycles[0].certificate.edges[1].obligation = id(99, ObligationId::new);
    assert!(matches!(
        verify_module_for_interpretation(&module, &wrong_obligation, &AdmissionProfile::default()),
        Err(VerificationError::RejectedControlCycle { .. })
    ));

    // The preserving edge's reflexivity proof cannot stand in for the strict
    // `rank - 1 < rank` descent.
    let mut altered = proof.clone();
    altered.control_cycles[0].certificate.edges[1].evidence =
        altered.control_cycles[0].certificate.edges[0]
            .evidence
            .clone();
    assert!(matches!(
        verify_module_for_interpretation(&module, &altered, &AdmissionProfile::default()),
        Err(VerificationError::RejectedControlCycle { .. })
    ));

    // A substituted ranking relation is a different component claim.
    let mut wrong_relation = proof.clone();
    wrong_relation.control_cycles[0]
        .certificate
        .ranking_relation = id(65, RankingRelationId::new);
    assert!(matches!(
        verify_module_for_interpretation(&module, &wrong_relation, &AdmissionProfile::default()),
        Err(VerificationError::RejectedControlCycle { .. })
    ));
}

#[test]
fn ranked_countdown_rejects_evidence_stale_for_the_reconstructed_component() {
    let module = ranked_countdown();
    let proof = ranked_countdown_proof(&module);
    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default())
        .expect("the certificate-backed countdown verifies");

    // Renumbering the guard edge changes the reconstructed component identity;
    // the retained certificate is stale for the new question.
    let mut renumbered = module.clone();
    let machine = &mut renumbered.machines[0];
    let moved_guard_edge = id(42, EdgeId::new);
    let TerminalRankedScc::Natural(components) =
        machine.ranked_scc.as_mut().expect("countdown fixture");
    let mut guard_row = components[0].edges.remove(0);
    guard_row.edge = moved_guard_edge;
    components[0].edges.push(guard_row);
    let Terminator::Conditional { when_true, .. } = &mut machine.blocks[1].terminator else {
        unreachable!("countdown header selects the cycle path")
    };
    when_true.edge = moved_guard_edge;
    assert_eq!(validate_module_representation(&renumbered), Ok(()));
    assert!(matches!(
        verify_module_for_interpretation(&renumbered, &proof, &AdmissionProfile::default()),
        Err(VerificationError::MissingControlCycleEvidence(_))
    ));

    // The stale evidence cannot be replayed under a fresh bundle either: the
    // reconstructed obligation ids commit to the new edge identities.
    let fresh = ranked_countdown_proof(&renumbered);
    assert_ne!(
        fresh.control_cycles[0].component, proof.control_cycles[0].component,
        "renumbered edge changes the reconstructed component identity"
    );
    verify_module_for_interpretation(&renumbered, &fresh, &AdmissionProfile::default())
        .expect("a fresh certificate answers the renumbered component");
}

#[test]
fn ranked_countdown_optimizer_authority_retains_proof_and_structural_frontiers() {
    let mut module = ranked_countdown();
    let place = add_loop_preserved_affine_parameter(&mut module);
    assert!(matches!(
        verify_module_for_optimization(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        ),
        Err(VerificationError::MissingEvidence(obligation))
            if obligation == id(1, ObligationId::new)
    ));
    let proof = ranked_countdown_proof(&module);

    let optimizable = verify_module_for_optimization(&module, &proof, &AdmissionProfile::default())
        .expect("the certified countdown has optimizer authority");
    assert_eq!(optimizable.module(), &module);
    assert_eq!(optimizable.proof_bundle(), &proof);
    assert_eq!(
        optimizable.reconstructed_obligations().obligations().len(),
        1
    );
    assert_eq!(optimizable.accepted_facts().len(), 1);
    let header = id(2, BlockId::new);
    let header_frontier = optimizable
        .structural_frontiers()
        .machine(module.entry)
        .and_then(|frontiers| frontiers.block_entry(header))
        .expect("optimizer authority retains the ranked header frontier");
    assert!(
        header_frontier
            .owned_places()
            .iter()
            .any(|owned| owned.place == place)
    );

    verify_module_for_interpretation(&module, &proof, &AdmissionProfile::default())
        .expect("interpreter authority remains independently constructible");
    verify_module_for_fixed_fuel(&module, &proof, &AdmissionProfile::default())
        .expect("fixed-fuel authority remains independently constructible");
    verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("the certified countdown carries ordinary executable authority");
}

#[test]
fn ranked_countdown_admits_extra_pure_scalar_work() {
    let mut module = ranked_countdown();
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    module.machines[0].blocks[0].operations.push(Operation {
        static_reach_binding: None,
        id: id(20, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(20, ValueId::new),
            scalar_type: ScalarType::Integer(integer),
        }),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(7),
        },
    });
    assert_eq!(validate_module_representation(&module), Ok(()));
    validate_module_for_interpretation(&module)
        .expect("ordinary pure scalar work beside the cycle is eligible");
    verify_module_for_interpretation(
        &module,
        &ranked_countdown_proof(&module),
        &AdmissionProfile::default(),
    )
    .expect("the retained certificate still answers the unchanged question");
}

#[test]
fn ranked_countdown_preserves_a_nonempty_structural_frontier() {
    let mut module = ranked_countdown();
    add_loop_preserved_affine_parameter(&mut module);
    assert_eq!(validate_module_representation(&module), Ok(()));
    validate_module_for_interpretation(&module)
        .expect("interpreter accepts a preserved affine countdown frontier");
    assert_eq!(validate_module(&module).map(|_| ()), Ok(()));
}

#[test]
fn ranked_countdown_rejects_a_cycle_body_that_changes_structural_custody() {
    let mut module = ranked_countdown();
    let place = add_loop_preserved_affine_parameter(&mut module);
    let machine = &mut module.machines[0];
    let header = id(2, BlockId::new);
    let Terminator::Conditional { when_true, .. } = &mut machine.blocks[1].terminator else {
        panic!("countdown header must select the cycle path")
    };
    when_true.trivial_affine_discards.push(place);
    assert_eq!(
        validate_module_representation(&module),
        Err(ModuleError::OwnedStructuralFrontierJoinMismatch(header))
    );
}

#[test]
fn ranked_countdown_rejects_false_arithmetic_without_inventing_a_missing_rank() {
    let module = ranked_countdown();
    let mut uncovered = module.clone();
    uncovered.machines[0].ranked_scc = None;
    assert_eq!(validate_module_representation(&uncovered), Ok(()));
    // Without rank evidence the decrement obligation still demands proof;
    // no termination question is reconstructed for the unranked cycle.
    assert!(matches!(
        verify_module_for_interpretation(&uncovered, &ProofBundle::default(), &AdmissionProfile::default()),
        Err(VerificationError::MissingEvidence(obligation))
            if obligation == id(1, ObligationId::new)
    ));

    // Forwarding the unchanged rank instead of `rank - 1` contradicts the
    // recorded strict successor and fails representation validation.
    let mut forwards_original = module.clone();
    let rank = id(2, ValueId::new);
    let decrement = &mut forwards_original.machines[0].blocks[2];
    let Terminator::Jump { arguments, .. } = &mut decrement.terminator else {
        panic!("decrement backedge")
    };
    arguments[0] = rank;
    assert!(matches!(
        validate_module_representation(&forwards_original),
        Err(ModuleError::InvalidRankedScc(_))
    ));

    // Widening the guard literal keeps the record representable, but the
    // reconstructed premises no longer answer the retained certificate: the
    // decrement evidence cites the original `1 <= rank` guard axiom.
    let proof = ranked_countdown_proof(&module);
    let mut wrong_guard = module;
    wrong_guard.machines[0].blocks[1].operations[0].kind = OperationKind::IntegerConstant {
        value: IntegerValue::Unsigned(1),
    };
    assert_eq!(validate_module_representation(&wrong_guard), Ok(()));
    assert!(matches!(
        verify_module_for_interpretation(&wrong_guard, &proof, &AdmissionProfile::default()),
        Err(VerificationError::RejectedEvidence { .. }
            | VerificationError::RejectedControlCycle { .. })
    ));
}

#[test]
fn unranked_scalar_cycle_is_interpreter_valid() {
    let module = unranked_scalar_cycle();
    assert_eq!(validate_module_representation(&module), Ok(()));
    validate_module_for_interpretation(&module)
        .expect("a scalar-only unranked cycle is interpreter-valid");
}

#[test]
fn unranked_effectful_unit_cycle_is_interpreter_valid() {
    let module = unranked_effectful_unit_cycle();
    assert_eq!(validate_module_representation(&module), Ok(()));
    validate_module_for_interpretation(&module)
        .expect("a unit-effect unranked cycle is interpreter-valid");
}

#[path = "ranked_scc/unranked_frontiers.rs"]
mod unranked_frontiers;

#[path = "ranked_scc/unranked_case_results.rs"]
mod unranked_case_results;

#[path = "ranked_scc/unranked_return_facts.rs"]
mod unranked_return_facts;

#[path = "ranked_scc/unranked_bindings.rs"]
mod unranked_bindings;

#[path = "ranked_scc/unranked_views.rs"]
mod unranked_views;

#[path = "ranked_scc/unranked_unit_calls.rs"]
mod unranked_unit_calls;

#[path = "ranked_scc/unranked_scalar_calls.rs"]
mod unranked_scalar_calls;

#[path = "ranked_scc/scalar_block_invariants.rs"]
mod scalar_block_invariants;

#[path = "ranked_scc/natural_stale_observations.rs"]
mod natural_stale_observations;

#[path = "ranked_scc/natural_topology.rs"]
mod natural_topology;

#[path = "ranked_scc/scalar_array_availability.rs"]
mod scalar_array_availability;

#[path = "ranked_scc/unranked_records.rs"]
mod unranked_records;

#[path = "ranked_scc/unranked_claims.rs"]
mod unranked_claims;
