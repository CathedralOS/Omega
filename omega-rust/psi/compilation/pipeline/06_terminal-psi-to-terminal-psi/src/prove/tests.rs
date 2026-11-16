use super::*;
use semantic_vocabulary::{IntegerSign, OperationId};
use terminal_check_tables::values::{ValueOrigin, ValueRow};

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
