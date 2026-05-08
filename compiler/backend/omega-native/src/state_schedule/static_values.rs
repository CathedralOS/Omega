use crate::control_flow::{OperationKind, StateFlow, TransitionFlow};
use crate::plan::NativePlan;
use omega_typed_program::expression::{BinaryOperator, Expression};
use omega_typed_program::statement::TransitionGuard;

pub(super) fn apply_static_operations(
    native_plan: &NativePlan,
    state: &StateFlow,
    aliases: &[(String, String)],
    values: &mut Vec<(String, String)>,
) {
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return;
    };

    for operation in operations {
        match &operation.kind {
            OperationKind::Assignment { target, value }
            | OperationKind::StaticAssignment { target, value } => {
                apply_static_assignment(target, value, aliases, values);
            }
            _ => {}
        };
    }
}

pub(super) fn guard_matches(
    guard: &TransitionGuard,
    aliases: &[(String, String)],
    values: &[(String, String)],
) -> Option<bool> {
    match guard {
        TransitionGuard::Always => Some(true),
        TransitionGuard::When(expression) => evaluate_boolean(expression, aliases, values),
    }
}

pub(super) fn resolve_static_value(
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

pub(super) fn argument_binding_place_name(
    expression: &Expression,
    aliases: &[(String, String)],
) -> Option<String> {
    match expression {
        Expression::Mutable(inner_expression) => {
            shallow_canonical_place_name(inner_expression, aliases)
        }
        _ => canonical_place_name(expression, aliases),
    }
}

pub(super) fn set_static_value(values: &mut Vec<(String, String)>, target: String, value: String) {
    if let Some((_, existing_value)) = values
        .iter_mut()
        .find(|(existing_target, _)| existing_target == &target)
    {
        *existing_value = value;
    } else {
        values.push((target, value));
    }
}

pub(super) fn select_transition<'plan>(
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

fn apply_static_assignment(
    target: &Expression,
    value: &Expression,
    aliases: &[(String, String)],
    values: &mut Vec<(String, String)>,
) {
    let Some(target_name) = shallow_canonical_place_name(target, aliases) else {
        return;
    };

    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            let field_target = format!("{target_name}::{}", field.name);
            if let Some(source_name) = canonical_place_name(&field.value, aliases) {
                copy_static_prefix(values, &source_name, &field_target);
            }
            if let Some(field_value) = resolve_static_value(&field.value, aliases, values) {
                set_static_value(values, field_target, field_value);
            }
        }
        return;
    }

    if let Some(source_name) = canonical_place_name(value, aliases) {
        copy_static_prefix(values, &source_name, &target_name);
    }

    let Some(value) = resolve_static_value(value, aliases, values) else {
        return;
    };

    set_static_value(values, target_name, value);
}

fn copy_static_prefix(values: &mut Vec<(String, String)>, source_name: &str, target_name: &str) {
    let source_prefix = format!("{source_name}::");
    let copied_values = values
        .iter()
        .filter_map(|(existing_name, value)| {
            existing_name
                .strip_prefix(&source_prefix)
                .map(|suffix| (format!("{target_name}::{suffix}"), value.clone()))
        })
        .collect::<Vec<_>>();

    for (copied_name, copied_value) in copied_values {
        set_static_value(values, copied_name, copied_value);
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

fn canonical_place_name(expression: &Expression, aliases: &[(String, String)]) -> Option<String> {
    let name = match expression {
        Expression::Mutable(inner_expression) => {
            return canonical_place_name(inner_expression, aliases);
        }
        Expression::Name(_) | Expression::Indexed(_) => expression.display_name(),
        _ => return None,
    };

    Some(resolve_alias(&name, aliases))
}

fn shallow_canonical_place_name(
    expression: &Expression,
    aliases: &[(String, String)],
) -> Option<String> {
    let name = match expression {
        Expression::Mutable(inner_expression) => {
            return shallow_canonical_place_name(inner_expression, aliases);
        }
        Expression::Name(_) | Expression::Indexed(_) => expression.display_name(),
        _ => return None,
    };

    Some(resolve_alias_once(&name, aliases))
}

fn resolve_alias(name: &str, aliases: &[(String, String)]) -> String {
    let mut resolved = name.to_owned();

    for _ in 0..aliases.len() {
        let Some((alias, target)) = aliases
            .iter()
            .rev()
            .find(|(alias, _)| alias_applies(&resolved, alias))
        else {
            return resolved;
        };

        resolved = replace_alias_prefix(&resolved, alias, target);
    }

    resolved
}

fn resolve_alias_once(name: &str, aliases: &[(String, String)]) -> String {
    aliases
        .iter()
        .rev()
        .find(|(alias, _)| alias_applies(name, alias))
        .map_or_else(
            || name.to_owned(),
            |(alias, target)| replace_alias_prefix(name, alias, target),
        )
}

fn alias_applies(name: &str, alias: &str) -> bool {
    name == alias
        || name.starts_with(&format!("{alias}::"))
        || name.starts_with(&format!("{alias}["))
}

fn replace_alias_prefix(name: &str, alias: &str, target: &str) -> String {
    if name == alias {
        return target.to_owned();
    }

    if let Some(suffix) = name.strip_prefix(&format!("{alias}::")) {
        return format!("{target}::{suffix}");
    }

    if let Some(suffix) = name.strip_prefix(alias) {
        return format!("{target}{suffix}");
    }

    name.to_owned()
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
