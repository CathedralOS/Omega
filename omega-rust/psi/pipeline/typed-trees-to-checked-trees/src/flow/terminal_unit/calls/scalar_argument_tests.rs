use super::*;

#[test]
fn checked_call_scalar_arguments_require_unique_bound_rows() {
    let state = SymbolHandle::from_arena_index(1);
    let coordinate = CheckedUnitCallCoordinate {
        statement_index: 2,
        call_ordinal: 3,
    };
    let parameters = [CheckedStructuralScalarParameterPlan {
        source_position: 4,
        primitive_type: PrimitiveType::Bool,
    }];
    for boundary in [false, true] {
        let role = if boundary {
            CheckedScalarExpressionRole::BoundaryCallArgument {
                call_ordinal: 3,
                argument_ordinal: 0,
            }
        } else {
            CheckedScalarExpressionRole::UnitCallArgument {
                call_ordinal: 3,
                argument_ordinal: 0,
            }
        };
        for mutation in 0..5 {
            let mut facts = CheckFacts::default();
            let plans = &mut facts.values.scalar_expressions;
            let binding = checked_trees::CheckedScalarExpressionBindings {
                state,
                statement_ordinal: 2,
                role,
                expression: typed_trees::expression::ExpressionHandle::from_arena_index(5),
                ..Default::default()
            };
            if mutation != 1 {
                plans.source_bindings.append(binding.clone());
            }
            if mutation == 2 {
                plans.source_bindings.append(binding);
            }
            plans
                .expressions
                .push(checked_trees::CheckedLocatedScalarExpression {
                    state,
                    statement_ordinal: 2,
                    role,
                    expression: CheckedScalarExpression::Boolean(Box::new(
                        CheckedBooleanExpression::Constant(true),
                    )),
                });
            if mutation == 3 {
                plans.expressions.push(plans.expressions[0].clone());
            }
            if mutation == 4 {
                plans.expressions[0].role = CheckedScalarExpressionRole::Return;
            }
            assert_eq!(
                checked_call_scalar_arguments(&facts, state, coordinate, &parameters, boundary)
                    .is_some(),
                mutation == 0,
                "boundary={boundary}, mutation={mutation}"
            );
        }
    }
}
