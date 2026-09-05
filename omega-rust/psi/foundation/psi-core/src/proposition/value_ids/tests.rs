use super::*;
use crate::{IntegerSign, IntegerType, IntegerValue, PlaceId, PropositionId, ScalarType};

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
    let mut queried = Vec::new();
    assert!(proposition.any_value_id(|value| {
        queried.push(value);
        value == identifier(2)
    }));
    assert_eq!(queried, [identifier(1), identifier(2)]);
}

#[test]
fn structural_and_opaque_dependencies_are_reported_without_hiding_values() {
    let proposition = Proposition::Conjunction(vec![
        Proposition::Atom(PropositionId::new(1).unwrap()),
        Proposition::Equal(
            ScalarTerm::BooleanField {
                root: PlaceId::new(1).unwrap(),
                path: Vec::new(),
            },
            ScalarTerm::value(identifier(1), ScalarType::Boolean),
        ),
    ]);
    let mut values = Vec::new();
    assert!(!proposition.visit_value_ids(|value| values.push(value)));
    assert_eq!(values, [identifier(1)]);
    assert!(!Proposition::Atom(PropositionId::new(1).unwrap()).any_value_id(|_| true));
    assert!(
        Proposition::Equal(ScalarTerm::Boolean(true), ScalarTerm::Boolean(true))
            .visit_value_ids(|_| panic!("literals have no value identity"))
    );
}
