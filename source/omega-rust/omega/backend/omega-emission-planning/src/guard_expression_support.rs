use crate::EmissionPlanningInput;
use psi_checked_trees::expression::{BinaryOperator, Expression};
use psi_checked_trees::statement::TransitionGuard;

pub(super) fn transition_guard_can_emit(
    input: &EmissionPlanningInput<'_>,
    source_key: omega_control_flow::StateKey,
    source_dispatch_index: u32,
    statement_index: usize,
    guard: &TransitionGuard,
) -> bool {
    if let Some(normalized) = normalized_boolean_wrapped_guard(guard) {
        return transition_guard_can_emit(
            input,
            source_key,
            source_dispatch_index,
            statement_index,
            &normalized,
        );
    }

    let TransitionGuard::When(expression) = guard else {
        return false;
    };

    expression_guard_can_emit(
        input,
        source_key,
        source_dispatch_index,
        statement_index,
        expression,
    )
}

pub(super) fn expression_guard_can_emit(
    input: &EmissionPlanningInput<'_>,
    source_key: omega_control_flow::StateKey,
    source_dispatch_index: u32,
    statement_index: usize,
    expression: &Expression,
) -> bool {
    if let Some(normalized) = normalized_boolean_wrapped_expression(expression) {
        return expression_guard_can_emit(
            input,
            source_key,
            source_dispatch_index,
            statement_index,
            &normalized,
        );
    }

    if let Some((expression, _expected_true)) = boolean_condition_expression(expression)
        && !matches!(expression, Expression::Binary(_))
    {
        return runtime_value_expression_can_emit(
            input,
            source_key,
            source_dispatch_index,
            statement_index,
            expression,
        );
    }

    let Expression::Binary(binary) = expression else {
        return false;
    };

    match binary.operator {
        BinaryOperator::And => {
            expression_guard_can_emit(
                input,
                source_key,
                source_dispatch_index,
                statement_index,
                &binary.left,
            ) && expression_guard_can_emit(
                input,
                source_key,
                source_dispatch_index,
                statement_index,
                &binary.right,
            )
        }
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual => {
            runtime_value_expression_can_emit(
                input,
                source_key,
                source_dispatch_index,
                statement_index,
                &binary.left,
            ) && runtime_value_expression_can_emit(
                input,
                source_key,
                source_dispatch_index,
                statement_index,
                &binary.right,
            )
        }
        _ => false,
    }
}

fn runtime_value_expression_can_emit(
    input: &EmissionPlanningInput<'_>,
    source_key: omega_control_flow::StateKey,
    source_dispatch_index: u32,
    statement_index: usize,
    expression: &Expression,
) -> bool {
    match expression {
        Expression::Binary(binary) => {
            matches!(
                binary.operator,
                // Shifts are now threaded: `<<` is signedness-agnostic, and the
                // guard value-operand site adjusts `>>` to a logical shift for an
                // unsigned shifted value (so `-8 >> 1` stays arithmetic `sar`).
                // Bitwise `& | ^` carry no signedness and are likewise safe.
                BinaryOperator::Add
                    | BinaryOperator::And
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::Multiply
                    | BinaryOperator::Subtract
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
            ) && runtime_value_expression_can_emit(
                input,
                source_key,
                source_dispatch_index,
                statement_index,
                &binary.left,
            ) && runtime_value_expression_can_emit(
                input,
                source_key,
                source_dispatch_index,
                statement_index,
                &binary.right,
            )
        }
        Expression::Name(_)
        | Expression::Member(_)
        | Expression::Indexed(_)
        | Expression::Borrow(_)
        | Expression::Boolean(_)
        | Expression::Integer(_)
        | Expression::String(_) => true,
        // A numeric `as` cast subject (`self.big as u8 == 44`): emittable when its
        // source is -- the guard value-operand path wraps it in a Convert.
        Expression::Cast(cast) => runtime_value_expression_can_emit(
            input,
            source_key,
            source_dispatch_index,
            statement_index,
            &cast.value,
        ),
        Expression::Call(_) => input
            .runtime_storage
            .transition_guard_result_slot(source_dispatch_index, source_key, statement_index)
            .is_some(),
        _ => false,
    }
}

fn boolean_condition_expression(expression: &Expression) -> Option<(&Expression, bool)> {
    let Expression::Binary(binary) = expression else {
        return Some((expression, true));
    };

    match (&binary.left, &binary.right, binary.operator) {
        (inner, Expression::Boolean(_), BinaryOperator::Equal | BinaryOperator::NotEqual)
        | (Expression::Boolean(_), inner, BinaryOperator::Equal | BinaryOperator::NotEqual) => {
            (!matches!(inner, Expression::Binary(_))).then_some((
                inner,
                matches!(binary.operator, BinaryOperator::Equal)
                    == matches!(
                        (&binary.left, &binary.right),
                        (_, Expression::Boolean(true)) | (Expression::Boolean(true), _)
                    ),
            ))
        }
        _ => Some((expression, true)),
    }
}

fn normalized_boolean_wrapped_guard(guard: &TransitionGuard) -> Option<TransitionGuard> {
    let TransitionGuard::When(expression) = guard else {
        return None;
    };
    normalized_boolean_wrapped_expression(expression).map(TransitionGuard::When)
}

fn normalized_boolean_wrapped_expression(expression: &Expression) -> Option<Expression> {
    let Expression::Binary(binary) = expression else {
        return None;
    };
    let (inner, expected_true) = match (&binary.left, &binary.right) {
        (inner, Expression::Boolean(value)) => (inner, *value),
        (Expression::Boolean(value), inner) => (inner, *value),
        _ => return None,
    };
    let expected_true = match binary.operator {
        BinaryOperator::Equal => expected_true,
        BinaryOperator::NotEqual => !expected_true,
        _ => return None,
    };
    if expected_true {
        return Some(inner.clone());
    }
    let Expression::Binary(inner_binary) = inner else {
        return None;
    };
    let inverted = match inner_binary.operator {
        BinaryOperator::Equal => BinaryOperator::NotEqual,
        BinaryOperator::NotEqual => BinaryOperator::Equal,
        BinaryOperator::Greater => BinaryOperator::LessOrEqual,
        BinaryOperator::GreaterOrEqual => BinaryOperator::Less,
        BinaryOperator::Less => BinaryOperator::GreaterOrEqual,
        BinaryOperator::LessOrEqual => BinaryOperator::Greater,
        _ => return None,
    };
    Some(Expression::Binary(Box::new(
        psi_checked_trees::expression::BinaryExpression {
            left: inner_binary.left.clone(),
            operator: inverted,
            right: inner_binary.right.clone(),
        },
    )))
}
