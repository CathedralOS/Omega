use super::aliases::{PlaceKey, canonical_place_key};
use omega_control_flow::TransitionFlow;
use omega_typed_program::expression::{BinaryOperator, Expression};
use omega_typed_program::statement::TransitionGuard;

pub(in crate::state_schedule) fn resolve_static_value(
    expression: &Expression,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &[(PlaceKey, String)],
) -> Option<String> {
    match expression {
        Expression::Mutable(inner_expression) => {
            resolve_static_value(inner_expression, aliases, values)
        }
        Expression::Name(_) | Expression::Indexed(_) => {
            let key = canonical_place_key(expression, aliases)?;
            values
                .iter()
                .find(|(target, _)| target == &key)
                .map(|(_, value)| value.clone())
                .or_else(|| static_symbol_name(expression))
        }
        Expression::Boolean(value) => Some(value.to_string()),
        Expression::Integer(value) => Some(value.to_string()),
        Expression::String(value) => Some(value.clone()),
        _ => None,
    }
}

pub(in crate::state_schedule) fn select_transition<'plan>(
    transitions: &'plan [TransitionFlow],
    values: &[(PlaceKey, String)],
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<Result<&'plan TransitionFlow, ()>> {
    for transition in transitions {
        match guard_matches(&transition.guard, aliases, values) {
            Some(true) => return Some(Ok(transition)),
            Some(false) => continue,
            None => return Some(Err(())),
        }
    }

    None
}

fn guard_matches(
    guard: &TransitionGuard,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &[(PlaceKey, String)],
) -> Option<bool> {
    match guard {
        TransitionGuard::Always => Some(true),
        TransitionGuard::When(expression) => evaluate_boolean(expression, aliases, values),
    }
}

fn evaluate_boolean(
    expression: &Expression,
    aliases: &[(PlaceKey, PlaceKey)],
    values: &[(PlaceKey, String)],
) -> Option<bool> {
    let Expression::Binary(binary) = expression else {
        return None;
    };

    match binary.operator {
        BinaryOperator::Equal => Some(
            resolve_static_value(&binary.left, aliases, values)?
                == resolve_static_value(&binary.right, aliases, values)?,
        ),
        BinaryOperator::NotEqual => Some(
            resolve_static_value(&binary.left, aliases, values)?
                != resolve_static_value(&binary.right, aliases, values)?,
        ),
        _ => None,
    }
}

fn static_symbol_name(expression: &Expression) -> Option<String> {
    let Expression::Name(path) = expression else {
        return None;
    };

    if path.symbol().is_valid() {
        Some(format!(
            "symbol:{}:{}",
            path.symbol().arena_index(),
            path.symbol().generation()
        ))
    } else {
        None
    }
}
