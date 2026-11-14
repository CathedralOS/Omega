use proof_admission::{ProofNode, ProofRule, check_certificate};
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
