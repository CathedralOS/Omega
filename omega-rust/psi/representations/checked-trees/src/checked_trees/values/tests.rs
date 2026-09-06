use super::*;

fn bound_plans() -> CheckedScalarExpressionPlans {
    let mut plans = CheckedScalarExpressionPlans::default();
    let state = SymbolHandle::from_arena_index(1);
    plans
        .source_bindings
        .append(CheckedScalarExpressionBindings {
            state,
            statement_ordinal: 2,
            role: CheckedScalarExpressionRole::Guard,
            expression: ExpressionHandle::from_arena_index(3),
            ..Default::default()
        });
    plans.expressions.push(CheckedLocatedScalarExpression {
        state,
        statement_ordinal: 2,
        role: CheckedScalarExpressionRole::Guard,
        expression: CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::Constant(
            true,
        ))),
    });
    plans
}

#[test]
fn bound_scalar_expression_lookup_requires_both_unique_coordinate_rows() {
    let state = SymbolHandle::from_arena_index(1);
    let role = CheckedScalarExpressionRole::Guard;
    for mutation in 0..8 {
        let mut plans = bound_plans();
        let binding = plans.source_bindings.iter().next().unwrap().0;
        match mutation {
            0 => plans.source_bindings = Arena::default(),
            1 => plans.expressions.clear(),
            2 | 3 => {
                let mut duplicate = plans.source_bindings.get(binding).clone();
                if mutation == 3 {
                    duplicate.expression = ExpressionHandle::from_arena_index(4);
                    duplicate.destination = SymbolHandle::from_arena_index(5);
                }
                plans.source_bindings.append(duplicate);
            }
            4 => plans.expressions.push(plans.expressions[0].clone()),
            5 => plans.source_bindings.get_mut(binding).role = CheckedScalarExpressionRole::Return,
            6 => plans.expressions[0].role = CheckedScalarExpressionRole::Return,
            _ => plans.source_bindings.get_mut(binding).state = SymbolHandle::from_parts(1, 2),
        }
        assert!(
            plans.bound_expression_at(state, 2, role).is_none(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn bound_scalar_expression_lookup_leaves_expected_identity_checks_to_consumer() {
    let plans = bound_plans();
    let state = SymbolHandle::from_arena_index(1);
    let role = CheckedScalarExpressionRole::Guard;
    let (binding, expression) = plans.bound_expression_at(state, 2, role).unwrap();
    assert_eq!(binding.expression, ExpressionHandle::from_arena_index(3));
    assert_eq!(expression, &plans.expressions[0].expression);
    assert!(plans.bound_expression_at(state, 3, role).is_none());
    assert!(
        plans
            .bound_expression_at(state, 2, CheckedScalarExpressionRole::Return)
            .is_none()
    );
}
