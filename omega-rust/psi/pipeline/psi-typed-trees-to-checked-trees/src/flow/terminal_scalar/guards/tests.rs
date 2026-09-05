use super::*;

fn negate(expression: CheckedBooleanExpression) -> CheckedBooleanExpression {
    CheckedBooleanExpression::Not(Box::new(expression))
}

fn plans(
    left: CheckedBooleanExpression,
    right: CheckedBooleanExpression,
) -> CheckedScalarExpressionPlans {
    let state = psi_symbols::SymbolHandle::from_arena_index(1);
    CheckedScalarExpressionPlans {
        expressions: [left, right]
            .into_iter()
            .enumerate()
            .map(
                |(index, expression)| psi_checked_trees::CheckedLocatedScalarExpression {
                    state,
                    statement_ordinal: u32::try_from(index).unwrap(),
                    role: CheckedScalarExpressionRole::Guard,
                    expression: CheckedScalarExpression::Boolean(Box::new(expression)),
                },
            )
            .collect(),
        ..Default::default()
    }
}

#[test]
fn complementary_guards_preserve_exact_operands_and_boolean_polarity() {
    let parameter = CheckedBooleanExpression::Parameter { position: 0 };
    let storage = CheckedBooleanExpression::StorageRead {
        symbol: psi_symbols::SymbolHandle::from_parts(2, 1),
    };
    let equal_false = |expression| CheckedBooleanExpression::Equal {
        left: Box::new(expression),
        right: Box::new(CheckedBooleanExpression::Constant(false)),
    };
    for (left, right, accepted) in [
        (parameter.clone(), negate(parameter.clone()), true),
        (equal_false(parameter.clone()), parameter.clone(), true),
        (
            negate(equal_false(negate(parameter.clone()))),
            parameter.clone(),
            true,
        ),
        (parameter.clone(), parameter.clone(), false),
        (
            parameter.clone(),
            negate(CheckedBooleanExpression::Parameter { position: 1 }),
            false,
        ),
        (storage.clone(), negate(storage.clone()), true),
        (
            storage.clone(),
            negate(CheckedBooleanExpression::StorageRead {
                symbol: psi_symbols::SymbolHandle::from_parts(2, 2),
            }),
            false,
        ),
        (storage, negate(parameter), false),
    ] {
        let plans = plans(left, right);
        assert_eq!(
            complementary(&plans, psi_symbols::SymbolHandle::from_arena_index(1), 0),
            accepted,
            "{plans:?}"
        );
    }
}

#[test]
fn complementary_guards_require_one_selected_row_per_exact_coordinate() {
    let parameter = CheckedBooleanExpression::Parameter { position: 0 };
    let original = plans(parameter.clone(), negate(parameter));
    let state = psi_symbols::SymbolHandle::from_arena_index(1);
    assert!(complementary(&original, state, 0));
    for mutation in 0..5 {
        let mut changed = original.clone();
        match mutation {
            0 => {
                changed.expressions.pop();
            }
            1 => {
                changed.expressions.remove(0);
            }
            2 => changed.expressions.push(changed.expressions[0].clone()),
            3 => changed.expressions.push(changed.expressions[1].clone()),
            _ => changed.expressions[1].state = psi_symbols::SymbolHandle::from_parts(1, 2),
        }
        assert!(!complementary(&changed, state, 0), "mutation {mutation}");
    }
    assert!(!complementary(&original, state, u32::MAX));
}
