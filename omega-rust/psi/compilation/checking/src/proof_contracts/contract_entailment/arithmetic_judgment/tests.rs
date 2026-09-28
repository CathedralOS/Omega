use super::{
    BigInt, BinaryOperator, Engine, ExpressionNode, Interval, Polynomial, TypedTrees,
    proof_integer_expression,
};

#[test]
fn symbolic_quotient_intervals_cover_every_small_signed_rectangle() {
    for dividend_low in -4..=4 {
        for dividend_high in dividend_low..=4 {
            for divisor_low in -4..=4 {
                for divisor_high in divisor_low..=4 {
                    let interval = |low, high| Interval {
                        low: Some(BigInt::from_i64(low)),
                        high: Some(BigInt::from_i64(high)),
                    };
                    let computed = Engine::bounded_quotient_interval(
                        &interval(dividend_low, dividend_high),
                        &interval(divisor_low, divisor_high),
                    );
                    if divisor_low <= 0 && divisor_high >= 0 {
                        assert_eq!(computed, Interval::unbounded());
                        continue;
                    }
                    let actual = (dividend_low..=dividend_high).flat_map(|dividend| {
                        (divisor_low..=divisor_high).map(move |divisor| dividend / divisor)
                    });
                    assert_eq!(
                        computed,
                        interval(actual.clone().min().unwrap(), actual.max().unwrap()),
                        "{dividend_low}..={dividend_high} divided by {divisor_low}..={divisor_high}"
                    );
                }
            }
        }
    }
    assert_eq!(
        Engine::bounded_quotient_interval(
            &Interval::unbounded(),
            &Interval::constant(BigInt::from_i64(2))
        ),
        Interval::unbounded(),
    );
    let dividend = Interval {
        low: Some(BigInt::from_i64(-7)),
        high: Some(BigInt::from_i64(5)),
    };
    for (divisor, low, high) in [
        (
            Interval {
                low: Some(BigInt::from_i64(2)),
                high: None,
            },
            -3,
            2,
        ),
        (
            Interval {
                low: None,
                high: Some(BigInt::from_i64(-2)),
            },
            -2,
            3,
        ),
    ] {
        assert_eq!(
            Engine::bounded_quotient_interval(&dividend, &divisor),
            Interval {
                low: Some(BigInt::from_i64(low)),
                high: Some(BigInt::from_i64(high))
            },
        );
    }
    assert_eq!(
        Engine::bounded_quotient_interval(
            &Interval::constant(BigInt::from_i64(7)),
            &Interval {
                low: Some(BigInt::from_i64(2)),
                high: None
            }
        ),
        Interval {
            low: Some(BigInt::zero()),
            high: Some(BigInt::from_i64(3))
        },
    );
}

#[test]
fn proof_integer_arithmetic_lands_complete_anonymous_peers() {
    for (arithmetic, expected) in [
        ("embed(7i32) / (1 + 1)", Some(3)),
        ("embed(7i32) % (1 + 1)", Some(1)),
        ("(1 + 6) / embed(2i32)", Some(3)),
        ("(1 + 6) % embed(2i32)", Some(1)),
        ("embed(7i32) / (1 / 2 * 2)", Some(7)),
        ("embed(7i32) % (1 / 2 * 2)", Some(0)),
        ("embed(7i32) / (1 / 2)", None),
        ("embed(7i32) % (1 / 2)", None),
    ] {
        let source = format!("machine predicate() ensures {arithmetic} == 0 {{}}");
        let program = crate::front_end::typed_program(&source);
        let expression = program
            .expression_table
            .iter_expressions()
            .find_map(|(_, node)| match node {
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
                    Some(binary.left)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(
            proof_integer_expression(&program, expression),
            expected.is_some(),
            "{arithmetic}"
        );
        let mut engine = Engine::for_proof_integer_formation(&program);
        assert_eq!(
            engine.normalize(expression),
            expected.map(|value| Polynomial::constant(BigInt::from_i64(value))),
            "{arithmetic}"
        );
    }
}

#[test]
fn disequality_strengthens_only_an_independently_proven_integer_direction() {
    let program = TypedTrees::default();
    let remaining = Polynomial::atom("remaining".to_owned());
    let floor = Polynomial::atom("floor".to_owned());
    for operator in [BinaryOperator::GreaterOrEqual, BinaryOperator::LessOrEqual] {
        for reverse in [false, true] {
            let mut engine = Engine::for_proof_integer_formation(&program);
            let mut comparisons = vec![
                (BinaryOperator::NotEqual, remaining.clone(), floor.clone()),
                (operator, remaining.clone(), floor.clone()),
            ];
            if reverse {
                comparisons.reverse();
            }
            assert!(engine.install_hypotheses(comparisons));
            let difference = if operator == BinaryOperator::GreaterOrEqual {
                remaining.sub(&floor)
            } else {
                floor.sub(&remaining)
            };
            assert!(engine.prove_at_least(&difference, &BigInt::from_i64(1)));
            assert!(!engine.prove_at_least(&difference, &BigInt::from_i64(2)));
            assert!(!engine.prove_at_least(&difference.neg(), &BigInt::zero()));
            assert!(!engine.requires_unsatisfiable);
        }
    }
    let mut unknown = Engine::for_proof_integer_formation(&program);
    assert!(unknown.install_hypotheses(vec![(
        BinaryOperator::NotEqual,
        remaining.clone(),
        floor.clone()
    ),]));
    assert!(!unknown.prove_at_least(&remaining.sub(&floor), &BigInt::from_i64(1)));
    assert!(!unknown.prove_at_least(&floor.sub(&remaining), &BigInt::from_i64(1)));
    assert!(!unknown.requires_unsatisfiable);
    let mut contradictory = Engine::for_proof_integer_formation(&program);
    assert!(contradictory.install_hypotheses(vec![
        (BinaryOperator::NotEqual, remaining.clone(), floor.clone()),
        (BinaryOperator::Equal, remaining, floor),
    ]));
    assert!(contradictory.requires_unsatisfiable);
}

#[test]
fn constant_multiplication_preserves_one_sided_bounds() {
    let positive = Interval {
        low: Some(BigInt::from_i64(2)),
        high: None,
    };
    let one = Interval::constant(BigInt::from_i64(1));
    assert_eq!(one.multiply(&positive), positive);
    assert_eq!(positive.multiply(&one), positive);
    let negative = Interval::constant(BigInt::from_i64(-3));
    let reflected = Interval {
        low: None,
        high: Some(BigInt::from_i64(-6)),
    };
    assert_eq!(negative.multiply(&positive), reflected);
    assert_eq!(positive.multiply(&negative), reflected);
    let zero = Interval::constant(BigInt::zero());
    assert_eq!(Interval::unbounded().multiply(&zero), zero);
    assert_eq!(zero.multiply(&Interval::unbounded()), zero);
}
