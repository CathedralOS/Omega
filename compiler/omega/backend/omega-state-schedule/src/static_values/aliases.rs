pub(crate) use omega_platform_interface::PlaceKey;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

pub(crate) fn argument_binding_place_key(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<PlaceKey> {
    match table.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            shallow_canonical_place_key(table, *inner_expression, aliases)
        }
        _ => canonical_place_key(table, expression, aliases),
    }
}

pub(crate) fn canonical_place_key(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<PlaceKey> {
    let key = match table.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            return canonical_place_key(table, *inner_expression, aliases);
        }
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) => {
            PlaceKey::from_expression_handle(table, expression)?
        }
        _ => return None,
    };

    Some(resolve_alias(&key, aliases))
}

pub(crate) fn shallow_canonical_place_key(
    table: &ExpressionTable,
    expression: ExpressionHandle,
    aliases: &[(PlaceKey, PlaceKey)],
) -> Option<PlaceKey> {
    let key = match table.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            return shallow_canonical_place_key(table, *inner_expression, aliases);
        }
        ExpressionNode::Name(_) | ExpressionNode::Indexed(_) => {
            PlaceKey::from_expression_handle(table, expression)?
        }
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
