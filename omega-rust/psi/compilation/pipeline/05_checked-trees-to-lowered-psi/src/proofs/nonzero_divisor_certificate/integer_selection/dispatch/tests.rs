use std::collections::BTreeSet;

use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
use proof_admission::check_certificate;
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
