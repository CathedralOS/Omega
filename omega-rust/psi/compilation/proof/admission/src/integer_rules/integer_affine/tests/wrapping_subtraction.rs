//! Wrapping subtraction of a literal and wrapping targets spelled in place.
//!
//! `x -w c` equals `x + (2^n - c)` modulo the width, and on an unsigned
//! carrier it wraps exactly when `c > x`. The checker reads it as the wrapping
//! sum with addend `-c`: the lower image `L - c` holds whether or not the
//! difference wraps (a wrapped result only grows by `2^n`), while the upper
//! image `U - c` needs the no-overflow conjunct `c <= x`. The exhaustive
//! tests enumerate every 8-bit operand to confirm each accepted mapping and
//! that every missing, weaker or misplaced conjunct rejects.

use super::{literal, unsigned_literal, value};
use crate::affine_witness;
use crate::integer_rules::integer_affine::{
    CheckedIntegerAffineForm, IntegerAffineBoundConversionError, IntegerAffineWitness,
    IntegerAffineWitnessError, check_integer_affine_witness, integer_affine_wrapping_evidence,
    map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

fn context(integer_type: IntegerType, values: u64) -> PropositionContext {
    PropositionContext::from_value_types(
        (1..=values).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap()
}

fn typed_literal(integer_type: IntegerType, number: i128) -> ScalarTerm {
    match integer_type.sign() {
        IntegerSign::Signed => literal(integer_type, number),
        IntegerSign::Unsigned => unsigned_literal(integer_type, number as u128),
    }
}

fn subtract(integer_type: IntegerType, left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
    ScalarTerm::wrapping_integer_subtract(integer_type, left, right).unwrap()
}

fn add(integer_type: IntegerType, left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
    ScalarTerm::wrapping_integer_add(integer_type, left, right).unwrap()
}

/// `y = x -w c` checked as a forward witness from `x` through one cited row.
fn subtraction_row(
    integer_type: IntegerType,
    subtrahend: i128,
) -> (ScalarTerm, ScalarTerm, CheckedIntegerAffineForm) {
    let operand = value(1, integer_type);
    let defined = value(2, integer_type);
    let axioms = [Proposition::Equal(
        defined.clone(),
        subtract(
            integer_type,
            operand.clone(),
            typed_literal(integer_type, subtrahend),
        ),
    )];
    let form = check_integer_affine_witness(
        &context(integer_type, 2),
        &axioms,
        &affine_witness(operand.clone(), defined.clone(), vec![0], vec![None]),
    )
    .expect("a wrapping difference of a literal is one checked step");
    (operand, defined, form)
}

/// The integer a one-sided literal relation on `x` asserts, evaluated at
/// the concrete operand `x`.
fn holds(relation: &Proposition, operand: &ScalarTerm, x: i128) -> bool {
    let number = |term: &ScalarTerm| match term.integer_value() {
        Some((_, IntegerValue::Signed(value))) => value,
        Some((_, IntegerValue::Unsigned(value))) => value as i128,
        None => panic!("one side of {relation:?} is a literal"),
    };
    match relation {
        Proposition::LessOrEqual(left, right) if left == operand => x <= number(right),
        Proposition::LessOrEqual(left, right) if right == operand => number(left) <= x,
        Proposition::LessThan(left, right) if left == operand => x < number(right),
        Proposition::LessThan(left, right) if right == operand => number(left) < x,
        other => panic!("unexpected relation {other:?}"),
    }
}

/// Every mapping the checker accepts for `x -w c` holds at every 8-bit
/// operand its premises admit, and the upper side is accepted only with the
/// exact no-overflow conjunct.
fn assert_subtraction_mapping_is_sound(integer_type: IntegerType, subtrahends: &[i128]) {
    let (minimum, maximum) = match integer_type.sign() {
        IntegerSign::Signed => (-128_i128, 127_i128),
        IntegerSign::Unsigned => (0, 255),
    };
    let wrap = |number: i128| match integer_type.sign() {
        IntegerSign::Signed => (number as i8) as i128,
        IntegerSign::Unsigned => (number as u8) as i128,
    };
    for &subtrahend in subtrahends {
        let (operand, defined, form) = subtraction_row(integer_type, subtrahend);
        let mut accepted = 0;
        for bound in minimum..=maximum {
            for (lower, strict) in [(true, false), (false, false), (true, true), (false, true)] {
                let endpoint = typed_literal(integer_type, bound);
                let relation = match (lower, strict) {
                    (true, false) => Proposition::LessOrEqual(endpoint, operand.clone()),
                    (false, false) => Proposition::LessOrEqual(operand.clone(), endpoint),
                    (true, true) => Proposition::LessThan(endpoint, operand.clone()),
                    (false, true) => Proposition::LessThan(operand.clone(), endpoint),
                };
                let required = integer_affine_wrapping_evidence(&form, &relation)
                    .expect("one literal relation on the operand");
                // Only the side the difference moves toward needs evidence:
                // subtracting a positive literal can cross the minimum, which
                // breaks the upper image, and a negative one the maximum.
                let expected_side = (subtrahend > 0 && !lower) || (subtrahend < 0 && lower);
                assert_eq!(!required.is_empty(), expected_side, "c={subtrahend}");
                if !required.is_empty() {
                    assert_eq!(
                        map_integer_affine_bound(&form, &relation),
                        Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
                        "c={subtrahend} bound={bound}",
                    );
                }
                let premise = if required.is_empty() {
                    relation.clone()
                } else {
                    Proposition::Conjunction(
                        std::iter::once(relation.clone())
                            .chain(required.iter().cloned())
                            .collect(),
                    )
                };
                let Ok(conclusion) = map_integer_affine_bound(&form, &premise) else {
                    continue;
                };
                accepted += 1;
                for x in minimum..=maximum {
                    if !holds(&relation, &operand, x)
                        || !required.iter().all(|fact| holds(fact, &operand, x))
                    {
                        continue;
                    }
                    let y = wrap(x - subtrahend);
                    assert!(
                        holds(&conclusion, &defined, y),
                        "{premise:?} admits x={x}, whose difference {y} breaks {conclusion:?}",
                    );
                }
            }
        }
        assert!(accepted > 0, "c={subtrahend} maps some bound");
    }
}

#[test]
fn unsigned_wrapping_subtraction_maps_every_accepted_bound_soundly() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_subtraction_mapping_is_sound(integer_type, &[0, 1, 5, 200, 255]);
}

#[test]
fn signed_wrapping_subtraction_maps_every_accepted_bound_soundly() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    assert_subtraction_mapping_is_sound(integer_type, &[-128, -3, -1, 0, 1, 5, 127]);
}

#[test]
fn unsigned_wrapping_subtraction_demands_exactly_the_minuend_floor() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let bound = |number| unsigned_literal(integer_type, number);
    let (operand, defined, form) = subtraction_row(integer_type, 1);

    // `x <= 10` maps to `x -w 1 <= 9` only while `1 <= x` rules out the
    // wrap from zero to 255.
    let upper = Proposition::LessOrEqual(operand.clone(), bound(10));
    let floor = Proposition::LessOrEqual(bound(1), operand.clone());
    assert_eq!(
        integer_affine_wrapping_evidence(&form, &upper),
        Ok(vec![floor.clone()])
    );
    assert_eq!(
        map_integer_affine_bound(&form, &upper),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
    );
    assert_eq!(
        map_integer_affine_bound(
            &form,
            &Proposition::Conjunction(vec![upper.clone(), floor.clone()])
        ),
        Ok(Proposition::LessOrEqual(defined.clone(), bound(9))),
    );
    let strict = Proposition::LessThan(operand.clone(), bound(10));
    assert_eq!(
        map_integer_affine_bound(
            &form,
            &Proposition::Conjunction(vec![strict, floor.clone()])
        ),
        Ok(Proposition::LessThan(defined.clone(), bound(9))),
    );

    // A different floor, the addition's headroom shape, or the floor stated
    // on the result are not the conjunct the step needs.
    for wrong in [
        Proposition::LessOrEqual(bound(2), operand.clone()),
        Proposition::LessOrEqual(bound(0), operand.clone()),
        Proposition::LessOrEqual(operand.clone(), bound(254)),
        Proposition::LessOrEqual(bound(1), defined.clone()),
    ] {
        assert_eq!(
            map_integer_affine_bound(
                &form,
                &Proposition::Conjunction(vec![upper.clone(), wrong.clone()])
            ),
            Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
            "{wrong:?}",
        );
    }

    // The lower side needs nothing: `3 <= x` maps to `2 <= x -w 1`, and a
    // conjunct offered there is unexpected rather than ignored.
    let lower = Proposition::LessOrEqual(bound(3), operand.clone());
    assert_eq!(integer_affine_wrapping_evidence(&form, &lower), Ok(vec![]));
    assert_eq!(
        map_integer_affine_bound(&form, &lower),
        Ok(Proposition::LessOrEqual(bound(2), defined.clone())),
    );
    assert_eq!(
        map_integer_affine_bound(&form, &Proposition::Conjunction(vec![lower, floor])),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
    );
    // `0 <= x` would map below zero: the unsigned carrier has no such bound.
    assert_eq!(
        map_integer_affine_bound(&form, &Proposition::LessOrEqual(bound(0), operand.clone())),
        Err(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier),
    );
}

#[test]
fn a_wrapping_target_spelled_in_place_is_its_own_definition() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let bound = |number| unsigned_literal(integer_type, number);
    let operand = value(1, integer_type);
    let context = context(integer_type, 2);
    let in_place = |target: ScalarTerm| {
        check_integer_affine_witness(
            &context,
            &[],
            &IntegerAffineWitness {
                root: operand.clone(),
                target,
                definition_axioms: vec![],
                literal_axioms: vec![],
            },
        )
    };

    // `x -w 1` needs no defining row: the upper image still needs `1 <= x`.
    let difference = subtract(integer_type, operand.clone(), bound(1));
    let form = in_place(difference.clone()).expect("an in-place difference checks");
    let upper = Proposition::LessOrEqual(operand.clone(), bound(4));
    let floor = Proposition::LessOrEqual(bound(1), operand.clone());
    assert_eq!(
        map_integer_affine_bound(&form, &upper),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
    );
    assert_eq!(
        map_integer_affine_bound(
            &form,
            &Proposition::Conjunction(vec![upper.clone(), floor.clone()])
        ),
        Ok(Proposition::LessOrEqual(difference.clone(), bound(3))),
    );
    assert_eq!(
        map_integer_affine_bound(
            &form,
            &Proposition::Conjunction(vec![
                upper.clone(),
                Proposition::LessOrEqual(bound(1), difference.clone()),
            ])
        ),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
    );

    // A sum with the literal on either side maps its upper side freely and
    // needs the headroom `x <= 254` for its lower side.
    for sum in [
        add(integer_type, operand.clone(), bound(1)),
        add(integer_type, bound(1), operand.clone()),
    ] {
        let form = in_place(sum.clone()).expect("an in-place sum checks");
        assert_eq!(
            map_integer_affine_bound(&form, &upper),
            Ok(Proposition::LessOrEqual(sum.clone(), bound(5))),
        );
        let lower = Proposition::LessOrEqual(bound(2), operand.clone());
        assert_eq!(
            map_integer_affine_bound(&form, &lower),
            Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
        );
        assert_eq!(
            map_integer_affine_bound(
                &form,
                &Proposition::Conjunction(vec![
                    lower,
                    Proposition::LessOrEqual(operand.clone(), bound(254)),
                ])
            ),
            Ok(Proposition::LessOrEqual(bound(3), sum)),
        );
    }
}

#[test]
fn an_in_place_step_follows_the_chain_it_extends() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let bound = |number| unsigned_literal(integer_type, number);
    let operand = value(1, integer_type);
    let defined = value(2, integer_type);
    let context = context(integer_type, 2);
    let axioms = [Proposition::Equal(
        defined.clone(),
        add(integer_type, operand.clone(), bound(2)),
    )];
    // `y = x +w 2`, then the target `y -w 1` in place: the subtraction's
    // floor is stated on `y`, the value the step reads.
    let difference = subtract(integer_type, defined.clone(), bound(1));
    let form = check_integer_affine_witness(
        &context,
        &axioms,
        &affine_witness(operand.clone(), difference.clone(), vec![0], vec![None]),
    )
    .expect("an in-place step extends a cited chain");
    let upper = Proposition::LessOrEqual(operand.clone(), bound(10));
    let floor = Proposition::LessOrEqual(bound(1), defined.clone());
    assert_eq!(
        integer_affine_wrapping_evidence(&form, &upper),
        Ok(vec![floor.clone()])
    );
    assert_eq!(
        map_integer_affine_bound(&form, &Proposition::Conjunction(vec![upper.clone(), floor])),
        Ok(Proposition::LessOrEqual(difference, bound(11))),
    );
    // The floor on the chain's root is not the floor on the value subtracted.
    assert_eq!(
        map_integer_affine_bound(
            &form,
            &Proposition::Conjunction(vec![
                upper,
                Proposition::LessOrEqual(bound(1), operand.clone()),
            ])
        ),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
    );

    // An in-place step must read the chain's last value, not its root.
    assert_eq!(
        check_integer_affine_witness(
            &context,
            &axioms,
            &affine_witness(
                operand.clone(),
                subtract(integer_type, operand, bound(1)),
                vec![0],
                vec![None]
            ),
        ),
        Err(IntegerAffineWitnessError::TargetMismatch),
    );
}

#[test]
fn in_place_targets_outside_one_literal_step_reject() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let wide = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
    let bound = |number| unsigned_literal(integer_type, number);
    let operand = value(1, integer_type);
    let other = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(wide)),
    ])
    .unwrap();
    let in_place = |root: &ScalarTerm, target: ScalarTerm| {
        check_integer_affine_witness(
            &context,
            &[],
            &IntegerAffineWitness {
                root: root.clone(),
                target,
                definition_axioms: vec![],
                literal_axioms: vec![],
            },
        )
    };
    for target in [
        // `c -w x` reverses the operand's order; it is not a translation.
        subtract(integer_type, bound(10), operand.clone()),
        // A non-literal subtrahend or addend has no fixed offset.
        subtract(integer_type, operand.clone(), other.clone()),
        add(integer_type, operand.clone(), other.clone()),
        add(integer_type, operand.clone(), operand.clone()),
        // The step must read the root itself.
        subtract(integer_type, other.clone(), bound(1)),
    ] {
        assert_eq!(
            in_place(&operand, target.clone()),
            Err(IntegerAffineWitnessError::EmptyDefinitionChain),
            "{target:?}",
        );
    }
    // A difference on another carrier is not a step of this chain.
    let wide_operand = value(3, wide);
    assert_eq!(
        in_place(
            &operand,
            subtract(wide, wide_operand, unsigned_literal(wide, 1))
        ),
        Err(IntegerAffineWitnessError::TargetTypeMismatch),
    );
}
