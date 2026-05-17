use crate::EmissionPlanningInput;
use omega_checked_trees::expression::{BinaryOperator, Expression};
use omega_core::arena::Arena;
use omega_state_graph::RuntimeTransitionTarget;
use omega_state_guards::{StateGuardLowering, lower_guard_conjunction};

use super::semantic_scope::proof_scope_suffix;
use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_guard_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, guard) in input.state_guards.guards.iter() {
        if matches!(
            guard.lowering,
            StateGuardLowering::NoOp
                | StateGuardLowering::CompareStaticValue
                | StateGuardLowering::CompareRuntimeValue
        ) {
            continue;
        }

        let clauses = lower_guard_conjunction(
            input.state_guards,
            input.layouts,
            input.runtime_storage,
            input.entry_key.machine,
            guard.source,
            guard.source.machine,
            guard.source_dispatch_index,
            guard.statement_order,
        );
        if !clauses.is_empty() {
            continue;
        }

        if guard.has_expression
            && guard_expression_can_emit(
                input,
                guard.source,
                guard.source_dispatch_index,
                guard.statement_index,
                &input.state_guards.expressions.to_tree(guard.expression),
            )
        {
            continue;
        }

        let machine_name = input
            .control_flow
            .machine_by_symbol(guard.source.machine)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>");
        let state_name = input
            .control_flow
            .state_by_key(guard.source)
            .map(|state| state.name.as_str())
            .unwrap_or("<unknown>");

        blockers.insert(blocker(
            "state guards",
            &format!(
                "#{} {}.{} edge {} -> #{} {} {:?}/{:?} `{}`{} needs runtime guard lowering",
                guard.source_dispatch_index,
                machine_name,
                state_name,
                guard.statement_order,
                guard.target_dispatch_index,
                runtime_transition_target_name(input, &guard.target),
                guard.kind,
                guard.lowering,
                input
                    .state_guards
                    .expressions
                    .display_name(guard.expression),
                proof_scope_suffix(input, guard.source)
            ),
        ));
    }
}

fn guard_expression_can_emit(
    input: &EmissionPlanningInput<'_>,
    source_key: omega_control_flow::StateKey,
    source_dispatch_index: u32,
    statement_index: usize,
    expression: &Expression,
) -> bool {
    if let Some(normalized) = normalized_boolean_wrapped_expression(expression) {
        return guard_expression_can_emit(
            input,
            source_key,
            source_dispatch_index,
            statement_index,
            &normalized,
        );
    }

    if let Some(expression) = boolean_condition_expression(expression) {
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

    matches!(
        binary.operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
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

fn boolean_condition_expression(expression: &Expression) -> Option<&Expression> {
    let Expression::Binary(binary) = expression else {
        return None;
    };

    match (&binary.left, &binary.right, binary.operator) {
        (inner, Expression::Boolean(_), BinaryOperator::Equal | BinaryOperator::NotEqual)
        | (Expression::Boolean(_), inner, BinaryOperator::Equal | BinaryOperator::NotEqual) => {
            (!matches!(inner, Expression::Binary(_))).then_some(inner)
        }
        _ => None,
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
                BinaryOperator::Add | BinaryOperator::Multiply | BinaryOperator::Subtract
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
        | Expression::Mutable(_)
        | Expression::Boolean(_)
        | Expression::Integer(_) => true,
        Expression::Call(_) => input
            .runtime_storage
            .transition_guard_result_slot(source_dispatch_index, source_key, statement_index)
            .is_some(),
        _ => false,
    }
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
        omega_checked_trees::expression::BinaryExpression {
            left: inner_binary.left.clone(),
            operator: inverted,
            right: inner_binary.right.clone(),
        },
    )))
}

fn runtime_transition_target_name(
    input: &EmissionPlanningInput<'_>,
    target: &RuntimeTransitionTarget,
) -> String {
    match target {
        RuntimeTransitionTarget::State { key } => input
            .control_flow
            .state_names_by_key(*key)
            .map(|(machine, state)| format!("{machine}.{state}"))
            .unwrap_or_else(|| "<unknown>.<unknown>".to_owned()),
        RuntimeTransitionTarget::Terminal => "terminal".to_owned(),
        RuntimeTransitionTarget::None => "none".to_owned(),
        RuntimeTransitionTarget::Unknown => "unknown".to_owned(),
    }
}
