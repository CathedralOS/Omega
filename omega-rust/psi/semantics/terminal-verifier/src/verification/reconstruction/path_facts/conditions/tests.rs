use super::*;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

fn value(index: u64, scalar_type: ScalarType) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(index).unwrap(), scalar_type)
}

fn integer(scalar_type: IntegerType, literal: i128) -> ScalarTerm {
    ScalarTerm::integer(
        scalar_type,
        match scalar_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(literal),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(literal).unwrap()),
        },
    )
    .unwrap()
}

fn selected(predicate: ScalarTerm, positive: bool) -> Proposition {
    let condition = value(1, ScalarType::Boolean);
    condition_fact(
        ValueId::new(1).unwrap(),
        positive,
        &[Proposition::Equal(condition, predicate)],
        &|id| ScalarTerm::value(id, ScalarType::Boolean),
    )
    .unwrap()
}

#[test]
fn equality_complements_keep_full_signed_and_unsigned_nonzero_meaning() {
    for (sign, bits) in [
        (IntegerSign::Unsigned, 8),
        (IntegerSign::Signed, 8),
        (IntegerSign::Signed, 1),
    ] {
        let scalar_type = IntegerType::new(sign, bits).unwrap();
        let subject = value(2, ScalarType::Integer(scalar_type));
        let zero = integer(scalar_type, 0);
        let expected = match (sign, bits) {
            (IntegerSign::Unsigned, _) => {
                Proposition::LessOrEqual(integer(scalar_type, 1), subject.clone())
            }
            (IntegerSign::Signed, 1) => {
                Proposition::LessOrEqual(subject.clone(), integer(scalar_type, -1))
            }
            (IntegerSign::Signed, _) => Proposition::Disjunction(vec![
                Proposition::LessOrEqual(subject.clone(), integer(scalar_type, -1)),
                Proposition::LessOrEqual(integer(scalar_type, 1), subject.clone()),
            ]),
        };
        for (left, right) in [
            (subject.clone(), zero.clone()),
            (zero.clone(), subject.clone()),
        ] {
            let predicate = ScalarTerm::IntegerEqual {
                scalar_type,
                left: Box::new(left.clone()),
                right: Box::new(right.clone()),
            };
            assert_eq!(
                selected(predicate.clone(), true),
                Proposition::Equal(left, right)
            );
            assert_eq!(selected(predicate.clone(), false), expected);
            assert_eq!(
                selected(ScalarTerm::boolean_not(predicate).unwrap(), true),
                expected
            );
        }
    }
}

#[test]
fn order_complements_reverse_endpoints_without_changing_carriers() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let scalar_type = IntegerType::new(sign, 16).unwrap();
        let left = value(2, ScalarType::Integer(scalar_type));
        let right = value(3, ScalarType::Integer(scalar_type));
        let less = ScalarTerm::IntegerLessThan {
            scalar_type,
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        };
        let inclusive = ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        };
        assert_eq!(
            selected(less.clone(), true),
            Proposition::LessThan(left.clone(), right.clone())
        );
        assert_eq!(
            selected(less, false),
            Proposition::LessOrEqual(right.clone(), left.clone())
        );
        assert_eq!(
            selected(inclusive.clone(), true),
            Proposition::LessOrEqual(left.clone(), right.clone())
        );
        assert_eq!(
            selected(inclusive, false),
            Proposition::LessThan(right.clone(), left.clone())
        );
        let equal = ScalarTerm::IntegerEqual {
            scalar_type,
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        };
        assert_eq!(
            selected(equal, false),
            Proposition::Disjunction(vec![
                Proposition::LessThan(left.clone(), right.clone()),
                Proposition::LessThan(right, left)
            ])
        );
    }
}

#[test]
fn boolean_aliases_and_false_wrappers_preserve_selected_polarity() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let subject = value(4, ScalarType::Integer(scalar_type));
    let comparison = ScalarTerm::IntegerEqual {
        scalar_type,
        left: Box::new(subject.clone()),
        right: Box::new(integer(scalar_type, 0)),
    };
    let condition = value(1, ScalarType::Boolean);
    let alias = value(2, ScalarType::Boolean);
    let comparison_value = value(3, ScalarType::Boolean);
    let axioms = [
        Proposition::Equal(comparison_value.clone(), comparison),
        Proposition::Equal(
            alias.clone(),
            ScalarTerm::BooleanEqual {
                left: Box::new(comparison_value),
                right: Box::new(ScalarTerm::Boolean(false)),
            },
        ),
        Proposition::Equal(condition.clone(), alias.clone()),
    ];
    assert_eq!(
        condition_fact(ValueId::new(1).unwrap(), true, &axioms, &|id| {
            ScalarTerm::value(id, ScalarType::Boolean)
        }),
        Some(Proposition::LessOrEqual(integer(scalar_type, 1), subject))
    );
    assert_eq!(
        condition_fact(ValueId::new(8).unwrap(), false, &axioms, &|id| {
            ScalarTerm::value(id, ScalarType::Boolean)
        }),
        Some(Proposition::Equal(
            value(8, ScalarType::Boolean),
            ScalarTerm::Boolean(false)
        ))
    );
    for positive in [false, true] {
        assert_eq!(
            selected(
                ScalarTerm::BooleanEqual {
                    left: Box::new(condition.clone()),
                    right: Box::new(alias.clone())
                },
                positive
            ),
            if positive {
                Proposition::Equal(condition.clone(), alias.clone())
            } else {
                Proposition::Equal(
                    ScalarTerm::boolean_not(condition.clone()).unwrap(),
                    alias.clone(),
                )
            }
        );
    }
}

#[test]
fn strict_literal_bounds_are_discrete_on_every_fixed_carrier() {
    use super::super::discrete::strict_bound;
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let scalar_type = IntegerType::new(sign, 8).unwrap();
        let subject = value(2, ScalarType::Integer(scalar_type));
        assert_eq!(
            strict_bound(&Proposition::LessThan(
                integer(scalar_type, 0),
                subject.clone()
            )),
            Some(Proposition::LessOrEqual(
                integer(scalar_type, 1),
                subject.clone()
            ))
        );
        assert_eq!(
            strict_bound(&Proposition::LessThan(
                subject.clone(),
                integer(scalar_type, 3)
            )),
            Some(Proposition::LessOrEqual(
                subject.clone(),
                integer(scalar_type, 2)
            ))
        );
        assert_eq!(
            strict_bound(&Proposition::LessThan(
                ScalarTerm::integer(scalar_type, scalar_type.maximum_value()).unwrap(),
                subject.clone()
            )),
            Some(Proposition::Falsehood)
        );
        assert_eq!(
            strict_bound(&Proposition::LessThan(
                subject,
                ScalarTerm::integer(scalar_type, scalar_type.minimum_value()).unwrap()
            )),
            Some(Proposition::Falsehood)
        );
    }
    let address = IntegerType::address(64).unwrap();
    assert!(
        strict_bound(&Proposition::LessThan(
            integer(address, 0),
            value(2, ScalarType::Integer(address))
        ))
        .is_none()
    );
}

#[test]
fn condition_alias_cycles_fail_closed_without_a_depth_limit() {
    let mut axioms = Vec::new();
    for index in 1..300 {
        axioms.push(Proposition::Equal(
            value(index, ScalarType::Boolean),
            value(index + 1, ScalarType::Boolean),
        ));
    }
    axioms.push(Proposition::Equal(
        value(300, ScalarType::Boolean),
        ScalarTerm::Boolean(true),
    ));
    assert_eq!(
        condition_fact(ValueId::new(1).unwrap(), true, &axioms, &|id| {
            ScalarTerm::value(id, ScalarType::Boolean)
        }),
        Some(Proposition::Truth)
    );
    axioms.pop();
    axioms.push(Proposition::Equal(
        value(300, ScalarType::Boolean),
        value(1, ScalarType::Boolean),
    ));
    assert!(
        condition_fact(ValueId::new(1).unwrap(), true, &axioms, &|id| {
            ScalarTerm::value(id, ScalarType::Boolean)
        })
        .is_none()
    );
}

#[test]
fn disequality_at_carrier_extrema_does_not_wrap_adjacent_bounds() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let scalar_type = IntegerType::new(sign, 128).unwrap();
        let subject = value(2, ScalarType::Integer(scalar_type));
        for maximum in [false, true] {
            let endpoint = if maximum {
                scalar_type.maximum_value()
            } else {
                scalar_type.minimum_value()
            };
            let adjacent = match (endpoint, maximum) {
                (IntegerValue::Signed(value), true) => IntegerValue::Signed(value - 1),
                (IntegerValue::Signed(value), false) => IntegerValue::Signed(value + 1),
                (IntegerValue::Unsigned(value), true) => IntegerValue::Unsigned(value - 1),
                (IntegerValue::Unsigned(value), false) => IntegerValue::Unsigned(value + 1),
            };
            let adjacent = ScalarTerm::integer(scalar_type, adjacent).unwrap();
            let predicate = ScalarTerm::IntegerEqual {
                scalar_type,
                left: Box::new(subject.clone()),
                right: Box::new(ScalarTerm::integer(scalar_type, endpoint).unwrap()),
            };
            let expected = if maximum {
                Proposition::LessOrEqual(subject.clone(), adjacent)
            } else {
                Proposition::LessOrEqual(adjacent, subject.clone())
            };
            assert_eq!(selected(predicate, false), expected);
        }
    }
}
