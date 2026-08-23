use super::StaticValue;
use super::aliases::{PlaceKey, canonical_place_key};
use omega_control_flow::TransitionFlow;
use psi_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};

pub(crate) fn resolve_static_value(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &[(PlaceKey, StaticValue)],
) -> Option<StaticValue> {
    match table.expression(expression) {
        ExpressionNode::Borrow(inner_expression) => {
            resolve_static_value(table, inner_expression.target, aliases, values)
        }
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) => {
            let key = canonical_place_key(table, expression, aliases)?;
            values
                .iter()
                .find(|(target, _)| target == &key)
                .map(|(_, value)| value.clone())
                .or_else(|| static_symbol_value(table, expression))
        }
        ExpressionNode::Boolean(value) => Some(StaticValue::Boolean(*value)),
        ExpressionNode::Integer(value) => value.value_i64().map(StaticValue::Integer),
        ExpressionNode::String(value) => Some(StaticValue::String(value.clone())),
        _ => None,
    }
}

pub(crate) fn select_transition<'plan>(
    table: &ExpressionTable,
    transitions: &'plan [TransitionFlow],
    values: &[(PlaceKey, StaticValue)],
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<Result<&'plan TransitionFlow, ()>> {
    for transition in transitions {
        match guard_matches(table, transition, aliases, values) {
            Some(true) => return Some(Ok(transition)),
            Some(false) => continue,
            None => return Some(Err(())),
        }
    }

    None
}

fn guard_matches(
    table: &ExpressionTable,
    transition: &TransitionFlow,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &[(PlaceKey, StaticValue)],
) -> Option<bool> {
    if transition.expressions.guard.is_valid() {
        evaluate_boolean(table, transition.expressions.guard, aliases, values)
    } else {
        Some(true)
    }
}

fn evaluate_boolean(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &[(PlaceKey, StaticValue)],
) -> Option<bool> {
    let ExpressionNode::Binary(binary) = table.expression(expression) else {
        return None;
    };

    match binary.operator {
        BinaryOperator::Equal => Some(
            resolve_static_value(table, binary.left, aliases, values)?
                == resolve_static_value(table, binary.right, aliases, values)?,
        ),
        BinaryOperator::NotEqual => Some(
            resolve_static_value(table, binary.left, aliases, values)?
                != resolve_static_value(table, binary.right, aliases, values)?,
        ),
        _ => None,
    }
}

fn static_symbol_value(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<StaticValue> {
    let ExpressionNode::Name(path) = table.expression(expression) else {
        return None;
    };

    path.symbol
        .is_valid()
        .then_some(StaticValue::Symbol(path.symbol))
}
