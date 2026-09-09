use super::*;
use semantic_vocabulary::BoundedIntegerType;
use terminal_psi::{ScalarRangeInvariant, ScalarRangeInvariantArrival};
use terminal_verifier::{ReconstructedTerminalObligationOwner, reconstruct_terminal_obligations};

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
}

fn range(value: u64, maximum: u128) -> Proposition {
    let value = ScalarTerm::value(id(value, ValueId::new), ScalarType::Integer(integer()));
    Proposition::Conjunction(vec![
        Proposition::LessOrEqual(
            ScalarTerm::integer(integer(), IntegerValue::Unsigned(0)).unwrap(),
            value.clone(),
        ),
        Proposition::LessOrEqual(
            value,
            ScalarTerm::integer(integer(), IntegerValue::Unsigned(maximum)).unwrap(),
        ),
    ])
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
    module.scalar_range_invariants = vec![ScalarRangeInvariant {
        machine: id(1, MachineId::new),
        header: id(2, BlockId::new),
        parameter: id(2, ValueId::new),
        bounds: BoundedIntegerType::new(
            integer(),
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(5),
        )
        .unwrap(),
        arrivals: vec![
            ScalarRangeInvariantArrival {
                edge: id(1, EdgeId::new),
                obligation: id(100, ObligationId::new),
            },
            ScalarRangeInvariantArrival {
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
            let ReconstructedTerminalObligationOwner::ScalarRangeInvariant { edge, .. } =
                question.owner
            else {
                panic!("fixture contains only invariant proof obligations");
            };
            let rule = if edge == id(1, EdgeId::new) {
                ProofRule::Assumption { index: 0 }
            } else {
                let Proposition::Conjunction(bounds) = &question.obligation.proposition else {
                    panic!("range obligation is a conjunction");
                };
                ProofRule::ConjunctionIntroduction(
                    bounds
                        .iter()
                        .map(|bound| ProofNode {
                            conclusion: bound.clone(),
                            rule: ProofRule::SemanticAxiom {
                                index: question
                                    .semantic_axioms
                                    .iter()
                                    .position(|axiom| axiom == bound)
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
fn scalar_range_invariants_require_establishment_and_preservation_without_ranking() {
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
fn scalar_range_invariants_reject_changed_establishment_preservation_and_bounds() {
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
            2 => {
                changed.scalar_range_invariants[0].bounds = BoundedIntegerType::new(
                    integer(),
                    IntegerValue::Unsigned(0),
                    IntegerValue::Unsigned(4),
                )
                .unwrap()
            }
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
fn scalar_range_invariant_rosters_bind_exact_headers_parameters_and_arrivals() {
    let module = module();
    for mutation in 0..9 {
        let mut changed = module.clone();
        match mutation {
            0 => changed
                .scalar_range_invariants
                .push(changed.scalar_range_invariants[0].clone()),
            1 => changed.scalar_range_invariants[0].header = id(3, BlockId::new),
            2 => changed.scalar_range_invariants[0].parameter = id(1, ValueId::new),
            3 => changed.scalar_range_invariants[0]
                .arrivals
                .pop()
                .map(|_| ())
                .unwrap(),
            4 => changed.scalar_range_invariants[0].arrivals.reverse(),
            5 => changed.scalar_range_invariants[0].arrivals[1].edge = id(5, EdgeId::new),
            6 => {
                changed.scalar_range_invariants[0].arrivals[1].obligation =
                    id(100, ObligationId::new)
            }
            7 => changed.scalar_range_invariants[0].machine = id(999, MachineId::new),
            8 => {
                changed.scalar_range_invariants[0].bounds = BoundedIntegerType::new(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    IntegerValue::Unsigned(0),
                    IntegerValue::Unsigned(5),
                )
                .unwrap()
            }
            _ => unreachable!(),
        }
        assert!(validate_module(&changed).is_err(), "mutation {mutation}");
    }
}

#[test]
fn scalar_range_invariant_checks_every_conditional_arrival() {
    let mut module = module();
    let machine = &mut module.machines[0];
    machine.parameters.push(ValueDeclaration {
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
        Err(ModuleError::InvalidScalarRangeInvariantArrivals { .. })
    ));
    module.scalar_range_invariants[0]
        .arrivals
        .push(ScalarRangeInvariantArrival {
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
        id: id(30, OperationId::new),
        result: OperationResult::Scalar(ValueDeclaration {
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
fn scalar_range_invariants_are_optional_and_first_arrival_bounds_are_not_imported() {
    let mut module = module();
    module.scalar_range_invariants.clear();
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
