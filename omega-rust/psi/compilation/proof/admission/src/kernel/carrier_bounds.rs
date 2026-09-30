use super::{PrimitiveJudgment, Proposition, PropositionContext, ScalarTerm, decide_primitive};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ScalarType, ValueId,
};

#[test]
fn fixed_integer_carrier_bounds_use_exact_declared_sign_width_and_context() {
    for bits in 1..=128 {
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            let integer = IntegerType::new(sign, bits).unwrap();
            let scalar = ScalarType::Integer(integer);
            let identity = ValueId::new(1).unwrap();
            let value = ScalarTerm::value(identity, scalar);
            let context = PropositionContext::from_value_types([(identity, scalar)]).unwrap();
            let minimum = ScalarTerm::integer(integer, integer.minimum_value()).unwrap();
            let maximum = ScalarTerm::integer(integer, integer.maximum_value()).unwrap();
            for goal in [
                Proposition::LessOrEqual(minimum.clone(), value.clone()),
                Proposition::LessOrEqual(value.clone(), maximum.clone()),
            ] {
                decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound).unwrap();
                assert!(
                    decide_primitive(
                        &PropositionContext::default(),
                        &goal,
                        PrimitiveJudgment::IntegerCarrierBound
                    )
                    .is_err()
                );
            }
            for goal in [
                Proposition::LessThan(minimum.clone(), value.clone()),
                Proposition::LessThan(value.clone(), maximum.clone()),
                Proposition::LessOrEqual(maximum.clone(), value.clone()),
                Proposition::LessOrEqual(value.clone(), minimum.clone()),
                Proposition::Equal(minimum, value.clone()),
                Proposition::LessOrEqual(value.clone(), value),
            ] {
                assert!(
                    decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn carrier_bounds_do_not_bless_wrong_types_addresses_or_unevaluated_arithmetic() {
    let identity = ValueId::new(1).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = ScalarTerm::value(identity, scalar);
    let zero = ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap();
    let goal = Proposition::LessOrEqual(zero.clone(), value.clone());
    for wrong in [
        ScalarType::Boolean,
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
    ] {
        let context = PropositionContext::from_value_types([(identity, wrong)]).unwrap();
        assert!(decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound).is_err());
    }
    let address = IntegerType::address(64).unwrap();
    let context =
        PropositionContext::from_value_types([(identity, ScalarType::Integer(address))]).unwrap();
    let address_goal = Proposition::LessOrEqual(
        ScalarTerm::integer(address, IntegerValue::Unsigned(0)).unwrap(),
        ScalarTerm::value(identity, ScalarType::Integer(address)),
    );
    assert!(
        decide_primitive(
            &context,
            &address_goal,
            PrimitiveJudgment::IntegerCarrierBound
        )
        .is_err()
    );
    let context = PropositionContext::from_value_types([(identity, scalar)]).unwrap();
    let arithmetic = ScalarTerm::ExactIntegerSubtract {
        scalar_type: integer,
        left: Box::new(zero.clone()),
        right: Box::new(value),
    };
    assert!(
        decide_primitive(
            &context,
            &Proposition::LessOrEqual(zero, arithmetic),
            PrimitiveJudgment::IntegerCarrierBound
        )
        .is_err()
    );
}

#[test]
fn mathematical_address_bounds_retain_the_exact_model_and_admit_evidence() {
    for bits in [16, 32, 64, 128] {
        let address = IntegerType::address(bits).unwrap();
        let identity = ValueId::new(1).unwrap();
        let context =
            PropositionContext::from_value_types([(identity, ScalarType::Integer(address))])
                .unwrap();
        let value = IntegerMathTerm::MathValue {
            source_type: address,
            value: identity,
        };
        let minimum = IntegerMathTerm::literal(address.minimum_value());
        let maximum = IntegerMathTerm::literal(address.maximum_value());
        for goal in [
            Proposition::IntegerMathLessOrEqual(minimum.clone(), value.clone()),
            Proposition::IntegerMathLessOrEqual(value.clone(), maximum.clone()),
        ] {
            let proof = crate::ProofNode {
                conclusion: goal.clone(),
                rule: crate::ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
            };
            crate::accept_certificate(&context, &goal, &[], &[], &proof).unwrap();
            assert!(
                decide_primitive(
                    &PropositionContext::default(),
                    &goal,
                    PrimitiveJudgment::IntegerCarrierBound,
                )
                .is_err()
            );
        }
        for goal in [
            Proposition::IntegerMathLessOrEqual(maximum.clone(), value.clone()),
            Proposition::IntegerMathLessOrEqual(value.clone(), minimum.clone()),
            Proposition::IntegerMathLessThan(minimum.clone(), value.clone()),
            Proposition::IntegerMathLessThan(value.clone(), maximum),
            Proposition::IntegerMathEqual(minimum.clone(), value.clone()),
            Proposition::IntegerMathLessOrEqual(value.clone(), value.clone()),
            Proposition::IntegerMathLessOrEqual(
                minimum.clone(),
                IntegerMathTerm::Subtract(Box::new(value), Box::new(minimum)),
            ),
        ] {
            assert!(
                decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound).is_err(),
                "the carrier judgment must not establish {goal:?}",
            );
        }
    }
}

#[test]
fn mathematical_address_bounds_reject_carrier_identity_drift_and_count_narrowing() {
    let address = IntegerType::address(128).unwrap();
    let identity = ValueId::new(1).unwrap();
    let value = IntegerMathTerm::MathValue {
        source_type: address,
        value: identity,
    };
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::literal(address.minimum_value()),
        value.clone(),
    );
    for (declared_identity, declared_type) in [
        (
            identity,
            ScalarType::Integer(IntegerType::address(64).unwrap()),
        ),
        (
            identity,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 128).unwrap()),
        ),
        (identity, ScalarType::Boolean),
        (ValueId::new(2).unwrap(), ScalarType::Integer(address)),
    ] {
        let context =
            PropositionContext::from_value_types([(declared_identity, declared_type)]).unwrap();
        assert!(decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound).is_err());
    }
    let context =
        PropositionContext::from_value_types([(identity, ScalarType::Integer(address))]).unwrap();
    let count_maximum = IntegerMathTerm::literal(IntegerValue::Unsigned(u128::from(u64::MAX)));
    let narrowed = Proposition::IntegerMathLessOrEqual(value, count_maximum);
    assert!(decide_primitive(&context, &narrowed, PrimitiveJudgment::IntegerCarrierBound).is_err());
}
