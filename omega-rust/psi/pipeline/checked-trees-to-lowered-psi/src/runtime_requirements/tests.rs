use super::*;

fn boolean_parameter() -> ValueDeclaration {
    ValueDeclaration {
        id: value_id(1),
        scalar_type: ScalarType::Boolean,
    }
}

fn lower(expression: &CheckedBooleanExpression) -> Result<Proposition, LoweringError> {
    lower_structural_runtime_requirement(expression, &[boolean_parameter()], &[], &[])
}

#[test]
fn wrappers_never_admit_body_locals_or_mutable_storage() {
    for unsupported in [
        CheckedBooleanExpression::Local { position: 0 },
        CheckedBooleanExpression::StorageRead {
            symbol: symbols::SymbolHandle::from_arena_index(9),
        },
    ] {
        for expression in [
            unsupported.clone(),
            CheckedBooleanExpression::Not(Box::new(unsupported.clone())),
            CheckedBooleanExpression::Or {
                left: Box::new(CheckedBooleanExpression::Constant(true)),
                right: Box::new(unsupported.clone()),
            },
            CheckedBooleanExpression::Equal {
                left: Box::new(CheckedBooleanExpression::Constant(false)),
                right: Box::new(unsupported),
            },
        ] {
            assert!(lower(&expression).is_err());
        }
    }
}

#[test]
fn boolean_parameters_require_exact_namespace_and_actual_carriers() {
    let expression = CheckedBooleanExpression::Parameter { position: 0 };
    let proposition = lower(&expression).unwrap();
    let wrong_type = ValueDeclaration {
        id: value_id(2),
        scalar_type: integer_scalar_type(PrimitiveType::U8).unwrap(),
    };
    assert!(lower_structural_runtime_requirement(&expression, &[wrong_type], &[], &[]).is_err());
    assert!(lower(&CheckedBooleanExpression::Parameter { position: 1 }).is_err());
    for substitutions in [BTreeMap::new(), BTreeMap::from([(value_id(1), wrong_type)])] {
        assert!(
            substitute_runtime_requirement_scalar_values(&mut proposition.clone(), &substitutions)
                .is_err()
        );
    }
    let actual = ValueDeclaration {
        id: value_id(8),
        scalar_type: ScalarType::Boolean,
    };
    let mut rebound = proposition;
    substitute_runtime_requirement_scalar_values(
        &mut rebound,
        &BTreeMap::from([(value_id(1), actual)]),
    )
    .unwrap();
    assert!(matches!(rebound, Proposition::Equal(left, right)
        if left == ScalarTerm::value(actual.id, actual.scalar_type)
            && right == ScalarTerm::boolean(true)
            || right == ScalarTerm::value(actual.id, actual.scalar_type)
                && left == ScalarTerm::boolean(true)));
}

#[test]
fn logical_substitution_preserves_requirement_children_and_equality_order() {
    let first = ScalarTerm::value(value_id(1), ScalarType::Boolean);
    let second = ScalarTerm::value(value_id(2), ScalarType::Boolean);
    let mut proposition = Proposition::Conjunction(vec![
        Proposition::Equal(first.clone(), second.clone()),
        Proposition::Disjunction(vec![
            Proposition::Equal(first, ScalarTerm::boolean(false)),
            Proposition::Equal(second, ScalarTerm::boolean(true)),
        ]),
    ]);
    let first_actual = ValueDeclaration {
        id: value_id(9),
        scalar_type: ScalarType::Boolean,
    };
    let second_actual = ValueDeclaration {
        id: value_id(3),
        scalar_type: ScalarType::Boolean,
    };
    substitute_runtime_requirement_scalar_values(
        &mut proposition,
        &BTreeMap::from([(value_id(1), first_actual), (value_id(2), second_actual)]),
    )
    .unwrap();
    assert_eq!(
        proposition,
        Proposition::Conjunction(vec![
            Proposition::Equal(
                ScalarTerm::value(first_actual.id, ScalarType::Boolean),
                ScalarTerm::value(second_actual.id, ScalarType::Boolean)
            ),
            Proposition::Disjunction(vec![
                Proposition::Equal(
                    ScalarTerm::value(first_actual.id, ScalarType::Boolean),
                    ScalarTerm::boolean(false)
                ),
                Proposition::Equal(
                    ScalarTerm::value(second_actual.id, ScalarType::Boolean),
                    ScalarTerm::boolean(true)
                ),
            ]),
        ])
    );
}

#[test]
fn strict_integer_requirements_keep_original_relation_and_reject_new_arithmetic() {
    let scalar_type = integer_scalar_type(PrimitiveType::U16).unwrap();
    let formal = ValueDeclaration {
        id: value_id(1),
        scalar_type,
    };
    let parameter = CheckedScalarExpression::Parameter {
        position: 0,
        primitive_type: PrimitiveType::U16,
    };
    let literal = CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(3).with_landing(
            numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::U16,
                domain: ArithmeticDomain::Exact,
            },
        ),
    };
    let predicate = |left| CheckedBooleanExpression::IntegerComparison {
        kind: CheckedIntegerComparisonKind::LessThan,
        left: Box::new(left),
        right: Box::new(literal.clone()),
    };
    assert!(matches!(
        lower_structural_runtime_requirement(&predicate(parameter.clone()), &[formal], &[], &[])
            .unwrap(),
        Proposition::LessThan(_, _)
    ));
    for kind in [
        checked_trees::CheckedIntegerBinaryKind::ExactAdd,
        checked_trees::CheckedIntegerBinaryKind::ExactDivide,
    ] {
        let arithmetic = CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type: PrimitiveType::U16,
            left: Box::new(parameter.clone()),
            right: Box::new(literal.clone()),
        };
        assert!(
            lower_structural_runtime_requirement(&predicate(arithmetic), &[formal], &[], &[])
                .is_err()
        );
    }
    for primitive_type in [PrimitiveType::Addr, PrimitiveType::F32] {
        assert!(
            lower_structural_runtime_requirement(
                &predicate(CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type,
                }),
                &[formal],
                &[],
                &[]
            )
            .is_err()
        );
    }
}
