//! Relative progress uses the current guarded word, never its desired rank.

use std::collections::BTreeSet;

use proof_admission::check_certificate;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;

#[test]
fn relative_policy_translation_canonically_proves_guarded_wrapping_descent() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let context =
        PropositionContext::from_value_types((1..=3).map(|id| (ValueId::new(id).unwrap(), scalar)))
            .unwrap();
    let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), scalar);
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
    let source = value(1);
    let defined = value(3);
    let direct =
        ScalarTerm::wrapping_integer_subtract(integer, source.clone(), literal(1)).unwrap();
    let equations = [
        Proposition::Equal(value(2), literal(1)),
        Proposition::Equal(
            defined.clone(),
            ScalarTerm::wrapping_integer_subtract(integer, source.clone(), value(2)).unwrap(),
        ),
    ];
    let requirements = [Proposition::LessOrEqual(literal(1), source.clone())];
    for target in [defined, direct] {
        let goal = Proposition::LessThan(target, source.clone());
        let produce = |requirements: &[Proposition], equations: &[Proposition]| {
            produce_checked_canonical_integer_proof(
                &context,
                &goal,
                requirements,
                equations,
                &BTreeSet::from([ValueId::new(1).unwrap()]),
                &mut crate::proofs::ProofSearchDeclines::default(),
            )
        };
        let proof = produce(&requirements, &equations)
            .expect("canonical custody proves guarded relative progress");
        check_certificate(&context, &goal, &requirements, &equations, &proof).unwrap();
        assert!(
            produce(&[], &equations).is_none(),
            "zero can wrap to MAX without the guard"
        );
        assert!(check_certificate(&context, &goal, &[], &equations, &proof).is_err());
        let wrong_guard = [Proposition::LessOrEqual(literal(0), source.clone())];
        assert!(produce(&wrong_guard, &equations).is_none());
    }
}

#[test]
fn relative_policy_translation_preserves_signed_ascent_and_exact_definition_custody() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let context =
        PropositionContext::from_value_types((1..=3).map(|id| (ValueId::new(id).unwrap(), scalar)))
            .unwrap();
    let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), scalar);
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Signed(number)).unwrap();
    let equations = [Proposition::Equal(
        value(2),
        ScalarTerm::wrapping_integer_add(integer, value(1), literal(1)).unwrap(),
    )];
    let requirements = [Proposition::LessOrEqual(value(1), literal(126))];
    let goal = Proposition::LessThan(value(1), value(2));
    let produce = |requirements: &[Proposition], equations: &[Proposition]| {
        produce_checked_canonical_integer_proof(
            &context,
            &goal,
            requirements,
            equations,
            &BTreeSet::from([ValueId::new(1).unwrap()]),
            &mut crate::proofs::ProofSearchDeclines::default(),
        )
    };
    let proof =
        produce(&requirements, &equations).expect("positive guarded displacement proves ascent");
    check_certificate(&context, &goal, &requirements, &equations, &proof).unwrap();
    assert!(produce(&[], &equations).is_none());
    assert!(produce(&requirements, &[]).is_none());
    let wrong = [Proposition::Equal(
        value(2),
        ScalarTerm::wrapping_integer_add(integer, value(3), literal(1)).unwrap(),
    )];
    assert!(produce(&requirements, &wrong).is_none());
    assert!(check_certificate(&context, &goal, &requirements, &wrong, &proof).is_err());
}
