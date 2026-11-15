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
