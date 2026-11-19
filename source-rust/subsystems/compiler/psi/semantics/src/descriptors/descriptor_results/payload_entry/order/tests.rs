use super::*;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
};

#[test]
fn exact_range_rearrangement_retains_subject_policy_and_premise() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = PropositionContext::from_value_types((1..=3).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer),
        )
    }))
    .unwrap();
    let value = |identity| {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer),
        )
    };
    let maximum =
        ScalarTerm::integer(integer, IntegerValue::Unsigned(u64::MAX.into()))
            .unwrap();
    let difference = ScalarTerm::ExactIntegerSubtract {
        scalar_type: integer,
        left: Box::new(maximum.clone()),
        right: Box::new(value(2)),
    };
    let premise = Proposition::LessOrEqual(value(1), difference);
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(
            Box::new(IntegerMathTerm::embed(value(1)).unwrap()),
            Box::new(IntegerMathTerm::embed(value(2)).unwrap()),
        ),
        IntegerMathTerm::embed(maximum.clone()).unwrap(),
    );
    let proof =
        super::super::prove(&context, &goal, std::slice::from_ref(&premise), 0)
            .unwrap();
    proof_admission::check_certificate(
        &context,
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &proof,
    )
    .unwrap();
    proof_admission::verify_bounded_certificate(
        &context,
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &proof,
        &mut proof_admission::Budget::default(),
    )
    .unwrap();
    assert!(
        proof_admission::check_certificate(&context, &goal, &[], &[], &proof)
            .is_err()
    );
    let wrong_subject = Proposition::LessOrEqual(
        value(3),
        match &premise {
            Proposition::LessOrEqual(_, right) => right.clone(),
            _ => unreachable!(),
        },
    );
    assert!(super::super::prove(&context, &goal, &[wrong_subject], 0).is_none());
    let wrapping = Proposition::LessOrEqual(
        value(1),
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type: integer,
            left: Box::new(maximum),
            right: Box::new(value(2)),
        },
    );
    assert!(super::super::prove(&context, &goal, &[wrapping], 0).is_none());
}
