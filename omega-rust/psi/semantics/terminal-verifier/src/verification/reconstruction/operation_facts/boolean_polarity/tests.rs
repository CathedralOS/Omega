use super::*;
use semantic_vocabulary::{IntegerSign, IntegerType, OperationId};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};
use terminal_semantics::{OperationSemanticError, goal_free_scalar_leaf_semantics};

fn value(index: u64) -> ValueId {
    ValueId::new(index).unwrap()
}

fn term(index: u64, scalar_type: ScalarType) -> ScalarTerm {
    ScalarTerm::value(value(index), scalar_type)
}

fn operation(kind: OperationKind) -> Operation {
    Operation {
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: value(3),
            scalar_type: ScalarType::Boolean,
        }),
        kind,
    }
}

#[test]
fn private_crash_projection_keeps_equation_without_call_proof_auxiliaries() {
    use super::super::{OperationFactPurpose, append_scalar_facts};

    let types = BTreeMap::from([
        (value(1), ScalarType::Boolean),
        (value(3), ScalarType::Boolean),
    ]);
    let observed = goal_free_scalar_leaf_semantics(
        &operation(OperationKind::BooleanNot { operand: value(1) }),
        &types,
    )
    .unwrap()
    .unwrap();
    let retained = Proposition::Implication {
        premise: Box::new(Proposition::Truth),
        conclusion: Box::new(Proposition::Equal(
            term(1, ScalarType::Boolean),
            ScalarTerm::boolean(false),
        )),
    };
    let mut crash_facts = vec![retained.clone()];
    append_scalar_facts(
        &observed,
        &types,
        OperationFactPurpose::PrivateCrashPredicates,
        &mut crash_facts,
    )
    .unwrap();
    assert_eq!(
        crash_facts,
        vec![retained.clone(), observed.result_equation().clone()]
    );

    let mut proof_facts = vec![retained];
    append_scalar_facts(
        &observed,
        &types,
        OperationFactPurpose::ProofObligations,
        &mut proof_facts,
    )
    .unwrap();
    assert_eq!(proof_facts[..2], crash_facts);
    assert_eq!(proof_facts[2..], implications(&observed, &types).unwrap());
    assert_eq!(proof_facts.len(), 4);
}

#[test]
fn exact_negation_polarities_have_no_external_premises() {
    let types = BTreeMap::from([
        (value(1), ScalarType::Boolean),
        (value(3), ScalarType::Boolean),
    ]);
    let observed = goal_free_scalar_leaf_semantics(
        &operation(OperationKind::BooleanNot { operand: value(1) }),
        &types,
    )
    .unwrap()
    .unwrap();
    let actual = implications(&observed, &types).unwrap();
    for (index, positive) in [true, false].into_iter().enumerate() {
        assert_eq!(
            actual[index],
            Proposition::Implication {
                premise: Box::new(Proposition::Equal(
                    term(1, ScalarType::Boolean),
                    ScalarTerm::boolean(!positive)
                )),
                conclusion: Box::new(Proposition::Equal(
                    term(3, ScalarType::Boolean),
                    ScalarTerm::boolean(positive)
                )),
            }
        );
    }
    let mut missing_result = types.clone();
    missing_result.remove(&value(3));
    assert!(implications(&observed, &missing_result).is_err());
    let mut retyped_result = types.clone();
    retyped_result.insert(
        value(3),
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    );
    assert!(implications(&observed, &retyped_result).is_err());
    assert!(matches!(
        goal_free_scalar_leaf_semantics(
            &operation(OperationKind::BooleanNot { operand: value(2) }),
            &types
        ),
        Err(OperationSemanticError::UnknownValue(_))
    ));
}

#[test]
fn integer_comparison_polarities_preserve_exact_strictness_and_operands() {
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap());
    let types = BTreeMap::from([
        (value(1), integer),
        (value(2), integer),
        (value(3), ScalarType::Boolean),
    ]);
    for (kind, positive, negative) in [
        (
            OperationKind::IntegerLessThan {
                left: value(1),
                right: value(2),
            },
            Proposition::LessThan(term(1, integer), term(2, integer)),
            Proposition::LessOrEqual(term(2, integer), term(1, integer)),
        ),
        (
            OperationKind::IntegerLessOrEqual {
                left: value(1),
                right: value(2),
            },
            Proposition::LessOrEqual(term(1, integer), term(2, integer)),
            Proposition::LessThan(term(2, integer), term(1, integer)),
        ),
    ] {
        let observed = goal_free_scalar_leaf_semantics(&operation(kind), &types)
            .unwrap()
            .unwrap();
        let actual = implications(&observed, &types).unwrap();
        for (index, premise) in [positive, negative].into_iter().enumerate() {
            assert_eq!(
                actual[index],
                Proposition::Implication {
                    premise: Box::new(premise),
                    conclusion: Box::new(Proposition::Equal(
                        term(3, ScalarType::Boolean),
                        ScalarTerm::boolean(index == 0)
                    )),
                }
            );
        }
    }
}

#[test]
fn boolean_equality_retains_both_operand_polarities() {
    let types = (1..=3)
        .map(|index| (value(index), ScalarType::Boolean))
        .collect();
    let observed = goal_free_scalar_leaf_semantics(
        &operation(OperationKind::BooleanEqual {
            left: value(1),
            right: value(2),
        }),
        &types,
    )
    .unwrap()
    .unwrap();
    let actual = implications(&observed, &types).unwrap();
    assert!(
        matches!(&actual[0], Proposition::Implication { premise, .. }
        if **premise == Proposition::Equal(term(1, ScalarType::Boolean), term(2, ScalarType::Boolean)))
    );
    assert!(
        matches!(&actual[1], Proposition::Implication { premise, .. }
        if matches!(premise.as_ref(), Proposition::Disjunction(parts) if parts.len() == 2))
    );
}
