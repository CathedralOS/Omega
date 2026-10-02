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
fn mathematical_equalities_replay_open_identities_without_extra_assumptions() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = wide_context(integer, 3);
    let add = |left, right| IntegerMathTerm::Add(Box::new(left), Box::new(right));
    let subtract = |left, right| IntegerMathTerm::Subtract(Box::new(left), Box::new(right));
    let one = || IntegerMathTerm::literal(IntegerValue::Unsigned(1));
    let original = subtract(
        add(math_value(1, integer), math_value(2, integer)),
        math_value(3, integer),
    );
    let advanced = subtract(
        add(add(math_value(1, integer), one()), math_value(2, integer)),
        add(math_value(3, integer), one()),
    );
    let equality = |left, right| {
        if left <= right {
            Proposition::IntegerMathEqual(left, right)
        } else {
            Proposition::IntegerMathEqual(right, left)
        }
    };
    let produce = |context: &PropositionContext, goal: &Proposition| {
        produce_checked_canonical_integer_proof(
            context,
            goal,
            &[],
            &[],
            &BTreeSet::new(),
            &mut crate::proofs::ProofSearchDeclines::default(),
        )
    };
    for goal in [
        equality(math_value(1, integer), math_value(1, integer)),
        equality(original.clone(), original.clone()),
        equality(original.clone(), advanced),
    ] {
        let proof = produce(&context, &goal).expect("checked open mathematical identity");
        check_certificate(&context, &goal, &[], &[], &proof).unwrap();
        assert!(
            check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof).is_err()
        );
        let changed_carrier = wide_context(IntegerType::new(IntegerSign::Signed, 64).unwrap(), 3);
        assert!(check_certificate(&changed_carrier, &goal, &[], &[], &proof).is_err());
        assert!(produce(&changed_carrier, &goal).is_none());
        let changed = equality(original.clone(), add(original.clone(), one()));
        assert!(check_certificate(&context, &changed, &[], &[], &proof).is_err());
    }
    for goal in [
        equality(math_value(1, integer), math_value(2, integer)),
        equality(original.clone(), add(original, one())),
    ] {
        assert!(
            produce(&context, &goal).is_none(),
            "unequal terms are not identities"
        );
    }
}

#[test]
fn address_distance_from_zero_uses_carrier_bounds_and_cited_zero_identity() {
    for bits in [16, 32, 64, 128] {
        let address = IntegerType::address(bits).unwrap();
        let context = wide_context(address, 2);
        let difference = IntegerMathTerm::Subtract(
            Box::new(math_value(1, address)),
            Box::new(math_value(2, address)),
        );
        let axioms = [Proposition::Equal(value(2, address), literal(0, address))];
        for (lower, bound) in [(true, 0_u128), (false, u64::MAX as u128)] {
            let bound = IntegerMathTerm::literal(IntegerValue::Unsigned(bound));
            let goal = if lower {
                Proposition::IntegerMathLessOrEqual(bound, difference.clone())
            } else {
                Proposition::IntegerMathLessOrEqual(difference.clone(), bound)
            };
            let produce = |axioms: &[Proposition]| {
                produce_checked_canonical_integer_proof(
                    &context,
                    &goal,
                    &[],
                    axioms,
                    &BTreeSet::new(),
                    &mut crate::proofs::ProofSearchDeclines::default(),
                )
            };
            if bits > 64 && !lower {
                assert!(
                    produce(&axioms).is_none(),
                    "wide address fit is not implied"
                );
                continue;
            }
            let proof = produce(&axioms).expect("distance fits through its retained address model");
            check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
            assert!(check_certificate(&context, &goal, &[], &[], &proof).is_err());
            let changed_zero = [Proposition::Equal(value(2, address), literal(1, address))];
            assert!(check_certificate(&context, &goal, &[], &changed_zero, &proof).is_err());
            assert!(
                check_certificate(&PropositionContext::default(), &goal, &[], &axioms, &proof)
                    .is_err()
            );
        }
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
            &mut crate::proofs::ProofSearchDeclines::default(),
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
            &mut crate::proofs::ProofSearchDeclines::default(),
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
                    &mut crate::proofs::ProofSearchDeclines::default(),
                )
                .unwrap_or_else(|| panic!("{sign:?}{width}: computed multiply addend {goal:?}"));
                assert_eq!(proof.conclusion, goal);
                check_certificate(&context, &goal, &[], &axioms, &proof)
                    .expect("independent kernel checks computed multiply addend");
            }
        }
    }
}
