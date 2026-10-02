use crate::{
    Budget, PrimitiveJudgment, ProofNode, ProofRule, Term, check_certificate,
    verify_bounded_certificate, verify_mathematical_certificate,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

fn carrier(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).unwrap()
}
fn value(carrier: IntegerType, identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(carrier),
    )
}
fn literal(carrier: IntegerType, number: u64) -> ScalarTerm {
    let number = match carrier.sign() {
        IntegerSign::Signed => IntegerValue::Signed(i128::from(number)),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(number)),
    };
    ScalarTerm::integer(carrier, number).unwrap()
}
fn add(carrier: IntegerType, left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
    ScalarTerm::WrappingIntegerAdd {
        scalar_type: carrier,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn subtract(carrier: IntegerType, left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
    ScalarTerm::WrappingIntegerSubtract {
        scalar_type: carrier,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn multiply(carrier: IntegerType, left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
    ScalarTerm::WrappingIntegerMultiply {
        scalar_type: carrier,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn context(carrier: IntegerType, count: u64) -> PropositionContext {
    PropositionContext::from_value_types((1..=count).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(carrier),
        )
    }))
    .unwrap()
}
fn certificate(conclusion: Proposition) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    }
}
fn gauss(carrier: IntegerType, wrong_step: bool) -> Proposition {
    let count = value(carrier, 1);
    let accumulated = value(carrier, 2);
    let next_accumulated = if wrong_step {
        accumulated.clone()
    } else {
        add(carrier, accumulated.clone(), count.clone())
    };
    let next_count = subtract(carrier, count.clone(), literal(carrier, 1));
    Proposition::Equal(
        add(
            carrier,
            multiply(carrier, next_accumulated, literal(carrier, 2)),
            multiply(
                carrier,
                next_count.clone(),
                add(carrier, next_count, literal(carrier, 1)),
            ),
        ),
        add(
            carrier,
            multiply(carrier, accumulated, literal(carrier, 2)),
            multiply(
                carrier,
                count.clone(),
                add(carrier, count, literal(carrier, 1)),
            ),
        ),
    )
}

#[test]
fn wrapping_polynomial_gauss_preservation_is_independently_certified() {
    for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
        for bits in [8, 64, 128] {
            let carrier = carrier(sign, bits);
            let proof = certificate(gauss(carrier, false));
            let context = context(carrier, 2);
            check_certificate(&context, &proof.conclusion, &[], &[], &proof).unwrap();
            let wrong = certificate(gauss(carrier, true));
            assert!(check_certificate(&context, &wrong.conclusion, &[], &[], &wrong).is_err());
        }
    }
}

#[test]
fn wrapping_polynomial_reduces_coefficients_without_cancelling_nonunits() {
    for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
        let carrier = carrier(sign, 8);
        let subject = value(carrier, 1);
        // 16*16 is zero modulo256, including signed canonical representatives.
        let zero_multiple = multiply(
            carrier,
            multiply(carrier, subject.clone(), literal(carrier, 16)),
            literal(carrier, 16),
        );
        let proof = certificate(Proposition::Equal(
            add(carrier, subject.clone(), zero_multiple),
            subject.clone(),
        ));
        check_certificate(&context(carrier, 2), &proof.conclusion, &[], &[], &proof).unwrap();
        let other = value(carrier, 2);
        let doubled = Proposition::Equal(
            multiply(carrier, subject.clone(), literal(carrier, 2)),
            multiply(carrier, other.clone(), literal(carrier, 2)),
        );
        let cancelled = certificate(Proposition::Equal(subject, other));
        assert!(
            check_certificate(
                &context(carrier, 2),
                &cancelled.conclusion,
                &[doubled],
                &[],
                &cancelled
            )
            .is_err()
        );
    }
}

#[test]
fn wrapping_polynomial_does_not_supply_order_or_disequality() {
    let carrier = carrier(IntegerSign::Unsigned, 8);
    let subject = value(carrier, 1);
    let overflowed = add(carrier, subject.clone(), literal(carrier, 1));
    let restored = add(
        carrier,
        add(carrier, subject.clone(), literal(carrier, 128)),
        literal(carrier, 128),
    );
    for goal in [
        Proposition::LessThan(subject.clone(), overflowed.clone()),
        Proposition::LessOrEqual(subject.clone(), overflowed),
        Proposition::LessThan(subject.clone(), restored.clone()),
        Proposition::Implication {
            premise: Box::new(Proposition::Equal(subject, restored)),
            conclusion: Box::new(Proposition::Falsehood),
        },
    ] {
        let proof = certificate(goal);
        assert!(
            check_certificate(&context(carrier, 1), &proof.conclusion, &[], &[], &proof).is_err()
        );
    }
}

#[test]
fn wrapping_polynomial_retains_policy_carrier_and_subject_identity() {
    let carrier = carrier(IntegerSign::Unsigned, 64);
    let subject = value(carrier, 1);
    let wrapped = add(carrier, subject.clone(), literal(carrier, 1));
    for changed in [
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type: carrier,
            left: Box::new(subject.clone()),
            right: Box::new(literal(carrier, 1)),
        },
        ScalarTerm::ExactIntegerAdd {
            scalar_type: carrier,
            left: Box::new(subject.clone()),
            right: Box::new(literal(carrier, 1)),
        },
        add(carrier, value(carrier, 99), literal(carrier, 1)),
        add(
            carrier,
            value(IntegerType::new(IntegerSign::Unsigned, 32).unwrap(), 1),
            literal(carrier, 1),
        ),
        add(
            carrier,
            value(IntegerType::new(IntegerSign::Signed, 64).unwrap(), 1),
            literal(carrier, 1),
        ),
    ] {
        let proof = certificate(Proposition::Equal(wrapped.clone(), changed));
        assert!(
            check_certificate(&context(carrier, 1), &proof.conclusion, &[], &[], &proof).is_err()
        );
    }
    let address = IntegerType::address(64).unwrap();
    let proof = certificate(Proposition::Equal(
        add(address, value(address, 1), literal(address, 0)),
        value(address, 1),
    ));
    assert!(check_certificate(&context(address, 1), &proof.conclusion, &[], &[], &proof).is_err());
}

#[test]
fn wrapping_polynomial_core_rechecks_the_primitive_and_rejects_forged_evidence() {
    let carrier = carrier(IntegerSign::Unsigned, 64);
    let context = context(carrier, 2);
    let proof = certificate(gauss(carrier, false));
    let mut denoted = verify_bounded_certificate(
        &context,
        &proof.conclusion,
        &[],
        &[],
        &proof,
        &mut Budget::default(),
    )
    .unwrap();
    denoted.certificate.term = denoted.arena.insert(Term::TwoZero);
    assert!(
        verify_mathematical_certificate(
            &mut denoted.arena,
            &denoted.certificate,
            &mut Budget::default()
        )
        .is_err()
    );
    let wrong = certificate(gauss(carrier, true));
    assert!(
        verify_bounded_certificate(
            &context,
            &wrong.conclusion,
            &[],
            &[],
            &wrong,
            &mut Budget::default()
        )
        .is_err()
    );
}

#[test]
fn wrapping_polynomial_expansion_and_depth_exhaustion_establish_nothing() {
    let carrier = carrier(IntegerSign::Unsigned, 64);
    let mut expanded = literal(carrier, 1);
    for identity in 1..=13 {
        expanded = multiply(
            carrier,
            expanded,
            add(carrier, value(carrier, identity), literal(carrier, 1)),
        );
    }
    assert_eq!(
        super::equal(&expanded, &expanded),
        Err(super::ClosedIntegerEvaluationError::ResourceLimitExceeded)
    );
    let mut deep = value(carrier, 1);
    for _ in 0..128 {
        deep = add(carrier, deep, literal(carrier, 1));
    }
    assert_eq!(
        super::equal(&deep, &value(carrier, 1)),
        Err(super::ClosedIntegerEvaluationError::ResourceLimitExceeded)
    );
}
