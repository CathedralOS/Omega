use super::{ProofRule, Proposition, PropositionContext, ScalarTerm, prove};
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
fn call_result_alias_preserves_divide_and_remainder_bounds() {
    for remainder in [false, true] {
        let semantic_axioms = operation_axioms(remainder);
        for goal in [
            upper_bound(),
            Proposition::LessOrEqual(literal(0), value(4)),
        ] {
            let proof = prove(&context(), &goal, &[], &semantic_axioms)
                .expect("computed argument bound follows the cited result alias");
            assert!(matches!(
                proof.rule,
                ProofRule::IntegerOrderSubstitution { .. }
            ));
            check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
        }
    }
}

#[test]
fn operation_bounds_cross_each_explicitly_cited_alias() {
    let mut semantic_axioms = operation_axioms(true);
    semantic_axioms[2] = Proposition::Equal(value(4), value(5));
    semantic_axioms.push(Proposition::Equal(value(5), value(3)));
    let goal = upper_bound();
    let proof = prove(&context(), &goal, &[], &semantic_axioms)
        .expect("two directed aliases retain the operation bound");
    check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
    for omitted in [2, 3] {
        let mut missing = semantic_axioms.clone();
        missing[omitted] = Proposition::Truth;
        assert!(prove(&context(), &goal, &[], &missing).is_none());
        assert!(check_certificate(&context(), &goal, &[], &missing, &proof).is_err());
    }
}

#[test]
fn missing_or_changed_alias_cannot_replace_the_cited_edge() {
    let semantic_axioms = operation_axioms(true);
    let goal = upper_bound();
    let proof = prove(&context(), &goal, &[], &semantic_axioms).unwrap();
    for replacement in [Proposition::Truth, Proposition::Equal(value(4), value(6))] {
        let mut changed = semantic_axioms.clone();
        changed[2] = replacement;
        assert!(prove(&context(), &goal, &[], &changed).is_none());
        assert!(check_certificate(&context(), &goal, &[], &changed, &proof).is_err());
    }
}

#[test]
fn canonical_reversed_alias_requires_a_new_symmetry_proof() {
    let semantic_axioms = operation_axioms(true);
    let goal = upper_bound();
    let original = prove(&context(), &goal, &[], &semantic_axioms).unwrap();
    let mut canonical = semantic_axioms.clone();
    canonical[2] = Proposition::Equal(value(3), value(4));
    assert!(check_certificate(&context(), &goal, &[], &canonical, &original).is_err());
    let proof = prove(&context(), &goal, &[], &canonical).expect("explicit equality symmetry");
    check_certificate(&context(), &goal, &[], &canonical, &proof).unwrap();
    let ProofRule::IntegerOrderSubstitution { equality, .. } = &proof.rule else {
        panic!("range transport retains equality evidence");
    };
    assert!(matches!(equality.rule, ProofRule::EqualitySymmetry { .. }));
}

#[test]
fn computed_argument_bounds_cross_only_the_proved_order_direction() {
    for strict in [false, true] {
        let mut axioms = operation_axioms(true);
        axioms[2] = if strict {
            Proposition::LessThan(value(4), value(3))
        } else {
            Proposition::LessOrEqual(value(4), value(3))
        };
        let goal = upper_bound();
        let proof = prove(&context(), &goal, &[], &axioms).expect("ordered result bound");
        check_certificate(&context(), &goal, &[], &axioms, &proof).unwrap();
        axioms[2] = Proposition::LessOrEqual(value(3), value(4));
        assert!(prove(&context(), &goal, &[], &axioms).is_none());
        assert!(check_certificate(&context(), &goal, &[], &axioms, &proof).is_err());
    }
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

#[test]
fn mathematical_exact_cast_bounds_replay_the_operation_and_result_alias() {
    for remainder in [false, true] {
        let semantic_axioms = operation_axioms(remainder);
        // The upper bound is narrower than the u16 carrier. Its proof must
        // cite the operation and alias; the lower bound zero needs neither.
        let goal = proof_admission::lift_fixed_integer_relation(&upper_bound()).unwrap();
        let proof = super::super::build(&context(), &goal, &[], &semantic_axioms)
            .expect("mathematical narrowing bound retains the scalar operation proof");
        check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
        let mut missing_alias = semantic_axioms.clone();
        missing_alias[2] = Proposition::Truth;
        assert!(check_certificate(&context(), &goal, &[], &missing_alias, &proof).is_err());
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
fn bitwise_and_value_mask_lands_through_a_prior_literal_axiom() {
    // `v3 = v1 & v2` proves only once `v2` lands as the non-negative mask.
    let landed = vec![
        Proposition::Equal(value(2), literal(15)),
        Proposition::Equal(
            value(3),
            ScalarTerm::IntegerBitwiseAnd {
                scalar_type: integer_type(),
                left: Box::new(value(1)),
                right: Box::new(value(2)),
            },
        ),
    ];
    let goal = Proposition::LessOrEqual(value(3), literal(15));
    let proof = prove(&context(), &goal, &[], &landed).expect("landed value mask");
    check_certificate(&context(), &goal, &[], &landed, &proof).unwrap();

    let mut unlanded = landed.clone();
    unlanded[0] = Proposition::Truth;
    assert!(prove(&context(), &goal, &[], &unlanded).is_none());
    assert!(check_certificate(&context(), &goal, &[], &unlanded, &proof).is_err());
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
