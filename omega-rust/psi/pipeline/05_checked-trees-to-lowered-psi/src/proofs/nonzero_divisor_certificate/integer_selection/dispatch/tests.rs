use std::collections::BTreeSet;

use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
use proof_admission::{ProofRule, check_certificate};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
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

fn wide_context(integer_type: IntegerType, identities: u64) -> PropositionContext {
    PropositionContext::from_value_types((1..=identities).map(|identity| {
        (
            ValueId::new(identity).expect("value identity"),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("typed value context")
}

fn math_value(identity: u64, integer_type: IntegerType) -> IntegerMathTerm {
    IntegerMathTerm::MathValue {
        source_type: integer_type,
        value: ValueId::new(identity).expect("value identity"),
    }
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

/// dice_roller shape: `state * 1103` over literal-defined i32 operands. The
/// landed equalities orient into both endpoints, so the multiply carrier
/// bounds produce without a cited range contract.
#[test]
fn multiply_carrier_bounds_land_on_literal_defined_operands() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for (width, left_lit, right_lit) in [(8, 7_u64, 11_u64), (32, 42, 1103), (64, 42, 1103)] {
            let integer_type = IntegerType::new(sign, width).expect("fixed integer type");
            let context = context(integer_type);
            let axioms = [
                Proposition::Equal(value(1, integer_type), literal(left_lit, integer_type)),
                Proposition::Equal(value(2, integer_type), literal(right_lit, integer_type)),
            ];
            let product = IntegerMathTerm::Multiply(
                Box::new(math_value(1, integer_type)),
                Box::new(math_value(2, integer_type)),
            );
            for goal in [
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(integer_type.minimum_value()),
                    product.clone(),
                ),
                Proposition::IntegerMathLessOrEqual(
                    product.clone(),
                    IntegerMathTerm::literal(integer_type.maximum_value()),
                ),
            ] {
                let proof = produce_checked_canonical_integer_proof(
                    &context,
                    &goal,
                    &[],
                    &axioms,
                    &BTreeSet::new(),
                )
                .unwrap_or_else(|| panic!("{sign:?}{width}: literal multiply bound {goal:?}"));
                assert_eq!(proof.conclusion, goal);
                check_certificate(&context, &goal, &[], &axioms, &proof)
                    .expect("independent kernel checks literal multiply bound");
            }
        }
    }
}

/// dice_roller shape: `d6 * m` where `d6` is a remainder-defined operand. The
/// remainder's total image bounds the operand without an external contract.
#[test]
fn multiply_carrier_bounds_land_on_remainder_defined_operands() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for width in [8, 32, 64] {
            let integer_type = IntegerType::new(sign, width).expect("fixed integer type");
            let context = wide_context(integer_type, 4);
            let axioms = [
                Proposition::Equal(value(1, integer_type), literal(90, integer_type)),
                Proposition::Equal(value(2, integer_type), literal(6, integer_type)),
                Proposition::Equal(
                    value(3, integer_type),
                    ScalarTerm::exact_integer_remainder(
                        integer_type,
                        value(1, integer_type),
                        value(2, integer_type),
                    )
                    .expect("exact remainder"),
                ),
                Proposition::Equal(value(4, integer_type), literal(10, integer_type)),
            ];
            let product = IntegerMathTerm::Multiply(
                Box::new(math_value(3, integer_type)),
                Box::new(math_value(4, integer_type)),
            );
            for goal in [
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(integer_type.minimum_value()),
                    product.clone(),
                ),
                Proposition::IntegerMathLessOrEqual(
                    product.clone(),
                    IntegerMathTerm::literal(integer_type.maximum_value()),
                ),
            ] {
                let proof = produce_checked_canonical_integer_proof(
                    &context,
                    &goal,
                    &[],
                    &axioms,
                    &BTreeSet::new(),
                )
                .unwrap_or_else(|| panic!("{sign:?}{width}: remainder multiply bound {goal:?}"));
                assert_eq!(proof.conclusion, goal);
                check_certificate(&context, &goal, &[], &axioms, &proof)
                    .expect("independent kernel checks remainder multiply bound");
            }
        }
    }
}

/// dice_roller shape: `v4 + 12345` where `v4 = v2 * v3` is a multiply-defined
/// operand. The multiply definition witness maps its landed literal
/// predecessors into oriented operand endpoints for the add bound.
#[test]
fn add_carrier_bounds_land_on_computed_multiply_operands() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for (width, left_lit, right_lit) in [(8, 3_u64, 11_u64), (32, 42, 1103), (64, 42, 1103)] {
            let integer_type = IntegerType::new(sign, width).expect("fixed integer type");
            let context = wide_context(integer_type, 5);
            let axioms = [
                Proposition::Equal(value(2, integer_type), literal(left_lit, integer_type)),
                Proposition::Equal(value(3, integer_type), literal(right_lit, integer_type)),
                Proposition::Equal(
                    value(4, integer_type),
                    ScalarTerm::exact_integer_multiply(
                        integer_type,
                        value(2, integer_type),
                        value(3, integer_type),
                    )
                    .expect("exact multiply"),
                ),
                Proposition::Equal(value(5, integer_type), literal(90, integer_type)),
            ];
            let sum = IntegerMathTerm::Add(
                Box::new(math_value(4, integer_type)),
                Box::new(math_value(5, integer_type)),
            );
            for goal in [
                Proposition::IntegerMathLessOrEqual(
                    IntegerMathTerm::literal(integer_type.minimum_value()),
                    sum.clone(),
                ),
                Proposition::IntegerMathLessOrEqual(
                    sum.clone(),
                    IntegerMathTerm::literal(integer_type.maximum_value()),
                ),
            ] {
                let proof = produce_checked_canonical_integer_proof(
                    &context,
                    &goal,
                    &[],
                    &axioms,
                    &BTreeSet::new(),
                )
                .unwrap_or_else(|| panic!("{sign:?}{width}: computed multiply addend {goal:?}"));
                assert_eq!(proof.conclusion, goal);
                check_certificate(&context, &goal, &[], &axioms, &proof)
                    .expect("independent kernel checks computed multiply addend");
            }
        }
    }
}
