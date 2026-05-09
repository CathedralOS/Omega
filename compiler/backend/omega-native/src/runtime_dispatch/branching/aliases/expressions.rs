use crate::runtime_dispatch::branching::aliases::{BranchParameterBinding, RuntimeBranchAlias};
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::{BinaryExpression, Expression, IndexedExpression, NamePath};
use omega_typed_program::name::ProgramName;

pub(in crate::runtime_dispatch::branching) fn resolve_branch_expression(
    expression: &Expression,
    branch_bindings: &[BranchParameterBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_branch_expression(target, branch_bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => branch_bindings
            .iter()
            .find(|binding| branch_binding_matches_path(binding, path))
            .map(|binding| append_place_suffix(&binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        Expression::Binary(binary) => Expression::Binary(Box::new(BinaryExpression {
            left: resolve_branch_expression(&binary.left, branch_bindings),
            operator: binary.operator,
            right: resolve_branch_expression(&binary.right, branch_bindings),
        })),
        _ => expression.clone(),
    }
}

pub(super) fn resolve_runtime_branch_alias_expression(
    expression: &Expression,
    source_key: StateKey,
    aliases: &[RuntimeBranchAlias],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target =
                resolve_runtime_branch_alias_expression(target, source_key, aliases);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Indexed(indexed) => Expression::Indexed(Box::new(IndexedExpression {
            collection: resolve_runtime_branch_alias_expression(
                &indexed.collection,
                source_key,
                aliases,
            ),
            index: resolve_runtime_branch_alias_expression(&indexed.index, source_key, aliases),
        })),
        Expression::Name(path) if !path.is_empty() => aliases
            .iter()
            .rev()
            .find(|alias| alias.source_key == source_key && alias_matches_path(alias, path))
            .map(|alias| append_place_suffix(&alias.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

fn branch_binding_matches_path(binding: &BranchParameterBinding, path: &NamePath) -> bool {
    symbol_matches_path(binding.parameter_symbol, path)
}

fn alias_matches_path(alias: &RuntimeBranchAlias, path: &NamePath) -> bool {
    symbol_matches_path(alias.parameter_symbol, path)
}

fn symbol_matches_path(symbol: SymbolHandle, path: &NamePath) -> bool {
    symbol.is_valid() && path.head_symbol().is_valid() && symbol == path.head_symbol()
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

fn indexed_expression_path(indexed: &IndexedExpression) -> Option<NamePath> {
    let Expression::Integer(index) = &indexed.index else {
        return None;
    };
    let mut path = match &indexed.collection {
        Expression::Name(path) => path.clone(),
        Expression::Indexed(inner_indexed) => indexed_expression_path(inner_indexed)?,
        _ => return None,
    };
    let last_segment = path.last_mut()?;
    *last_segment = ProgramName::generated(format!("{last_segment}[{index}]"));
    Some(path)
}
