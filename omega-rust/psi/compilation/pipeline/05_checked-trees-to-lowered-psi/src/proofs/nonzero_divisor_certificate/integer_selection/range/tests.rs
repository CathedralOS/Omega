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
fn bitwise_and_bounded_mask_bounds_reach_the_result() {
    // `v3 = v1 & v2` where `v2` is not a literal but carries an
    // axiomatic upper bound: on an unsigned carrier the mask is
    // non-negative by type, so `v2 <= 15` alone gives the image
    // `[0, 15]` exactly like a literal mask.
    let mut semantic_axioms = bitwise_and_axioms(&value(2));
    semantic_axioms.insert(0, Proposition::LessOrEqual(value(2), literal(15)));
    for goal in [
        Proposition::LessOrEqual(value(3), literal(15)),
        Proposition::LessOrEqual(literal(0), value(3)),
        Proposition::LessOrEqual(value(4), literal(15)),
    ] {
        let proof = prove(&context(), &goal, &[], &semantic_axioms)
            .expect("the bounded mask image reaches the result");
        check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
    }

    // The bound pair may live on a term an `Equal` axiom aliases the
    // mask operand to; the image still reaches the result.
    let mut aliased = bitwise_and_axioms(&value(2));
    aliased.splice(
        0..0,
        [
            Proposition::Equal(value(2), value(5)),
            Proposition::LessOrEqual(value(5), literal(15)),
        ],
    );
    for goal in [
        Proposition::LessOrEqual(value(3), literal(15)),
        Proposition::LessOrEqual(literal(0), value(3)),
    ] {
        let proof = prove(&context(), &goal, &[], &aliased)
            .expect("the aliased mask's bound image reaches the result");
        check_certificate(&context(), &goal, &[], &aliased, &proof).unwrap();
    }

    // With no bound axioms on the mask or any alias, nothing proves.
    let unbounded = bitwise_and_axioms(&value(2));
    for goal in [
        Proposition::LessOrEqual(value(3), literal(15)),
        Proposition::LessOrEqual(literal(0), value(3)),
    ] {
        assert!(prove(&context(), &goal, &[], &unbounded).is_none());
    }
}

#[test]
fn bitwise_and_signed_bounded_mask_needs_nonnegativity() {
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
    // `v3 = v1 & v2` on a signed carrier: an upper bound alone leaves the
    // sign bit reachable, so `v2 <= 15` without `0 <= v2` admits nothing.
    let definition = Proposition::Equal(
        signed_value(3),
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type: signed_type,
            left: Box::new(signed_value(1)),
            right: Box::new(signed_value(2)),
        },
    );
    let semantic_axioms = vec![
        Proposition::LessOrEqual(signed_value(2), signed_literal(15)),
        definition.clone(),
    ];
    for goal in [
        Proposition::LessOrEqual(signed_value(3), signed_literal(15)),
        Proposition::LessOrEqual(signed_literal(0), signed_value(3)),
    ] {
        assert!(prove(&context, &goal, &[], &semantic_axioms).is_none());
    }
    // Once `0 <= v2` is axiomatic the image `[0, 15]` proves.
    let mut bounded = semantic_axioms.clone();
    bounded.insert(
        1,
        Proposition::LessOrEqual(signed_literal(0), signed_value(2)),
    );
    for goal in [
        Proposition::LessOrEqual(signed_value(3), signed_literal(15)),
        Proposition::LessOrEqual(signed_literal(0), signed_value(3)),
    ] {
        let proof = prove(&context, &goal, &[], &bounded)
            .expect("the signed bounded mask image reaches the result");
        check_certificate(&context, &goal, &[], &bounded, &proof).unwrap();
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

// Or-bounds are bound transports, not total images: `x | s` keeps no
// contiguous image, so only the endpoint cascade — reaching the derived
// closure — mints them; `range::prove` sees none.
fn bound_prove(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
) -> Option<proof_admission::ProofNode> {
    super::super::bound::prove_candidate_endpoint(
        context,
        goal,
        &[],
        semantic_axioms,
        &mut super::super::super::affine_custody::DefinitionIndex::new(
            context,
            &[],
            semantic_axioms,
        ),
    )
}

fn bitwise_or_axioms(sibling: &ScalarTerm) -> Vec<Proposition> {
    vec![
        Proposition::Equal(
            value(3),
            ScalarTerm::IntegerBitwiseOr {
                scalar_type: integer_type(),
                left: Box::new(value(1)),
                right: Box::new(sibling.clone()),
            },
        ),
        Proposition::Equal(value(4), value(3)),
    ]
}

#[test]
fn bitwise_or_literal_image_bounds_reach_the_result() {
    // `v3 = v1 | 6` with `v1 <= 15`: an or sets no bit above the wider
    // bound's width, so the image tops at the all-ones mask `15` — not at
    // endpoint addition's `21`.
    let mut semantic_axioms = bitwise_or_axioms(&literal(6));
    semantic_axioms.insert(0, Proposition::LessOrEqual(value(1), literal(15)));
    for goal in [
        Proposition::LessOrEqual(value(3), literal(15)),
        Proposition::LessOrEqual(value(4), literal(15)),
    ] {
        let proof = bound_prove(&context(), &goal, &semantic_axioms)
            .expect("the literal or-image reaches the result");
        check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
    }

    // A lower bound on the operand survives the or: `x | s >= x` once the
    // sibling is non-negative.
    let mut lowered = bitwise_or_axioms(&literal(6));
    lowered.insert(0, Proposition::LessOrEqual(literal(5), value(1)));
    let goal = Proposition::LessOrEqual(literal(5), value(3));
    let proof = bound_prove(&context(), &goal, &lowered).unwrap();
    check_certificate(&context(), &goal, &[], &lowered, &proof).unwrap();
}

#[test]
fn bitwise_or_bounded_image_bounds_reach_the_result() {
    // `v3 = v1 | v2` on the unsigned carrier: `v2 <= 15` supplies the
    // sibling bound while the carrier proves its non-negativity, and
    // `v1 <= 15` maps the result to the all-ones mask at the wider width —
    // `15`, where endpoint addition would claim `30`.
    let mut semantic_axioms = bitwise_or_axioms(&value(2));
    semantic_axioms.splice(
        0..0,
        [
            Proposition::LessOrEqual(value(1), literal(15)),
            Proposition::LessOrEqual(value(2), literal(15)),
        ],
    );
    for goal in [
        Proposition::LessOrEqual(value(3), literal(15)),
        Proposition::LessOrEqual(value(4), literal(15)),
        Proposition::LessOrEqual(literal(0), value(3)),
    ] {
        let proof = bound_prove(&context(), &goal, &semantic_axioms)
            .expect("the bounded or-image reaches the result");
        check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
    }

    // The bound pair may live on a term an `Equal` axiom aliases the
    // sibling to; the image still reaches the result.
    let mut aliased = bitwise_or_axioms(&value(2));
    aliased.splice(
        0..0,
        [
            Proposition::LessOrEqual(value(1), literal(15)),
            Proposition::Equal(value(2), value(5)),
            Proposition::LessOrEqual(value(5), literal(15)),
        ],
    );
    for goal in [
        Proposition::LessOrEqual(value(3), literal(15)),
        Proposition::LessOrEqual(literal(0), value(3)),
    ] {
        let proof = bound_prove(&context(), &goal, &aliased)
            .expect("the aliased sibling's bound image reaches the result");
        check_certificate(&context(), &goal, &[], &aliased, &proof).unwrap();
    }

    // With the sibling unbounded the image tops at the carrier width, so
    // `v3 <= 15` does not prove.
    let mut unbounded = bitwise_or_axioms(&value(2));
    unbounded.insert(0, Proposition::LessOrEqual(value(1), literal(15)));
    assert!(
        bound_prove(
            &context(),
            &Proposition::LessOrEqual(value(3), literal(15)),
            &unbounded,
        )
        .is_none()
    );
}

#[test]
fn bitwise_or_signed_bounded_image_needs_sibling_nonnegativity() {
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
    let definition = Proposition::Equal(
        signed_value(3),
        ScalarTerm::IntegerBitwiseOr {
            scalar_type: signed_type,
            left: Box::new(signed_value(1)),
            right: Box::new(signed_value(2)),
        },
    );
    // On a signed carrier `v2 <= 15` alone still caps the image: a
    // negative operand keeps its sign bit, so the result stays at or below
    // the wider bound's all-ones mask. The rooted side is `v2` while `v1`
    // resolves as the bounded sibling.
    let semantic_axioms = vec![
        Proposition::LessOrEqual(signed_literal(0), signed_value(1)),
        Proposition::LessOrEqual(signed_value(1), signed_literal(15)),
        Proposition::LessOrEqual(signed_value(2), signed_literal(15)),
        definition.clone(),
    ];
    let capped = Proposition::LessOrEqual(signed_value(3), signed_literal(15));
    let proof = bound_prove(&context, &capped, &semantic_axioms)
        .expect("a possibly-negative operand still caps the or-image");
    check_certificate(&context, &capped, &[], &semantic_axioms, &proof).unwrap();
    // Without `0 <= v2` the lower image does not exist: `x | s >= x` needs
    // `0 <= s`, and `v2` can drive the result negative.
    assert!(
        bound_prove(
            &context,
            &Proposition::LessOrEqual(signed_literal(0), signed_value(3)),
            &semantic_axioms,
        )
        .is_none()
    );
    // Once `0 <= v2` is axiomatic the lower transport proves the floor too.
    let mut bounded = semantic_axioms.clone();
    bounded.insert(
        2,
        Proposition::LessOrEqual(signed_literal(0), signed_value(2)),
    );
    let floored = Proposition::LessOrEqual(signed_literal(0), signed_value(3));
    let proof = bound_prove(&context, &floored, &bounded)
        .expect("the signed bounded or-image proves the floor");
    check_certificate(&context, &floored, &[], &bounded, &proof).unwrap();
    // A negative signed sibling literal forces the sign bit: even the
    // trivially true carrier bounds must not be produced.
    let negative = vec![
        Proposition::LessOrEqual(signed_literal(0), signed_value(1)),
        Proposition::LessOrEqual(signed_value(1), signed_literal(15)),
        Proposition::Equal(
            signed_value(3),
            ScalarTerm::IntegerBitwiseOr {
                scalar_type: signed_type,
                left: Box::new(signed_value(1)),
                right: Box::new(signed_literal(-1)),
            },
        ),
    ];
    for goal in [
        Proposition::LessOrEqual(signed_value(3), signed_literal(-1)),
        Proposition::LessOrEqual(signed_value(1), signed_value(3)),
    ] {
        assert!(bound_prove(&context, &goal, &negative).is_none());
    }
}
