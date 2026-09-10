use super::*;
use terminal_psi::ContractClause;
use terminal_verifier::reconstruct_terminal_obligations;

fn successor(edge: u64, target: u64) -> SuccessorEdge {
    SuccessorEdge {
        edge: id(edge, EdgeId::new),
        target: id(target, BlockId::new),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

fn block(identity: u64, terminator: Terminator) -> Block {
    Block {
        id: id(identity, BlockId::new),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator,
    }
}

fn return_value(edge: u64, value: u64) -> Terminator {
    Terminator::Return {
        edge: id(edge, EdgeId::new),
        value: id(value, ValueId::new),
        cleanup_actions: Vec::new(),
    }
}

fn jump(edge: u64, target: u64) -> Terminator {
    Terminator::Jump {
        edge: id(edge, EdgeId::new),
        target: id(target, BlockId::new),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    }
}

fn result_equals(value: u64) -> Proposition {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    Proposition::Equal(
        ScalarTerm::value(id(14, ValueId::new), scalar_type),
        ScalarTerm::value(id(value, ValueId::new), scalar_type),
    )
}

fn two_returns(late_value: u64) -> TerminalModule {
    let mut module = unranked_scalar_cycle();
    let machine = &mut module.machines[0];
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap());
    machine.parameters = vec![
        ValueDeclaration {
            qualifications: Default::default(),
            id: id(10, ValueId::new),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: id(11, ValueId::new),
            scalar_type: ScalarType::Boolean,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: id(12, ValueId::new),
            scalar_type,
        },
        ValueDeclaration {
            qualifications: Default::default(),
            id: id(13, ValueId::new),
            scalar_type,
        },
    ];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        qualifications: Default::default(),
        id: id(14, ValueId::new),
        scalar_type,
    });
    // The late return deliberately sorts before its cyclic predecessor. The
    // Kahn prefix contains only entry and early return, in that order.
    machine.blocks = vec![
        block(
            1,
            Terminator::Conditional {
                condition: id(10, ValueId::new),
                when_true: successor(1, 4),
                when_false: successor(2, 3),
            },
        ),
        block(2, return_value(3, late_value)),
        block(
            3,
            Terminator::Conditional {
                condition: id(11, ValueId::new),
                when_true: successor(4, 3),
                when_false: successor(5, 2),
            },
        ),
        block(4, return_value(6, 12)),
    ];
    machine.contract.ensures = vec![ContractClause {
        obligation: id(1, ObligationId::new),
        proposition: result_equals(12),
    }];
    module
}

fn return_equality_certificate(value: u64) -> ProofBundle {
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: id(1, ObligationId::new),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: id(1, EvidenceIdentity::new),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: result_equals(value),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                },
            }),
        }],
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
    }
}

#[test]
fn late_return_prevents_an_early_return_equality_becoming_an_all_return_fact() {
    for reverse_storage in [false, true] {
        let mut module = two_returns(13);
        if reverse_storage {
            module.machines[0].blocks.reverse();
        }
        let questions = reconstruct_terminal_obligations(&module).unwrap();
        let [guarantee] = questions.obligations() else {
            panic!("the published guarantee remains an obligation")
        };
        assert_eq!(guarantee.obligation.proposition, result_equals(12));
        assert!(
            guarantee.semantic_axioms.is_empty(),
            "neither distinct return equality holds on every normal return: {:?}",
            guarantee.semantic_axioms
        );
    }
}

#[test]
fn forged_early_return_guarantee_rejects_when_the_late_return_differs() {
    let module = two_returns(13);
    // With both Boolean inputs false and values 12 and 13 unequal, execution
    // returns value 13. The old Kahn prefix nevertheless supplied result=12
    // as semantic axiom zero and accepted this one-node certificate.
    let result = verify_module(
        &module,
        &return_equality_certificate(12),
        &AdmissionProfile::default(),
    );
    assert!(
        matches!(result, Err(VerificationError::RejectedEvidence { obligation, .. })
            if obligation == id(1, ObligationId::new)),
        "the false all-return guarantee must reject: {result:?}"
    );
}

#[test]
fn common_return_equality_survives_a_cyclic_residual_and_storage_reordering() {
    for reverse_storage in [false, true] {
        let mut module = two_returns(12);
        if reverse_storage {
            module.machines[0].blocks.reverse();
        }
        let questions = reconstruct_terminal_obligations(&module).unwrap();
        assert_eq!(
            questions.obligations()[0].semantic_axioms,
            vec![result_equals(12)]
        );
        verify_module(
            &module,
            &return_equality_certificate(12),
            &AdmissionProfile::default(),
        )
        .expect("the same value is returned on every normal exit");
    }
}

#[test]
fn the_only_return_behind_a_cycle_retains_its_own_result_identity() {
    let mut module = two_returns(13);
    let machine = &mut module.machines[0];
    machine.blocks[0].terminator = jump(1, 3);
    machine
        .blocks
        .retain(|block| block.id != id(4, BlockId::new));
    machine.contract.ensures[0].proposition = result_equals(13);
    let questions = reconstruct_terminal_obligations(&module).unwrap();
    assert_eq!(
        questions.obligations()[0].semantic_axioms,
        vec![result_equals(13)]
    );
    verify_module(
        &module,
        &return_equality_certificate(13),
        &AdmissionProfile::default(),
    )
    .expect("a residual return supplies its own result equality");
}

#[test]
fn an_infinite_component_adds_no_normal_return_path() {
    let mut module = two_returns(13);
    let machine = &mut module.machines[0];
    machine
        .blocks
        .retain(|block| block.id != id(2, BlockId::new));
    machine
        .blocks
        .iter_mut()
        .find(|block| block.id == id(3, BlockId::new))
        .unwrap()
        .terminator = jump(4, 3);
    let questions = reconstruct_terminal_obligations(&module).unwrap();
    assert_eq!(
        questions.obligations()[0].semantic_axioms,
        vec![result_equals(12)]
    );
    verify_module(
        &module,
        &return_equality_certificate(12),
        &AdmissionProfile::default(),
    )
    .expect("partial correctness does not require the cyclic component to terminate");
}
