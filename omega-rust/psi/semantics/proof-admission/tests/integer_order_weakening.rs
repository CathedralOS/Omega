use proof_admission::{
    AcceptedProofRule, PrimitiveJudgment, ProofNode, ProofRule, accept_certificate,
    check_certificate,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};

fn integer_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap())
}

fn value(identity: u64, scalar_type: ScalarType) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type)
}

fn context(left: ScalarType, right: ScalarType) -> PropositionContext {
    PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), left),
        (ValueId::new(2).unwrap(), right),
        (ValueId::new(3).unwrap(), left),
    ])
    .unwrap()
}

fn proof(premise: Proposition) -> ProofNode {
    let (Proposition::Equal(left, right) | Proposition::LessThan(left, right)) = &premise else {
        unreachable!()
    };
    ProofNode {
        conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
        rule: ProofRule::IntegerOrderWeakening {
            relation: Box::new(ProofNode {
                conclusion: premise,
                rule: ProofRule::Assumption { index: 0 },
            }),
        },
    }
}

#[test]
fn equality_and_strict_order_weaken_with_the_exact_cited_endpoints() {
    let left = value(1, integer_type());
    let right = value(2, integer_type());
    for premise in [
        Proposition::Equal(left.clone(), right.clone()),
        Proposition::LessThan(left, right),
    ] {
        let proof = proof(premise.clone());
        let accepted = accept_certificate(
            &context(integer_type(), integer_type()),
            &proof.conclusion,
            std::slice::from_ref(&premise),
            &[],
            &proof,
        )
        .unwrap();
        assert!(
            accepted
                .rules
                .contains(&AcceptedProofRule::IntegerOrderWeakening)
        );
        assert_eq!(accepted.assumptions.len(), 1);
        assert_eq!(accepted.assumptions[0].proposition, premise);
        assert!(
            check_certificate(
                &context(integer_type(), integer_type()),
                &proof.conclusion,
                &[],
                &[],
                &proof
            )
            .is_err()
        );
    }
}

#[test]
fn reflexive_order_uses_the_existing_reflexive_equality_child() {
    let term = value(1, integer_type());
    let mut proof = proof(Proposition::Equal(term.clone(), term));
    let ProofRule::IntegerOrderWeakening { relation } = &mut proof.rule else {
        unreachable!()
    };
    relation.rule = ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality);
    check_certificate(
        &context(integer_type(), integer_type()),
        &proof.conclusion,
        &[],
        &[],
        &proof,
    )
    .unwrap();
}

#[test]
fn weakening_reuses_only_the_existing_fixed_integer_math_bridge() {
    let premise = Proposition::LessThan(value(1, integer_type()), value(2, integer_type()));
    let mut proof = proof(premise.clone());
    proof.conclusion = proof_admission::lift_fixed_integer_relation(&proof.conclusion).unwrap();
    check_certificate(
        &context(integer_type(), integer_type()),
        &proof.conclusion,
        std::slice::from_ref(&premise),
        &[],
        &proof,
    )
    .unwrap();
    proof.conclusion = proof_admission::lift_fixed_integer_relation(&Proposition::LessOrEqual(
        value(2, integer_type()),
        value(1, integer_type()),
    ))
    .unwrap();
    assert!(
        check_certificate(
            &context(integer_type(), integer_type()),
            &proof.conclusion,
            &[premise],
            &[],
            &proof
        )
        .is_err()
    );
}

#[test]
fn weakening_does_not_reverse_order_or_change_an_endpoint() {
    let left = value(1, integer_type());
    let right = value(2, integer_type());
    for premise in [
        Proposition::Equal(left.clone(), right.clone()),
        Proposition::LessThan(left.clone(), right.clone()),
    ] {
        for conclusion in [
            Proposition::LessOrEqual(right.clone(), left.clone()),
            Proposition::LessOrEqual(left.clone(), value(3, integer_type())),
        ] {
            let mut proof = proof(premise.clone());
            proof.conclusion = conclusion;
            assert!(
                check_certificate(
                    &context(integer_type(), integer_type()),
                    &proof.conclusion,
                    std::slice::from_ref(&premise),
                    &[],
                    &proof
                )
                .is_err()
            );
        }
    }
}

#[test]
fn boolean_and_mixed_carrier_equalities_do_not_establish_integer_order() {
    for (left, right) in [
        (ScalarType::Boolean, ScalarType::Boolean),
        (
            integer_type(),
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
        ),
    ] {
        let premise = Proposition::Equal(value(1, left), value(2, right));
        let proof = proof(premise.clone());
        assert!(
            check_certificate(
                &context(left, right),
                &proof.conclusion,
                &[premise],
                &[],
                &proof
            )
            .is_err()
        );
    }
}
