pub(crate) use omega_platform_interface::PlaceKey;
use omega_typed_program::expression::Expression;

pub(crate) fn argument_binding_place_key(
    expression: &Expression,
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<PlaceKey> {
    match expression {
        Expression::Mutable(inner_expression) => {
            shallow_canonical_place_key(inner_expression, aliases)
        }
        _ => canonical_place_key(expression, aliases),
    }
}

pub(crate) fn canonical_place_key(
    expression: &Expression,
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<PlaceKey> {
    let key = match expression {
        Expression::Mutable(inner_expression) => {
            return canonical_place_key(inner_expression, aliases);
        }
        Expression::Name(_) | Expression::Indexed(_) => PlaceKey::from_expression(expression)?,
        _ => return None,
    };

    Some(resolve_alias(&key, aliases))
}

pub(crate) fn shallow_canonical_place_key(
    expression: &Expression,
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<PlaceKey> {
    let key = match expression {
        Expression::Mutable(inner_expression) => {
            return shallow_canonical_place_key(inner_expression, aliases);
        }
        Expression::Name(_) | Expression::Indexed(_) => PlaceKey::from_expression(expression)?,
        _ => return None,
    };

    Some(resolve_alias_once(&key, aliases))
}

fn resolve_alias(key: &PlaceKey, aliases: &[(PlaceKey, PlaceKey)]) -> PlaceKey {
    let mut resolved = key.clone();

    for _ in 0..aliases.len() {
        let Some((alias, target)) = aliases
            .iter()
            .rev()
            .find(|(alias, _)| resolved.starts_with(alias))
        else {
            return resolved;
        };

        resolved = resolved.replace_prefix(alias, target);
    }

    resolved
}

fn resolve_alias_once(key: &PlaceKey, aliases: &[(PlaceKey, PlaceKey)]) -> PlaceKey {
    aliases
        .iter()
        .rev()
        .find(|(alias, _)| key.starts_with(alias))
        .map_or_else(
            || key.clone(),
            |(alias, target)| key.replace_prefix(alias, target),
        )
}
