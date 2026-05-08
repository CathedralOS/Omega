use crate::control_flow::StateKey;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

use super::storage_places::indexed_expression_path;
use crate::runtime_dispatch::branching::{
    RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchBindingKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeAliasBinding {
    pub(super) source_key: StateKey,
    pub(super) parameter_name: ProgramName,
    pub(super) expression: Expression,
}

pub(super) fn set_runtime_alias(
    aliases: &mut Vec<RuntimeAliasBinding>,
    alias: RuntimeAliasBinding,
) {
    if let Some(existing_alias) = aliases.iter_mut().find(|existing_alias| {
        existing_alias.source_key == alias.source_key
            && existing_alias.parameter_name == alias.parameter_name
    }) {
        *existing_alias = alias;
    } else {
        aliases.push(alias);
    }
}

pub(super) fn strip_mutable_expression(expression: Expression) -> Expression {
    match expression {
        Expression::Mutable(target) => *target,
        _ => expression,
    }
}

pub(super) fn resolve_runtime_alias_expression(
    expression: &Expression,
    source_key: StateKey,
    aliases: &[RuntimeAliasBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => Expression::Mutable(Box::new(
            resolve_runtime_alias_expression(target, source_key, aliases),
        )),
        Expression::Indexed(indexed) => Expression::Indexed(Box::new(
            omega_typed_program::expression::IndexedExpression {
                collection: resolve_runtime_alias_expression(
                    &indexed.collection,
                    source_key,
                    aliases,
                ),
                index: resolve_runtime_alias_expression(&indexed.index, source_key, aliases),
            },
        )),
        Expression::Name(path) if !path.is_empty() => aliases
            .iter()
            .rev()
            .find(|alias| alias.source_key == source_key && alias.parameter_name == path[0])
            .map(|alias| append_place_suffix(&alias.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

pub(super) fn resolve_leaf_binding_expression(
    expression: &Expression,
    bindings: &[RuntimeLeafBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_leaf_binding_expression(target, bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => bindings
            .iter()
            .find(|binding| {
                binding.parameter_name == path[0]
                    && binding.kind == RuntimeLeafBranchBindingKind::LeafParameter
            })
            .or_else(|| {
                bindings
                    .iter()
                    .find(|binding| binding.parameter_name == path[0])
            })
            .map(|binding| append_place_suffix(&binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

pub(super) fn resolve_straight_line_binding_expression(
    expression: &Expression,
    bindings: &[RuntimeStraightLineBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_straight_line_binding_expression(target, bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => bindings
            .iter()
            .find(|binding| {
                binding.parameter_name == path[0]
                    && binding.kind == RuntimeStraightLineBranchBindingKind::TargetParameter
            })
            .or_else(|| {
                bindings
                    .iter()
                    .find(|binding| binding.parameter_name == path[0])
            })
            .map(|binding| append_place_suffix(&binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

pub(super) fn append_place_suffix(expression: &Expression, suffix: &[ProgramName]) -> Expression {
    if suffix.is_empty() {
        return expression.clone();
    }

    match expression {
        Expression::Name(path) => {
            let mut resolved_path = path.clone();
            resolved_path.extend_from_slice(suffix);
            Expression::Name(resolved_path)
        }
        Expression::Indexed(indexed) => {
            if let Some(mut indexed_path) = indexed_expression_path(indexed) {
                indexed_path.extend_from_slice(suffix);
                Expression::Name(indexed_path)
            } else {
                expression.clone()
            }
        }
        Expression::Mutable(target) => {
            Expression::Mutable(Box::new(append_place_suffix(target, suffix)))
        }
        _ => expression.clone(),
    }
}
