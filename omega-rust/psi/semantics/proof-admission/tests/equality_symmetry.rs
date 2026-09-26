use proof_admission::{
    AcceptedProofRule, ProofNode, ProofRule, accept_certificate, check_certificate,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};

fn fixture(scalar_type: ScalarType) -> (PropositionContext, Proposition, ProofNode) {
    let context = PropositionContext::from_value_types(
        [1, 256, 257].map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
    )
    .unwrap();
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
    let premise = Proposition::Equal(value(256), value(1));
    let proof = ProofNode {
        conclusion: Proposition::Equal(value(1), value(256)),
        rule: ProofRule::EqualitySymmetry {
            equality: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    (context, premise, proof)
}

#[test]
fn scalar_symmetry_rechecks_the_exact_child_and_records_its_citation() {
    for scalar_type in [
        ScalarType::Boolean,
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap()),
    ] {
        let (context, premise, proof) = fixture(scalar_type);
        let accepted = accept_certificate(
            &context,
            &proof.conclusion,
            &[],
            std::slice::from_ref(&premise),
            &proof,
        )
        .unwrap();
        assert!(
            accepted
                .rules
                .contains(&AcceptedProofRule::EqualitySymmetry)
        );
        assert_eq!(accepted.semantic_axioms.len(), 1);
        assert_eq!(accepted.semantic_axioms[0].proposition, premise);
        assert!(check_certificate(&context, &proof.conclusion, &[], &[], &proof).is_err());
        assert!(
            check_certificate(
                &context,
                &proof.conclusion,
                &[],
                std::slice::from_ref(&proof.conclusion),
                &proof
            )
            .is_err()
        );
        let mut changed = proof.clone();
        changed.conclusion = Proposition::Equal(
            ScalarTerm::value(ValueId::new(257).unwrap(), scalar_type),
            ScalarTerm::value(ValueId::new(256).unwrap(), scalar_type),
        );
        assert!(
            check_certificate(&context, &changed.conclusion, &[], &[premise], &changed).is_err()
        );
    }
}

#[test]
fn scalar_symmetry_accepts_projected_equality_but_not_an_unproved_child() {
    let (context, premise, mut proof) = fixture(ScalarType::Boolean);
    let assumptions = [Proposition::Conjunction(vec![
        Proposition::Truth,
        premise.clone(),
    ])];
    let ProofRule::EqualitySymmetry { equality } = &mut proof.rule else {
        unreachable!()
    };
    equality.rule = ProofRule::ConjunctionElimination {
        conjunction: Box::new(ProofNode {
            conclusion: assumptions[0].clone(),
            rule: ProofRule::Assumption { index: 0 },
        }),
        conjunct: 1,
    };
    check_certificate(&context, &proof.conclusion, &assumptions, &[], &proof).unwrap();
    assert!(
        check_certificate(
            &context,
            &proof.conclusion,
            &[Proposition::Truth],
            &[],
            &proof
        )
        .is_err()
    );
}

#[test]
fn scalar_symmetry_does_not_turn_a_different_relation_into_equality() {
    let (context, _, mut proof) = fixture(ScalarType::Boolean);
    let ProofRule::EqualitySymmetry { equality } = &mut proof.rule else {
        unreachable!()
    };
    equality.conclusion = Proposition::Truth;
    equality.rule = ProofRule::Primitive(proof_admission::PrimitiveJudgment::Truth);
    assert!(check_certificate(&context, &proof.conclusion, &[], &[], &proof).is_err());
}
