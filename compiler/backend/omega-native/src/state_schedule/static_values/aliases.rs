use omega_typed_program::expression::Expression;

pub(in crate::state_schedule) fn argument_binding_place_name(
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

pub(in crate::state_schedule) fn canonical_place_name(
    expression: &Expression,
    aliases: &[(String, String)],
) -> Option<String> {
    let name = match expression {
        Expression::Mutable(inner_expression) => {
            return canonical_place_name(inner_expression, aliases);
        }
        Expression::Name(_) | Expression::Indexed(_) => expression.display_name(),
        _ => return None,
    };

    Some(resolve_alias(&name, aliases))
}

pub(in crate::state_schedule) fn shallow_canonical_place_name(
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
