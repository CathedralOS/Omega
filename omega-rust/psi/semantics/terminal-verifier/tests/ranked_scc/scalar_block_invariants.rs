use super::*;
use terminal_psi::{ScalarBlockInvariant, ScalarBlockInvariantArrival};
use terminal_verifier::{ReconstructedTerminalObligationOwner, reconstruct_terminal_obligations};

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
}

fn range(value: u64, maximum: u128) -> Proposition {
    let value = ScalarTerm::value(id(value, ValueId::new), ScalarType::Integer(integer()));
    let mut bounds = vec![
        Proposition::LessOrEqual(
            ScalarTerm::integer(integer(), IntegerValue::Unsigned(0)).unwrap(),
            value.clone(),
        ),
        Proposition::LessOrEqual(
            value,
            ScalarTerm::integer(integer(), IntegerValue::Unsigned(maximum)).unwrap(),
        ),
    ];
    bounds.sort();
    Proposition::Conjunction(bounds)
}

fn module() -> TerminalModule {
    let mut module = ranked_countdown();
    let machine = &mut module.machines[0];
    machine.ranked_scc = None;
    machine.contract.requires = vec![range(1, 5)];
    // A preserving cycle isolates invariant establishment from arithmetic proof search.
    machine.blocks[2].operations.clear();
    let Terminator::Jump { arguments, .. } = &mut machine.blocks[2].terminator else {
        panic!("fixture latch is a jump");
    };
    *arguments = vec![id(2, ValueId::new)];
    module.scalar_block_invariants = vec![ScalarBlockInvariant {
        machine: id(1, MachineId::new),
        header: id(2, BlockId::new),
        predicate: range(2, 5),
        arrivals: vec![
            ScalarBlockInvariantArrival {
                edge: id(1, EdgeId::new),
                obligation: id(100, ObligationId::new),
            },
            ScalarBlockInvariantArrival {
                edge: id(4, EdgeId::new),
                obligation: id(101, ObligationId::new),
            },
        ],
    }];
    module
}

fn evidence(module: &TerminalModule) -> ProofBundle {
    let questions = reconstruct_terminal_obligations(module).unwrap();
    let evidence = questions
        .obligations()
        .iter()
        .map(|question| {
            let ReconstructedTerminalObligationOwner::ScalarBlockInvariant { .. } = question.owner
            else {
                panic!("fixture contains only invariant proof obligations");
            };
            let rule = if let Some(index) = question
                .requirements
                .iter()
                .position(|requirement| requirement == &question.obligation.proposition)
            {
                ProofRule::Assumption { index }
            } else {
                let Proposition::Conjunction(members) = &question.obligation.proposition else {
                    panic!("fixture range");
                };
                ProofRule::ConjunctionIntroduction(
                    members
                        .iter()
                        .map(|member| ProofNode {
                            conclusion: member.clone(),
                            rule: ProofRule::SemanticAxiom {
                                index: question
                                    .semantic_axioms
                                    .iter()
                                    .position(|axiom| axiom == member)
                                    .unwrap(),
                            },
                        })
                        .collect(),
                )
            };
            ObligationEvidence {
                obligation: question.obligation.id,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: id(question.obligation.id.get(), EvidenceIdentity::new),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: question.obligation.proposition.clone(),
                        rule,
                    },
                }),
            }
        })
        .collect();
    ProofBundle {
        evidence,
        ..ProofBundle::default()
    }
}

#[test]
fn scalar_block_invariants_require_establishment_and_preservation_without_ranking() {
    let module = module();
    assert_eq!(
        terminal_verifier::maximum_registered_obligation_id(&module).unwrap(),
        101
    );
    let proofs = evidence(&module);
    verify_module(&module, &proofs, &AdmissionProfile::default()).unwrap();
    let questions = reconstruct_terminal_obligations(&module).unwrap();
    assert_eq!(questions.obligations().len(), 2);
    let establishment = questions
        .obligations()
        .iter()
        .find(|question| question.obligation.id == id(100, ObligationId::new))
        .unwrap();
    assert!(
        establishment.semantic_axioms.is_empty(),
        "initial arrival cannot cite the proposed header range"
    );
    for omitted in 0..proofs.evidence.len() {
        let mut changed = proofs.clone();
        changed.evidence.remove(omitted);
        assert!(matches!(
            verify_module(&module, &changed, &AdmissionProfile::default()),
            Err(VerificationError::MissingEvidence(_))
        ));
    }
    let mut reordered = module.clone();
    reordered.machines[0].blocks.reverse();
    verify_module(&reordered, &proofs, &AdmissionProfile::default()).unwrap();
}

#[test]
fn scalar_block_invariants_reject_changed_establishment_preservation_and_bounds() {
    let module = module();
    let proofs = evidence(&module);
    for mutation in 0..4 {
        let mut changed = module.clone();
        match mutation {
            0 => changed.machines[0].contract.requires.clear(),
            1 => {
                let Terminator::Jump { arguments, .. } =
                    &mut changed.machines[0].blocks[2].terminator
                else {
                    panic!("latch jump");
                };
                // Zero is well typed and available, but old evidence proves the header value.
                *arguments = vec![id(3, ValueId::new)];
            }
            2 => changed.scalar_block_invariants[0].predicate = range(2, 4),
            3 => changed.machines[0].contract.requires = vec![range(1, 6)],
            _ => unreachable!(),
        }
        assert!(
            verify_module(&changed, &proofs, &AdmissionProfile::default()).is_err(),
            "mutation {mutation}"
        );
    }
    let mut wrong = proofs.clone();
    wrong.evidence[1].route = wrong.evidence[0].route.clone();
    assert!(verify_module(&module, &wrong, &AdmissionProfile::default()).is_err());
}

#[test]
fn scalar_block_invariant_rosters_bind_exact_headers_scope_and_arrivals() {
    let module = module();
    for mutation in 0..12 {
        let mut changed = module.clone();
        match mutation {
            0 => changed
                .scalar_block_invariants
                .push(changed.scalar_block_invariants[0].clone()),
            1 => changed.scalar_block_invariants[0].header = id(3, BlockId::new),
            2 => changed.scalar_block_invariants[0].predicate = range(3, 5),
            3 => changed.scalar_block_invariants[0]
                .arrivals
                .pop()
                .map(|_| ())
                .unwrap(),
            4 => changed.scalar_block_invariants[0].arrivals.reverse(),
            5 => changed.scalar_block_invariants[0].arrivals[1].edge = id(5, EdgeId::new),
            6 => {
                changed.scalar_block_invariants[0].arrivals[1].obligation =
                    id(100, ObligationId::new)
            }
            7 => changed.scalar_block_invariants[0].machine = id(999, MachineId::new),
            8 => {
                changed.scalar_block_invariants[0].predicate = Proposition::Equal(
                    ScalarTerm::value(
                        id(2, ValueId::new),
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
                    ),
                    ScalarTerm::value(id(2, ValueId::new), ScalarType::Integer(integer())),
                )
            }
            9 => {
                changed.scalar_block_invariants[0].predicate =
                    Proposition::Atom(semantic_vocabulary::PropositionId::new(1).unwrap())
            }
            10 => {
                changed.scalar_block_invariants[0].header = changed.machines[0].entry;
                changed.scalar_block_invariants[0].predicate = Proposition::Truth;
                changed.scalar_block_invariants[0].arrivals.clear();
            }
            11 => {
                changed.scalar_block_invariants[0].predicate = Proposition::Equal(
                    ScalarTerm::boolean_field(
                        PlaceId::new(1).unwrap(),
                        semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    ),
                    ScalarTerm::Boolean(true),
                )
            }
            _ => unreachable!(),
        }
        assert!(validate_module(&changed).is_err(), "mutation {mutation}");
    }
}

#[test]
fn scalar_block_invariant_checks_every_conditional_arrival() {
    let mut module = module();
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(40, ValueId::new),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks[2].terminator = Terminator::Conditional {
        condition: id(40, ValueId::new),
        when_true: SuccessorEdge {
            edge: id(4, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: vec![id(2, ValueId::new)],
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
        when_false: SuccessorEdge {
            edge: id(6, EdgeId::new),
            target: id(2, BlockId::new),
            arguments: vec![id(2, ValueId::new)],
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    };
    assert!(matches!(
        validate_module(&module),
        Err(ModuleError::InvalidScalarBlockInvariantArrivals { .. })
    ));
    module.scalar_block_invariants[0]
        .arrivals
        .push(ScalarBlockInvariantArrival {
            edge: id(6, EdgeId::new),
            obligation: id(102, ObligationId::new),
        });
    let proofs = evidence(&module);
    assert_eq!(proofs.evidence.len(), 3);
    verify_module(&module, &proofs, &AdmissionProfile::default()).unwrap();
    let mut omitted = proofs.clone();
    omitted
        .evidence
        .retain(|proof| proof.obligation != id(102, ObligationId::new));
    assert!(matches!(
        verify_module(&module, &omitted, &AdmissionProfile::default()),
        Err(VerificationError::MissingEvidence(_))
    ));
    let mut invalid = module.clone();
    invalid.machines[0].blocks[2].operations.push(Operation {
        static_reach_binding: None,
        id: id(30, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: id(30, ValueId::new),
            scalar_type: ScalarType::Integer(integer()),
        }),
        kind: OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(6),
        },
    });
    let Terminator::Conditional { when_false, .. } = &mut invalid.machines[0].blocks[2].terminator
    else {
        panic!("conditional latch");
    };
    when_false.arguments[0] = id(30, ValueId::new);
    validate_module(&invalid).unwrap();
    assert!(
        verify_module(&invalid, &proofs, &AdmissionProfile::default()).is_err(),
        "a second arrival cannot replace its actual argument with the header hypothesis"
    );
}

#[test]
fn scalar_block_invariants_are_optional_and_first_arrival_bounds_are_not_imported() {
    let mut module = module();
    module.scalar_block_invariants.clear();
    assert_eq!(
        terminal_verifier::maximum_registered_obligation_id(&module).unwrap(),
        0
    );
    let questions = reconstruct_terminal_obligations(&module).unwrap();
    assert!(questions.obligations().is_empty());
    verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .unwrap();
}

fn acyclic_module() -> TerminalModule {
    let mut module = module();
    let machine = &mut module.machines[0];
    let terminal = machine.blocks[3].terminator.clone();
    machine.blocks.truncate(2);
    machine.blocks[1].operations.clear();
    machine.blocks[1].terminator = terminal;
    machine.parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(40, ValueId::new),
        scalar_type: ScalarType::Boolean,
    });
    // Two distinct actual edges reach the same block, even though both carry
    // the same scalar value. Coverage is by edge, not predecessor block.
    let successor = |edge| SuccessorEdge {
        edge: id(edge, EdgeId::new),
        target: id(2, BlockId::new),
        arguments: vec![id(1, ValueId::new)],
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    machine.blocks[0].terminator = Terminator::Conditional {
        condition: id(40, ValueId::new),
        when_true: successor(1),
        when_false: successor(4),
    };
    module
}

#[test]
fn acyclic_scalar_block_invariants_check_each_selected_actual_and_arrival() {
    let module = acyclic_module();
    let proofs = evidence(&module);
    verify_module(&module, &proofs, &AdmissionProfile::default()).unwrap();
    let questions = reconstruct_terminal_obligations(&module).unwrap();
    assert_eq!(questions.obligations().len(), 2);
    for question in questions.obligations() {
        assert_eq!(question.obligation.proposition, range(1, 5));
        assert!(!question.semantic_axioms.contains(&range(2, 5)));
    }
    let mut missing = module.clone();
    missing.scalar_block_invariants[0].arrivals.pop();
    assert!(matches!(
        validate_module(&missing),
        Err(ModuleError::InvalidScalarBlockInvariantArrivals { .. })
    ));
    let mut wrong_actual = module.clone();
    wrong_actual.machines[0].parameters.push(ValueDeclaration {
        qualifications: Default::default(),
        id: id(41, ValueId::new),
        scalar_type: ScalarType::Integer(integer()),
    });
    let Terminator::Conditional { when_false, .. } =
        &mut wrong_actual.machines[0].blocks[0].terminator
    else {
        panic!("acyclic conditional");
    };
    when_false.arguments[0] = id(41, ValueId::new);
    validate_module(&wrong_actual).unwrap();
    assert!(verify_module(&wrong_actual, &proofs, &AdmissionProfile::default()).is_err());
    for value in [3, 999] {
        let mut wrong_scope = module.clone();
        wrong_scope.scalar_block_invariants[0].predicate = range(value, 5);
        assert!(validate_module(&wrong_scope).is_err());
    }
    // Machine formals are intentionally in scope; they are not forged header
    // parameters and retain the same meaning on each arrival.
    let mut formal = module.clone();
    formal.scalar_block_invariants[0].predicate = range(1, 5);
    let formal_proofs = evidence(&formal);
    verify_module(&formal, &formal_proofs, &AdmissionProfile::default()).unwrap();
}

#[test]
fn empty_fact_arrival_cannot_assume_its_destination_predicate() {
    let mut module = acyclic_module();
    module.machines[0].blocks[0].terminator = Terminator::Jump {
        edge: id(1, EdgeId::new),
        target: id(2, BlockId::new),
        arguments: vec![id(1, ValueId::new)],
        structural_arguments: Vec::new(),
        residual_affine_discards: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    module.scalar_block_invariants[0].arrivals.truncate(1);
    let proofs = evidence(&module);
    module.machines[0].contract.requires.clear();
    let questions = reconstruct_terminal_obligations(&module).unwrap();
    assert_eq!(questions.obligations().len(), 1);
    assert!(questions.obligations()[0].semantic_axioms.is_empty());
    assert!(questions.obligations()[0].requirements.is_empty());
    assert!(verify_module(&module, &proofs, &AdmissionProfile::default()).is_err());
}

#[test]
fn legacy_countdown_block_assertions_do_not_retain_initial_arrival_aliases() {
    let mut module = ranked_countdown();
    module.machines[0].contract.requires = vec![range(1, 5)];
    module.scalar_block_invariants = self::module().scalar_block_invariants;
    let validated = validate_module_for_interpretation(&module).unwrap();
    let questions =
        terminal_verifier::reconstruct_interpretable_terminal_obligations(validated).unwrap();
    let establishment = questions
        .obligations()
        .iter()
        .find(|question| {
            matches!(question.owner, ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
            edge, ..
        } if edge == id(1, EdgeId::new))
        })
        .unwrap();
    assert_eq!(establishment.obligation.proposition, range(1, 5));
    assert!(establishment.semantic_axioms.is_empty());
    let preservation = questions
        .obligations()
        .iter()
        .find(|question| {
            matches!(question.owner, ReconstructedTerminalObligationOwner::ScalarBlockInvariant {
            edge, ..
        } if edge == id(4, EdgeId::new))
        })
        .unwrap();
    assert_eq!(preservation.obligation.proposition, range(6, 5));
    let Proposition::Conjunction(bounds) = range(2, 5) else {
        unreachable!()
    };
    assert!(
        bounds
            .iter()
            .all(|bound| preservation.semantic_axioms.contains(bound))
    );
    let initial_alias = Proposition::Equal(
        ScalarTerm::value(id(2, ValueId::new), ScalarType::Integer(integer())),
        ScalarTerm::value(id(1, ValueId::new), ScalarType::Integer(integer())),
    );
    assert!(
        !preservation.semantic_axioms.contains(&initial_alias),
        "the initial header binding is not an arbitrary-iteration fact"
    );
    let context = semantic_vocabulary::PropositionContext::from_value_types((1..=6).map(|value| {
        (
            id(value, ValueId::new),
            if value == 4 {
                ScalarType::Boolean
            } else {
                ScalarType::Integer(integer())
            },
        )
    }))
    .unwrap();
    assert!(
        proof_admission::accept_certificate(
            &context,
            &preservation.obligation.proposition,
            &preservation.requirements,
            &preservation.semantic_axioms,
            &ProofNode {
                conclusion: preservation.obligation.proposition.clone(),
                rule: ProofRule::Assumption { index: 0 },
            },
        )
        .is_err(),
        "entry bounds alone do not establish the actual next argument"
    );
    for index in 0..preservation.semantic_axioms.len() {
        assert!(
            proof_admission::accept_certificate(
                &context,
                &initial_alias,
                &preservation.requirements,
                &preservation.semantic_axioms,
                &ProofNode {
                    conclusion: initial_alias.clone(),
                    rule: ProofRule::SemanticAxiom { index }
                },
            )
            .is_err(),
            "stale initial alias must not be available at preservation"
        );
    }
}

#[test]
fn block_predicate_facts_split_only_unconditional_conjunctions() {
    let mut module = module();
    let alternative = Proposition::Disjunction(vec![Proposition::Truth, Proposition::Falsehood]);
    let implication = Proposition::Implication {
        premise: Box::new(Proposition::Falsehood),
        conclusion: Box::new(Proposition::Truth),
    };
    module.scalar_block_invariants[0].predicate =
        Proposition::Conjunction(vec![range(2, 5), alternative.clone(), implication.clone()]);
    let questions = reconstruct_terminal_obligations(&module).unwrap();
    let preservation = questions
        .obligations()
        .iter()
        .find(|question| {
            matches!(
                question.owner,
                ReconstructedTerminalObligationOwner::ScalarBlockInvariant { edge, .. }
                    if edge == id(4, EdgeId::new)
            )
        })
        .unwrap();
    let facts = &preservation.semantic_axioms;
    let Proposition::Conjunction(bounds) = range(2, 5) else {
        unreachable!()
    };
    assert!(bounds.iter().all(|bound| facts.contains(bound)));
    assert!(facts.contains(&alternative));
    assert!(facts.contains(&implication));
    assert!(!facts.contains(&Proposition::Falsehood));
    assert!(!facts.contains(&Proposition::Truth));
}
