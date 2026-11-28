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

#[test]
fn mathematical_strict_order_weakens_across_carriers_only_at_its_own_endpoints() {
    use semantic_vocabulary::IntegerMathTerm;
    let math = |id, bits| IntegerMathTerm::MathValue {
        source_type: IntegerType::new(IntegerSign::Signed, bits).unwrap(),
        value: ValueId::new(id).unwrap(),
    };
    let left = math(1, 16);
    let right = math(2, 32);
    let ctx = context(
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap()),
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
    );
    let premise = Proposition::IntegerMathLessThan(left.clone(), right.clone());
    for (expected, accepted) in [
        (
            Proposition::IntegerMathLessOrEqual(left.clone(), right.clone()),
            true,
        ),
        (
            Proposition::IntegerMathLessOrEqual(right.clone(), left.clone()),
            false,
        ),
        (
            Proposition::IntegerMathLessOrEqual(left, math(3, 16)),
            false,
        ),
    ] {
        let proof = ProofNode {
            conclusion: expected.clone(),
            rule: ProofRule::IntegerOrderWeakening {
                relation: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        assert_eq!(
            check_certificate(&ctx, &expected, std::slice::from_ref(&premise), &[], &proof).is_ok(),
            accepted
        );
    }
}
