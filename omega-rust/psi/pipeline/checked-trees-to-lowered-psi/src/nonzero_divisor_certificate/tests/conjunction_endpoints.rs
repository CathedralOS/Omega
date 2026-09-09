//! Direct arithmetic endpoints retain their original conjunction citations.

use super::*;
use proof_admission::check_certificate;
use semantic_vocabulary::IntegerMathTerm;

fn fixture() -> (
    PropositionContext,
    Proposition,
    Proposition,
    Vec<Proposition>,
) {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Boolean),
    ])
    .unwrap();
    let literal =
        |number| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap();
    let parameter = value(1, integer_type);
    let bound = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(parameter.clone(), literal(9)),
        Proposition::LessOrEqual(literal(0), parameter),
    ]);
    let axioms = vec![Proposition::Equal(value(5, integer_type), literal(1))];
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(1).unwrap(),
            }),
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: ValueId::new(5).unwrap(),
            }),
        ),
        IntegerMathTerm::literal(IntegerValue::Unsigned(255)),
    );
    (context, goal, bound, axioms)
}

#[test]
fn exact_add_projects_direct_and_nested_conjunction_endpoints() {
    let (context, goal, bound, axioms) = fixture();
    for nested in [false, true] {
        let premise = if nested {
            Proposition::Conjunction(vec![
                Proposition::Truth,
                Proposition::Conjunction(vec![Proposition::Truth, bound.clone()]),
            ])
        } else {
            bound.clone()
        };
        for semantic in [false, true] {
            let (assumptions, semantic_axioms) = if semantic {
                (Vec::new(), vec![axioms[0].clone(), premise.clone()])
            } else {
                (vec![premise.clone()], axioms.clone())
            };
            let proof = prove_canonical_integer_proposition(
                &context,
                &goal,
                &assumptions,
                &semantic_axioms,
            )
            .expect("range endpoint remains available inside its conjunction");
            check_certificate(&context, &goal, &assumptions, &semantic_axioms, &proof)
                .expect("kernel reconstructs original conjunction citation positions");
            assert!(
                check_certificate(&context, &goal, &[], &axioms, &proof).is_err(),
                "endpoint proof cannot survive removal of its cited range"
            );
        }
    }
}

#[test]
fn exact_add_does_not_import_an_unselected_disjunction_endpoint() {
    let (context, goal, bound, axioms) = fixture();
    let assumptions = [Proposition::Disjunction(vec![bound, Proposition::Truth])];
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms).is_none(),
        "the unrestricted alternative permits overflowing input 255"
    );
}

#[test]
fn exact_add_does_not_import_an_undischarged_implication_endpoint() {
    let (context, goal, bound, axioms) = fixture();
    let assumptions = [Proposition::Implication {
        premise: Box::new(Proposition::Equal(
            ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Boolean),
            ScalarTerm::boolean(true),
        )),
        conclusion: Box::new(bound),
    }];
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms).is_none(),
        "an unknown Boolean premise cannot supply the array parameter bound"
    );
}
