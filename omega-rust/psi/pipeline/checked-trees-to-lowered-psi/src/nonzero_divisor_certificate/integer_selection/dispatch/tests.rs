use std::collections::BTreeSet;

use crate::produce_checked_canonical_integer_proof;
use proof_admission::{ProofRule, check_certificate};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

fn value(identity: u64, integer_type: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).expect("value identity"),
        ScalarType::Integer(integer_type),
    )
}

fn literal(number: u64, integer_type: IntegerType) -> ScalarTerm {
    let number = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(i128::from(number)),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(number)),
    };
    ScalarTerm::integer(integer_type, number).expect("small fitting literal")
}

fn context(integer_type: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types((1..=3).map(|identity| {
        (
            ValueId::new(identity).expect("value identity"),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("typed value context")
}

#[test]
fn non_strict_call_requirements_reuse_exact_strict_endpoint_certificates() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for width in [8, 16, 32, 64] {
            let integer_type = IntegerType::new(sign, width).expect("fixed integer type");
            let goal = Proposition::LessOrEqual(value(1, integer_type), value(2, integer_type));
            for (left, right, admitted) in [(1, 2, true), (1, 1, true), (2, 1, false)] {
                let axioms = [
                    Proposition::Equal(value(1, integer_type), literal(left, integer_type)),
                    Proposition::Equal(value(2, integer_type), literal(right, integer_type)),
                ];
                let proof = produce_checked_canonical_integer_proof(
                    &context(integer_type),
                    &goal,
                    &[],
                    &axioms,
                    &BTreeSet::new(),
                );
                assert_eq!(
                    proof.is_some(),
                    admitted,
                    "{sign:?}{width}: {left} <= {right}"
                );
                if let Some(proof) = proof {
                    assert_eq!(proof.conclusion, goal);
                    check_certificate(&context(integer_type), &goal, &[], &axioms, &proof)
                        .expect("independent kernel checks original <= goal");
                    if left < right {
                        assert!(matches!(
                            proof.rule,
                            ProofRule::IntegerOrderWeakening { .. }
                        ));
                    }
                }
            }
        }
    }
}

#[test]
fn non_strict_endpoint_transport_keeps_nested_and_reversed_citation_custody() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let goal = Proposition::LessOrEqual(value(1, integer_type), value(2, integer_type));
    let axioms = [
        Proposition::Equal(literal(1, integer_type), value(3, integer_type)),
        Proposition::Conjunction(vec![
            Proposition::Equal(value(3, integer_type), value(1, integer_type)),
            Proposition::Equal(value(2, integer_type), literal(2, integer_type)),
        ]),
    ];
    let proof = produce_checked_canonical_integer_proof(
        &context(integer_type),
        &goal,
        &[],
        &axioms,
        &BTreeSet::new(),
    )
    .expect("both endpoint equality chains retained");
    check_certificate(&context(integer_type), &goal, &[], &axioms, &proof)
        .expect("kernel replays all endpoint citations");
    for omitted in 0..axioms.len() {
        let incomplete = axioms
            .iter()
            .enumerate()
            .filter(|(position, _)| *position != omitted)
            .map(|(_, axiom)| axiom.clone())
            .collect::<Vec<_>>();
        assert!(
            produce_checked_canonical_integer_proof(
                &context(integer_type),
                &goal,
                &[],
                &incomplete,
                &BTreeSet::new(),
            )
            .is_none()
        );
        assert!(
            check_certificate(&context(integer_type), &goal, &[], &incomplete, &proof).is_err()
        );
    }
    let opposite = Proposition::LessOrEqual(value(2, integer_type), value(1, integer_type));
    assert!(
        produce_checked_canonical_integer_proof(
            &context(integer_type),
            &opposite,
            &[],
            &axioms,
            &BTreeSet::new(),
        )
        .is_none()
    );
    let mut corrupted = proof.clone();
    corrupted.conclusion = opposite.clone();
    assert!(
        check_certificate(&context(integer_type), &opposite, &[], &axioms, &corrupted).is_err()
    );
}

#[test]
fn non_strict_endpoint_certificates_cannot_change_the_value_carrier() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let other_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let goal = Proposition::LessOrEqual(value(1, integer_type), value(2, integer_type));
    let axioms = [
        Proposition::Equal(value(1, integer_type), literal(1, integer_type)),
        Proposition::Equal(value(2, integer_type), literal(2, integer_type)),
    ];
    let wrong_context = PropositionContext::from_value_types([
        (
            ValueId::new(1).expect("left identity"),
            ScalarType::Integer(integer_type),
        ),
        (
            ValueId::new(2).expect("right identity"),
            ScalarType::Integer(other_type),
        ),
    ])
    .expect("mixed carrier context");
    assert!(
        produce_checked_canonical_integer_proof(
            &wrong_context,
            &goal,
            &[],
            &axioms,
            &BTreeSet::new(),
        )
        .is_none()
    );
    let mixed = Proposition::LessOrEqual(value(1, integer_type), value(2, other_type));
    assert!(
        produce_checked_canonical_integer_proof(
            &wrong_context,
            &mixed,
            &[],
            &axioms,
            &BTreeSet::new(),
        )
        .is_none()
    );
}
