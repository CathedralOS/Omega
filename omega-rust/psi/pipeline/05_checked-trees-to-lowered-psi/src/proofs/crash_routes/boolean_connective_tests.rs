//! Boolean connective lowering tests.

use super::{
    CheckedBooleanExpression, LoweringError, ScalarType, ValueDeclaration,
    checked_boolean_proposition, checked_boolean_scalar_term, value_id,
};
use crate::proofs::{
    CheckedIntegerComparisonKind, CheckedScalarExpression, PrimitiveType, Proposition, ScalarTerm,
    integer_scalar_type,
};

fn compound_equality() -> CheckedBooleanExpression {
    CheckedBooleanExpression::Equal {
        left: Box::new(CheckedBooleanExpression::And {
            left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
            right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
        }),
        right: Box::new(CheckedBooleanExpression::Or {
            left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
            right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
        }),
    }
}

#[test]
fn compound_expansion_shares_its_budget_across_branches_and_conjuncts() {
    let values = [7, 19].map(|identity| ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(identity),
        scalar_type: ScalarType::Boolean,
    });
    let predicate = compound_equality();
    assert!(checked_boolean_proposition(&predicate, &values).is_ok());
    let mut nested = predicate.clone();
    for _ in 0..7 {
        nested = CheckedBooleanExpression::Equal {
            left: Box::new(nested.clone()),
            right: Box::new(nested),
        };
    }
    let mut wide = predicate.clone();
    for _ in 0..8 {
        wide = CheckedBooleanExpression::And {
            left: Box::new(wide.clone()),
            right: Box::new(wide),
        };
    }
    for expression in [nested, wide] {
        assert!(matches!(
            checked_boolean_proposition(&expression, &values),
            Err(LoweringError::Unsupported(
                "crash Boolean expansion exceeds its lowering budget"
            ))
        ));
    }
    let mut deep = predicate;
    for _ in 0..64 {
        deep = CheckedBooleanExpression::Not(Box::new(deep));
    }
    assert!(matches!(
        checked_boolean_proposition(&deep, &values),
        Err(LoweringError::Unsupported(
            "crash Boolean expansion exceeds its depth limit"
        ))
    ));
}

#[test]
fn compound_equality_cannot_hide_unnormalized_constants_or_foreign_leaves() {
    let values = [7, 19].map(|identity| ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(identity),
        scalar_type: ScalarType::Boolean,
    });
    for invalid in [
        CheckedBooleanExpression::Constant(false),
        CheckedBooleanExpression::Parameter { position: 2 },
    ] {
        let predicate = CheckedBooleanExpression::Equal {
            left: Box::new(CheckedBooleanExpression::And {
                left: Box::new(CheckedBooleanExpression::Parameter { position: 0 }),
                right: Box::new(invalid),
            }),
            right: Box::new(CheckedBooleanExpression::Parameter { position: 1 }),
        };
        for expression in [
            predicate.clone(),
            CheckedBooleanExpression::Not(Box::new(predicate)),
        ] {
            assert!(checked_boolean_proposition(&expression, &values).is_err());
        }
    }
}

#[test]
fn scalar_atoms_keep_their_existing_crash_predicate_encoding() {
    let values = [ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(7),
        scalar_type: integer_scalar_type(PrimitiveType::U64).unwrap(),
    }];
    let parameter = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U64,
    };
    for kind in [
        CheckedIntegerComparisonKind::Equal,
        CheckedIntegerComparisonKind::LessThan,
        CheckedIntegerComparisonKind::LessOrEqual,
    ] {
        let comparison = CheckedBooleanExpression::IntegerComparison {
            kind,
            left: Box::new(parameter.clone()),
            right: Box::new(parameter.clone()),
        };
        for expression in [
            comparison.clone(),
            CheckedBooleanExpression::Not(Box::new(comparison)),
        ] {
            let mut terms = [
                checked_boolean_scalar_term(&expression, &values, &[]).unwrap(),
                ScalarTerm::boolean(true),
            ];
            terms.sort();
            let expected = Proposition::Equal(terms[0].clone(), terms[1].clone());
            let actual = checked_boolean_proposition(&expression, &values).unwrap();
            assert_eq!(
                terminal_codec::canonical_proposition_order_key(&actual).unwrap(),
                terminal_codec::canonical_proposition_order_key(&expected).unwrap()
            );
        }
    }
}

#[test]
fn connective_constant_children_still_require_prior_normalization() {
    let values = [ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(7),
        scalar_type: ScalarType::Boolean,
    }];
    for literal in [false, true] {
        for conjunction in [false, true] {
            let left = Box::new(CheckedBooleanExpression::Parameter { position: 0 });
            let right = Box::new(CheckedBooleanExpression::Constant(literal));
            let expression = if conjunction {
                CheckedBooleanExpression::And { left, right }
            } else {
                CheckedBooleanExpression::Or { left, right }
            };
            for expression in [
                expression.clone(),
                CheckedBooleanExpression::Not(Box::new(expression)),
            ] {
                assert!(matches!(
                    checked_boolean_proposition(&expression, &values),
                    Err(LoweringError::Unsupported(
                        "constant crash predicates must normalize before terminal lowering"
                    ))
                ));
            }
        }
    }
}

#[test]
fn negated_connectives_lower_to_logical_propositions_without_scalar_operations() {
    let values = [7, 19].map(|identity| ValueDeclaration {
        qualifications: Default::default(),
        id: value_id(identity),
        scalar_type: ScalarType::Boolean,
    });
    for conjunction in [false, true] {
        let left = CheckedBooleanExpression::Parameter { position: 0 };
        let right = CheckedBooleanExpression::Parameter { position: 1 };
        let expression = if conjunction {
            CheckedBooleanExpression::And {
                left: Box::new(left.clone()),
                right: Box::new(right.clone()),
            }
        } else {
            CheckedBooleanExpression::Or {
                left: Box::new(left.clone()),
                right: Box::new(right.clone()),
            }
        };
        let negated = CheckedBooleanExpression::Not(Box::new(expression.clone()));
        let expected = if conjunction {
            CheckedBooleanExpression::Or {
                left: Box::new(CheckedBooleanExpression::Not(Box::new(left))),
                right: Box::new(CheckedBooleanExpression::Not(Box::new(right))),
            }
        } else {
            CheckedBooleanExpression::And {
                left: Box::new(CheckedBooleanExpression::Not(Box::new(left))),
                right: Box::new(CheckedBooleanExpression::Not(Box::new(right))),
            }
        };
        let expected = checked_boolean_proposition(&expected, &values).unwrap();
        assert_eq!(
            checked_boolean_proposition(&negated, &values).unwrap(),
            expected
        );
        assert!(checked_boolean_scalar_term(&negated, &values, &[]).is_err());
        for wrapped in [
            CheckedBooleanExpression::Equal {
                left: Box::new(negated),
                right: Box::new(CheckedBooleanExpression::Constant(true)),
            },
            CheckedBooleanExpression::Equal {
                left: Box::new(CheckedBooleanExpression::Constant(false)),
                right: Box::new(expression),
            },
        ] {
            assert_eq!(
                checked_boolean_proposition(&wrapped, &values).unwrap(),
                expected
            );
        }
    }
}
