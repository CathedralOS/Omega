use super::{Proposition, PropositionContext, ScalarTerm, prove};
use proof_admission::check_certificate;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType, ValueId};

fn integer_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer_type()),
    )
}

fn literal(number: u128) -> ScalarTerm {
    ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(number)).unwrap()
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types((1..=6).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }))
    .unwrap()
}

fn operation_axioms(remainder: bool) -> Vec<Proposition> {
    let operation = if remainder {
        ScalarTerm::ExactIntegerRemainder {
            scalar_type: integer_type(),
            left: Box::new(value(1)),
            right: Box::new(value(2)),
        }
    } else {
        ScalarTerm::ExactIntegerDivide {
            scalar_type: integer_type(),
            left: Box::new(value(1)),
            right: Box::new(value(2)),
        }
    };
    vec![
        Proposition::Equal(value(2), literal(256)),
        Proposition::Equal(value(3), operation),
        Proposition::Equal(value(4), value(3)),
    ]
}

fn upper_bound() -> Proposition {
    Proposition::LessOrEqual(value(4), literal(255))
}

#[test]
fn operation_definition_and_divisor_are_required_for_the_transported_bound() {
    for remainder in [false, true] {
        let semantic_axioms = operation_axioms(remainder);
        let goal = upper_bound();
        let proof = prove(&context(), &goal, &[], &semantic_axioms).unwrap();
        let mut changed_divisor = semantic_axioms.clone();
        changed_divisor[0] =
            Proposition::Equal(value(2), literal(if remainder { 512 } else { 128 }));
        let mut missing_definition = semantic_axioms.clone();
        missing_definition[1] = Proposition::Truth;
        let mut missing_divisor = semantic_axioms.clone();
        missing_divisor[0] = Proposition::Truth;
        for changed in [changed_divisor, missing_definition, missing_divisor] {
            assert!(prove(&context(), &goal, &[], &changed).is_none());
            assert!(check_certificate(&context(), &goal, &[], &changed, &proof).is_err());
        }
    }
}

fn bitwise_and_axioms(mask: &ScalarTerm) -> Vec<Proposition> {
    vec![
        Proposition::Equal(
            value(3),
            ScalarTerm::IntegerBitwiseAnd {
                scalar_type: integer_type(),
                left: Box::new(value(1)),
                right: Box::new(mask.clone()),
            },
        ),
        Proposition::Equal(value(4), value(3)),
    ]
}

#[test]
fn bitwise_and_mask_bounds_reach_the_aliased_result() {
    // `v3 = v1 & 15` has the total image `[0, 15]`; the alias `v4 = v3`
    // carries both endpoints across. The operand's own range is irrelevant.
    let semantic_axioms = bitwise_and_axioms(&literal(15));
    for goal in [
        Proposition::LessOrEqual(value(4), literal(15)),
        Proposition::LessOrEqual(literal(0), value(4)),
    ] {
        let proof = prove(&context(), &goal, &[], &semantic_axioms)
            .expect("the mask image crosses the result alias");
        check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
    }

    // The mask image also lands on the operation result itself.
    let goal = Proposition::LessOrEqual(value(3), literal(15));
    let proof = prove(&context(), &goal, &[], &semantic_axioms).unwrap();
    check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
    // A relaxed goal just inside the mask still proves through the image.
    let relaxed = Proposition::LessOrEqual(value(3), literal(300));
    let proof = prove(&context(), &relaxed, &[], &semantic_axioms).unwrap();
    check_certificate(&context(), &relaxed, &[], &semantic_axioms, &proof).unwrap();

    // Dropping the definition or the alias removes the bound.
    for omitted in [0, 1] {
        let mut missing = semantic_axioms.clone();
        missing[omitted] = Proposition::Truth;
        assert!(
            prove(
                &context(),
                &Proposition::LessOrEqual(value(4), literal(15)),
                &[],
                &missing,
            )
            .is_none()
        );
        assert!(
            check_certificate(
                &context(),
                &Proposition::LessOrEqual(value(4), literal(15)),
                &[],
                &missing,
                &proof,
            )
            .is_err()
        );
    }
}

#[test]
fn bitwise_and_negative_signed_mask_never_proves() {
    let signed_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let signed_value = |identity: u64| {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(signed_type),
        )
    };
    let signed_literal =
        |number: i128| ScalarTerm::integer(signed_type, IntegerValue::Signed(number)).unwrap();
    let context = PropositionContext::from_value_types((1..=3).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(signed_type),
        )
    }))
    .unwrap();
    // `x & -1` keeps the sign bit reachable; the step must not produce a
    // bound, not even the trivially true carrier one.
    let semantic_axioms = vec![Proposition::Equal(
        signed_value(3),
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type: signed_type,
            left: Box::new(signed_value(1)),
            right: Box::new(signed_literal(-1)),
        },
    )];
    for goal in [
        Proposition::LessOrEqual(signed_value(3), signed_literal(0)),
        Proposition::LessOrEqual(signed_literal(0), signed_value(3)),
    ] {
        assert!(prove(&context, &goal, &[], &semantic_axioms).is_none());
    }
}
