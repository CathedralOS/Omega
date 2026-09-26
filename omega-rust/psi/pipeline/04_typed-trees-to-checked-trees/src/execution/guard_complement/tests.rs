use super::guard_pair;
use checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionPlans,
    CheckedScalarExpressionRole,
};

fn plans(
    first: CheckedBooleanExpression,
    second: CheckedBooleanExpression,
) -> CheckedScalarExpressionPlans {
    let state = symbols::SymbolHandle::from_arena_index(1);
    CheckedScalarExpressionPlans {
        expressions: [first, second]
            .into_iter()
            .enumerate()
            .map(
                |(index, expression)| checked_trees::CheckedLocatedScalarExpression {
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
fn guard_pairs_require_one_selected_row_per_exact_coordinate() {
    let parameter = CheckedBooleanExpression::Parameter { position: 0 };
    let original = plans(
        parameter.clone(),
        CheckedBooleanExpression::Not(Box::new(parameter)),
    );
    let state = symbols::SymbolHandle::from_arena_index(1);
    assert!(guard_pair(&original, state, 0).is_some());
    for mutation in 0..6 {
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
            4 => changed.expressions[1].state = symbols::SymbolHandle::from_parts(1, 2),
            _ => changed.expressions[1].role = CheckedScalarExpressionRole::Return,
        }
        assert!(
            guard_pair(&changed, state, 0).is_none(),
            "mutation {mutation}"
        );
    }
    assert!(guard_pair(&original, state, u32::MAX).is_none());
}
