use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath,
    TableIndexedExpression, TableNamePath,
};
use omega_typed_program::name::ProgramName;

use super::storage_places::indexed_expression_path;
use omega_runtime_branching::{
    RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchBindingKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeAliasBinding {
    pub(super) source_key: StateKey,
    pub(super) parameter_symbol: SymbolHandle,
    pub(super) parameter_name: ProgramName,
    pub(super) expression_source_key: StateKey,
    pub(super) expression: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeResolvedExpression {
    pub(super) source_key: StateKey,
    pub(super) expression: Expression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeResolvedExpressionHandle {
    pub(super) source_key: StateKey,
    pub(super) expression: ExpressionHandle,
}

pub(super) fn set_runtime_alias(
    aliases: &mut Vec<RuntimeAliasBinding>,
    alias: RuntimeAliasBinding,
) {
    if let Some(existing_alias) = aliases.iter_mut().find(|existing_alias| {
        existing_alias.source_key == alias.source_key
            && existing_alias.parameter_symbol == alias.parameter_symbol
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

pub(super) fn strip_mutable_expression_handle(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => *target,
        _ => expression,
    }
}

pub(super) fn resolve_runtime_alias_expression(
    expression: &Expression,
    source_key: StateKey,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
) -> Expression {
    resolve_runtime_alias_binding(expression, source_key, aliases, alias_expressions).expression
}

pub(super) fn resolve_runtime_alias_binding(
    expression: &Expression,
    source_key: StateKey,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
) -> RuntimeResolvedExpression {
    match expression {
        Expression::Mutable(target) => {
            let resolved =
                resolve_runtime_alias_binding(target, source_key, aliases, alias_expressions);
            RuntimeResolvedExpression {
                source_key: resolved.source_key,
                expression: Expression::Mutable(Box::new(resolved.expression)),
            }
        }
        Expression::Indexed(indexed) => {
            let collection = resolve_runtime_alias_binding(
                &indexed.collection,
                source_key,
                aliases,
                alias_expressions,
            );
            let index = resolve_runtime_alias_binding(
                &indexed.index,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpression {
                source_key: collection.source_key,
                expression: Expression::Indexed(Box::new(
                    omega_typed_program::expression::IndexedExpression {
                        collection: collection.expression,
                        index: index.expression,
                    },
                )),
            }
        }
        Expression::Name(path) if !path.is_empty() => aliases
            .iter()
            .rev()
            .find(|alias| alias.source_key == source_key && alias_matches_path(alias, path))
            .map(|alias| RuntimeResolvedExpression {
                source_key: alias.expression_source_key,
                expression: alias_expressions
                    .to_tree_with_place_suffix(alias.expression, &path[1..]),
            })
            .unwrap_or_else(|| RuntimeResolvedExpression {
                source_key,
                expression: expression.clone(),
            }),
        _ => RuntimeResolvedExpression {
            source_key,
            expression: expression.clone(),
        },
    }
}

pub(super) fn resolve_runtime_alias_binding_handle(
    expression: ExpressionHandle,
    source_key: StateKey,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &mut ExpressionTable,
) -> RuntimeResolvedExpressionHandle {
    match alias_expressions.expression(expression).clone() {
        ExpressionNode::Mutable(target) => {
            let resolved = resolve_runtime_alias_binding_handle(
                target,
                source_key,
                aliases,
                alias_expressions,
            );
            RuntimeResolvedExpressionHandle {
                source_key: resolved.source_key,
                expression: alias_expressions.insert(ExpressionNode::Mutable(resolved.expression)),
            }
        }
        ExpressionNode::Indexed(TableIndexedExpression { collection, index }) => {
            let collection = resolve_runtime_alias_binding_handle(
                collection,
                source_key,
                aliases,
                alias_expressions,
            );
            let index =
                resolve_runtime_alias_binding_handle(index, source_key, aliases, alias_expressions);
            RuntimeResolvedExpressionHandle {
                source_key: collection.source_key,
                expression: alias_expressions.insert(ExpressionNode::Indexed(
                    TableIndexedExpression {
                        collection: collection.expression,
                        index: index.expression,
                    },
                )),
            }
        }
        ExpressionNode::Name(path) if path.members.count() > 0 => aliases
            .iter()
            .rev()
            .find(|alias| alias.source_key == source_key && alias_matches_table_path(alias, &path))
            .map(|alias| RuntimeResolvedExpressionHandle {
                source_key: alias.expression_source_key,
                expression: alias_expressions.insert_copy_with_member_suffix(
                    alias.expression,
                    path.members,
                    1,
                ),
            })
            .unwrap_or(RuntimeResolvedExpressionHandle {
                source_key,
                expression,
            }),
        _ => RuntimeResolvedExpressionHandle {
            source_key,
            expression,
        },
    }
}

pub(super) fn resolve_leaf_binding_expression(
    table: &ExpressionTable,
    expression: &Expression,
    bindings: &[RuntimeLeafBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_leaf_binding_expression(table, target, bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => bindings
            .iter()
            .find(|binding| {
                leaf_binding_matches_path(binding, path)
                    && binding.kind == RuntimeLeafBranchBindingKind::LeafParameter
            })
            .or_else(|| {
                bindings
                    .iter()
                    .find(|binding| leaf_binding_matches_path(binding, path))
            })
            .map(|binding| table.to_tree_with_place_suffix(binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

pub(super) fn resolve_straight_line_binding_expression(
    table: &ExpressionTable,
    expression: &Expression,
    bindings: &[RuntimeStraightLineBranchBinding],
) -> Expression {
    match expression {
        Expression::Mutable(target) => {
            let resolved_target = resolve_straight_line_binding_expression(table, target, bindings);
            if matches!(resolved_target, Expression::Mutable(_)) {
                resolved_target
            } else {
                Expression::Mutable(Box::new(resolved_target))
            }
        }
        Expression::Name(path) if !path.is_empty() => bindings
            .iter()
            .find(|binding| {
                straight_line_binding_matches_path(binding, path)
                    && binding.kind == RuntimeStraightLineBranchBindingKind::TargetParameter
            })
            .or_else(|| {
                bindings
                    .iter()
                    .find(|binding| straight_line_binding_matches_path(binding, path))
            })
            .map(|binding| table.to_tree_with_place_suffix(binding.expression, &path[1..]))
            .unwrap_or_else(|| expression.clone()),
        _ => expression.clone(),
    }
}

fn alias_matches_path(alias: &RuntimeAliasBinding, path: &NamePath) -> bool {
    symbol_matches_path(alias.parameter_symbol, path)
}

fn alias_matches_table_path(alias: &RuntimeAliasBinding, path: &TableNamePath) -> bool {
    alias.parameter_symbol.is_valid()
        && path.head_symbol.is_valid()
        && alias.parameter_symbol == path.head_symbol
}

fn leaf_binding_matches_path(binding: &RuntimeLeafBranchBinding, path: &NamePath) -> bool {
    symbol_matches_path(binding.parameter_symbol, path)
}

fn straight_line_binding_matches_path(
    binding: &RuntimeStraightLineBranchBinding,
    path: &NamePath,
) -> bool {
    symbol_matches_path(binding.parameter_symbol, path)
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
