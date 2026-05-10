use super::aliases::{PlaceKey, canonical_place_key};
use omega_control_flow::TransitionFlow;
use omega_typed_program::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_typed_program::statement::TransitionGuard;

pub(crate) fn resolve_static_value(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &[(PlaceKey, String)],
) -> Option<String> {
    match table.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            resolve_static_value(table, *inner_expression, aliases, values)
        }
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) => {
            let key = canonical_place_key(table, expression, aliases)?;
            values
                .iter()
                .find(|(target, _)| target == &key)
                .map(|(_, value)| value.clone())
                .or_else(|| static_symbol_name(table, expression))
        }
        ExpressionNode::Boolean(value) => Some(value.to_string()),
        ExpressionNode::Integer(value) => Some(value.to_string()),
        ExpressionNode::String(value) => Some(value.clone()),
        _ => None,
    }
}

pub(crate) fn select_transition<'plan>(
    table: &ExpressionTable,
    transitions: &'plan [TransitionFlow],
    values: &[(PlaceKey, String)],
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
    values: &[(PlaceKey, String)],
) -> Option<bool> {
    match transition.guard {
        TransitionGuard::Always => Some(true),
        TransitionGuard::When(_) => {
            evaluate_boolean(table, transition.expressions.guard?, aliases, values)
        }
    }
}

fn evaluate_boolean(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &[(PlaceKey, String)],
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

fn static_symbol_name(table: &ExpressionTable, expression: ExpressionHandle) -> Option<String> {
    let ExpressionNode::Name(path) = table.expression(expression) else {
        return None;
    };

    if path.symbol.is_valid() {
        Some(format!(
            "symbol:{}:{}",
            path.symbol.arena_index(),
            path.symbol.generation()
        ))
    } else {
        None
    }
}
