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
