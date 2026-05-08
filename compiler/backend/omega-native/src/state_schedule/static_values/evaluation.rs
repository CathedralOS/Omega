use super::aliases::canonical_place_name;
use crate::control_flow::TransitionFlow;
use omega_typed_program::expression::{BinaryOperator, Expression};
use omega_typed_program::statement::TransitionGuard;

pub(in crate::state_schedule) fn resolve_static_value(
    expression: &Expression,
    aliases: &[(String, String)],
    values: &[(String, String)],
) -> Option<String> {
    match expression {
        Expression::Mutable(inner_expression) => {
            resolve_static_value(inner_expression, aliases, values)
        }
        Expression::Name(_) | Expression::Indexed(_) => {
            let name = canonical_place_name(expression, aliases)?;
            values
                .iter()
                .find(|(target, _)| target == &name)
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
    values: &[(String, String)],
    aliases: &[(String, String)],
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
    aliases: &[(String, String)],
    values: &[(String, String)],
) -> Option<bool> {
    match guard {
        TransitionGuard::Always => Some(true),
        TransitionGuard::When(expression) => evaluate_boolean(expression, aliases, values),
    }
}

fn evaluate_boolean(
    expression: &Expression,
    aliases: &[(String, String)],
    values: &[(String, String)],
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

    if path
        .first()
        .and_then(|segment| segment.chars().next())
        .is_some_and(char::is_uppercase)
    {
        Some(expression.display_name())
    } else {
        None
    }
}
