use super::{IntegerMathTerm, Proposition, ScalarTerm, ValueId};
use crate::{IntegerSign, IntegerType, IntegerValue, ScalarType};

fn identifier(index: u64) -> ValueId {
    ValueId::new(index).unwrap()
}

#[test]
fn visits_scalar_and_mathematical_values_in_occurrence_order() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let proposition = Proposition::Implication {
        premise: Box::new(Proposition::Equal(
            ScalarTerm::BooleanNot {
                operand: Box::new(ScalarTerm::value(identifier(1), ScalarType::Boolean)),
            },
            ScalarTerm::value(identifier(2), ScalarType::Boolean),
        )),
        conclusion: Box::new(Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::MathValue {
                    source_type: integer_type,
                    value: identifier(3),
                }),
                Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(7))),
            ),
            IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: identifier(1),
            },
        )),
    };
    let mut values = Vec::new();
    assert!(proposition.visit_value_ids(|value| values.push(value)));
    assert_eq!(
        values,
        [identifier(1), identifier(2), identifier(3), identifier(1)]
    );
    let mut bounded = Vec::new();
    let mut remaining = 10;
    assert_eq!(
        proposition.visit_value_ids_bounded(&mut remaining, |value| bounded.push(value)),
        Some(true)
    );
    assert_eq!(bounded, values);
    assert_eq!(remaining, 0);
    let mut queried = Vec::new();
    assert!(proposition.any_value_id(|value| {
        queried.push(value);
        value == identifier(2)
    }));
    assert_eq!(queried, [identifier(1), identifier(2)]);
}

#[test]
fn bounded_visits_charge_literals_operators_and_repeated_values() {
    let proposition = Proposition::Equal(
        ScalarTerm::BooleanNot {
            operand: Box::new(ScalarTerm::BooleanEqual {
                left: Box::new(ScalarTerm::value(identifier(1), ScalarType::Boolean)),
                right: Box::new(ScalarTerm::value(identifier(1), ScalarType::Boolean)),
            }),
        },
        ScalarTerm::Boolean(false),
    );
    let mut values = Vec::new();
    let mut remaining = 5;
    assert_eq!(
        proposition.visit_value_ids_bounded(&mut remaining, |value| values.push(value)),
        None
    );
    assert_eq!(values, [identifier(1), identifier(1)]);
    assert_eq!(remaining, 0);
    remaining = 6;
    assert_eq!(
        proposition.visit_value_ids_bounded(&mut remaining, |_| {}),
        Some(true)
    );
    assert_eq!(remaining, 0);
    assert_eq!(
        Proposition::Truth.visit_value_ids_bounded(&mut remaining, |_| {}),
        None
    );
}

#[test]
fn bounded_visits_distinguish_opaque_identity_from_exhaustion() {
    let opaque = Proposition::Atom(crate::PropositionId::new(1).unwrap());
    assert_eq!(opaque.visit_value_ids_bounded(&mut 1, |_| {}), Some(false));
    assert_eq!(opaque.visit_value_ids_bounded(&mut 0, |_| {}), None);
}

#[test]
fn wide_propositions_stop_at_the_budget_and_keep_occurrence_order() {
    for constructor in [Proposition::Conjunction, Proposition::Disjunction] {
        let proposition = constructor(
            (1..=65_536)
                .map(|ordinal| {
                    Proposition::Equal(
                        ScalarTerm::value(identifier(ordinal), ScalarType::Boolean),
                        ScalarTerm::Boolean(true),
                    )
                })
                .collect(),
        );
        for (budget, expected) in [
            (1, vec![]),
            (4, vec![identifier(1)]),
            (7, vec![identifier(1), identifier(2)]),
        ] {
            let mut remaining = budget;
            let mut values = Vec::new();
            assert_eq!(
                proposition.visit_value_ids_bounded(&mut remaining, |value| values.push(value)),
                None
            );
            assert_eq!(remaining, 0);
            assert_eq!(values, expected);
        }
        let mut queried = Vec::new();
        assert!(proposition.any_value_id(|value| {
            queried.push(value);
            value == identifier(2)
        }));
        assert_eq!(queried, [identifier(1), identifier(2)]);
    }
}

#[test]
fn proposition_list_frames_do_not_change_exact_node_budgets() {
    for constructor in [Proposition::Conjunction, Proposition::Disjunction] {
        for parts in [vec![], vec![Proposition::Truth, Proposition::Falsehood]] {
            let mut remaining = 1 + parts.len();
            assert_eq!(
                constructor(parts).visit_value_ids_bounded(&mut remaining, |_| {}),
                Some(true)
            );
            assert_eq!(remaining, 0);
        }
    }
}
