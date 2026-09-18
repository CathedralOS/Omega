//! Rational bound match dispatch tests.

use super::{
    BigRational, BinaryOperator, ExpressionNode, RationalBounds, RationalInterval, RationalLattice,
    excludes_zero,
};
use numerics::bignum::BigInt;

fn fraction(numerator: i64, denominator: i64) -> BigRational {
    BigRational::from_integer(BigInt::from_i64(numerator))
        .div(&BigRational::from_integer(BigInt::from_i64(denominator)))
        .expect("nonzero test denominator")
}

fn lattice_contains(lattice: &RationalLattice, value: &BigRational) -> bool {
    let difference = value.sub(&lattice.offset);
    if lattice.stride.is_zero() {
        difference.is_zero()
    } else {
        difference
            .div(&lattice.stride)
            .is_some_and(|multiple| multiple.to_integer_exact().is_some())
    }
}

#[test]
fn lattice_zero_neighbors_preserve_exact_rational_spacing() {
    for offset_numerator in -7..=7 {
        for stride_numerator in -5..=5 {
            let lattice = RationalLattice {
                offset: fraction(offset_numerator, 3),
                stride: fraction(stride_numerator, 2),
            };
            let Some([negative, positive]) = lattice.zero_neighbors() else {
                assert!(
                    lattice.stride.is_zero() || lattice_contains(&lattice, &BigRational::zero())
                );
                continue;
            };
            assert!(!lattice_contains(&lattice, &BigRational::zero()));
            assert!(negative.cmp_value(&BigRational::zero()).is_lt());
            assert!(positive.cmp_value(&BigRational::zero()).is_gt());
            assert!(lattice_contains(&lattice, &negative));
            assert!(lattice_contains(&lattice, &positive));
            let spacing = if lattice.stride.is_negative() {
                lattice.stride.negate()
            } else {
                lattice.stride.clone()
            };
            assert_eq!(positive.sub(&negative), spacing);
        }
    }
}

#[test]
fn lattice_gap_refinement_retains_every_sampled_point_in_original_bounds() {
    for offset_numerator in -4..=4 {
        for stride_numerator in -3..=3 {
            for low_numerator in -3..=0 {
                for high_numerator in 0..=3 {
                    let lattice = RationalLattice {
                        offset: fraction(offset_numerator, 3),
                        stride: fraction(stride_numerator, 2),
                    };
                    let low = fraction(low_numerator, 2);
                    let high = fraction(high_numerator, 2);
                    let mut points = Vec::new();
                    for multiple in -12..=12 {
                        let point = lattice
                            .offset
                            .add(&lattice.stride.mul(&fraction(multiple, 1)));
                        if !point.cmp_value(&low).is_lt() && !point.cmp_value(&high).is_gt() {
                            points.push(point);
                        }
                    }
                    let mut bounds = RationalBounds {
                        containing_zero: Some(RationalInterval {
                            low: low.clone(),
                            high: high.clone(),
                        }),
                        lattice: Some(lattice),
                        fractional_history: true,
                        ..RationalBounds::default()
                    };
                    bounds.refine_zero_gap();
                    assert!(bounds.intervals().count() <= 3);
                    assert!(bounds.fractional_history);
                    for interval in bounds.intervals() {
                        assert!(!interval.low.cmp_value(&low).is_lt());
                        assert!(!interval.high.cmp_value(&high).is_gt());
                        assert!(!interval.low.cmp_value(&interval.high).is_gt());
                    }
                    for point in points {
                        assert!(
                            bounds
                                .intervals()
                                .any(|interval| !point.cmp_value(&interval.low).is_lt()
                                    && !point.cmp_value(&interval.high).is_gt())
                        );
                        if point.is_zero() {
                            assert!(!bounds.excludes_zero());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn joining_a_zero_arm_restores_the_nonzero_obligation() {
    for values in [[1, 3], [3, 1]] {
        let mut bounds = RationalBounds::constant(fraction(values[0], 1));
        bounds.include(RationalBounds::constant(fraction(values[1], 1)));
        let mut difference = bounds
            .apply(
                BinaryOperator::Subtract,
                &RationalBounds::constant(fraction(2, 1)),
            )
            .expect("defined difference");
        assert!(difference.excludes_zero(), "arithmetic split the zero gap");
        difference.include(RationalBounds::constant(fraction(0, 1)));
        difference.refine_zero_gap();
        assert!(!difference.excludes_zero());
        assert!(
            RationalBounds::constant(fraction(1, 1))
                .apply(BinaryOperator::Divide, &difference)
                .is_err()
        );
    }
}

#[test]
fn rational_lattice_operations_contain_sampled_exact_values() {
    for left_offset in -2..=2 {
        for right_offset in -2..=2 {
            for left_stride in 0..=2 {
                for right_stride in 0..=2 {
                    let left = RationalLattice {
                        offset: fraction(left_offset, 2),
                        stride: fraction(left_stride, 2),
                    };
                    let right = RationalLattice {
                        offset: fraction(right_offset, 3),
                        stride: fraction(right_stride, 3),
                    };
                    let joined = left.join(&right).expect("rational gcd");
                    for operator in [
                        BinaryOperator::Add,
                        BinaryOperator::Subtract,
                        BinaryOperator::Multiply,
                        BinaryOperator::Divide,
                    ] {
                        let Some(result) = left.apply(operator, &right) else {
                            assert_eq!(operator, BinaryOperator::Divide);
                            assert!(right_stride != 0 || right_offset == 0);
                            continue;
                        };
                        for left_multiple in -2..=2 {
                            let left_value = left
                                .offset
                                .add(&left.stride.mul(&fraction(left_multiple, 1)));
                            assert!(lattice_contains(&joined, &left_value));
                            for right_multiple in -2..=2 {
                                let right_value = right
                                    .offset
                                    .add(&right.stride.mul(&fraction(right_multiple, 1)));
                                assert!(lattice_contains(&joined, &right_value));
                                let value = match operator {
                                    BinaryOperator::Add => left_value.add(&right_value),
                                    BinaryOperator::Subtract => left_value.sub(&right_value),
                                    BinaryOperator::Multiply => left_value.mul(&right_value),
                                    BinaryOperator::Divide => {
                                        left_value.div(&right_value).expect("fixed nonzero divisor")
                                    }
                                    _ => unreachable!(),
                                };
                                assert!(
                                    lattice_contains(&result, &value),
                                    "{operator:?}: {value:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn singleton_divisor_partitions_preserve_exact_quotient_lattices() {
    for numerator_denominator in [1, 2] {
        let mut numerators = RationalBounds::constant(fraction(12, numerator_denominator));
        numerators.include(RationalBounds::constant(fraction(
            25,
            numerator_denominator,
        )));
        for negative_divisor in [-3, -2, -1] {
            for positive_divisor in [1, 2, 3] {
                for divisor_denominator in [1, 2] {
                    let mut divisors =
                        RationalBounds::constant(fraction(negative_divisor, divisor_denominator));
                    divisors.include(RationalBounds::constant(fraction(
                        positive_divisor,
                        divisor_denominator,
                    )));
                    let quotients = numerators
                        .apply(BinaryOperator::Divide, &divisors)
                        .expect("nonzero singleton divisors");
                    let lattice = quotients.lattice.as_ref().expect("joined quotient lattice");
                    for numerator in [12, 25] {
                        for divisor in [negative_divisor, positive_divisor] {
                            let quotient = fraction(numerator, numerator_denominator)
                                .div(&fraction(divisor, divisor_denominator))
                                .expect("nonzero divisor");
                            assert!(lattice_contains(lattice, &quotient));
                        }
                    }
                    assert_eq!(
                        quotients.fractional_history,
                        numerators.fractional_history
                            || divisors.fractional_history
                            || !lattice.is_integral()
                    );
                }
            }
        }
    }
    let mut divisors = RationalBounds::constant(fraction(-2, 1));
    divisors.include(RationalBounds::constant(fraction(2, 1)));
    let mut numerator = RationalBounds::constant(fraction(14, 1));
    numerator.fractional_history = true;
    let quotient = numerator
        .apply(BinaryOperator::Divide, &divisors)
        .expect("integral quotient");
    assert!(
        quotient
            .lattice
            .as_ref()
            .expect("quotient lattice")
            .is_integral()
    );
    assert!(
        quotient.fractional_history,
        "integrality cannot erase inherited warning evidence"
    );
}

#[test]
fn integer_endpoints_do_not_prove_fractional_interiors_integral() {
    let mut bounds = RationalBounds::constant(fraction(1, 1));
    bounds.include(RationalBounds::constant(fraction(3, 2)));
    bounds.include(RationalBounds::constant(fraction(2, 1)));
    assert!(
        bounds
            .intervals()
            .all(|interval| interval.low.to_integer_exact().is_some()
                && interval.high.to_integer_exact().is_some())
    );
    assert!(
        !bounds
            .lattice
            .as_ref()
            .expect("rational lattice")
            .is_integral()
    );
    let doubled = bounds
        .apply(
            BinaryOperator::Multiply,
            &RationalBounds::constant(fraction(2, 1)),
        )
        .expect("defined arithmetic");
    assert!(
        doubled
            .lattice
            .as_ref()
            .expect("transported lattice")
            .is_integral()
    );
    assert!(
        doubled.fractional_history,
        "cancellation cannot erase diagnostic history"
    );

    let mut divisors = RationalBounds::constant(fraction(1, 1));
    divisors.include(RationalBounds::constant(fraction(4, 1)));
    divisors.include(RationalBounds::constant(fraction(6, 1)));
    let quotients = RationalBounds::constant(fraction(6, 1))
        .apply(BinaryOperator::Divide, &divisors)
        .expect("nonzero interval");
    assert!(
        quotients
            .intervals()
            .all(|interval| interval.low.to_integer_exact().is_some()
                && interval.high.to_integer_exact().is_some())
    );
    let lattice = quotients
        .lattice
        .as_ref()
        .expect("admissible quotients retain a lattice");
    assert!(
        lattice_contains(lattice, &fraction(3, 2)) && !lattice.is_integral(),
        "6/4 is fractional inside integer extrema 1..6"
    );
}

#[test]
fn nonconstant_divisors_enumerate_their_admissible_lattice_points() {
    // A joined same-sign hull is wider than a singleton, yet its lattice
    // still names each admissible divisor exactly: {2,3} divides 24 into an
    // integral quotient set.
    let mut divisors = RationalBounds::constant(fraction(2, 1));
    divisors.include(RationalBounds::constant(fraction(3, 1)));
    let quotients = RationalBounds::constant(fraction(24, 1))
        .apply(BinaryOperator::Divide, &divisors)
        .expect("nonzero same-sign hull");
    let lattice = quotients
        .lattice
        .as_ref()
        .expect("enumerated quotient lattice");
    for quotient in [12, 8] {
        assert!(lattice_contains(lattice, &fraction(quotient, 1)));
    }
    assert!(lattice.is_integral());

    // Opposite-sign hulls enumerate independently.
    let mut divisors = RationalBounds::constant(fraction(-3, 1));
    divisors.include(RationalBounds::constant(fraction(-2, 1)));
    divisors.include(RationalBounds::constant(fraction(2, 1)));
    divisors.include(RationalBounds::constant(fraction(3, 1)));
    let quotients = RationalBounds::constant(fraction(12, 1))
        .apply(BinaryOperator::Divide, &divisors)
        .expect("nonzero opposite-sign hulls");
    let lattice = quotients
        .lattice
        .as_ref()
        .expect("enumerated quotient lattice");
    for quotient in [-6, -4, 6, 4] {
        assert!(lattice_contains(lattice, &fraction(quotient, 1)));
    }
    assert!(lattice.is_integral());

    // Fractional divisor points divide exactly as rationals.
    let mut divisors = RationalBounds::constant(fraction(1, 2));
    divisors.include(RationalBounds::constant(fraction(3, 2)));
    let quotients = RationalBounds::constant(fraction(6, 1))
        .apply(BinaryOperator::Divide, &divisors)
        .expect("nonzero fractional hull");
    let lattice = quotients
        .lattice
        .as_ref()
        .expect("enumerated quotient lattice");
    for quotient in [12, 4] {
        assert!(lattice_contains(lattice, &fraction(quotient, 1)));
    }
    assert!(lattice.is_integral());
}

#[test]
fn nonconstant_divisor_enumeration_is_hull_exact_and_bounded() {
    // A hull point off the divisor lattice is not admissible: {3} on 0+2Z
    // contributes nothing, leaving no quotient lattice rather than dividing
    // by a value the bound cannot produce.
    let mut divisors = RationalBounds {
        positive: Some(RationalInterval::constant(fraction(3, 1))),
        lattice: Some(RationalLattice {
            offset: BigRational::zero(),
            stride: fraction(2, 1),
        }),
        ..RationalBounds::default()
    };
    let quotients = RationalBounds::constant(fraction(6, 1))
        .apply(BinaryOperator::Divide, &divisors)
        .expect("nonzero hull");
    assert!(quotients.lattice.is_none());

    // A wider admissible set than the enumeration bound declines rather than
    // scanning a range no authored bound could distinguish.
    divisors = RationalBounds {
        positive: Some(RationalInterval {
            low: fraction(1, 1),
            high: fraction(1_000_000, 1),
        }),
        lattice: Some(RationalLattice {
            offset: BigRational::zero(),
            stride: fraction(1, 1),
        }),
        ..RationalBounds::default()
    };
    let quotients = RationalBounds::constant(fraction(6, 1))
        .apply(BinaryOperator::Divide, &divisors)
        .expect("nonzero hull");
    assert!(quotients.lattice.is_none());

    // An admissible zero divisor still propagates the nonzero obligation.
    let divisors = RationalBounds {
        containing_zero: Some(RationalInterval {
            low: fraction(-1, 1),
            high: fraction(2, 1),
        }),
        lattice: Some(RationalLattice {
            offset: BigRational::zero(),
            stride: fraction(1, 1),
        }),
        ..RationalBounds::default()
    };
    assert!(
        RationalBounds::constant(fraction(6, 1))
            .apply(BinaryOperator::Divide, &divisors)
            .is_err()
    );
}

#[test]
fn rational_bounds_contain_interior_and_endpoint_arithmetic() {
    for left_low in -2..=2 {
        for left_high in left_low..=2 {
            for right_low in -2..=2 {
                for right_high in right_low..=2 {
                    for operator in [
                        BinaryOperator::Add,
                        BinaryOperator::Subtract,
                        BinaryOperator::Multiply,
                        BinaryOperator::Divide,
                    ] {
                        let left = RationalInterval {
                            low: fraction(left_low, 2),
                            high: fraction(left_high, 2),
                        };
                        let right = RationalInterval {
                            low: fraction(right_low, 2),
                            high: fraction(right_high, 2),
                        };
                        let result = left.apply(operator, &right);
                        if operator == BinaryOperator::Divide && right_low <= 0 && right_high >= 0 {
                            assert!(result.is_err(), "zero-containing denominator interval");
                            continue;
                        }
                        let bounds = result.expect("defined rational bounds");
                        for left in [
                            fraction(left_low, 2),
                            fraction(left_low + left_high, 4),
                            fraction(left_high, 2),
                        ] {
                            for right in [
                                fraction(right_low, 2),
                                fraction(right_low + right_high, 4),
                                fraction(right_high, 2),
                            ] {
                                let actual = match operator {
                                    BinaryOperator::Add => left.add(&right),
                                    BinaryOperator::Subtract => left.sub(&right),
                                    BinaryOperator::Multiply => left.mul(&right),
                                    BinaryOperator::Divide => {
                                        left.div(&right).expect("zero-free interval")
                                    }
                                    _ => unreachable!(),
                                };
                                assert!(!bounds.low.cmp_value(&actual).is_gt());
                                assert!(!bounds.high.cmp_value(&actual).is_lt());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn sign_partitioned_bounds_preserve_all_category_pairs() {
    fn bounds(categories: u8) -> RationalBounds {
        let mut result = RationalBounds::default();
        for (category, low, high) in [(1, -3, -1), (2, 1, 3), (4, -1, 1)] {
            if categories & category != 0 {
                result.include_interval(RationalInterval {
                    low: fraction(low, 2),
                    high: fraction(high, 2),
                });
            }
        }
        result
    }

    let empty = RationalBounds::default();
    assert!(!empty.excludes_zero());
    assert!(empty.apply(BinaryOperator::Add, &bounds(1)).is_err());
    assert!(bounds(1).apply(BinaryOperator::Add, &empty).is_err());
    for left_categories in 1..8 {
        for right_categories in 1..8 {
            let left = bounds(left_categories);
            let right = bounds(right_categories);
            for operator in [
                BinaryOperator::Add,
                BinaryOperator::Subtract,
                BinaryOperator::Multiply,
                BinaryOperator::Divide,
            ] {
                let result = left.apply(operator, &right);
                if operator == BinaryOperator::Divide && right_categories & 4 != 0 {
                    assert!(
                        result.is_err(),
                        "zero-containing alternatives cannot be discarded"
                    );
                    continue;
                }
                let result = result.expect("defined category arithmetic");
                assert!(result.intervals().count() <= 3);
                for left in left.intervals() {
                    for right in right.intervals() {
                        for left in [&left.low, &left.high] {
                            for right in [&right.low, &right.high] {
                                let actual = match operator {
                                    BinaryOperator::Add => left.add(right),
                                    BinaryOperator::Subtract => left.sub(right),
                                    BinaryOperator::Multiply => left.mul(right),
                                    BinaryOperator::Divide => {
                                        left.div(right).expect("zero-free denominator")
                                    }
                                    _ => unreachable!(),
                                };
                                assert!(
                                    result.intervals().any(|range| !range
                                        .low
                                        .cmp_value(&actual)
                                        .is_gt()
                                        && !range.high.cmp_value(&actual).is_lt())
                                );
                                if actual.is_zero() {
                                    assert!(!result.excludes_zero());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn rational_bounds_visit_result_operations_once_without_subject_execution() {
    use source_files_to_tokens::Lexer;
    use typed_trees::statement::StatementNode;

    for (term, operation, expected_operator) in [
        (
            "(match (1u8 / 0 == 0) { true -> 1, false -> 2 })",
            " + ",
            BinaryOperator::Add,
        ),
        (
            "(match (1u8 / 0 == 0) { true -> -1, false -> 1 })",
            " * ",
            BinaryOperator::Multiply,
        ),
    ] {
        let expression = std::iter::repeat_n(term, 24)
            .collect::<Vec<_>>()
            .join(operation);
        let source = format!("machine choose() -> u8 {{ {expression} }}");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(
            &Lexer::new(&source).tokenize().expect("tokens"),
        )
        .expect("syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolved");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("typed");
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let StatementNode::Expression(root) =
            program.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("expression fixture");
        };
        super::super::validate_graph(&program, root).expect("validated caller precondition");
        let mut visits = 0;
        assert!(excludes_zero(&program, root, |expression| {
        visits += 1;
        assert!(matches!(program.expression_table.expression(expression), ExpressionNode::Binary(binary) if binary.operator == expected_operator));
        validation::has_builtin_binary_expression_meaning(&program, machine, Some(state), expression)
    }).expect("zero-free sum or product"));
        assert_eq!(
            visits, 23,
            "one visit per operation, no branch combinations or subject operations"
        );
        assert!(
            excludes_zero(&program, root, |_| false).is_err(),
            "token spelling grants no builtin authority"
        );
    }
}
