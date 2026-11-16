use super::*;
use semantic_vocabulary::{IntegerSign, OperationId};
use terminal_check_tables::values::{ValueOrigin, ValueRow};

#[test]
fn symbolic_ieee_bounds_exclude_their_exact_crash_complements() {
    use semantic_vocabulary::{IeeeFloatComparisonKind as Kind, IeeeFloatFormat, IeeeFloatValue};
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let scalar = ScalarType::IeeeFloat(format);
        let mut values = IdTable::default();
        let id = values.push(ValueRow {
            scalar_type: scalar,
            origin: ValueOrigin::MachineParameter { position: 0 },
            rigid: None,
        });
        let value = ScalarTerm::value(id, scalar);
        let zero = ScalarTerm::IeeeFloatLiteral(match format {
            IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0),
            IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0),
        });
        let one = ScalarTerm::IeeeFloatLiteral(match format {
            IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary32(0x3f80_0000),
            IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(0x3ff0_0000_0000_0000),
        });
        let relation = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        };
        let assumptions = [
            relation(Kind::Equal, value.clone(), value.clone()),
            relation(Kind::LessOrEqual, zero.clone(), value.clone()),
            relation(Kind::LessThan, value.clone(), one.clone()),
        ];
        let failures = [
            relation(Kind::NotEqual, value.clone(), value.clone()),
            relation(Kind::LessThan, value.clone(), zero.clone()),
            relation(Kind::LessOrEqual, one, value.clone()),
        ];
        let context = PropositionContext::from_value_types([(id, scalar)]).unwrap();
        for (index, failure) in failures.iter().enumerate() {
            let goal = Proposition::Implication {
                premise: Box::new(failure.clone()),
                conclusion: Box::new(Proposition::Falsehood),
            };
            // Equality alone is the Eq/Ne premise; the bounds are separate
            // cases, since their positive order already excludes NaN.
            let selected = if index == 0 {
                &assumptions[..1]
            } else {
                &assumptions[..]
            };
            let proof = Prover::new(&context, selected, &[], &values)
                .prove(&goal)
                .unwrap();
            let accepted =
                proof_admission::accept_certificate(&context, &goal, selected, &[], &proof)
                    .unwrap();
            assert!(accepted.semantic_axioms.is_empty());
            assert!(accepted.assumptions.iter().any(|used| used.index == index));
            let mut missing = selected.to_vec();
            missing.remove(index);
            assert!(
                Prover::new(&context, &missing, &[], &values)
                    .prove(&goal)
                    .is_err()
            );
        }
        // An exact logical complement can refute its matching predicate,
        // but does not establish the reversed positive order for NaN.
        let unordered_lower = relation(Kind::NotLessThan, value.clone(), zero.clone());
        let goal = Proposition::Implication {
            premise: Box::new(relation(Kind::LessThan, value.clone(), zero.clone())),
            conclusion: Box::new(Proposition::Falsehood),
        };
        let proof = Prover::new(
            &context,
            std::slice::from_ref(&unordered_lower),
            &[],
            &values,
        )
        .prove(&goal)
        .unwrap();
        proof_admission::accept_certificate(
            &context,
            &goal,
            std::slice::from_ref(&unordered_lower),
            &[],
            &proof,
        )
        .unwrap();
        assert!(
            Prover::new(&context, &[unordered_lower], &[], &values)
                .prove(&relation(Kind::LessOrEqual, zero, value))
                .is_err()
        );
    }
}

#[test]
fn false_ieee_assumptions_use_checked_contradiction_with_the_actual_equation() {
    use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue};
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let scalar = ScalarType::IeeeFloat(format);
        let mut values = IdTable::default();
        let value = values.push(ValueRow {
            scalar_type: scalar,
            origin: ValueOrigin::Operation(OperationId::new(1).unwrap()),
            rigid: None,
        });
        let value = ScalarTerm::value(value, scalar);
        let [zero, quarter, nan] = match format {
            IeeeFloatFormat::Binary32 => {
                [0, 0x3e80_0000, 0x7fc0_0042].map(IeeeFloatValue::Binary32)
            }
            IeeeFloatFormat::Binary64 => {
                [0, 0x3fd0_0000_0000_0000, 0x7ff8_0000_0000_0042].map(IeeeFloatValue::Binary64)
            }
        }
        .map(ScalarTerm::IeeeFloatLiteral);
        let comparison = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        };
        let unordered = comparison(
            IeeeFloatComparisonKind::NotEqual,
            value.clone(),
            value.clone(),
        );
        let invalid = Proposition::Disjunction(vec![
            unordered.clone(),
            comparison(IeeeFloatComparisonKind::LessThan, value.clone(), zero),
        ]);
        let excludes_invalid = Proposition::Implication {
            premise: Box::new(invalid),
            conclusion: Box::new(Proposition::Falsehood),
        };
        let context = PropositionContext::from_value_types(
            values.iter().map(|(id, row)| (id, row.scalar_type)),
        )
        .unwrap();
        for (literal, excludes) in [(quarter, true), (nan, false)] {
            let axioms = [Proposition::Equal(value.clone(), literal)];
            let result = Prover::new(&context, &[], &axioms, &values).prove(&excludes_invalid);
            assert_eq!(result.is_ok(), excludes);
            if let Ok(proof) = result {
                let accepted = proof_admission::accept_certificate(
                    &context,
                    &excludes_invalid,
                    &[],
                    &axioms,
                    &proof,
                )
                .unwrap();
                assert!(accepted.assumptions.is_empty());
                assert_eq!(accepted.semantic_axioms.len(), 1);
                assert_eq!(accepted.semantic_axioms[0].index, 0);
            }
            let assumptions = [Proposition::Truth, unordered.clone()];
            let result = Prover::new(&context, &assumptions, &axioms, &values)
                .prove(&Proposition::Falsehood);
            assert_eq!(result.is_ok(), excludes);
            if let Ok(proof) = result {
                let accepted = proof_admission::accept_certificate(
                    &context,
                    &Proposition::Falsehood,
                    &assumptions,
                    &axioms,
                    &proof,
                )
                .unwrap();
                assert_eq!(accepted.assumptions.len(), 1);
                assert_eq!(accepted.assumptions[0].index, 1);
                assert_eq!(accepted.semantic_axioms.len(), 1);
                assert_eq!(accepted.semantic_axioms[0].index, 0);
            }
        }
    }
}

#[test]
fn a_later_named_result_can_bound_the_same_expression_more_tightly() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [dividend, divisor, first, second] = [
        ValueOrigin::MachineParameter { position: 0 },
        ValueOrigin::MachineParameter { position: 1 },
        ValueOrigin::Operation(OperationId::new(1).unwrap()),
        ValueOrigin::Operation(OperationId::new(2).unwrap()),
    ]
    .map(|origin| {
        let id = values.push(ValueRow {
            scalar_type,
            origin,
            rigid: None,
        });
        ScalarTerm::value(id, scalar_type)
    });
    let expression =
        ScalarTerm::exact_integer_remainder(integer, dividend, divisor.clone()).unwrap();
    let zero = ScalarTerm::integer(integer, integer.zero()).unwrap();
    let endpoint = IntegerValue::Unsigned(100);
    let limit = ScalarTerm::integer(integer, endpoint).unwrap();
    let assumptions = [
        Proposition::LessThan(zero, divisor),
        Proposition::LessOrEqual(second.clone(), limit.clone()),
    ];
    let axioms = [
        Proposition::Equal(first, expression.clone()),
        Proposition::Equal(second, expression.clone()),
    ];

    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let mut prover = Prover::new(&context, &assumptions, &axioms, &values);
    let (literal, proof) = prover.side(&expression, Direction::Upper).unwrap();
    assert_eq!(literal, endpoint);
    let goal = Proposition::LessOrEqual(expression, limit);
    proof_admission::accept_certificate(&context, &goal, &assumptions, &axioms, &proof.unwrap())
        .unwrap();
}

#[test]
fn policy_translation_maps_bounds_only_with_the_required_headroom() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [input, result] = [
        ValueOrigin::MachineParameter { position: 0 },
        ValueOrigin::Operation(OperationId::new(1).unwrap()),
    ]
    .map(|origin| {
        let id = values.push(ValueRow {
            scalar_type,
            origin,
            rigid: None,
        });
        ScalarTerm::value(id, scalar_type)
    });
    let literal = |n| ScalarTerm::integer(integer, IntegerValue::Unsigned(n)).unwrap();
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let add = ScalarTerm::wrapping_integer_add(integer, input.clone(), literal(2)).unwrap();
    let subtract =
        ScalarTerm::wrapping_integer_subtract(integer, input.clone(), literal(2)).unwrap();
    for (expression, guard, dependent_bound, ordinary_bound) in [
        (
            add,
            Proposition::LessOrEqual(input.clone(), literal(9)),
            Proposition::LessOrEqual(literal(2), result.clone()),
            Proposition::LessThan(result.clone(), literal(12)),
        ),
        (
            subtract,
            Proposition::LessOrEqual(literal(2), input.clone()),
            Proposition::LessOrEqual(result.clone(), literal(253)),
            Proposition::LessOrEqual(literal(0), result.clone()),
        ),
    ] {
        let axioms = [Proposition::Equal(result.clone(), expression)];
        let assumptions = [guard];
        let mut prover = Prover::new(&context, &assumptions, &axioms, &values);
        for goal in [&dependent_bound, &ordinary_bound] {
            let proof = prover.prove(goal).unwrap();
            proof_admission::accept_certificate(&context, goal, &assumptions, &axioms, &proof)
                .unwrap();
        }
        assert!(
            Prover::new(&context, &[], &axioms, &values)
                .prove(&dependent_bound)
                .is_err()
        );
    }
}

#[test]
fn remaining_capacity_bounds_the_exact_sum_and_its_use() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let mut values = IdTable::default();
    let [position, count, length, capacity, next] = [
        ValueOrigin::MachineParameter { position: 0 },
        ValueOrigin::MachineParameter { position: 1 },
        ValueOrigin::MachineParameter { position: 2 },
        ValueOrigin::Operation(OperationId::new(1).unwrap()),
        ValueOrigin::Operation(OperationId::new(2).unwrap()),
    ]
    .map(|origin| {
        let id = values.push(ValueRow {
            scalar_type,
            origin,
            rigid: None,
        });
        ScalarTerm::value(id, scalar_type)
    });
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let remaining =
        ScalarTerm::exact_integer_subtract(integer, length.clone(), position.clone()).unwrap();
    let sum = ScalarTerm::exact_integer_add(integer, position.clone(), count.clone()).unwrap();
    let sum_fits = Proposition::IntegerMathLessOrEqual(
        lift_exact_integer_order(&Proposition::LessOrEqual(sum.clone(), sum.clone()))
            .and_then(|goal| match goal {
                Proposition::IntegerMathLessOrEqual(term, _) => Some(term),
                _ => None,
            })
            .unwrap(),
        IntegerMathTerm::literal(integer.maximum_value().unwrap()),
    );
    let capacity_equation = Proposition::Equal(capacity.clone(), remaining);
    let next_equation = Proposition::Equal(next.clone(), sum);
    let assumptions = [
        Proposition::LessOrEqual(position, length.clone()),
        Proposition::LessOrEqual(count.clone(), capacity.clone()),
    ];
    let arithmetic_axioms = [capacity_equation.clone()];
    let evaluated_axioms = [capacity_equation, next_equation];
    let next_in_bounds = Proposition::LessOrEqual(next, length);
    for (goal, axioms) in [
        (&sum_fits, arithmetic_axioms.as_slice()),
        (&next_in_bounds, evaluated_axioms.as_slice()),
    ] {
        let proof = Prover::new(&context, &assumptions, axioms, &values)
            .prove(goal)
            .unwrap();
        proof_admission::accept_certificate(&context, goal, &assumptions, axioms, &proof).unwrap();
        let reversed = [
            assumptions[0].clone(),
            Proposition::LessOrEqual(capacity.clone(), count.clone()),
        ];
        assert!(
            Prover::new(&context, &reversed, axioms, &values)
                .prove(goal)
                .is_err()
        );
    }
}

#[test]
fn ieee_order_search_uses_only_connected_ordered_premises() {
    use semantic_vocabulary::{IeeeFloatComparisonKind as Kind, IeeeFloatFormat, IeeeFloatValue};
    let format = IeeeFloatFormat::Binary64;
    let scalar = ScalarType::IeeeFloat(format);
    let mut values = IdTable::default();
    let id = values.push(ValueRow {
        scalar_type: scalar,
        origin: ValueOrigin::MachineParameter { position: 0 },
        rigid: None,
    });
    let value = ScalarTerm::value(id, scalar);
    let literal = |bits| ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(bits));
    let relation = |kind, left, right| Proposition::ScalarIeeeFloatComparison {
        kind,
        format,
        left,
        right,
    };
    let premise = relation(
        Kind::LessOrEqual,
        value.clone(),
        literal(0x3fd0_0000_0000_0000),
    );
    let goal = relation(
        Kind::LessOrEqual,
        value.clone(),
        literal(0x3fe0_0000_0000_0000),
    );
    let context = PropositionContext::from_value_types([(id, scalar)]).unwrap();
    let assumptions = [premise.clone()];
    let proof = Prover::new(&context, &assumptions, &[], &values)
        .prove(&goal)
        .unwrap();
    proof_admission::accept_certificate(&context, &goal, &assumptions, &[], &proof).unwrap();
    assert!(
        Prover::new(&context, &[], &[], &values)
            .prove(&goal)
            .is_err()
    );
    let complement = relation(
        Kind::NotLessThan,
        literal(0x3fd0_0000_0000_0000),
        value.clone(),
    );
    assert!(
        Prover::new(&context, &[complement], &[], &values)
            .prove(&goal)
            .is_err()
    );
    let nan_bound = relation(
        Kind::LessOrEqual,
        value.clone(),
        literal(0x7ff8_0000_0000_0042),
    );
    assert!(
        Prover::new(&context, &[nan_bound], &[], &values)
            .prove(&goal)
            .is_err()
    );
    let negative_zero = literal(0x8000_0000_0000_0000);
    let zero_bound = relation(Kind::LessOrEqual, value.clone(), literal(0));
    let zero_goal = relation(Kind::LessOrEqual, value, negative_zero);
    let assumptions = [zero_bound];
    let proof = Prover::new(&context, &assumptions, &[], &values)
        .prove(&zero_goal)
        .unwrap();
    proof_admission::accept_certificate(&context, &zero_goal, &assumptions, &[], &proof).unwrap();
}

#[test]
fn ieee_literal_and_copy_equations_reach_the_closed_kernel_judgment() {
    use semantic_vocabulary::{IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue};
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let scalar = ScalarType::IeeeFloat(format);
        let mut values = IdTable::default();
        let [value, copied] = [1, 2].map(|raw| {
            let id = values.push(ValueRow {
                scalar_type: scalar,
                origin: ValueOrigin::Operation(OperationId::new(raw).unwrap()),
                rigid: None,
            });
            ScalarTerm::value(id, scalar)
        });
        let [zero, quarter, half, nan] = match format {
            IeeeFloatFormat::Binary32 => {
                [0, 0x3e80_0000, 0x3f00_0000, 0x7fc0_0042].map(IeeeFloatValue::Binary32)
            }
            IeeeFloatFormat::Binary64 => [
                0,
                0x3fd0_0000_0000_0000,
                0x3fe0_0000_0000_0000,
                0x7ff8_0000_0000_0042,
            ]
            .map(IeeeFloatValue::Binary64),
        }
        .map(ScalarTerm::IeeeFloatLiteral);
        let relation = |left, right| Proposition::ScalarIeeeFloatComparison {
            kind: IeeeFloatComparisonKind::LessOrEqual,
            format,
            left,
            right,
        };
        let goal = Proposition::Conjunction(vec![
            relation(zero, copied.clone()),
            relation(copied.clone(), half),
        ]);
        let context = PropositionContext::from_value_types(
            values.iter().map(|(id, row)| (id, row.scalar_type)),
        )
        .unwrap();
        for (literal, expected) in [(quarter, true), (nan, false)] {
            let axioms = [
                Proposition::Equal(value.clone(), literal),
                Proposition::Equal(copied.clone(), value.clone()),
            ];
            let result = Prover::new(&context, &[], &axioms, &values).prove(&goal);
            assert_eq!(result.is_ok(), expected);
            if let Ok(proof) = result {
                let accepted =
                    proof_admission::accept_certificate(&context, &goal, &[], &axioms, &proof)
                        .unwrap();
                assert_eq!(accepted.semantic_axioms.len(), 2);
            }
        }
    }
}

#[test]
fn compound_exact_orders_prove_value_relations_and_reject_unsound_variants() {
    use proof_admission::MathematicalCoreDecision;

    let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let u64_scalar = ScalarType::Integer(u64_type);
    let mut values = IdTable::default();
    let [index, offset, length] = [0, 1, 2].map(|position| {
        let id = values.push(ValueRow {
            scalar_type: u64_scalar,
            origin: ValueOrigin::MachineParameter { position },
            rigid: None,
        });
        ScalarTerm::value(id, u64_scalar)
    });
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let lit = |n| ScalarTerm::integer(u64_type, IntegerValue::Unsigned(n)).unwrap();
    let add = |a: &ScalarTerm, b: &ScalarTerm| {
        ScalarTerm::exact_integer_add(u64_type, a.clone(), b.clone()).unwrap()
    };
    let sub = |a: &ScalarTerm, b: &ScalarTerm| {
        ScalarTerm::exact_integer_subtract(u64_type, a.clone(), b.clone()).unwrap()
    };

    let index_below_length = Proposition::LessThan(index.clone(), length.clone());
    let offset_below_length = Proposition::LessThan(offset.clone(), length.clone());
    let one = lit(1);
    for (assumption, goal) in [
        (
            Proposition::LessThan(add(&index, &one), length.clone()),
            &index_below_length,
        ),
        (
            Proposition::LessThan(add(&one, &index), length.clone()),
            &index_below_length,
        ),
        (
            Proposition::LessOrEqual(add(&add(&index, &one), &one), length.clone()),
            &index_below_length,
        ),
        (
            Proposition::LessThan(add(&index, &offset), length.clone()),
            &index_below_length,
        ),
        (
            Proposition::LessThan(add(&index, &offset), length.clone()),
            &offset_below_length,
        ),
        (
            Proposition::Equal(add(&index, &one), length.clone()),
            &index_below_length,
        ),
        (
            Proposition::LessOrEqual(index.clone(), sub(&length, &one)),
            &index_below_length,
        ),
        (
            Proposition::LessThan(index.clone(), sub(&length, &offset)),
            &index_below_length,
        ),
        (
            Proposition::LessOrEqual(add(&index, &one), sub(&length, &one)),
            &index_below_length,
        ),
    ] {
        let assumptions = [assumption];
        let proof = Prover::new(&context, &assumptions, &[], &values)
            .prove(goal)
            .unwrap_or_else(|err| panic!("failed on {assumptions:?} |- {goal:?}: {err}"));
        let accepted =
            proof_admission::accept_certificate(&context, goal, &assumptions, &[], &proof).unwrap();
        assert!(
            matches!(
                accepted.mathematical_core,
                MathematicalCoreDecision::Judged(_)
            ),
            "{:?}",
            accepted.mathematical_core
        );
    }

    // Insufficient headroom, wrapping arithmetic, and saturating arithmetic must fail closed.
    let wrapping_plus_one =
        ScalarTerm::wrapping_integer_add(u64_type, index.clone(), one.clone()).unwrap();
    let saturating_plus_one =
        ScalarTerm::saturating_integer_add(u64_type, index.clone(), one.clone()).unwrap();
    for (assumption, invalid_goal) in [
        (
            Proposition::LessThan(index.clone(), length.clone()),
            Proposition::LessThan(add(&index, &one), length.clone()),
        ),
        (
            Proposition::LessOrEqual(add(&index, &one), length.clone()),
            Proposition::LessThan(add(&index, &one), length.clone()),
        ),
        (
            Proposition::LessThan(wrapping_plus_one, length.clone()),
            index_below_length.clone(),
        ),
        (
            Proposition::LessThan(saturating_plus_one, length.clone()),
            index_below_length,
        ),
    ] {
        let assumptions = [assumption];
        assert!(
            Prover::new(&context, &assumptions, &[], &values)
                .prove(&invalid_goal)
                .is_err(),
            "unexpectedly proved {assumptions:?} |- {invalid_goal:?}"
        );
    }

    // Signed offsets may be negative unless proved `>= 0` or `> 0`.
    let i64_type = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let i64_scalar = ScalarType::Integer(i64_type);
    let mut signed_values = IdTable::default();
    let [a, d, b] = [0, 1, 2].map(|position| {
        let id = signed_values.push(ValueRow {
            scalar_type: i64_scalar,
            origin: ValueOrigin::MachineParameter { position },
            rigid: None,
        });
        ScalarTerm::value(id, i64_scalar)
    });
    let signed_context = PropositionContext::from_value_types(
        signed_values.iter().map(|(id, row)| (id, row.scalar_type)),
    )
    .unwrap();
    let signed_zero = ScalarTerm::integer(i64_type, IntegerValue::Signed(0)).unwrap();
    let signed_sum = ScalarTerm::exact_integer_add(i64_type, a.clone(), d.clone()).unwrap();
    let signed_diff = ScalarTerm::exact_integer_subtract(i64_type, b.clone(), d.clone()).unwrap();
    let signed_goal = Proposition::LessOrEqual(a.clone(), b.clone());
    let signed_strict_goal = Proposition::LessThan(a.clone(), b.clone());
    for unguarded in [
        Proposition::LessThan(signed_sum.clone(), b.clone()),
        Proposition::LessOrEqual(a, signed_diff),
    ] {
        assert!(
            Prover::new(&signed_context, &[unguarded], &[], &signed_values)
                .prove(&signed_goal)
                .is_err()
        );
    }
    let guarded = [
        Proposition::LessThan(signed_sum, b),
        Proposition::LessOrEqual(signed_zero, d),
    ];
    let proof = Prover::new(&signed_context, &guarded, &[], &signed_values)
        .prove(&signed_strict_goal)
        .unwrap();
    let accepted = proof_admission::accept_certificate(
        &signed_context,
        &signed_strict_goal,
        &guarded,
        &[],
        &proof,
    )
    .unwrap();
    assert!(matches!(
        accepted.mathematical_core,
        MathematicalCoreDecision::Judged(_)
    ));
}

#[test]
fn compound_constant_offsets_tighten_literal_and_chained_bounds_without_poisoning_ssa_results() {
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let u8_scalar = ScalarType::Integer(u8_type);
    let mut values = IdTable::default();
    let [index_id, limit_id, count_id, next_id] = [
        ValueOrigin::MachineParameter { position: 0 },
        ValueOrigin::MachineParameter { position: 1 },
        ValueOrigin::MachineParameter { position: 2 },
        ValueOrigin::Operation(OperationId::new(1).unwrap()),
    ]
    .map(|origin| {
        values.push(ValueRow {
            scalar_type: u8_scalar,
            origin,
            rigid: None,
        })
    });
    let [index, limit, count, next] =
        [index_id, limit_id, count_id, next_id].map(|id| ScalarTerm::value(id, u8_scalar));
    let context =
        PropositionContext::from_value_types(values.iter().map(|(id, row)| (id, row.scalar_type)))
            .unwrap();
    let lit = |n| ScalarTerm::integer(u8_type, IntegerValue::Unsigned(n)).unwrap();
    let add = |a: &ScalarTerm, b: &ScalarTerm| {
        ScalarTerm::exact_integer_add(u8_type, a.clone(), b.clone()).unwrap()
    };
    let sub = |a: &ScalarTerm, b: &ScalarTerm| {
        ScalarTerm::exact_integer_subtract(u8_type, a.clone(), b.clone()).unwrap()
    };

    // `index + 1 < 4` tightens `index` all the way to `<= 2`, not just `<= 3`.
    let assumptions = [Proposition::LessThan(add(&index, &lit(1)), lit(4))];
    let mut prover = Prover::new(&context, &assumptions, &[], &values);
    assert_eq!(
        prover.literal_bounds(index_id),
        (None, Some(IntegerValue::Unsigned(2)))
    );
    let exact_goal = Proposition::LessOrEqual(index.clone(), lit(2));
    let proof = prover.prove(&exact_goal).unwrap();
    proof_admission::accept_certificate(&context, &exact_goal, &assumptions, &[], &proof).unwrap();
    assert!(
        prover
            .prove(&Proposition::LessOrEqual(index.clone(), lit(1)))
            .is_err()
    );

    // `index + 2 <= limit` with `limit <= 10` yields `index <= 8`, and
    // `0 < count - 1` yields `2 <= count`.
    let chained = [
        Proposition::LessOrEqual(add(&index, &lit(2)), limit.clone()),
        Proposition::LessOrEqual(limit, lit(10)),
        Proposition::LessThan(lit(0), sub(&count, &lit(1))),
    ];
    let mut prover = Prover::new(&context, &chained, &[], &values);
    for goal in [
        Proposition::LessOrEqual(index.clone(), lit(8)),
        Proposition::LessOrEqual(lit(2), count.clone()),
    ] {
        let proof = prover.prove(&goal).unwrap();
        proof_admission::accept_certificate(&context, &goal, &chained, &[], &proof).unwrap();
    }
    assert!(
        prover
            .prove(&Proposition::LessOrEqual(index.clone(), lit(7)))
            .is_err()
    );
    assert!(
        prover
            .prove(&Proposition::LessOrEqual(lit(3), count))
            .is_err()
    );

    // Querying `index`'s upper bound first must not poison an SSA result `next = index + 1`.
    let ssa_assumptions = [Proposition::LessOrEqual(index.clone(), lit(5))];
    let ssa_axioms = [Proposition::Equal(next.clone(), add(&index, &lit(1)))];
    let mut prover = Prover::new(&context, &ssa_assumptions, &ssa_axioms, &values);
    assert_eq!(
        prover.literal_bounds(index_id),
        (None, Some(IntegerValue::Unsigned(5)))
    );
    let next_goal = Proposition::LessOrEqual(next, lit(6));
    let proof = prover.prove(&next_goal).unwrap();
    proof_admission::accept_certificate(
        &context,
        &next_goal,
        &ssa_assumptions,
        &ssa_axioms,
        &proof,
    )
    .unwrap();
}
